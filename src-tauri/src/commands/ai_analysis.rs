use serde::Deserialize;
use serde_json::json;

use crate::{
    app_state, jobs,
    providers::analysis::{
        capabilities::CAPABILITY_ROUTED_PROVIDER_ID, TEST_SAMPLE_ANALYSIS_MODEL,
        TEST_SAMPLE_ANALYSIS_PROVIDER_ID,
    },
    storage,
};

#[derive(Debug, Deserialize)]
#[cfg_attr(feature = "ts-export", derive(ts_rs::TS))]
#[cfg_attr(
    feature = "ts-export",
    ts(export, export_to = "../../src/api/generated/", optional_fields)
)]
#[serde(rename_all = "camelCase")]
pub struct StartAiAnalysisInput {
    feed_item_id: String,
    prompt_preset_id: Option<String>,
    custom_question: Option<String>,
    provider_mode: Option<String>,
}

#[derive(Debug, Deserialize)]
#[cfg_attr(feature = "ts-export", derive(ts_rs::TS))]
#[cfg_attr(
    feature = "ts-export",
    ts(export, export_to = "../../src/api/generated/")
)]
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
    let explicit_provider_id = input
        .provider_mode
        .as_deref()
        .map(str::trim)
        .filter(|value| !value.is_empty());
    let (provider_id, model) = match explicit_provider_id {
        Some(provider_id) => {
            let model = if provider_id == TEST_SAMPLE_ANALYSIS_PROVIDER_ID {
                TEST_SAMPLE_ANALYSIS_MODEL.to_owned()
            } else {
                settings.ai_providers.general_analysis_model
            };
            (provider_id.to_owned(), model)
        }
        // No explicit override: defer provider selection to capability routing
        // at run time (ADR 0060 as amended) instead of guessing a default here.
        None => (
            CAPABILITY_ROUTED_PROVIDER_ID.to_owned(),
            CAPABILITY_ROUTED_PROVIDER_ID.to_owned(),
        ),
    };
    let job = state
        .create_ai_analysis_job(storage::NewAiAnalysisJob {
            feed_item_id: input.feed_item_id,
            prompt_preset_id: input.prompt_preset_id,
            custom_question: input.custom_question,
            provider_id,
            model,
            prompt_version: Some("m13.source_grounded.v2".to_owned()),
        })
        .map_err(|error| error.to_string())?;
    record_ai_analysis_diagnostic(
        &state,
        &job,
        "queued",
        "info",
        "AI analysis job queued.",
        json!({
            "feedItemId": job.feed_item_id,
            "providerId": job.provider_id,
            "model": job.model,
            "promptPresetId": job.prompt_preset_id,
            "promptVersion": job.prompt_version,
            "hasCustomQuestion": job.custom_question.is_some()
        }),
    );

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
    record_ai_analysis_diagnostic(
        &state,
        &job,
        "queued",
        "info",
        "AI analysis job retry queued.",
        json!({
            "feedItemId": job.feed_item_id,
            "providerId": job.provider_id,
            "model": job.model,
            "promptPresetId": job.prompt_preset_id,
            "promptVersion": job.prompt_version,
            "hasCustomQuestion": job.custom_question.is_some()
        }),
    );

    spawn_analysis_job(state.inner().clone(), job.id.clone());

    Ok(job)
}

fn record_ai_analysis_diagnostic(
    state: &app_state::AppState,
    job: &storage::AiAnalysisJob,
    stage: &str,
    severity: &str,
    message: &str,
    metadata: serde_json::Value,
) {
    let _ = state.record_diagnostic_event(storage::NewDiagnosticEvent {
        occurred_at: None,
        module: "ai_analysis".to_owned(),
        scope: Some(storage::DiagnosticScope {
            scope_type: "ai_analysis_job".to_owned(),
            id: Some(job.id.clone()),
        }),
        stage: stage.to_owned(),
        severity: severity.to_owned(),
        message: message.to_owned(),
        metadata: Some(metadata),
    });
}

fn spawn_analysis_job(state: app_state::AppState, job_id: String) {
    jobs::handlers::enqueue_per_job(&state, jobs::handlers::AI_ANALYSIS_KIND, &job_id);
}
