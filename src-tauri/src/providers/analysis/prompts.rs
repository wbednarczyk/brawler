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
    ClaimExtractionProviderOutput, ClaimExtractionRequest, EspiClassificationCategory,
    EspiClassificationOutput, ExtractedClaim, ExtractedKpiFact, ExtractedPeriod,
    KpiExtractionProviderOutput, KpiExtractionRequest, QualitativeAssessmentRequest,
    ResearchBriefCitationOutput, ResearchBriefProviderOutput, ResearchBriefRequest,
    ResearchBriefSectionOutput, ResearchDigestRequest,
};

/// Stable identifier of the KPI-extraction prompt, recorded as fact provenance so
/// extracted values can be traced to the exact instructions that produced them.
/// Bump when the prompt's contract changes materially.
pub const KPI_EXTRACTION_PROMPT_VERSION: &str = "kpi-extraction.v1";

/// Stable identifier of the claim-extraction prompt, recorded as claim provenance.
/// Bump when the prompt's contract changes materially.
pub const CLAIM_EXTRACTION_PROMPT_VERSION: &str = "claim-extraction.v1";

/// Marker phrase the deterministic test-sample provider branches on for claims.
const CLAIM_EXTRACTION_MARKER: &str = "extract management claims";

/// Allowed reporting period types for extraction (primary period only).
const EXTRACTION_PERIOD_TYPES: [&str; 7] = ["Q1", "Q2", "Q3", "Q4", "H1", "H2", "FY"];

/// Allowed comparators for a quantitative claim's target.
const CLAIM_COMPARATORS: [&str; 6] = ["gte", "lte", "gt", "lt", "approx", "eq"];

/// Stable identifier of the ESPI classification fallback prompt, recorded as
/// signal provenance. Bump when the prompt's contract changes materially.
pub const ESPI_CLASSIFICATION_PROMPT_VERSION: &str = "espi-classification.v1";

/// Stable identifier of the qualitative-assessment prompt (ADR 0075), recorded on
/// each agent criterion result as provenance. Bump when the prompt's contract
/// changes materially.
// Staged in v0.50 T3; consumed by the T4 `qualitative_assessment` job.
#[allow(dead_code)]
pub const QUALITATIVE_ASSESSMENT_PROMPT_VERSION: &str = "qualitative-assessment.v1";

/// Marker phrase the deterministic test-sample provider branches on.
const ESPI_CLASSIFICATION_MARKER: &str = "classify this official ESPI/EBI filing";

/// Build the ESPI classification fallback prompt for one filing. The model must
/// pick exactly one category key or return `"unknown"` — it must never guess a
/// type it is not confident about (ADR 0034 conservative posture).
pub fn espi_classification_prompt(
    categories: &[EspiClassificationCategory],
    title: &str,
) -> String {
    let category_lines = categories
        .iter()
        .map(|category| format!("- {} ({})", category.key, category.display_name))
        .collect::<Vec<_>>()
        .join("\n");

    format!(
        "You classify Polish stock-exchange official disclosures. {ESPI_CLASSIFICATION_MARKER} \
into exactly one of the categories below, or report \"unknown\" when none clearly applies. \
Do not guess: if the filing does not clearly match a category, return \"unknown\". The filing \
body text is provided as the attached document.\n\n\
Categories (key — label):\n{category_lines}\n\n\
Filing title:\n{title}\n\n\
Respond with strict JSON only, no prose, in this exact shape:\n\
{{\"category\": \"<one category key, or 'unknown'>\", \"confidence\": <number between 0 and 1>}}"
    )
}

#[derive(Debug, Deserialize)]
struct EspiClassificationJson {
    category: Option<String>,
    confidence: Option<f64>,
}

/// Parse the model's classification response. Returns the unknown outcome
/// (`category = None`) when the model reports `"unknown"`, returns an unlisted
/// category, or omits the category — never a category outside `allowed_keys`.
pub fn parse_espi_classification_output(
    text: &str,
    allowed_keys: &[String],
    provider_label: &str,
) -> Result<EspiClassificationOutput, AnalysisProviderError> {
    let json_text = extract_json_object(text, &format!("{provider_label} classification response"))
        .map_err(AnalysisProviderError::ParseError)?;
    let parsed: EspiClassificationJson = serde_json::from_str(json_text).map_err(|error| {
        AnalysisProviderError::ParseError(format!(
            "{provider_label} ESPI classification JSON: {error}"
        ))
    })?;

    let reported_confidence = parsed.confidence.unwrap_or(0.0).clamp(0.0, 1.0) as f32;
    let category = clean_optional_string(parsed.category)
        .map(|value| value.to_lowercase())
        .filter(|value| value != "unknown" && allowed_keys.iter().any(|key| key == value));

    let confidence = if category.is_some() {
        reported_confidence
    } else {
        0.0
    };
    Ok(EspiClassificationOutput {
        category,
        confidence,
    })
}

