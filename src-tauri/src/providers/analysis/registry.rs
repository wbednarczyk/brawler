//! Registry/factory for AI analysis providers.
//!
//! Resolves a provider id to a boxed [`AiAnalysisProvider`] and its credential,
//! replacing the previously duplicated `match provider_id` arms in the analysis
//! jobs. Adding a provider means extending the match arms here only; callers
//! (jobs, commands) depend on the provider-neutral trait, not concrete types.

use super::{
    AiAnalysisProvider, ClaudeAnalysisProvider, DocumentSupport, GeminiAnalysisProvider,
    OpenAiAnalysisProvider, TestSampleAnalysisProvider, TEST_SAMPLE_ANALYSIS_PROVIDER_ID,
};
use crate::providers::credentials;

/// Stable id for the Gemini analysis provider.
pub const GEMINI_ANALYSIS_PROVIDER_ID: &str = "provider_gemini";
/// Stable id for the Anthropic (Claude) analysis provider.
pub const ANTHROPIC_ANALYSIS_PROVIDER_ID: &str = "provider_anthropic";
/// Stable id for the OpenAI (ChatGPT) analysis provider.
pub const OPENAI_ANALYSIS_PROVIDER_ID: &str = "provider_openai";
/// Stable id for the generic OpenAI-compatible analysis provider (ADR 0060): a
/// user-supplied base URL speaking the OpenAI chat-completions shape.
pub const OPENAI_COMPATIBLE_ANALYSIS_PROVIDER_ID: &str = "provider_openai_compatible";

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
        AnalysisProviderCatalogEntry {
            provider_id: OPENAI_COMPATIBLE_ANALYSIS_PROVIDER_ID,
            label: "OpenAI-compatible (custom)",
            models: &[],
            default_model: "",
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
        OPENAI_COMPATIBLE_ANALYSIS_PROVIDER_ID,
        TEST_SAMPLE_ANALYSIS_PROVIDER_ID,
    ]
}

/// Whether a provider requires an API credential to run.
pub fn analysis_provider_requires_credential(provider_id: &str) -> bool {
    provider_id != TEST_SAMPLE_ANALYSIS_PROVIDER_ID
}

/// Static mirror of each provider's [`AiAnalysisProvider::document_support`],
/// keyed by provider id, so callers that only need this fact (settings
/// validation) do not have to build a live provider to ask it. Pinned against
/// every actually-built provider by
/// `document_support_static_map_matches_built_providers` below — if a
/// provider's real `document_support()` ever changes, that test catches the
/// drift.
pub fn analysis_provider_document_support(provider_id: &str) -> DocumentSupport {
    match provider_id {
        GEMINI_ANALYSIS_PROVIDER_ID => DocumentSupport::Native,
        ANTHROPIC_ANALYSIS_PROVIDER_ID => DocumentSupport::Native,
        TEST_SAMPLE_ANALYSIS_PROVIDER_ID => DocumentSupport::Native,
        OPENAI_ANALYSIS_PROVIDER_ID => DocumentSupport::TextOnly,
        OPENAI_COMPATIBLE_ANALYSIS_PROVIDER_ID => DocumentSupport::TextOnly,
        _ => DocumentSupport::None,
    }
}

/// Read the configured API key for an analysis provider, if any.
///
/// Returns `None` for keyless providers (the test sample) and for configured
/// providers whose key is not set.
pub fn read_analysis_provider_api_key(provider_id: &str) -> Option<String> {
    credentials::read_provider_api_key(provider_id).unwrap_or(None)
}

