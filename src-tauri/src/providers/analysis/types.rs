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

pub trait AiAnalysisProvider {
    fn provider_id(&self) -> &'static str;
    fn model(&self) -> &str;
    fn analyze(
        &self,
        request: &AnalysisRequest,
    ) -> Result<AnalysisProviderOutput, AnalysisProviderError>;

    fn generate_research_brief(
        &self,
        request: &ResearchBriefRequest,
    ) -> Result<ResearchBriefProviderOutput, AnalysisProviderError>;
}
