//! Autonomous report pipeline orchestrator (North Star, v0.49.0, ADR 0055).
//!
//! The pipeline is **chained durable-queue jobs**: one `autopilot_stage` job per
//! stage (`fetch → extract → diff → cross_reference → notify`), each stamped with
//! the parent run id and enqueuing the next on success. Each stage **reuses the
//! existing service** (report fetch, KPI extraction, report diff, claims
//! cross-reference) — the orchestrator is thin glue, never a reimplementation. A
//! crash mid-stage resumes that stage only (the durable queue reclaims it). A
//! fatal stage failure finalizes the run as `failed` but still surfaces a
//! notification (no silent dead-end); the user can re-trigger.
//!
//! Detection ([`run_detection_sweep`]) is event-driven off source-refresh
//! completion: it scans companies opted into automation for newly-arrived
//! periodic reports and starts a run, idempotently (at most one run per
//! `(company, report document)`). Runs only while the app is open.

use serde::{Deserialize, Serialize};

use crate::app_state::AppState;
use crate::providers::analysis::{
    KPI_EXTRACTION_PROMPT_VERSION, TEST_SAMPLE_ANALYSIS_MODEL, TEST_SAMPLE_ANALYSIS_PROVIDER_ID,
};
use crate::storage::{self, MODE_AUTOPILOT};

/// Durable-queue job kind for one pipeline stage.
pub const AUTOPILOT_STAGE_KIND: &str = "autopilot_stage";

const STAGE_FETCH: &str = "fetch";
const STAGE_EXTRACT: &str = "extract";
const STAGE_DIFF: &str = "diff";
const STAGE_CROSS_REFERENCE: &str = "cross_reference";
const STAGE_NOTIFY: &str = "notify";

/// Payload for an `autopilot_stage` job: which run, which stage.
#[derive(Debug, Serialize, Deserialize)]
pub struct StagePayload {
    pub run_id: String,
    pub stage: String,
}

fn stage_job_id(run_id: &str, stage: &str) -> String {
    format!("autopilot:{run_id}:{stage}")
}

fn next_stage(stage: &str) -> Option<&'static str> {
    match stage {
        STAGE_FETCH => Some(STAGE_EXTRACT),
        STAGE_EXTRACT => Some(STAGE_DIFF),
        STAGE_DIFF => Some(STAGE_CROSS_REFERENCE),
        STAGE_CROSS_REFERENCE => Some(STAGE_NOTIFY),
        _ => None,
    }
}

/// Enqueue the first stage of a run onto the durable queue.
pub fn enqueue_first_stage(state: &AppState, run_id: &str) {
    enqueue_stage(state, run_id, STAGE_FETCH);
}

fn enqueue_stage(state: &AppState, run_id: &str, stage: &str) {
    let payload = serde_json::to_string(&StagePayload {
        run_id: run_id.to_owned(),
        stage: stage.to_owned(),
    })
    .unwrap_or_else(|_| "{}".to_owned());
    // max_attempts = 1: a stage failure is handled in `run_stage` (finalize the run
    // as failed, still notified) and returns Ok, so the queue never retries a stage.
    // Crash-resume still works — a `running` row is reclaimed and re-run on startup.
    // (Auto-retry of transient stage failures is a tracked enhancement, not wired.)
    if let Err(error) = state.jobs().enqueue(
        &stage_job_id(run_id, stage),
        AUTOPILOT_STAGE_KIND,
        &payload,
        1,
    ) {
        log::warn!("autopilot: failed to enqueue stage {stage} for run {run_id}: {error}");
    }
}

