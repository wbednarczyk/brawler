//! Registry/factory for video transcript providers.
//!
//! Transcription is Gemini-only today, but dispatch goes through this registry
//! (rather than an inline `match` in the runner) so additional transcript
//! providers can be added here without touching the job.

use super::{GeminiTranscriptProvider, TestSampleTranscriptProvider, VideoTranscriptProvider};
use crate::providers::credentials;

/// Stable id for the Gemini transcript provider.
pub const GEMINI_TRANSCRIPT_PROVIDER_ID: &str = "provider_gemini";
/// Id for the deterministic offline test-sample transcript provider.
pub const TEST_SAMPLE_TRANSCRIPT_PROVIDER_ID: &str = "test_sample";

/// Build a boxed transcript provider for the given mode, model, and timeout.
pub fn build_transcript_provider(
    provider_id: &str,
    model: &str,
    timeout_seconds: i64,
) -> Result<Box<dyn VideoTranscriptProvider>, String> {
    match provider_id {
        TEST_SAMPLE_TRANSCRIPT_PROVIDER_ID => Ok(Box::new(TestSampleTranscriptProvider)),
        GEMINI_TRANSCRIPT_PROVIDER_ID => {
            let api_key = credentials::read_gemini_transcription_api_key().unwrap_or(None);
            Ok(Box::new(
                GeminiTranscriptProvider::live(api_key, model.to_owned(), timeout_seconds)
                    .map_err(|error| error.to_string())?,
            ))
        }
        other => Err(format!("Unknown transcript provider mode: {other}")),
    }
}
