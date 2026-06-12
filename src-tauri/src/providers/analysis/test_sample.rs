use super::types::{
    AiAnalysisProvider, AnalysisProviderError, AnalysisProviderOutput, AnalysisRequest,
    AnalysisSourceReference, ResearchBriefCitationOutput, ResearchBriefProviderOutput,
    ResearchBriefRequest, ResearchBriefSectionOutput, ResearchDigestRequest,
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

    fn generate_research_brief(
        &self,
        request: &ResearchBriefRequest,
    ) -> Result<ResearchBriefProviderOutput, AnalysisProviderError> {
        let citations = request
            .evidence_items
            .iter()
            .take(6)
            .enumerate()
            .map(|(index, item)| ResearchBriefCitationOutput {
                citation_key: format!("E{}", index + 1),
                evidence_type: item.evidence_type.clone(),
                evidence_id: item.source_id.clone(),
                label: item.title.clone(),
                snippet: item.summary.clone(),
            })
            .collect::<Vec<_>>();

        if citations.is_empty() {
            return Err(AnalysisProviderError::ParseError(
                "research brief requires at least one evidence item".to_owned(),
            ));
        }

        let citation_keys = citations
            .iter()
            .map(|citation| citation.citation_key.clone())
            .collect::<Vec<_>>();

        Ok(ResearchBriefProviderOutput {
            title: format!("Research brief for {}", request.scope_id),
            summary: format!(
                "Deterministic source-grounded brief for {} research evidence.",
                request.scope_type
            ),
            sections: vec![
                ResearchBriefSectionOutput {
                    heading: "What changed".to_owned(),
                    body: "Recent tracked evidence should be reviewed together before drawing conclusions."
                        .to_owned(),
                    citation_keys: citation_keys.clone(),
                },
                ResearchBriefSectionOutput {
                    heading: "Open checks".to_owned(),
                    body: "Verify management claims and upcoming events against the cited source material."
                        .to_owned(),
                    citation_keys,
                },
            ],
            language: Some("en".to_owned()),
            citations,
        })
    }

    fn generate_research_digest(
        &self,
        request: &ResearchDigestRequest,
    ) -> Result<ResearchBriefProviderOutput, AnalysisProviderError> {
        let citations = request
            .evidence_items
            .iter()
            .take(8)
            .enumerate()
            .map(|(index, item)| ResearchBriefCitationOutput {
                citation_key: format!("E{}", index + 1),
                evidence_type: item.evidence_type.clone(),
                evidence_id: item.source_id.clone(),
                label: item.title.clone(),
                snippet: item.summary.clone(),
            })
            .collect::<Vec<_>>();

        if citations.is_empty() {
            return Err(AnalysisProviderError::ParseError(
                "research digest requires at least one evidence item".to_owned(),
            ));
        }

        let citation_keys = citations
            .iter()
            .map(|citation| citation.citation_key.clone())
            .collect::<Vec<_>>();

        Ok(ResearchBriefProviderOutput {
            title: format!("Research digest for {}", request.scope_id),
            summary: format!(
                "Deterministic digest for changed {} research evidence and open reminders.",
                request.scope_type
            ),
            sections: vec![
                ResearchBriefSectionOutput {
                    heading: "Review queue".to_owned(),
                    body: "Start with changed evidence and open reminders before marking the scope reviewed."
                        .to_owned(),
                    citation_keys: citation_keys.clone(),
                },
                ResearchBriefSectionOutput {
                    heading: "Follow-ups".to_owned(),
                    body: "Resolve stale claims, dated events, and open research questions against the cited evidence."
                        .to_owned(),
                    citation_keys,
                },
            ],
            language: Some("en".to_owned()),
            citations,
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