/// Run one pipeline stage (the `autopilot_stage` handler entry point). On success
/// enqueues the next stage; on a fatal domain failure finalizes the run as
/// `failed` (still notified). Returns `Err` only for an infra/serialization error
/// the queue should retry.
pub fn run_stage(state: &AppState, payload: &str) -> Result<(), String> {
    let payload: StagePayload = serde_json::from_str(payload).map_err(|e| e.to_string())?;
    let run = state
        .autopilot()
        .get_run(&payload.run_id)
        .map_err(|e| e.to_string())?;

    // Idempotent: a finalized run does no more work (e.g. a duplicate/reclaimed job).
    if matches!(run.status.as_str(), "succeeded" | "failed" | "partial") {
        return Ok(());
    }

    let _ = state
        .autopilot()
        .set_run_stage(&run.id, &payload.stage, "running");

    let outcome = match payload.stage.as_str() {
        STAGE_FETCH => stage_fetch(state, &run),
        STAGE_EXTRACT => stage_extract(state, &run),
        STAGE_DIFF => stage_diff(state, &run),
        STAGE_CROSS_REFERENCE => stage_cross_reference(state, &run),
        STAGE_NOTIFY => return finalize_notify(state, &run),
        other => Err(format!("unknown autopilot stage: {other}")),
    };

    match outcome {
        Ok(()) => {
            if let Some(next) = next_stage(&payload.stage) {
                enqueue_stage(state, &run.id, next);
            }
            Ok(())
        }
        Err(message) => {
            // Fatal for this run: finalize as failed, but still surface a
            // notification describing how far it got. No queue retry (the user can
            // re-trigger); returning Ok keeps the job from looping.
            let _ = state.autopilot().finalize_run(
                &run.id,
                "failed",
                &payload.stage,
                Some(&format!("Autopilot stopped at {} stage.", payload.stage)),
                Some(&message),
            );
            log::warn!(
                "autopilot run {} failed at {} stage: {message}",
                run.id,
                payload.stage
            );
            Ok(())
        }
    }
}

/// Stage 1 — ensure the detected report document's file is downloaded. Idempotent
/// (an already-fetched document is a no-op). Reuses the shared fetch path.
fn stage_fetch(state: &AppState, run: &storage::AutopilotRun) -> Result<(), String> {
    let fetcher = crate::document_fetcher::HttpDocumentFetcher::new();
    crate::report_documents_capture::fetch_report_document(
        state,
        &fetcher,
        &run.report_document_id,
    )?;
    Ok(())
}

