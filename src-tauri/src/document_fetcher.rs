use std::time::Duration;
use thiserror::Error;

const DEFAULT_USER_AGENT: &str = "Brawler/1.0 (+https://github.com/wbednarczyk/brawler)";
const DEFAULT_TIMEOUT_SECS: u64 = 30;
const MAX_DOCUMENT_SIZE: usize = 50 * 1024 * 1024; // 50 MB

#[derive(Debug, Clone)]
pub struct FetchedDocument {
    pub bytes: Vec<u8>,
    pub content_type: Option<String>,
}

#[derive(Debug, Error)]
pub enum DocumentFetcherError {
    #[error("HTTP request failed: {0}")]
    Request(#[from] reqwest::Error),
    #[error("document size {size} exceeds maximum {max}")]
    DocumentTooLarge { size: usize, max: usize },
    #[error("invalid content type: {0}")]
    InvalidContentType(String),
}

pub trait DocumentFetcher {
    fn fetch(&self, url: &str) -> Result<FetchedDocument, DocumentFetcherError>;
}

pub struct HttpDocumentFetcher;

impl HttpDocumentFetcher {
    pub fn new() -> Self {
        Self
    }
}

impl Default for HttpDocumentFetcher {
    fn default() -> Self {
        Self::new()
    }
}

impl DocumentFetcher for HttpDocumentFetcher {
    fn fetch(&self, url: &str) -> Result<FetchedDocument, DocumentFetcherError> {
        let client = reqwest::blocking::Client::builder()
            .user_agent(DEFAULT_USER_AGENT)
            .timeout(Duration::from_secs(DEFAULT_TIMEOUT_SECS))
            .build()?;

        let response = client.get(url).send()?;
        let response = response.error_for_status()?;

        let content_type = response
            .headers()
            .get("content-type")
            .and_then(|v| v.to_str().ok())
            .map(|s| s.to_owned());

        let bytes = response.bytes()?;
        let size = bytes.len();

        if size > MAX_DOCUMENT_SIZE {
            return Err(DocumentFetcherError::DocumentTooLarge {
                size,
                max: MAX_DOCUMENT_SIZE,
            });
        }

        Ok(FetchedDocument {
            bytes: bytes.to_vec(),
            content_type,
        })
    }
}

#[cfg(test)]
pub struct FakeDocumentFetcher {
    pub response: Result<FetchedDocument, DocumentFetcherError>,
}

#[cfg(test)]
impl FakeDocumentFetcher {
    pub fn new_success(bytes: Vec<u8>, content_type: Option<String>) -> Self {
        Self {
            response: Ok(FetchedDocument {
                bytes,
                content_type,
            }),
        }
    }

    pub fn new_error(error: DocumentFetcherError) -> Self {
        Self {
            response: Err(error),
        }
    }
}

#[cfg(test)]
impl DocumentFetcher for FakeDocumentFetcher {
    fn fetch(&self, _url: &str) -> Result<FetchedDocument, DocumentFetcherError> {
        // Since DocumentFetcherError is not Clone, we can't clone the response.
        // The fake fetcher is only used in capture tests where we don't reuse it.
        match &self.response {
            Ok(doc) => Ok(FetchedDocument {
                bytes: doc.bytes.clone(),
                content_type: doc.content_type.clone(),
            }),
            Err(err) => Err(DocumentFetcherError::InvalidContentType(err.to_string())),
        }
    }
}
