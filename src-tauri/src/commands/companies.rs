use crate::{app_state, jobs::source_refresh, storage};

#[tauri::command]
pub fn list_companies(
    state: tauri::State<'_, app_state::AppState>,
) -> Result<Vec<storage::Company>, String> {
    state.list_companies().map_err(|error| error.to_string())
}

#[tauri::command]
pub fn create_company(
    input: storage::NewCompany,
    state: tauri::State<'_, app_state::AppState>,
) -> Result<storage::Company, String> {
    state
        .create_company(input)
        .map_err(|error| error.to_string())
}

#[tauri::command]
pub fn lookup_company(
    input: storage::CompanyLookupInput,
    state: tauri::State<'_, app_state::AppState>,
) -> Result<Option<storage::CompanyLookupResult>, String> {
    let first_result = state
        .lookup_company(input.clone())
        .map_err(|error| error.to_string())?;
    if first_result.is_some() || !source_refresh::should_bootstrap_gpw_registry(&input, &state)? {
        return Ok(first_result);
    }

    source_refresh::refresh_gpw_company_registry_for_trigger(&state, "lookup")?;

    state
        .lookup_company(input)
        .map_err(|error| error.to_string())
}

#[tauri::command]
pub fn delete_company(
    company_id: String,
    state: tauri::State<'_, app_state::AppState>,
) -> Result<(), String> {
    state
        .delete_company(&company_id)
        .map_err(|error| error.to_string())
}
