//! Mistral analysis provider (ADR 0077 §4, tier-4) — OCR + chat.
//!
//! The G0 spike (ADR 0077 Evidence) chose Mistral OCR as the tier-4 substrate:
//! a document is OCR'd to markdown, then a confirmed per-company profile parses
//! it deterministically (`fundamentals::extraction::ocr`). This provider owns
//! the two Mistral endpoints it needs:
//!
//! - **OCR** `POST /v1/ocr` (`mistral-ocr-latest`): the document as a base64
//!   data-URL. OCR v4 returns each page's tables **separately** in
//!   `pages[].tables`; [`ocr_document`](MistralAnalysisProvider::ocr_document)
//!   stitches them back into the page markdown (unstitched output scores 0 —
//!   spike), returning the full-document markdown the parser consumes.
//! - **Chat** `POST /v1/chat/completions` (`mistral-small-latest` by default),
//!   OpenAI-compatible shape, for the analysis paths and the T4.3 profile
//!   bootstrap.
//!
//! Error mapping mirrors the shared taxonomy (401/403 → not-configured, 429 →
//! limit, 5xx → unavailable, 400 → error). No retry lives here — the queue's
//! capped backoff and the provider pool's cooldown own that.

use async_trait::async_trait;
use base64::{engine::general_purpose::STANDARD, Engine as _};
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

/// Default user-selectable chat model. OCR uses a fixed provider-internal model.
pub const DEFAULT_MISTRAL_ANALYSIS_MODEL: &str = "mistral-small-latest";
/// The OCR model is provider-internal (not a user-selectable analysis model).
pub const MISTRAL_OCR_MODEL: &str = "mistral-ocr-latest";
const MISTRAL_ANALYSIS_TIMEOUT_SECONDS: u64 = 90;
const MISTRAL_ANALYSIS_TIMEOUT_ENV: &str = "BRAWLER_MISTRAL_ANALYSIS_TIMEOUT_SECONDS";
const MISTRAL_OCR_URL: &str = "https://api.mistral.ai/v1/ocr";
const MISTRAL_CHAT_URL: &str = "https://api.mistral.ai/v1/chat/completions";

/// Transport seam over the two Mistral endpoints, generic so tests can inject a
/// mock without a live HTTP client.
#[async_trait]
pub trait MistralClient: Send + Sync {
    async fn ocr(
        &self,
        api_key: &str,
        request: &MistralOcrRequest,
    ) -> Result<MistralOcrResponse, AnalysisProviderError>;

    async fn create_completion(
        &self,
        model: &str,
        api_key: &str,
        request: &MistralChatRequest,
    ) -> Result<MistralChatResponse, AnalysisProviderError>;
}

pub struct ReqwestMistralClient {
    client: Client,
}

impl ReqwestMistralClient {
    pub fn new(configured_timeout_seconds: u64) -> Result<Self, AnalysisProviderError> {
        let timeout_seconds =
            effective_timeout_seconds(MISTRAL_ANALYSIS_TIMEOUT_ENV, configured_timeout_seconds);
        let client = Client::builder()
            .timeout(Duration::from_secs(timeout_seconds))
            .build()
            .map_err(|error| AnalysisProviderError::NetworkError(describe_reqwest_error(&error)))?;
        Ok(Self { client })
    }
}

#[async_trait]
impl MistralClient for ReqwestMistralClient {
    async fn ocr(
        &self,
        api_key: &str,
        request: &MistralOcrRequest,
    ) -> Result<MistralOcrResponse, AnalysisProviderError> {
        let response = self
            .client
            .post(MISTRAL_OCR_URL)
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
            return Err(map_mistral_http_error(status.as_u16(), &body));
        }
        serde_json::from_str(&body).map_err(|error| {
            AnalysisProviderError::ParseError(format!("Mistral OCR response JSON: {error}"))
        })
    }

    async fn create_completion(
        &self,
        _model: &str,
        api_key: &str,
        request: &MistralChatRequest,
    ) -> Result<MistralChatResponse, AnalysisProviderError> {
        let response = self
            .client
            .post(MISTRAL_CHAT_URL)
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
            return Err(map_mistral_http_error(status.as_u16(), &body));
        }
        serde_json::from_str(&body).map_err(|error| {
            AnalysisProviderError::ParseError(format!("Mistral chat response JSON: {error}"))
        })
    }
}

pub struct MistralAnalysisProvider<C = ReqwestMistralClient> {
    api_key: Option<String>,
    model: String,
    client: C,
}

