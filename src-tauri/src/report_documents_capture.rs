use crate::document_fetcher::{DocumentFetcher, DocumentFetcherError, FetchedDocument};
use crate::storage::{self, CaptureReportDocumentInput, StorageResult};
use serde::Serialize;
use sha2::{Digest, Sha256};

/// Per-document capture lock (#487 P1 r3, adversarial review on the
/// atomicity-wave PR): [`super::capture_report_document`],
/// [`super::fetch_report_document`], and [`super::fetch_pending_attachments`]
/// hold this lock across a document's entire capture body (inspect →
/// resolve/heal/reset → fetch → store → row write), so a second caller for
/// the same document waits and then observes the first caller's published
/// row through the normal `fetch_status == "fetched"` short-circuit instead
/// of interleaving with it. Compare-and-set on the row alone cannot close
/// this: it can arbitrate who *wins* a race, but it cannot see a concurrent
/// REPAIR that restores the identical bytes to the same path between another
/// caller's inspection and its own write — only serializing the whole
/// capture closes that window.
///
/// Brawler is a single-instance process (the Tauri single-instance plugin
/// plus the in-process MCP server both run inside the one app process), so
/// this in-process lock is the complete solution — no cross-process lock
/// file or advisory lock is needed.
///
/// Usage: `let doc_lock = doc_lock::lock_document(&doc_id); let _guard =
/// doc_lock.lock().unwrap_or_else(|p| p.into_inner());` — keep both bindings
/// alive for the whole capture body; the guard borrows through the `Arc`, so
/// it must not outlive it.
mod doc_lock {

    use std::collections::HashMap;
    use std::sync::{Arc, LazyLock, Mutex};

    static DOC_LOCKS: LazyLock<Mutex<HashMap<String, Arc<Mutex<()>>>>> =
        LazyLock::new(|| Mutex::new(HashMap::new()));

    /// Returns the mutex serializing captures of report document `id`, creating
    /// it on first use.
    ///
    /// ponytail: entries are never evicted — one `Arc<Mutex<()>>` per document id
    /// ever captured, for the life of the process. Bounded by document count
    /// (thousands, not millions) and negligible per entry; add `Weak`-based
    /// cleanup if that stops holding.
    pub(crate) fn lock_document(id: &str) -> Arc<Mutex<()>> {
        let mut locks = DOC_LOCKS
            .lock()
            .unwrap_or_else(|poisoned| poisoned.into_inner());
        Arc::clone(
            locks
                .entry(id.to_owned())
                .or_insert_with(|| Arc::new(Mutex::new(()))),
        )
    }
}

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
    // `create_or_find_pending_report_document` is its own atomic upsert on
    // the (company_id, url) UNIQUE key, so it needs no lock. Once the
    // document id is known, the ENTIRE rest of this body (inspect →
    // resolve/heal/reset → fetch → store → row write) runs under that
    // document's lock (#487 P1 r3, see `doc_lock`): a second caller for the
    // same document waits here and then takes the `fetch_status ==
    // "fetched"` short-circuit below against the first caller's already
    // -published row, instead of interleaving with it.
    let doc = state.create_or_find_pending_report_document(input)?;
    let doc_id = doc.id.clone();
    let doc_lock = doc_lock::lock_document(&doc_id);
    let _doc_guard = doc_lock.lock().unwrap_or_else(|p| p.into_inner());

    // Re-read UNDER the lock: the upsert's snapshot may predate a capture that
    // completed while this caller waited for the lock (astra r4).
    let doc = state.get_report_document(&doc_id)?;
    let doc = resolve_before_fetch(state, doc)?;

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

        // Same per-document lock as `capture_report_document` /
        // `fetch_report_document` (#487 P1 r3): this batch path writes the
        // same file+row pair those do, so it must not interleave with a
        // concurrent capture of the same document either.
        let doc_lock = doc_lock::lock_document(&doc.id);
        let _doc_guard = doc_lock.lock().unwrap_or_else(|p| p.into_inner());

        // The pending list was read before the lock: re-read and resolve the
        // row now, so a document captured meanwhile is skipped, not re-fetched.
        let doc = resolve_before_fetch(state, state.get_report_document(&doc.id)?)?;
        if doc.fetch_status == "fetched" {
            continue;
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
    // Entire body under this document's lock (#487 P1 r3, see `doc_lock`) —
    // same reasoning as `capture_report_document`.
    let doc_lock = doc_lock::lock_document(doc_id);
    let _doc_guard = doc_lock.lock().unwrap_or_else(|p| p.into_inner());

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
    store_fetched_document_with(state, doc_id, url, fetched, || {}, || {})
}

