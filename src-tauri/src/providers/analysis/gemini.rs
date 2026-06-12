use reqwest::blocking::Client;
use serde::{Deserialize, Serialize};
use std::time::Duration;

use crate::providers::common::{
    clean_optional_string, describe_reqwest_error, effective_timeout_seconds, extract_json_object,
    summarize_provider_error_body,
};

use super::types::{
    AiAnalysisProvider, AnalysisProviderError, AnalysisProviderOutput, AnalysisRequest,
    AnalysisSourceReference, ResearchBriefCitationOutput, ResearchBriefProviderOutput,
    ResearchBriefRequest, ResearchBriefSectionOutput, ResearchDigestRequest,
};

pub const DEFAULT_GEMINI_ANALYSIS_MODEL: &str = "gemini-2.5-flash";
const GEMINI_ANALYSIS_TIMEOUT_SECONDS: u64 = 90;
const GEMINI_ANALYSIS_TIMEOUT_ENV: &str = "BRAWLER_GEMINI_ANALYSIS_TIMEOUT_SECONDS";

pub trait GeminiAnalysisGenerateContentClient {
    fn generate_content(
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

impl GeminiAnalysisGenerateContentClient for ReqwestGeminiAnalysisGenerateContentClient {
    fn generate_content(
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
            .map_err(|error| AnalysisProviderError::NetworkError(describe_reqwest_error(&error)))?;
        let status = response.status();
        let body = response
            .text()
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

    fn analyze(
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
            .generate_content(&self.model, api_key, &gemini_request)?;
        let output_text = extract_gemini_analysis_text(&response)?;
        parse_gemini_analysis_output(&output_text)
    }

    fn generate_research_brief(
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
            .generate_content(&self.model, api_key, &gemini_request)?;
        let output_text = extract_gemini_analysis_text(&response)?;
        parse_gemini_research_brief_output(&output_text)
    }

    fn generate_research_digest(
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
            .generate_content(&self.model, api_key, &gemini_request)?;
        let output_text = extract_gemini_analysis_text(&response)?;
        parse_gemini_research_brief_output(&output_text)
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
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct GeminiAnalysisGenerationConfig {
    response_mime_type: String,
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

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
struct GeminiAnalysisJson {
    summary: String,
    significance: String,
    reasoning: String,
    language: Option<String>,
    #[serde(default)]
    tags: Vec<String>,
    #[serde(default)]
    source_references: Vec<GeminiAnalysisSourceReferenceJson>,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
struct GeminiAnalysisSourceReferenceJson {
    source_url: String,
    label: Option<String>,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
struct GeminiResearchBriefJson {
    title: String,
    summary: String,
    #[serde(default)]
    sections: Vec<GeminiResearchBriefSectionJson>,
    language: Option<String>,
    #[serde(default)]
    citations: Vec<GeminiResearchBriefCitationJson>,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
struct GeminiResearchBriefSectionJson {
    heading: String,
    body: String,
    #[serde(default)]
    citation_keys: Vec<String>,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
struct GeminiResearchBriefCitationJson {
    citation_key: String,
    evidence_type: String,
    evidence_id: String,
    label: String,
    snippet: Option<String>,
}

fn gemini_analysis_request(request: &AnalysisRequest) -> GeminiAnalysisGenerateContentRequest {
    let item = &request.feed_item;
    let custom_question = request
        .custom_question
        .as_deref()
        .map(str::trim)
        .filter(|value| !value.is_empty())
        .unwrap_or("Summarize the selected source item for investor research.");
    let source_text = if item.body_text.trim().is_empty() {
        item.summary.as_str()
    } else {
        item.body_text.as_str()
    };
    let prompt = format!(
        "Analyze this source item for an investor research workflow. \
Return only JSON with this exact shape: \
{{\"summary\":\"...\",\"significance\":\"low|medium|high|unknown\",\"tags\":[\"...\"],\"reasoning\":\"...\",\"language\":\"en\",\"sourceReferences\":[{{\"sourceUrl\":\"...\",\"label\":\"...\"}}]}}. \
Use only the provided source context. Cite the source URL. Do not include markdown fences, commentary, buy/sell/hold recommendations, portfolio allocation advice, or personalized investment advice.\n\n\
Prompt preset: {prompt_preset}\n\
User question: {custom_question}\n\
Company: {company}\n\
Item type: {item_type}\n\
Title: {title}\n\
Summary: {summary}\n\
Source body: {source_text}\n\
Source: {source}\n\
Attribution: {attribution}\n\
Source URL: {source_url}\n\
Language: {language}",
        prompt_preset = request.prompt_preset_id,
        company = item.company,
        item_type = item.item_type,
        title = item.title,
        summary = item.summary,
        source = item.source,
        attribution = item.attribution,
        source_url = item.source_url,
        language = item.language,
    );

    GeminiAnalysisGenerateContentRequest {
        contents: vec![GeminiAnalysisContent {
            parts: vec![GeminiAnalysisPart::Text { text: prompt }],
        }],
        generation_config: GeminiAnalysisGenerationConfig {
            response_mime_type: "application/json".to_owned(),
        },
    }
}

fn gemini_research_brief_request(
    request: &ResearchBriefRequest,
) -> GeminiAnalysisGenerateContentRequest {
    let evidence = request
        .evidence_items
        .iter()
        .take(120)
        .enumerate()
        .map(|(index, item)| {
            format!(
                "E{index}: evidenceType={evidence_type}; evidenceId={evidence_id}; companyId={company_id}; occurredAt={occurred_at}; title={title}; summary={summary}; sourceUrl={source_url}; attribution={attribution}; trust={trust}",
                index = index + 1,
                evidence_type = item.evidence_type,
                evidence_id = item.source_id,
                company_id = item.company_id,
                occurred_at = item.occurred_at,
                title = item.title,
                summary = item.summary.as_deref().unwrap_or(""),
                source_url = item.source_url.as_deref().unwrap_or(""),
                attribution = item.attribution.as_deref().unwrap_or(""),
                trust = item.trust_category,
            )
        })
        .collect::<Vec<_>>()
        .join("\n");
    let prompt = format!(
        "Create a source-grounded investor research brief for the selected {scope_type}. \
Return only JSON with this exact shape: \
{{\"title\":\"...\",\"summary\":\"...\",\"sections\":[{{\"heading\":\"...\",\"body\":\"...\",\"citationKeys\":[\"E1\"]}}],\"language\":\"en\",\"citations\":[{{\"citationKey\":\"E1\",\"evidenceType\":\"feed_item\",\"evidenceId\":\"feed_01\",\"label\":\"...\",\"snippet\":\"...\"}}]}}. \
Use only the evidence below. Every material claim in every section must cite one or more evidence keys. \
Do not include markdown fences, commentary outside JSON, buy/sell/hold recommendations, price targets, portfolio allocation advice, or personalized investment advice.\n\n\
Scope type: {scope_type}\n\
Scope id: {scope_id}\n\
Evidence:\n{evidence}",
        scope_type = request.scope_type,
        scope_id = request.scope_id,
    );

    GeminiAnalysisGenerateContentRequest {
        contents: vec![GeminiAnalysisContent {
            parts: vec![GeminiAnalysisPart::Text { text: prompt }],
        }],
        generation_config: GeminiAnalysisGenerationConfig {
            response_mime_type: "application/json".to_owned(),
        },
    }
}

fn gemini_research_digest_request(
    request: &ResearchDigestRequest,
) -> GeminiAnalysisGenerateContentRequest {
    let evidence = request
        .evidence_items
        .iter()
        .take(140)
        .enumerate()
        .map(|(index, item)| {
            format!(
                "E{index}: evidenceType={evidence_type}; evidenceId={evidence_id}; companyId={company_id}; occurredAt={occurred_at}; title={title}; summary={summary}; sourceUrl={source_url}; attribution={attribution}; trust={trust}",
                index = index + 1,
                evidence_type = item.evidence_type,
                evidence_id = item.source_id,
                company_id = item.company_id,
                occurred_at = item.occurred_at,
                title = item.title,
                summary = item.summary.as_deref().unwrap_or(""),
                source_url = item.source_url.as_deref().unwrap_or(""),
                attribution = item.attribution.as_deref().unwrap_or(""),
                trust = item.trust_category,
            )
        })
        .collect::<Vec<_>>()
        .join("\n");
    let prompt = format!(
        "Create a source-grounded research digest for the selected {scope_type}. \
Focus on changed evidence, open reminders, upcoming review work, and unresolved questions. \
Return only JSON with this exact shape: \
{{\"title\":\"...\",\"summary\":\"...\",\"sections\":[{{\"heading\":\"...\",\"body\":\"...\",\"citationKeys\":[\"E1\"]}}],\"language\":\"en\",\"citations\":[{{\"citationKey\":\"E1\",\"evidenceType\":\"feed_item\",\"evidenceId\":\"feed_01\",\"label\":\"...\",\"snippet\":\"...\"}}]}}. \
Use only the evidence below. Every material claim in every section must cite one or more evidence keys. \
Do not include markdown fences, commentary outside JSON, buy/sell/hold recommendations, price targets, portfolio allocation advice, or personalized investment advice.\n\n\
Scope type: {scope_type}\n\
Scope id: {scope_id}\n\
Evidence:\n{evidence}",
        scope_type = request.scope_type,
        scope_id = request.scope_id,
    );

    GeminiAnalysisGenerateContentRequest {
        contents: vec![GeminiAnalysisContent {
            parts: vec![GeminiAnalysisPart::Text { text: prompt }],
        }],
        generation_config: GeminiAnalysisGenerationConfig {
            response_mime_type: "application/json".to_owned(),
        },
    }
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

fn parse_gemini_analysis_output(
    text: &str,
) -> Result<AnalysisProviderOutput, AnalysisProviderError> {
    let json_text =
        extract_json_object(text, "Gemini response").map_err(AnalysisProviderError::ParseError)?;
    let parsed: GeminiAnalysisJson = serde_json::from_str(json_text).map_err(|error| {
        AnalysisProviderError::ParseError(format!("Gemini analysis JSON: {error}"))
    })?;
    let significance = parsed.significance.trim().to_lowercase();

    if !["low", "medium", "high", "unknown"].contains(&significance.as_str()) {
        return Err(AnalysisProviderError::ParseError(format!(
            "Gemini analysis significance is not supported: {}",
            parsed.significance
        )));
    }

    let summary = parsed.summary.trim().to_owned();
    let reasoning = parsed.reasoning.trim().to_owned();
    if summary.is_empty() || reasoning.is_empty() {
        return Err(AnalysisProviderError::ParseError(
            "Gemini analysis did not include usable summary and reasoning".to_owned(),
        ));
    }

    let source_references = parsed
        .source_references
        .into_iter()
        .filter_map(|reference| {
            let source_url = reference.source_url.trim().to_owned();
            if source_url.is_empty() {
                None
            } else {
                Some(AnalysisSourceReference {
                    source_url,
                    label: clean_optional_string(reference.label),
                })
            }
        })
        .collect::<Vec<_>>();

    if source_references.is_empty() {
        return Err(AnalysisProviderError::ParseError(
            "Gemini analysis did not include source references".to_owned(),
        ));
    }

    Ok(AnalysisProviderOutput {
        summary,
        significance,
        reasoning,
        language: clean_optional_string(parsed.language),
        tags: parsed
            .tags
            .into_iter()
            .map(|tag| tag.trim().to_owned())
            .filter(|tag| !tag.is_empty())
            .collect(),
        source_references,
    })
}

fn parse_gemini_research_brief_output(
    text: &str,
) -> Result<ResearchBriefProviderOutput, AnalysisProviderError> {
    let json_text =
        extract_json_object(text, "Gemini response").map_err(AnalysisProviderError::ParseError)?;
    let parsed: GeminiResearchBriefJson = serde_json::from_str(json_text).map_err(|error| {
        AnalysisProviderError::ParseError(format!("Gemini research brief JSON: {error}"))
    })?;

    let title = parsed.title.trim().to_owned();
    let summary = parsed.summary.trim().to_owned();
    if title.is_empty() || summary.is_empty() {
        return Err(AnalysisProviderError::ParseError(
            "Gemini research brief did not include usable title and summary".to_owned(),
        ));
    }

    let sections = parsed
        .sections
        .into_iter()
        .filter_map(|section| {
            let heading = section.heading.trim().to_owned();
            let body = section.body.trim().to_owned();
            let citation_keys = section
                .citation_keys
                .into_iter()
                .map(|value| value.trim().to_owned())
                .filter(|value| !value.is_empty())
                .collect::<Vec<_>>();
            if heading.is_empty() || body.is_empty() {
                None
            } else {
                Some(ResearchBriefSectionOutput {
                    heading,
                    body,
                    citation_keys,
                })
            }
        })
        .collect::<Vec<_>>();

    let citations = parsed
        .citations
        .into_iter()
        .filter_map(|citation| {
            let citation_key = citation.citation_key.trim().to_owned();
            let evidence_type = citation.evidence_type.trim().to_owned();
            let evidence_id = citation.evidence_id.trim().to_owned();
            let label = citation.label.trim().to_owned();
            if citation_key.is_empty()
                || evidence_type.is_empty()
                || evidence_id.is_empty()
                || label.is_empty()
            {
                None
            } else {
                Some(ResearchBriefCitationOutput {
                    citation_key,
                    evidence_type,
                    evidence_id,
                    label,
                    snippet: clean_optional_string(citation.snippet),
                })
            }
        })
        .collect::<Vec<_>>();

    if sections.is_empty() || citations.is_empty() {
        return Err(AnalysisProviderError::ParseError(
            "Gemini research brief did not include sections and citations".to_owned(),
        ));
    }

    Ok(ResearchBriefProviderOutput {
        title,
        summary,
        sections,
        language: clean_optional_string(parsed.language),
        citations,
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
        500..=599 => AnalysisProviderError::ProviderError(format!(
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

    impl GeminiAnalysisGenerateContentClient for MockGeminiAnalysisClient {
        fn generate_content(
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
    fn gemini_analysis_provider_requires_configuration() {
        let provider = GeminiAnalysisProvider::with_client(
            None,
            DEFAULT_GEMINI_ANALYSIS_MODEL,
            MockGeminiAnalysisClient {
                response_text: "{}".to_owned(),
            },
        );
        let error = provider
            .analyze(&sample_request())
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

        let output = provider
            .analyze(&sample_request())
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
    fn gemini_analysis_rejects_missing_source_references() {
        let error = parse_gemini_analysis_output(
            r#"{"summary":"Revenue improved.","significance":"medium","tags":[],"reasoning":"The report cites higher revenue.","language":"en","sourceReferences":[]}"#,
        )
        .expect_err("source references are required");

        assert_eq!(error.code(), "parse_error");
    }

    #[test]
    fn gemini_research_brief_parser_ignores_trailing_text_after_json() {
        let output = parse_gemini_research_brief_output(
            r#"
            {"title":"Research brief","summary":"Summary with {brace} text.","sections":[{"heading":"What changed","body":"Body cites evidence.","citationKeys":["E1"]}],"language":"en","citations":[{"citationKey":"E1","evidenceType":"feed_item","evidenceId":"feed_01","label":"Report","snippet":"Snippet"}]}

            I used the provided evidence only.
            "#,
        )
        .expect("trailing text after the first complete JSON object should be ignored");

        assert_eq!(output.title, "Research brief");
        assert_eq!(output.summary, "Summary with {brace} text.");
        assert_eq!(output.citations.len(), 1);
    }

    #[test]
    fn gemini_research_brief_parser_handles_fenced_json_with_trailing_text() {
        let output = parse_gemini_research_brief_output(
            r#"
            ```json
            {"title":"Research brief","summary":"Summary.","sections":[{"heading":"What changed","body":"Body cites evidence.","citationKeys":["E1"]}],"language":"en","citations":[{"citationKey":"E1","evidenceType":"feed_item","evidenceId":"feed_01","label":"Report","snippet":"Snippet"}]}
            ```

            Done.
            "#,
        )
        .expect("fenced JSON should parse even when the provider appends text");

        assert_eq!(output.title, "Research brief");
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
        let output = provider.analyze(&request).map_err(|error| {
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