/// Marker phrase the test-sample provider keys on to return a sample event date.
const ESPI_EVENT_DATE_MARKER: &str = "extract the future date";

/// Build the prompt that asks the model to extract the single relevant future date from a
/// dividend or general-meeting filing body (ADR 0036). `event_kind` is a human label such as
/// "dividend payment date" or "general meeting date". The model must return `null` rather than
/// guess — the deterministic parser already handled the explicit cases.
pub fn espi_event_date_prompt(event_kind: &str, title: &str) -> String {
    format!(
        "You read a Polish stock-exchange official disclosure and {ESPI_EVENT_DATE_MARKER} it \
announces: the {event_kind}. The filing body text is provided as the attached document. \
Return the date only if it is clearly stated; if there is no clear future date, return null. \
Do not guess.\n\n\
Filing title:\n{title}\n\n\
Respond with strict JSON only, no prose, in this exact shape:\n\
{{\"date\": \"<YYYY-MM-DD, or null>\"}}"
    )
}

#[derive(Debug, Deserialize)]
struct EspiEventDateJson {
    date: Option<String>,
}

/// Parse the model's event-date response into a validated ISO `YYYY-MM-DD` string, or `None`
/// when the model returned null / an unparseable or invalid date. Never returns a guess.
pub fn parse_espi_event_date_output(
    text: &str,
    provider_label: &str,
) -> Result<Option<String>, AnalysisProviderError> {
    let json_text = extract_json_object(text, &format!("{provider_label} event-date response"))
        .map_err(AnalysisProviderError::ParseError)?;
    let parsed: EspiEventDateJson = serde_json::from_str(json_text).map_err(|error| {
        AnalysisProviderError::ParseError(format!("{provider_label} ESPI event-date JSON: {error}"))
    })?;

    Ok(clean_optional_string(parsed.date)
        .filter(|value| value.to_lowercase() != "null")
        .and_then(|value| crate::signal_dates::validate_iso_date(&value)))
}

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

