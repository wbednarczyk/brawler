use crate::{app_state, jobs, storage};

#[tauri::command]
pub fn list_company_signals(
    input: storage::CompanySignalListInput,
    state: tauri::State<'_, app_state::AppState>,
) -> Result<Vec<storage::CompanySignal>, String> {
    state
        .list_company_signals(input)
        .map_err(|error| error.to_string())
}

#[tauri::command]
pub fn confirm_company_signal(
    input: storage::CompanySignalActionInput,
    state: tauri::State<'_, app_state::AppState>,
) -> Result<storage::CompanySignal, String> {
    state
        .confirm_company_signal(&input.id)
        .map_err(|error| error.to_string())
}

#[tauri::command]
pub fn reject_company_signal(
    input: storage::CompanySignalActionInput,
    state: tauri::State<'_, app_state::AppState>,
) -> Result<(), String> {
    state
        .reject_company_signal(&input.id)
        .map_err(|error| error.to_string())
}

/// Run the opt-in AI classification fallback over unknown official filings.
/// No-op (returns `enabled: false`) unless the user enabled the fallback.
#[tauri::command]
pub async fn run_ai_signal_classification(
    state: tauri::State<'_, app_state::AppState>,
) -> Result<jobs::signal_classification::AiSignalClassificationSummary, String> {
    jobs::signal_classification::run_ai_signal_classification(&state).await
}
