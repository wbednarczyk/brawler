use crate::{app_state, storage};

#[tauri::command]
pub fn list_company_events(
    input: storage::CompanyEventListInput,
    state: tauri::State<'_, app_state::AppState>,
) -> Result<Vec<storage::CompanyEvent>, String> {
    state
        .list_company_events(input)
        .map_err(|error| error.to_string())
}

#[tauri::command]
pub fn create_company_event(
    input: storage::NewCompanyEvent,
    state: tauri::State<'_, app_state::AppState>,
) -> Result<storage::CompanyEvent, String> {
    state
        .create_company_event(input)
        .map_err(|error| error.to_string())
}
