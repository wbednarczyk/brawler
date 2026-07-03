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

/// One KPI the extractor should look for, drawn from the company's applicable
/// canonical packs plus its custom KPI definitions (v0.36 AI KPI extraction).
#[derive(Debug, Clone)]
pub struct KpiCatalogEntry {
    pub metric_key: String,
    pub label: String,
    /// `value_kind` from the KPI definition (e.g. currency_amount, ratio, per_share, count).
    pub value_kind: String,
    pub unit: Option<String>,
}

/// Request to extract KPI values from a single report document. The document
/// bytes are delivered separately via [`AiAnalysisProvider::complete_document`];
/// this request carries only the grounding metadata the prompt needs.
#[derive(Debug, Clone)]
pub struct KpiExtractionRequest {
    pub company_name: String,
    /// Statement-type pack hint (industrial, bank, insurer, ...) when known.
    pub statement_type: Option<String>,
    /// KPIs the model should prioritise (canonical + custom for this company).
    pub known_kpis: Vec<KpiCatalogEntry>,
    /// Expected primary period from the triggering event (e.g. "Q3 2025"), when known.
    pub period_hint: Option<String>,
}

/// The primary reporting period the model detected in the document. Only the
/// primary period is extracted; prior-year comparative columns are ignored
/// (they arrive from their own reports).
#[derive(Debug, Clone)]
pub struct ExtractedPeriod {
    pub fiscal_year: i64,
    /// Q1 | Q2 | Q3 | Q4 | H1 | H2 | FY.
    pub period_type: String,
    pub period_end_date: Option<String>,
}

/// One proposed KPI value. Never committed as a fact without explicit user
/// confirmation; every proposal carries the verbatim source snippet it came from.
#[derive(Debug, Clone)]
pub struct ExtractedKpiFact {
    /// Matches a known KPI key, or a model-proposed key when `is_proposed_kpi`.
    pub metric_key: String,
    pub label: String,
    /// Decimal value as text (never a binary float), per the fundamentals data model.
    pub value_numeric: String,
    pub unit: Option<String>,
    pub currency: Option<String>,
    /// The figure exactly as printed (e.g. "142 312"), before scale normalisation.
    pub as_reported_value: Option<String>,
    /// The reported scale word/multiplier (e.g. "tys.", "mln") when present.
    pub as_reported_scale: Option<String>,
    /// Measure window the value represents (e.g. quarter, ytd, ttm) when discernible.
    pub measure_window: Option<String>,
    /// low | medium | high.
    pub confidence: Option<String>,
    pub source_snippet: Option<String>,
    /// True when the metric is beyond the supplied taxonomy (a suggestion the user
    /// may accept as a new custom KPI definition before it is kept).
    pub is_proposed_kpi: bool,
}

/// Provider-neutral KPI extraction output parsed from the model's response.
#[derive(Debug, Clone)]
pub struct KpiExtractionProviderOutput {
    pub period: Option<ExtractedPeriod>,
    /// The document's default reporting currency (ISO 4217) when stated.
    pub currency: Option<String>,
    pub language: Option<String>,
    pub facts: Vec<ExtractedKpiFact>,
}

/// Request to extract management claims from a single report document or transcript.
/// The source text/bytes are delivered separately via [`AiAnalysisProvider::complete_document`].
#[derive(Debug, Clone)]
pub struct ClaimExtractionRequest {
    pub company_name: String,
    /// Human label of the source kind for the prompt ("report document" | "transcript").
    pub source_kind: String,
}

/// One proposed management claim. Never materialized as a claim without explicit
/// user confirmation; every proposal carries the verbatim source snippet it came from.
#[derive(Debug, Clone)]
pub struct ExtractedClaim {
    pub statement: String,
    /// Optional due period the statement points at (e.g. "by Q4 2026").
    pub due_fiscal_year: Option<i64>,
    /// Q1 | Q2 | Q3 | Q4 | H1 | H2 | FY when discernible.
    pub due_period_type: Option<String>,
    /// For a quantitative claim, the metric/comparator/value the promise targets.
    pub target_metric_key: Option<String>,
    pub target_comparator: Option<String>,
    pub target_value_numeric: Option<String>,
    pub target_unit: Option<String>,
    /// low | medium | high.
    pub confidence: Option<String>,
    pub source_snippet: Option<String>,
}

/// Provider-neutral claim extraction output parsed from the model's response.
#[derive(Debug, Clone)]
pub struct ClaimExtractionProviderOutput {
    pub language: Option<String>,
    pub claims: Vec<ExtractedClaim>,
}

/// One candidate category offered to the ESPI classification fallback prompt.
#[derive(Debug, Clone)]
pub struct EspiClassificationCategory {
    pub key: String,
    pub display_name: String,
}

/// The model's classification of one filing. `category` is `None` for the
/// unknown outcome (no confident category), never a guessed type (ADR 0034).
#[derive(Debug, Clone)]
pub struct EspiClassificationOutput {
    pub category: Option<String>,
    pub confidence: f32,
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

    /// Pool failover eligibility (ADR 0061 decision 5): availability failures only —
    /// rate limits, server-side unavailability, and network errors. Client errors
    /// (bad request, missing credential) and valid-200-bad-content parse errors must
    /// surface, never fail over.
    pub fn is_availability_error(&self) -> bool {
        matches!(
            self,
            Self::ProviderLimit | Self::ProviderUnavailable(_) | Self::NetworkError(_)
        )
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn is_availability_error_matches_only_availability_failures() {
        let availability_errors = [
            AnalysisProviderError::ProviderLimit,
            AnalysisProviderError::ProviderUnavailable("temporarily down".to_owned()),
            AnalysisProviderError::NetworkError("connection reset".to_owned()),
        ];
        for error in &availability_errors {
            assert!(
                error.is_availability_error(),
                "{} should be an availability error",
                error.code()
            );
        }

        let non_availability_errors = [
            AnalysisProviderError::ProviderNotConfigured,
            AnalysisProviderError::ProviderError("bad request".to_owned()),
            AnalysisProviderError::ParseError("missing text".to_owned()),
            AnalysisProviderError::Unknown("unexpected".to_owned()),
        ];
        for error in &non_availability_errors {
            assert!(
                !error.is_availability_error(),
                "{} should not be an availability error",
                error.code()
            );
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
