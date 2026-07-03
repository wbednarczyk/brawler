//! Durable-queue job: agent-assessed qualitative quality-framework criteria
//! (v0.50.0, [ADR 0075](../../docs/adr/0075-qualitative-assessment-frameworks.md)).
//!
//! One request **per criterion per company** through the `QualitativeAssessment`
//! capability pool (ADR 0060), grounded ONLY in app-held evidence gathered via the
//! research-evidence boundary — no web access. Every stored result is agent opinion
//! (`source = agent`), cited, regeneratable, and it never mutates quantitative data.
//! Citations that do not resolve to supplied evidence are rejected (uncited
//! reasoning is never stored); no configured provider fails the job with a clear
//! error rather than degrading (matches feed-analysis behavior).

use std::collections::HashSet;

use crate::{
    app_state::AppState,
    providers::analysis::{
        capabilities::AiCapability, parse_qualitative_assessment_output,
        qualitative_assessment_prompt, AnalysisDocument, QualitativeAssessmentOutput,
        QualitativeAssessmentRequest, QUALITATIVE_ASSESSMENT_PROMPT_VERSION,
    },
    storage::{
        PersistQualitativeAssessmentInput, QualitativeCriterionResult, ResearchEvidenceInput,
        ResearchEvidenceItem,
    },
};

/// Durable-queue job kind for qualitative assessment (analysis worker lane).
pub const QUALITATIVE_ASSESSMENT_KIND: &str = "qualitative_assessment";

/// Company-scoped evidence types gathered for grounding (the research-evidence
/// boundary set the research-brief/digest collectors use). Report documents,
/// claims + verdicts, notebook notes, signals, and research questions all surface
/// here; the model is instructed to cite only what it is given.
const COMPANY_EVIDENCE_TYPES: [&str; 7] = [
    "feed_item",
    "notebook_entry",
    "claim",
    "company_event",
    "transcript_segment",
    "ai_analysis",
    "research_question",
];

/// Upper bound on evidence items per assessment request, to bound the prompt size
/// (an over-context bundle is an owner decision — ADR 0075 open item — not silently
/// truncated beyond this soft cap shared with the brief/digest collectors).
const EVIDENCE_LIMIT: i64 = 80;

/// The durable job payload. `criterion_ids` is the panel's single-criterion re-run
/// hook (T5 `rerun_qualitative_criterion`); absent ⇒ all qualitative criteria of
/// the framework are assessed.
#[derive(Debug, serde::Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct QualitativeAssessmentJobPayload {
    pub company_id: String,
    pub framework_id: String,
    #[serde(default)]
    pub criterion_ids: Option<Vec<String>>,
}

