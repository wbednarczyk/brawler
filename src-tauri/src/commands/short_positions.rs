//! KNF short-selling read command (v0.55 T4b, ADR 0069 decision 3). A thin async
//! wrapper over `AppState::short_positions()` on the typed [`CommandError`]
//! envelope (ADR 0070), offloaded off the UI thread via `spawn_blocking` (DoD §C).
//! Read-only — the register is populated by the daily `knf-short-selling` adapter.

use crate::app_state;
use crate::commands::error::{CommandError, CommandErrorCode};
use crate::storage;

/// The per-company short-selling view (Spółka ownership tool): active positions, change history,
/// the last remembered exit, the aggregate net short %, and the 30-day pp change.
#[tauri::command]
pub async fn list_short_positions(
    input: storage::ShortPositionsInput,
    state: tauri::State<'_, app_state::AppState>,
) -> Result<storage::ShortPositionsView, CommandError> {
    let state = state.inner().clone();
    tauri::async_runtime::spawn_blocking(move || {
        state
            .short_positions()
            .short_positions_view(&input.company_id)
            .map_err(CommandError::from)
    })
    .await
    .map_err(|error| {
        CommandError::new(CommandErrorCode::Internal, format!("task failed: {error}"))
    })?
}
