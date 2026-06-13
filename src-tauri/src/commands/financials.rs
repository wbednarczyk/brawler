use crate::{app_state, storage};

#[tauri::command]
pub fn list_kpi_definitions(
    input: storage::ListKpiDefinitionsInput,
    state: tauri::State<'_, app_state::AppState>,
) -> Result<Vec<storage::KpiDefinition>, String> {
    state
        .list_kpi_definitions(input)
        .map_err(|error| error.to_string())
}

#[tauri::command]
pub fn create_kpi_definition(
    input: storage::NewKpiDefinition,
    state: tauri::State<'_, app_state::AppState>,
) -> Result<storage::KpiDefinition, String> {
    state
        .create_kpi_definition(input)
        .map_err(|error| error.to_string())
}

#[tauri::command]
pub fn list_financial_periods(
    input: storage::ListFinancialPeriodsInput,
    state: tauri::State<'_, app_state::AppState>,
) -> Result<Vec<storage::FinancialPeriod>, String> {
    state
        .list_financial_periods(input)
        .map_err(|error| error.to_string())
}

#[tauri::command]
pub fn create_financial_period(
    input: storage::NewFinancialPeriod,
    state: tauri::State<'_, app_state::AppState>,
) -> Result<storage::FinancialPeriod, String> {
    state
        .create_financial_period(input)
        .map_err(|error| error.to_string())
}

#[tauri::command]
pub fn update_financial_period(
    input: storage::UpdateFinancialPeriod,
    state: tauri::State<'_, app_state::AppState>,
) -> Result<storage::FinancialPeriod, String> {
    state
        .update_financial_period(input)
        .map_err(|error| error.to_string())
}

#[tauri::command]
pub fn delete_financial_period(
    id: String,
    state: tauri::State<'_, app_state::AppState>,
) -> Result<(), String> {
    state
        .delete_financial_period(&id)
        .map_err(|error| error.to_string())
}

#[tauri::command]
pub fn list_kpi_relevance(
    company_id: String,
    state: tauri::State<'_, app_state::AppState>,
) -> Result<Vec<storage::KpiRelevance>, String> {
    state
        .list_kpi_relevance(&company_id)
        .map_err(|error| error.to_string())
}

#[tauri::command]
pub fn create_kpi_relevance(
    input: storage::NewKpiRelevance,
    state: tauri::State<'_, app_state::AppState>,
) -> Result<storage::KpiRelevance, String> {
    state
        .create_kpi_relevance(input)
        .map_err(|error| error.to_string())
}

#[tauri::command]
pub fn update_kpi_relevance(
    input: storage::UpdateKpiRelevance,
    state: tauri::State<'_, app_state::AppState>,
) -> Result<storage::KpiRelevance, String> {
    state
        .update_kpi_relevance(input)
        .map_err(|error| error.to_string())
}

#[tauri::command]
pub fn delete_kpi_relevance(
    id: String,
    state: tauri::State<'_, app_state::AppState>,
) -> Result<(), String> {
    state
        .delete_kpi_relevance(&id)
        .map_err(|error| error.to_string())
}

#[tauri::command]
pub fn list_financial_facts(
    input: storage::ListFinancialFactsInput,
    state: tauri::State<'_, app_state::AppState>,
) -> Result<Vec<storage::FinancialFact>, String> {
    state
        .list_financial_facts(input)
        .map_err(|error| error.to_string())
}

#[tauri::command]
pub fn create_financial_fact(
    input: storage::NewFinancialFact,
    state: tauri::State<'_, app_state::AppState>,
) -> Result<storage::FinancialFact, String> {
    state
        .create_financial_fact(input)
        .map_err(|error| error.to_string())
}

#[tauri::command]
pub fn update_financial_fact(
    input: storage::UpdateFinancialFact,
    state: tauri::State<'_, app_state::AppState>,
) -> Result<storage::FinancialFact, String> {
    state
        .update_financial_fact(input)
        .map_err(|error| error.to_string())
}

#[tauri::command]
pub fn delete_financial_fact(
    id: String,
    state: tauri::State<'_, app_state::AppState>,
) -> Result<(), String> {
    state
        .delete_financial_fact(&id)
        .map_err(|error| error.to_string())
}
