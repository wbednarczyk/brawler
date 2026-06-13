use serde::Serialize;
use crate::document_fetcher::DocumentFetcher;
use crate::storage::{self, CaptureReportDocumentInput, StorageResult};

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct DocumentCaptureResult {
    pub document_id: String,
    pub local_path: Option<String>,
    pub success: bool,
    pub error: Option<String>,
}

pub fn capture_report_document(
    state: &crate::storage::AppState,
    fetcher: &dyn DocumentFetcher,
    input: CaptureReportDocumentInput,
) -> StorageResult<DocumentCaptureResult> {
    // Create or find the pending document
    let doc = state.create_or_find_pending_report_document(input)?;
    let doc_id = doc.id.clone();
    let data_dir = state.data_dir().to_path_buf();

    // Try to fetch the document
    match fetcher.fetch(&doc.url) {
        Ok(fetched) => {
            // Determine file extension from content type or URL
            let extension = determine_extension(&fetched.content_type, &doc.url);
            let local_path = format!("report_documents/{}.{}", doc_id, extension);
            let full_path = data_dir.join(&local_path);

            // Create directory if it doesn't exist
            if let Some(parent) = full_path.parent() {
                std::fs::create_dir_all(parent)
                    .map_err(|e| storage::StorageError::Json(serde_json::Error::io(e)))?;
            }

            // Write file to disk
            std::fs::write(&full_path, &fetched.bytes)
                .map_err(|e| storage::StorageError::Json(serde_json::Error::io(e)))?;

            let byte_size = fetched.bytes.len() as i64;

            // Mark as fetched
            state.mark_report_document_fetched(
                &doc_id,
                Some(&local_path),
                fetched.content_type.as_deref(),
                None,
                Some(byte_size),
            )?;

            Ok(DocumentCaptureResult {
                document_id: doc_id,
                local_path: Some(local_path),
                success: true,
                error: None,
            })
        }
        Err(err) => {
            let error_msg = err.to_string();

            // Mark as failed
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

fn determine_extension(content_type: &Option<String>, url: &str) -> String {
    // Try to determine from content type
    if let Some(ct) = content_type {
        match ct.as_str() {
            "application/pdf" => return "pdf".to_owned(),
            "text/html" => return "html".to_owned(),
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

    // Try to determine from URL
    if let Some(path) = url.split('?').next() {
        if let Some(extension) = path.split('.').next_back() {
            if !extension.is_empty() && extension.len() < 10 {
                return extension.to_lowercase();
            }
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
