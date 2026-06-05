use crate::{app_state, storage};
use serde::Deserialize;

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ImportContentsInput {
    pub contents: String,
}

#[tauri::command]
pub fn export_research_data(
    state: tauri::State<'_, app_state::AppState>,
) -> Result<storage::ExportPayload, String> {
    state
        .export_research_data()
        .map_err(|error| error.to_string())
}

#[tauri::command]
pub fn preview_research_import(
    input: ImportContentsInput,
    state: tauri::State<'_, app_state::AppState>,
) -> Result<storage::ImportPreview, String> {
    state
        .preview_research_import(&input.contents)
        .map_err(|error| error.to_string())
}

#[tauri::command]
pub fn apply_research_import(
    input: ImportContentsInput,
    state: tauri::State<'_, app_state::AppState>,
) -> Result<storage::ImportApplyResult, String> {
    state
        .apply_research_import(&input.contents)
        .map_err(|error| error.to_string())
}

#[tauri::command]
pub fn export_settings_data(
    state: tauri::State<'_, app_state::AppState>,
) -> Result<storage::ExportPayload, String> {
    state
        .export_settings_data()
        .map_err(|error| error.to_string())
}

#[tauri::command]
pub fn preview_settings_import(
    input: ImportContentsInput,
    state: tauri::State<'_, app_state::AppState>,
) -> Result<storage::ImportPreview, String> {
    state
        .preview_settings_import(&input.contents)
        .map_err(|error| error.to_string())
}

#[tauri::command]
pub fn apply_settings_import(
    input: ImportContentsInput,
    state: tauri::State<'_, app_state::AppState>,
) -> Result<storage::ImportApplyResult, String> {
    state
        .apply_settings_import(&input.contents)
        .map_err(|error| error.to_string())
}
