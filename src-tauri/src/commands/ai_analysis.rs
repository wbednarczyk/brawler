use serde::Deserialize;

use crate::{
    app_state, jobs,
    providers::analysis::{TEST_SAMPLE_ANALYSIS_MODEL, TEST_SAMPLE_ANALYSIS_PROVIDER_ID},
    storage,
};

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct StartAiAnalysisInput {
    feed_item_id: String,
    prompt_preset_id: Option<String>,
    custom_question: Option<String>,
    provider_mode: Option<String>,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ListAiAnalysisInput {
    feed_item_id: String,
}

#[tauri::command]
pub async fn start_ai_analysis(
    input: StartAiAnalysisInput,
    state: tauri::State<'_, app_state::AppState>,
) -> Result<storage::AiAnalysisJob, String> {
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
        .create_ai_analysis_job(storage::NewAiAnalysisJob {
            feed_item_id: input.feed_item_id,
            prompt_preset_id: input.prompt_preset_id,
            custom_question: input.custom_question,
            provider_id,
            model,
            prompt_version: Some("m13.source_grounded.v1".to_owned()),
        })
        .map_err(|error| error.to_string())?;

    spawn_analysis_job(state.inner().clone(), job.id.clone());

    Ok(job)
}

#[tauri::command]
pub fn list_ai_analysis(
    input: ListAiAnalysisInput,
    state: tauri::State<'_, app_state::AppState>,
) -> Result<Vec<storage::AiAnalysisJob>, String> {
    state
        .list_ai_analysis_jobs(&input.feed_item_id)
        .map_err(|error| error.to_string())
}

#[tauri::command]
pub async fn retry_ai_analysis(
    job_id: String,
    state: tauri::State<'_, app_state::AppState>,
) -> Result<storage::AiAnalysisJob, String> {
    let job = state
        .get_ai_analysis_job(&job_id)
        .map_err(|error| error.to_string())?;

    spawn_analysis_job(state.inner().clone(), job.id.clone());

    Ok(job)
}

fn spawn_analysis_job(state: app_state::AppState, job_id: String) {
    tauri::async_runtime::spawn_blocking(move || {
        if let Err(error) = jobs::ai_analysis::run_ai_analysis_job(&state, &job_id) {
            let _ = state.mark_ai_analysis_job_failed(&job_id, "unknown", &error);
        }
    });
}
