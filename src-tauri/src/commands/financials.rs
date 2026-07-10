use crate::{app_state, document_fetcher, report_documents_capture, storage};

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

#[tauri::command]
pub fn capture_report_document(
    input: storage::CaptureReportDocumentInput,
    state: tauri::State<'_, app_state::AppState>,
) -> Result<report_documents_capture::DocumentCaptureResult, String> {
    let fetcher = document_fetcher::HttpDocumentFetcher::new();
    report_documents_capture::capture_report_document(&state, &fetcher, input)
        .map_err(|error| error.to_string())
}

#[tauri::command]
pub fn list_report_documents(
    company_id: String,
    state: tauri::State<'_, app_state::AppState>,
) -> Result<Vec<storage::ReportDocument>, String> {
    state
        .list_report_documents_by_company(&company_id)
        .map_err(|error| error.to_string())
}

/// Recompute `doc_kind` for every stored report document (ADR 0077 §1, T1.2).
/// Classification is deterministic Rust code, so this is an idempotent self-heal
/// used to backfill rows that predate the taxonomy; new documents are already
/// classified at ingestion. Offloaded off the UI thread (corpus-wide scan).
#[tauri::command]
pub async fn reclassify_report_documents(
    state: tauri::State<'_, app_state::AppState>,
) -> Result<storage::ReclassifyReportDocumentsSummary, String> {
    let state = state.inner().clone();
    tauri::async_runtime::spawn_blocking(move || {
        state
            .reclassify_report_documents()
            .map_err(|error| error.to_string())
    })
    .await
    .map_err(|error| format!("reclassify task failed: {error}"))?
}

#[tauri::command]
pub fn resolve_ir_report(
    input: crate::ir_resolution::ResolveIrReportInput,
    state: tauri::State<'_, app_state::AppState>,
) -> Result<crate::ir_resolution::IrReportResolution, String> {
    let fetcher = document_fetcher::HttpDocumentFetcher::new();
    crate::ir_resolution::resolve_ir_report(&state, &fetcher, input)
}
