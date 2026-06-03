use super::types::{
    AiAnalysisProvider, AnalysisProviderError, AnalysisProviderOutput, AnalysisRequest,
    AnalysisSourceReference,
};

pub const TEST_SAMPLE_ANALYSIS_PROVIDER_ID: &str = "test_sample";
pub const TEST_SAMPLE_ANALYSIS_MODEL: &str = "test-sample-analysis-v1";

pub struct TestSampleAnalysisProvider;

impl AiAnalysisProvider for TestSampleAnalysisProvider {
    fn provider_id(&self) -> &'static str {
        TEST_SAMPLE_ANALYSIS_PROVIDER_ID
    }

    fn model(&self) -> &str {
        TEST_SAMPLE_ANALYSIS_MODEL
    }

    fn analyze(
        &self,
        request: &AnalysisRequest,
    ) -> Result<AnalysisProviderOutput, AnalysisProviderError> {
        let item = &request.feed_item;
        let context = if let Some(question) = &request.custom_question {
            format!(" The user asked: {question}")
        } else {
            String::new()
        };

        Ok(AnalysisProviderOutput {
            summary: format!("{}: {}", item.company, item.title),
            significance: "medium".to_owned(),
            reasoning: format!(
                "Deterministic source-grounded sample based on {} from {}.{}",
                item.source, item.attribution, context
            ),
            language: Some(item.language.clone()),
            tags: vec![
                request.prompt_preset_id.clone(),
                item.item_type.clone(),
                "test-sample".to_owned(),
            ],
            source_references: vec![AnalysisSourceReference {
                source_url: item.source_url.clone(),
                label: Some(item.source.clone()),
            }],
        })
    }
}

#[cfg(test)]
mod tests {
    use crate::{
        providers::analysis::{AiAnalysisProvider, TestSampleAnalysisProvider},
        storage::FeedItem,
    };

    use super::super::types::AnalysisRequest;

    #[test]
    fn test_sample_provider_returns_source_grounded_output() {
        let provider = TestSampleAnalysisProvider;
        let output = provider
            .analyze(&AnalysisRequest {
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
            })
            .expect("test provider should produce output");

        assert_eq!(provider.provider_id(), "test_sample");
        assert_eq!(provider.model(), "test-sample-analysis-v1");
        assert_eq!(output.significance, "medium");
        assert_eq!(output.source_references.len(), 1);
    }
}
