use async_trait::async_trait;
use reqwest::Client;
use serde::{Deserialize, Serialize};
use std::time::Duration;

use crate::providers::common::{
    describe_reqwest_error, effective_timeout_seconds, summarize_provider_error_body,
};

use super::prompts;
use super::types::{
    AiAnalysisProvider, AnalysisDocument, AnalysisProviderError, AnalysisProviderOutput,
    AnalysisRequest, DocumentSupport, ResearchBriefProviderOutput, ResearchBriefRequest,
    ResearchDigestRequest,
};

pub const DEFAULT_GEMINI_ANALYSIS_MODEL: &str = "gemini-2.5-flash";
const GEMINI_ANALYSIS_TIMEOUT_SECONDS: u64 = 90;
const GEMINI_ANALYSIS_TIMEOUT_ENV: &str = "BRAWLER_GEMINI_ANALYSIS_TIMEOUT_SECONDS";

#[async_trait]
pub trait GeminiAnalysisGenerateContentClient: Send + Sync {
    async fn generate_content(
        &self,
        model: &str,
        api_key: &str,
        request: &GeminiAnalysisGenerateContentRequest,
    ) -> Result<GeminiAnalysisGenerateContentResponse, AnalysisProviderError>;
}

pub struct ReqwestGeminiAnalysisGenerateContentClient {
    client: Client,
}

impl ReqwestGeminiAnalysisGenerateContentClient {
    pub fn new(configured_timeout_seconds: u64) -> Result<Self, AnalysisProviderError> {
        let timeout_seconds =
            effective_timeout_seconds(GEMINI_ANALYSIS_TIMEOUT_ENV, configured_timeout_seconds);
        let client = Client::builder()
            .timeout(Duration::from_secs(timeout_seconds))
            .build()
            .map_err(|error| AnalysisProviderError::NetworkError(describe_reqwest_error(&error)))?;

        Ok(Self { client })
    }
}

#[async_trait]
impl GeminiAnalysisGenerateContentClient for ReqwestGeminiAnalysisGenerateContentClient {
    async fn generate_content(
        &self,
        model: &str,
        api_key: &str,
        request: &GeminiAnalysisGenerateContentRequest,
    ) -> Result<GeminiAnalysisGenerateContentResponse, AnalysisProviderError> {
        let endpoint = format!(
            "https://generativelanguage.googleapis.com/v1beta/models/{model}:generateContent"
        );
        let response = self
            .client
            .post(endpoint)
            .header("x-goog-api-key", api_key)
            .json(request)
            .send()
            .await
            .map_err(|error| AnalysisProviderError::NetworkError(describe_reqwest_error(&error)))?;
        let status = response.status();
        let body = response
            .text()
            .await
            .map_err(|error| AnalysisProviderError::NetworkError(describe_reqwest_error(&error)))?;

        if !status.is_success() {
            return Err(map_gemini_analysis_http_error(status.as_u16(), &body));
        }

        serde_json::from_str(&body).map_err(|error| {
            AnalysisProviderError::ParseError(format!("Gemini response JSON: {error}"))
        })
    }
}

pub struct GeminiAnalysisProvider<C = ReqwestGeminiAnalysisGenerateContentClient> {
    api_key: Option<String>,
    model: String,
    client: C,
}

impl GeminiAnalysisProvider<ReqwestGeminiAnalysisGenerateContentClient> {
    pub fn live(
        api_key: Option<String>,
        model: impl Into<String>,
        timeout_seconds: i64,
    ) -> Result<Self, AnalysisProviderError> {
        let timeout_seconds = u64::try_from(timeout_seconds)
            .ok()
            .filter(|value| *value > 0)
            .unwrap_or(GEMINI_ANALYSIS_TIMEOUT_SECONDS);
        Ok(Self {
            api_key,
            model: model.into(),
            client: ReqwestGeminiAnalysisGenerateContentClient::new(timeout_seconds)?,
        })
    }
}

impl<C> GeminiAnalysisProvider<C>
where
    C: GeminiAnalysisGenerateContentClient,
{
    pub fn with_client(api_key: Option<String>, model: impl Into<String>, client: C) -> Self {
        Self {
            api_key,
            model: model.into(),
            client,
        }
    }
}

