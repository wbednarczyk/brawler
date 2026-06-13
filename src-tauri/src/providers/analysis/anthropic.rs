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

pub const DEFAULT_ANTHROPIC_ANALYSIS_MODEL: &str = "claude-sonnet-4-6";
const ANTHROPIC_ANALYSIS_TIMEOUT_SECONDS: u64 = 90;
const ANTHROPIC_ANALYSIS_TIMEOUT_ENV: &str = "BRAWLER_ANTHROPIC_ANALYSIS_TIMEOUT_SECONDS";
const ANTHROPIC_VERSION: &str = "2023-06-01";
const ANTHROPIC_MAX_OUTPUT_TOKENS: u32 = 8192;

#[async_trait]
pub trait AnthropicMessagesClient: Send + Sync {
    async fn create_message(
        &self,
        model: &str,
        api_key: &str,
        request: &AnthropicMessagesRequest,
    ) -> Result<AnthropicMessagesResponse, AnalysisProviderError>;
}

pub struct ReqwestAnthropicMessagesClient {
    client: Client,
}

impl ReqwestAnthropicMessagesClient {
    pub fn new(configured_timeout_seconds: u64) -> Result<Self, AnalysisProviderError> {
        let timeout_seconds =
            effective_timeout_seconds(ANTHROPIC_ANALYSIS_TIMEOUT_ENV, configured_timeout_seconds);
        let client = Client::builder()
            .timeout(Duration::from_secs(timeout_seconds))
            .build()
            .map_err(|error| AnalysisProviderError::NetworkError(describe_reqwest_error(&error)))?;

        Ok(Self { client })
    }
}

#[async_trait]
impl AnthropicMessagesClient for ReqwestAnthropicMessagesClient {
    async fn create_message(
        &self,
        _model: &str,
        api_key: &str,
        request: &AnthropicMessagesRequest,
    ) -> Result<AnthropicMessagesResponse, AnalysisProviderError> {
        let response = self
            .client
            .post("https://api.anthropic.com/v1/messages")
            .header("x-api-key", api_key)
            .header("anthropic-version", ANTHROPIC_VERSION)
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
            return Err(map_anthropic_http_error(status.as_u16(), &body));
        }

        serde_json::from_str(&body).map_err(|error| {
            AnalysisProviderError::ParseError(format!("Anthropic response JSON: {error}"))
        })
    }
}

pub struct ClaudeAnalysisProvider<C = ReqwestAnthropicMessagesClient> {
    api_key: Option<String>,
    model: String,
    client: C,
}

impl ClaudeAnalysisProvider<ReqwestAnthropicMessagesClient> {
    pub fn live(
        api_key: Option<String>,
        model: impl Into<String>,
        timeout_seconds: i64,
    ) -> Result<Self, AnalysisProviderError> {
        let timeout_seconds = u64::try_from(timeout_seconds)
            .ok()
            .filter(|value| *value > 0)
            .unwrap_or(ANTHROPIC_ANALYSIS_TIMEOUT_SECONDS);
        Ok(Self {
            api_key,
            model: model.into(),
            client: ReqwestAnthropicMessagesClient::new(timeout_seconds)?,
        })
    }
}

impl<C> ClaudeAnalysisProvider<C>
where
    C: AnthropicMessagesClient,
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
        let request = AnthropicMessagesRequest {
            model: self.model.clone(),
            max_tokens: ANTHROPIC_MAX_OUTPUT_TOKENS,
            messages: vec![AnthropicMessage {
                role: "user".to_owned(),
                content: prompt,
            }],
        };
        let response = self
            .client
            .create_message(&self.model, api_key, &request)
            .await?;
        extract_anthropic_text(&response)
    }
}

#[async_trait]
impl<C> AiAnalysisProvider for ClaudeAnalysisProvider<C>
where
    C: AnthropicMessagesClient,
{
    fn provider_id(&self) -> &'static str {
        "provider_anthropic"
    }

    fn model(&self) -> &str {
        &self.model
    }

    async fn analyze(
        &self,
        request: &AnalysisRequest,
    ) -> Result<AnalysisProviderOutput, AnalysisProviderError> {
        let text = self.complete(prompts::analysis_prompt(request)).await?;
        prompts::parse_analysis_output(&text, "Claude")
    }

    async fn generate_research_brief(
        &self,
        request: &ResearchBriefRequest,
    ) -> Result<ResearchBriefProviderOutput, AnalysisProviderError> {
        let text = self
            .complete(prompts::research_brief_prompt(request))
            .await?;
        prompts::parse_research_brief_output(&text, "Claude")
    }

    async fn generate_research_digest(
        &self,
        request: &ResearchDigestRequest,
    ) -> Result<ResearchBriefProviderOutput, AnalysisProviderError> {
        let text = self
            .complete(prompts::research_digest_prompt(request))
            .await?;
        prompts::parse_research_brief_output(&text, "Claude")
    }
}

