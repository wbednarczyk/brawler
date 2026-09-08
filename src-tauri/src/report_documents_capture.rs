use crate::document_fetcher::{DocumentFetcher, DocumentFetcherError, FetchedDocument};
use crate::storage::{self, CaptureReportDocumentInput, StorageResult};
use serde::Serialize;
use sha2::{Digest, Sha256};

#[derive(Debug, Serialize)]
#[cfg_attr(feature = "ts-export", derive(ts_rs::TS))]
#[cfg_attr(
    feature = "ts-export",
    ts(export, export_to = "../../src/api/generated/")
)]
#[serde(rename_all = "camelCase")]
pub struct DocumentCaptureResult {
    pub document_id: String,
    pub local_path: Option<String>,
    pub success: bool,
    pub error: Option<String>,
}

/// Result of fetching the files for `pending` ESPI/EBI attachment documents (ADR 0036).
#[derive(Debug, Default, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct AttachmentFetchSummary {
    pub stored: usize,
    pub failed: usize,
}

/// Register-and-fetch one report document by URL. **Never downgrades a
/// fetched document** (#455): before any network call, [`resolve_before_fetch`]
/// checks the row's on-disk identity — an already-`fetched` row whose file
/// still matches its stored hash/size is returned as-is, and a `failed` row
/// whose file matches is healed back to `fetched`, in both cases without a
/// fetch. If the fetch itself fails, the failure write is guarded
/// (`mark_report_document_failed`'s atomic guard): a concurrent capture that
/// already stored the file wins, and this call reports that existing
/// document as success rather than "no file".
pub fn capture_report_document(
    state: &crate::storage::AppState,
    fetcher: &dyn DocumentFetcher,
    input: CaptureReportDocumentInput,
) -> StorageResult<DocumentCaptureResult> {
    let doc = state.create_or_find_pending_report_document(input)?;
    let doc = resolve_before_fetch(state, doc)?;
    let doc_id = doc.id.clone();

    if doc.fetch_status == "fetched" {
        return Ok(DocumentCaptureResult {
            document_id: doc_id,
            local_path: doc.local_path,
            success: true,
            error: None,
        });
    }

    match fetcher.fetch(&doc.url) {
        Ok(fetched) => {
            let local_path = store_fetched_document(state, &doc_id, &doc.url, &fetched)?;

            Ok(DocumentCaptureResult {
                document_id: doc_id,
                local_path: Some(local_path),
                success: true,
                error: None,
            })
        }
        Err(err) => {
            let error_msg = err.to_string();
            let after = state.mark_report_document_failed(&doc_id, &error_msg)?;
            if after.fetch_status == "fetched" {
                // The downgrade was declined: a concurrent capture already
                // stored the file. Report that document, never "no file".
                return Ok(DocumentCaptureResult {
                    document_id: doc_id,
                    local_path: after.local_path,
                    success: true,
                    error: None,
                });
            }

            Ok(DocumentCaptureResult {
                document_id: doc_id,
                local_path: None,
                success: false,
                error: Some(error_msg),
            })
        }
    }
}

/// Resolve a document's on-disk identity before a fetch is attempted
/// (#455). A `fetched` row whose file still matches its stored hash/size
/// needs no refetch; a `failed` row whose file matches is healed back to
/// `fetched` (recovering a document a prior unconditional `mark_failed`
/// downgraded even though its bytes were already on disk); a `fetched` row
/// whose file is missing or no longer matches is reset to `pending` so the
/// normal fetch below runs. `pending`/`metadata_only` rows pass through
/// unchanged.
fn resolve_before_fetch(
    state: &crate::storage::AppState,
    doc: crate::storage::ReportDocument,
) -> StorageResult<crate::storage::ReportDocument> {
    resolve_before_fetch_bounded(state, doc, 1)
}

