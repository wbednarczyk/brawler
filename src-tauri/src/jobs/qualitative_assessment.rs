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

use std::collections::{HashMap, HashSet};

use crate::{
    app_state::AppState,
    providers::analysis::{
        capabilities::AiCapability, parse_qualitative_assessment_output,
        qualitative_assessment_prompt, AnalysisDocument, QualitativeAssessmentOutput,
        QualitativeAssessmentRequest, QUALITATIVE_ASSESSMENT_PROMPT_VERSION,
    },
    report_diff::{
        classify_statement,
        extraction::{extract_report, SourceFormat},
        period_sort_key,
    },
    storage::{
        PersistQualitativeAssessmentInput, QualitativeCriterionResult, ReportDocument,
        ResearchEvidenceInput, ResearchEvidenceItem, ResearchEvidenceReviewState,
    },
};

/// Durable-queue job kind for qualitative assessment (analysis worker lane).
pub const QUALITATIVE_ASSESSMENT_KIND: &str = "qualitative_assessment";

/// Company-scoped evidence types gathered from the research-evidence timeline (the
/// boundary set the research-brief/digest collectors use): claims + verdicts,
/// notebook notes, typed signals surfaced as `company_event`, transcript segments,
/// prior AI analyses, and research questions. Feed items include `official_report`
/// disclosures — but only their title + truncated summary, NOT the stored report
/// TEXT. The latest stored report documents are added separately as first-class,
/// citable evidence carrying the extracted report text (ADR 0075 Decision 3 — see
/// `gather_report_documents`); the model is instructed to cite only what it is given.
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

/// Evidence-type marker for a stored report document surfaced as first-class,
/// citable evidence (ADR 0075 Decision 3). Distinct from an `official_report` feed
/// item — that carries only a title + truncated summary, whereas this item carries
/// the **extracted report text** the qualitative verdict must be grounded in. The
/// Rust citation path stores `evidenceType` as a free string, so introducing this
/// type needs no enum change; validation resolves against the supplied bundle.
const REPORT_DOCUMENT_EVIDENCE_TYPE: &str = "report_document";

/// Owner-ratified bundle-size bound (ADR 0075 open item): the extracted text of a
/// stored report document embedded per assessment is capped at this many characters,
/// with a truncation marker appended when cut. Keeps the two report documents from
/// dominating the prompt while still delivering the substantive report text.
const REPORT_EXCERPT_CHAR_LIMIT: usize = 12_000;

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

    // Diagnostics lifecycle (P1, owner T7): mirror the ai_analysis pattern so a
    // failing run is never silent — the panel poll and the Diagnostics view both
    // read this. Developer-mode gated inside `record_diagnostic_event`, so these
    // are no-ops in normal use. Scope id matches the durable job id core
    // (`<company>:<framework>`) the status read keys on.
    record_qualitative_diagnostic(
        state,
        &input.company_id,
        &input.framework_id,
        "started",
        "info",
        "Qualitative assessment job started.",
        serde_json::json!({
            "companyId": input.company_id,
            "frameworkId": input.framework_id,
            "criterionIds": input.criterion_ids,
        }),
    );

    match run_assessment_inner(state, &input) {
        Ok(assessed) => {
            record_qualitative_diagnostic(
                state,
                &input.company_id,
                &input.framework_id,
                "completed",
                "info",
                "Qualitative assessment job completed.",
                serde_json::json!({
                    "companyId": input.company_id,
                    "frameworkId": input.framework_id,
                    "criteriaAssessed": assessed,
                }),
            );
            Ok(())
        }
        Err(error) => {
            record_qualitative_diagnostic(
                state,
                &input.company_id,
                &input.framework_id,
                "failed",
                "error",
                "Qualitative assessment job failed.",
                serde_json::json!({
                    "companyId": input.company_id,
                    "frameworkId": input.framework_id,
                    "error": error,
                }),
            );
            Err(error)
        }
    }
}

/// Record one qualitative-assessment diagnostic event (developer-mode gated,
/// best-effort — mirrors `ai_analysis::record_ai_analysis_diagnostic`). The scope
/// id is `<company>:<framework>`, the same key the durable job id and the
/// `get_qualitative_assessment_status` read are built from.
fn record_qualitative_diagnostic(
    state: &AppState,
    company_id: &str,
    framework_id: &str,
    stage: &str,
    severity: &str,
    message: &str,
    metadata: serde_json::Value,
) {
    let _ = state.record_diagnostic_event(crate::storage::NewDiagnosticEvent {
        occurred_at: None,
        module: "qualitative_assessment".to_owned(),
        scope: Some(crate::storage::DiagnosticScope {
            scope_type: "qualitative_assessment_job".to_owned(),
            id: Some(format!("{company_id}:{framework_id}")),
        }),
        stage: stage.to_owned(),
        severity: severity.to_owned(),
        message: message.to_owned(),
        metadata: Some(metadata),
    });
}