/// Stage 2 — extract KPIs from the report. Reuses the AI KPI-extraction job. In
/// `autopilot` mode, auto-confirms each proposal as an `auto_unreviewed` fact
/// (cited, flagged, reversible); in `assist` mode the proposals stay `pending`
/// for the user to confirm. Degrades gracefully when no real AI provider is
/// configured — and, in `autopilot` mode, also when the configured provider is
/// the **test-sample** provider (its placeholder KPIs must never be auto-committed
/// as facts) — recording that extraction was unavailable and continuing to diff
/// (AI cost stays bounded: at most one extraction per detected report).
fn stage_extract(state: &AppState, run: &storage::AutopilotRun) -> Result<(), String> {
    let settings = state.get_settings().map_err(|e| e.to_string())?;
    let provider_id = settings
        .ai_providers
        .general_analysis_provider
        .as_deref()
        .map(str::trim)
        .filter(|value| !value.is_empty());

    let Some(provider_id) = provider_id else {
        // No AI configured: degrade rather than loop or use sample data.
        let delta = serde_json::json!({
            "extractionAvailable": false,
            "reason": "no_ai_provider",
        });
        let _ = state
            .autopilot()
            .set_kpi_delta_json(&run.id, &delta.to_string());
        return Ok(());
    };

    let provider_id = provider_id.to_owned();

    // Autopilot must never auto-commit facts from a non-real provider. The
    // test-sample analysis provider returns placeholder KPIs, so in `autopilot`
    // mode it would auto-confirm sample data as `auto_unreviewed` facts — treat it
    // as "no real provider" and degrade (ADR 0055; AGENTS.md: mocks are never
    // completion evidence). `assist` mode still runs: its proposals stay `pending`
    // and are user-gated, so no sample fact is ever committed without review.
    if run.mode == MODE_AUTOPILOT && provider_id == TEST_SAMPLE_ANALYSIS_PROVIDER_ID {
        let delta = serde_json::json!({
            "extractionAvailable": false,
            "reason": "test_sample_provider",
        });
        let _ = state
            .autopilot()
            .set_kpi_delta_json(&run.id, &delta.to_string());
        return Ok(());
    }

    let model = if provider_id == TEST_SAMPLE_ANALYSIS_PROVIDER_ID {
        TEST_SAMPLE_ANALYSIS_MODEL.to_owned()
    } else {
        settings.ai_providers.general_analysis_model.clone()
    };

    let job = state
        .create_kpi_extraction_job(storage::NewKpiExtractionJob {
            company_id: run.company_id.clone(),
            report_document_id: run.report_document_id.clone(),
            provider_id,
            model,
            prompt_version: KPI_EXTRACTION_PROMPT_VERSION.to_owned(),
            period_hint: None,
        })
        .map_err(|e| e.to_string())?;

    // We are already on the worker thread; run the extraction inline.
    crate::jobs::kpi_extraction::run_kpi_extraction_job(state, &job.id)?;
    let completed = state
        .get_kpi_extraction_job(&job.id)
        .map_err(|e| e.to_string())?;

    // A failed extraction must fail the stage — never record it as a silent
    // success. `run_kpi_extraction_job` returns `Ok` even when it marks the job
    // `failed` (e.g. a transient "Gemini service unavailable" 503), so counting
    // `proposals` here would record `proposed: 0` on a `succeeded` run and block
    // re-detection forever. Propagating the error lets the queue retry with
    // backoff and — once attempts are spent — dead-letter the run (ADR 0059).
    if completed.status == "failed" {
        return Err(format!(
            "KPI extraction failed: {}",
            completed.error.as_deref().unwrap_or("unknown error")
        ));
    }

    let proposed = completed.proposals.len();
    let mut confirmed_ids: Vec<String> = Vec::new();
    if run.mode == MODE_AUTOPILOT {
        for proposal in &completed.proposals {
            if proposal.status != "pending" {
                continue;
            }
            match state.autopilot_auto_confirm_proposal(&proposal.id) {
                Ok(fact) => confirmed_ids.push(fact.id),
                // A proposal the model couldn't place (missing period, etc.) stays
                // pending for review rather than failing the run.
                Err(error) => log::info!(
                    "autopilot run {}: proposal {} left for review: {error}",
                    run.id,
                    proposal.id
                ),
            }
        }
        if !confirmed_ids.is_empty() {
            state
                .autopilot()
                .add_produced_facts(&run.id, &confirmed_ids)
                .map_err(|e| e.to_string())?;
        }
    }

    let delta = serde_json::json!({
        "extractionAvailable": true,
        "proposed": proposed,
        "autoConfirmed": confirmed_ids.len(),
        "mode": run.mode,
    });
    let _ = state
        .autopilot()
        .set_kpi_delta_json(&run.id, &delta.to_string());
    Ok(())
}

/// Stage 3 — find the consecutive same-type statement to diff the new report
/// against and record the document pair. The diff itself is an on-demand read
/// model (ADR 0052); we store the reference so Today/Pulse can open it. A
/// first-ever report (no prior statement) is normal — no diff ref, not a failure.
fn stage_diff(state: &AppState, run: &storage::AutopilotRun) -> Result<(), String> {
    let pair = crate::commands::report_diff::diff_pair_for_newer(
        state,
        &run.company_id,
        &run.report_document_id,
    )?;
    if let Some((older_id, newer_id, statement_type)) = pair {
        let diff_ref = serde_json::json!({
            "olderReportDocumentId": older_id,
            "newerReportDocumentId": newer_id,
            "statementType": statement_type,
        });
        let _ = state
            .autopilot()
            .set_report_diff_ref(&run.id, &diff_ref.to_string());
    }
    Ok(())
}

/// Stage 4 — cross-reference the new report against open claims to verify and open
/// research questions for the company. Decision-support only: this reports what to
/// verify, never a judgment.
fn stage_cross_reference(state: &AppState, run: &storage::AutopilotRun) -> Result<(), String> {
    let claims = state
        .list_claims_to_verify(&run.company_id)
        .map_err(|e| e.to_string())?;
    let open_questions = state
        .list_research_questions(storage::ResearchQuestionListInput {
            scope_type: Some("company".to_owned()),
            scope_id: Some(run.company_id.clone()),
            status: Some("open".to_owned()),
        })
        .map(|q| q.len())
        .unwrap_or(0);

    let cross_refs = serde_json::json!({
        "claimsOverdue": claims.overdue.len(),
        "claimsDue": claims.due.len(),
        "openQuestions": open_questions,
    });
    let _ = state
        .autopilot()
        .set_cross_refs_json(&run.id, &cross_refs.to_string());
    Ok(())
}