#[async_trait]
impl<C> AiAnalysisProvider for GeminiAnalysisProvider<C>
where
    C: GeminiAnalysisGenerateContentClient,
{
    fn provider_id(&self) -> &'static str {
        "provider_gemini"
    }

    fn model(&self) -> &str {
        &self.model
    }

    async fn analyze(
        &self,
        request: &AnalysisRequest,
    ) -> Result<AnalysisProviderOutput, AnalysisProviderError> {
        let api_key = self
            .api_key
            .as_deref()
            .filter(|value| !value.trim().is_empty())
            .ok_or(AnalysisProviderError::ProviderNotConfigured)?;
        let gemini_request = gemini_analysis_request(request);
        let response = self
            .client
            .generate_content(&self.model, api_key, &gemini_request)
            .await?;
        let output_text = extract_gemini_analysis_text(&response)?;
        prompts::parse_analysis_output(&output_text, "Gemini")
    }

    async fn generate_research_brief(
        &self,
        request: &ResearchBriefRequest,
    ) -> Result<ResearchBriefProviderOutput, AnalysisProviderError> {
        let api_key = self
            .api_key
            .as_deref()
            .filter(|value| !value.trim().is_empty())
            .ok_or(AnalysisProviderError::ProviderNotConfigured)?;
        let gemini_request = gemini_research_brief_request(request);
        let response = self
            .client
            .generate_content(&self.model, api_key, &gemini_request)
            .await?;
        let output_text = extract_gemini_analysis_text(&response)?;
        prompts::parse_research_brief_output(&output_text, "Gemini")
    }

    async fn generate_research_digest(
        &self,
        request: &ResearchDigestRequest,
    ) -> Result<ResearchBriefProviderOutput, AnalysisProviderError> {
        let api_key = self
            .api_key
            .as_deref()
            .filter(|value| !value.trim().is_empty())
            .ok_or(AnalysisProviderError::ProviderNotConfigured)?;
        let gemini_request = gemini_research_digest_request(request);
        let response = self
            .client
            .generate_content(&self.model, api_key, &gemini_request)
            .await?;
        let output_text = extract_gemini_analysis_text(&response)?;
        prompts::parse_research_brief_output(&output_text, "Gemini")
    }

    fn document_support(&self) -> DocumentSupport {
        DocumentSupport::Native
    }

    async fn complete_document(
        &self,
        prompt: &str,
        document: &AnalysisDocument,
    ) -> Result<String, AnalysisProviderError> {
        let api_key = self
            .api_key
            .as_deref()
            .filter(|value| !value.trim().is_empty())
            .ok_or(AnalysisProviderError::ProviderNotConfigured)?;
        let request = GeminiAnalysisGenerateContentRequest {
            contents: vec![GeminiAnalysisContent {
                parts: vec![
                    GeminiAnalysisPart::Text {
                        text: prompt.to_owned(),
                    },
                    gemini_document_part(document),
                ],
            }],
            generation_config: GeminiAnalysisGenerationConfig {
                response_mime_type: "application/json".to_owned(),
                max_output_tokens: GEMINI_MAX_OUTPUT_TOKENS,
            },
        };
        let response = self
            .client
            .generate_content(&self.model, api_key, &request)
            .await?;
        extract_gemini_analysis_text(&response)
    }
}

fn gemini_document_part(document: &AnalysisDocument) -> GeminiAnalysisPart {
    use base64::Engine as _;
    match document {
        AnalysisDocument::Native { mime_type, data } => GeminiAnalysisPart::InlineData {
            inline_data: GeminiInlineData {
                mime_type: mime_type.clone(),
                data: base64::engine::general_purpose::STANDARD.encode(data),
            },
        },
        AnalysisDocument::Text { text } => GeminiAnalysisPart::Text { text: text.clone() },
    }
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct GeminiAnalysisGenerateContentRequest {
    contents: Vec<GeminiAnalysisContent>,
    generation_config: GeminiAnalysisGenerationConfig,
}

#[derive(Debug, Serialize)]
pub struct GeminiAnalysisContent {
    parts: Vec<GeminiAnalysisPart>,
}

#[derive(Debug, Serialize)]
#[serde(untagged)]
pub enum GeminiAnalysisPart {
    Text { text: String },
    InlineData { inline_data: GeminiInlineData },
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "snake_case")]
pub struct GeminiInlineData {
    mime_type: String,
    data: String,
}

