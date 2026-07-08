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
    /// Facts already present at their slot on a re-extraction (re-observations):
    /// same value, or a value that diverges from the stored one. Lets the UI
    /// report "already recorded" honestly instead of a bare "no new values".
    pub skipped_fact_ids: Vec<String>,
    /// How many re-observed slots carried a value that disagrees with the stored
    /// fact (never silently overwritten — surfaced for ratification).
    pub divergent_count: i64,
    /// Serialized `DriftReport` JSON when the layout drifted.
    pub drift_json: Option<String>,
}

/// Normalizes the optional trust-ladder `mode` input (default `autopilot`;
/// rejects anything other than `autopilot`/`assist`) — shared by both extraction
/// entry points so they validate identically.
fn normalize_mode(raw: Option<&str>) -> Result<String, String> {
    match raw.map(str::trim) {
        None | Some("") => Ok(storage::MODE_AUTOPILOT.to_owned()),
        Some(value) if value == storage::MODE_AUTOPILOT || value == storage::MODE_ASSIST => {
            Ok(value.to_owned())
        }
        Some(other) => Err(format!(
            "unknown extraction mode '{other}' (expected 'autopilot' or 'assist')"
        )),
    }
}

/// Maps the internal pipeline result to the serializable UI summary.
fn summarize(
    result: jobs::structured_extraction::StructuredExtractionResult,
) -> StructuredExtractionSummary {
    StructuredExtractionSummary {
        acceptance: result.acceptance.as_str().to_owned(),
        tier: result.tier.map(|t| t.as_str().to_owned()),
        emitted: result.emitted,
        produced_fact_ids: result.produced_fact_ids,
        skipped_fact_ids: result.skipped_fact_ids,
        divergent_count: result.divergences.len() as i64,
        drift_json: result.drift_json,
    }
}

#[tauri::command]
pub async fn run_structured_extraction(
    input: RunStructuredExtractionInput,
    state: tauri::State<'_, app_state::AppState>,
) -> Result<StructuredExtractionSummary, String> {
    let state = state.inner().clone();
    let mode = normalize_mode(input.mode.as_deref())?;

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
        .map(summarize)
    })
    .await
    .map_err(|e| format!("structured extraction task failed: {e}"))?
}

#[derive(Debug, Deserialize)]
#[cfg_attr(feature = "ts-export", derive(ts_rs::TS))]
#[cfg_attr(
    feature = "ts-export",
    ts(export, export_to = "../../src/api/generated/", optional_fields)
)]
#[serde(rename_all = "camelCase")]
pub struct ExtractReportDocumentDataInput {
    pub company_id: String,
    pub report_document_id: String,
    /// Trust-ladder mode (`autopilot` | `assist`, default `autopilot`) — same
    /// semantics as `run_structured_extraction`; the per-fact confirmation state
    /// is derived from the validation outcome, never a caller-chosen literal.
    pub mode: Option<String>,
}

