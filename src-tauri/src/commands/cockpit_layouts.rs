use crate::{app_state, storage};

// Research cockpit layout commands (ADR 0053). Named saved layouts are the only
// persisted cockpit state (decision 3A): SQLite, not localStorage. The two
// domain errors are surfaced as the stable contract codes the frontend matches
// on; everything else falls through to its descriptive message.
fn cockpit_error_code(error: storage::StorageError) -> String {
    match error {
        storage::StorageError::CockpitLayoutNotFound { .. } => {
            "cockpit_layout_not_found".to_owned()
        }
        storage::StorageError::InvalidCockpitLayoutName { .. } => {
            "invalid_cockpit_layout_name".to_owned()
        }
        storage::StorageError::DuplicateCockpitLayoutName { .. } => {
            "duplicate_cockpit_layout_name".to_owned()
        }
        other => other.to_string(),
    }
}

/// Input for `rename_cockpit_layout` (issue #89) — in-place rename by id.
#[derive(Debug, serde::Deserialize)]
#[cfg_attr(feature = "ts-export", derive(ts_rs::TS))]
#[cfg_attr(
    feature = "ts-export",
    ts(export, export_to = "../../src/api/generated/")
)]
#[serde(rename_all = "camelCase")]
pub struct RenameCockpitLayoutInput {
    pub layout_id: String,
    pub name: String,
}

#[tauri::command]
pub fn list_cockpit_layouts(
    state: tauri::State<'_, app_state::AppState>,
) -> Result<Vec<storage::CockpitLayout>, String> {
    state
        .cockpit_layouts()
        .list_cockpit_layouts()
        .map_err(cockpit_error_code)
}

#[tauri::command]
pub fn save_cockpit_layout(
    input: storage::NewCockpitLayout,
    state: tauri::State<'_, app_state::AppState>,
) -> Result<storage::CockpitLayout, String> {
    state
        .cockpit_layouts()
        .save_cockpit_layout(input)
        .map_err(cockpit_error_code)
}

#[tauri::command]
pub fn rename_cockpit_layout(
    input: RenameCockpitLayoutInput,
    state: tauri::State<'_, app_state::AppState>,
) -> Result<storage::CockpitLayout, String> {
    state
        .cockpit_layouts()
        .rename_cockpit_layout(&input.layout_id, &input.name)
        .map_err(cockpit_error_code)
}

#[tauri::command]
pub fn delete_cockpit_layout(
    layout_id: String,
    state: tauri::State<'_, app_state::AppState>,
) -> Result<(), String> {
    state
        .cockpit_layouts()
        .delete_cockpit_layout(&layout_id)
        .map_err(cockpit_error_code)
}
