use std::collections::HashSet;

use crate::{
    app_state,
    providers::{
        analysis::{
            AiAnalysisProvider, GeminiAnalysisProvider, ResearchBriefProviderOutput,
            ResearchBriefRequest, TestSampleAnalysisProvider, TEST_SAMPLE_ANALYSIS_PROVIDER_ID,
        },
        credentials,
    },
    storage,
};

pub fn run_research_brief_job(
    state: &app_state::AppState,
    job_id: &str,
) -> Result<storage::ResearchBriefJob, String> {
    let job = state
        .get_research_brief_job(job_id)
        .map_err(|error| error.to_string())?;
    if job.status == "succeeded" {
        return Ok(job);
    }

    let provider = provider_for_job(state, &job)?;
    let context = state
        .collect_research_brief_evidence(job_id)
        .map_err(|error| error.to_string())?;
    if context.evidence_items.is_empty() {
        return state
            .mark_research_brief_job_failed(
                job_id,
                "empty_evidence",
                "No research evidence is available for this brief.",
            )
            .map_err(|error| error.to_string());
    }

    state
        .mark_research_brief_job_running(job_id)
        .map_err(|error| error.to_string())?;

    let output = match provider.generate_research_brief(&ResearchBriefRequest {
        scope_type: context.scope_type,
        scope_id: context.scope_id,
        evidence_items: context.evidence_items.clone(),
    }) {
        Ok(output) => output,
        Err(error) => {
            return state
                .mark_research_brief_job_failed(job_id, error.code(), &error.to_string())
                .map_err(|storage_error| storage_error.to_string());
        }
    };

    let completed = completed_brief_from_provider(job_id, &context.evidence_items, output)?;
    state
        .complete_research_brief_job(completed)
        .map_err(|error| error.to_string())
}

fn provider_for_job(
    state: &app_state::AppState,
    job: &storage::ResearchBriefJob,
) -> Result<Box<dyn AiAnalysisProvider>, String> {
    match job.provider_id.as_str() {
        TEST_SAMPLE_ANALYSIS_PROVIDER_ID => Ok(Box::new(TestSampleAnalysisProvider)),
        "provider_gemini" => {
            let settings = state.get_settings().map_err(|error| error.to_string())?;
            let api_key = credentials::read_gemini_general_analysis_api_key().unwrap_or(None);
            Ok(Box::new(
                GeminiAnalysisProvider::live(
                    api_key,
                    job.model.clone(),
                    settings.ai_providers.general_analysis_timeout_seconds,
                )
                .map_err(|error| error.to_string())?,
            ))
        }
        other => Err(format!("Unknown AI analysis provider: {other}")),
    }
}

fn completed_brief_from_provider(
    job_id: &str,
    evidence_items: &[storage::ResearchEvidenceItem],
    output: ResearchBriefProviderOutput,
) -> Result<storage::CompletedResearchBrief, String> {
    let evidence_refs = evidence_items
        .iter()
        .map(|item| (item.evidence_type.clone(), item.source_id.clone()))
        .collect::<HashSet<_>>();
    let citation_keys = output
        .citations
        .iter()
        .map(|citation| citation.citation_key.clone())
        .collect::<HashSet<_>>();

    if output.sections.is_empty() {
        return Err("Research brief did not include sections.".to_owned());
    }
    if citation_keys.is_empty() {
        return Err("Research brief did not include citations.".to_owned());
    }

    for section in &output.sections {
        if section.citation_keys.is_empty() {
            return Err(format!(
                "Research brief section '{}' did not include citations.",
                section.heading
            ));
        }
        for citation_key in &section.citation_keys {
            if !citation_keys.contains(citation_key) {
                return Err(format!(
                    "Research brief section '{}' referenced unknown citation {}.",
                    section.heading, citation_key
                ));
            }
        }
    }

    let citations = output
        .citations
        .into_iter()
        .map(|citation| {
            if !evidence_refs
                .contains(&(citation.evidence_type.clone(), citation.evidence_id.clone()))
            {
                return Err(format!(
                    "Research brief citation {} references unavailable evidence.",
                    citation.citation_key
                ));
            }
            Ok(storage::NewResearchBriefCitation {
                citation_key: citation.citation_key,
                evidence_type: citation.evidence_type,
                evidence_id: citation.evidence_id,
                label: citation.label,
                snippet: citation.snippet,
            })
        })
        .collect::<Result<Vec<_>, _>>()?;

    Ok(storage::CompletedResearchBrief {
        job_id: job_id.to_owned(),
        title: output.title,
        summary: output.summary,
        content_markdown: render_brief_markdown(&output.sections),
        language: output.language,
        citations,
    })
}