/// The reachable, one-click entry point for the structured pipeline over a
/// **single stored report document** (closes the ADR 0061 S5 live-path gap: the
/// deterministic pipeline previously had no UI caller outside autopilot). Unlike
/// `run_structured_extraction`, the caller supplies no period — it is derived
/// server-side by [`jobs::structured_extraction::derive_report_period`], exactly
/// as the autopilot stage does, so the UI never invents the reporting period.
/// Errors when the period can't be derived (no stored file, unparsable ESEF, or
/// a PDF whose title/URL carries no classifiable period). Confirmation semantics
/// are unchanged from the pipeline — facts land per `mode` + validation outcome.
#[tauri::command]
pub async fn extract_report_document_data(
    input: ExtractReportDocumentDataInput,
    state: tauri::State<'_, app_state::AppState>,
) -> Result<StructuredExtractionSummary, String> {
    let state = state.inner().clone();
    let mode = normalize_mode(input.mode.as_deref())?;

    tauri::async_runtime::spawn_blocking(move || {
        let document_id = input.report_document_id.clone();
        // Diagnostics lifecycle (T7-A pattern, owner T7): a zero-fact run must
        // never be invisible. Developer-mode gated inside `record_diagnostic_event`,
        // best-effort, so these are no-ops in normal use.
        record_extraction_diagnostic(
            &state,
            &document_id,
            "started",
            "info",
            "Report-document extraction started.",
            serde_json::json!({
                "companyId": input.company_id,
                "reportDocumentId": document_id,
                "mode": mode,
            }),
        );

        let document = state
            .get_report_document(&document_id)
            .map_err(|e| e.to_string())?;
        let Some((fiscal_year, period_type, period_end)) =
            jobs::structured_extraction::derive_report_period(&state, &document)
        else {
            // Honest failure: an unrecognised format or a non-iXBRL render (e.g.
            // a pdf2htmlEX visual `.xhtml`) — never a silent empty-success.
            let reason = "could not determine the reporting period for this document — its \
                 title/URL carries no parseable period, or the stored file is not a \
                 recognised report format (a non-tagged visual .xhtml render, or a \
                 report package with no inline-XBRL instance)"
                .to_owned();
            record_extraction_diagnostic(
                &state,
                &document_id,
                "no_period",
                "warning",
                "Extraction skipped: reporting period could not be derived.",
                serde_json::json!({
                    "reportDocumentId": document_id,
                    "contentType": document.content_type,
                    "reason": reason,
                }),
            );
            return Err(reason);
        };

        let result = jobs::structured_extraction::run_structured_extraction(
            &state,
            &input.company_id,
            &document_id,
            fiscal_year,
            period_type,
            &period_end,
            &mode,
        );

        match &result {
            Ok(res) => {
                let severity = if res.emitted { "info" } else { "warning" };
                let message = if res.emitted {
                    "Report-document extraction completed with facts."
                } else {
                    "Report-document extraction produced no facts."
                };
                record_extraction_diagnostic(
                    &state,
                    &document_id,
                    if res.emitted { "completed" } else { "empty" },
                    severity,
                    message,
                    serde_json::json!({
                        "reportDocumentId": document_id,
                        "fiscalYear": fiscal_year,
                        "periodType": period_type,
                        "periodEnd": period_end,
                        "acceptance": res.acceptance.as_str(),
                        "tier": res.tier.map(|t| t.as_str()),
                        "emitted": res.emitted,
                        "factCount": res.produced_fact_ids.len(),
                        "skippedCount": res.skipped_fact_ids.len(),
                        "divergentCount": res.divergences.len(),
                    }),
                );
                // Each re-observed slot whose fresh value disagrees with the
                // stored (possibly confirmed) fact is never silently overwritten
                // (owner T7) — surface it as a warning for ratification.
                for divergence in &res.divergences {
                    record_extraction_diagnostic(
                        &state,
                        &document_id,
                        "value_divergence",
                        "warning",
                        "Re-extraction observed a different value than the stored fact — kept the stored value.",
                        serde_json::json!({
                            "reportDocumentId": document_id,
                            "factId": divergence.fact_id,
                            "metricKey": divergence.metric_key,
                            "storedValue": divergence.existing,
                            "extractedValue": divergence.incoming,
                        }),
                    );
                }
            }
            Err(error) => record_extraction_diagnostic(
                &state,
                &document_id,
                "failed",
                "error",
                "Report-document extraction failed.",
                serde_json::json!({
                    "reportDocumentId": document_id,
                    "error": error,
                }),
            ),
        }

        result.map(summarize)
    })
    .await
    .map_err(|e| format!("structured extraction task failed: {e}"))?
}

/// Record one report-document extraction diagnostic event (developer-mode
/// gated, best-effort — mirrors `qualitative_assessment::record_qualitative_diagnostic`
/// and `ai_analysis::record_ai_analysis_diagnostic`). Scope id is the report
/// document id, so the Diagnostics view groups a document's whole extraction
/// lifecycle (`started` → `completed`/`empty`/`no_period`/`failed`).
fn record_extraction_diagnostic(
    state: &app_state::AppState,
    report_document_id: &str,
    stage: &str,
    severity: &str,
    message: &str,
    metadata: serde_json::Value,
) {
    let _ = state.record_diagnostic_event(storage::NewDiagnosticEvent {
        occurred_at: None,
        module: "fundamentals_extraction".to_owned(),
        scope: Some(storage::DiagnosticScope {
            scope_type: "extract_report_document".to_owned(),
            id: Some(report_document_id.to_owned()),
        }),
        stage: stage.to_owned(),
        severity: severity.to_owned(),
        message: message.to_owned(),
        metadata: Some(metadata),
    });
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