/// Generous output budget so thorough multi-KPI extraction is not truncated.
const GEMINI_MAX_OUTPUT_TOKENS: u32 = 16384;

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct GeminiAnalysisGenerationConfig {
    response_mime_type: String,
    max_output_tokens: u32,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct GeminiAnalysisGenerateContentResponse {
    candidates: Vec<GeminiAnalysisCandidate>,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct GeminiAnalysisCandidate {
    content: GeminiAnalysisResponseContent,
}

#[derive(Debug, Deserialize)]
pub struct GeminiAnalysisResponseContent {
    parts: Vec<GeminiAnalysisResponsePart>,
}

#[derive(Debug, Deserialize)]
pub struct GeminiAnalysisResponsePart {
    text: Option<String>,
}

fn gemini_text_request(prompt: String) -> GeminiAnalysisGenerateContentRequest {
    GeminiAnalysisGenerateContentRequest {
        contents: vec![GeminiAnalysisContent {
            parts: vec![GeminiAnalysisPart::Text { text: prompt }],
        }],
        generation_config: GeminiAnalysisGenerationConfig {
            response_mime_type: "application/json".to_owned(),
            max_output_tokens: GEMINI_MAX_OUTPUT_TOKENS,
        },
    }
}

fn gemini_analysis_request(request: &AnalysisRequest) -> GeminiAnalysisGenerateContentRequest {
    gemini_text_request(prompts::analysis_prompt(request))
}

fn gemini_research_brief_request(
    request: &ResearchBriefRequest,
) -> GeminiAnalysisGenerateContentRequest {
    gemini_text_request(prompts::research_brief_prompt(request))
}

fn gemini_research_digest_request(
    request: &ResearchDigestRequest,
) -> GeminiAnalysisGenerateContentRequest {
    gemini_text_request(prompts::research_digest_prompt(request))
}

fn extract_gemini_analysis_text(
    response: &GeminiAnalysisGenerateContentResponse,
) -> Result<String, AnalysisProviderError> {
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
            AnalysisProviderError::ParseError("Gemini response did not include text".to_owned())
        })
}

fn map_gemini_analysis_http_error(status: u16, body: &str) -> AnalysisProviderError {
    let cause = summarize_provider_error_body(body);
    match status {
        401 | 403 => AnalysisProviderError::ProviderNotConfigured,
        429 => AnalysisProviderError::ProviderLimit,
        503 => AnalysisProviderError::ProviderUnavailable(format!(
            "Gemini service unavailable: {cause}"
        )),
        400 => AnalysisProviderError::ProviderError(format!(
            "Gemini rejected the analysis request: {cause}"
        )),
        500..=599 => AnalysisProviderError::ProviderUnavailable(format!(
            "Gemini service error ({status}): {cause}"
        )),
        _ => AnalysisProviderError::ProviderError(format!("Gemini error ({status}): {cause}")),
    }
}

#[cfg(test)]
mod tests {
    use crate::{
        providers::analysis::{AiAnalysisProvider, AnalysisRequest},
        storage::FeedItem,
    };

    use super::*;

    struct MockGeminiAnalysisClient {
        response_text: String,
    }

    fn block_on<F: std::future::Future>(future: F) -> F::Output {
        tauri::async_runtime::block_on(future)
    }

    #[async_trait]
    impl GeminiAnalysisGenerateContentClient for MockGeminiAnalysisClient {
        async fn generate_content(
            &self,
            model: &str,
            api_key: &str,
            request: &GeminiAnalysisGenerateContentRequest,
        ) -> Result<GeminiAnalysisGenerateContentResponse, AnalysisProviderError> {
            assert_eq!(model, DEFAULT_GEMINI_ANALYSIS_MODEL);
            assert_eq!(api_key, "test-api-key");
            assert_eq!(request.contents.len(), 1);
            assert_eq!(request.contents[0].parts.len(), 1);
            match &request.contents[0].parts[0] {
                GeminiAnalysisPart::Text { text } => {
                    assert!(text.contains("Analyze this source item"));
                    assert!(text.contains("Source URL: https://example.com/report"));
                }
                GeminiAnalysisPart::InlineData { .. } => {
                    panic!("analysis request should use a text part")
                }
            }

            Ok(GeminiAnalysisGenerateContentResponse {
                candidates: vec![GeminiAnalysisCandidate {
                    content: GeminiAnalysisResponseContent {
                        parts: vec![GeminiAnalysisResponsePart {
                            text: Some(self.response_text.clone()),
                        }],
                    },
                }],
            })
        }
    }

