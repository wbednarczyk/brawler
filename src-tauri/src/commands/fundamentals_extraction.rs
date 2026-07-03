//! Tauri commands for structured-first fundamentals extraction (ADR 0061 S5).
//!
//! Exposes the deterministic pipeline as an explicit user action over a stored
//! report document (the reachable entry point for the PDF tier, whose period is
//! user-supplied), plus a read for the per-fact provenance the KPI display
//! badges. Extraction touches the filesystem and DB, so it is offloaded off the
//! UI thread.

use serde::{Deserialize, Serialize};

use crate::{app_state, jobs, storage};

#[derive(Debug, Deserialize)]
#[cfg_attr(feature = "ts-export", derive(ts_rs::TS))]
#[cfg_attr(
    feature = "ts-export",
    ts(export, export_to = "../../src/api/generated/", optional_fields)
)]
#[serde(rename_all = "camelCase")]
pub struct RunStructuredExtractionInput {
    pub company_id: String,
    pub report_document_id: String,
    pub fiscal_year: i64,
    pub period_type: String,
    pub period_end: String,
    /// Trust-ladder mode: `autopilot` | `assist`. Defaults to `autopilot` for a
    /// user-invoked deterministic run. The per-fact confirmation state is
    /// derived from the validation outcome (Accepted/witness → `confirmed`;
    /// unproven → `auto_unreviewed`/`pending` by mode), never from a
    /// caller-chosen literal (ADR 0061 dec. 3/8/9).
    pub mode: Option<String>,
}

/// Serializable summary of a structured extraction run for the UI.
#[derive(Debug, Serialize)]
#[cfg_attr(feature = "ts-export", derive(ts_rs::TS))]
#[cfg_attr(
    feature = "ts-export",
    ts(export, export_to = "../../src/api/generated/")
)]
#[serde(rename_all = "camelCase")]
pub struct StructuredExtractionSummary {
    /// accepted | accepted_via_witness | accepted_unreviewed | flagged | empty
    pub acceptance: String,
    /// esef | pdf | html_aggregator | … (the tier that produced the facts).
    pub tier: Option<String>,
    pub emitted: bool,
    pub produced_fact_ids: Vec<String>,
    /// Serialized `DriftReport` JSON when the layout drifted.
    pub drift_json: Option<String>,
}

#[tauri::command]
pub async fn run_structured_extraction(
    input: RunStructuredExtractionInput,
    state: tauri::State<'_, app_state::AppState>,
) -> Result<StructuredExtractionSummary, String> {
    let state = state.inner().clone();
    let mode = match input.mode.as_deref().map(str::trim) {
        None | Some("") => storage::MODE_AUTOPILOT.to_owned(),
        Some(value) if value == storage::MODE_AUTOPILOT || value == storage::MODE_ASSIST => {
            value.to_owned()
        }
        Some(other) => {
            return Err(format!(
                "unknown extraction mode '{other}' (expected 'autopilot' or 'assist')"
            ))
        }
    };

    // Offload the filesystem read + parse + DB writes off the UI thread.
    tauri::async_runtime::spawn_blocking(move || {
        jobs::structured_extraction::run_structured_extraction(
            &state,
            &input.company_id,
            &input.report_document_id,
            input.fiscal_year,
            &input.period_type,
            &input.period_end,
            &mode,
        )
        .map(|result| StructuredExtractionSummary {
            acceptance: result.acceptance.as_str().to_owned(),
            tier: result.tier.map(|t| t.as_str().to_owned()),
            emitted: result.emitted,
            produced_fact_ids: result.produced_fact_ids,
            drift_json: result.drift_json,
        })
    })
    .await
    .map_err(|e| format!("structured extraction task failed: {e}"))?
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ListFactProvenanceInput {
    pub fact_ids: Vec<String>,
}

#[tauri::command]
pub fn list_fact_provenance(
    input: ListFactProvenanceInput,
    state: tauri::State<'_, app_state::AppState>,
) -> Result<Vec<storage::FactProvenance>, String> {
    state
        .fundamentals_provenance()
        .get_many(&input.fact_ids)
        .map_err(|error| error.to_string())
}

/// Every fact currently flagged by the pipeline (drift / contradiction) — the
/// "structure changed" review surface.
#[tauri::command]
pub fn list_flagged_fact_provenance(
    state: tauri::State<'_, app_state::AppState>,
) -> Result<Vec<storage::FactProvenance>, String> {
    state
        .fundamentals_provenance()
        .list_flagged()
        .map_err(|error| error.to_string())
}
