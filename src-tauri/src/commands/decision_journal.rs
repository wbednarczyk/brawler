//! Decision journal IPC commands (ADR 0071, v0.52.0). Among the first commands
//! on the typed [`CommandError`] envelope (ADR 0070): they return
//! `Result<T, CommandError>` so the frontend can branch on a machine-readable
//! `code`. Thin async wrappers over `AppState::decision_journal()`, offloaded
//! off the UI thread via `spawn_blocking` (DoD §C) — entries are immutable, so
//! there is no update/delete command by design.

use crate::app_state;
use crate::commands::error::{CommandError, CommandErrorCode};
use crate::storage;

#[tauri::command]
pub async fn create_decision_entry(
    input: storage::NewDecisionEntry,
    state: tauri::State<'_, app_state::AppState>,
) -> Result<storage::DecisionEntry, CommandError> {
    let state = state.inner().clone();
    tauri::async_runtime::spawn_blocking(move || {
        state
            .decision_journal()
            .create_decision_entry(input)
            .map_err(CommandError::from)
    })
    .await
    .map_err(|error| {
        CommandError::new(CommandErrorCode::Internal, format!("task failed: {error}"))
    })?
}

#[tauri::command]
pub async fn list_decision_entries(
    input: storage::DecisionEntryListInput,
    state: tauri::State<'_, app_state::AppState>,
) -> Result<Vec<storage::DecisionEntry>, CommandError> {
    let state = state.inner().clone();
    tauri::async_runtime::spawn_blocking(move || {
        state
            .decision_journal()
            .list_decision_entries(input)
            .map_err(CommandError::from)
    })
    .await
    .map_err(|error| {
        CommandError::new(CommandErrorCode::Internal, format!("task failed: {error}"))
    })?
}
