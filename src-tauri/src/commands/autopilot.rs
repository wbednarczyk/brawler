//! Tauri commands for the autonomous report pipeline (North Star, v0.49.0, ADR 0055).
//!
//! The trust ladder (per-company `off`/`assist`/`autopilot` mode), the run/review
//! read models, run-level undo, and a manual trigger. The pipeline itself runs on
//! the durable queue (`crate::jobs::autopilot`); these commands are cheap CRUD over
//! the autopilot store, so they stay synchronous. The global confirm-before-commit
//! default is never changed here — automation is a per-company opt-in.

use serde::{Deserialize, Serialize};

use crate::{app_state, jobs, storage};

#[derive(Debug, Deserialize)]
#[cfg_attr(feature = "ts-export", derive(ts_rs::TS))]
#[cfg_attr(
    feature = "ts-export",
    ts(export, export_to = "../../src/api/generated/")
)]
#[serde(rename_all = "camelCase")]
pub struct CompanyIdInput {
    pub company_id: String,
}

#[derive(Debug, Deserialize)]
#[cfg_attr(feature = "ts-export", derive(ts_rs::TS))]
#[cfg_attr(
    feature = "ts-export",
    ts(export, export_to = "../../src/api/generated/")
)]
#[serde(rename_all = "camelCase")]
pub struct SetCompanyAutopilotInput {
    pub company_id: String,
    pub mode: String,
}

#[derive(Debug, Deserialize)]
#[cfg_attr(feature = "ts-export", derive(ts_rs::TS))]
#[cfg_attr(
    feature = "ts-export",
    ts(export, export_to = "../../src/api/generated/")
)]
#[serde(rename_all = "camelCase")]
pub struct RunIdInput {
    pub run_id: String,
}

#[derive(Debug, Deserialize)]
#[cfg_attr(feature = "ts-export", derive(ts_rs::TS))]
#[cfg_attr(
    feature = "ts-export",
    ts(export, export_to = "../../src/api/generated/", optional_fields)
)]
#[serde(rename_all = "camelCase")]
pub struct SetRunNotificationStateInput {
    pub run_id: String,
    pub notification_state: String,
}

#[derive(Debug, Deserialize)]
#[cfg_attr(feature = "ts-export", derive(ts_rs::TS))]
#[cfg_attr(
    feature = "ts-export",
    ts(export, export_to = "../../src/api/generated/")
)]
#[serde(rename_all = "camelCase")]
pub struct TriggerAutopilotRunInput {
    pub company_id: String,
    pub report_document_id: String,
}

#[derive(Debug, Serialize)]
#[cfg_attr(feature = "ts-export", derive(ts_rs::TS))]
#[cfg_attr(
    feature = "ts-export",
    ts(export, export_to = "../../src/api/generated/")
)]
#[serde(rename_all = "camelCase")]
pub struct UndoAutopilotRunResult {
    pub run_id: String,
    pub reverted_fact_ids: Vec<String>,
}

/// Read a company's trust-ladder mode (a company with no setting reads `off`).
#[tauri::command]
pub fn get_company_autopilot(
    input: CompanyIdInput,
    state: tauri::State<'_, app_state::AppState>,
) -> Result<storage::CompanyAutopilot, String> {
    let mode = state
        .autopilot()
        .get_mode(&input.company_id)
        .map_err(|error| error.to_string())?;
    Ok(storage::CompanyAutopilot {
        company_id: input.company_id,
        mode,
    })
}

/// Set a company's trust-ladder mode. Validates the mode and that the company
/// exists; never alters already-produced facts or runs.
#[tauri::command]
pub fn set_company_autopilot(
    input: SetCompanyAutopilotInput,
    state: tauri::State<'_, app_state::AppState>,
) -> Result<storage::CompanyAutopilot, String> {
    if !storage::is_valid_autopilot_mode(&input.mode) {
        return Err("invalid_autopilot_mode".to_owned());
    }
    if !company_exists(&state, &input.company_id)? {
        return Err("company_not_found".to_owned());
    }
    state
        .autopilot()
        .set_mode(&input.company_id, &input.mode)
        .map_err(|error| error.to_string())
}

/// Every company with an explicit autopilot mode (the rest default to `off`). For
/// the per-company settings surface (ADR 0056).
#[tauri::command]
pub fn list_company_autopilot_modes(
    state: tauri::State<'_, app_state::AppState>,
) -> Result<Vec<storage::CompanyAutopilot>, String> {
    state
        .autopilot()
        .list_modes()
        .map_err(|error| error.to_string())
}

#[derive(Debug, Deserialize)]
#[cfg_attr(feature = "ts-export", derive(ts_rs::TS))]
#[cfg_attr(
    feature = "ts-export",
    ts(export, export_to = "../../src/api/generated/")
)]
#[serde(rename_all = "camelCase")]
pub struct SetCompaniesAutopilotInput {
    pub company_ids: Vec<String>,
    pub mode: String,
}