/// Stage 5 — compose the single notification summary and finalize the run. The
/// notification stays `unread` for the Today/Pulse "what changed" surface.
fn finalize_notify(state: &AppState, run: &storage::AutopilotRun) -> Result<(), String> {
    // Re-read so the incrementally-written stage columns are present.
    let run = state
        .autopilot()
        .get_run(&run.id)
        .map_err(|e| e.to_string())?;
    let summary = compose_summary(&run);
    state
        .autopilot()
        .finalize_run(&run.id, "succeeded", STAGE_NOTIFY, Some(&summary), None)
        .map_err(|e| e.to_string())?;
    Ok(())
}

/// Build a concise, decision-support-framed "what changed" line from the stage
/// outputs. Plain facts (counts + whether a diff is available), never advice.
fn compose_summary(run: &storage::AutopilotRun) -> String {
    let mut parts: Vec<String> = Vec::new();

    if let Some(delta) = run
        .kpi_delta_json
        .as_deref()
        .and_then(|j| serde_json::from_str::<serde_json::Value>(j).ok())
    {
        if delta.get("extractionAvailable").and_then(|v| v.as_bool()) == Some(false) {
            parts.push("KPI extraction unavailable (no AI provider configured)".to_owned());
        } else {
            let proposed = delta.get("proposed").and_then(|v| v.as_u64()).unwrap_or(0);
            let confirmed = delta
                .get("autoConfirmed")
                .and_then(|v| v.as_u64())
                .unwrap_or(0);
            if run.mode == MODE_AUTOPILOT {
                parts.push(format!(
                    "{confirmed} KPI auto-confirmed (unreviewed) of {proposed} extracted"
                ));
            } else {
                parts.push(format!(
                    "{proposed} KPI extracted, pending your confirmation"
                ));
            }
        }
    }

    if run.report_diff_ref.is_some() {
        parts.push("report diff vs the previous statement available".to_owned());
    }

    if let Some(refs) = run
        .cross_refs_json
        .as_deref()
        .and_then(|j| serde_json::from_str::<serde_json::Value>(j).ok())
    {
        let overdue = refs
            .get("claimsOverdue")
            .and_then(|v| v.as_u64())
            .unwrap_or(0);
        let due = refs.get("claimsDue").and_then(|v| v.as_u64()).unwrap_or(0);
        let questions = refs
            .get("openQuestions")
            .and_then(|v| v.as_u64())
            .unwrap_or(0);
        let to_verify = overdue + due;
        if to_verify > 0 {
            parts.push(format!("{to_verify} claim(s) to verify"));
        }
        if questions > 0 {
            parts.push(format!("{questions} open research question(s)"));
        }
    }

    if parts.is_empty() {
        "New report processed.".to_owned()
    } else {
        format!("New report processed — {}.", parts.join("; "))
    }
}

/// Detection sweep — event-driven off source-refresh completion. For every company
/// opted into automation, start an autopilot run for the newest periodic-report
/// document (per statement type) that does not yet have one. Idempotent: the
/// `(company, report document)` uniqueness guarantees at most one run per report,
/// and limiting to the newest per type avoids back-filling the whole history on
/// first opt-in. Best-effort: logs and continues on per-company errors.
pub fn run_detection_sweep(state: &AppState) {
    let company_ids = match state.autopilot().opted_in_company_ids() {
        Ok(ids) => ids,
        Err(error) => {
            log::warn!("autopilot detection: failed to list opted-in companies: {error}");
            return;
        }
    };

    for company_id in company_ids {
        let mode = match state.autopilot().get_mode(&company_id) {
            Ok(mode) => mode,
            Err(_) => continue,
        };
        if mode == storage::MODE_OFF {
            continue;
        }

        let documents = match state.list_report_documents_by_company(&company_id) {
            Ok(docs) => docs,
            Err(error) => {
                log::warn!("autopilot detection: list documents failed for {company_id}: {error}");
                continue;
            }
        };

        for document in newest_periodic_reports_per_type(documents) {
            let run_id = format!("autopilot_run:{company_id}:{}", document.id);
            match state.autopilot().create_run_if_absent(
                &run_id,
                &company_id,
                &document.id,
                "detection",
                &mode,
            ) {
                Ok(Some(run)) => {
                    log::info!(
                        "autopilot: detected new report for {company_id}, starting run {}",
                        run.id
                    );
                    enqueue_first_stage(state, &run.id);
                }
                Ok(None) => {} // already has a run (dedup)
                Err(error) => {
                    log::warn!("autopilot detection: create run failed for {company_id}: {error}");
                }
            }
        }
    }
}