/// The assessment work itself: gather evidence, assess each in-scope criterion,
/// validate citations, and persist one immutable snapshot. Returns the number of
/// criteria assessed (0 for a clean no-op). The public wrapper records the
/// diagnostics lifecycle around this.
fn run_assessment_inner(
    state: &AppState,
    input: &QualitativeAssessmentJobPayload,
) -> Result<usize, String> {
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
        return Ok(0);
    }

    let evidence = gather_company_evidence(state, &input.company_id)?;
    // The supplied evidence is identical for every criterion: build the citation
    // lookup once (not once per criterion), and clone the evidence into the reused
    // request a single time rather than N times inside the loop. The reference
    // rule itself has one home: `storage::supplied_evidence_refs`.
    let supplied = crate::storage::supplied_evidence_refs(&evidence);

    let settings = state.get_settings().map_err(|error| error.to_string())?;
    let timeout_seconds = settings.ai_providers.general_analysis_timeout_seconds;
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
        // Prose output follows the app language; snippets stay verbatim.
        output_language: settings.locale.clone(),
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
            framework_id: input.framework_id.clone(),
            company_id: input.company_id.clone(),
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

    Ok(assessed)
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
    // Report documents lead the bundle: for a company whose main material is its
    // reports, the stored report text is the primary grounding — the research
    // timeline (claims, notes, signals, questions) follows.
    let mut items = gather_report_documents(state, company_id)?;
    items.extend(timeline.items);
    Ok(items)
}

/// Surface the latest stored report documents as first-class, citable evidence
/// carrying the extracted report text (ADR 0075 Decision 3). At most one document
/// per financial-statement type (consolidated + standalone) — the same
/// classification the autopilot detector groups on ([`classify_statement`]) — so
/// the bundle never exceeds two report documents (owner-ratified bound).
fn gather_report_documents(
    state: &AppState,
    company_id: &str,
) -> Result<Vec<ResearchEvidenceItem>, String> {
    let documents = state
        .list_report_documents_by_company(company_id)
        .map_err(|error| error.to_string())?;
    Ok(newest_statement_documents(documents)
        .into_iter()
        .map(|document| report_document_evidence_item(state, document))
        .collect())
}

/// Keep the newest fetched financial-statement document per statement type — the
/// evidence-side analogue of the autopilot detector's `newest_periodic_reports_per_type`,
/// reusing the shared [`classify_statement`] helper. Only `fetched` documents carry
/// a stored file whose text can be extracted; non-statement filings and unfetched
/// documents are skipped. Recency is the report's domain period ([`period_sort_key`],
/// derived from the title/URL — not ingestion order), tie-broken by `created_at`.
fn newest_statement_documents(documents: Vec<ReportDocument>) -> Vec<ReportDocument> {
    let mut newest: HashMap<String, ReportDocument> = HashMap::new();
    for document in documents {
        if document.fetch_status != "fetched" {
            continue;
        }
        let title = document.title.clone().unwrap_or_default();
        let Some(statement) = classify_statement(&title, &document.url) else {
            continue;
        };
        let key = statement.as_str().to_owned();
        match newest.get(&key) {
            Some(current) if !is_newer_report(current, &document) => {}
            _ => {
                newest.insert(key, document);
            }
        }
    }
    newest.into_values().collect()
}

/// Whether `candidate` is a newer report than `current` for its statement type:
/// a later domain period wins, tie-broken by `created_at` (a stable last resort,
/// never the primary signal — a history backfill ingests old reports with a fresh
/// `created_at`).
fn is_newer_report(current: &ReportDocument, candidate: &ReportDocument) -> bool {
    report_recency_key(candidate) > report_recency_key(current)
}

fn report_recency_key(document: &ReportDocument) -> (i32, u8, String) {
    let (year, period) =
        period_sort_key(document.title.as_deref().unwrap_or(""), &document.url).unwrap_or((0, 0));
    (year, period, document.created_at.clone())
}