/// Build a boxed analysis provider for the given id, credential, model, and timeout.
///
/// `openai_compatible_base_url` is only consulted for
/// [`OPENAI_COMPATIBLE_ANALYSIS_PROVIDER_ID`]: the user-configured
/// `openai_compatible_base_url` setting (ADR 0060). It is ignored for every
/// other provider id.
pub fn build_analysis_provider(
    provider_id: &str,
    api_key: Option<String>,
    model: &str,
    timeout_seconds: i64,
    openai_compatible_base_url: Option<&str>,
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
        OPENAI_COMPATIBLE_ANALYSIS_PROVIDER_ID => {
            let base_url = openai_compatible_base_url
                .map(str::trim)
                .filter(|value| !value.is_empty())
                .ok_or_else(|| {
                    "OpenAI-compatible provider requires the openai_compatible_base_url setting"
                        .to_owned()
                })?;
            Ok(Box::new(
                OpenAiAnalysisProvider::live_compatible(
                    api_key,
                    model.to_owned(),
                    timeout_seconds,
                    base_url,
                )
                .map_err(|error| error.to_string())?,
            ))
        }
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
        assert_eq!(catalog.len(), 4);
        for entry in catalog {
            // The OpenAI-compatible entry has no curated model list — the model
            // is a freeform, user-supplied id (ADR 0060) — so it has no
            // default-in-list invariant to check.
            if !entry.models.is_empty() {
                assert!(
                    entry.models.contains(&entry.default_model),
                    "default model for {} must be in its model list",
                    entry.provider_id
                );
            }
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
        assert!(build_analysis_provider(
            GEMINI_ANALYSIS_PROVIDER_ID,
            None,
            "gemini-3.5-flash",
            90,
            None
        )
        .is_ok());
        assert!(build_analysis_provider(
            ANTHROPIC_ANALYSIS_PROVIDER_ID,
            None,
            "claude-sonnet-4-6",
            90,
            None
        )
        .is_ok());
        assert!(
            build_analysis_provider(OPENAI_ANALYSIS_PROVIDER_ID, None, "gpt-5.5", 90, None).is_ok()
        );
        assert!(
            build_analysis_provider(TEST_SAMPLE_ANALYSIS_PROVIDER_ID, None, "test", 90, None)
                .is_ok()
        );
        assert!(build_analysis_provider("provider_unknown", None, "x", 90, None).is_err());
    }

    #[test]
    fn builds_openai_compatible_provider_when_base_url_configured() {
        let provider = build_analysis_provider(
            OPENAI_COMPATIBLE_ANALYSIS_PROVIDER_ID,
            None,
            "custom-model",
            90,
            Some("https://example.test/v1"),
        );

        assert!(provider.is_ok());
        assert_eq!(
            provider.expect("provider built").provider_id(),
            OPENAI_COMPATIBLE_ANALYSIS_PROVIDER_ID
        );
    }

    #[test]
    fn rejects_openai_compatible_provider_without_base_url() {
        let missing = build_analysis_provider(
            OPENAI_COMPATIBLE_ANALYSIS_PROVIDER_ID,
            None,
            "custom-model",
            90,
            None,
        );
        assert!(missing.is_err());

        let blank = build_analysis_provider(
            OPENAI_COMPATIBLE_ANALYSIS_PROVIDER_ID,
            None,
            "custom-model",
            90,
            Some("   "),
        );
        assert!(blank.is_err());
    }

    #[test]
    fn document_support_static_map_matches_built_providers() {
        // Every actually-built provider's live `document_support()` must agree
        // with `analysis_provider_document_support`'s static answer for the
        // same id — the static map exists so settings validation can ask the
        // question without building a live provider, and it must never drift
        // from reality (Radicle 6ea2a8a).
        let cases: [(&str, &str, Option<&str>); 5] = [
            (GEMINI_ANALYSIS_PROVIDER_ID, "gemini-3.5-flash", None),
            (ANTHROPIC_ANALYSIS_PROVIDER_ID, "claude-sonnet-4-6", None),
            (OPENAI_ANALYSIS_PROVIDER_ID, "gpt-5.5", None),
            (
                OPENAI_COMPATIBLE_ANALYSIS_PROVIDER_ID,
                "custom-model",
                Some("https://example.test/v1"),
            ),
            (TEST_SAMPLE_ANALYSIS_PROVIDER_ID, "test", None),
        ];

        for (provider_id, model, base_url) in cases {
            let provider = build_analysis_provider(provider_id, None, model, 90, base_url)
                .unwrap_or_else(|error| panic!("{provider_id} should build: {error}"));
            assert_eq!(
                provider.document_support(),
                analysis_provider_document_support(provider_id),
                "static map must match the built provider's document_support for {provider_id}"
            );
        }
    }

    #[test]
    fn document_support_defaults_to_none_for_unknown_provider() {
        assert_eq!(
            analysis_provider_document_support("provider_unknown"),
            DocumentSupport::None
        );
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
        assert!(analysis_provider_requires_credential(
            OPENAI_COMPATIBLE_ANALYSIS_PROVIDER_ID
        ));
        assert!(!analysis_provider_requires_credential(
            TEST_SAMPLE_ANALYSIS_PROVIDER_ID
        ));
    }
}