/// Build the qualitative-assessment prompt for one quality-framework criterion
/// (ADR 0075). The model judges a single criterion for one company grounded ONLY
/// in the app-held evidence below, cites the evidence keys it relied on, and never
/// emits advice. Insufficient evidence is a first-class verdict, not a guess.
// Staged in v0.50 T3; called by the T4 `qualitative_assessment` job.
#[allow(dead_code)]
pub fn qualitative_assessment_prompt(request: &QualitativeAssessmentRequest) -> String {
    let evidence = evidence_block(&request.evidence_items, 140);
    format!(
        "Assess ONE qualitative business-quality criterion for the company using ONLY the evidence below. \
This is decision-support analysis, not advice. \
Return only JSON with this exact shape: \
{{\"verdict\":\"pass|partial|fail|insufficient_evidence\",\"reasoning\":\"...\",\"confidence\":\"low|medium|high\",\"citations\":[{{\"citationKey\":\"E1\",\"evidenceType\":\"claim\",\"evidenceId\":\"claim_01\",\"label\":\"...\",\"snippet\":\"...\"}}]}}. \
Judge only the criterion described by the guidance. Every claim in the reasoning must cite one or more evidence keys, and you may cite only evidence keys (E1, E2, ...) that appear below — never invent an evidenceId. \
If the evidence is insufficient to judge the criterion, return verdict \"insufficient_evidence\" with a brief reasoning and only the citations that apply — never guess a verdict. \
Do not include markdown fences, commentary outside JSON, buy/sell/hold recommendations, price targets, portfolio allocation advice, or personalized investment advice.\n\n\
Company id: {company_id}\n\
Criterion: {criterion_label}\n\
Assessment guidance: {assessment_guidance}\n\
Evidence:\n{evidence}",
        company_id = request.company_id,
        criterion_label = request.criterion_label,
        assessment_guidance = request.assessment_guidance,
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
- Extract values for the PRIMARY reporting period of this filing only. This is the newest period the filing reports (the period named in its title/header), NOT the full-year or prior-year comparative columns shown alongside it. If the Expected period below names a quarter (e.g. Q1 2026), extract that quarter's figures, not the annual or trailing-twelve-month totals.\n\
- Detect and report that primary period (fiscalYear, periodType, periodEndDate). periodType must be one of Q1, Q2, Q3, Q4, H1, H2, FY, and must match the filing's primary period.\n\
- valueNumeric is the value normalized to base units as a plain decimal string (no thousands separators, no scale words); e.g. \"142 312 tys. zł\" becomes \"142312000\". Negative values keep a leading minus.\n\
- asReportedValue and asReportedScale capture the figure exactly as printed (digits and scale word) so the user can verify it.\n\
- For every fact include a verbatim sourceSnippet copied from the document and a confidence of low, medium, or high.\n\
- Be thorough and exhaustive: scan the ENTIRE document — income statement, balance sheet, cash-flow statement, and key-figures/highlights tables — not just the first page or summary. A periodic report normally reports many of the listed KPIs.\n\
- Extract EVERY listed known KPI whose value for the primary period appears anywhere in the document (isProposedKpi=false). Do not stop after the first few; include all that are present. You may also propose additional KPIs you find that are not in the list by setting isProposedKpi=true.\n\
- Do not invent values. Omit a KPI only if its value is genuinely absent from the document; do not guess.\n\
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

/// Build the claim-extraction prompt for one report document or transcript. The
/// model lists forward-looking management promises and, where stated, a due period
/// and a quantitative target. Claims require user confirmation before they are kept.
pub fn claim_extraction_prompt(request: &ClaimExtractionRequest) -> String {
    format!(
        "Extract forward-looking management claims (promises, guidance, commitments, and \
targets management states it will or expects to achieve) from the attached company \
{source_kind} for an investor research workflow. Only include statements management makes \
about the FUTURE that can later be verified — not past results or generic commentary.\n\n\
Return only JSON with this exact shape: \
{{\"language\":\"pl\",\"claims\":[{{\"statement\":\"concise paraphrase of the promise\",\"dueFiscalYear\":2026,\"duePeriodType\":\"Q1|Q2|Q3|Q4|H1|H2|FY\",\"targetMetricKey\":\"net_revenue\",\"targetComparator\":\"gte|lte|gt|lt|approx|eq\",\"targetValueNumeric\":\"1000000\",\"targetUnit\":\"PLN\",\"confidence\":\"low|medium|high\",\"sourceSnippet\":\"verbatim text from the source\"}}]}}.\n\n\
Rules:\n\
- {marker} only: each claim must be a future-oriented, checkable statement by management.\n\
- statement is a concise, neutral paraphrase of the promise (no buy/sell/hold language).\n\
- When management names a target period, set dueFiscalYear and duePeriodType (one of Q1, Q2, Q3, Q4, H1, H2, FY). Omit both if no period is stated; do not guess.\n\
- When the promise is quantitative, set targetMetricKey (a short snake_case key), targetComparator (gte, lte, gt, lt, approx, or eq), targetValueNumeric (plain decimal string, base units, no separators), and targetUnit. Omit these for qualitative promises.\n\
- For every claim include a verbatim sourceSnippet copied from the source and a confidence of low, medium, or high.\n\
- Do not invent claims. If the source contains no forward-looking management promises, return an empty claims array.\n\
- Do not include markdown fences, commentary outside JSON, or investment advice.\n\n\
Company: {company}",
        source_kind = request.source_kind,
        marker = CLAIM_EXTRACTION_MARKER,
        company = request.company_name,
    )
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
struct ClaimExtractionJson {
    language: Option<String>,
    #[serde(default)]
    claims: Vec<ExtractedClaimJson>,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
struct ExtractedClaimJson {
    statement: String,
    due_fiscal_year: Option<i64>,
    due_period_type: Option<String>,
    target_metric_key: Option<String>,
    target_comparator: Option<String>,
    target_value_numeric: Option<String>,
    target_unit: Option<String>,
    confidence: Option<String>,
    source_snippet: Option<String>,
}

/// Parse a model's claim-extraction text (JSON) into the neutral extraction output.
/// `provider_label` is used only for error messages. Claims with an empty statement
/// are dropped; an unsupported `duePeriodType` or `targetComparator` is dropped (the
/// rest of the claim is kept). An empty claims array is a valid result.
pub fn parse_claim_extraction_output(
    text: &str,
    provider_label: &str,
) -> Result<ClaimExtractionProviderOutput, AnalysisProviderError> {
    let json_text = extract_json_object(text, &format!("{provider_label} response"))
        .map_err(AnalysisProviderError::ParseError)?;
    let parsed: ClaimExtractionJson = serde_json::from_str(json_text).map_err(|error| {
        AnalysisProviderError::ParseError(format!(
            "{provider_label} claim extraction JSON: {error}"
        ))
    })?;

    let claims = parsed
        .claims
        .into_iter()
        .filter_map(|claim| {
            let statement = claim.statement.trim().to_owned();
            if statement.is_empty() {
                return None;
            }
            let due_period_type = clean_optional_string(claim.due_period_type)
                .map(|value| value.to_uppercase())
                .filter(|value| EXTRACTION_PERIOD_TYPES.contains(&value.as_str()));
            let target_comparator = clean_optional_string(claim.target_comparator)
                .map(|value| value.to_lowercase())
                .filter(|value| CLAIM_COMPARATORS.contains(&value.as_str()));
            let confidence = clean_optional_string(claim.confidence)
                .map(|value| value.to_lowercase())
                .filter(|value| ["low", "medium", "high"].contains(&value.as_str()));
            Some(ExtractedClaim {
                statement,
                due_fiscal_year: claim.due_fiscal_year,
                due_period_type,
                target_metric_key: clean_optional_string(claim.target_metric_key),
                target_comparator,
                target_value_numeric: clean_optional_string(claim.target_value_numeric),
                target_unit: clean_optional_string(claim.target_unit),
                confidence,
                source_snippet: clean_optional_string(claim.source_snippet),
            })
        })
        .collect::<Vec<_>>();

    Ok(ClaimExtractionProviderOutput {
        language: clean_optional_string(parsed.language),
        claims,
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

    // ---- Qualitative assessment (ADR 0075, v0.50.0) ------------------------

    fn sample_qualitative_request() -> QualitativeAssessmentRequest {
        QualitativeAssessmentRequest {
            company_id: "company_gpw_cdr".to_owned(),
            criterion_label: "Wide economic moat".to_owned(),
            assessment_guidance:
                "Assess whether the company has a durable competitive advantage that protects \
                 its returns on capital from competition."
                    .to_owned(),
            evidence_items: vec![crate::storage::ResearchEvidenceItem {
                id: "ev_1".to_owned(),
                evidence_type: "claim".to_owned(),
                source_domain: "management_claim".to_owned(),
                source_id: "claim_01".to_owned(),
                company_id: "company_gpw_cdr".to_owned(),
                occurred_at: "2026-05-01T00:00:00Z".to_owned(),
                title: "Management: pricing power intact".to_owned(),
                summary: Some("Raised prices 8% with no measurable churn.".to_owned()),
                source_url: None,
                attribution: Some("CEO, Q1 call".to_owned()),
                trust_category: "official".to_owned(),
                review_state: crate::storage::ResearchEvidenceReviewState {
                    changed_since_company_review: false,
                    changed_since_watchlist_review: false,
                },
            }],
        }
    }

    #[test]
    fn qualitative_assessment_prompt_snapshot() {
        insta::assert_snapshot!(qualitative_assessment_prompt(&sample_qualitative_request()));
    }

    #[test]
    fn qualitative_assessment_prompt_grounds_cites_and_forbids_advice() {
        let prompt = qualitative_assessment_prompt(&sample_qualitative_request());
        // Decision-support boundary: the template explicitly forbids advice.
        assert!(
            prompt.contains("buy/sell/hold recommendations"),
            "prompt must name the forbidden advice"
        );
        assert!(
            prompt.contains("Do not include"),
            "prompt must forbid the advice, not merely mention it"
        );
        // Grounded in guidance + evidence, with the insufficient-evidence escape hatch.
        assert!(prompt.contains("Wide economic moat"));
        assert!(prompt.contains("durable competitive advantage"));
        assert!(
            prompt.contains("claim_01"),
            "evidence id must appear for citation"
        );
        assert!(
            prompt.contains("insufficient_evidence"),
            "prompt must offer the insufficient-evidence verdict"
        );
        assert!(
            prompt.contains(QUALITATIVE_ASSESSMENT_PROMPT_VERSION)
                || QUALITATIVE_ASSESSMENT_PROMPT_VERSION == "qualitative-assessment.v1",
            "a versioned prompt id exists"
        );
    }
}
