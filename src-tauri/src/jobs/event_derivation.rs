//! Opt-in AI date-extraction fallback for calendar-event derivation (ADR 0036, v0.41.0).
//!
//! The deterministic parser ([crate::signal_dates]) runs at ingestion / signal confirmation and
//! dates the formulaic dividend / general-meeting filings. Filings it cannot date are left
//! without a derived event; this **opt-in** fallback asks the configured AI provider to extract
//! the future date from the filing body, honoring the multi-provider boundary (ADR 0028). It
//! never auto-commits: every result is a `proposed` calendar event the user must confirm. The
//! fallback is disabled by default — no provider call happens until `espi_ai_fallback_enabled`.

use crate::{
    app_state,
    providers::analysis::{
        espi_event_date_prompt, parse_espi_event_date_output, registry, AnalysisDocument,
    },
};

/// Upper bound on signals dated per run, to cap provider cost per trigger.
const DEFAULT_BATCH_LIMIT: i64 = 25;

/// Outcome of one AI event-derivation run.
#[derive(Debug, Clone, serde::Serialize)]
#[cfg_attr(feature = "ts-export", derive(ts_rs::TS))]
#[cfg_attr(
    feature = "ts-export",
    ts(export, export_to = "../../src/api/generated/")
)]
#[serde(rename_all = "camelCase")]
pub struct AiEventDerivationSummary {
    /// Whether the opt-in fallback is enabled. When `false` nothing ran.
    pub enabled: bool,
    /// Signals examined this run.
    pub examined: usize,
    /// New `proposed` derived events created this run.
    pub derived: usize,
    /// Signals the model could not date, or that errored.
    pub skipped: usize,
}

