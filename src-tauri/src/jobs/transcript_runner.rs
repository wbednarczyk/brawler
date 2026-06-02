use crate::{
    app_state,
    providers::{
        credentials,
        transcripts::{
            GeminiTranscriptProvider, TestSampleTranscriptProvider, VideoTranscriptProvider,
        },
    },
    storage,
};

pub fn run_video_transcript_job(
    state: &app_state::AppState,
    job_id: &str,
    provider_mode: Option<&str>,
) -> Result<storage::TranscriptJob, String> {
    let provider_mode = provider_mode
        .map(str::trim)
        .filter(|value| !value.is_empty())
        .unwrap_or("provider_gemini");
    let settings = state.get_settings().map_err(|error| error.to_string())?;
    let provider: Box<dyn VideoTranscriptProvider> = match provider_mode {
        "test_sample" => Box::new(TestSampleTranscriptProvider),
        "provider_gemini" => {
            let api_key = credentials::read_gemini_transcription_api_key().unwrap_or(None);
            Box::new(
                GeminiTranscriptProvider::live(
                    api_key,
                    settings.ai_providers.youtube_transcription_model.clone(),
                    settings.ai_providers.youtube_transcription_timeout_seconds,
                )
                .map_err(|error| error.to_string())?,
            )
        }
        other => return Err(format!("Unknown transcript provider mode: {other}")),
    };
    let job = state
        .get_transcript_job(job_id)
        .map_err(|error| error.to_string())?;

    if job.status == "completed" {
        return Ok(job);
    }

    state
        .mark_transcript_job_running(job_id)
        .map_err(|error| error.to_string())?;

    match provider.transcribe(&job) {
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

            state
                .mark_transcript_job_completed(job_id)
                .map_err(|error| error.to_string())
        }
        Err(error) => state
            .mark_transcript_job_failed(job_id, error.code(), &error.to_string())
            .map_err(|storage_error| storage_error.to_string()),
    }
}
