use crate::{app_state, storage};

#[tauri::command]
pub fn backup_status(
    state: tauri::State<'_, app_state::AppState>,
) -> Result<storage::BackupStatus, String> {
    state.backup_status().map_err(|error| error.to_string())
}

#[tauri::command]
pub fn create_backup(
    state: tauri::State<'_, app_state::AppState>,
) -> Result<storage::BackupStatus, String> {
    state.create_backup().map_err(|error| error.to_string())
}

#[tauri::command]
pub fn restore_backup(
    file_name: String,
    state: tauri::State<'_, app_state::AppState>,
) -> Result<(), String> {
    state
        .request_restore(&file_name)
        .map_err(|error| error.to_string())
}
