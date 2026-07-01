//! Registry/factory for AI analysis providers.
//!
//! Resolves a provider id to a boxed [`AiAnalysisProvider`] and its credential,
//! replacing the previously duplicated `match provider_id` arms in the analysis
//! jobs. Adding a provider means extending the match arms here only; callers
//! (jobs, commands) depend on the provider-neutral trait, not concrete types.

use super::{
    AiAnalysisProvider, ClaudeAnalysisProvider, GeminiAnalysisProvider, OpenAiAnalysisProvider,
    TestSampleAnalysisProvider, TEST_SAMPLE_ANALYSIS_PROVIDER_ID,
};
use crate::providers::credentials;

/// Stable id for the Gemini analysis provider.
pub const GEMINI_ANALYSIS_PROVIDER_ID: &str = "provider_gemini";
/// Stable id for the Anthropic (Claude) analysis provider.
pub const ANTHROPIC_ANALYSIS_PROVIDER_ID: &str = "provider_anthropic";
/// Stable id for the OpenAI (ChatGPT) analysis provider.
pub const OPENAI_ANALYSIS_PROVIDER_ID: &str = "provider_openai";

/// A user-selectable analysis provider with its curated model list and default.
pub struct AnalysisProviderCatalogEntry {
    pub provider_id: &'static str,
    pub label: &'static str,
    pub models: &'static [&'static str],
    pub default_model: &'static str,
    pub requires_credential: bool,
}

/// The single source of truth for selectable analysis providers and their models
/// (ADR 0028). Drives settings validation and the settings selection UI. Exact
/// model ids are curated here; defaults are the balanced mid-tier per provider.
pub fn analysis_provider_catalog() -> &'static [AnalysisProviderCatalogEntry] {
    &[
        AnalysisProviderCatalogEntry {
            provider_id: GEMINI_ANALYSIS_PROVIDER_ID,
            label: "Gemini",
            models: &[
                "gemini-3.5-flash",
                "gemini-3.1-pro-preview",
                "gemini-2.5-flash",
                "gemini-2.5-flash-lite",
            ],
            default_model: "gemini-3.5-flash",
            requires_credential: true,
        },
        AnalysisProviderCatalogEntry {
            provider_id: ANTHROPIC_ANALYSIS_PROVIDER_ID,
            label: "Claude (Anthropic)",
            models: &[
                "claude-sonnet-4-6",
                "claude-opus-4-8",
                "claude-haiku-4-5-20251001",
            ],
            default_model: "claude-sonnet-4-6",
            requires_credential: true,
        },
        AnalysisProviderCatalogEntry {
            provider_id: OPENAI_ANALYSIS_PROVIDER_ID,
            label: "OpenAI (ChatGPT)",
            models: &["gpt-5.5", "gpt-5.1"],
            default_model: "gpt-5.5",
            requires_credential: true,
        },
    ]
}

/// All selectable analysis provider ids (catalog order; excludes the test sample).
pub fn selectable_analysis_provider_ids() -> Vec<&'static str> {
    analysis_provider_catalog()
        .iter()
        .map(|entry| entry.provider_id)
        .collect()
}

/// Union of all curated analysis model ids across providers.
pub fn analysis_model_ids() -> Vec<&'static str> {
    analysis_provider_catalog()
        .iter()
        .flat_map(|entry| entry.models.iter().copied())
        .collect()
}

/// All analysis provider ids the app can dispatch to (selectable + test sample).
pub fn analysis_provider_ids() -> &'static [&'static str] {
    &[
        GEMINI_ANALYSIS_PROVIDER_ID,
        ANTHROPIC_ANALYSIS_PROVIDER_ID,
        OPENAI_ANALYSIS_PROVIDER_ID,
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
        ANTHROPIC_ANALYSIS_PROVIDER_ID => Ok(Box::new(
            ClaudeAnalysisProvider::live(api_key, model.to_owned(), timeout_seconds)
                .map_err(|error| error.to_string())?,
        )),
        OPENAI_ANALYSIS_PROVIDER_ID => Ok(Box::new(
            OpenAiAnalysisProvider::live(api_key, model.to_owned(), timeout_seconds)
                .map_err(|error| error.to_string())?,
        )),
        other => Err(format!("Unknown AI analysis provider: {other}")),
    }
}

/// Wrap a built provider with a per-provider concurrency gate (ADR 0059): the
/// returned provider holds a permit from `semaphore` for the duration of each model
/// call, so at most `semaphore`'s permit count of calls to this provider run at
/// once. See [`super::gate::GatedAnalysisProvider`].
pub fn gate_analysis_provider(
    provider: Box<dyn AiAnalysisProvider>,
    semaphore: std::sync::Arc<tokio::sync::Semaphore>,
) -> Box<dyn AiAnalysisProvider> {
    Box::new(super::gate::GatedAnalysisProvider::new(provider, semaphore))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn catalog_lists_providers_with_valid_defaults() {
        let catalog = analysis_provider_catalog();
        assert_eq!(catalog.len(), 3);
        for entry in catalog {
            assert!(
                entry.models.contains(&entry.default_model),
                "default model for {} must be in its model list",
                entry.provider_id
            );
            assert!(entry.requires_credential);
        }
    }

    #[test]
    fn model_ids_include_provider_defaults() {
        let models = analysis_model_ids();
        assert!(models.contains(&"gemini-3.5-flash"));
        assert!(models.contains(&"claude-sonnet-4-6"));
        assert!(models.contains(&"gpt-5.5"));
    }

    #[test]
    fn builds_each_known_provider_and_rejects_unknown() {
        assert!(
            build_analysis_provider(GEMINI_ANALYSIS_PROVIDER_ID, None, "gemini-3.5-flash", 90)
                .is_ok()
        );
        assert!(build_analysis_provider(
            ANTHROPIC_ANALYSIS_PROVIDER_ID,
            None,
            "claude-sonnet-4-6",
            90
        )
        .is_ok());
        assert!(build_analysis_provider(OPENAI_ANALYSIS_PROVIDER_ID, None, "gpt-5.5", 90).is_ok());
        assert!(
            build_analysis_provider(TEST_SAMPLE_ANALYSIS_PROVIDER_ID, None, "test", 90).is_ok()
        );
        assert!(build_analysis_provider("provider_unknown", None, "x", 90).is_err());
    }

    #[test]
    fn only_real_providers_require_credentials() {
        assert!(analysis_provider_requires_credential(
            GEMINI_ANALYSIS_PROVIDER_ID
        ));
        assert!(analysis_provider_requires_credential(
            ANTHROPIC_ANALYSIS_PROVIDER_ID
        ));
        assert!(analysis_provider_requires_credential(
            OPENAI_ANALYSIS_PROVIDER_ID
        ));
        assert!(!analysis_provider_requires_credential(
            TEST_SAMPLE_ANALYSIS_PROVIDER_ID
        ));
    }
}
