//! Pre-report expectations IPC commands (ADR 0071, v0.52.0). On the typed
//! [`CommandError`] envelope (ADR 0070): editing a frozen expectation surfaces
//! as `conflict`, so the frontend can distinguish "the report already landed"
//! from a validation error. Thin async wrappers over
//! `AppState::report_expectations()`, offloaded off the UI thread (DoD §C).
//!
//! `expectation_review` is a composed read model (expectation vs confirmed
//! facts) — no stored projection, no automated judgment scoring (ADR 0042).

use crate::app_state;
use crate::commands::error::{CommandError, CommandErrorCode};
use crate::storage;

fn task_failed(error: impl std::fmt::Display) -> CommandError {
    CommandError::new(CommandErrorCode::Internal, format!("task failed: {error}"))
}

#[tauri::command]
pub async fn create_report_expectation(
    input: storage::NewReportExpectation,
    state: tauri::State<'_, app_state::AppState>,
) -> Result<storage::ReportExpectation, CommandError> {
    let state = state.inner().clone();
    tauri::async_runtime::spawn_blocking(move || {
        state
            .report_expectations()
            .create_report_expectation(input)
            .map_err(CommandError::from)
    })
    .await
    .map_err(task_failed)?
}

#[tauri::command]
pub async fn update_report_expectation(
    input: storage::UpdateReportExpectation,
    state: tauri::State<'_, app_state::AppState>,
) -> Result<storage::ReportExpectation, CommandError> {
    let state = state.inner().clone();
    tauri::async_runtime::spawn_blocking(move || {
        state
            .report_expectations()
            .update_report_expectation(input)
            .map_err(CommandError::from)
    })
    .await
    .map_err(task_failed)?
}

#[tauri::command]
pub async fn list_report_expectations(
    input: storage::ListReportExpectationsInput,
    state: tauri::State<'_, app_state::AppState>,
) -> Result<Vec<storage::ReportExpectation>, CommandError> {
    let state = state.inner().clone();
    tauri::async_runtime::spawn_blocking(move || {
        state
            .report_expectations()
            .list_report_expectations(input)
            .map_err(CommandError::from)
    })
    .await
    .map_err(task_failed)?
}

#[tauri::command]
pub async fn expectation_review(
    input: storage::ExpectationReviewInput,
    state: tauri::State<'_, app_state::AppState>,
) -> Result<storage::ExpectationReview, CommandError> {
    let state = state.inner().clone();
    tauri::async_runtime::spawn_blocking(move || {
        state
            .report_expectations()
            .expectation_review(&input.company_id, &input.event_key)
            .map_err(CommandError::from)
    })
    .await
    .map_err(task_failed)?
}

#[tauri::command]
pub async fn record_expectation_resolution(
    input: storage::RecordExpectationResolutionInput,
    state: tauri::State<'_, app_state::AppState>,
) -> Result<storage::ReportExpectation, CommandError> {
    let state = state.inner().clone();
    tauri::async_runtime::spawn_blocking(move || {
        state
            .report_expectations()
            .record_expectation_resolution(input)
            .map_err(CommandError::from)
    })
    .await
    .map_err(task_failed)?
}