/// Keep the newest periodic-report (financial-statement) document per statement
/// type. A document is a periodic report when it classifies as a financial
/// statement and still carries a file (not `metadata_only`). "Newest" is the
/// report's **disclosure date** ([`report_disclosure_key`]), never `created_at`:
/// an on-track history backfill ingests old reports with a fresh `created_at`, so
/// ranking on insert order fires autopilot on a years-old report (`d60305c`).
fn newest_periodic_reports_per_type(
    documents: Vec<storage::ReportDocument>,
) -> Vec<storage::ReportDocument> {
    use std::collections::HashMap;
    let mut newest: HashMap<String, storage::ReportDocument> = HashMap::new();
    for document in documents {
        if document.fetch_status == "metadata_only" {
            continue;
        }
        let title = document.title.clone().unwrap_or_default();
        let Some(statement) = crate::report_diff::classify_statement(&title, &document.url) else {
            continue;
        };
        let key = statement.as_str().to_owned();
        match newest.get(&key) {
            Some(current) if report_disclosure_key(current) >= report_disclosure_key(&document) => {
            }
            _ => {
                newest.insert(key, document);
            }
        }
    }
    newest.into_values().collect()
}

/// A sortable **disclosure-date** key (`YYYY-MM-DD`) for ranking report recency —
/// the domain date, not `created_at`/ingestion order ([data-model.md] Model
/// Principles; guardrail `d60305c`). The accepted ESPI/EBI attachment sources embed
/// the disclosure month in the URL as `/emitent/YYYY-MM/`; use it (day `01`, which
/// is enough for the quarterly cadence detection ranks). Falls back to `fetched_at`,
/// then `created_at` only as a last resort (a non-`emitent`, never-fetched doc).
fn report_disclosure_key(document: &storage::ReportDocument) -> String {
    if let Some(month) = disclosure_month_from_url(&document.url) {
        return format!("{month}-01");
    }
    if let Some(fetched) = document.fetched_at.as_deref() {
        if fetched.len() >= 10 {
            return fetched[..10].to_owned();
        }
    }
    if document.created_at.len() >= 10 {
        return document.created_at[..10].to_owned();
    }
    document.created_at.clone()
}

