mod anthropic;
pub mod capabilities;
mod gate;
mod gemini;
mod openai;
pub mod pool;
mod prompts;
pub mod registry;
mod test_sample;
mod types;

pub use anthropic::{ClaudeAnalysisProvider, DEFAULT_ANTHROPIC_ANALYSIS_MODEL};
pub use gemini::{
    GeminiAnalysisGenerateContentClient, GeminiAnalysisGenerateContentRequest,
    GeminiAnalysisGenerateContentResponse, GeminiAnalysisProvider, DEFAULT_GEMINI_ANALYSIS_MODEL,
};
pub use openai::{OpenAiAnalysisProvider, DEFAULT_OPENAI_ANALYSIS_MODEL};
pub use prompts::{
    claim_extraction_prompt, espi_classification_prompt, espi_event_date_prompt,
    kpi_extraction_prompt, parse_claim_extraction_output, parse_espi_classification_output,
    parse_espi_event_date_output, parse_kpi_extraction_output, CLAIM_EXTRACTION_PROMPT_VERSION,
    ESPI_CLASSIFICATION_PROMPT_VERSION, KPI_EXTRACTION_PROMPT_VERSION,
};
pub use test_sample::{
    TestSampleAnalysisProvider, TEST_SAMPLE_ANALYSIS_MODEL, TEST_SAMPLE_ANALYSIS_PROVIDER_ID,
    TEST_SAMPLE_IR_PICK_URL,
};
pub use types::{
    AiAnalysisProvider, AnalysisDocument, AnalysisProviderError, AnalysisProviderOutput,
    AnalysisRequest, AnalysisSourceReference, ClaimExtractionProviderOutput,
    ClaimExtractionRequest, DocumentSupport, EspiClassificationCategory, EspiClassificationOutput,
    ExtractedClaim, ExtractedKpiFact, ExtractedPeriod, KpiCatalogEntry,
    KpiExtractionProviderOutput, KpiExtractionRequest, ResearchBriefCitationOutput,
    ResearchBriefProviderOutput, ResearchBriefRequest, ResearchBriefSectionOutput,
    ResearchDigestRequest,
};