impl MistralAnalysisProvider<ReqwestMistralClient> {
    pub fn live(
        api_key: Option<String>,
        model: impl Into<String>,
        timeout_seconds: i64,
    ) -> Result<Self, AnalysisProviderError> {
        let timeout_seconds = u64::try_from(timeout_seconds)
            .ok()
            .filter(|value| *value > 0)
            .unwrap_or(MISTRAL_ANALYSIS_TIMEOUT_SECONDS);
        Ok(Self {
            api_key,
            model: model.into(),
            client: ReqwestMistralClient::new(timeout_seconds)?,
        })
    }
}

impl<C> MistralAnalysisProvider<C>
where
    C: MistralClient,
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

    /// OCRs a document to its full-document markdown, stitching each page's
    /// separately-returned tables back in (spike: unstitched output scores 0).
    /// The deterministic tier-4 parser consumes this markdown (T4.3). The
    /// public entry point is the [`AiAnalysisProvider::ocr_document`] trait
    /// method (so the pool/gate wrappers forward it); this is its implementation.
    async fn ocr_to_markdown(
        &self,
        bytes: &[u8],
        mime_type: &str,
    ) -> Result<String, AnalysisProviderError> {
        let api_key = self.api_key()?;
        let data_url = format!("data:{};base64,{}", mime_type, STANDARD.encode(bytes));
        let request = MistralOcrRequest {
            model: MISTRAL_OCR_MODEL.to_owned(),
            document: MistralOcrDocument {
                doc_type: "document_url".to_owned(),
                document_url: data_url,
            },
        };
        let response = self.client.ocr(api_key, &request).await?;
        Ok(stitch_ocr_markdown(&response))
    }

    async fn chat_complete(&self, prompt: String) -> Result<String, AnalysisProviderError> {
        let api_key = self.api_key()?;
        let request = MistralChatRequest {
            model: self.model.clone(),
            messages: vec![MistralChatMessage {
                role: "user".to_owned(),
                content: prompt,
            }],
            response_format: MistralResponseFormat {
                format_type: "json_object".to_owned(),
            },
        };
        let response = self
            .client
            .create_completion(&self.model, api_key, &request)
            .await?;
        extract_mistral_text(&response)
    }
}

#[async_trait]
impl<C> AiAnalysisProvider for MistralAnalysisProvider<C>
where
    C: MistralClient,
{
    fn provider_id(&self) -> &'static str {
        "provider_mistral"
    }

    fn model(&self) -> &str {
        &self.model
    }

    async fn analyze(
        &self,
        request: &AnalysisRequest,
    ) -> Result<AnalysisProviderOutput, AnalysisProviderError> {
        let text = self
            .chat_complete(prompts::analysis_prompt(request))
            .await?;
        prompts::parse_analysis_output(&text, "Mistral")
    }

    async fn generate_research_brief(
        &self,
        request: &ResearchBriefRequest,
    ) -> Result<ResearchBriefProviderOutput, AnalysisProviderError> {
        let text = self
            .chat_complete(prompts::research_brief_prompt(request))
            .await?;
        prompts::parse_research_brief_output(&text, "Mistral")
    }

    async fn generate_research_digest(
        &self,
        request: &ResearchDigestRequest,
    ) -> Result<ResearchBriefProviderOutput, AnalysisProviderError> {
        let text = self
            .chat_complete(prompts::research_digest_prompt(request))
            .await?;
        prompts::parse_research_brief_output(&text, "Mistral")
    }

    /// Native: Mistral ingests documents via OCR
    /// ([`ocr_document`](Self::ocr_document)). The extraction pipeline reaches
    /// the OCR path directly at the tier-4 hook (T4.3), not through
    /// `complete_document`.
    fn document_support(&self) -> DocumentSupport {
        DocumentSupport::Native
    }

    /// Tier-4 OCR entry point (ADR 0077 §4): OCRs the document to stitched
    /// markdown the deterministic parser consumes. Overrides the trait default so
    /// the `VisionExtraction` pool reaches OCR through the trait object without
    /// downcasting.
    async fn ocr_document(
        &self,
        bytes: &[u8],
        mime_type: &str,
    ) -> Result<String, AnalysisProviderError> {
        self.ocr_to_markdown(bytes, mime_type).await
    }

    async fn complete_document(
        &self,
        prompt: &str,
        document: &AnalysisDocument,
    ) -> Result<String, AnalysisProviderError> {
        match document {
            AnalysisDocument::Text { text } => {
                self.chat_complete(format!("{prompt}\n\nDocument text:\n{text}"))
                    .await
            }
            // Native bytes: OCR to stitched markdown, then chat over it. This is
            // the generic document-completion contract behind `document_support()
            // == Native` — the capability validator admits Mistral into
            // Document-class pools on that promise, so an erroring arm here would
            // poison such a pool at runtime (the 6ea2a8a failure under a Native
            // label). The tier-4 hook (T4.3) does NOT go through here — it calls
            // `ocr_document` directly and feeds the deterministic parser.
            AnalysisDocument::Native { mime_type, data } => {
                let markdown = self.ocr_document(data, mime_type).await?;
                self.chat_complete(format!("{prompt}\n\nDocument (OCR markdown):\n{markdown}"))
                    .await
            }
        }
    }
}