/// [`resolve_before_fetch`], bounded to one retry (#487 P2): the `fetched`
/// branch's reset-to-pending is a compare-and-set on the identity this call
/// inspected (`mark_report_document_pending_for_refetch`'s guard) — a
/// concurrent capture that published a fresh, valid identity for this
/// document between the inspection and the reset must not be erased. When
/// the guard declines, the row that won is re-inspected once more (not
/// looped unboundedly — a live, ongoing race converges on the caller's own
/// next attempt, not inside this call).
fn resolve_before_fetch_bounded(
    state: &crate::storage::AppState,
    doc: crate::storage::ReportDocument,
    retries_left: u8,
) -> StorageResult<crate::storage::ReportDocument> {
    match doc.fetch_status.as_str() {
        "fetched" => {
            if local_file_matches_row(state, &doc).is_some() {
                Ok(doc)
            } else {
                log::warn!(
                    "report document {} marked fetched but its file is missing or no longer \
                     matches its stored hash/size; resetting to pending for refetch",
                    doc.id
                );
                let (after, applied) = state
                    .report_documents()
                    .mark_report_document_pending_for_refetch(
                        &doc.id,
                        &doc.fetch_status,
                        doc.local_path.as_deref(),
                        doc.content_hash.as_deref(),
                    )?;
                if applied || retries_left == 0 {
                    Ok(after)
                } else {
                    // Declined: a concurrent capture published a different
                    // identity since this snapshot. Re-inspect the fresher
                    // row instead of erasing what it just published.
                    resolve_before_fetch_bounded(state, after, retries_left - 1)
                }
            }
        }
        "failed" => {
            if local_file_matches_row(state, &doc).is_some() {
                state
                    .report_documents()
                    .heal_report_document_failed_if_unchanged(
                        &doc.id,
                        doc.local_path.as_deref(),
                        doc.content_type.as_deref(),
                        doc.content_hash.as_deref(),
                        doc.byte_size,
                    )
            } else {
                Ok(doc)
            }
        }
        _ => Ok(doc),
    }
}

/// Whether a document row's `local_path` points at a regular, readable file
/// on disk whose size and SHA-256 match the row's stored `byte_size` /
/// `content_hash` — the **verified identity** check (#455): "a file exists at
/// this path" is not enough, the bytes must be the ones the row claims.
/// Returns the resolved absolute path on a match. A row missing either
/// stored value cannot be verified and is treated as not matching (the
/// conservative choice — refetch rather than trust an unverifiable claim).
fn local_file_matches_row(
    state: &crate::storage::AppState,
    doc: &crate::storage::ReportDocument,
) -> Option<std::path::PathBuf> {
    let local_path = doc.local_path.as_deref()?;
    if local_path.trim().is_empty() {
        return None;
    }
    let expected_size = doc.byte_size?;
    let expected_hash = doc.content_hash.as_deref()?;

    let full_path = state.data_dir().join(local_path);
    let metadata = std::fs::metadata(&full_path).ok()?;
    if !metadata.is_file() || metadata.len() as i64 != expected_size {
        return None;
    }
    let bytes = std::fs::read(&full_path).ok()?;
    (content_hash_hex(&bytes) == expected_hash).then_some(full_path)
}

/// Fetch and store the files for report documents registered `pending` during ingestion
/// (periodic-report ESPI/EBI attachments). Each fetch is throttled; failures are recorded on
/// the document and never abort the batch. Idempotent: already-fetched documents are not
/// re-listed. Used by source refresh and on-track backfill (ADR 0036).
pub fn fetch_pending_attachments(
    state: &crate::storage::AppState,
    fetcher: &dyn DocumentFetcher,
) -> StorageResult<AttachmentFetchSummary> {
    let pending = state.list_pending_attachment_documents()?;
    let mut summary = AttachmentFetchSummary::default();

    for (index, doc) in pending.iter().enumerate() {
        if index > 0 {
            std::thread::sleep(std::time::Duration::from_secs(1));
        }

        match fetcher.fetch(&doc.url) {
            Ok(fetched) => {
                store_fetched_document(state, &doc.id, &doc.url, &fetched)?;
                summary.stored += 1;
            }
            Err(err) => {
                state.mark_report_document_failed(&doc.id, &err.to_string())?;
                summary.failed += 1;
            }
        }
    }

    Ok(summary)
}

