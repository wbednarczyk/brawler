use crate::storage::{CompanyLookupResult, TranscriptJob};
use thiserror::Error;

#[derive(Debug, Clone)]
pub struct TranscriptSegmentDraft {
    pub start_seconds: Option<i64>,
    pub end_seconds: Option<i64>,
    pub speaker: Option<String>,
    pub text: String,
    pub language: Option<String>,
}

#[derive(Debug, Clone)]
pub struct TranscriptProviderOutput {
    pub segments: Vec<TranscriptSegmentDraft>,
    pub recognized_company_candidates: Vec<CompanyLookupResult>,
}

#[derive(Debug, Error)]
pub enum TranscriptProviderError {
    #[error("provider is not configured")]
    ProviderNotConfigured,
    #[error("provider limit reached")]
    ProviderLimit,
    #[error("provider is temporarily unavailable: {0}")]
    ProviderUnavailable(String),
    #[error("provider error: {0}")]
    ProviderError(String),
    #[error("network error: {0}")]
    NetworkError(String),
    #[error("invalid source URL")]
    InvalidSourceUrl,
    #[error("parse error: {0}")]
    ParseError(String),
    #[error("unknown provider error: {0}")]
    Unknown(String),
}

impl TranscriptProviderError {
    pub fn code(&self) -> &'static str {
        match self {
            Self::ProviderNotConfigured => "provider_not_configured",
            Self::ProviderLimit => "provider_limit",
            Self::ProviderUnavailable(_) => "provider_unavailable",
            Self::ProviderError(_) => "provider_error",
            Self::NetworkError(_) => "network_error",
            Self::InvalidSourceUrl => "invalid_source_url",
            Self::ParseError(_) => "parse_error",
            Self::Unknown(_) => "unknown",
        }
    }
}

pub trait VideoTranscriptProvider {
    fn provider_id(&self) -> &'static str;
    fn transcribe(
        &self,
        job: &TranscriptJob,
    ) -> Result<TranscriptProviderOutput, TranscriptProviderError>;
}
