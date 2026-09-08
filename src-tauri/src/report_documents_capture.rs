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
                state
                    .report_documents()
                    .mark_report_document_pending_for_refetch(&doc.id)
            }
        }
        "failed" => {
            if local_file_matches_row(state, &doc).is_some() {
                state.mark_report_document_fetched(
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
/// Returns the relative `local_path`.
///
/// **Atomic publish** (#455): bytes land at `<final>.part` first, then a single
/// `rename` publishes them at the final path — the row is updated only after that
/// rename succeeds, so a crash mid-write never leaves a truncated file at the path
/// the row claims (an interrupted publish leaves only the `.part` file behind, and
/// the final path is untouched — whatever it was before).
///
/// The server's `content_type` is stored **verbatim** — it is the audit value that
/// makes the "server said X, bytes are Y" mismatch measurable (epic #229 T1). The
/// container is recorded alongside it, from the bytes, so nothing downstream has to
/// trust the extension or the header (T2). If the container write fails the row stays
/// `NULL` and the startup self-heal repairs it, so a fetch is never lost over it.
fn store_fetched_document(
    state: &crate::storage::AppState,
    doc_id: &str,
    url: &str,
    fetched: &FetchedDocument,
) -> StorageResult<String> {
    let extension = determine_extension(&fetched.content_type, url);
    let local_path = format!("report_documents/{doc_id}.{extension}");
    let full_path = state.data_dir().join(&local_path);

    if let Some(parent) = full_path.parent() {
        std::fs::create_dir_all(parent)
            .map_err(|e| storage::StorageError::Json(serde_json::Error::io(e)))?;
    }

    let mut part_path_os = full_path.clone().into_os_string();
    part_path_os.push(".part");
    let part_path = std::path::PathBuf::from(part_path_os);

    std::fs::write(&part_path, &fetched.bytes)
        .map_err(|e| storage::StorageError::Json(serde_json::Error::io(e)))?;
    std::fs::rename(&part_path, &full_path)
        .map_err(|e| storage::StorageError::Json(serde_json::Error::io(e)))?;

    let content_hash = content_hash_hex(&fetched.bytes);
    let byte_size = fetched.bytes.len() as i64;

    state.mark_report_document_fetched(
        doc_id,
        Some(&local_path),
        fetched.content_type.as_deref(),
        Some(&content_hash),
        Some(byte_size),
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

    Ok(local_path)
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
mod tests {
    use super::*;

    #[test]
    fn determine_extension_from_content_type() {
        assert_eq!(
            determine_extension(&Some("application/pdf".to_owned()), "http://example.com"),
            "pdf"
        );
        assert_eq!(
            determine_extension(&Some("text/html".to_owned()), "http://example.com"),
            "html"
        );
        // Structured ESEF/iXBRL statements (ADR 0061 decision 1b).
        assert_eq!(
            determine_extension(
                &Some("application/xhtml+xml".to_owned()),
                "http://example.com"
            ),
            "xhtml"
        );
    }

    #[test]
    fn determine_extension_from_url() {
        assert_eq!(
            determine_extension(&None, "http://example.com/document.pdf"),
            "pdf"
        );
        assert_eq!(
            determine_extension(&None, "http://example.com/doc?v=1"),
            "doc"
        );
    }

    #[test]
    fn determine_extension_default() {
        assert_eq!(
            determine_extension(&None, "http://example.com/document"),
            "bin"
        );
    }

    /// Epic #229 T2: the fetch path records what the bytes REALLY are. The
    /// maintainer's corpus stores 38 XML documents under a `.pdf` name served as
    /// `application/pdf` — name and header agree with each other and both lie.
    /// The stored `content_type` stays verbatim (it is the audit value); the new
    /// `detected_container` carries the truth.
    #[test]
    fn store_time_sniff_records_the_real_container_not_the_lying_name() {
        use crate::document_fetcher::FakeDocumentFetcher;
        use crate::storage::{AppState, CaptureReportDocumentInput};

        let dir = std::env::temp_dir().join(format!(
            "brawler-capture-sniff-{}-{}",
            std::process::id(),
            "xhtml-under-pdf"
        ));
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(&dir).expect("data dir");
        let connection = crate::storage::open_in_memory_database().expect("db");
        connection
            .execute(
                "INSERT INTO companies (id, exchange, ticker, qualified_ticker, display_name)
                 VALUES ('c1', 'gpw', 'ABC', 'GPW:ABC', 'ABC SA')",
                [],
            )
            .expect("company");
        let state = AppState::with_data_dir(connection, dir);

        // The real pdf2htmlEX shape from the corpus: XHTML bytes, `.pdf` URL,
        // `application/pdf` header.
        let fetcher = FakeDocumentFetcher::new_success(
            b"<?xml version=\"1.0\" encoding=\"utf-8\"?>\n<!-- Created by pdf2htmlEX -->\n<html></html>"
                .to_vec(),
            Some("application/pdf".to_owned()),
        );
        let captured = capture_report_document(
            &state,
            &fetcher,
            CaptureReportDocumentInput {
                company_id: "c1".to_owned(),
                source_type: "espi_attachment".to_owned(),
                url: "https://x/raport-okresowy.pdf".to_owned(),
                period_id: None,
                origin_ref: None,
                title: Some("Raport okresowy".to_owned()),
                attribution: None,
            },
        )
        .expect("capture");
        assert!(captured.success);

        let document = state
            .get_report_document(&captured.document_id)
            .expect("document");
        assert_eq!(
            document.detected_container,
            Some("xml".to_owned()),
            "the fetch path must record the magic-byte container, not the .pdf name"
        );
        assert_eq!(
            document.content_type,
            Some("application/pdf".to_owned()),
            "the server's content type stays verbatim — it is the audit value"
        );
        assert_eq!(
            crate::report_documents_container::resolved_source_format(&document),
            Some(crate::report_diff::extraction::SourceFormat::Xhtml),
            "container truth must beat the extension for every downstream consumer"
        );
    }

    // -----------------------------------------------------------------
    // #455: capture never downgrades a fetched document; verified identity
    // decides heal-vs-refetch, never "bytes on disk win" unchecked.
    // -----------------------------------------------------------------

    fn fresh_capture_state(suffix: &str) -> (crate::storage::AppState, String) {
        use crate::storage::AppState;

        let dir = std::env::temp_dir().join(format!(
            "brawler-capture-455-{}-{suffix}",
            std::process::id()
        ));
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(&dir).expect("data dir");
        let connection = crate::storage::open_in_memory_database().expect("db");
        connection
            .execute(
                "INSERT INTO companies (id, exchange, ticker, qualified_ticker, display_name)
                 VALUES ('c1', 'gpw', 'ABC', 'GPW:ABC', 'ABC SA')",
                [],
            )
            .expect("company");
        (AppState::with_data_dir(connection, dir), "c1".to_owned())
    }

    fn capture_input(company_id: &str, url: &str) -> crate::storage::CaptureReportDocumentInput {
        crate::storage::CaptureReportDocumentInput {
            company_id: company_id.to_owned(),
            source_type: "user_url".to_owned(),
            url: url.to_owned(),
            period_id: None,
            origin_ref: None,
            title: None,
            attribution: None,
        }
    }

    #[test]
    fn capture_returns_the_fetched_row_without_refetching() {
        use crate::document_fetcher::FakeDocumentFetcher;

        let (state, company_id) = fresh_capture_state("no-refetch");
        let first_fetcher = FakeDocumentFetcher::new_success(
            b"%PDF-1.7 real bytes".to_vec(),
            Some("application/pdf".to_owned()),
        );
        let first = capture_report_document(
            &state,
            &first_fetcher,
            capture_input(&company_id, "https://x/report.pdf"),
        )
        .expect("first capture");
        assert!(first.success);

        let second_fetcher = FakeDocumentFetcher::new_error(
            crate::document_fetcher::DocumentFetcherError::InvalidUrl("must not be called".into()),
        );
        let second = capture_report_document(
            &state,
            &second_fetcher,
            capture_input(&company_id, "https://x/report.pdf"),
        )
        .expect("second capture");
        assert!(
            second.success,
            "an already-fetched matching file must report success"
        );
        assert_eq!(second.document_id, first.document_id);
        assert_eq!(second.local_path, first.local_path);
        assert_eq!(
            second_fetcher.calls.get(),
            0,
            "a matching identity must never trigger a network fetch"
        );
    }

    #[test]
    fn capture_heals_a_failed_row_whose_file_matches_its_hash() {
        use crate::document_fetcher::FakeDocumentFetcher;

        let (state, company_id) = fresh_capture_state("heal-failed");
        let first_fetcher = FakeDocumentFetcher::new_success(
            b"%PDF-1.7 real bytes".to_vec(),
            Some("application/pdf".to_owned()),
        );
        let first = capture_report_document(
            &state,
            &first_fetcher,
            capture_input(&company_id, "https://x/report.pdf"),
        )
        .expect("first capture");
        assert!(first.success);

        // Simulate the pre-#455 bug: an unconditional mark_failed downgraded
        // an already-fetched row while its file/hash stayed on disk.
        {
            let raw = state.checkout_for_tests().expect("raw connection");
            raw.execute(
                "UPDATE report_documents SET fetch_status = 'failed', fetch_error = 'stale error' WHERE id = ?1",
                rusqlite::params![first.document_id],
            )
            .expect("seed the downgraded state");
        }

        let second_fetcher = FakeDocumentFetcher::new_error(
            crate::document_fetcher::DocumentFetcherError::InvalidUrl("must not be called".into()),
        );
        let second = capture_report_document(
            &state,
            &second_fetcher,
            capture_input(&company_id, "https://x/report.pdf"),
        )
        .expect("healing capture");
        assert!(
            second.success,
            "a matching-hash failed row must heal to fetched"
        );
        assert_eq!(
            second_fetcher.calls.get(),
            0,
            "healing a verified-identity row must never trigger a network fetch"
        );

        let healed = state
            .get_report_document(&first.document_id)
            .expect("document");
        assert_eq!(healed.fetch_status, "fetched");
        assert!(
            healed.fetch_error.is_none(),
            "the stale error must be cleared on heal"
        );
    }

    #[test]
    fn capture_refetches_when_the_file_is_missing() {
        use crate::document_fetcher::FakeDocumentFetcher;

        let (state, company_id) = fresh_capture_state("missing-file");
        let first_fetcher = FakeDocumentFetcher::new_success(
            b"%PDF-1.7 real bytes".to_vec(),
            Some("application/pdf".to_owned()),
        );
        let first = capture_report_document(
            &state,
            &first_fetcher,
            capture_input(&company_id, "https://x/report.pdf"),
        )
        .expect("first capture");
        assert!(first.success);
        let full_path = state.data_dir().join(first.local_path.as_ref().unwrap());
        std::fs::remove_file(&full_path).expect("delete the stored file");

        let second_fetcher = FakeDocumentFetcher::new_success(
            b"%PDF-1.7 refetched bytes".to_vec(),
            Some("application/pdf".to_owned()),
        );
        let second = capture_report_document(
            &state,
            &second_fetcher,
            capture_input(&company_id, "https://x/report.pdf"),
        )
        .expect("refetch capture");
        assert!(second.success);
        assert_eq!(
            second_fetcher.calls.get(),
            1,
            "a missing file must trigger exactly one refetch"
        );

        let refetched = state
            .get_report_document(&first.document_id)
            .expect("document");
        assert_eq!(refetched.fetch_status, "fetched");
        assert!(
            std::fs::exists(state.data_dir().join(refetched.local_path.unwrap())).unwrap_or(false)
        );
    }

    #[test]
    fn capture_refetches_when_the_file_hash_mismatches() {
        use crate::document_fetcher::FakeDocumentFetcher;

        let (state, company_id) = fresh_capture_state("hash-mismatch");
        let first_fetcher = FakeDocumentFetcher::new_success(
            b"%PDF-1.7 real bytes".to_vec(),
            Some("application/pdf".to_owned()),
        );
        let first = capture_report_document(
            &state,
            &first_fetcher,
            capture_input(&company_id, "https://x/report.pdf"),
        )
        .expect("first capture");
        assert!(first.success);
        let full_path = state.data_dir().join(first.local_path.as_ref().unwrap());
        std::fs::write(&full_path, b"truncated garbage").expect("corrupt the stored file");

        let second_fetcher = FakeDocumentFetcher::new_success(
            b"%PDF-1.7 refetched bytes".to_vec(),
            Some("application/pdf".to_owned()),
        );
        let second = capture_report_document(
            &state,
            &second_fetcher,
            capture_input(&company_id, "https://x/report.pdf"),
        )
        .expect("refetch capture");
        assert!(second.success);
        assert_eq!(
            second_fetcher.calls.get(),
            1,
            "a hash mismatch must trigger exactly one refetch"
        );

        let refetched_bytes =
            std::fs::read(state.data_dir().join(second.local_path.unwrap())).expect("bytes");
        assert_eq!(refetched_bytes, b"%PDF-1.7 refetched bytes");
    }

    #[test]
    fn an_interrupted_publish_leaves_no_partial_final_file() {
        use crate::document_fetcher::FakeDocumentFetcher;
        use crate::storage::CaptureReportDocumentInput;

        let (state, company_id) = fresh_capture_state("interrupted-publish");
        let doc = state
            .create_or_find_pending_report_document(CaptureReportDocumentInput {
                company_id: company_id.clone(),
                source_type: "user_url".to_owned(),
                url: "https://x/report.pdf".to_owned(),
                period_id: None,
                origin_ref: None,
                title: None,
                attribution: None,
            })
            .expect("pending document");

        // Pre-create the FINAL path as a directory so the atomic rename step
        // fails after the `.part` write succeeds — proving the final path is
        // only ever touched by that rename, never a direct partial write.
        let final_path = state
            .data_dir()
            .join(format!("report_documents/{}.pdf", doc.id));
        std::fs::create_dir_all(&final_path).expect("pre-create final path as a directory");

        let fetcher = FakeDocumentFetcher::new_success(
            b"%PDF-1.7 real bytes".to_vec(),
            Some("application/pdf".to_owned()),
        );
        let fetched = fetcher.fetch(&doc.url).expect("fake fetch");
        let result = store_fetched_document(&state, &doc.id, &doc.url, &fetched);
        assert!(result.is_err(), "the rename onto a directory must fail");

        let part_path = state
            .data_dir()
            .join(format!("report_documents/{}.pdf.part", doc.id));
        assert!(
            part_path.exists(),
            "the .part file must remain after a failed publish"
        );
        assert!(
            final_path.is_dir(),
            "the final path must never become a partial file — it stays whatever it was before the failed rename"
        );
    }
}