/// Run one qualitative-assessment job: gather evidence, assess each in-scope
/// qualitative criterion through the capability pool, validate citations, and
/// persist the batch as one immutable `source = agent` snapshot. Returns `Err`
/// (queue retry) on any provider/parse/citation failure — nothing is stored
/// unless every criterion produced a valid, cited result.
pub fn run_qualitative_assessment_job(state: &AppState, payload: &str) -> Result<(), String> {
    let input: QualitativeAssessmentJobPayload = serde_json::from_str(payload)
        .map_err(|error| format!("invalid qualitative_assessment payload: {error}"))?;

    let framework = state
        .get_quality_framework(&input.framework_id)
        .map_err(|error| error.to_string())?;
    let criteria: Vec<_> = framework
        .criteria
        .iter()
        .filter(|criterion| criterion.kind == "qualitative")
        .filter(|criterion| {
            input
                .criterion_ids
                .as_ref()
                .is_none_or(|ids| ids.iter().any(|id| id == &criterion.id))
        })
        .collect();
    if criteria.is_empty() {
        // Nothing to assess is a clean no-op, not a failure: an all-quantitative
        // framework, or a T5 single-criterion re-run whose criterion was since
        // deleted, must not burn the queue's retry budget on a condition that can
        // never become true on retry (mirrors signal_classification's nothing-to-do).
        log::info!(
            "module=qualitative_assessment stage=noop reason=no_qualitative_criteria frameworkId={}",
            input.framework_id
        );
        return Ok(());
    }

    let evidence = gather_company_evidence(state, &input.company_id)?;
    // The supplied evidence is identical for every criterion: build the citation
    // lookup once (not once per criterion), and clone the evidence into the reused
    // request a single time rather than N times inside the loop.
    let supplied: HashSet<(&str, &str)> = evidence
        .iter()
        .map(|item| (item.evidence_type.as_str(), item.source_id.as_str()))
        .collect();

    let timeout_seconds = state
        .get_settings()
        .map_err(|error| error.to_string())?
        .ai_providers
        .general_analysis_timeout_seconds;
    // No provider configured is a hard, clear error — never a silent degrade.
    let provider = crate::jobs::build_capability_provider(
        state,
        AiCapability::QualitativeAssessment,
        timeout_seconds,
    )?;

    let mut results = Vec::with_capacity(criteria.len());
    let mut request = QualitativeAssessmentRequest {
        company_id: input.company_id.clone(),
        criterion_label: String::new(),
        assessment_guidance: String::new(),
        evidence_items: evidence.clone(),
    };
    for criterion in criteria {
        request.criterion_label = criterion.label.clone();
        request.assessment_guidance = criterion.assessment_guidance.clone().unwrap_or_default();
        let prompt = qualitative_assessment_prompt(&request);
        // The prompt is self-contained (evidence embedded); the document transport
        // carries no extra payload — the capability is text-in/JSON-out.
        let text = tauri::async_runtime::block_on(provider.complete_document(
            &prompt,
            &AnalysisDocument::Text {
                text: String::new(),
            },
        ))
        .map_err(|error| error.to_string())?;

        let output = parse_qualitative_assessment_output(&text, provider.provider_id())
            .map_err(|error| error.to_string())?;
        let citations_json = validate_and_serialize_citations(&output, &supplied)?;

        results.push(QualitativeCriterionResult {
            criterion_id: criterion.id.clone(),
            ordinal: criterion.ordinal,
            label: criterion.label.clone(),
            verdict: output.verdict,
            reasoning: output.reasoning,
            citations_json,
            confidence: output.confidence,
            prompt_version: QUALITATIVE_ASSESSMENT_PROMPT_VERSION.to_owned(),
        });
    }

    let assessed = results.len();
    let evaluation = state
        .persist_qualitative_assessment(PersistQualitativeAssessmentInput {
            framework_id: input.framework_id,
            company_id: input.company_id,
            results,
        })
        .map_err(|error| error.to_string())?;

    log::info!(
        "module=qualitative_assessment stage=done providerId={} frameworkId={} companyId={} criteria={}",
        provider.provider_id(),
        evaluation.framework_id,
        evaluation.company_id,
        assessed,
    );

    Ok(())
}

fn gather_company_evidence(
    state: &AppState,
    company_id: &str,
) -> Result<Vec<ResearchEvidenceItem>, String> {
    let timeline = state
        .list_research_evidence(ResearchEvidenceInput {
            company_id: Some(company_id.to_owned()),
            watchlist_id: None,
            evidence_types: Some(
                COMPANY_EVIDENCE_TYPES
                    .iter()
                    .map(|value| (*value).to_owned())
                    .collect(),
            ),
            changed_since_review_only: None,
            limit: Some(EVIDENCE_LIMIT),
        })
        .map_err(|error| error.to_string())?;
    Ok(timeline.items)
}

