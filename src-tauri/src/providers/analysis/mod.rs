mod anthropic;
mod gemini;
mod openai;
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
pub use prompts::{kpi_extraction_prompt, parse_kpi_extraction_output, KPI_EXTRACTION_PROMPT_VERSION};
pub use test_sample::{
    TestSampleAnalysisProvider, TEST_SAMPLE_ANALYSIS_MODEL, TEST_SAMPLE_ANALYSIS_PROVIDER_ID,
};
pub use types::{
    AiAnalysisProvider, AnalysisDocument, AnalysisProviderError, AnalysisProviderOutput,
    AnalysisRequest, AnalysisSourceReference, DocumentSupport, ExtractedKpiFact, ExtractedPeriod,
    KpiCatalogEntry, KpiExtractionProviderOutput, KpiExtractionRequest, ResearchBriefCitationOutput,
    ResearchBriefProviderOutput, ResearchBriefRequest, ResearchBriefSectionOutput,
    ResearchDigestRequest,
};