// ---- OCR request/response (minimal; serde-default tolerant) ----

#[derive(Debug, Serialize)]
pub struct MistralOcrRequest {
    model: String,
    document: MistralOcrDocument,
}

#[derive(Debug, Serialize)]
pub struct MistralOcrDocument {
    #[serde(rename = "type")]
    doc_type: String,
    document_url: String,
}

#[derive(Debug, Default, Deserialize)]
pub struct MistralOcrResponse {
    #[serde(default)]
    pub pages: Vec<MistralOcrPage>,
}

#[derive(Debug, Default, Deserialize)]
pub struct MistralOcrPage {
    #[serde(default)]
    pub markdown: String,
    /// OCR v4 returns tables separately from the page markdown; each carries its
    /// own markdown rendering that must be stitched back in.
    #[serde(default)]
    pub tables: Vec<MistralOcrTable>,
}

#[derive(Debug, Default, Deserialize)]
pub struct MistralOcrTable {
    #[serde(default)]
    pub markdown: Option<String>,
}

/// Stitches OCR pages into one markdown document, folding each page's
/// separately-returned tables in after its narrative markdown.
fn stitch_ocr_markdown(response: &MistralOcrResponse) -> String {
    let mut out = String::new();
    for page in &response.pages {
        let page_md = page.markdown.trim();
        if !page_md.is_empty() {
            if !out.is_empty() {
                out.push_str("\n\n");
            }
            out.push_str(page_md);
        }
        for table in &page.tables {
            if let Some(table_md) = table.markdown.as_deref() {
                let table_md = table_md.trim();
                if !table_md.is_empty() {
                    if !out.is_empty() {
                        out.push_str("\n\n");
                    }
                    out.push_str(table_md);
                }
            }
        }
    }
    out
}

// ---- Chat request/response (OpenAI-compatible; minimal) ----

#[derive(Debug, Serialize)]
pub struct MistralChatRequest {
    model: String,
    messages: Vec<MistralChatMessage>,
    response_format: MistralResponseFormat,
}

#[derive(Debug, Serialize)]
pub struct MistralChatMessage {
    role: String,
    content: String,
}

#[derive(Debug, Serialize)]
pub struct MistralResponseFormat {
    #[serde(rename = "type")]
    format_type: String,
}

#[derive(Debug, Default, Deserialize)]
pub struct MistralChatResponse {
    #[serde(default)]
    choices: Vec<MistralChoice>,
}

#[derive(Debug, Deserialize)]
pub struct MistralChoice {
    message: MistralResponseMessage,
}

#[derive(Debug, Deserialize)]
pub struct MistralResponseMessage {
    #[serde(default)]
    content: Option<String>,
}

fn extract_mistral_text(response: &MistralChatResponse) -> Result<String, AnalysisProviderError> {
    response
        .choices
        .first()
        .and_then(|choice| choice.message.content.clone())
        .map(|text| text.trim().to_owned())
        .filter(|text| !text.is_empty())
        .ok_or_else(|| {
            AnalysisProviderError::ParseError("Mistral response did not include text".to_owned())
        })
}