/// Fetch and store a single report document by id, returning the updated document.
/// Idempotent: a document whose file still matches its stored identity is returned
/// unchanged (never downgraded, never re-fetched — #455, see
/// [`resolve_before_fetch`]). Used by the report-over-report diff's fetch-on-demand
/// (ADR 0052), so a pending statement can be compared without a full backfill. A
/// fetch failure is recorded on the document and surfaced as an error string, unless
/// a concurrent capture already stored the file (the guarded failure write declines
/// the downgrade and that existing document is returned instead).
pub fn fetch_report_document(
    state: &crate::storage::AppState,
    fetcher: &dyn DocumentFetcher,
    doc_id: &str,
) -> Result<crate::storage::ReportDocument, FetchDocumentError> {
    let document = state
        .get_report_document(doc_id)
        .map_err(FetchDocumentError::fatal)?;
    let document = resolve_before_fetch(state, document).map_err(FetchDocumentError::fatal)?;
    if document.fetch_status == "fetched" {
        return Ok(document); // already fetched (verified identity, or just healed)
    }
    match fetcher.fetch(&document.url) {
        Ok(fetched) => {
            store_fetched_document(state, &document.id, &document.url, &fetched)
                .map_err(FetchDocumentError::fatal)?;
            state
                .get_report_document(doc_id)
                .map_err(FetchDocumentError::fatal)
        }
        Err(err) => {
            let error = FetchDocumentError::from_fetcher(&err);
            if let Ok(after) = state.mark_report_document_failed(&document.id, &error.message) {
                if after.fetch_status == "fetched" {
                    // The downgrade was declined: a concurrent capture
                    // already stored the file.
                    return Ok(after);
                }
            }
            Err(error)
        }
    }
}

/// Failure from [`fetch_report_document`], carrying whether a retry may
/// succeed. Network-level errors (connect, timeout, HTTP status — the
/// [`DocumentFetcherError::Request`] variant) are `transient`; storage errors
/// and structural rejections (too large, bad content type) are not. Callers
/// that don't retry render it via `Display` exactly like the old `String`.
#[derive(Debug)]
pub struct FetchDocumentError {
    pub transient: bool,
    pub message: String,
}

impl FetchDocumentError {
    fn fatal(error: impl std::fmt::Display) -> Self {
        Self {
            transient: false,
            message: error.to_string(),
        }
    }

    fn from_fetcher(error: &DocumentFetcherError) -> Self {
        Self {
            transient: matches!(error, DocumentFetcherError::Request(_)),
            message: error.to_string(),
        }
    }
}

impl std::fmt::Display for FetchDocumentError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(&self.message)
    }
}

/// Write fetched bytes under `report_documents/`, recording the relative path, content type,
/// SHA-256 content hash, byte size, and the **magic-byte container** on the document.
/// Returns the relative `local_path` of whichever identity ends up published — this
/// attempt's, or a concurrent attempt's if this one lost the race (see below).
fn store_fetched_document(
    state: &crate::storage::AppState,
    doc_id: &str,
    url: &str,
    fetched: &FetchedDocument,
) -> StorageResult<String> {
    store_fetched_document_with(state, doc_id, url, fetched, || {})
}