#[derive(Debug, Serialize)]
pub struct AnthropicMessagesRequest {
    model: String,
    max_tokens: u32,
    messages: Vec<AnthropicMessage>,
}

#[derive(Debug, Serialize)]
pub struct AnthropicMessage {
    role: String,
    content: String,
}

#[derive(Debug, Deserialize)]
pub struct AnthropicMessagesResponse {
    #[serde(default)]
    content: Vec<AnthropicContentBlock>,
}

#[derive(Debug, Deserialize)]
pub struct AnthropicContentBlock {
    #[serde(rename = "type")]
    block_type: String,
    #[serde(default)]
    text: Option<String>,
}

fn extract_anthropic_text(
    response: &AnthropicMessagesResponse,
) -> Result<String, AnalysisProviderError> {
    response
        .content
        .iter()
        .filter(|block| block.block_type == "text")
        .find_map(|block| block.text.clone())
        .map(|text| text.trim().to_owned())
        .filter(|text| !text.is_empty())
        .ok_or_else(|| {
            AnalysisProviderError::ParseError("Anthropic response did not include text".to_owned())
        })
}

fn map_anthropic_http_error(status: u16, body: &str) -> AnalysisProviderError {
    let cause = summarize_provider_error_body(body);
    match status {
        401 | 403 => AnalysisProviderError::ProviderNotConfigured,
        429 => AnalysisProviderError::ProviderLimit,
        529 => AnalysisProviderError::ProviderUnavailable(format!(
            "Anthropic service overloaded: {cause}"
        )),
        503 => AnalysisProviderError::ProviderUnavailable(format!(
            "Anthropic service unavailable: {cause}"
        )),
        400 => AnalysisProviderError::ProviderError(format!(
            "Anthropic rejected the analysis request: {cause}"
        )),
        500..=599 => AnalysisProviderError::ProviderError(format!(
            "Anthropic service error ({status}): {cause}"
        )),
        _ => AnalysisProviderError::ProviderError(format!("Anthropic error ({status}): {cause}")),
    }
}

#[cfg(test)]
mod tests {
    use crate::providers::analysis::{AiAnalysisProvider, AnalysisRequest};
    use crate::storage::FeedItem;

    use super::*;

    struct MockAnthropicClient {
        response_text: String,
    }

    fn block_on<F: std::future::Future>(future: F) -> F::Output {
        tauri::async_runtime::block_on(future)
    }

    #[async_trait]
    impl AnthropicMessagesClient for MockAnthropicClient {
        async fn create_message(
            &self,
            model: &str,
            api_key: &str,
            request: &AnthropicMessagesRequest,
        ) -> Result<AnthropicMessagesResponse, AnalysisProviderError> {
            assert_eq!(model, DEFAULT_ANTHROPIC_ANALYSIS_MODEL);
            assert_eq!(api_key, "test-api-key");
            assert_eq!(request.messages.len(), 1);
            assert_eq!(request.messages[0].role, "user");
            assert!(request.messages[0]
                .content
                .contains("Analyze this source item"));

            Ok(AnthropicMessagesResponse {
                content: vec![AnthropicContentBlock {
                    block_type: "text".to_owned(),
                    text: Some(self.response_text.clone()),
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
    fn claude_provider_requires_configuration() {
        let provider = ClaudeAnalysisProvider::with_client(
            None,
            DEFAULT_ANTHROPIC_ANALYSIS_MODEL,
            MockAnthropicClient {
                response_text: "{}".to_owned(),
            },
        );
        let error =
            block_on(provider.analyze(&sample_request())).expect_err("unconfigured Claude fails");

        assert_eq!(provider.provider_id(), "provider_anthropic");
        assert_eq!(error.code(), "provider_not_configured");
    }

    #[test]
    fn claude_provider_parses_structured_output() {
        let provider = ClaudeAnalysisProvider::with_client(
            Some("test-api-key".to_owned()),
            DEFAULT_ANTHROPIC_ANALYSIS_MODEL,
            MockAnthropicClient {
                response_text: r#"{"summary":"Revenue improved.","significance":"medium","tags":["earnings"],"reasoning":"The report cites higher revenue.","language":"en","sourceReferences":[{"sourceUrl":"https://example.com/report","label":"Report"}]}"#.to_owned(),
            },
        );

        let output =
            block_on(provider.analyze(&sample_request())).expect("mock Claude response parses");

        assert_eq!(output.summary, "Revenue improved.");
        assert_eq!(output.significance, "medium");
        assert_eq!(output.source_references.len(), 1);
    }

    #[test]
    fn anthropic_http_error_maps_overloaded_to_unavailable() {
        let error = map_anthropic_http_error(
            529,
            r#"{"type":"error","error":{"type":"overloaded_error","message":"Overloaded"}}"#,
        );

        assert_eq!(error.code(), "provider_unavailable");
        assert!(error.to_string().contains("Overloaded"));
    }
}
