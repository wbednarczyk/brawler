use serde::Deserialize;

use crate::{app_state, logging};

const DEFAULT_LOG_ENTRY_LIMIT: usize = 300;

#[derive(Debug, Deserialize)]
#[cfg_attr(feature = "ts-export", derive(ts_rs::TS))]
#[cfg_attr(
    feature = "ts-export",
    ts(export, export_to = "../../src/api/generated/", optional_fields)
)]
#[serde(rename_all = "camelCase")]
pub struct ListLogEntriesInput {
    limit: Option<usize>,
}

#[tauri::command]
pub fn get_log_status(
    state: tauri::State<'_, app_state::AppState>,
) -> Result<logging::LogStatus, String> {
    ensure_developer_mode_enabled(&state)?;

    let settings = state.get_settings().map_err(|error| error.to_string())?;
    let resolved = logging::resolve_log_settings(&settings.logs);

    logging::log_status(state.data_dir(), &resolved).map_err(|error| error.to_string())
}

#[tauri::command]
pub fn list_log_entries(
    input: Option<ListLogEntriesInput>,
    state: tauri::State<'_, app_state::AppState>,
) -> Result<Vec<logging::LogEntry>, String> {
    ensure_developer_mode_enabled(&state)?;

    let settings = state.get_settings().map_err(|error| error.to_string())?;
    let resolved = logging::resolve_log_settings(&settings.logs);
    let limit = input
        .and_then(|input| input.limit)
        .unwrap_or(DEFAULT_LOG_ENTRY_LIMIT);

    logging::read_log_entries(state.data_dir(), resolved.max_files, limit)
        .map_err(|error| error.to_string())
}

#[tauri::command]
pub fn open_logs_directory(state: tauri::State<'_, app_state::AppState>) -> Result<(), String> {
    ensure_developer_mode_enabled(&state)?;

    let logs_dir = logging::logs_dir(state.data_dir());
    std::fs::create_dir_all(&logs_dir).map_err(|error| error.to_string())?;
    tauri_plugin_opener::open_path(logs_dir, None::<&str>).map_err(|error| error.to_string())
}

fn ensure_developer_mode_enabled(state: &app_state::AppState) -> Result<(), String> {
    let settings = state.get_settings().map_err(|error| error.to_string())?;
    if settings.developer_mode {
        Ok(())
    } else {
        Err("Developer mode is not active.".to_owned())
    }
}
