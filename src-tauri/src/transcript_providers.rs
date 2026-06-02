use crate::storage::{CompanyLookupResult, TranscriptJob};
use reqwest::blocking::Client;
use serde::{Deserialize, Deserializer, Serialize};
use std::error::Error as StdError;
use std::time::Duration;
use thiserror::Error;

pub const DEFAULT_GEMINI_TRANSCRIPTION_MODEL: &str = "gemini-2.5-flash";
const GEMINI_REQUEST_TIMEOUT_SECONDS: u64 = 300;
const GEMINI_REQUEST_TIMEOUT_ENV: &str = "BRAWLER_GEMINI_REQUEST_TIMEOUT_SECONDS";

#[derive(Debug, Clone)]
pub struct TranscriptSegmentDraft {
    pub start_seconds: Option<i64>,
    pub end_seconds: Option<i64>,
    pub speaker: Option<String>,
    pub text: String,
    pub language: Option<String>,
}

#[derive(Debug, Clone)]
pub struct TranscriptProviderOutput {
    pub segments: Vec<TranscriptSegmentDraft>,
    pub recognized_company_candidates: Vec<CompanyLookupResult>,
}

#[derive(Debug, Error)]
pub enum TranscriptProviderError {
    #[error("provider is not configured")]
    ProviderNotConfigured,
    #[error("provider limit reached")]
    ProviderLimit,
    #[error("provider is temporarily unavailable: {0}")]
    ProviderUnavailable(String),
    #[error("provider error: {0}")]
    ProviderError(String),
    #[error("network error: {0}")]
    NetworkError(String),
    #[error("invalid source URL")]
    InvalidSourceUrl,
    #[error("parse error: {0}")]
    ParseError(String),
    #[error("unknown provider error: {0}")]
    Unknown(String),
}

impl TranscriptProviderError {
    pub fn code(&self) -> &'static str {
        match self {
            Self::ProviderNotConfigured => "provider_not_configured",
            Self::ProviderLimit => "provider_limit",
            Self::ProviderUnavailable(_) => "provider_unavailable",
            Self::ProviderError(_) => "provider_error",
            Self::NetworkError(_) => "network_error",
            Self::InvalidSourceUrl => "invalid_source_url",
            Self::ParseError(_) => "parse_error",
            Self::Unknown(_) => "unknown",
        }
    }
}

pub trait VideoTranscriptProvider {
    fn provider_id(&self) -> &'static str;
    fn transcribe(
        &self,
        job: &TranscriptJob,
    ) -> Result<TranscriptProviderOutput, TranscriptProviderError>;
}

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

pub trait GeminiGenerateContentClient {
    fn generate_content(
        &self,
        model: &str,
        api_key: &str,
        request: &GeminiGenerateContentRequest,
    ) -> Result<GeminiGenerateContentResponse, TranscriptProviderError>;
}

pub struct ReqwestGeminiGenerateContentClient {
    client: Client,
}

impl ReqwestGeminiGenerateContentClient {
    pub fn new(configured_timeout_seconds: u64) -> Result<Self, TranscriptProviderError> {
        let timeout_seconds = effective_gemini_request_timeout_seconds(configured_timeout_seconds);
        let client = Client::builder()
            .timeout(Duration::from_secs(timeout_seconds))
            .build()
            .map_err(|error| {
                TranscriptProviderError::NetworkError(describe_reqwest_error(&error))
            })?;

        Ok(Self { client })
    }
}

fn effective_gemini_request_timeout_seconds(configured_timeout_seconds: u64) -> u64 {
    std::env::var(GEMINI_REQUEST_TIMEOUT_ENV)
        .ok()
        .and_then(|value| value.trim().parse::<u64>().ok())
        .filter(|value| *value > 0)
        .unwrap_or_else(|| configured_timeout_seconds.max(1))
}

impl GeminiGenerateContentClient for ReqwestGeminiGenerateContentClient {
    fn generate_content(
        &self,
        model: &str,
        api_key: &str,
        request: &GeminiGenerateContentRequest,
    ) -> Result<GeminiGenerateContentResponse, TranscriptProviderError> {
        let endpoint = format!(
            "https://generativelanguage.googleapis.com/v1beta/models/{model}:generateContent"
        );
        let response = self
            .client
            .post(endpoint)
            .header("x-goog-api-key", api_key)
            .json(request)
            .send()
            .map_err(|error| {
                TranscriptProviderError::NetworkError(describe_reqwest_error(&error))
            })?;
        let status = response.status();
        let body = response.text().map_err(|error| {
            TranscriptProviderError::NetworkError(describe_reqwest_error(&error))
        })?;

        if !status.is_success() {
            return Err(map_gemini_http_error(status.as_u16(), &body));
        }

        serde_json::from_str(&body).map_err(|error| {
            TranscriptProviderError::ParseError(format!("Gemini response JSON: {error}"))
        })
    }
}

