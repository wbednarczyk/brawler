//! Tauri commands for AI KPI extraction (v0.36.0, epic 9879941).
//!
//! Extraction is an explicit user action over a stored report document. The job
//! produces proposals; committing a value to a fact is a separate confirm step.

use serde::Deserialize;

use crate::{
    app_state, jobs,
    providers::analysis::{
        KPI_EXTRACTION_PROMPT_VERSION, TEST_SAMPLE_ANALYSIS_MODEL, TEST_SAMPLE_ANALYSIS_PROVIDER_ID,
    },
    storage,
};

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct StartKpiExtractionInput {
    report_document_id: String,
    period_hint: Option<String>,
    provider_mode: Option<String>,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ListKpiExtractionInput {
    report_document_id: String,
}

#[tauri::command]
pub async fn start_kpi_extraction(
    input: StartKpiExtractionInput,
    state: tauri::State<'_, app_state::AppState>,
) -> Result<storage::KpiExtractionJob, String> {
    let document = state
        .get_report_document(&input.report_document_id)
        .map_err(|error| error.to_string())?;

    let settings = state.get_settings().map_err(|error| error.to_string())?;
    let provider_id = input
        .provider_mode
        .as_deref()
        .map(str::trim)
        .filter(|value| !value.is_empty())
        .or(settings.ai_providers.general_analysis_provider.as_deref())
        .unwrap_or(TEST_SAMPLE_ANALYSIS_PROVIDER_ID)
        .to_owned();
    let model = if provider_id == TEST_SAMPLE_ANALYSIS_PROVIDER_ID {
        TEST_SAMPLE_ANALYSIS_MODEL.to_owned()
    } else {
        settings.ai_providers.general_analysis_model
    };

    let job = state
        .create_kpi_extraction_job(storage::NewKpiExtractionJob {
            company_id: document.company_id,
            report_document_id: input.report_document_id,
            provider_id,
            model,
            prompt_version: KPI_EXTRACTION_PROMPT_VERSION.to_owned(),
            period_hint: input.period_hint,
        })
        .map_err(|error| error.to_string())?;

    spawn_extraction_job(state.inner().clone(), job.id.clone());

    Ok(job)
}

#[tauri::command]
pub async fn retry_kpi_extraction(
    job_id: String,
    state: tauri::State<'_, app_state::AppState>,
) -> Result<storage::KpiExtractionJob, String> {
    let job = state
        .get_kpi_extraction_job(&job_id)
        .map_err(|error| error.to_string())?;
    state
        .mark_kpi_extraction_job_running(&job.id)
        .map_err(|error| error.to_string())?;
    spawn_extraction_job(state.inner().clone(), job.id.clone());
    Ok(job)
}

#[tauri::command]
pub fn list_kpi_extraction(
    input: ListKpiExtractionInput,
    state: tauri::State<'_, app_state::AppState>,
) -> Result<Vec<storage::KpiExtractionJob>, String> {
    state
        .list_kpi_extraction_jobs_by_document(&input.report_document_id)
        .map_err(|error| error.to_string())
}

#[tauri::command]
pub fn confirm_kpi_proposal(
    input: storage::ConfirmKpiProposalInput,
    state: tauri::State<'_, app_state::AppState>,
) -> Result<storage::FinancialFact, String> {
    state
        .confirm_kpi_proposal(input)
        .map_err(|error| error.to_string())
}

#[tauri::command]
pub fn reject_kpi_proposal(
    proposal_id: String,
    state: tauri::State<'_, app_state::AppState>,
) -> Result<storage::KpiExtractionProposal, String> {
    state
        .reject_kpi_proposal(&proposal_id)
        .map_err(|error| error.to_string())
}

fn spawn_extraction_job(state: app_state::AppState, job_id: String) {
    tauri::async_runtime::spawn_blocking(move || {
        if let Err(error) = jobs::kpi_extraction::run_kpi_extraction_job(&state, &job_id) {
            let _ = state.mark_kpi_extraction_job_failed(&job_id, "unknown", &error);
        }
    });
}
