//! Registry/factory for AI analysis providers.
//!
//! Resolves a provider id to a boxed [`AiAnalysisProvider`] and its credential,
//! replacing the previously duplicated `match provider_id` arms in the analysis
//! jobs. Adding a provider means extending the match arms here only; callers
//! (jobs, commands) depend on the provider-neutral trait, not concrete types.

use super::{
    AiAnalysisProvider, GeminiAnalysisProvider, TestSampleAnalysisProvider,
    TEST_SAMPLE_ANALYSIS_PROVIDER_ID,
};
use crate::providers::credentials;

/// Stable id for the Gemini analysis provider.
pub const GEMINI_ANALYSIS_PROVIDER_ID: &str = "provider_gemini";

/// All analysis provider ids the app can dispatch to (selectable + test sample).
pub fn analysis_provider_ids() -> &'static [&'static str] {
    &[
        GEMINI_ANALYSIS_PROVIDER_ID,
        TEST_SAMPLE_ANALYSIS_PROVIDER_ID,
    ]
}

/// Whether a provider requires an API credential to run.
pub fn analysis_provider_requires_credential(provider_id: &str) -> bool {
    provider_id != TEST_SAMPLE_ANALYSIS_PROVIDER_ID
}

/// Read the configured API key for an analysis provider, if any.
///
/// Returns `None` for keyless providers (the test sample) and for configured
/// providers whose key is not set.
pub fn read_analysis_provider_api_key(provider_id: &str) -> Option<String> {
    credentials::read_provider_api_key(provider_id).unwrap_or(None)
}

/// Build a boxed analysis provider for the given id, credential, model, and timeout.
pub fn build_analysis_provider(
    provider_id: &str,
    api_key: Option<String>,
    model: &str,
    timeout_seconds: i64,
) -> Result<Box<dyn AiAnalysisProvider>, String> {
    match provider_id {
        TEST_SAMPLE_ANALYSIS_PROVIDER_ID => Ok(Box::new(TestSampleAnalysisProvider)),
        GEMINI_ANALYSIS_PROVIDER_ID => Ok(Box::new(
            GeminiAnalysisProvider::live(api_key, model.to_owned(), timeout_seconds)
                .map_err(|error| error.to_string())?,
        )),
        other => Err(format!("Unknown AI analysis provider: {other}")),
    }
}
