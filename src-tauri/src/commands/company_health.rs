//! Company-health read model (ADR 0083 Decisions 2–4): deterministic Piotroski
//! F + Altman Z″ scores for the Quality panel.
//!
//! A computed read model (no stored projection): the [`health`] engine scores
//! over the shared [`MetricsContext`] (confirmed facts only). Offloaded off the
//! UI thread via `spawn_blocking` — it loads the metric catalog + every period's
//! facts and evaluates both scores per FY period. Decision support only:
//! published-formula citations, never verdict language.

use serde::Serialize;

use crate::app_state::AppState;
use crate::fundamentals::health::{self, HealthPeriodScores, ALTMAN_VARIANT, PIOTROSKI_VARIANT};

/// Everything the Quality panel's health section renders for one company.
#[derive(Debug, Clone, Serialize)]
#[cfg_attr(feature = "ts-export", derive(ts_rs::TS))]
#[cfg_attr(
    feature = "ts-export",
    ts(export, export_to = "../../src/api/generated/")
)]
#[serde(rename_all = "camelCase")]
pub struct CompanyHealth {
    pub company_id: String,
    /// The company's `statement_type`; `not_applicable` states carry it as their
    /// reason (financials are excluded — ADR 0083 Decision 4).
    pub statement_type: String,
    /// Published-formula citation labels, always visible beside the scores.
    pub piotroski_variant: String,
    pub altman_variant: String,
    /// The latest FY period's scores (the tiles), or `None` when the company has
    /// no annual period yet.
    #[serde(skip_serializing_if = "Option::is_none")]
    #[cfg_attr(feature = "ts-export", ts(optional))]
    pub latest: Option<HealthPeriodScores>,
    /// Every FY period, newest-first (the trend), including `latest`.
    pub history: Vec<HealthPeriodScores>,
}

/// Compute the health read model for one company (sync core, unit-testable).
pub fn compute_company_health(state: &AppState, company_id: &str) -> Result<CompanyHealth, String> {
    let statement_type = state
        .companies()
        .get_statement_type(company_id)
        .map_err(|error| error.to_string())?;
    let context = state
        .quality_frameworks()
        .metrics_context(company_id)
        .map_err(|error| error.to_string())?;

    let history = health::company_health(&context, &statement_type);
    let latest = history.first().cloned();

    Ok(CompanyHealth {
        company_id: company_id.to_owned(),
        statement_type,
        piotroski_variant: PIOTROSKI_VARIANT.to_owned(),
        altman_variant: ALTMAN_VARIANT.to_owned(),
        latest,
        history,
    })
}

/// Piotroski F + Altman Z″ for the Quality panel (ADR 0083). Offloaded — reads
/// the catalog + every period's confirmed facts and scores each FY period.
#[tauri::command]
pub async fn get_company_health(
    company_id: String,
    state: tauri::State<'_, AppState>,
) -> Result<CompanyHealth, String> {
    let state = state.inner().clone();
    tauri::async_runtime::spawn_blocking(move || compute_company_health(&state, &company_id))
        .await
        .map_err(|error| format!("company health task failed: {error}"))?
}

/// Headless health-facts backfill (ADR 0083 Decision 4 amendment (d), T3 gate):
/// force-re-extract the deterministic ESEF facts over a company's stored packages
/// so the v0.57 health concepts (`current_assets`/`current_liabilities`/
/// `retained_earnings`/`long_term_debt`) persist as confirmed facts. Stored facts
/// predate the concept-map extension and the history sweep won't fill the gap (it
/// only re-attacks zero-fact periods). Additive + idempotent — an existing fact is
/// re-observed, never overwritten. **Documented headless-only** (no UI): invoked
/// for the T9 live pass and the T3 gate harness. Offloaded off the UI thread.
#[tauri::command]
pub async fn backfill_company_health_facts(
    company_id: String,
    state: tauri::State<'_, AppState>,
) -> Result<crate::jobs::health_facts_backfill::HealthFactsBackfillResult, String> {
    let state = state.inner().clone();
    tauri::async_runtime::spawn_blocking(move || {
        crate::jobs::health_facts_backfill::backfill_company_health_facts(&state, &company_id)
    })
    .await
    .map_err(|error| format!("health facts backfill task failed: {error}"))?
}

/// The company's red-flags panel state (ADR 0083 Decision 8): active flags +
/// acknowledged history, computed at read (no stored projection). Offloaded —
/// composes raised signals + auditor / short-spike read models.
#[tauri::command]
pub async fn get_red_flags(
    company_id: String,
    state: tauri::State<'_, AppState>,
) -> Result<crate::storage::RedFlagsView, String> {
    let state = state.inner().clone();
    tauri::async_runtime::spawn_blocking(move || {
        state
            .red_flags()
            .red_flags_view(&company_id)
            .map_err(|error| error.to_string())
    })
    .await
    .map_err(|error| format!("red flags task failed: {error}"))?
}

/// Acknowledge a red flag by its deterministic id (ADR 0083 Decision 8):
/// idempotent, moves the flag from `active` to `history`, and the same evidence
/// never re-raises. Returns the refreshed view.
#[tauri::command]
pub async fn acknowledge_red_flag(
    input: crate::storage::AcknowledgeRedFlagInput,
    state: tauri::State<'_, AppState>,
) -> Result<crate::storage::RedFlagsView, String> {
    let state = state.inner().clone();
    tauri::async_runtime::spawn_blocking(move || {
        state
            .red_flags()
            .acknowledge(&input.flag_id)
            .map_err(|error| error.to_string())
    })
    .await
    .map_err(|error| format!("acknowledge red flag task failed: {error}"))?
}
