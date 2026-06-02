use crate::{app_state, storage};

#[tauri::command]
pub fn get_settings(
    state: tauri::State<'_, app_state::AppState>,
) -> Result<storage::UserSettings, String> {
    state.get_settings().map_err(|error| error.to_string())
}

#[tauri::command]
pub fn update_settings(
    input: storage::SettingsUpdate,
    state: tauri::State<'_, app_state::AppState>,
) -> Result<storage::UserSettings, String> {
    state
        .update_settings(input)
        .map_err(|error| error.to_string())
}
