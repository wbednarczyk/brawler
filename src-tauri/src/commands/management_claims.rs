use crate::{app_state, storage};

#[tauri::command]
pub fn list_management_claims(
    company_id: String,
    state: tauri::State<'_, app_state::AppState>,
) -> Result<Vec<storage::ManagementClaim>, String> {
    state
        .list_management_claims(&company_id)
        .map_err(|error| error.to_string())
}

#[tauri::command]
pub fn create_management_claim(
    input: storage::NewManagementClaim,
    state: tauri::State<'_, app_state::AppState>,
) -> Result<storage::ManagementClaim, String> {
    state
        .create_management_claim(input)
        .map_err(|error| error.to_string())
}

#[tauri::command]
pub fn update_management_claim(
    input: storage::ManagementClaimUpdate,
    state: tauri::State<'_, app_state::AppState>,
) -> Result<storage::ManagementClaim, String> {
    state
        .update_management_claim(input)
        .map_err(|error| error.to_string())
}

#[tauri::command]
pub fn set_claim_verdict(
    input: storage::SetClaimVerdictInput,
    state: tauri::State<'_, app_state::AppState>,
) -> Result<storage::ManagementClaim, String> {
    state
        .set_claim_verdict(input)
        .map_err(|error| error.to_string())
}

#[tauri::command]
pub fn delete_management_claim(
    id: String,
    state: tauri::State<'_, app_state::AppState>,
) -> Result<(), String> {
    state
        .delete_management_claim(&id)
        .map_err(|error| error.to_string())
}

#[tauri::command]
pub fn list_claims_to_verify(
    company_id: String,
    state: tauri::State<'_, app_state::AppState>,
) -> Result<storage::ClaimsToVerify, String> {
    state
        .list_claims_to_verify(&company_id)
        .map_err(|error| error.to_string())
}
