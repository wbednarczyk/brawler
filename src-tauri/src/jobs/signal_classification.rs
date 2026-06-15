//! Async opt-in AI classification fallback (v0.40.0, ADR 0034).
//!
//! The deterministic rule classifier runs at ingestion and covers the formulaic
//! majority of ESPI/EBI filings. Filings it leaves unknown can be classified by
//! this **opt-in** fallback, which honors the multi-provider analysis boundary
//! (ADR 0028). It never auto-commits: every result is persisted as a `proposed`
//! signal that the user must confirm. The fallback is disabled by default — no
//! provider call happens until the user enables `espi_ai_fallback_enabled`.

use crate::{
    app_state,
    providers::analysis::{
        espi_classification_prompt, parse_espi_classification_output, registry, AnalysisDocument,
        EspiClassificationCategory,
    },
    storage::ProposedSignalInput,
};

/// The active official-report adapter whose unknown filings the fallback covers.
/// Source-neutral by design; a future `gpw-espi-ebi` re-enable adds its id here.
const BANKIER_COMPANY_ADAPTER_ID: &str = "bankier-company-komunikaty";

/// Upper bound on filings classified per run, to cap provider cost per trigger.
const DEFAULT_BATCH_LIMIT: i64 = 25;

/// Outcome of one fallback run.
#[derive(Debug, Clone, serde::Serialize)]
#[serde(rename_all = "camelCase")]
pub struct AiSignalClassificationSummary {
    /// Whether the opt-in fallback is enabled. When `false` nothing ran.
    pub enabled: bool,
    /// Unclassified filings examined this run.
    pub examined: usize,
    /// New `proposed` signals created this run.
    pub proposed: usize,
    /// Filings the model classified as unknown or that errored.
    pub skipped: usize,
}