/// [`store_fetched_document`] with two test-only interleaving seams.
/// `before_publish` runs after this attempt's bytes are durably on disk
/// under their own unique temp name (written + fsynced) and right before
/// the no-clobber publish onto the final path — production always passes a
/// no-op closure; a test can run a second, competing capture of the SAME
/// document inside it to deterministically reproduce the concurrent-capture
/// race (#487 P1). `after_publish` runs once THIS attempt's bytes are the
/// ones at the final path but before the row write that records them —
/// exactly the window #487 P1 r3 (second adversarial review) named: a
/// second caller starting here would, pre-lock, see a `pending` row next to
/// an already-published file and treat the file as stale residue to
/// reclaim. A test drives that window (while still holding the caller's
/// document lock) to prove the lock now makes a second caller wait instead.
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
    after_publish: impl FnOnce(),
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

    after_publish();

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

    // Stamp the container only for the identity the row actually accepted —
    // a declined (stale) publication must not overwrite the winner's stamp.
    if published.content_hash.as_deref() != Some(content_hash.as_str()) {
        return Ok(published.local_path.unwrap_or(local_path));
    }
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

/// Publish `part_path` onto `full_path` without clobbering another
/// process's crash residue (#487 P1 r3). Returns whether THIS call's bytes
/// are the ones now at `full_path`.
///
/// `hard_link` (not `rename`) is the publish primitive: it fails with
/// `AlreadyExists` instead of silently overwriting, so this attempt never
/// clobbers whatever is already there.
///
/// **Under the per-document lock** (`doc_lock`, held by every caller of
/// [`store_fetched_document_with`] for this document's entire capture
/// body), a plain regular file already sitting at `full_path` when this
/// runs is UNAMBIGUOUS: it cannot be a concurrent capture's legitimate,
/// in-flight publish of a DIFFERENT identity — no other in-process caller
/// can be inside this document's capture body at the same time, and Brawler
/// is a single-instance process, so there is no other process either. And
/// it cannot be the row's own currently-verified `fetched` identity either
/// — if it were, `resolve_before_fetch` (run earlier in this same locked
/// call) would already have short-circuited before a fetch was ever
/// attempted. So the only thing it can be is residue: bytes an earlier,
/// now-superseded attempt for this SAME document left behind (a
/// corrupted/mismatched file `resolve_before_fetch` reset the row away from
/// without deleting, or a crash between an old publish and its row write) —
/// safe to clear and retry once.
///
/// The `local_file_matches_row` re-check is kept anyway as defence in depth
/// (mirrors the DB layer's compare-and-set guards, `status.rs`): if it were
/// ever to report a match here, that would mean the lock invariant above
/// was violated by a bug elsewhere, and the safest response is still to
/// leave the file untouched and report the row's identity rather than
/// clobber it.
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
                // Defence in depth only — see the doc comment above. Under
                // the per-document lock this should be unreachable; if hit,
                // treat it the same as a legitimate concurrent owner:
                // leave the file alone and report the row's identity.
                let _ = std::fs::remove_file(part_path);
                return Ok(false);
            }

            // Residue of a superseded earlier attempt for this SAME
            // document (see the doc comment above) — safe to clear and
            // retry once.
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
#[path = "report_documents_capture_tests.rs"]
mod tests;
