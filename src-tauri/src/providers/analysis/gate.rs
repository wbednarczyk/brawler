//! Per-provider concurrency gate ([ADR 0059](../../../../docs/adr/0059-worker-pools-and-queue-fairness.md)).
//!
//! [`GatedAnalysisProvider`] wraps any [`AiAnalysisProvider`] with a shared
//! semaphore so at most `N` calls to a given provider run at once — the real AI
//! cost/rate ceiling, independent of how many worker threads exist. Applied at
//! build time (`registry::build_gated_analysis_provider`), so every method call on
//! the returned provider is gated with no per-call-site wiring. The semaphore is
//! shared across the `autopilot` and `ai` lanes, so the limit is a true per-provider
//! total, not per-lane.

use std::sync::Arc;

use async_trait::async_trait;
use tokio::sync::Semaphore;

use super::types::{
    AiAnalysisProvider, AnalysisDocument, AnalysisProviderError, AnalysisProviderOutput,
    AnalysisRequest, DocumentSupport, ResearchBriefProviderOutput, ResearchBriefRequest,
    ResearchDigestRequest,
};

/// A provider decorator that holds a permit from a shared per-provider semaphore
/// for the whole duration of each model call.
pub struct GatedAnalysisProvider {
    inner: Box<dyn AiAnalysisProvider>,
    semaphore: Arc<Semaphore>,
}

impl GatedAnalysisProvider {
    pub fn new(inner: Box<dyn AiAnalysisProvider>, semaphore: Arc<Semaphore>) -> Self {
        Self { inner, semaphore }
    }
}

#[async_trait]
impl AiAnalysisProvider for GatedAnalysisProvider {
    fn provider_id(&self) -> &'static str {
        self.inner.provider_id()
    }

    fn model(&self) -> &str {
        self.inner.model()
    }

    fn document_support(&self) -> DocumentSupport {
        self.inner.document_support()
    }

    async fn analyze(
        &self,
        request: &AnalysisRequest,
    ) -> Result<AnalysisProviderOutput, AnalysisProviderError> {
        let _permit = self.semaphore.acquire().await;
        self.inner.analyze(request).await
    }

    async fn generate_research_brief(
        &self,
        request: &ResearchBriefRequest,
    ) -> Result<ResearchBriefProviderOutput, AnalysisProviderError> {
        let _permit = self.semaphore.acquire().await;
        self.inner.generate_research_brief(request).await
    }

    async fn generate_research_digest(
        &self,
        request: &ResearchDigestRequest,
    ) -> Result<ResearchBriefProviderOutput, AnalysisProviderError> {
        let _permit = self.semaphore.acquire().await;
        self.inner.generate_research_digest(request).await
    }

    async fn complete_document(
        &self,
        prompt: &str,
        document: &AnalysisDocument,
    ) -> Result<String, AnalysisProviderError> {
        let _permit = self.semaphore.acquire().await;
        self.inner.complete_document(prompt, document).await
    }

    async fn ocr_document(
        &self,
        bytes: &[u8],
        mime_type: &str,
    ) -> Result<String, AnalysisProviderError> {
        let _permit = self.semaphore.acquire().await;
        self.inner.ocr_document(bytes, mime_type).await
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::providers::analysis::TestSampleAnalysisProvider;
    use crate::storage::FeedItem;

    /// The gate is transparent: it delegates identity and forwards calls to the
    /// inner provider unchanged (the semaphore only bounds concurrency). Concurrency
    /// bounding itself is covered by the `provider_semaphore` store test.
    #[test]
    fn gate_delegates_identity_and_forwards_calls() {
        let semaphore = Arc::new(Semaphore::new(1));
        let gated = GatedAnalysisProvider::new(Box::new(TestSampleAnalysisProvider), semaphore);

        assert_eq!(gated.provider_id(), "test_sample");
        assert_eq!(gated.model(), "test-sample-analysis-v1");

        let output = tauri::async_runtime::block_on(gated.analyze(&AnalysisRequest {
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
        }))
        .expect("gated provider forwards analyze to the inner provider");
        assert_eq!(output.source_references.len(), 1);
    }
}
