//! Tauri commands for AI claim extraction (v0.42.0, epic cbf6999, ADR 0040).
//!
//! Extraction is an explicit user action over a stored report document or a
//! transcript. The job produces proposals; materialising a claim is a separate
//! confirm step. Mirrors the KPI extraction commands.

use serde::Deserialize;

use crate::{
    app_state, jobs,
    providers::analysis::{
        CLAIM_EXTRACTION_PROMPT_VERSION, TEST_SAMPLE_ANALYSIS_MODEL,
        TEST_SAMPLE_ANALYSIS_PROVIDER_ID,
    },
    storage,
};

#[derive(Debug, Deserialize)]
#[cfg_attr(feature = "ts-export", derive(ts_rs::TS))]
#[cfg_attr(
    feature = "ts-export",
    ts(export, export_to = "../../src/api/generated/")
)]
#[serde(rename_all = "camelCase")]
pub struct StartClaimExtractionInput {
    #[cfg_attr(
        feature = "ts-export",
        ts(as = "crate::api_ts_unions::ClaimExtractionSourceType")
    )]
    source_type: String,
    source_id: String,
    #[cfg_attr(feature = "ts-export", ts(optional, type = "string | null"))]
    provider_mode: Option<String>,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ListClaimExtractionInput {
    source_type: String,
    source_id: String,
}

#[tauri::command]
pub async fn start_claim_extraction(
    input: StartClaimExtractionInput,
    state: tauri::State<'_, app_state::AppState>,
) -> Result<storage::ClaimExtractionJob, String> {
    let company_id = resolve_company_id(&state, &input.source_type, &input.source_id)?;

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
        .create_claim_extraction_job(storage::NewClaimExtractionJob {
            company_id,
            source_type: input.source_type,
            source_id: input.source_id,
            provider_id,
            model,
            prompt_version: CLAIM_EXTRACTION_PROMPT_VERSION.to_owned(),
        })
        .map_err(|error| error.to_string())?;

    spawn_extraction_job(state.inner().clone(), job.id.clone());

    Ok(job)
}

#[tauri::command]
pub async fn retry_claim_extraction(
    job_id: String,
    state: tauri::State<'_, app_state::AppState>,
) -> Result<storage::ClaimExtractionJob, String> {
    let job = state
        .mark_claim_extraction_job_running(&job_id)
        .map_err(|error| error.to_string())?;
    spawn_extraction_job(state.inner().clone(), job.id.clone());
    Ok(job)
}

#[tauri::command]
pub fn list_claim_extraction(
    input: ListClaimExtractionInput,
    state: tauri::State<'_, app_state::AppState>,
) -> Result<Vec<storage::ClaimExtractionJob>, String> {
    state
        .list_claim_extraction_jobs_by_source(&input.source_type, &input.source_id)
        .map_err(|error| error.to_string())
}

#[tauri::command]
pub fn confirm_claim_proposal(
    input: storage::ConfirmClaimProposalInput,
    state: tauri::State<'_, app_state::AppState>,
) -> Result<storage::ManagementClaim, String> {
    state
        .confirm_claim_proposal(input)
        .map_err(|error| error.to_string())
}

#[tauri::command]
pub fn reject_claim_proposal(
    proposal_id: String,
    state: tauri::State<'_, app_state::AppState>,
) -> Result<storage::ClaimExtractionJob, String> {
    state
        .reject_claim_proposal(&proposal_id)
        .map_err(|error| error.to_string())
}

fn resolve_company_id(
    state: &app_state::AppState,
    source_type: &str,
    source_id: &str,
) -> Result<String, String> {
    match source_type {
        "transcript" => {
            let job = state
                .get_transcript_job(source_id)
                .map_err(|error| error.to_string())?;
            job.company_id
                .ok_or_else(|| "the transcript is not linked to a company".to_owned())
        }
        "report_document" => state
            .get_report_document(source_id)
            .map(|document| document.company_id)
            .map_err(|error| error.to_string()),
        other => Err(format!("unsupported claim extraction source type: {other}")),
    }
}

fn spawn_extraction_job(state: app_state::AppState, job_id: String) {
    tauri::async_runtime::spawn_blocking(move || {
        if let Err(error) = jobs::claim_extraction::run_claim_extraction_job(&state, &job_id) {
            let _ = state.mark_claim_extraction_job_failed(&job_id, "extraction_failed", &error);
        }
    });
}