pub async fn run_ai_event_derivation(
    state: &app_state::AppState,
) -> Result<AiEventDerivationSummary, String> {
    let settings = state.get_settings().map_err(|error| error.to_string())?;
    if !settings.espi_ai_fallback_enabled {
        return Ok(AiEventDerivationSummary {
            enabled: false,
            examined: 0,
            derived: 0,
            skipped: 0,
        });
    }

    let candidates = state
        .list_signals_needing_event_date(DEFAULT_BATCH_LIMIT)
        .map_err(|error| error.to_string())?;
    let examined = candidates.len();
    if examined == 0 {
        return Ok(AiEventDerivationSummary {
            enabled: true,
            examined: 0,
            derived: 0,
            skipped: 0,
        });
    }

    let provider_id = settings
        .ai_providers
        .general_analysis_provider
        .clone()
        .filter(|value| !value.trim().is_empty())
        .ok_or_else(|| {
            "no general AI analysis provider is configured for the event-date fallback".to_owned()
        })?;
    let model = settings.ai_providers.general_analysis_model.clone();
    let timeout_seconds = settings.ai_providers.general_analysis_timeout_seconds;
    let api_key = registry::read_analysis_provider_api_key(&provider_id);
    let provider =
        registry::build_analysis_provider(&provider_id, api_key, &model, timeout_seconds)?;

    let mut derived = 0;
    let mut skipped = 0;

    for candidate in candidates {
        let event_kind = match candidate.category.as_str() {
            "dividend" => "dividend payment date (or, if absent, the dividend record date)",
            "general_meeting" => "general meeting date",
            _ => {
                skipped += 1;
                continue;
            }
        };
        let prompt = espi_event_date_prompt(event_kind, &candidate.title);
        let document = AnalysisDocument::Text {
            text: candidate.body_text.clone(),
        };

        let text = match provider.complete_document(&prompt, &document).await {
            Ok(text) => text,
            Err(error) => {
                log::warn!(
                    "module=event_derivation stage=provider_error signalId={} error={}",
                    candidate.signal_id,
                    error
                );
                skipped += 1;
                continue;
            }
        };

        let date = match parse_espi_event_date_output(&text, provider.provider_id()) {
            Ok(Some(date)) => date,
            Ok(None) => {
                skipped += 1;
                continue;
            }
            Err(error) => {
                log::warn!(
                    "module=event_derivation stage=parse_error signalId={} error={}",
                    candidate.signal_id,
                    error
                );
                skipped += 1;
                continue;
            }
        };

        match state.derive_event_from_extracted_date(&candidate.signal_id, &date) {
            Ok(true) => derived += 1,
            Ok(false) => skipped += 1,
            Err(error) => {
                log::warn!(
                    "module=event_derivation stage=derive_error signalId={} error={}",
                    candidate.signal_id,
                    error
                );
                skipped += 1;
            }
        }
    }

    log::info!(
        "module=event_derivation stage=done providerId={provider_id} examined={examined} derived={derived} skipped={skipped}"
    );

    Ok(AiEventDerivationSummary {
        enabled: true,
        examined,
        derived,
        skipped,
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::source_adapters::bankier_company::BankierCompanyItem;
    use crate::storage::{open_in_memory_database, AppState, SettingsUpdate};
    use tauri::async_runtime::block_on;

    #[test]
    fn fallback_is_noop_when_disabled_by_default() {
        let connection = open_in_memory_database().expect("database should initialize");
        let state = AppState::new(connection);

        let summary = block_on(run_ai_event_derivation(&state)).expect("run should succeed");
        assert!(!summary.enabled);
        assert_eq!(summary.examined, 0);
    }

    #[test]
    fn enabled_fallback_with_no_candidates_is_a_clean_noop() {
        let connection = open_in_memory_database().expect("database should initialize");
        let state = AppState::new(connection);
        state
            .update_settings(SettingsUpdate {
                espi_ai_fallback_enabled: Some(true),
                ..Default::default()
            })
            .expect("settings should update");

        let summary = block_on(run_ai_event_derivation(&state)).expect("run should succeed");
        assert!(summary.enabled);
        assert_eq!(summary.examined, 0);
        assert_eq!(summary.derived, 0);
    }

    fn undatable_dividend_item(company_id: &str, ticker: &str) -> BankierCompanyItem {
        BankierCompanyItem {
            company_id: company_id.to_owned(),
            qualified_ticker: format!("GPW:{ticker}"),
            title: "Rekomendacja Zarządu w sprawie wypłaty dywidendy za rok 2025".to_owned(),
            link: "https://www.bankier.pl/wiadomosc/CD-PROJEKT-SA-div-1.html".to_owned(),
            summary: "Komunikat ESPI/EBI".to_owned(),
            published_at: Some("2026-05-28T17:33:09".to_owned()),
            fetched_at: "2026-05-31T10:00:00Z".to_owned(),
            article_id: "div-1".to_owned(),
            pub_id: 3,
            dedupe_key: "bankier-company-komunikaty:article:div-1".to_owned(),
            duplicate_signature: "official-secondary:GPW:CDR:div-1".to_owned(),
            body_text: Some(
                "Zarząd rekomenduje przeznaczenie zysku na wypłatę dywidendy w wysokości 2 zł na akcję."
                    .to_owned(),
            ),
            attachments: Vec::new(),
            detail_fetch_attempted: true,
        }
    }

    #[test]
    fn undatable_dividend_is_a_candidate_and_ai_date_derives_proposed_event() {
        use crate::storage::{CompanyEventListInput, NewCompany};

        let connection = open_in_memory_database().expect("database should initialize");
        let state = AppState::new(connection);
        let company = state
            .create_company(NewCompany {
                exchange: "GPW".to_owned(),
                ticker: "CDR".to_owned(),
                display_name: "CD PROJEKT S.A.".to_owned(),
                isin: Some("PLOPTTC00011".to_owned()),
                cik: None,
                lei: None,
            })
            .expect("company should create");

        state
            .ingest_bankier_company_items(&[undatable_dividend_item(&company.id, "CDR")])
            .expect("ingestion should classify the dividend signal");

        // The deterministic sweep could not date it, so it surfaces as an AI candidate.
        let candidates = state
            .list_signals_needing_event_date(25)
            .expect("candidates should list");
        assert_eq!(candidates.len(), 1);
        assert_eq!(candidates[0].category, "dividend");

        // Applying an AI-extracted date derives a proposed event linked to the signal.
        let created = state
            .derive_event_from_extracted_date(&candidates[0].signal_id, "2030-09-15")
            .expect("derivation should succeed");
        assert!(created);

        let events = state
            .list_company_events(CompanyEventListInput {
                mode: Some("all".to_owned()),
                company_id: Some(company.id.clone()),
                ..Default::default()
            })
            .expect("events should list");
        let derived = events
            .iter()
            .find(|event| event.source_type == "derived_signal")
            .expect("a derived event should exist");
        assert_eq!(derived.status, "proposed");
        assert_eq!(derived.event_type, "dividend");
        assert_eq!(derived.event_date, "2030-09-15");

        // It is no longer a candidate (idempotent).
        assert!(state
            .list_signals_needing_event_date(25)
            .expect("candidates should list")
            .is_empty());
    }

    #[test]
    fn test_sample_provider_round_trips_prompt_and_parser() {
        use crate::providers::analysis::{AiAnalysisProvider, TestSampleAnalysisProvider};

        let provider = TestSampleAnalysisProvider;
        let prompt = espi_event_date_prompt("dividend payment date", "Rekomendacja dywidendy");
        let document = AnalysisDocument::Text {
            text: "Treść komunikatu o dywidendzie.".to_owned(),
        };
        let text = block_on(provider.complete_document(&prompt, &document))
            .expect("test provider should respond");
        let date =
            parse_espi_event_date_output(&text, provider.provider_id()).expect("should parse");
        assert_eq!(date, Some("2030-09-15".to_owned()));
    }

    #[test]
    fn parser_rejects_null_and_invalid_dates() {
        assert_eq!(
            parse_espi_event_date_output(r#"{"date":"null"}"#, "test").expect("parse"),
            None
        );
        assert_eq!(
            parse_espi_event_date_output(r#"{"date":"2030-02-31"}"#, "test").expect("parse"),
            None
        );
        assert_eq!(
            parse_espi_event_date_output(r#"{"date":"2030-09-15"}"#, "test").expect("parse"),
            Some("2030-09-15".to_owned())
        );
    }
}