fn map_mistral_http_error(status: u16, body: &str) -> AnalysisProviderError {
    let cause = summarize_provider_error_body(body);
    match status {
        401 | 403 => AnalysisProviderError::ProviderNotConfigured,
        429 => AnalysisProviderError::ProviderLimit,
        400 => {
            AnalysisProviderError::ProviderError(format!("Mistral rejected the request: {cause}"))
        }
        500..=599 => AnalysisProviderError::ProviderUnavailable(format!(
            "Mistral service error ({status}): {cause}"
        )),
        _ => AnalysisProviderError::ProviderError(format!("Mistral error ({status}): {cause}")),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::fundamentals::extraction::ocr::{
        parse_ocr_markdown, OcrExtractionProfile, ValueColumnLayout,
    };
    use crate::fundamentals::extraction::pdf::UnitScale;
    use crate::fundamentals::extraction::{FactPeriod, SourceTier};
    use crate::storage::FeedItem;
    use std::collections::BTreeMap;

    fn block_on<F: std::future::Future>(future: F) -> F::Output {
        tauri::async_runtime::block_on(future)
    }

    /// A mock covering both endpoints; each returns a canned body. When
    /// `expect_prompt_contains` is set, the chat endpoint asserts the incoming
    /// prompt carries that text (used to prove Native docs reach chat as
    /// stitched OCR markdown).
    struct MockMistralClient {
        ocr: MistralOcrResponse,
        chat_text: String,
        expect_prompt_contains: Option<String>,
    }

    #[async_trait]
    impl MistralClient for MockMistralClient {
        async fn ocr(
            &self,
            api_key: &str,
            request: &MistralOcrRequest,
        ) -> Result<MistralOcrResponse, AnalysisProviderError> {
            assert_eq!(api_key, "test-api-key");
            assert_eq!(request.model, MISTRAL_OCR_MODEL);
            assert!(request
                .document
                .document_url
                .starts_with("data:application/pdf;base64,"));
            // Re-emit the canned response.
            Ok(MistralOcrResponse {
                pages: self
                    .ocr
                    .pages
                    .iter()
                    .map(|p| MistralOcrPage {
                        markdown: p.markdown.clone(),
                        tables: p
                            .tables
                            .iter()
                            .map(|t| MistralOcrTable {
                                markdown: t.markdown.clone(),
                            })
                            .collect(),
                    })
                    .collect(),
            })
        }

        async fn create_completion(
            &self,
            model: &str,
            api_key: &str,
            request: &MistralChatRequest,
        ) -> Result<MistralChatResponse, AnalysisProviderError> {
            assert_eq!(model, DEFAULT_MISTRAL_ANALYSIS_MODEL);
            assert_eq!(api_key, "test-api-key");
            assert_eq!(request.messages.len(), 1);
            if let Some(expected) = &self.expect_prompt_contains {
                assert!(
                    request.messages[0].content.contains(expected),
                    "chat prompt should contain {expected:?}, got: {}",
                    request.messages[0].content
                );
            }
            Ok(MistralChatResponse {
                choices: vec![MistralChoice {
                    message: MistralResponseMessage {
                        content: Some(self.chat_text.clone()),
                    },
                }],
            })
        }
    }

    fn empty_mock() -> MockMistralClient {
        MockMistralClient {
            ocr: MistralOcrResponse::default(),
            chat_text: "{}".to_owned(),
            expect_prompt_contains: None,
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
    fn mistral_provider_requires_configuration() {
        let provider = MistralAnalysisProvider::with_client(
            None,
            DEFAULT_MISTRAL_ANALYSIS_MODEL,
            empty_mock(),
        );
        let error =
            block_on(provider.analyze(&sample_request())).expect_err("unconfigured Mistral fails");
        assert_eq!(provider.provider_id(), "provider_mistral");
        assert_eq!(error.code(), "provider_not_configured");
    }

    #[test]
    fn mistral_ocr_document_requires_configuration() {
        let provider = MistralAnalysisProvider::with_client(
            None,
            DEFAULT_MISTRAL_ANALYSIS_MODEL,
            empty_mock(),
        );
        let error = block_on(provider.ocr_document(&[0x50, 0x44, 0x46], "application/pdf"))
            .expect_err("unconfigured OCR fails");
        assert_eq!(error.code(), "provider_not_configured");
    }

    #[test]
    fn mistral_analyze_parses_structured_output() {
        let provider = MistralAnalysisProvider::with_client(
            Some("test-api-key".to_owned()),
            DEFAULT_MISTRAL_ANALYSIS_MODEL,
            MockMistralClient {
                ocr: MistralOcrResponse::default(),
                chat_text: r#"{"summary":"Revenue improved.","significance":"medium","tags":["earnings"],"reasoning":"Higher revenue.","language":"en","sourceReferences":[{"sourceUrl":"https://example.com/report","label":"Report"}]}"#.to_owned(),
                expect_prompt_contains: None,
            },
        );
        let output =
            block_on(provider.analyze(&sample_request())).expect("mock Mistral response parses");
        assert_eq!(output.summary, "Revenue improved.");
        assert_eq!(output.significance, "medium");
    }

    // Guardrail (QG T4.1): `document_support() == Native` is a CONTRACT — the
    // capability validator admits Mistral into Document-class pools on its
    // strength (settings.rs Document⇒Native check), so `complete_document`
    // with Native bytes must actually work: OCR → stitched markdown → chat.
    // An erroring Native arm would recreate the poisoned-pool failure the
    // hard-fail exists to prevent (6ea2a8a), just under a Native label.
    #[test]
    fn mistral_completes_a_native_document_via_ocr_then_chat() {
        let provider = MistralAnalysisProvider::with_client(
            Some("test-api-key".to_owned()),
            DEFAULT_MISTRAL_ANALYSIS_MODEL,
            MockMistralClient {
                ocr: MistralOcrResponse {
                    pages: vec![MistralOcrPage {
                        markdown: "# Sprawozdanie".to_owned(),
                        tables: vec![MistralOcrTable {
                            markdown: Some(
                                "| Pozycja | 2025 |\n| --- | --- |\n| Przychody | 1 500 |"
                                    .to_owned(),
                            ),
                        }],
                    }],
                },
                chat_text: r#"{"answer":"ok"}"#.to_owned(),
                // The stitched table must reach the chat prompt — this is what
                // makes the Native contract real, not nominal.
                expect_prompt_contains: Some("| Przychody | 1 500 |".to_owned()),
            },
        );
        let document = AnalysisDocument::Native {
            mime_type: "application/pdf".to_owned(),
            data: vec![0x50, 0x44, 0x46],
        };
        let output = block_on(provider.complete_document("Extract KPIs.", &document))
            .expect("Native document completes via OCR+chat");
        assert_eq!(output, r#"{"answer":"ok"}"#);
    }

    #[test]
    fn mistral_document_support_is_native() {
        let provider = MistralAnalysisProvider::with_client(
            Some("test-api-key".to_owned()),
            DEFAULT_MISTRAL_ANALYSIS_MODEL,
            empty_mock(),
        );
        assert_eq!(provider.document_support(), DocumentSupport::Native);
    }

    #[test]
    fn mistral_ocr_stitches_separate_tables_into_markdown() {
        // OCR v4 shape: page markdown carries the narrative; the statement table
        // arrives separately in pages[].tables. The stitch must fold it back in
        // so the deterministic parser can read it (unstitched → 0 facts).
        let provider = MistralAnalysisProvider::with_client(
            Some("test-api-key".to_owned()),
            DEFAULT_MISTRAL_ANALYSIS_MODEL,
            MockMistralClient {
                ocr: MistralOcrResponse {
                    pages: vec![MistralOcrPage {
                        markdown: "# Rachunek zysków i strat\n\nDane w tys. zł.".to_owned(),
                        tables: vec![MistralOcrTable {
                            markdown: Some(
                                "| Pozycja | 2025 |\n| --- | --- |\n| Przychody ze sprzedaży | 1 500 |"
                                    .to_owned(),
                            ),
                        }],
                    }],
                },
                chat_text: "{}".to_owned(),
                expect_prompt_contains: None,
            },
        );
        let markdown = block_on(provider.ocr_document(&[0x50, 0x44, 0x46], "application/pdf"))
            .expect("OCR stitch succeeds");
        assert!(markdown.contains("Rachunek zysków i strat"));
        assert!(markdown.contains("| Przychody ze sprzedaży | 1 500 |"));

        // The stitched markdown parses to a fact through a profile.
        let profile = OcrExtractionProfile::bootstrap(
            "GPW:TST",
            UnitScale::Thousands,
            BTreeMap::from([("przychody ze sprzedaży".to_string(), "revenue".to_string())]),
            ValueColumnLayout::CurrentPeriodFirst,
            Vec::new(),
            false,
        );
        let facts = parse_ocr_markdown(
            &markdown,
            &profile,
            &FactPeriod::Instant("2025-09-30".to_string()),
            SourceTier::AiText,
        );
        assert_eq!(facts.len(), 1);
        assert_eq!(facts[0].metric_key, "revenue");
        assert_eq!(facts[0].value.to_string(), "1500000");
    }

    #[test]
    fn mistral_http_error_maps_status_taxonomy() {
        let body = r#"{"error":{"message":"boom"}}"#;
        assert_eq!(
            map_mistral_http_error(401, body).code(),
            "provider_not_configured"
        );
        assert_eq!(map_mistral_http_error(429, body).code(), "provider_limit");
        assert_eq!(
            map_mistral_http_error(500, body).code(),
            "provider_unavailable"
        );
        assert_eq!(
            map_mistral_http_error(503, body).code(),
            "provider_unavailable"
        );
        assert_eq!(map_mistral_http_error(400, body).code(), "provider_error");
    }
}