    #[test]
    fn gemini_reports_native_document_support() {
        let provider = GeminiAnalysisProvider::with_client(
            Some("test-api-key".to_owned()),
            DEFAULT_GEMINI_ANALYSIS_MODEL,
            MockGeminiAnalysisClient {
                response_text: "{}".to_owned(),
            },
        );

        assert_eq!(
            provider.document_support(),
            crate::providers::analysis::DocumentSupport::Native
        );
    }

    #[test]
    fn gemini_document_part_encodes_native_bytes_as_inline_data() {
        let part = gemini_document_part(&crate::providers::analysis::AnalysisDocument::Native {
            mime_type: "application/pdf".to_owned(),
            data: b"PDF".to_vec(),
        });

        match part {
            GeminiAnalysisPart::InlineData { inline_data } => {
                assert_eq!(inline_data.mime_type, "application/pdf");
                assert_eq!(inline_data.data, "UERG");
            }
            GeminiAnalysisPart::Text { .. } => panic!("native document should map to inline data"),
        }
    }

    #[test]
    fn gemini_analysis_provider_requires_configuration() {
        let provider = GeminiAnalysisProvider::with_client(
            None,
            DEFAULT_GEMINI_ANALYSIS_MODEL,
            MockGeminiAnalysisClient {
                response_text: "{}".to_owned(),
            },
        );
        let error = block_on(provider.analyze(&sample_request()))
            .expect_err("unconfigured Gemini should fail");

        assert_eq!(provider.provider_id(), "provider_gemini");
        assert_eq!(error.code(), "provider_not_configured");
    }

    #[test]
    fn gemini_analysis_provider_parses_structured_output() {
        let provider = GeminiAnalysisProvider::with_client(
            Some("test-api-key".to_owned()),
            DEFAULT_GEMINI_ANALYSIS_MODEL,
            MockGeminiAnalysisClient {
                response_text: r#"{"summary":"Revenue improved.","significance":"medium","tags":["earnings","guidance"],"reasoning":"The report cites higher revenue.","language":"en","sourceReferences":[{"sourceUrl":"https://example.com/report","label":"Report"}]}"#.to_owned(),
            },
        );

        let output = block_on(provider.analyze(&sample_request()))
            .expect("mock Gemini response should parse");

        assert_eq!(output.summary, "Revenue improved.");
        assert_eq!(output.significance, "medium");
        assert_eq!(output.tags, vec!["earnings", "guidance"]);
        assert_eq!(output.source_references.len(), 1);
        assert_eq!(
            output.source_references[0].source_url,
            "https://example.com/report"
        );
    }

    #[test]
    fn gemini_analysis_http_error_includes_provider_cause() {
        let error = map_gemini_analysis_http_error(
            400,
            r#"{"error":{"status":"INVALID_ARGUMENT","message":"Request is invalid"}}"#,
        );

        assert_eq!(error.code(), "provider_error");
        assert!(error
            .to_string()
            .contains("INVALID_ARGUMENT: Request is invalid"));
    }

    #[test]
    fn gemini_http_error_maps_status_taxonomy() {
        let body = r#"{"error":{"status":"INTERNAL","message":"boom"}}"#;

        assert_eq!(
            map_gemini_analysis_http_error(500, body).code(),
            "provider_unavailable"
        );
        assert_eq!(
            map_gemini_analysis_http_error(503, body).code(),
            "provider_unavailable"
        );
        assert_eq!(
            map_gemini_analysis_http_error(400, body).code(),
            "provider_error"
        );
        assert_eq!(
            map_gemini_analysis_http_error(401, body).code(),
            "provider_not_configured"
        );
        assert_eq!(
            map_gemini_analysis_http_error(429, body).code(),
            "provider_limit"
        );
    }

