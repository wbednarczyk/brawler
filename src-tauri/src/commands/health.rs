use crate::{app_state, storage, HealthResponse};

#[tauri::command]
pub fn health() -> HealthResponse {
    HealthResponse {
        status: "ok",
        version: env!("CARGO_PKG_VERSION"),
    }
}

#[tauri::command]
pub fn database_status(
    state: tauri::State<'_, app_state::AppState>,
) -> Result<storage::DatabaseStatus, String> {
    state.database_status().map_err(|error| error.to_string())
}