fn render_brief_markdown(
    sections: &[crate::providers::analysis::ResearchBriefSectionOutput],
) -> String {
    sections
        .iter()
        .map(|section| {
            let citations = section
                .citation_keys
                .iter()
                .map(|key| format!("[{key}]"))
                .collect::<Vec<_>>()
                .join(" ");
            format!("## {}\n\n{} {}", section.heading, section.body, citations)
                .trim()
                .to_owned()
        })
        .collect::<Vec<_>>()
        .join("\n\n")
}

#[cfg(test)]
mod tests {
    use super::completed_brief_from_provider;
    use crate::{
        providers::analysis::{
            ResearchBriefCitationOutput, ResearchBriefProviderOutput, ResearchBriefSectionOutput,
        },
        storage::{ResearchEvidenceItem, ResearchEvidenceReviewState},
    };

    #[test]
    fn rejects_unknown_citation_keys() {
        let result = completed_brief_from_provider(
            "job_1",
            &[sample_evidence()],
            ResearchBriefProviderOutput {
                title: "Brief".to_owned(),
                summary: "Summary".to_owned(),
                sections: vec![ResearchBriefSectionOutput {
                    heading: "Changed".to_owned(),
                    body: "Body".to_owned(),
                    citation_keys: vec!["E2".to_owned()],
                }],
                language: Some("en".to_owned()),
                citations: vec![sample_citation()],
            },
        );

        assert!(result.is_err());
    }

    #[test]
    fn renders_grounded_brief_markdown() {
        let result = completed_brief_from_provider(
            "job_1",
            &[sample_evidence()],
            ResearchBriefProviderOutput {
                title: "Brief".to_owned(),
                summary: "Summary".to_owned(),
                sections: vec![ResearchBriefSectionOutput {
                    heading: "Changed".to_owned(),
                    body: "Body".to_owned(),
                    citation_keys: vec!["E1".to_owned()],
                }],
                language: Some("en".to_owned()),
                citations: vec![sample_citation()],
            },
        )
        .expect("brief should render");

        assert!(result.content_markdown.contains("## Changed"));
        assert!(result.content_markdown.contains("[E1]"));
        assert_eq!(result.citations.len(), 1);
    }

    fn sample_evidence() -> ResearchEvidenceItem {
        ResearchEvidenceItem {
            id: "evidence_feed_1".to_owned(),
            evidence_type: "feed_item".to_owned(),
            source_domain: "feed".to_owned(),
            source_id: "feed_1".to_owned(),
            company_id: "company_gpw_cdr".to_owned(),
            occurred_at: "2026-06-11T10:00:00Z".to_owned(),
            title: "Report".to_owned(),
            summary: Some("Summary".to_owned()),
            source_url: Some("https://example.com".to_owned()),
            attribution: Some("Example".to_owned()),
            trust_category: "official_report".to_owned(),
            review_state: ResearchEvidenceReviewState {
                changed_since_company_review: true,
                changed_since_watchlist_review: true,
            },
        }
    }

    fn sample_citation() -> ResearchBriefCitationOutput {
        ResearchBriefCitationOutput {
            citation_key: "E1".to_owned(),
            evidence_type: "feed_item".to_owned(),
            evidence_id: "feed_1".to_owned(),
            label: "Report".to_owned(),
            snippet: Some("Summary".to_owned()),
        }
    }
}