/// Set the same autopilot mode on many companies at once (bulk action, ADR 0056).
/// Returns the number of companies updated.
#[tauri::command]
pub fn set_companies_autopilot(
    input: SetCompaniesAutopilotInput,
    state: tauri::State<'_, app_state::AppState>,
) -> Result<usize, String> {
    if !storage::is_valid_autopilot_mode(&input.mode) {
        return Err("invalid_autopilot_mode".to_owned());
    }
    state
        .autopilot()
        .set_modes(&input.company_ids, &input.mode)
        .map_err(|error| error.to_string())
}

/// List recent autopilot runs for the notification/review surface.
#[tauri::command]
pub fn list_autopilot_runs(
    input: storage::ListAutopilotRunsInput,
    state: tauri::State<'_, app_state::AppState>,
) -> Result<Vec<storage::AutopilotRun>, String> {
    state
        .autopilot()
        .list_runs(&input)
        .map_err(|error| error.to_string())
}

/// Fetch one run's full composed result.
#[tauri::command]
pub fn get_autopilot_run(
    input: RunIdInput,
    state: tauri::State<'_, app_state::AppState>,
) -> Result<storage::AutopilotRun, String> {
    state
        .autopilot()
        .get_run(&input.run_id)
        .map_err(|error| error.to_string())
}

/// Mark a run's notification `read` or `dismissed` (drives Today/Pulse).
#[tauri::command]
pub fn set_autopilot_run_notification_state(
    input: SetRunNotificationStateInput,
    state: tauri::State<'_, app_state::AppState>,
) -> Result<storage::AutopilotRun, String> {
    if !matches!(
        input.notification_state.as_str(),
        "unread" | "read" | "dismissed"
    ) {
        return Err("invalid_notification_state".to_owned());
    }
    state
        .autopilot()
        .set_notification_state(&input.run_id, &input.notification_state)
        .map_err(|error| error.to_string())
}

/// Revert exactly the facts a run produced (recorded on the run). Reuses fact
/// deletion; idempotent — undoing an already-undone run reverts nothing.
#[tauri::command]
pub fn undo_autopilot_run(
    input: RunIdInput,
    state: tauri::State<'_, app_state::AppState>,
) -> Result<UndoAutopilotRunResult, String> {
    let run = state
        .autopilot()
        .get_run(&input.run_id)
        .map_err(|error| error.to_string())?;
    let mut reverted = Vec::new();
    for fact_id in &run.produced_fact_ids {
        if state.delete_financial_fact(fact_id).is_ok() {
            reverted.push(fact_id.clone());
        }
    }
    state
        .autopilot()
        .clear_produced_facts(&run.id)
        .map_err(|error| error.to_string())?;
    Ok(UndoAutopilotRunResult {
        run_id: run.id,
        reverted_fact_ids: reverted,
    })
}

/// Manually start a run for an already-detected report (re-run / explicit kick).
/// Subject to the same `(company, report document)` dedup as automatic detection.
#[tauri::command]
pub fn trigger_autopilot_run(
    input: TriggerAutopilotRunInput,
    state: tauri::State<'_, app_state::AppState>,
) -> Result<storage::AutopilotRun, String> {
    let document = state
        .get_report_document(&input.report_document_id)
        .map_err(|_| "report_document_not_found".to_owned())?;
    if document.company_id != input.company_id {
        return Err("company_not_found".to_owned());
    }
    // A run already exists for this report unless the company is opted in; for a
    // manual trigger we capture whatever mode is set, defaulting to `assist` so a
    // manual run never silently auto-commits without an explicit `autopilot` opt-in.
    let mode = state
        .autopilot()
        .get_mode(&input.company_id)
        .map_err(|error| error.to_string())?;
    let mode = if mode == storage::MODE_AUTOPILOT {
        storage::MODE_AUTOPILOT
    } else {
        storage::MODE_ASSIST
    };

    let run_id = format!(
        "autopilot_run:{}:{}",
        input.company_id, input.report_document_id
    );
    match state
        .autopilot()
        .create_run_if_absent(
            &run_id,
            &input.company_id,
            &input.report_document_id,
            "manual",
            mode,
            // A manual run belongs to no sweep, so it charges no sweep budget.
            None,
        )
        .map_err(|error| error.to_string())?
    {
        Some(run) => {
            jobs::autopilot::enqueue_first_stage(&state, &run.id);
            Ok(run)
        }
        None => Err("autopilot_run_in_progress".to_owned()),
    }
}

fn company_exists(state: &app_state::AppState, company_id: &str) -> Result<bool, String> {
    Ok(state
        .list_companies()
        .map_err(|error| error.to_string())?
        .iter()
        .any(|company| company.id == company_id))
}
