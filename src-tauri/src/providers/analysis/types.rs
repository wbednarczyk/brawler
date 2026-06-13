use async_trait::async_trait;

use crate::storage::{FeedItem, ResearchEvidenceItem};
use thiserror::Error;

#[derive(Debug)]
pub struct AnalysisRequest {
    pub feed_item: FeedItem,
    pub prompt_preset_id: String,
    pub custom_question: Option<String>,
}

#[derive(Debug, Clone)]
pub struct AnalysisProviderOutput {
    pub summary: String,
    pub significance: String,
    pub reasoning: String,
    pub language: Option<String>,
    pub tags: Vec<String>,
    pub source_references: Vec<AnalysisSourceReference>,
}

#[derive(Debug, Clone)]
pub struct AnalysisSourceReference {
    pub source_url: String,
    pub label: Option<String>,
}

#[derive(Debug, Clone)]
pub struct ResearchBriefRequest {
    pub scope_type: String,
    pub scope_id: String,
    pub evidence_items: Vec<ResearchEvidenceItem>,
}

#[derive(Debug, Clone)]
pub struct ResearchDigestRequest {
    pub scope_type: String,
    pub scope_id: String,
    pub evidence_items: Vec<ResearchEvidenceItem>,
}

#[derive(Debug, Clone)]
pub struct ResearchBriefProviderOutput {
    pub title: String,
    pub summary: String,
    pub sections: Vec<ResearchBriefSectionOutput>,
    pub language: Option<String>,
    pub citations: Vec<ResearchBriefCitationOutput>,
}

#[derive(Debug, Clone)]
pub struct ResearchBriefSectionOutput {
    pub heading: String,
    pub body: String,
    pub citation_keys: Vec<String>,
}

#[derive(Debug, Clone)]
pub struct ResearchBriefCitationOutput {
    pub citation_key: String,
    pub evidence_type: String,
    pub evidence_id: String,
    pub label: String,
    pub snippet: Option<String>,
}

#[derive(Debug, Error)]
pub enum AnalysisProviderError {
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
    #[error("parse error: {0}")]
    ParseError(String),
    #[error("unknown provider error: {0}")]
    Unknown(String),
}

impl AnalysisProviderError {
    pub fn code(&self) -> &'static str {
        match self {
            Self::ProviderNotConfigured => "provider_not_configured",
            Self::ProviderLimit => "provider_limit",
            Self::ProviderUnavailable(_) => "provider_unavailable",
            Self::ProviderError(_) => "provider_error",
            Self::NetworkError(_) => "network_error",
            Self::ParseError(_) => "parse_error",
            Self::Unknown(_) => "unknown",
        }
    }
}

/// A report document delivered to a provider for grounded analysis/extraction
/// (ADR 0028). The hybrid model carries either native bytes (for providers that
/// ingest files directly) or pre-extracted text as a universal fallback.
#[derive(Debug, Clone)]
pub enum AnalysisDocument {
    /// Native document bytes (e.g. a PDF) plus its MIME type.
    Native { mime_type: String, data: Vec<u8> },
    /// Pre-extracted plain text fallback.
    Text { text: String },
}

/// How a provider can accept report documents (ADR 0028). The v0.36 extraction
/// job uses this to choose whether to send native bytes or extracted text.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum DocumentSupport {
    /// No document input.
    None,
    /// Only pre-extracted text.
    TextOnly,
    /// Native document bytes (and text).
    Native,
}

#[async_trait]
pub trait AiAnalysisProvider: Send + Sync {
    fn provider_id(&self) -> &'static str;
    fn model(&self) -> &str;
    async fn analyze(
        &self,
        request: &AnalysisRequest,
    ) -> Result<AnalysisProviderOutput, AnalysisProviderError>;

    async fn generate_research_brief(
        &self,
        request: &ResearchBriefRequest,
    ) -> Result<ResearchBriefProviderOutput, AnalysisProviderError>;

    async fn generate_research_digest(
        &self,
        request: &ResearchDigestRequest,
    ) -> Result<ResearchBriefProviderOutput, AnalysisProviderError>;

    /// How this provider can accept report documents. Defaults to no support.
    fn document_support(&self) -> DocumentSupport {
        DocumentSupport::None
    }

    /// Send a prompt grounded on a report document and return the raw model text.
    /// The caller parses the result (e.g. the v0.36 extraction job). Defaults to
    /// an error for providers without document support.
    async fn complete_document(
        &self,
        _prompt: &str,
        _document: &AnalysisDocument,
    ) -> Result<String, AnalysisProviderError> {
        Err(AnalysisProviderError::ProviderError(
            "provider does not support document input".to_owned(),
        ))
    }
}
