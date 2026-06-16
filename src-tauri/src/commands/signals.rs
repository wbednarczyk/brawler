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

#[derive(Debug, serde::Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ConfirmDerivedEventInput {
    event_id: String,
    /// `"confirm"` places the proposed event on the calendar; `"reject"` discards it.
    action: String,
}

/// Confirm or reject a `proposed` derived calendar event (dividend / general-meeting date).
/// A guessed-date event is never auto-confirmed (ADR 0036).
#[tauri::command]
pub fn confirm_derived_event(
    input: ConfirmDerivedEventInput,
    state: tauri::State<'_, app_state::AppState>,
) -> Result<(), String> {
    let confirm = match input.action.as_str() {
        "confirm" => true,
        "reject" => false,
        other => return Err(format!("unknown derived-event action: {other}")),
    };
    state
        .confirm_derived_event(&input.event_id, confirm)
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

/// Run the opt-in AI date-extraction fallback, deriving `proposed` calendar events for
/// confirmed dividend / general-meeting signals the deterministic parser could not date
/// (ADR 0036). No-op (returns `enabled: false`) unless the user enabled the fallback.
#[tauri::command]
pub async fn run_ai_event_derivation(
    state: tauri::State<'_, app_state::AppState>,
) -> Result<jobs::event_derivation::AiEventDerivationSummary, String> {
    jobs::event_derivation::run_ai_event_derivation(&state).await
}