/// [`store_fetched_document`] with a test-only interleaving seam.
/// `before_publish` runs after this attempt's bytes are durably on disk
/// under their own unique temp name (written + fsynced) and right before
/// the no-clobber publish onto the final path — production always passes a
/// no-op closure; a test can run a second, competing capture of the SAME
/// document inside it to deterministically reproduce the concurrent-capture
/// race (#487 P1).
///
/// **Unique temp name + fsync + no-clobber publish**: two concurrent
/// captures of the same document must never share one `.part` name (a
/// shared name lets the slower writer corrupt the faster one's still-in-flight
/// bytes before either publishes), so each attempt writes to
/// `<final>.<pid>-<nanos>.part` and `fsync`s it before publishing. Publishing
/// uses `hard_link` rather than `rename`: `hard_link` fails with
/// `AlreadyExists` instead of silently overwriting, so whichever attempt
/// publishes first keeps its bytes at the final path — a slower attempt can
/// never clobber them. The loser never touches the final path; it deletes
/// its own temp file and reports whichever identity actually won (re-read
/// from the row, converging with the winner's own guarded write below).
///
/// The row update is itself guarded (`mark_report_document_fetched_if_unchanged`,
/// #487 P1): it writes this attempt's identity only if the row is not
/// already `fetched` with a DIFFERENT hash, so even the rare case of two
/// attempts both winning the file race in different runs (e.g. concurrent
/// retention pruning) cannot end with the row claiming an identity that
/// doesn't match the file its own publish just created.
///
/// The server's `content_type` is stored **verbatim** — it is the audit value that
/// makes the "server said X, bytes are Y" mismatch measurable (epic #229 T1). The
/// container is recorded alongside it, from the bytes, so nothing downstream has to
/// trust the extension or the header (T2). If the container write fails the row stays
/// `NULL` and the startup self-heal repairs it, so a fetch is never lost over it.
fn store_fetched_document_with(
    state: &crate::storage::AppState,
    doc_id: &str,
    url: &str,
    fetched: &FetchedDocument,
    before_publish: impl FnOnce(),
) -> StorageResult<String> {
    let extension = determine_extension(&fetched.content_type, url);
    let local_path = format!("report_documents/{doc_id}.{extension}");
    let full_path = state.data_dir().join(&local_path);

    if let Some(parent) = full_path.parent() {
        std::fs::create_dir_all(parent)
            .map_err(|e| storage::StorageError::Json(serde_json::Error::io(e)))?;
    }

    let part_path = {
        let nanos = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .map(|d| d.as_nanos())
            .unwrap_or_default();
        let mut os = full_path.clone().into_os_string();
        os.push(format!(".{}-{nanos}.part", std::process::id()));
        std::path::PathBuf::from(os)
    };

    {
        use std::io::Write;
        let mut file = std::fs::File::create(&part_path)
            .map_err(|e| storage::StorageError::Json(serde_json::Error::io(e)))?;
        file.write_all(&fetched.bytes)
            .map_err(|e| storage::StorageError::Json(serde_json::Error::io(e)))?;
        file.sync_all()
            .map_err(|e| storage::StorageError::Json(serde_json::Error::io(e)))?;
    }

    before_publish();

    let published_here = publish_no_clobber(state, doc_id, &part_path, &full_path)?;

    if !published_here {
        // Lost the race: another attempt's bytes are already at the final
        // path. Never touch them — report whichever identity the row
        // resolves to (the winner's, once its own guarded write lands).
        let winner = state.get_report_document(doc_id)?;
        return Ok(winner.local_path.unwrap_or(local_path));
    }

    let content_hash = content_hash_hex(&fetched.bytes);
    let byte_size = fetched.bytes.len() as i64;

    let published = state
        .report_documents()
        .mark_report_document_fetched_if_unchanged(
            doc_id,
            Some(&local_path),
            fetched.content_type.as_deref(),
            &content_hash,
            byte_size,
        )?;

    if let Err(error) = state.set_report_document_detected_container(
        doc_id,
        crate::fundamentals::extraction::container::detect_container(&fetched.bytes).as_str(),
    ) {
        // Best-effort: the bytes are already on disk and the row already fetched.
        // Losing the fetch over the stamp would be worse than a NULL the startup
        // self-heal repairs on the next start.
        log::warn!("container stamp failed for report document {doc_id}: {error}");
    }

    Ok(published.local_path.unwrap_or(local_path))
}

/// Publish `part_path` onto `full_path` without clobbering a concurrent
/// attempt's already-published bytes (#487 P1). Returns whether THIS call's
/// bytes are the ones now at `full_path`.
///
/// `hard_link` (not `rename`) is the publish primitive: it fails with
/// `AlreadyExists` instead of silently overwriting, so a slower attempt can
/// never clobber a faster one's already-published file. But a plain regular
/// file already sitting at `full_path` is ambiguous — it can be either a
/// concurrent capture's legitimate publish (must not be touched) or this
/// SAME document's own stale bytes left over from an earlier fetch this
/// call is superseding (a corrupted/mismatched file `resolve_before_fetch`
/// already reset the row away from — see its `local_path`-cleared
/// `pending` row). The row breaks the tie: re-read it fresh and check
/// [`local_file_matches_row`] — if the on-disk file still verifies against
/// a currently-`fetched` row, it is a legitimate concurrent publish and
/// this attempt lost the race; otherwise it is orphaned/stale and safe to
/// clear before retrying the publish once.
/// Publish `part` at `full` without clobbering an existing file. `hard_link`
/// refuses an existing target on every platform; where the filesystem has no
/// hard links (exFAT/FAT32 data dirs, some network mounts) fall back to
/// `rename`, which on Windows also refuses an existing target and on Unix
/// replaces it — the row guard (`mark_fetched_if_unchanged`) still keeps the
/// row consistent with whichever bytes landed.
fn link_or_rename(part: &std::path::Path, full: &std::path::Path) -> std::io::Result<()> {
    match std::fs::hard_link(part, full) {
        Ok(()) => Ok(()),
        Err(e) if e.kind() == std::io::ErrorKind::AlreadyExists => Err(e),
        Err(_) => std::fs::rename(part, full),
    }
}