    #[test]
    #[ignore = "live Gemini smoke test; requires GEMINI_API_KEY and BRAWLER_GEMINI_ANALYSIS_SMOKE_SOURCE_URL/TITLE/BODY"]
    fn live_gemini_analyzes_feed_item() -> Result<(), String> {
        let api_key = required_env(
            "GEMINI_API_KEY",
            "set GEMINI_API_KEY to run the live Gemini analysis smoke test",
        )?;
        let source_url = required_env(
            "BRAWLER_GEMINI_ANALYSIS_SMOKE_SOURCE_URL",
            "set BRAWLER_GEMINI_ANALYSIS_SMOKE_SOURCE_URL to the source URL for the sample feed item",
        )?;
        let title = required_env(
            "BRAWLER_GEMINI_ANALYSIS_SMOKE_TITLE",
            "set BRAWLER_GEMINI_ANALYSIS_SMOKE_TITLE to the sample feed item title",
        )?;
        let body = required_env(
            "BRAWLER_GEMINI_ANALYSIS_SMOKE_BODY",
            "set BRAWLER_GEMINI_ANALYSIS_SMOKE_BODY to the sample source text",
        )?;
        let model = std::env::var("BRAWLER_GEMINI_ANALYSIS_SMOKE_MODEL")
            .unwrap_or_else(|_| DEFAULT_GEMINI_ANALYSIS_MODEL.to_owned());
        let custom_question = std::env::var("BRAWLER_GEMINI_ANALYSIS_SMOKE_QUESTION").ok();
        let provider = GeminiAnalysisProvider::live(
            Some(api_key),
            model.clone(),
            GEMINI_ANALYSIS_TIMEOUT_SECONDS as i64,
        )
        .map_err(|error| format!("live Gemini analysis provider should initialize: {error}"))?;
        let request = AnalysisRequest {
            feed_item: FeedItem {
                id: "live_smoke_feed_item".to_owned(),
                company: std::env::var("BRAWLER_GEMINI_ANALYSIS_SMOKE_COMPANY")
                    .unwrap_or_else(|_| "Unmatched".to_owned()),
                item_type: std::env::var("BRAWLER_GEMINI_ANALYSIS_SMOKE_TYPE")
                    .unwrap_or_else(|_| "official_report".to_owned()),
                source: std::env::var("BRAWLER_GEMINI_ANALYSIS_SMOKE_SOURCE")
                    .unwrap_or_else(|_| "Manual live smoke".to_owned()),
                time: "2026-06-03T10:00:00Z".to_owned(),
                title,
                unread: false,
                saved: false,
                source_url,
                language: std::env::var("BRAWLER_GEMINI_ANALYSIS_SMOKE_LANGUAGE")
                    .unwrap_or_else(|_| "en".to_owned()),
                published_at: "2026-06-03T10:00:00Z".to_owned(),
                fetched_at: "2026-06-03T10:05:00Z".to_owned(),
                attribution: std::env::var("BRAWLER_GEMINI_ANALYSIS_SMOKE_ATTRIBUTION")
                    .unwrap_or_else(|_| "Manual live smoke".to_owned()),
                summary: std::env::var("BRAWLER_GEMINI_ANALYSIS_SMOKE_SUMMARY").unwrap_or_default(),
                body_text: body,
                attachments: Vec::new(),
            },
            prompt_preset_id: std::env::var("BRAWLER_GEMINI_ANALYSIS_SMOKE_PROMPT_PRESET")
                .unwrap_or_else(|_| "default_summary".to_owned()),
            custom_question,
        };
        let output = block_on(provider.analyze(&request)).map_err(|error| {
            format!(
                "live Gemini analysis failed: model={model}, code={}, error={error}",
                error.code()
            )
        })?;

        assert!(
            !output.summary.trim().is_empty(),
            "live Gemini analysis must return a summary"
        );
        assert!(
            !output.source_references.is_empty(),
            "live Gemini analysis must return at least one source reference"
        );
        eprintln!(
            "Live Gemini analysis smoke passed: provider={}, model={model}, job_status=succeeded, source_references={}",
            provider.provider_id(),
            output.source_references.len()
        );
        Ok(())
    }

    fn sample_request() -> AnalysisRequest {
        AnalysisRequest {
            feed_item: FeedItem {
                id: "feed_1".to_owned(),
                company: "GPW:CDR".to_owned(),
                item_type: "official_report".to_owned(),
                source: "Bankier Company Komunikaty".to_owned(),
                time: "2026-06-03T10:00:00Z".to_owned(),
                title: "Quarterly report".to_owned(),
                unread: true,
                saved: false,
                source_url: "https://example.com/report".to_owned(),
                language: "en".to_owned(),
                published_at: "2026-06-03T10:00:00Z".to_owned(),
                fetched_at: "2026-06-03T10:05:00Z".to_owned(),
                attribution: "Bankier".to_owned(),
                summary: "Summary".to_owned(),
                body_text: "Body".to_owned(),
                attachments: Vec::new(),
            },
            prompt_preset_id: "default_summary".to_owned(),
            custom_question: None,
        }
    }

    fn required_env(name: &str, message: &str) -> Result<String, String> {
        std::env::var(name)
            .map(|value| value.trim().to_owned())
            .ok()
            .filter(|value| !value.is_empty())
            .ok_or_else(|| message.to_owned())
    }
}
