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
    ExtractedKpiFact, ExtractedPeriod, KpiExtractionProviderOutput, KpiExtractionRequest,
    ResearchBriefCitationOutput, ResearchBriefProviderOutput, ResearchBriefRequest,
    ResearchBriefSectionOutput, ResearchDigestRequest,
};

/// Stable identifier of the KPI-extraction prompt, recorded as fact provenance so
/// extracted values can be traced to the exact instructions that produced them.
/// Bump when the prompt's contract changes materially.
pub const KPI_EXTRACTION_PROMPT_VERSION: &str = "kpi-extraction.v1";

/// Allowed reporting period types for extraction (primary period only).
const EXTRACTION_PERIOD_TYPES: [&str; 7] = ["Q1", "Q2", "Q3", "Q4", "H1", "H2", "FY"];

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

/// Build the KPI-extraction prompt for a report document. The document bytes are
/// sent alongside this prompt via the provider's `complete_document` path; this
/// text only grounds the model in the company, its KPI taxonomy, and the JSON
/// contract. Only the primary reporting period is extracted.
pub fn kpi_extraction_prompt(request: &KpiExtractionRequest) -> String {
    let known_kpis = if request.known_kpis.is_empty() {
        "(none provided — propose the standard financial KPIs you find)".to_owned()
    } else {
        request
            .known_kpis
            .iter()
            .map(|entry| {
                format!(
                    "- {key} — {label} (valueKind={value_kind}{unit})",
                    key = entry.metric_key,
                    label = entry.label,
                    value_kind = entry.value_kind,
                    unit = entry
                        .unit
                        .as_deref()
                        .map(|unit| format!(", unit={unit}"))
                        .unwrap_or_default(),
                )
            })
            .collect::<Vec<_>>()
            .join("\n")
    };
    let statement_type = request.statement_type.as_deref().unwrap_or("unknown");
    let period_hint = request
        .period_hint
        .as_deref()
        .map(str::trim)
        .filter(|value| !value.is_empty())
        .unwrap_or("unknown — detect it from the document");

    format!(
        "Extract financial KPI values from the attached company report document for an \
investor research workflow. \
Return only JSON with this exact shape: \
{{\"period\":{{\"fiscalYear\":2025,\"periodType\":\"Q1|Q2|Q3|Q4|H1|H2|FY\",\"periodEndDate\":\"YYYY-MM-DD\"}},\"currency\":\"PLN\",\"language\":\"pl\",\"facts\":[{{\"metricKey\":\"revenue\",\"label\":\"Revenue\",\"valueNumeric\":\"142312000\",\"unit\":\"PLN\",\"currency\":\"PLN\",\"asReportedValue\":\"142 312\",\"asReportedScale\":\"tys.\",\"measureWindow\":\"quarter|ytd|ttm|fy\",\"confidence\":\"low|medium|high\",\"sourceSnippet\":\"verbatim text from the document\",\"isProposedKpi\":false}}]}}.\n\n\
Rules:\n\
- Extract values for the PRIMARY reporting period of this document only. Ignore prior-year comparative columns and other periods.\n\
- Detect the primary period (fiscalYear, periodType, periodEndDate). periodType must be one of Q1, Q2, Q3, Q4, H1, H2, FY.\n\
- valueNumeric is the value normalized to base units as a plain decimal string (no thousands separators, no scale words); e.g. \"142 312 tys. zł\" becomes \"142312000\". Negative values keep a leading minus.\n\
- asReportedValue and asReportedScale capture the figure exactly as printed (digits and scale word) so the user can verify it.\n\
- For every fact include a verbatim sourceSnippet copied from the document and a confidence of low, medium, or high.\n\
- First extract the listed known KPIs that appear in the document (isProposedKpi=false). You may also propose additional KPIs you find that are not in the list by setting isProposedKpi=true.\n\
- Do not invent values. Omit any KPI you cannot find rather than guessing.\n\
- Do not include markdown fences, commentary outside JSON, buy/sell/hold recommendations, price targets, or any investment advice.\n\n\
Company: {company}\n\
Statement type: {statement_type}\n\
Expected period: {period_hint}\n\
Known KPIs:\n{known_kpis}",
        company = request.company_name,
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

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
struct KpiExtractionJson {
    period: Option<ExtractedPeriodJson>,
    currency: Option<String>,
    language: Option<String>,
    #[serde(default)]
    facts: Vec<ExtractedKpiFactJson>,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
struct ExtractedPeriodJson {
    fiscal_year: Option<i64>,
    period_type: Option<String>,
    period_end_date: Option<String>,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
struct ExtractedKpiFactJson {
    metric_key: String,
    label: Option<String>,
    value_numeric: String,
    unit: Option<String>,
    currency: Option<String>,
    as_reported_value: Option<String>,
    as_reported_scale: Option<String>,
    measure_window: Option<String>,
    confidence: Option<String>,
    source_snippet: Option<String>,
    #[serde(default)]
    is_proposed_kpi: bool,
}

/// Parse a model's KPI-extraction text (JSON) into the neutral extraction output.
/// `provider_label` is used only for error messages. Invalid individual facts are
/// dropped; a detected period with an unsupported `periodType` is dropped (the
/// user then maps the period during confirmation).
pub fn parse_kpi_extraction_output(
    text: &str,
    provider_label: &str,
) -> Result<KpiExtractionProviderOutput, AnalysisProviderError> {
    let json_text = extract_json_object(text, &format!("{provider_label} response"))
        .map_err(AnalysisProviderError::ParseError)?;
    let parsed: KpiExtractionJson = serde_json::from_str(json_text).map_err(|error| {
        AnalysisProviderError::ParseError(format!("{provider_label} KPI extraction JSON: {error}"))
    })?;

    let period = parsed.period.and_then(|period| {
        let fiscal_year = period.fiscal_year?;
        let period_type = period.period_type?.trim().to_uppercase();
        if !EXTRACTION_PERIOD_TYPES.contains(&period_type.as_str()) {
            return None;
        }
        Some(ExtractedPeriod {
            fiscal_year,
            period_type,
            period_end_date: clean_optional_string(period.period_end_date),
        })
    });

    let facts = parsed
        .facts
        .into_iter()
        .filter_map(|fact| {
            let metric_key = fact.metric_key.trim().to_owned();
            let value_numeric = fact.value_numeric.trim().to_owned();
            if metric_key.is_empty() || value_numeric.is_empty() {
                return None;
            }
            let label = fact
                .label
                .map(|label| label.trim().to_owned())
                .filter(|label| !label.is_empty())
                .unwrap_or_else(|| metric_key.clone());
            let confidence = clean_optional_string(fact.confidence)
                .map(|value| value.to_lowercase())
                .filter(|value| ["low", "medium", "high"].contains(&value.as_str()));
            Some(ExtractedKpiFact {
                metric_key,
                label,
                value_numeric,
                unit: clean_optional_string(fact.unit),
                currency: clean_optional_string(fact.currency),
                as_reported_value: clean_optional_string(fact.as_reported_value),
                as_reported_scale: clean_optional_string(fact.as_reported_scale),
                measure_window: clean_optional_string(fact.measure_window),
                confidence,
                source_snippet: clean_optional_string(fact.source_snippet),
                is_proposed_kpi: fact.is_proposed_kpi,
            })
        })
        .collect::<Vec<_>>();

    if facts.is_empty() {
        return Err(AnalysisProviderError::ParseError(format!(
            "{provider_label} KPI extraction did not include any usable facts"
        )));
    }

    Ok(KpiExtractionProviderOutput {
        period,
        currency: clean_optional_string(parsed.currency),
        language: clean_optional_string(parsed.language),
        facts,
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
    fn kpi_extraction_prompt_lists_known_kpis_and_period_hint() {
        let prompt = kpi_extraction_prompt(&KpiExtractionRequest {
            company_name: "cyber_Folks".to_owned(),
            statement_type: Some("industrial".to_owned()),
            known_kpis: vec![super::super::types::KpiCatalogEntry {
                metric_key: "revenue".to_owned(),
                label: "Revenue".to_owned(),
                value_kind: "currency_amount".to_owned(),
                unit: Some("PLN".to_owned()),
            }],
            period_hint: Some("Q3 2025".to_owned()),
        });

        assert!(prompt.contains("cyber_Folks"));
        assert!(prompt.contains("revenue — Revenue"));
        assert!(prompt.contains("Q3 2025"));
        assert!(prompt.contains("PRIMARY reporting period"));
    }

    #[test]
    fn kpi_extraction_parser_extracts_facts_and_period() {
        let output = parse_kpi_extraction_output(
            r#"{"period":{"fiscalYear":2025,"periodType":"q3","periodEndDate":"2025-09-30"},"currency":"PLN","language":"pl","facts":[{"metricKey":"revenue","label":"Revenue","valueNumeric":"142312000","unit":"PLN","currency":"PLN","asReportedValue":"142 312","asReportedScale":"tys.","measureWindow":"quarter","confidence":"HIGH","sourceSnippet":"przychody 142 312 tys. zl","isProposedKpi":false},{"metricKey":"backlog","label":"Backlog","valueNumeric":"410000000","confidence":"medium","isProposedKpi":true}]}"#,
            "Gemini",
        )
        .expect("valid extraction output should parse");

        let period = output.period.expect("period detected");
        assert_eq!(period.fiscal_year, 2025);
        assert_eq!(period.period_type, "Q3");
        assert_eq!(output.facts.len(), 2);
        assert_eq!(output.facts[0].confidence.as_deref(), Some("high"));
        assert!(!output.facts[0].is_proposed_kpi);
        assert!(output.facts[1].is_proposed_kpi);
    }

    #[test]
    fn kpi_extraction_parser_drops_invalid_facts_and_bad_period() {
        let output = parse_kpi_extraction_output(
            r#"{"period":{"fiscalYear":2025,"periodType":"FY2025"},"facts":[{"metricKey":"","valueNumeric":"1"},{"metricKey":"revenue","valueNumeric":""},{"metricKey":"net_profit","valueNumeric":"5000","confidence":"definitely"}]}"#,
            "Gemini",
        )
        .expect("one usable fact remains");

        assert!(output.period.is_none(), "unsupported periodType is dropped");
        assert_eq!(output.facts.len(), 1);
        assert_eq!(output.facts[0].metric_key, "net_profit");
        assert!(
            output.facts[0].confidence.is_none(),
            "out-of-range confidence is dropped"
        );
    }

    #[test]
    fn kpi_extraction_parser_rejects_when_no_usable_facts() {
        let error = parse_kpi_extraction_output(
            r#"{"period":{"fiscalYear":2025,"periodType":"Q3"},"facts":[]}"#,
            "Gemini",
        )
        .expect_err("an extraction with no facts is an error");

        assert_eq!(error.code(), "parse_error");
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