fn publish_no_clobber(
    state: &crate::storage::AppState,
    doc_id: &str,
    part_path: &std::path::Path,
    full_path: &std::path::Path,
) -> StorageResult<bool> {
    match link_or_rename(part_path, full_path) {
        Ok(()) => {
            let _ = std::fs::remove_file(part_path);
            Ok(true)
        }
        Err(e) if e.kind() == std::io::ErrorKind::AlreadyExists => {
            let is_regular_file = std::fs::metadata(full_path)
                .map(|m| m.is_file())
                .unwrap_or(false);
            if !is_regular_file {
                // A directory (or other non-file obstruction) can never be a
                // legitimate publish — surface it, don't silently swallow it
                // as "someone else already fetched it".
                return Err(storage::StorageError::Io(e));
            }

            let fresh_row = state.get_report_document(doc_id)?;
            if local_file_matches_row(state, &fresh_row).is_some() {
                // A concurrent capture legitimately owns this path. Leave it
                // alone; this attempt lost the race.
                let _ = std::fs::remove_file(part_path);
                return Ok(false);
            }

            // Orphaned/stale bytes at this document's own path (no row
            // currently claims them as its verified `fetched` identity) —
            // safe to clear and retry once.
            let _ = std::fs::remove_file(full_path);
            match link_or_rename(part_path, full_path) {
                Ok(()) => {
                    let _ = std::fs::remove_file(part_path);
                    Ok(true)
                }
                Err(e) => Err(storage::StorageError::Io(e)),
            }
        }
        Err(e) => Err(storage::StorageError::Json(serde_json::Error::io(e))),
    }
}

/// SHA-256 lowercase-hex of a byte buffer — shared with the MCP source-blob
/// pin (#384), which must hash exactly the bytes it writes.
pub(crate) fn content_hash_hex(bytes: &[u8]) -> String {
    let mut hasher = Sha256::new();
    hasher.update(bytes);
    let digest = hasher.finalize();
    let mut hex = String::with_capacity(digest.len() * 2);
    for byte in digest {
        hex.push_str(&format!("{byte:02x}"));
    }
    hex
}

fn determine_extension(content_type: &Option<String>, url: &str) -> String {
    // Try to determine from content type
    if let Some(ct) = content_type {
        match ct.as_str() {
            "application/pdf" => return "pdf".to_owned(),
            "text/html" => return "html".to_owned(),
            "application/xhtml+xml" => return "xhtml".to_owned(),
            "text/plain" => return "txt".to_owned(),
            "application/msword" => return "doc".to_owned(),
            "application/vnd.openxmlformats-officedocument.wordprocessingml.document" => {
                return "docx".to_owned()
            }
            "application/vnd.ms-excel" => return "xls".to_owned(),
            "application/vnd.openxmlformats-officedocument.spreadsheetml.sheet" => {
                return "xlsx".to_owned()
            }
            _ => {}
        }
    }

    // Try to determine from the URL's last path segment (ignoring any query string).
    if let Some(path) = url.split('?').next() {
        let segment = path.rsplit('/').next().unwrap_or(path);
        let candidate = if let Some((_, ext)) = segment.rsplit_once('.') {
            // Has a dot: the trailing token is the extension.
            (!ext.is_empty() && ext.len() < 10).then_some(ext)
        } else if !segment.is_empty() && segment.len() < 5 {
            // No dot: accept only a short bare token as an extension-like hint.
            Some(segment)
        } else {
            None
        };
        if let Some(ext) = candidate {
            return ext.to_lowercase();
        }
    }

    // Default extension
    "bin".to_owned()
}

#[cfg(test)]
mod tests;
