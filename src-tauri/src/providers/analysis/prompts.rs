//! Provider-neutral prompt construction and output parsing for AI analysis.
//!
//! The prompt text and the expected JSON output shape are identical across
//! providers (ADR 0028) — only the wire envelope differs. Each adapter wraps
//! these prompt strings in its request format and feeds the model's text back
//! into these parsers, so prompts and parsing live in one place.

use serde::Deserialize;

use crate::providers::common::{clean_optional_string, extract_json_object};

use super::types::{
    AnalysisProviderError, AnalysisProviderOutput, AnalysisRequest, AnalysisSourceReference,
    ResearchBriefCitationOutput, ResearchBriefProviderOutput, ResearchBriefRequest,
    ResearchBriefSectionOutput, ResearchDigestRequest,
};

/// Build the analysis prompt for a single feed item.
pub fn analysis_prompt(request: &AnalysisRequest) -> String {
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
    format!(
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
    )
}

fn evidence_block(items: &[crate::storage::ResearchEvidenceItem], limit: usize) -> String {
    items
        .iter()
        .take(limit)
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
        .join("\n")
}

/// Build the research-brief prompt for a scope's evidence set.
pub fn research_brief_prompt(request: &ResearchBriefRequest) -> String {
    let evidence = evidence_block(&request.evidence_items, 120);
    format!(
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
    )
}

/// Build the research-digest prompt for a scope's evidence set.
pub fn research_digest_prompt(request: &ResearchDigestRequest) -> String {
    let evidence = evidence_block(&request.evidence_items, 140);
    format!(
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
    )
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
struct AnalysisJson {
    summary: String,
    significance: String,
    reasoning: String,
    language: Option<String>,
    #[serde(default)]
    tags: Vec<String>,
    #[serde(default)]
    source_references: Vec<AnalysisSourceReferenceJson>,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
struct AnalysisSourceReferenceJson {
    source_url: String,
    label: Option<String>,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
struct ResearchBriefJson {
    title: String,
    summary: String,
    #[serde(default)]
    sections: Vec<ResearchBriefSectionJson>,
    language: Option<String>,
    #[serde(default)]
    citations: Vec<ResearchBriefCitationJson>,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
struct ResearchBriefSectionJson {
    heading: String,
    body: String,
    #[serde(default)]
    citation_keys: Vec<String>,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
struct ResearchBriefCitationJson {
    citation_key: String,
    evidence_type: String,
    evidence_id: String,
    label: String,
    snippet: Option<String>,
}

/// Parse a model's analysis text (JSON) into the neutral analysis output.
/// `provider_label` is used only for error messages.
pub fn parse_analysis_output(
    text: &str,
    provider_label: &str,
) -> Result<AnalysisProviderOutput, AnalysisProviderError> {
    let json_text = extract_json_object(text, &format!("{provider_label} response"))
        .map_err(AnalysisProviderError::ParseError)?;
    let parsed: AnalysisJson = serde_json::from_str(json_text).map_err(|error| {
        AnalysisProviderError::ParseError(format!("{provider_label} analysis JSON: {error}"))
    })?;
    let significance = parsed.significance.trim().to_lowercase();

    if !["low", "medium", "high", "unknown"].contains(&significance.as_str()) {
        return Err(AnalysisProviderError::ParseError(format!(
            "{provider_label} analysis significance is not supported: {}",
            parsed.significance
        )));
    }

    let summary = parsed.summary.trim().to_owned();
    let reasoning = parsed.reasoning.trim().to_owned();
    if summary.is_empty() || reasoning.is_empty() {
        return Err(AnalysisProviderError::ParseError(format!(
            "{provider_label} analysis did not include usable summary and reasoning"
        )));
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
        return Err(AnalysisProviderError::ParseError(format!(
            "{provider_label} analysis did not include source references"
        )));
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

/// Parse a model's research-brief/digest text (JSON) into the neutral output.
/// `provider_label` is used only for error messages.
pub fn parse_research_brief_output(
    text: &str,
    provider_label: &str,
) -> Result<ResearchBriefProviderOutput, AnalysisProviderError> {
    let json_text = extract_json_object(text, &format!("{provider_label} response"))
        .map_err(AnalysisProviderError::ParseError)?;
    let parsed: ResearchBriefJson = serde_json::from_str(json_text).map_err(|error| {
        AnalysisProviderError::ParseError(format!("{provider_label} research brief JSON: {error}"))
    })?;

    let title = parsed.title.trim().to_owned();
    let summary = parsed.summary.trim().to_owned();
    if title.is_empty() || summary.is_empty() {
        return Err(AnalysisProviderError::ParseError(format!(
            "{provider_label} research brief did not include usable title and summary"
        )));
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
        return Err(AnalysisProviderError::ParseError(format!(
            "{provider_label} research brief did not include sections and citations"
        )));
    }

    Ok(ResearchBriefProviderOutput {
        title,
        summary,
        sections,
        language: clean_optional_string(parsed.language),
        citations,
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn analysis_parser_rejects_missing_source_references() {
        let error = parse_analysis_output(
            r#"{"summary":"Revenue improved.","significance":"medium","tags":[],"reasoning":"The report cites higher revenue.","language":"en","sourceReferences":[]}"#,
            "Gemini",
        )
        .expect_err("source references are required");

        assert_eq!(error.code(), "parse_error");
    }

    #[test]
    fn research_brief_parser_ignores_trailing_text_after_json() {
        let output = parse_research_brief_output(
            r#"
            {"title":"Research brief","summary":"Summary with {brace} text.","sections":[{"heading":"What changed","body":"Body cites evidence.","citationKeys":["E1"]}],"language":"en","citations":[{"citationKey":"E1","evidenceType":"feed_item","evidenceId":"feed_01","label":"Report","snippet":"Snippet"}]}

            I used the provided evidence only.
            "#,
            "Gemini",
        )
        .expect("trailing text after the first complete JSON object should be ignored");

        assert_eq!(output.title, "Research brief");
        assert_eq!(output.summary, "Summary with {brace} text.");
        assert_eq!(output.citations.len(), 1);
    }

    #[test]
    fn research_brief_parser_handles_fenced_json_with_trailing_text() {
        let output = parse_research_brief_output(
            r#"
            ```json
            {"title":"Research brief","summary":"Summary.","sections":[{"heading":"What changed","body":"Body cites evidence.","citationKeys":["E1"]}],"language":"en","citations":[{"citationKey":"E1","evidenceType":"feed_item","evidenceId":"feed_01","label":"Report","snippet":"Snippet"}]}
            ```

            Done.
            "#,
            "Gemini",
        )
        .expect("fenced JSON should parse even when the provider appends text");

        assert_eq!(output.title, "Research brief");
    }
}