fn describe_reqwest_error(error: &reqwest::Error) -> String {
    let mut details = vec![error.to_string()];
    let mut kinds = Vec::new();

    if error.is_timeout() {
        kinds.push("timeout");
    }
    if error.is_connect() {
        kinds.push("connect");
    }
    if error.is_request() {
        kinds.push("request");
    }
    if error.is_body() {
        kinds.push("body");
    }
    if error.is_decode() {
        kinds.push("decode");
    }
    if error.is_status() {
        kinds.push("status");
    }
    if !kinds.is_empty() {
        details.push(format!("kind={}", kinds.join(",")));
    }
    if let Some(url) = error.url() {
        details.push(format!("url={url}"));
    }

    let mut source = error.source();
    while let Some(cause) = source {
        details.push(format!("cause={cause}"));
        source = cause.source();
    }

    details.join("; ")
}

pub struct GeminiTranscriptProvider<C = ReqwestGeminiGenerateContentClient> {
    api_key: Option<String>,
    model: String,
    client: C,
}

impl GeminiTranscriptProvider<ReqwestGeminiGenerateContentClient> {
    pub fn live(
        api_key: Option<String>,
        model: impl Into<String>,
        timeout_seconds: i64,
    ) -> Result<Self, TranscriptProviderError> {
        let timeout_seconds = u64::try_from(timeout_seconds)
            .ok()
            .filter(|value| *value > 0)
            .unwrap_or(GEMINI_REQUEST_TIMEOUT_SECONDS);
        Ok(Self {
            api_key,
            model: model.into(),
            client: ReqwestGeminiGenerateContentClient::new(timeout_seconds)?,
        })
    }
}

impl<C> GeminiTranscriptProvider<C>
where
    C: GeminiGenerateContentClient,
{
    pub fn with_client(api_key: Option<String>, model: impl Into<String>, client: C) -> Self {
        Self {
            api_key,
            model: model.into(),
            client,
        }
    }
}

impl<C> VideoTranscriptProvider for GeminiTranscriptProvider<C>
where
    C: GeminiGenerateContentClient,
{
    fn provider_id(&self) -> &'static str {
        "provider_gemini"
    }

    fn transcribe(
        &self,
        job: &TranscriptJob,
    ) -> Result<TranscriptProviderOutput, TranscriptProviderError> {
        validate_youtube_url(&job.source_url)?;
        let api_key = self
            .api_key
            .as_deref()
            .filter(|value| !value.trim().is_empty())
            .ok_or(TranscriptProviderError::ProviderNotConfigured)?;
        let request = gemini_transcript_request(&job.source_url);
        let response = self
            .client
            .generate_content(&self.model, api_key, &request)?;
        let output_text = extract_gemini_text(&response)?;
        let segments = parse_gemini_transcript_segments(&output_text)?;

        Ok(TranscriptProviderOutput {
            segments,
            recognized_company_candidates: Vec::new(),
        })
    }
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct GeminiGenerateContentRequest {
    contents: Vec<GeminiContent>,
    generation_config: GeminiGenerationConfig,
}

#[derive(Debug, Serialize)]
pub struct GeminiContent {
    parts: Vec<GeminiPart>,
}

#[derive(Debug, Serialize)]
#[serde(untagged)]
pub enum GeminiPart {
    Text { text: String },
    FileData { file_data: GeminiFileData },
}