/// Build one citable evidence item from a stored report document: its extracted
/// text (capped, see [`extract_report_excerpt`]) rides in `summary`, which the
/// prompt's evidence block renders verbatim. `source_id` is the document id, so a
/// `report_document` citation resolves against the supplied bundle and validates.
fn report_document_evidence_item(
    state: &AppState,
    document: ReportDocument,
) -> ResearchEvidenceItem {
    let occurred_at = document
        .fetched_at
        .clone()
        .unwrap_or_else(|| document.created_at.clone());
    let title = document
        .title
        .clone()
        .unwrap_or_else(|| "Stored report document".to_owned());
    let summary = extract_report_excerpt(state, &document);
    ResearchEvidenceItem {
        id: format!("report_document:{}", document.id),
        evidence_type: REPORT_DOCUMENT_EVIDENCE_TYPE.to_owned(),
        source_domain: REPORT_DOCUMENT_EVIDENCE_TYPE.to_owned(),
        source_id: document.id.clone(),
        company_id: document.company_id.clone(),
        occurred_at,
        title,
        summary: Some(summary),
        source_url: Some(document.url.clone()),
        attribution: document.attribution.clone(),
        trust_category: "official".to_owned(),
        review_state: ResearchEvidenceReviewState {
            changed_since_company_review: false,
            changed_since_watchlist_review: false,
        },
    }
}

/// Extract the stored report document's text (bounded to [`REPORT_EXCERPT_CHAR_LIMIT`]).
/// On any unavailability — no local file, unreadable file, or no extractable text
/// layer (scanned/image PDF) — falls back to a short marker note rather than
/// failing the assessment, so the document still appears as citable evidence.
fn extract_report_excerpt(state: &AppState, document: &ReportDocument) -> String {
    let Some(local_path) = document.local_path.as_deref() else {
        return "[stored report document; no local file — text unavailable]".to_owned();
    };
    let path = state.data_dir().join(local_path);
    let bytes = match std::fs::read(&path) {
        Ok(bytes) => bytes,
        Err(error) => return format!("[stored report document; file unreadable: {error}]"),
    };
    let format = SourceFormat::resolve(document.content_type.as_deref(), &document.url);
    let outcome = extract_report(&bytes, format);
    let text = outcome
        .sections
        .iter()
        .map(|section| {
            if section.heading.trim().is_empty() {
                section.body.clone()
            } else {
                format!("{}\n{}", section.heading, section.body)
            }
        })
        .collect::<Vec<_>>()
        .join("\n\n");
    if text.trim().is_empty() {
        return format!(
            "[stored report document; no extractable text layer ({})]",
            outcome.state.as_str()
        );
    }
    truncate_excerpt(&text, REPORT_EXCERPT_CHAR_LIMIT)
}

/// Cap an excerpt at `limit` characters, appending a `[truncated]` marker when cut.
fn truncate_excerpt(text: &str, limit: usize) -> String {
    if text.chars().count() <= limit {
        return text.to_owned();
    }
    let head: String = text.chars().take(limit).collect();
    format!("{head} [truncated]")
}

