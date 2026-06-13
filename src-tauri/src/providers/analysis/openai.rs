use async_trait::async_trait;
use reqwest::Client;
use serde::{Deserialize, Serialize};
use std::time::Duration;

use crate::providers::common::{
    describe_reqwest_error, effective_timeout_seconds, summarize_provider_error_body,
};

use super::prompts;
use super::types::{
    AiAnalysisProvider, AnalysisProviderError, AnalysisProviderOutput, AnalysisRequest,
    ResearchBriefProviderOutput, ResearchBriefRequest, ResearchDigestRequest,
};

pub const DEFAULT_OPENAI_ANALYSIS_MODEL: &str = "gpt-5.5";
const OPENAI_ANALYSIS_TIMEOUT_SECONDS: u64 = 90;
const OPENAI_ANALYSIS_TIMEOUT_ENV: &str = "BRAWLER_OPENAI_ANALYSIS_TIMEOUT_SECONDS";

#[async_trait]
pub trait OpenAiChatCompletionsClient: Send + Sync {
    async fn create_completion(
        &self,
        model: &str,
        api_key: &str,
        request: &OpenAiChatCompletionsRequest,
    ) -> Result<OpenAiChatCompletionsResponse, AnalysisProviderError>;
}

pub struct ReqwestOpenAiChatCompletionsClient {
    client: Client,
}

impl ReqwestOpenAiChatCompletionsClient {
    pub fn new(configured_timeout_seconds: u64) -> Result<Self, AnalysisProviderError> {
        let timeout_seconds =
            effective_timeout_seconds(OPENAI_ANALYSIS_TIMEOUT_ENV, configured_timeout_seconds);
        let client = Client::builder()
            .timeout(Duration::from_secs(timeout_seconds))
            .build()
            .map_err(|error| AnalysisProviderError::NetworkError(describe_reqwest_error(&error)))?;

        Ok(Self { client })
    }
}

#[async_trait]
impl OpenAiChatCompletionsClient for ReqwestOpenAiChatCompletionsClient {
    async fn create_completion(
        &self,
        _model: &str,
        api_key: &str,
        request: &OpenAiChatCompletionsRequest,
    ) -> Result<OpenAiChatCompletionsResponse, AnalysisProviderError> {
        let response = self
            .client
            .post("https://api.openai.com/v1/chat/completions")
            .bearer_auth(api_key)
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
            return Err(map_openai_http_error(status.as_u16(), &body));
        }

        serde_json::from_str(&body).map_err(|error| {
            AnalysisProviderError::ParseError(format!("OpenAI response JSON: {error}"))
        })
    }
}

pub struct OpenAiAnalysisProvider<C = ReqwestOpenAiChatCompletionsClient> {
    api_key: Option<String>,
    model: String,
    client: C,
}

impl OpenAiAnalysisProvider<ReqwestOpenAiChatCompletionsClient> {
    pub fn live(
        api_key: Option<String>,
        model: impl Into<String>,
        timeout_seconds: i64,
    ) -> Result<Self, AnalysisProviderError> {
        let timeout_seconds = u64::try_from(timeout_seconds)
            .ok()
            .filter(|value| *value > 0)
            .unwrap_or(OPENAI_ANALYSIS_TIMEOUT_SECONDS);
        Ok(Self {
            api_key,
            model: model.into(),
            client: ReqwestOpenAiChatCompletionsClient::new(timeout_seconds)?,
        })
    }
}

impl<C> OpenAiAnalysisProvider<C>
where
    C: OpenAiChatCompletionsClient,
{
    pub fn with_client(api_key: Option<String>, model: impl Into<String>, client: C) -> Self {
        Self {
            api_key,
            model: model.into(),
            client,
        }
    }

    fn api_key(&self) -> Result<&str, AnalysisProviderError> {
        self.api_key
            .as_deref()
            .filter(|value| !value.trim().is_empty())
            .ok_or(AnalysisProviderError::ProviderNotConfigured)
    }

    async fn complete(&self, prompt: String) -> Result<String, AnalysisProviderError> {
        let api_key = self.api_key()?;
        let request = OpenAiChatCompletionsRequest {
            model: self.model.clone(),
            messages: vec![OpenAiMessage {
                role: "user".to_owned(),
                content: prompt,
            }],
            response_format: OpenAiResponseFormat {
                format_type: "json_object".to_owned(),
            },
        };
        let response = self
            .client
            .create_completion(&self.model, api_key, &request)
            .await?;
        extract_openai_text(&response)
    }
}

#[async_trait]
impl<C> AiAnalysisProvider for OpenAiAnalysisProvider<C>
where
    C: OpenAiChatCompletionsClient,
{
    fn provider_id(&self) -> &'static str {
        "provider_openai"
    }

    fn model(&self) -> &str {
        &self.model
    }

    async fn analyze(
        &self,
        request: &AnalysisRequest,
    ) -> Result<AnalysisProviderOutput, AnalysisProviderError> {
        let text = self.complete(prompts::analysis_prompt(request)).await?;
        prompts::parse_analysis_output(&text, "OpenAI")
    }

    async fn generate_research_brief(
        &self,
        request: &ResearchBriefRequest,
    ) -> Result<ResearchBriefProviderOutput, AnalysisProviderError> {
        let text = self
            .complete(prompts::research_brief_prompt(request))
            .await?;
        prompts::parse_research_brief_output(&text, "OpenAI")
    }

    async fn generate_research_digest(
        &self,
        request: &ResearchDigestRequest,
    ) -> Result<ResearchBriefProviderOutput, AnalysisProviderError> {
        let text = self
            .complete(prompts::research_digest_prompt(request))
            .await?;
        prompts::parse_research_brief_output(&text, "OpenAI")
    }
}

