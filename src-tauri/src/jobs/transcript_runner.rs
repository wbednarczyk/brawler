use crate::{
    app_state,
    providers::transcripts::{registry, VideoTranscriptProvider},
    storage,
};

pub fn run_video_transcript_job(
    state: &app_state::AppState,
    job_id: &str,
    provider_mode: Option<&str>,
) -> Result<storage::TranscriptJob, String> {
    let started_at = std::time::Instant::now();
    let provider_mode = provider_mode
        .map(str::trim)
        .filter(|value| !value.is_empty())
        .unwrap_or("provider_gemini");
    let settings = state.get_settings().map_err(|error| error.to_string())?;
    let provider: Box<dyn VideoTranscriptProvider> = registry::build_transcript_provider(
        provider_mode,
        &settings.ai_providers.youtube_transcription_model,
        settings.ai_providers.youtube_transcription_timeout_seconds,
    )?;
    let job = state
        .get_transcript_job(job_id)
        .map_err(|error| error.to_string())?;

    if job.status == "completed" {
        return Ok(job);
    }

    state
        .mark_transcript_job_running(job_id)
        .map_err(|error| error.to_string())?;

    match tauri::async_runtime::block_on(provider.transcribe(&job)) {
        Ok(output) => {
            for segment in output.segments {
                state
                    .create_transcript_segment(storage::NewTranscriptSegment {
                        transcript_job_id: job_id.to_owned(),
                        company_id: job.company_id.clone(),
                        start_seconds: segment.start_seconds,
                        end_seconds: segment.end_seconds,
                        speaker: segment.speaker,
                        text: segment.text,
                        language: segment.language,
                    })
                    .map_err(|error| error.to_string())?;
            }

            let completed = state
                .mark_transcript_job_completed(job_id)
                .map_err(|error| error.to_string())?;
            record_transcript_metrics(state, &completed.provider_id, "succeeded", started_at);
            Ok(completed)
        }
        Err(error) => {
            let failed = state
                .mark_transcript_job_failed(job_id, error.code(), &error.to_string())
                .map_err(|storage_error| storage_error.to_string())?;
            record_transcript_metrics(state, &failed.provider_id, "failed", started_at);
            Ok(failed)
        }
    }
}

fn record_transcript_metrics(
    state: &app_state::AppState,
    provider_id: &str,
    status: &str,
    started_at: std::time::Instant,
) {
    state.increment_runtime_counter(
        "brawler_transcript_runs_total",
        &[("provider_id", provider_id), ("status", status)],
    );
    state.observe_runtime_duration_seconds(
        "brawler_transcript_duration_seconds",
        &[("provider_id", provider_id), ("status", status)],
        started_at.elapsed().as_secs_f64(),
    );
}
