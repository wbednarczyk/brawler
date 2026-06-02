use serde::Deserialize;

use crate::{app_state, jobs, storage};

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct RunVideoTranscriptJobInput {
    job_id: String,
    provider_mode: Option<String>,
}

#[tauri::command]
pub fn list_video_transcript_jobs(
    input: storage::TranscriptJobListInput,
    state: tauri::State<'_, app_state::AppState>,
) -> Result<Vec<storage::TranscriptJob>, String> {
    state
        .list_transcript_jobs(input)
        .map_err(|error| error.to_string())
}

#[tauri::command]
pub fn delete_video_transcript_job(
    job_id: String,
    state: tauri::State<'_, app_state::AppState>,
) -> Result<(), String> {
    state
        .delete_transcript_job(&job_id)
        .map_err(|error| error.to_string())
}

#[tauri::command]
pub fn create_video_transcript_job(
    input: storage::NewTranscriptJob,
    state: tauri::State<'_, app_state::AppState>,
) -> Result<storage::TranscriptJob, String> {
    state
        .create_transcript_job(input)
        .map_err(|error| error.to_string())
}

#[tauri::command]
pub fn update_video_transcript_job(
    input: storage::UpdateTranscriptJobInput,
    state: tauri::State<'_, app_state::AppState>,
) -> Result<storage::TranscriptJob, String> {
    state
        .update_transcript_job(input)
        .map_err(|error| error.to_string())
}

#[tauri::command]
pub fn list_transcript_segments(
    transcript_job_id: String,
    state: tauri::State<'_, app_state::AppState>,
) -> Result<Vec<storage::TranscriptSegment>, String> {
    state
        .list_transcript_segments(&transcript_job_id)
        .map_err(|error| error.to_string())
}

#[tauri::command]
pub fn resolve_transcript_job_company(
    input: storage::ResolveTranscriptJobCompanyInput,
    state: tauri::State<'_, app_state::AppState>,
) -> Result<storage::TranscriptJob, String> {
    state
        .resolve_transcript_job_company(input)
        .map_err(|error| error.to_string())
}

#[tauri::command]
pub async fn run_video_transcript_job(
    input: RunVideoTranscriptJobInput,
    state: tauri::State<'_, app_state::AppState>,
) -> Result<storage::TranscriptJob, String> {
    let state = state.inner().clone();

    jobs::scheduler::run_blocking_task(move || {
        jobs::transcript_runner::run_video_transcript_job(
            &state,
            &input.job_id,
            input.provider_mode.as_deref(),
        )
    })
    .await
}