#[derive(Debug, Serialize)]
pub struct OpenAiChatCompletionsRequest {
    model: String,
    messages: Vec<OpenAiMessage>,
    response_format: OpenAiResponseFormat,
}

#[derive(Debug, Serialize)]
pub struct OpenAiMessage {
    role: String,
    content: String,
}

#[derive(Debug, Serialize)]
pub struct OpenAiResponseFormat {
    #[serde(rename = "type")]
    format_type: String,
}

#[derive(Debug, Deserialize)]
pub struct OpenAiChatCompletionsResponse {
    #[serde(default)]
    choices: Vec<OpenAiChoice>,
}

#[derive(Debug, Deserialize)]
pub struct OpenAiChoice {
    message: OpenAiResponseMessage,
}

#[derive(Debug, Deserialize)]
pub struct OpenAiResponseMessage {
    #[serde(default)]
    content: Option<String>,
}

fn extract_openai_text(
    response: &OpenAiChatCompletionsResponse,
) -> Result<String, AnalysisProviderError> {
    response
        .choices
        .first()
        .and_then(|choice| choice.message.content.clone())
        .map(|text| text.trim().to_owned())
        .filter(|text| !text.is_empty())
        .ok_or_else(|| {
            AnalysisProviderError::ParseError("OpenAI response did not include text".to_owned())
        })
}

fn map_openai_http_error(status: u16, body: &str) -> AnalysisProviderError {
    let cause = summarize_provider_error_body(body);
    match status {
        401 | 403 => AnalysisProviderError::ProviderNotConfigured,
        429 => AnalysisProviderError::ProviderLimit,
        503 => AnalysisProviderError::ProviderUnavailable(format!(
            "OpenAI service unavailable: {cause}"
        )),
        400 => AnalysisProviderError::ProviderError(format!(
            "OpenAI rejected the analysis request: {cause}"
        )),
        500..=599 => AnalysisProviderError::ProviderError(format!(
            "OpenAI service error ({status}): {cause}"
        )),
        _ => AnalysisProviderError::ProviderError(format!("OpenAI error ({status}): {cause}")),
    }
}

#[cfg(test)]
mod tests {
    use crate::providers::analysis::{AiAnalysisProvider, AnalysisRequest};
    use crate::storage::FeedItem;

    use super::*;

    struct MockOpenAiClient {
        response_text: String,
    }

    fn block_on<F: std::future::Future>(future: F) -> F::Output {
        tauri::async_runtime::block_on(future)
    }

    #[async_trait]
    impl OpenAiChatCompletionsClient for MockOpenAiClient {
        async fn create_completion(
            &self,
            model: &str,
            api_key: &str,
            request: &OpenAiChatCompletionsRequest,
        ) -> Result<OpenAiChatCompletionsResponse, AnalysisProviderError> {
            assert_eq!(model, DEFAULT_OPENAI_ANALYSIS_MODEL);
            assert_eq!(api_key, "test-api-key");
            assert_eq!(request.response_format.format_type, "json_object");
            assert_eq!(request.messages.len(), 1);
            assert!(request.messages[0]
                .content
                .contains("Analyze this source item"));

            Ok(OpenAiChatCompletionsResponse {
                choices: vec![OpenAiChoice {
                    message: OpenAiResponseMessage {
                        content: Some(self.response_text.clone()),
                    },
                }],
            })
        }
    }

    fn sample_request() -> AnalysisRequest {
        AnalysisRequest {
            feed_item: FeedItem {
                id: "feed_1".to_owned(),
                company: "GPW:CDR".to_owned(),
                item_type: "official_report".to_owned(),
                source: "Bankier".to_owned(),
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

    #[test]
    fn openai_provider_requires_configuration() {
        let provider = OpenAiAnalysisProvider::with_client(
            None,
            DEFAULT_OPENAI_ANALYSIS_MODEL,
            MockOpenAiClient {
                response_text: "{}".to_owned(),
            },
        );
        let error =
            block_on(provider.analyze(&sample_request())).expect_err("unconfigured OpenAI fails");

        assert_eq!(provider.provider_id(), "provider_openai");
        assert_eq!(error.code(), "provider_not_configured");
    }

    #[test]
    fn openai_provider_parses_structured_output() {
        let provider = OpenAiAnalysisProvider::with_client(
            Some("test-api-key".to_owned()),
            DEFAULT_OPENAI_ANALYSIS_MODEL,
            MockOpenAiClient {
                response_text: r#"{"summary":"Revenue improved.","significance":"high","tags":["earnings"],"reasoning":"The report cites higher revenue.","language":"en","sourceReferences":[{"sourceUrl":"https://example.com/report","label":"Report"}]}"#.to_owned(),
            },
        );

        let output =
            block_on(provider.analyze(&sample_request())).expect("mock OpenAI response parses");

        assert_eq!(output.summary, "Revenue improved.");
        assert_eq!(output.significance, "high");
        assert_eq!(output.source_references.len(), 1);
    }

    #[test]
    fn openai_http_error_maps_rate_limit() {
        let error = map_openai_http_error(
            429,
            r#"{"error":{"message":"Rate limit reached","type":"rate_limit_error"}}"#,
        );

        assert_eq!(error.code(), "provider_limit");
    }
}
