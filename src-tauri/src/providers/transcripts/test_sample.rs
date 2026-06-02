use crate::storage::TranscriptJob;

use super::types::{
    TranscriptProviderError, TranscriptProviderOutput, TranscriptSegmentDraft,
    VideoTranscriptProvider,
};

pub struct TestSampleTranscriptProvider;

impl VideoTranscriptProvider for TestSampleTranscriptProvider {
    fn provider_id(&self) -> &'static str {
        "test_sample"
    }

    fn transcribe(
        &self,
        job: &TranscriptJob,
    ) -> Result<TranscriptProviderOutput, TranscriptProviderError> {
        if !job.source_url.contains("youtube.com") && !job.source_url.contains("youtu.be") {
            return Err(TranscriptProviderError::InvalidSourceUrl);
        }

        Ok(TranscriptProviderOutput {
            segments: vec![
                TranscriptSegmentDraft {
                    start_seconds: Some(0),
                    end_seconds: Some(42),
                    speaker: Some("Management".to_owned()),
                    text: "Welcome to the quarterly conference. We will discuss recent execution and priorities.".to_owned(),
                    language: Some("en".to_owned()),
                },
                TranscriptSegmentDraft {
                    start_seconds: Some(43),
                    end_seconds: Some(105),
                    speaker: Some("Management".to_owned()),
                    text: "The board expects the next product milestone to be delivered within two quarters.".to_owned(),
                    language: Some("en".to_owned()),
                },
            ],
            recognized_company_candidates: Vec::new(),
        })
    }
}

#[cfg(test)]
mod tests {
    use crate::providers::transcripts::VideoTranscriptProvider;

    use super::TestSampleTranscriptProvider;

    fn sample_job(source_url: &str) -> crate::storage::TranscriptJob {
        crate::storage::TranscriptJob {
            id: "job_01".to_owned(),
            company_id: None,
            company: None,
            company_name: None,
            provider_id: "provider_gemini".to_owned(),
            source_type: "youtube_url".to_owned(),
            source_url: source_url.to_owned(),
            source_label: None,
            company_resolution_status: "unresolved".to_owned(),
            recognized_company_candidates: Vec::new(),
            status: "queued".to_owned(),
            error_code: None,
            created_at: "2026-06-01T10:00:00Z".to_owned(),
            started_at: None,
            finished_at: None,
            error: None,
        }
    }

    #[test]
    fn test_sample_provider_returns_segments() {
        let provider = TestSampleTranscriptProvider;
        let output = provider
            .transcribe(&sample_job("https://www.youtube.com/watch?v=sample"))
            .expect("sample provider should return transcript segments");

        assert_eq!(provider.provider_id(), "test_sample");
        assert_eq!(output.segments.len(), 2);
        assert_eq!(output.segments[1].start_seconds, Some(43));
    }
}