/// Reject any citation that does not resolve to a supplied evidence id (ADR 0075
/// guardrail — the research-brief `rejects_unknown_citation_keys` precedent):
/// uncited reasoning is never stored. A `pass`/`partial`/`fail` verdict must cite
/// at least one evidence key; `insufficient_evidence` may cite none. Returns the
/// serialized citation array on success.
fn validate_and_serialize_citations(
    output: &QualitativeAssessmentOutput,
    supplied: &HashSet<(&str, &str)>,
) -> Result<String, String> {
    for citation in &output.citations {
        if !supplied.contains(&(
            citation.evidence_type.as_str(),
            citation.evidence_id.as_str(),
        )) {
            return Err(format!(
                "qualitative assessment citation {} references evidence {}/{} that was not supplied",
                citation.citation_key, citation.evidence_type, citation.evidence_id
            ));
        }
    }
    if output.verdict != "insufficient_evidence" && output.citations.is_empty() {
        return Err(format!(
            "qualitative assessment verdict '{}' requires at least one citation",
            output.verdict
        ));
    }
    serde_json::to_string(&output.citations)
        .map_err(|error| format!("failed to serialize qualitative citations: {error}"))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::providers::analysis::{
        capabilities::AiCapability, QualitativeAssessmentCitation, TEST_SAMPLE_ANALYSIS_MODEL,
        TEST_SAMPLE_ANALYSIS_PROVIDER_ID,
    };
    use crate::storage::{
        open_in_memory_database, AppState, ListFrameworkEvaluationsInput, NewCompany,
        NewFrameworkCriterion, NewQualityFramework, NewResearchQuestion,
        ResearchEvidenceReviewState,
    };

    /// Route the qualitative-assessment capability to the credential-less
    /// test-sample provider by seeding the `capability_providers` settings row
    /// directly — `update_settings` would reject `test_sample` as not
    /// user-selectable (the same bypass the KPI-extraction sentinel test uses).
    fn state_with_test_provider() -> AppState {
        let connection = open_in_memory_database().expect("database should initialize");
        let capability_providers_json = serde_json::json!({
            AiCapability::QualitativeAssessment.key(): [
                { "provider": TEST_SAMPLE_ANALYSIS_PROVIDER_ID, "model": TEST_SAMPLE_ANALYSIS_MODEL },
            ]
        })
        .to_string();
        connection
            .execute(
                "INSERT INTO settings (key, value, value_type) VALUES ('capability_providers', ?1, 'json') \
                 ON CONFLICT(key) DO UPDATE SET value = excluded.value",
                [capability_providers_json],
            )
            .expect("seed capability_providers bypassing validation");
        AppState::new(connection)
    }

    fn tracked_company(state: &AppState) -> String {
        state
            .create_company(NewCompany {
                exchange: "GPW".to_owned(),
                ticker: "QA".to_owned(),
                display_name: "Qualitative Assessment Test Co.".to_owned(),
                isin: Some("PLQA00000000".to_owned()),
                cik: None,
                lei: None,
            })
            .expect("company should create")
            .id
    }

    fn framework_with_qualitative_criterion(state: &AppState) -> (String, String) {
        let framework = state
            .create_quality_framework(NewQualityFramework {
                name: "Qual Framework".to_owned(),
                description: None,
            })
            .expect("framework should create");
        let criterion = state
            .create_framework_criterion(NewFrameworkCriterion {
                framework_id: framework.id.clone(),
                label: "Wide economic moat".to_owned(),
                kind: Some("qualitative".to_owned()),
                assessment_guidance: Some(
                    "Assess whether the company has a durable competitive advantage.".to_owned(),
                ),
                ..Default::default()
            })
            .expect("qualitative criterion should create");
        (framework.id, criterion.id)
    }

    /// Seed `count` company-scoped research questions — each surfaces as one
    /// `research_question` evidence item in the timeline.
    fn seed_evidence(state: &AppState, company_id: &str, count: usize) {
        for index in 0..count {
            state
                .create_research_question(NewResearchQuestion {
                    scope_type: "company".to_owned(),
                    scope_id: company_id.to_owned(),
                    title: format!("Research question {index}"),
                    body: Some("Body of the seeded research question.".to_owned()),
                })
                .expect("research question should create");
        }
    }

    fn run_job(state: &AppState, company_id: &str, framework_id: &str) {
        let payload = serde_json::json!({
            "companyId": company_id,
            "frameworkId": framework_id,
        })
        .to_string();
        state
            .jobs()
            .enqueue("qa-job", QUALITATIVE_ASSESSMENT_KIND, &payload, 1)
            .expect("enqueue");
        let worker = crate::jobs::handlers::build_worker(state.clone());
        worker.run_until_idle().expect("drain the queue");
    }

    fn latest_agent_results(
        state: &AppState,
        framework_id: &str,
        company_id: &str,
    ) -> Vec<crate::storage::CriterionResult> {
        state
            .list_framework_evaluations(ListFrameworkEvaluationsInput {
                framework_id: framework_id.to_owned(),
                company_id: company_id.to_owned(),
            })
            .expect("evaluations should list")
            .into_iter()
            .next()
            .expect("a snapshot should exist")
            .results
    }

    #[test]
    fn assesses_with_evidence_and_stores_a_cited_agent_verdict() {
        let state = state_with_test_provider();
        let company_id = tracked_company(&state);
        let (framework_id, criterion_id) = framework_with_qualitative_criterion(&state);
        seed_evidence(&state, &company_id, 2);

        run_job(&state, &company_id, &framework_id);

        let counts = state.jobs().counts().expect("counts");
        assert_eq!(counts.succeeded, 1, "the job ran to success");
        assert_eq!(counts.failed, 0);

        let results = latest_agent_results(&state, &framework_id, &company_id);
        assert_eq!(results.len(), 1);
        let result = &results[0];
        assert_eq!(result.source, "agent");
        assert_eq!(result.criterion_id.as_deref(), Some(criterion_id.as_str()));
        assert_eq!(result.verdict, "pass");
        assert_eq!(result.confidence.as_deref(), Some("high"));
        assert_eq!(
            result.prompt_version.as_deref(),
            Some(QUALITATIVE_ASSESSMENT_PROMPT_VERSION)
        );
        let citations: Vec<QualitativeAssessmentCitation> =
            serde_json::from_str(result.citations.as_deref().expect("citations stored"))
                .expect("citations parse");
        assert_eq!(citations.len(), 1, "a cited response was stored");
        assert_eq!(citations[0].evidence_type, "research_question");
    }

    #[test]
    fn records_a_low_confidence_verdict_when_evidence_is_thin() {
        let state = state_with_test_provider();
        let company_id = tracked_company(&state);
        let (framework_id, _) = framework_with_qualitative_criterion(&state);
        // A single evidence item → the sample provider returns a low-confidence partial.
        seed_evidence(&state, &company_id, 1);

        run_job(&state, &company_id, &framework_id);

        assert_eq!(state.jobs().counts().expect("counts").succeeded, 1);
        let results = latest_agent_results(&state, &framework_id, &company_id);
        assert_eq!(results[0].verdict, "partial");
        assert_eq!(results[0].confidence.as_deref(), Some("low"));
        assert_eq!(results[0].source, "agent");
    }

    #[test]
    fn records_insufficient_evidence_when_no_evidence_exists() {
        let state = state_with_test_provider();
        let company_id = tracked_company(&state);
        let (framework_id, _) = framework_with_qualitative_criterion(&state);
        // No evidence seeded: the honest verdict is insufficient_evidence, no citations.

        run_job(&state, &company_id, &framework_id);

        assert_eq!(state.jobs().counts().expect("counts").succeeded, 1);
        assert_eq!(state.jobs().counts().expect("counts").failed, 0);
        let results = latest_agent_results(&state, &framework_id, &company_id);
        assert_eq!(results[0].verdict, "insufficient_evidence");
        assert_eq!(results[0].source, "agent");
        let citations: Vec<QualitativeAssessmentCitation> =
            serde_json::from_str(results[0].citations.as_deref().expect("citations stored"))
                .expect("citations parse");
        assert!(
            citations.is_empty(),
            "insufficient_evidence carries no citations"
        );
    }

    #[test]
    fn framework_without_qualitative_criteria_is_a_clean_noop() {
        // Nothing to assess must succeed as a no-op (not a failure that the queue
        // retries to terminal): no snapshot is written, no provider is even called.
        let state = state_with_test_provider();
        let company_id = tracked_company(&state);
        let framework = state
            .create_quality_framework(NewQualityFramework {
                name: "No qualitative criteria".to_owned(),
                description: None,
            })
            .expect("framework should create");
        seed_evidence(&state, &company_id, 2);

        run_job(&state, &company_id, &framework.id);

        let counts = state.jobs().counts().expect("counts");
        assert_eq!(
            counts.succeeded, 1,
            "a nothing-to-assess run is a clean no-op"
        );
        assert_eq!(counts.failed, 0);
        assert!(
            latest_snapshot_absent(&state, &framework.id, &company_id),
            "no snapshot is written when there is nothing to assess"
        );
    }

    #[test]
    fn fails_cleanly_with_no_provider_configured() {
        // No general provider and no capability mapping → the job fails with a
        // clear error rather than degrading (ADR 0075).
        let state = AppState::new(open_in_memory_database().expect("db"));
        let company_id = tracked_company(&state);
        let (framework_id, _) = framework_with_qualitative_criterion(&state);
        seed_evidence(&state, &company_id, 2);

        let payload =
            serde_json::json!({ "companyId": company_id, "frameworkId": framework_id }).to_string();
        let error = run_qualitative_assessment_job(&state, &payload)
            .expect_err("no provider must fail the job");
        assert!(
            error.contains("no usable AI provider"),
            "error should name the missing provider: {error}"
        );
        // Nothing persisted.
        assert!(latest_snapshot_absent(&state, &framework_id, &company_id));
    }

    fn latest_snapshot_absent(state: &AppState, framework_id: &str, company_id: &str) -> bool {
        state
            .list_framework_evaluations(ListFrameworkEvaluationsInput {
                framework_id: framework_id.to_owned(),
                company_id: company_id.to_owned(),
            })
            .expect("evaluations should list")
            .is_empty()
    }

    /// The `(evidenceType, evidenceId)` lookup the job builds once per run and
    /// passes to `validate_and_serialize_citations`.
    fn supplied_set(items: &[ResearchEvidenceItem]) -> HashSet<(&str, &str)> {
        items
            .iter()
            .map(|item| (item.evidence_type.as_str(), item.source_id.as_str()))
            .collect()
    }

    fn sample_evidence_item(evidence_type: &str, source_id: &str) -> ResearchEvidenceItem {
        ResearchEvidenceItem {
            id: format!("ev_{source_id}"),
            evidence_type: evidence_type.to_owned(),
            source_domain: "management_claim".to_owned(),
            source_id: source_id.to_owned(),
            company_id: "company_gpw_qa".to_owned(),
            occurred_at: "2026-05-01T00:00:00Z".to_owned(),
            title: "Sample".to_owned(),
            summary: None,
            source_url: None,
            attribution: None,
            trust_category: "official".to_owned(),
            review_state: ResearchEvidenceReviewState {
                changed_since_company_review: false,
                changed_since_watchlist_review: false,
            },
        }
    }

    #[test]
    fn rejects_a_citation_not_referencing_supplied_evidence() {
        // ADR 0075 guardrail: a citation to an evidence id that was not supplied
        // is rejected — uncited/hallucinated reasoning is never stored.
        let evidence = vec![sample_evidence_item("claim", "claim_01")];
        let output = QualitativeAssessmentOutput {
            verdict: "pass".to_owned(),
            reasoning: "Grounded claim.".to_owned(),
            confidence: "high".to_owned(),
            citations: vec![QualitativeAssessmentCitation {
                citation_key: "E1".to_owned(),
                evidence_type: "claim".to_owned(),
                evidence_id: "ghost_99".to_owned(),
                label: None,
                snippet: None,
            }],
        };
        let error = validate_and_serialize_citations(&output, &supplied_set(&evidence))
            .expect_err("an uncited reference must be rejected");
        assert!(error.contains("was not supplied"), "error: {error}");
    }

    #[test]
    fn rejects_a_non_insufficient_verdict_with_no_citations() {
        let evidence = vec![sample_evidence_item("claim", "claim_01")];
        let output = QualitativeAssessmentOutput {
            verdict: "pass".to_owned(),
            reasoning: "Unsupported claim.".to_owned(),
            confidence: "high".to_owned(),
            citations: vec![],
        };
        let error = validate_and_serialize_citations(&output, &supplied_set(&evidence))
            .expect_err("a pass verdict must cite at least one evidence key");
        assert!(
            error.contains("requires at least one citation"),
            "error: {error}"
        );
    }

    #[test]
    fn accepts_a_supplied_citation() {
        let evidence = vec![sample_evidence_item("claim", "claim_01")];
        let output = QualitativeAssessmentOutput {
            verdict: "pass".to_owned(),
            reasoning: "Grounded claim.".to_owned(),
            confidence: "high".to_owned(),
            citations: vec![QualitativeAssessmentCitation {
                citation_key: "E1".to_owned(),
                evidence_type: "claim".to_owned(),
                evidence_id: "claim_01".to_owned(),
                label: Some("Pricing power".to_owned()),
                snippet: None,
            }],
        };
        let json = validate_and_serialize_citations(&output, &supplied_set(&evidence))
            .expect("a supplied citation validates");
        assert!(json.contains("claim_01"));
    }
}
