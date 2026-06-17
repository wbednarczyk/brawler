use crate::{app_state, storage};

#[tauri::command]
pub fn list_report_season(
    input: storage::ReportSeasonInput,
    state: tauri::State<'_, app_state::AppState>,
) -> Result<storage::ReportSeasonResult, String> {
    state
        .list_report_season(input)
        .map_err(|error| error.to_string())
}

#[tauri::command]
pub fn get_pre_report_card(
    input: storage::PreReportCardInput,
    state: tauri::State<'_, app_state::AppState>,
) -> Result<storage::PreReportCard, String> {
    state
        .get_pre_report_card(input)
        .map_err(|error| error.to_string())
}

#[tauri::command]
pub fn mark_report_prepared(
    input: storage::MarkReportPreparedInput,
    state: tauri::State<'_, app_state::AppState>,
) -> Result<storage::ReportPreparation, String> {
    state
        .mark_report_prepared(input)
        .map_err(|error| error.to_string())
}

#[tauri::command]
pub fn mark_report_processed(
    input: storage::MarkReportProcessedInput,
    state: tauri::State<'_, app_state::AppState>,
) -> Result<storage::ReportPreparation, String> {
    state
        .mark_report_processed(input)
        .map_err(|error| error.to_string())
}