/// Extract the disclosure month `YYYY-MM` from an ESPI/EBI attachment URL's
/// `/emitent/YYYY-MM/` segment (bonnier.pl and bankier.pl both use it). `None` for
/// any URL without that segment (e.g. an IR landing page).
fn disclosure_month_from_url(url: &str) -> Option<String> {
    const MARKER: &str = "/emitent/";
    let start = url.find(MARKER)? + MARKER.len();
    let rest = url.get(start..)?;
    let bytes = rest.as_bytes();
    // Expect exactly "YYYY-MM/".
    if bytes.len() < 8
        || !bytes[..4].iter().all(u8::is_ascii_digit)
        || bytes[4] != b'-'
        || !bytes[5..7].iter().all(u8::is_ascii_digit)
        || bytes[7] != b'/'
    {
        return None;
    }
    let month: u32 = rest[5..7].parse().ok()?;
    if !(1..=12).contains(&month) {
        return None;
    }
    Some(rest[..7].to_owned())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::storage::{open_in_memory_database, NewCompany};

    /// Guardrail (issue a3643d7): in `autopilot` mode the test-sample analysis
    /// provider must be treated as "no real provider" — extraction degrades and
    /// nothing is auto-confirmed, so sample/placeholder KPIs never land as
    /// `auto_unreviewed` facts (ADR 0055; AGENTS.md: mocks are not completion
    /// evidence).
    #[test]
    fn autopilot_does_not_auto_confirm_with_test_sample_provider() {
        // The settings layer rejects the test-sample provider as user-selectable
        // (its primary protection — `selectable_analysis_provider_ids` excludes it).
        // Inject it straight into the settings row to simulate the state the guard
        // defends against (a corrupt or imported settings value that bypassed
        // validation), then assert autopilot still refuses to auto-confirm.
        let connection = open_in_memory_database().expect("db");
        connection
            .execute(
                "INSERT INTO settings (key, value, value_type) \
                 VALUES ('general_analysis_provider', ?1, 'string') \
                 ON CONFLICT(key) DO UPDATE SET value = excluded.value",
                [TEST_SAMPLE_ANALYSIS_PROVIDER_ID],
            )
            .expect("inject test-sample provider bypassing validation");
        let state = AppState::new(connection);
        let company = state
            .create_company(NewCompany {
                exchange: "GPW".to_owned(),
                ticker: "CDR".to_owned(),
                display_name: "CD PROJEKT S.A.".to_owned(),
                isin: None,
                cik: None,
                lei: None,
            })
            .expect("company");

        let run_id = "run_test_sample";
        state
            .autopilot()
            .create_run_if_absent(run_id, &company.id, "doc1", "manual", MODE_AUTOPILOT)
            .expect("create run")
            .expect("run created");
        let run = state.autopilot().get_run(run_id).expect("get run");

        stage_extract(&state, &run).expect("extract stage");

        let after = state.autopilot().get_run(run_id).expect("get run");
        let delta = after.kpi_delta_json.expect("kpi delta recorded");
        assert!(
            delta.contains("test_sample_provider"),
            "degrade reason should be test_sample_provider, got: {delta}"
        );
        assert!(
            delta.contains("\"extractionAvailable\":false"),
            "extraction should be unavailable, got: {delta}"
        );
        assert!(
            after.produced_fact_ids.is_empty(),
            "no sample facts may be auto-confirmed, got: {:?}",
            after.produced_fact_ids
        );
    }

    /// Guardrail (ADR 0059): a failed KPI extraction must fail the extract stage,
    /// never be recorded as a silent success. `run_kpi_extraction_job` returns `Ok`
    /// even when it marks the job `failed` (e.g. a transient provider 503), so
    /// without the status check `stage_extract` would record `proposed: 0` on a
    /// `succeeded` run and block re-detection forever.
    #[test]
    fn extract_stage_fails_when_kpi_extraction_fails() {
        // An unknown provider id makes the inline KPI extraction fail deterministically
        // at provider resolution (no network, no document needed). Inject it straight
        // into settings — the validation layer would reject it as user-selectable, but
        // this simulates the runtime state the guard must handle.
        let connection = open_in_memory_database().expect("db");
        connection
            .execute(
                "INSERT INTO settings (key, value, value_type) \
                 VALUES ('general_analysis_provider', 'nonexistent_provider_xyz', 'string') \
                 ON CONFLICT(key) DO UPDATE SET value = excluded.value",
                [],
            )
            .expect("inject unknown provider");
        let state = AppState::new(connection);
        let company = state
            .create_company(NewCompany {
                exchange: "GPW".to_owned(),
                ticker: "CBF".to_owned(),
                display_name: "Cyber_Folks S.A.".to_owned(),
                isin: None,
                cik: None,
                lei: None,
            })
            .expect("company");

        // A real report document (FK-referenced by the KPI job).
        let doc = state
            .create_or_find_pending_report_document(storage::CaptureReportDocumentInput {
                company_id: company.id.clone(),
                source_type: "user_url".to_owned(),
                url: "https://example.com/ssf.pdf".to_owned(),
                period_id: None,
                origin_ref: None,
                title: Some("Cyber_Folks 2026 Q1 SSF".to_owned()),
                attribution: None,
            })
            .expect("report document");

        let run_id = "run_failing_extract";
        state
            .autopilot()
            .create_run_if_absent(run_id, &company.id, &doc.id, "manual", MODE_AUTOPILOT)
            .expect("create run")
            .expect("run created");
        let run = state.autopilot().get_run(run_id).expect("get run");

        let result = stage_extract(&state, &run);
        assert!(
            result.is_err(),
            "a failed KPI extraction must fail the extract stage, got: {result:?}"
        );
        assert!(
            result.unwrap_err().contains("KPI extraction failed"),
            "the error should surface the extraction failure"
        );
    }

    fn report_doc(id: &str, url: &str, title: &str, created_at: &str) -> storage::ReportDocument {
        storage::ReportDocument {
            id: id.to_owned(),
            company_id: "company_gpw_cbf".to_owned(),
            period_id: None,
            source_type: "user_url".to_owned(),
            origin_ref: None,
            url: url.to_owned(),
            local_path: Some("report_documents/x.pdf".to_owned()),
            content_type: Some("application/pdf".to_owned()),
            content_hash: None,
            byte_size: Some(1),
            title: Some(title.to_owned()),
            attribution: None,
            fetch_status: "fetched".to_owned(),
            fetch_error: None,
            fetched_at: None,
            created_at: created_at.to_owned(),
            updated_at: created_at.to_owned(),
        }
    }

    #[test]
    fn disclosure_key_reads_the_emitent_month_from_espi_urls() {
        // Both accepted ESPI attachment hosts embed /emitent/YYYY-MM/.
        assert_eq!(
            disclosure_month_from_url(
                "https://bonnier.pl/static/att/emitent/2026-05/20260520_172023_x_ssf.pdf"
            ),
            Some("2026-05".to_owned())
        );
        assert_eq!(
            disclosure_month_from_url(
                "https://www.bankier.pl/static/att/emitent/2023-09/c-F-2023-Q2-SSF.pdf"
            ),
            Some("2023-09".to_owned())
        );
        // No /emitent/ segment (e.g. an IR landing page) → no month.
        assert_eq!(
            disclosure_month_from_url("https://modivo.pl/relacje-inwestorskie"),
            None
        );

        // Key falls back to fetched_at, then created_at, when the URL has no month.
        let mut doc = report_doc(
            "d",
            "https://example.com/ir",
            "Q1 SSF",
            "2026-06-15T10:00:00Z",
        );
        doc.fetched_at = Some("2023-08-01T09:00:00Z".to_owned());
        assert_eq!(report_disclosure_key(&doc), "2023-08-01");
        doc.fetched_at = None;
        assert_eq!(report_disclosure_key(&doc), "2026-06-15");
    }

    /// Guardrail (`d60305c`): detection must rank by the report's disclosure date,
    /// not `created_at`. Real-data-shaped: an on-track backfill gives the OLD 2023
    /// report a NEWER `created_at` than the actual-latest 2026 report — ranking on
    /// `created_at` (the bug) picks 2023; ranking on disclosure picks 2026.
    #[test]
    fn newest_per_type_ranks_by_disclosure_not_created_at() {
        let stale_2023 = report_doc(
            "doc_2023_q2_ssf",
            "https://www.bankier.pl/static/att/emitent/2023-09/c-F-2023-Q2-SSF.pdf",
            "Cyber Folks 2023 Q2 SSF",
            "2026-06-15T16:49:36.268Z", // backfilled later → newer created_at
        );
        let latest_2026 = report_doc(
            "doc_2026_q1_ssf",
            "https://bonnier.pl/static/att/emitent/2026-05/20260520_x_ssf.pdf",
            "Cyber Folks 2026 Q1 SSF",
            "2026-06-15T16:49:36.167Z", // ingested earlier → older created_at
        );

        let picked = newest_periodic_reports_per_type(vec![stale_2023, latest_2026]);

        assert_eq!(picked.len(), 1, "both are the same statement type (ssf)");
        assert_eq!(
            picked[0].id, "doc_2026_q1_ssf",
            "the actual-latest report must win, not the recently-backfilled old one"
        );
    }
}