#[derive(Debug, Serialize)]
pub struct GeminiFileData {
    file_uri: String,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct GeminiGenerationConfig {
    response_mime_type: String,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct GeminiGenerateContentResponse {
    candidates: Vec<GeminiCandidate>,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct GeminiCandidate {
    content: GeminiResponseContent,
}

#[derive(Debug, Deserialize)]
pub struct GeminiResponseContent {
    parts: Vec<GeminiResponsePart>,
}

#[derive(Debug, Deserialize)]
pub struct GeminiResponsePart {
    text: Option<String>,
}

#[derive(Debug, Deserialize)]
pub struct GeminiTranscriptJson {
    segments: Vec<GeminiTranscriptJsonSegment>,
}

#[derive(Debug, Deserialize)]
pub struct GeminiTranscriptJsonSegment {
    #[serde(default, deserialize_with = "deserialize_optional_timestamp_seconds")]
    start_seconds: Option<i64>,
    #[serde(default, deserialize_with = "deserialize_optional_timestamp_seconds")]
    end_seconds: Option<i64>,
    speaker: Option<String>,
    text: String,
    language: Option<String>,
}

fn deserialize_optional_timestamp_seconds<'de, D>(deserializer: D) -> Result<Option<i64>, D::Error>
where
    D: Deserializer<'de>,
{
    let value = Option::<serde_json::Value>::deserialize(deserializer)?;
    let Some(value) = value else {
        return Ok(None);
    };

    match value {
        serde_json::Value::Number(number) => {
            if let Some(integer) = number.as_i64() {
                return Ok(Some(integer));
            }
            if let Some(decimal) = number.as_f64() {
                return Ok(Some(decimal.floor() as i64));
            }
            Err(serde::de::Error::custom(
                "timestamp must be a finite number of seconds",
            ))
        }
        serde_json::Value::Null => Ok(None),
        _ => Err(serde::de::Error::custom(
            "timestamp must be null or a number of seconds",
        )),
    }
}

fn gemini_transcript_request(source_url: &str) -> GeminiGenerateContentRequest {
    GeminiGenerateContentRequest {
        contents: vec![GeminiContent {
            parts: vec![
                GeminiPart::FileData {
                    file_data: GeminiFileData {
                        file_uri: source_url.to_owned(),
                    },
                },
                GeminiPart::Text {
                    text: "Transcribe this YouTube video for an investor research notebook. Return only JSON with this exact shape: {\"segments\":[{\"start_seconds\":null,\"end_seconds\":null,\"speaker\":null,\"text\":\"...\",\"language\":\"pl\"}]}. Use timestamps when available. Do not include markdown fences, commentary, recommendations, or investment advice.".to_owned(),
                },
            ],
        }],
        generation_config: GeminiGenerationConfig {
            response_mime_type: "application/json".to_owned(),
        },
    }
}

fn extract_gemini_text(
    response: &GeminiGenerateContentResponse,
) -> Result<String, TranscriptProviderError> {
    response
        .candidates
        .first()
        .and_then(|candidate| {
            candidate
                .content
                .parts
                .iter()
                .find_map(|part| part.text.clone())
        })
        .map(|text| text.trim().to_owned())
        .filter(|text| !text.is_empty())
        .ok_or_else(|| {
            TranscriptProviderError::ParseError("Gemini response did not include text".to_owned())
        })
}

fn parse_gemini_transcript_segments(
    text: &str,
) -> Result<Vec<TranscriptSegmentDraft>, TranscriptProviderError> {
    let json_text = extract_json_object(text);
    let parsed: GeminiTranscriptJson = serde_json::from_str(json_text).map_err(|error| {
        TranscriptProviderError::ParseError(format!("Gemini transcript JSON: {error}"))
    })?;
    let segments: Vec<TranscriptSegmentDraft> = parsed
        .segments
        .into_iter()
        .map(|segment| TranscriptSegmentDraft {
            start_seconds: segment.start_seconds,
            end_seconds: segment.end_seconds,
            speaker: clean_optional_string(segment.speaker),
            text: segment.text.trim().to_owned(),
            language: clean_optional_string(segment.language),
        })
        .filter(|segment| !segment.text.is_empty())
        .collect();

    if segments.is_empty() {
        return Err(TranscriptProviderError::ParseError(
            "Gemini transcript did not contain usable segments".to_owned(),
        ));
    }

    Ok(segments)
}

fn extract_json_object(text: &str) -> &str {
    let trimmed = text.trim();
    if let Some(stripped) = trimmed
        .strip_prefix("```json")
        .or_else(|| trimmed.strip_prefix("```"))
        .and_then(|value| value.strip_suffix("```"))
    {
        return stripped.trim();
    }

    trimmed
}

fn clean_optional_string(value: Option<String>) -> Option<String> {
    value
        .map(|value| value.trim().to_owned())
        .filter(|value| !value.is_empty())
}

fn validate_youtube_url(source_url: &str) -> Result<(), TranscriptProviderError> {
    let url = url::Url::parse(source_url).map_err(|_| TranscriptProviderError::InvalidSourceUrl)?;
    let host = url
        .host_str()
        .map(str::to_lowercase)
        .ok_or(TranscriptProviderError::InvalidSourceUrl)?;

    if host == "youtu.be" || host == "youtube.com" || host.ends_with(".youtube.com") {
        return Ok(());
    }

    Err(TranscriptProviderError::InvalidSourceUrl)
}

fn map_gemini_http_error(status: u16, body: &str) -> TranscriptProviderError {
    let cause = summarize_gemini_error_body(body);
    match status {
        401 | 403 => TranscriptProviderError::ProviderNotConfigured,
        429 => TranscriptProviderError::ProviderLimit,
        503 => TranscriptProviderError::ProviderUnavailable(format!(
            "Gemini service unavailable: {cause}"
        )),
        400 => TranscriptProviderError::ProviderError(format!(
            "Gemini rejected the YouTube URL or request: {cause}"
        )),
        500..=599 => TranscriptProviderError::ProviderError(format!(
            "Gemini service error ({status}): {cause}"
        )),
        _ => TranscriptProviderError::ProviderError(format!("Gemini error ({status}): {cause}")),
    }
}

fn summarize_gemini_error_body(body: &str) -> String {
    #[derive(Deserialize)]
    struct ErrorEnvelope {
        error: Option<ErrorBody>,
    }

    #[derive(Deserialize)]
    struct ErrorBody {
        message: Option<String>,
        status: Option<String>,
    }

    serde_json::from_str::<ErrorEnvelope>(body)
        .ok()
        .and_then(|envelope| envelope.error)
        .map(|error| match (error.status, error.message) {
            (Some(status), Some(message)) => format!("{status}: {message}"),
            (Some(status), None) => status,
            (None, Some(message)) => message,
            (None, None) => "provider returned an error without a message".to_owned(),
        })
        .unwrap_or_else(|| {
            let trimmed = body.trim();
            if trimmed.is_empty() {
                "provider returned an empty error body".to_owned()
            } else {
                trimmed.chars().take(500).collect()
            }
        })
}

#[cfg(test)]
mod tests {
    use super::*;

    struct MockGeminiClient {
        response_text: String,
    }

    impl GeminiGenerateContentClient for MockGeminiClient {
        fn generate_content(
            &self,
            model: &str,
            api_key: &str,
            request: &GeminiGenerateContentRequest,
        ) -> Result<GeminiGenerateContentResponse, TranscriptProviderError> {
            assert_eq!(model, DEFAULT_GEMINI_TRANSCRIPTION_MODEL);
            assert_eq!(api_key, "test-api-key");
            assert_eq!(request.contents.len(), 1);
            assert_eq!(request.contents[0].parts.len(), 2);

            match &request.contents[0].parts[0] {
                GeminiPart::FileData { file_data } => {
                    assert_eq!(file_data.file_uri, "https://www.youtube.com/watch?v=sample");
                }
                GeminiPart::Text { .. } => panic!("first request part should be file data"),
            }

            match &request.contents[0].parts[1] {
                GeminiPart::Text { text } => assert!(text.contains("Transcribe")),
                GeminiPart::FileData { .. } => panic!("second request part should be text"),
            }

            Ok(GeminiGenerateContentResponse {
                candidates: vec![GeminiCandidate {
                    content: GeminiResponseContent {
                        parts: vec![GeminiResponsePart {
                            text: Some(self.response_text.clone()),
                        }],
                    },
                }],
            })
        }
    }

    fn sample_job(source_url: &str) -> TranscriptJob {
        TranscriptJob {
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

    #[test]
    fn gemini_transcript_parser_accepts_fractional_seconds() {
        let segments = parse_gemini_transcript_segments(
            r#"{"segments":[{"start_seconds":0.861,"end_seconds":4.277,"speaker":null,"text":"Hello.","language":"en"}]}"#,
        )
        .expect("fractional timestamps should parse");

        assert_eq!(segments.len(), 1);
        assert_eq!(segments[0].start_seconds, Some(0));
        assert_eq!(segments[0].end_seconds, Some(4));
    }

    #[test]
    fn gemini_provider_requires_configuration() {
        let provider = GeminiTranscriptProvider::with_client(
            None,
            DEFAULT_GEMINI_TRANSCRIPTION_MODEL,
            MockGeminiClient {
                response_text: "{}".to_owned(),
            },
        );
        let error = provider
            .transcribe(&sample_job("https://www.youtube.com/watch?v=sample"))
            .expect_err("unconfigured Gemini should fail");

        assert_eq!(provider.provider_id(), "provider_gemini");
        assert_eq!(error.code(), "provider_not_configured");
    }

    #[test]
    fn gemini_provider_parses_structured_segments() {
        let provider = GeminiTranscriptProvider::with_client(
            Some("test-api-key".to_owned()),
            DEFAULT_GEMINI_TRANSCRIPTION_MODEL,
            MockGeminiClient {
                response_text: r#"{"segments":[{"start_seconds":1,"end_seconds":9,"speaker":"CEO","text":"Revenue guidance remains unchanged.","language":"en"}]}"#.to_owned(),
            },
        );

        let output = provider
            .transcribe(&sample_job("https://www.youtube.com/watch?v=sample"))
            .expect("mock Gemini response should parse");

        assert_eq!(output.segments.len(), 1);
        assert_eq!(output.segments[0].start_seconds, Some(1));
        assert_eq!(output.segments[0].end_seconds, Some(9));
        assert_eq!(output.segments[0].speaker.as_deref(), Some("CEO"));
        assert_eq!(output.segments[0].language.as_deref(), Some("en"));
        assert_eq!(
            output.segments[0].text,
            "Revenue guidance remains unchanged."
        );
    }

    #[test]
    fn gemini_provider_rejects_non_youtube_urls_before_network_call() {
        let provider = GeminiTranscriptProvider::with_client(
            Some("test-api-key".to_owned()),
            DEFAULT_GEMINI_TRANSCRIPTION_MODEL,
            MockGeminiClient {
                response_text: "{}".to_owned(),
            },
        );
        let error = provider
            .transcribe(&sample_job("https://example.com/video"))
            .expect_err("non-YouTube URL should be rejected");

        assert_eq!(error.code(), "invalid_source_url");
    }

    #[test]
    fn gemini_http_error_includes_provider_cause() {
        let error = map_gemini_http_error(
            400,
            r#"{"error":{"status":"INVALID_ARGUMENT","message":"Video is unavailable"}}"#,
        );

        assert_eq!(error.code(), "provider_error");
        assert!(error
            .to_string()
            .contains("INVALID_ARGUMENT: Video is unavailable"));
    }

    #[test]
    fn gemini_unavailable_maps_to_temporary_provider_state() {
        let error = map_gemini_http_error(
            503,
            r#"{"error":{"status":"UNAVAILABLE","message":"This model is currently experiencing high demand."}}"#,
        );

        assert_eq!(error.code(), "provider_unavailable");
        assert!(error.to_string().contains("UNAVAILABLE"));
        assert!(error.to_string().contains("high demand"));
    }

    #[test]
    #[ignore = "live Gemini smoke test; requires GEMINI_API_KEY and BRAWLER_GEMINI_SMOKE_YOUTUBE_URL"]
    fn live_gemini_transcribes_youtube_url() -> Result<(), String> {
        let api_key = std::env::var("GEMINI_API_KEY")
            .map_err(|_| "set GEMINI_API_KEY to run the live Gemini transcript smoke test")?;
        let source_url = std::env::var("BRAWLER_GEMINI_SMOKE_YOUTUBE_URL").map_err(|_| {
            "set BRAWLER_GEMINI_SMOKE_YOUTUBE_URL to a supported public YouTube URL"
        })?;
        let model = std::env::var("BRAWLER_GEMINI_SMOKE_MODEL")
            .unwrap_or_else(|_| DEFAULT_GEMINI_TRANSCRIPTION_MODEL.to_owned());
        let provider = GeminiTranscriptProvider::live(
            Some(api_key),
            model.clone(),
            GEMINI_REQUEST_TIMEOUT_SECONDS as i64,
        )
        .map_err(|error| format!("live Gemini provider should initialize: {error}"))?;
        let output = provider
            .transcribe(&sample_job(&source_url))
            .map_err(|error| {
                format!(
                    "live Gemini transcript failed: model={model}, code={}, error={error}",
                    error.code()
                )
            })?;

        assert!(
            !output.segments.is_empty(),
            "live Gemini smoke test must create at least one transcript segment"
        );
        eprintln!(
            "Live Gemini smoke passed: model={model}, segments={}",
            output.segments.len()
        );
        Ok(())
    }
}
