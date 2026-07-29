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

pub fn capture_report_document(
    state: &crate::storage::AppState,
    fetcher: &dyn DocumentFetcher,
    input: CaptureReportDocumentInput,
) -> StorageResult<DocumentCaptureResult> {
    // Create or find the pending document
    let doc = state.create_or_find_pending_report_document(input)?;
    let doc_id = doc.id.clone();

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
            state.mark_report_document_failed(&doc_id, &error_msg)?;

            Ok(DocumentCaptureResult {
                document_id: doc_id,
                local_path: None,
                success: false,
                error: Some(error_msg),
            })
        }
    }
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
/// Idempotent: a document that already has a stored file is returned unchanged.
/// Used by the report-over-report diff's fetch-on-demand (ADR 0052), so a pending
/// statement can be compared without a full backfill. A fetch failure is recorded
/// on the document and surfaced as an error string.
pub fn fetch_report_document(
    state: &crate::storage::AppState,
    fetcher: &dyn DocumentFetcher,
    doc_id: &str,
) -> Result<crate::storage::ReportDocument, FetchDocumentError> {
    let document = state
        .get_report_document(doc_id)
        .map_err(FetchDocumentError::fatal)?;
    if document
        .local_path
        .as_deref()
        .is_some_and(|path| !path.trim().is_empty())
    {
        return Ok(document); // already fetched
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
            let _ = state.mark_report_document_failed(&document.id, &error.message);
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
/// SHA-256 content hash, and byte size on the document. Returns the relative `local_path`.
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

    std::fs::write(&full_path, &fetched.bytes)
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

    Ok(local_path)
}

fn content_hash_hex(bytes: &[u8]) -> String {
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
}