pub async fn run_ai_signal_classification(
    state: &app_state::AppState,
) -> Result<AiSignalClassificationSummary, String> {
    let settings = state.get_settings().map_err(|error| error.to_string())?;
    if !settings.espi_ai_fallback_enabled {
        return Ok(AiSignalClassificationSummary {
            enabled: false,
            examined: 0,
            proposed: 0,
            skipped: 0,
        });
    }

    let provider_id = settings
        .ai_providers
        .general_analysis_provider
        .clone()
        .filter(|value| !value.trim().is_empty())
        .ok_or_else(|| {
            "no general AI analysis provider is configured for the classification fallback"
                .to_owned()
        })?;
    let model = settings.ai_providers.general_analysis_model.clone();
    let timeout_seconds = settings.ai_providers.general_analysis_timeout_seconds;
    let api_key = registry::read_analysis_provider_api_key(&provider_id);
    let provider =
        registry::build_analysis_provider(&provider_id, api_key, &model, timeout_seconds)?;

    let categories = state
        .list_signal_categories_for_ai()
        .map_err(|error| error.to_string())?;
    let allowed_keys: Vec<String> = categories
        .iter()
        .map(|category| category.key.clone())
        .collect();
    let prompt_categories: Vec<EspiClassificationCategory> = categories
        .iter()
        .map(|category| EspiClassificationCategory {
            key: category.key.clone(),
            display_name: category.display_name.clone(),
        })
        .collect();

    let filings = state
        .list_unclassified_official_filings(BANKIER_COMPANY_ADAPTER_ID, DEFAULT_BATCH_LIMIT)
        .map_err(|error| error.to_string())?;
    let examined = filings.len();
    let mut proposed = 0;
    let mut skipped = 0;

    for filing in filings {
        let prompt = espi_classification_prompt(&prompt_categories, &filing.title);
        let body = if filing.body_text.trim().is_empty() {
            filing.title.clone()
        } else {
            filing.body_text.clone()
        };
        let document = AnalysisDocument::Text { text: body };

        let text = match provider.complete_document(&prompt, &document).await {
            Ok(text) => text,
            Err(error) => {
                log::warn!(
                    "module=signal_classification stage=provider_error feedItemId={} error={}",
                    filing.feed_item_id,
                    error
                );
                skipped += 1;
                continue;
            }
        };

        let output =
            match parse_espi_classification_output(&text, &allowed_keys, provider.provider_id()) {
                Ok(output) => output,
                Err(error) => {
                    log::warn!(
                        "module=signal_classification stage=parse_error feedItemId={} error={}",
                        filing.feed_item_id,
                        error
                    );
                    skipped += 1;
                    continue;
                }
            };

        match output.category {
            Some(category) => {
                let created = state
                    .propose_company_signal(ProposedSignalInput {
                        feed_item_id: filing.feed_item_id.clone(),
                        company_id: filing.company_id.clone(),
                        category,
                        confidence: output.confidence as f64,
                        signal_date: filing.signal_date.clone(),
                        provider_id: provider.provider_id().to_owned(),
                        model_id: model.clone(),
                    })
                    .map_err(|error| error.to_string())?;
                if created {
                    proposed += 1;
                } else {
                    skipped += 1;
                }
            }
            None => skipped += 1,
        }
    }

    log::info!(
        "module=signal_classification stage=done providerId={} examined={} proposed={} skipped={}",
        provider_id,
        examined,
        proposed,
        skipped
    );

    Ok(AiSignalClassificationSummary {
        enabled: true,
        examined,
        proposed,
        skipped,
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::providers::analysis::{AiAnalysisProvider, TestSampleAnalysisProvider};
    use crate::storage::{open_in_memory_database, AppState, SettingsUpdate};
    use tauri::async_runtime::block_on;

    #[test]
    fn fallback_is_noop_when_disabled_by_default() {
        let connection = open_in_memory_database().expect("database should initialize");
        let state = AppState::new(connection);

        let summary = block_on(run_ai_signal_classification(&state)).expect("run should succeed");
        assert!(!summary.enabled);
        assert_eq!(summary.examined, 0);
        assert_eq!(summary.proposed, 0);
    }

    #[test]
    fn enabled_fallback_with_no_pending_filings_is_a_clean_noop() {
        let connection = open_in_memory_database().expect("database should initialize");
        let state = AppState::new(connection);
        state
            .update_settings(SettingsUpdate {
                espi_ai_fallback_enabled: Some(true),
                ..Default::default()
            })
            .expect("settings should update");
        assert!(
            state
                .get_settings()
                .expect("settings reload")
                .espi_ai_fallback_enabled,
            "flag should persist as enabled"
        );

        // The default provider (provider_gemini) builds without a key; with no
        // unclassified filings the run does no provider work and reports clean.
        let summary = block_on(run_ai_signal_classification(&state)).expect("run should succeed");
        assert!(summary.enabled);
        assert_eq!(summary.examined, 0);
        assert_eq!(summary.proposed, 0);
    }

    #[test]
    fn test_sample_provider_classifies_via_prompt_and_parser() {
        // Validates the prompt marker, provider round-trip, and parser together.
        let provider = TestSampleAnalysisProvider;
        let categories = vec![
            EspiClassificationCategory {
                key: "guidance_change".to_owned(),
                display_name: "Guidance change".to_owned(),
            },
            EspiClassificationCategory {
                key: "dividend".to_owned(),
                display_name: "Dividend".to_owned(),
            },
        ];
        let allowed_keys: Vec<String> = categories
            .iter()
            .map(|category| category.key.clone())
            .collect();
        let prompt = espi_classification_prompt(&categories, "Aktualizacja prognozy finansowej");
        let document = AnalysisDocument::Text {
            text: "Treść komunikatu.".to_owned(),
        };

        let text = block_on(provider.complete_document(&prompt, &document))
            .expect("test provider should respond");
        let output = parse_espi_classification_output(&text, &allowed_keys, provider.provider_id())
            .expect("classification should parse");

        assert_eq!(output.category.as_deref(), Some("guidance_change"));
        assert!(output.confidence > 0.0);
    }

    #[test]
    fn parser_rejects_categories_outside_the_taxonomy() {
        let allowed_keys = vec!["dividend".to_owned()];
        let output = parse_espi_classification_output(
            r#"{"category":"made_up_category","confidence":0.95}"#,
            &allowed_keys,
            "test",
        )
        .expect("should parse");
        assert!(
            output.category.is_none(),
            "unknown/unlisted categories map to the unknown outcome"
        );
        assert_eq!(output.confidence, 0.0);
    }
}