/// Reject any citation that does not resolve to a supplied evidence id (ADR 0075
/// guardrail — the research-brief `rejects_unknown_citation_keys` precedent):
/// uncited reasoning is never stored. A `pass`/`partial`/`fail` verdict must cite
/// at least one evidence key; `insufficient_evidence` may cite none. Returns the
/// serialized citation array on success.
fn validate_and_serialize_citations(
    output: &QualitativeAssessmentOutput,
    supplied: &HashSet<(String, String)>,
) -> Result<String, String> {
    for citation in &output.citations {
        if !crate::storage::citation_resolves(
            supplied,
            &citation.evidence_type,
            &citation.evidence_id,
        ) {
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
        open_in_memory_database, AppState, CaptureReportDocumentInput,
        ListFrameworkEvaluationsInput, NewCompany, NewFrameworkCriterion, NewQualityFramework,
        NewResearchQuestion, ResearchEvidenceReviewState,
    };
    use std::path::Path;

    /// Seed the `capability_providers` settings row so the qualitative-assessment
    /// capability routes to the credential-less test-sample provider — `update_settings`
    /// would reject `test_sample` as not user-selectable (the same bypass the
    /// KPI-extraction sentinel test uses).
    fn seed_test_provider(connection: &rusqlite::Connection) {
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
    }

    fn state_with_test_provider() -> AppState {
        let connection = open_in_memory_database().expect("database should initialize");
        seed_test_provider(&connection);
        AppState::new(connection)
    }

    /// A test-provider state rooted at an explicit `data_dir`, so a stored report
    /// document's on-disk file resolves for text extraction.
    fn state_with_test_provider_at(data_dir: std::path::PathBuf) -> AppState {
        let connection = open_in_memory_database().expect("database should initialize");
        seed_test_provider(&connection);
        AppState::with_data_dir(connection, data_dir)
    }

    /// Build an ESEF-style `.xhtml` report body whose visible text repeats a
    /// recognizable moat phrase, sized above the extractor's text-layer threshold.
    fn report_html(repetitions: usize) -> String {
        let paragraph = "<p>The company sustains a wide economic moat through proprietary \
             technology and high customer switching costs that protect its returns on capital \
             from competition.</p>";
        let mut body = String::from("<html><body>");
        for _ in 0..repetitions {
            body.push_str(paragraph);
        }
        body.push_str("</body></html>");
        body
    }

    /// Capture + mark-fetched a consolidated financial-statement report document with
    /// an extractable text file on disk. Returns the document id.
    fn seed_report_document(
        state: &AppState,
        company_id: &str,
        dir: &Path,
        repetitions: usize,
    ) -> String {
        let document = state
            .create_or_find_pending_report_document(CaptureReportDocumentInput {
                company_id: company_id.to_owned(),
                source_type: "user_url".to_owned(),
                url: "https://example.com/emitent/2026-05/ssf-2026.xhtml".to_owned(),
                period_id: None,
                origin_ref: None,
                title: Some("Skonsolidowane sprawozdanie finansowe 2026".to_owned()),
                attribution: Some("Investor relations".to_owned()),
            })
            .expect("report document should be captured");
        let body = report_html(repetitions);
        std::fs::write(dir.join("ssf-2026.xhtml"), &body).expect("write sample report");
        state
            .mark_report_document_fetched(
                &document.id,
                Some("ssf-2026.xhtml"),
                Some("application/xhtml+xml"),
                None,
                Some(body.len() as i64),
            )
            .expect("mark report document fetched");
        document.id
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
    fn gathers_stored_report_document_text_as_citable_evidence() {
        // ADR 0075 Decision 3: the assessment must be grounded in stored report
        // documents. The latest fetched financial-statement document surfaces as a
        // first-class `report_document` evidence item carrying the extracted text.
        let dir =
            std::env::temp_dir().join(format!("brawler-qa-report-gather-{}", std::process::id()));
        std::fs::create_dir_all(&dir).expect("temp data dir");
        let state = state_with_test_provider_at(dir.clone());
        let company_id = tracked_company(&state);
        let document_id = seed_report_document(&state, &company_id, &dir, 40);

        let evidence = gather_company_evidence(&state, &company_id).expect("gather evidence");
        let report = evidence
            .iter()
            .find(|item| item.evidence_type == REPORT_DOCUMENT_EVIDENCE_TYPE)
            .expect("a report_document evidence item is present");
        assert_eq!(report.source_id, document_id, "cited by the document id");
        assert!(
            report
                .summary
                .as_deref()
                .unwrap_or_default()
                .contains("wide economic moat"),
            "the excerpt carries the extracted report text"
        );
    }

    #[test]
    fn assesses_a_report_only_company_and_cites_the_report() {
        // A company whose only material is a stored report no longer dead-ends at
        // insufficient_evidence: the report text grounds a cited verdict.
        let dir =
            std::env::temp_dir().join(format!("brawler-qa-report-cite-{}", std::process::id()));
        std::fs::create_dir_all(&dir).expect("temp data dir");
        let state = state_with_test_provider_at(dir.clone());
        let company_id = tracked_company(&state);
        let (framework_id, _) = framework_with_qualitative_criterion(&state);
        let document_id = seed_report_document(&state, &company_id, &dir, 40);

        run_job(&state, &company_id, &framework_id);

        let counts = state.jobs().counts().expect("counts");
        assert_eq!(counts.succeeded, 1, "the report-grounded job succeeds");
        assert_eq!(counts.failed, 0);

        let results = latest_agent_results(&state, &framework_id, &company_id);
        let citations: Vec<QualitativeAssessmentCitation> =
            serde_json::from_str(results[0].citations.as_deref().expect("citations stored"))
                .expect("citations parse");
        assert_eq!(
            citations[0].evidence_type, REPORT_DOCUMENT_EVIDENCE_TYPE,
            "the stored report document is a citable evidence type"
        );
        assert_eq!(
            citations[0].evidence_id, document_id,
            "the citation resolves to the supplied report document"
        );
    }

    #[test]
    fn report_excerpt_is_capped_and_marked_when_long() {
        // Owner-ratified bound: the embedded report text is capped and marked so a
        // long report cannot dominate the prompt.
        let dir =
            std::env::temp_dir().join(format!("brawler-qa-report-cap-{}", std::process::id()));
        std::fs::create_dir_all(&dir).expect("temp data dir");
        let state = state_with_test_provider_at(dir.clone());
        let company_id = tracked_company(&state);
        // ~160 visible chars/paragraph × 200 ≫ the 12k cap.
        seed_report_document(&state, &company_id, &dir, 200);

        let evidence = gather_company_evidence(&state, &company_id).expect("gather evidence");
        let excerpt = evidence
            .iter()
            .find(|item| item.evidence_type == REPORT_DOCUMENT_EVIDENCE_TYPE)
            .and_then(|item| item.summary.clone())
            .expect("report excerpt present");
        assert!(
            excerpt.ends_with("[truncated]"),
            "a truncated excerpt is marked"
        );
        assert!(
            excerpt.chars().count() <= REPORT_EXCERPT_CHAR_LIMIT + " [truncated]".chars().count(),
            "the excerpt is capped at the owner-ratified bound"
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
    fn records_a_failed_diagnostic_when_the_job_fails() {
        crate::providers::credentials::scrub_provider_env_fallbacks();
        // P1 (owner T7): a failing assessment job (no provider) must leave a
        // `failed` diagnostic event carrying the error string — mirroring the
        // ai_analysis lifecycle — not vanish silently (the panel showed a hint
        // that cleared with an empty Diagnostics view). Developer-mode gated, so
        // enable it first (same as the ai_analysis diagnostic test).
        let state = AppState::new(open_in_memory_database().expect("db"));
        state
            .set_developer_mode_enabled(true)
            .expect("developer mode should enable");
        let company_id = tracked_company(&state);
        let (framework_id, _) = framework_with_qualitative_criterion(&state);
        seed_evidence(&state, &company_id, 2);

        let payload =
            serde_json::json!({ "companyId": company_id, "frameworkId": framework_id }).to_string();
        let error = run_qualitative_assessment_job(&state, &payload)
            .expect_err("no provider must fail the job");

        let events = state
            .list_diagnostic_events(20)
            .expect("diagnostic events should list");
        let failed = events
            .iter()
            .find(|event| event.stage == "failed")
            .expect("a failed diagnostic event is recorded");
        assert_eq!(failed.module, "qualitative_assessment");
        assert_eq!(failed.severity, "error");
        assert_eq!(
            failed.scope.as_ref().map(|scope| scope.scope_type.as_str()),
            Some("qualitative_assessment_job")
        );
        assert!(
            failed
                .metadata
                .to_string()
                .contains("no usable AI provider"),
            "the failed diagnostic carries the backend error string: {}",
            failed.metadata
        );
        assert!(error.contains("no usable AI provider"));
    }

    #[test]
    fn records_a_completed_diagnostic_on_success() {
        let state = state_with_test_provider();
        state
            .set_developer_mode_enabled(true)
            .expect("developer mode should enable");
        let company_id = tracked_company(&state);
        let (framework_id, _) = framework_with_qualitative_criterion(&state);
        seed_evidence(&state, &company_id, 2);

        let payload =
            serde_json::json!({ "companyId": company_id, "frameworkId": framework_id }).to_string();
        run_qualitative_assessment_job(&state, &payload).expect("job should succeed");

        let stages = state
            .list_diagnostic_events(20)
            .expect("diagnostic events should list")
            .into_iter()
            .map(|event| event.stage)
            .collect::<HashSet<_>>();
        assert!(stages.contains("started"), "records a started stage");
        assert!(stages.contains("completed"), "records a completed stage");
    }

    #[test]
    fn fails_cleanly_with_no_provider_configured() {
        crate::providers::credentials::scrub_provider_env_fallbacks();
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
    /// passes to `validate_and_serialize_citations` — via the shared single-home
    /// builder, so these tests exercise the same reference rule production uses.
    fn supplied_set(items: &[ResearchEvidenceItem]) -> HashSet<(String, String)> {
        crate::storage::supplied_evidence_refs(items)
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
