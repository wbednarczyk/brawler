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
    capabilities::{AiCapability, CAPABILITY_ROUTED_PROVIDER_ID},
    KPI_EXTRACTION_PROMPT_VERSION, TEST_SAMPLE_ANALYSIS_PROVIDER_ID,
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
    let job_id = stage_job_id(run_id, stage);
    // Stage job ids are deterministic (`autopilot:{run_id}:{stage}`), and a run id is
    // itself deterministic on `(company, report_document)` (`create_run_if_absent`).
    // So a run can be *recreated* under the same id (self-heal of a failed run with
    // no produced facts, or any future recovery path) while its stage's `job_queue`
    // row from the run's prior life still exists — possibly already terminal
    // (`succeeded`/`failed`). Plain `enqueue` is `INSERT OR IGNORE`: against an
    // existing row that is a silent no-op, so the recreated run would never be
    // driven again (bug dce9ce8 — a run stuck at pending/fetch forever). `reschedule`
    // is the correct primitive here: it re-arms an existing terminal row back to
    // `pending`, matches the fresh `payload`, and — critically — leaves a `running`
    // row untouched so an in-flight stage is never disturbed or double-run.
    match state
        .jobs()
        .reschedule(&job_id, AUTOPILOT_STAGE_KIND, &payload, 1)
    {
        Ok(true) => {}
        Ok(false) => {
            // The only way `reschedule` reports "not (re)armed" is an existing row
            // still `running` — expected when this stage is already in flight from
            // a prior life; logged so a silent no-op can never hide unnoticed again.
            log::info!(
                "autopilot: stage {stage} job {job_id} for run {run_id} already running, not re-armed"
            );
        }
        Err(error) => {
            log::warn!("autopilot: failed to enqueue stage {stage} for run {run_id}: {error}");
        }
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

/// How many of a structured-extraction result's produced facts need no further
/// review (`confirmed` or `auto_unreviewed`), given the run's mode. Bug e77a1a2:
/// the run's `kpi_delta_json` used to carry only `produced` (a raw fact count)
/// for the structured tier, with no honest "auto-confirmed" figure — the Today
/// card's summary then read a *different* branch's `autoConfirmed` key (always
/// absent here) and silently defaulted to 0, showing "0 of 0" for a run that had
/// just committed dozens of facts. [`Acceptance`] is a single verdict for the
/// whole batch (`run_structured_extraction` applies one `confirmation_state` to
/// every fact it emits), so every produced fact shares the same state:
/// `Accepted`/`AcceptedViaWitness` auto-confirm in **both** modes (the clean gate
/// already proved them); `AcceptedUnreviewed` follows the mode ladder
/// (`auto_unreviewed` in autopilot, `pending` in assist — not yet reviewed, so
/// not counted here). `Flagged`/`Empty` never reach this (nothing was emitted).
fn structured_facts_auto_confirmed(
    acceptance: crate::fundamentals::extraction::pipeline::Acceptance,
    produced: usize,
    mode: &str,
) -> usize {
    use crate::fundamentals::extraction::pipeline::Acceptance;
    match acceptance {
        Acceptance::Accepted | Acceptance::AcceptedViaWitness => produced,
        Acceptance::AcceptedUnreviewed if mode == MODE_AUTOPILOT => produced,
        _ => 0,
    }
}

/// Merges a "structure changed" flag carried over from a structured-extraction
/// attempt into whichever branch's `kpi_delta` ends up composed (structured
/// itself, or the AI fallback it fell through to) — so a drift/contradiction
/// is never silently dropped just because AI ultimately produced the facts.
/// A no-op when nothing changed (the common case).
fn merge_structure_flag(
    delta: &mut serde_json::Value,
    structure_changed: bool,
    drift_json: Option<&str>,
) {
    if !structure_changed {
        return;
    }
    if let Some(obj) = delta.as_object_mut() {
        obj.insert("structureChanged".to_owned(), serde_json::Value::Bool(true));
        if let Some(drift) = drift_json {
            obj.insert(
                "driftJson".to_owned(),
                serde_json::Value::String(drift.to_owned()),
            );
        }
    }
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
    // Structured-first (ADR 0061 dec. 3/8/9): for a tagged ESEF/iXBRL filing or
    // a PDF whose reporting period can be derived from its title/URL,
    // deterministic extraction runs before AI, in **both** modes — assist gets
    // the same structured-first benefit, not just autopilot. A validated
    // (Accepted/AcceptedViaWitness) set auto-confirms in both modes; an
    // unreviewed-but-uncontradicted set keeps the existing ladder
    // (auto_unreviewed in autopilot, pending in assist). A flagged (drifted or
    // contradicted) structured attempt still falls through to AI below — a
    // drifted profile must not blind the company — but its "structure changed"
    // signal is carried into whichever branch's delta ends up composed.
    let mut carried_structure_changed = false;
    let mut carried_drift_json: Option<String> = None;

    match try_structured_extraction(state, run) {
        Ok(Some(result)) if result.emitted => {
            if !result.produced_fact_ids.is_empty() {
                state
                    .autopilot()
                    .add_produced_facts(&run.id, &result.produced_fact_ids)
                    .map_err(|e| e.to_string())?;
            }
            let produced = result.produced_fact_ids.len();
            let auto_confirmed =
                structured_facts_auto_confirmed(result.acceptance, produced, &run.mode);
            let mut delta = serde_json::json!({
                "extractionAvailable": true,
                "structured": true,
                "tier": result.tier.map(|t| t.as_str()),
                "produced": produced,
                // Normalized counts (bug e77a1a2) — the same keys the AI branch below
                // writes, so `compose_summary` and the frontend read one honest shape
                // regardless of which tier produced the facts.
                "factsProposed": produced,
                "factsAutoConfirmed": auto_confirmed,
                "mode": run.mode,
            });
            merge_structure_flag(
                &mut delta,
                result.structure_changed,
                result.drift_json.as_deref(),
            );
            let _ = state
                .autopilot()
                .set_kpi_delta_json(&run.id, &delta.to_string());
            return Ok(());
        }
        // Flagged (drift/contradiction) or empty: nothing emitted structurally.
        // Carry the structure-changed signal into the AI branch's delta below
        // rather than dropping it — the AI path still runs (never blind the
        // company), its facts just stay unreviewed/pending as before.
        Ok(Some(result)) => {
            carried_structure_changed = result.structure_changed;
            carried_drift_json = result.drift_json;
        }
        // Not eligible for structured extraction (no derivable period, unparsable, …).
        Ok(None) => {}
        Err(error) => log::info!(
            "autopilot run {}: structured extraction skipped: {error}",
            run.id
        ),
    }

    // ADR 0060 as amended: resolve the KPI extraction capability's pool
    // (capability_providers map, else the general fallback) instead of reading
    // `general_analysis_provider` directly.
    let members = crate::jobs::resolve_capability_members(state, AiCapability::KpiExtraction)?;

    let Some(primary) = members.first() else {
        // No AI configured: degrade rather than loop or use sample data.
        let mut delta = serde_json::json!({
            "extractionAvailable": false,
            "reason": "no_ai_provider",
        });
        merge_structure_flag(
            &mut delta,
            carried_structure_changed,
            carried_drift_json.as_deref(),
        );
        let _ = state
            .autopilot()
            .set_kpi_delta_json(&run.id, &delta.to_string());
        return Ok(());
    };

    // Autopilot must never auto-commit facts from a non-real provider. The
    // test-sample analysis provider returns placeholder KPIs, so in `autopilot`
    // mode it would auto-confirm sample data as `auto_unreviewed` facts — treat it
    // as "no real provider" and degrade (ADR 0055; CLAUDE.md: mocks are never
    // completion evidence). `assist` mode still runs: its proposals stay `pending`
    // and are user-gated, so no sample fact is ever committed without review.
    // Only the resolved *primary* member is checked: it is what the pool tries
    // first, and a pool that starts on the test-sample provider is exactly the
    // "no real provider configured" case this guard exists for.
    if run.mode == MODE_AUTOPILOT && primary.provider_id == TEST_SAMPLE_ANALYSIS_PROVIDER_ID {
        let mut delta = serde_json::json!({
            "extractionAvailable": false,
            "reason": "test_sample_provider",
        });
        merge_structure_flag(
            &mut delta,
            carried_structure_changed,
            carried_drift_json.as_deref(),
        );
        let _ = state
            .autopilot()
            .set_kpi_delta_json(&run.id, &delta.to_string());
        return Ok(());
    }

    let job = state
        .create_kpi_extraction_job(storage::NewKpiExtractionJob {
            company_id: run.company_id.clone(),
            report_document_id: run.report_document_id.clone(),
            provider_id: CAPABILITY_ROUTED_PROVIDER_ID.to_owned(),
            model: CAPABILITY_ROUTED_PROVIDER_ID.to_owned(),
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

    let mut delta = serde_json::json!({
        "extractionAvailable": true,
        "proposed": proposed,
        "autoConfirmed": confirmed_ids.len(),
        // Normalized counts (bug e77a1a2), same keys the structured branch above
        // writes.
        "factsProposed": proposed,
        "factsAutoConfirmed": confirmed_ids.len(),
        "mode": run.mode,
    });
    merge_structure_flag(
        &mut delta,
        carried_structure_changed,
        carried_drift_json.as_deref(),
    );
    let _ = state
        .autopilot()
        .set_kpi_delta_json(&run.id, &delta.to_string());
    Ok(())
}

/// Attempts structured-first extraction for a document eligible for it (ADR
/// 0061 dec. 3/8/9): a tagged ESEF/iXBRL `.xhtml` filing, or a PDF whose
/// reporting period can be derived from its title/URL. Returns `Ok(None)` when
/// the document is not eligible (unparsable ESEF, or a PDF whose period can't
/// be classified), so the caller falls back to the AI path. Runs in **both**
/// trust-ladder modes — [`crate::jobs::structured_extraction::
/// run_structured_extraction`] derives the per-fact confirmation state from
/// `run.mode` and the pipeline's acceptance.
///
/// - **Xhtml/ESEF**: the period is self-derived from the iXBRL contexts (ESEF
///   is an annual filing → `FY` at the latest context date).
/// - **Pdf**: the period is derived from the document's title/URL via
///   [`crate::report_diff::classify::period_sort_key`], assuming a calendar
///   fiscal year (index 1→`Q1`/`-03-31`, 2→`H1`/`-06-30`, 3→`Q3`/`-09-30`,
///   4→`FY`/`-12-31`). An unparseable or ambiguous intra-year period (index
///   `0`) is not guessed — it falls to AI. A non-calendar fiscal year is
///   either caught later by the cross-period comparative check or, worst
///   case, falls to AI on the next report.
fn try_structured_extraction(
    state: &AppState,
    run: &storage::AutopilotRun,
) -> Result<Option<crate::jobs::structured_extraction::StructuredExtractionResult>, String> {
    use crate::fundamentals::extraction::{esef::parse_esef, primary_period_end};
    use crate::report_diff::classify::period_sort_key;
    use crate::report_diff::extraction::SourceFormat;

    let document = state
        .get_report_document(&run.report_document_id)
        .map_err(|e| e.to_string())?;
    let Some(local_path) = document.local_path.clone() else {
        return Ok(None);
    };
    let format = SourceFormat::resolve(document.content_type.as_deref(), &local_path);

    let (fiscal_year, period_type, period_end): (i64, &'static str, String) = match format {
        SourceFormat::Xhtml => {
            let bytes = match std::fs::read(state.data_dir().join(&local_path)) {
                Ok(b) => b,
                Err(_) => return Ok(None),
            };
            let facts = match parse_esef(&bytes) {
                Ok(f) => f,
                Err(_) => return Ok(None), // Not valid iXBRL → fall back to AI.
            };
            let Some(period_end) = primary_period_end(&facts) else {
                return Ok(None);
            };
            let Some(fiscal_year) = period_end.get(0..4).and_then(|y| y.parse::<i64>().ok()) else {
                return Ok(None);
            };
            (fiscal_year, "FY", period_end)
        }
        SourceFormat::Pdf => {
            let title = document.title.as_deref().unwrap_or("");
            let Some((year, period_index)) = period_sort_key(title, &document.url) else {
                return Ok(None); // No parseable period → fall back to AI.
            };
            let period_type = match period_index {
                1 => "Q1",
                2 => "H1",
                3 => "Q3",
                4 => "FY",
                // Unknown intra-year period (0) — never guess; fall back to AI.
                _ => return Ok(None),
            };
            let period_end = match period_type {
                "Q1" => format!("{year}-03-31"),
                "H1" => format!("{year}-06-30"),
                "Q3" => format!("{year}-09-30"),
                _ => format!("{year}-12-31"),
            };
            (i64::from(year), period_type, period_end)
        }
    };

    let result = crate::jobs::structured_extraction::run_structured_extraction(
        state,
        &run.company_id,
        &run.report_document_id,
        fiscal_year,
        period_type,
        &period_end,
        &run.mode,
    )?;
    Ok(Some(result))
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
            // Normalized counts (bug e77a1a2): both the structured and AI branches of
            // `stage_extract` write these same keys now, so this reads one honest
            // shape regardless of which tier produced the facts — previously this
            // read AI-only `proposed`/`autoConfirmed`, which the structured branch
            // never wrote, silently defaulting to 0 for every structured-tier run.
            let proposed = delta
                .get("factsProposed")
                .and_then(|v| v.as_u64())
                .unwrap_or(0);
            let confirmed = delta
                .get("factsAutoConfirmed")
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
                Ok(None) => {
                    // Already has a run for this (company, document) -- dedup. But a
                    // non-terminal run's current-stage job may never have actually been
                    // armed (bug dce9ce8): a stale `job_queue` row left `succeeded` by
                    // an unrelated prior life of the same deterministic stage id made
                    // the original `enqueue_stage` call a silent no-op, so `run_stage`
                    // was never invoked and the run stuck at pending/fetch forever with
                    // no later event to retry it. Re-arm on every sweep instead of only
                    // at creation: safe even for a genuinely in-flight run, since
                    // `enqueue_stage`/`reschedule` leaves a `running` row untouched and
                    // resetting an already-`pending` row to `pending` is a no-op.
                    if let Ok(existing) = state.autopilot().get_run(&run_id) {
                        if matches!(existing.status.as_str(), "pending" | "running") {
                            enqueue_stage(state, &existing.id, &existing.stage);
                        }
                    }
                }
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
/// ranking on insert order fires autopilot on a years-old report (`d60305c`). On a
/// disclosure-date TIE (e.g. a PDF and its structured xhtml sibling from the same
/// filing), the structured xhtml document wins — see [`prefers_candidate`]
/// (ADR 0061 decision 1b).
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
            Some(current) if !prefers_candidate(current, &document) => {}
            _ => {
                newest.insert(key, document);
            }
        }
    }
    newest.into_values().collect()
}

/// Whether `candidate` should replace `current` as the newest document for its
/// statement type: a strictly newer disclosure date always wins; on a
/// disclosure-date TIE, a structured xhtml document wins over a non-xhtml one
/// (ADR 0061 decision 1b) — never the reverse, so this never overrides an
/// actually-newer report.
fn prefers_candidate(
    current: &storage::ReportDocument,
    candidate: &storage::ReportDocument,
) -> bool {
    use std::cmp::Ordering;
    match report_disclosure_key(candidate).cmp(&report_disclosure_key(current)) {
        Ordering::Greater => true,
        Ordering::Less => false,
        Ordering::Equal => is_structured_document(candidate) && !is_structured_document(current),
    }
}

/// Whether a report document's format is a structured ESEF/iXBRL (xhtml)
/// statement rather than a PDF, resolved from its content type and/or
/// URL/local path. Used only to break a disclosure-date tie in
/// [`prefers_candidate`].
fn is_structured_document(document: &storage::ReportDocument) -> bool {
    use crate::report_diff::extraction::SourceFormat;

    SourceFormat::resolve(document.content_type.as_deref(), &document.url) == SourceFormat::Xhtml
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
    use crate::storage::{open_in_memory_database, CaptureReportDocumentInput, NewCompany};

    // A minimal balanced ESEF/iXBRL instance (45m = 20m + 25m at 2026-03-31).
    const ESEF: &str = r#"<html xmlns:ix="http://www.xbrl.org/2013/inlineXBRL"
      xmlns:xbrli="http://www.xbrl.org/2003/instance"
      xmlns:iso4217="http://www.xbrl.org/2003/iso4217">
      <xbrli:context id="c"><xbrli:period><xbrli:instant>2026-03-31</xbrli:instant></xbrli:period></xbrli:context>
      <xbrli:unit id="pln"><xbrli:measure>iso4217:PLN</xbrli:measure></xbrli:unit>
      <ix:nonFraction name="ifrs-full:Assets" contextRef="c" unitRef="pln" scale="3">45 000</ix:nonFraction>
      <ix:nonFraction name="ifrs-full:Liabilities" contextRef="c" unitRef="pln" scale="3">20 000</ix:nonFraction>
      <ix:nonFraction name="ifrs-full:Equity" contextRef="c" unitRef="pln" scale="3">25 000</ix:nonFraction>
    </html>"#;

    /// ADR 0061: in autopilot mode a tagged ESEF filing is extracted
    /// deterministically before AI — validated facts land auto_unreviewed with
    /// `esef`/`passed` provenance, and the AI path is skipped.
    #[test]
    fn autopilot_esef_uses_structured_extraction_and_skips_ai() {
        let dir =
            std::env::temp_dir().join(format!("brawler-autopilot-esef-{}", std::process::id()));
        std::fs::create_dir_all(&dir).expect("temp dir");
        let connection = open_in_memory_database().expect("db");
        let state = AppState::with_data_dir(connection, dir.clone());
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
        let document = state
            .create_or_find_pending_report_document(CaptureReportDocumentInput {
                company_id: company.id.clone(),
                source_type: "user_url".to_owned(),
                url: "https://example.com/annual-2026.xhtml".to_owned(),
                period_id: None,
                origin_ref: None,
                title: Some("Annual 2026 ESEF".to_owned()),
                attribution: None,
            })
            .expect("document");
        std::fs::write(dir.join("annual.xhtml"), ESEF.as_bytes()).expect("write esef");
        state
            .mark_report_document_fetched(
                &document.id,
                Some("annual.xhtml"),
                Some("application/xhtml+xml"),
                None,
                Some(ESEF.len() as i64),
            )
            .expect("mark fetched");

        let run_id = "run_esef";
        state
            .autopilot()
            .create_run_if_absent(run_id, &company.id, &document.id, "manual", MODE_AUTOPILOT)
            .expect("create run")
            .expect("run created");
        let run = state.autopilot().get_run(run_id).expect("get run");

        stage_extract(&state, &run).expect("extract stage");

        let after = state.autopilot().get_run(run_id).expect("get run");
        assert!(
            !after.produced_fact_ids.is_empty(),
            "structured ESEF facts should be auto-committed"
        );
        let delta = after.kpi_delta_json.clone().expect("kpi delta recorded");
        assert!(delta.contains("\"structured\":true"), "delta: {delta}");
        assert!(
            delta.contains("esef"),
            "delta should name the tier: {delta}"
        );
        // Bug e77a1a2: the structured tier used to carry only a raw `produced`
        // count, with no honest "auto-confirmed" figure — the notification then
        // read the AI branch's `autoConfirmed` key (always absent here) and
        // silently defaulted to 0 (live evidence: a real run reported "0 KPI
        // auto-confirmed of 0 extracted" while 40 structured facts were stored).
        assert!(
            delta.contains("\"factsProposed\":3"),
            "delta should carry the normalized proposed count: {delta}"
        );
        assert!(
            delta.contains("\"factsAutoConfirmed\":3"),
            "a validation-clean ESEF set auto-confirms all 3 facts: {delta}"
        );

        let provenance = state
            .fundamentals_provenance()
            .get_many(&after.produced_fact_ids)
            .expect("provenance");
        assert_eq!(provenance.len(), after.produced_fact_ids.len());
        assert!(provenance
            .iter()
            .all(|p| p.source_tier == "esef" && p.validation_status == "passed"));

        // ADR 0061 dec. 3/8/9: a validation-clean structured set auto-confirms
        // outright — even in autopilot mode, it does not land `auto_unreviewed`.
        let facts = state
            .list_financial_facts(storage::ListFinancialFactsInput {
                company_id: Some(company.id.clone()),
                period_id: None,
                definition_id: None,
            })
            .expect("list facts");
        assert!(
            facts
                .iter()
                .filter(|f| after.produced_fact_ids.contains(&f.id))
                .all(|f| f.confirmation_state == "confirmed"),
            "facts: {facts:?}"
        );

        // No AI job was ever created — structured extraction alone satisfied the stage.
        assert!(
            state
                .list_kpi_extraction_jobs_by_document(&document.id)
                .expect("list jobs")
                .is_empty(),
            "AI must be skipped entirely when structured extraction emits"
        );

        // The composed notification summary itself must reflect the honest count,
        // not "0 KPI auto-confirmed (unreviewed) of 0 extracted" — the exact
        // real-world symptom of bug e77a1a2.
        let summary = compose_summary(&after);
        assert!(
            summary.contains("3 KPI auto-confirmed (unreviewed) of 3 extracted"),
            "summary: {summary}"
        );
    }

    /// Bug e77a1a2 — direct regression for `compose_summary`, isolated from the
    /// full structured-extraction pipeline: a structured-tier delta shaped
    /// exactly like the live report (40 facts stored, `autopilot` mode) must
    /// summarize with the real count, not silently default to "0 KPI
    /// auto-confirmed (unreviewed) of 0 extracted" because the structured tier
    /// used to omit the AI-only `proposed`/`autoConfirmed` keys `compose_summary`
    /// read.
    #[test]
    fn compose_summary_reports_honest_counts_for_a_structured_tier_delta() {
        let connection = open_in_memory_database().expect("db");
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
        let run_id = "run_structured_counts";
        state
            .autopilot()
            .create_run_if_absent(run_id, &company.id, "doc1", "manual", MODE_AUTOPILOT)
            .expect("create run")
            .expect("run created");
        let delta = serde_json::json!({
            "extractionAvailable": true,
            "structured": true,
            "tier": "esef",
            "produced": 40,
            "factsProposed": 40,
            "factsAutoConfirmed": 40,
            "mode": MODE_AUTOPILOT,
        });
        state
            .autopilot()
            .set_kpi_delta_json(run_id, &delta.to_string())
            .expect("set delta");
        let run = state.autopilot().get_run(run_id).expect("get run");

        let summary = compose_summary(&run);
        assert!(
            summary.contains("40 KPI auto-confirmed (unreviewed) of 40 extracted"),
            "summary: {summary}"
        );
    }

    /// A per-call-unique scratch dir: a fixed pid-only dir would collide across
    /// parallel `#[test]` threads and loop iterations sharing this file's data
    /// dir (the same flakiness class fixed in `jobs::structured_extraction`'s
    /// test module).
    fn unique_temp_dir(label: &str) -> std::path::PathBuf {
        use std::sync::atomic::{AtomicU64, Ordering};
        static COUNTER: AtomicU64 = AtomicU64::new(0);
        let n = COUNTER.fetch_add(1, Ordering::Relaxed);
        std::env::temp_dir().join(format!(
            "brawler-autopilot-{}-{label}-{n}",
            std::process::id()
        ))
    }

    /// Builds a minimal, valid single-page PDF whose extracted text reproduces
    /// `lines` (padded past `pdf-extract`'s 200-chars/page no-text-layer floor
    /// with statement boilerplate) — see `jobs::structured_extraction`'s test
    /// module for the full rationale; duplicated here rather than shared since
    /// each module's tests are self-contained.
    fn minimal_text_pdf(lines: &[&str]) -> Vec<u8> {
        let filler = "Nota objasniajaca do sprawozdania finansowego za okres sprawozdawczy.";
        let mut all_lines: Vec<&str> = lines.to_vec();
        while all_lines.iter().map(|l| l.len() + 1).sum::<usize>() < 220 {
            all_lines.push(filler);
        }
        let mut content = String::from("BT /F1 12 Tf 40 750 Td 16 TL\n");
        for (i, line) in all_lines.iter().enumerate() {
            if i > 0 {
                content.push_str("T*\n");
            }
            let escaped = line
                .replace('\\', "\\\\")
                .replace('(', "\\(")
                .replace(')', "\\)");
            content.push_str(&format!("({escaped}) Tj\n"));
        }
        content.push_str("ET");

        let objects = [
            "<</Type/Catalog/Pages 2 0 R>>".to_owned(),
            "<</Type/Pages/Kids[3 0 R]/Count 1>>".to_owned(),
            "<</Type/Page/Parent 2 0 R/Resources<</Font<</F1 4 0 R>>>>/MediaBox[0 0 612 792]/Contents 5 0 R>>"
                .to_owned(),
            "<</Type/Font/Subtype/Type1/BaseFont/Helvetica>>".to_owned(),
            format!(
                "<</Length {}>>\nstream\n{}\nendstream",
                content.len(),
                content
            ),
        ];
        let mut buf = b"%PDF-1.4\n".to_vec();
        let mut offsets = Vec::with_capacity(objects.len());
        for (i, obj) in objects.iter().enumerate() {
            offsets.push(buf.len());
            buf.extend_from_slice(format!("{} 0 obj\n{obj}\nendobj\n", i + 1).as_bytes());
        }
        let xref_offset = buf.len();
        buf.extend_from_slice(format!("xref\n0 {}\n", objects.len() + 1).as_bytes());
        buf.extend_from_slice(b"0000000000 65535 f \n");
        for offset in &offsets {
            buf.extend_from_slice(format!("{offset:010} 00000 n \n").as_bytes());
        }
        buf.extend_from_slice(
            format!(
                "trailer\n<</Size {}/Root 1 0 R>>\nstartxref\n{}\n%%EOF",
                objects.len() + 1,
                xref_offset
            )
            .as_bytes(),
        );
        buf
    }

    /// Seeds a company + fetched PDF report document whose `title` carries a
    /// parseable period (`report_diff::classify::period_sort_key`) and whose
    /// file contains `lines`, so `try_structured_extraction`'s PDF branch has
    /// both a period to derive and text to parse.
    fn seed_pdf_report(
        state: &AppState,
        dir: &std::path::Path,
        title: &str,
        lines: &[&str],
    ) -> (String, String) {
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
        let document = state
            .create_or_find_pending_report_document(CaptureReportDocumentInput {
                company_id: company.id.clone(),
                source_type: "user_url".to_owned(),
                url: "https://example.com/report.pdf".to_owned(),
                period_id: None,
                origin_ref: None,
                title: Some(title.to_owned()),
                attribution: None,
            })
            .expect("document");
        let bytes = minimal_text_pdf(lines);
        std::fs::write(dir.join("report.pdf"), &bytes).expect("write pdf");
        state
            .mark_report_document_fetched(
                &document.id,
                Some("report.pdf"),
                Some("application/pdf"),
                None,
                Some(bytes.len() as i64),
            )
            .expect("mark fetched");
        (company.id, document.id)
    }

    /// ASCII-only profile so the fixture never has to round-trip Polish
    /// diacritics through the hand-built PDF (matches the technique in
    /// `jobs::structured_extraction`'s test module).
    fn ascii_profile(
        company_id: &str,
        labels: &[(&str, &str)],
    ) -> crate::fundamentals::extraction::profile::ExtractionProfile {
        crate::fundamentals::extraction::profile::ExtractionProfile {
            company_id: company_id.to_owned(),
            template_hash: "test-template".to_owned(),
            unit_scale: crate::fundamentals::extraction::pdf::UnitScale::Thousands,
            label_map: labels
                .iter()
                .map(|(l, m)| (l.to_string(), m.to_string()))
                .collect(),
            version: 1,
        }
    }

    /// ADR 0061 dec. 3/8/9: a PDF report is equally eligible for structured-first
    /// extraction in autopilot mode (not just ESEF/iXBRL); a validation-clean
    /// balance-sheet set auto-confirms and the AI job is skipped entirely.
    #[test]
    fn autopilot_pdf_with_derivable_period_uses_structured_extraction_and_skips_ai() {
        let dir = unique_temp_dir("pdf-autopilot");
        std::fs::create_dir_all(&dir).expect("temp dir");
        let connection = open_in_memory_database().expect("db");
        let state = AppState::with_data_dir(connection, dir.clone());
        let (company_id, document_id) = seed_pdf_report(
            &state,
            &dir,
            "CD PROJEKT 2026 Q1 SSF",
            &[
                "Total Assets Line 45 000",
                "Total Liabilities Line 20 000",
                "Total Equity Line 25 000",
            ],
        );
        let profile = ascii_profile(
            &company_id,
            &[
                ("total assets line", "total_assets"),
                ("total liabilities line", "total_liabilities"),
                ("total equity line", "total_equity"),
            ],
        );
        state
            .fundamentals_provenance()
            .upsert_profile(&profile)
            .expect("seed profile");

        let run_id = "run_pdf_autopilot";
        state
            .autopilot()
            .create_run_if_absent(run_id, &company_id, &document_id, "manual", MODE_AUTOPILOT)
            .expect("create run")
            .expect("run created");
        let run = state.autopilot().get_run(run_id).expect("get run");

        stage_extract(&state, &run).expect("extract stage");

        let after = state.autopilot().get_run(run_id).expect("get run");
        assert!(
            !after.produced_fact_ids.is_empty(),
            "PDF-tier facts should be committed"
        );
        let delta = after.kpi_delta_json.clone().expect("kpi delta recorded");
        assert!(delta.contains("\"structured\":true"), "delta: {delta}");
        assert!(
            delta.contains("pdf"),
            "delta should name the pdf tier: {delta}"
        );

        let provenance = state
            .fundamentals_provenance()
            .get_many(&after.produced_fact_ids)
            .expect("provenance");
        assert!(provenance
            .iter()
            .all(|p| p.source_tier == "pdf" && p.validation_status == "passed"));

        let facts = state
            .list_financial_facts(storage::ListFinancialFactsInput {
                company_id: Some(company_id.clone()),
                period_id: None,
                definition_id: None,
            })
            .expect("list facts");
        assert!(
            facts
                .iter()
                .filter(|f| after.produced_fact_ids.contains(&f.id))
                .all(|f| f.confirmation_state == "confirmed"),
            "facts: {facts:?}"
        );

        assert!(
            state
                .list_kpi_extraction_jobs_by_document(&document_id)
                .expect("list jobs")
                .is_empty(),
            "no AI job should be created when structured extraction emits"
        );
    }

    /// ADR 0061 dec. 3/8/9: structured-first runs in **both** modes now, not
    /// just autopilot. An uncontradicted-but-unproven (`AcceptedUnreviewed`) set
    /// keeps the pre-existing trust ladder: `auto_unreviewed` in autopilot,
    /// `pending` in assist.
    #[test]
    fn structured_first_runs_in_assist_mode_and_unreviewed_facts_follow_trust_ladder() {
        for (mode, expected_state) in [
            (MODE_AUTOPILOT, "auto_unreviewed"),
            (storage::MODE_ASSIST, "pending"),
        ] {
            let dir = unique_temp_dir("pdf-ladder");
            std::fs::create_dir_all(&dir).expect("temp dir");
            let connection = open_in_memory_database().expect("db");
            let state = AppState::with_data_dir(connection, dir.clone());
            let (company_id, document_id) = seed_pdf_report(
                &state,
                &dir,
                "CD PROJEKT 2026 Q1 SSF",
                &["Zysk netto 12 000"],
            );

            let run_id = format!("run_ladder_{mode}");
            state
                .autopilot()
                .create_run_if_absent(&run_id, &company_id, &document_id, "manual", mode)
                .expect("create run")
                .expect("run created");
            let run = state.autopilot().get_run(&run_id).expect("get run");

            stage_extract(&state, &run).expect("extract stage");

            let after = state.autopilot().get_run(&run_id).expect("get run");
            assert!(
                !after.produced_fact_ids.is_empty(),
                "mode={mode}: an uncontradicted parse should still emit"
            );
            let facts = state
                .list_financial_facts(storage::ListFinancialFactsInput {
                    company_id: Some(company_id.clone()),
                    period_id: None,
                    definition_id: None,
                })
                .expect("list facts");
            let states: Vec<&str> = facts
                .iter()
                .filter(|f| after.produced_fact_ids.contains(&f.id))
                .map(|f| f.confirmation_state.as_str())
                .collect();
            assert_eq!(states, vec![expected_state], "mode={mode}");
        }
    }

    /// ADR 0061 dec. 3/8/9: a flagged (drifted) structured attempt must not
    /// blind the company — it still falls through to the AI path below — but
    /// the "structure changed" signal is carried into whichever branch's delta
    /// ends up composed (here, the `no_ai_provider` degrade, since no AI is
    /// configured in this offline test).
    #[test]
    fn flagged_structured_attempt_falls_through_and_carries_structure_changed_flag() {
        let dir = unique_temp_dir("pdf-flagged");
        std::fs::create_dir_all(&dir).expect("temp dir");
        let connection = open_in_memory_database().expect("db");
        connection
            .execute(
                "UPDATE settings SET value = '' WHERE key = 'general_analysis_provider'",
                [],
            )
            .expect("clear default provider for offline drain");
        let state = AppState::with_data_dir(connection, dir.clone());
        // The confirmed profile expects three lines; this report drops the
        // equity line entirely — a real "structure changed" scenario.
        let (company_id, document_id) = seed_pdf_report(
            &state,
            &dir,
            "CD PROJEKT 2026 Q1 SSF",
            &["Total Assets Line 45 000", "Total Liabilities Line 20 000"],
        );
        let profile = ascii_profile(
            &company_id,
            &[
                ("total assets line", "total_assets"),
                ("total liabilities line", "total_liabilities"),
                ("total equity line", "total_equity"),
            ],
        );
        state
            .fundamentals_provenance()
            .upsert_profile(&profile)
            .expect("seed profile");

        let run_id = "run_flagged";
        state
            .autopilot()
            .create_run_if_absent(run_id, &company_id, &document_id, "manual", MODE_AUTOPILOT)
            .expect("create run")
            .expect("run created");
        let run = state.autopilot().get_run(run_id).expect("get run");

        stage_extract(&state, &run).expect("extract stage");

        let after = state.autopilot().get_run(run_id).expect("get run");
        assert!(
            after.produced_fact_ids.is_empty(),
            "a flagged drift must not commit structured facts"
        );
        let delta = after.kpi_delta_json.expect("kpi delta recorded");
        assert!(delta.contains("no_ai_provider"), "delta: {delta}");
        assert!(
            delta.contains("\"structureChanged\":true"),
            "delta: {delta}"
        );
        assert!(delta.contains("driftJson"), "delta: {delta}");
        assert!(
            delta.contains("total equity line"),
            "the drift diff should name the dropped label: {delta}"
        );
    }

    /// Guardrail (issue a3643d7): in `autopilot` mode the test-sample analysis
    /// provider must be treated as "no real provider" — extraction degrades and
    /// nothing is auto-confirmed, so sample/placeholder KPIs never land as
    /// `auto_unreviewed` facts (ADR 0055; CLAUDE.md: mocks are not completion
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

    /// ADR 0060 as amended / ADR 0061 decision 5: when nothing is configured for
    /// the KPI extraction capability at all (no capability map entry, no general
    /// provider), `resolve_capability_members` returns empty and the stage must
    /// degrade with `no_ai_provider` — unchanged from the pre-pool behavior.
    #[test]
    fn stage_extract_degrades_when_no_provider_configured() {
        let connection = open_in_memory_database().expect("db");
        connection
            .execute(
                "UPDATE settings SET value = '' WHERE key = 'general_analysis_provider'",
                [],
            )
            .expect("clear the default general provider");
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

        let run_id = "run_no_provider";
        state
            .autopilot()
            .create_run_if_absent(run_id, &company.id, "doc1", "manual", MODE_AUTOPILOT)
            .expect("create run")
            .expect("run created");
        let run = state.autopilot().get_run(run_id).expect("get run");

        stage_extract(&state, &run).expect("an unconfigured capability must degrade, not error");

        let after = state.autopilot().get_run(run_id).expect("get run");
        let delta = after.kpi_delta_json.expect("kpi delta recorded");
        assert!(
            delta.contains("no_ai_provider"),
            "degrade reason should be no_ai_provider, got: {delta}"
        );
        assert!(
            delta.contains("\"extractionAvailable\":false"),
            "extraction should be unavailable, got: {delta}"
        );
    }

    /// ADR 0060 as amended: once a capability's pool resolves to a real
    /// (non-test-sample) primary member, the stage must create the KPI
    /// extraction job routed through the capability-pool sentinel rather than a
    /// concrete provider id — the run-time pool, not the enqueue-time settings
    /// snapshot, decides which provider actually serves the call.
    #[test]
    fn stage_extract_creates_sentinel_routed_job_when_a_provider_is_configured() {
        let connection = open_in_memory_database().expect("db");
        let state = AppState::new(connection);
        state
            .update_settings(storage::SettingsUpdate {
                general_analysis_provider: Some(
                    crate::providers::analysis::registry::GEMINI_ANALYSIS_PROVIDER_ID.to_owned(),
                ),
                general_analysis_model: Some("gemini-3.5-flash".to_owned()),
                ..Default::default()
            })
            .expect("settings should update");
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
        let document = state
            .create_or_find_pending_report_document(CaptureReportDocumentInput {
                company_id: company.id.clone(),
                source_type: "user_url".to_owned(),
                url: "https://example.com/report-sentinel.pdf".to_owned(),
                period_id: None,
                origin_ref: None,
                title: Some("Sentinel report".to_owned()),
                attribution: None,
            })
            .expect("report document should be captured");

        let run_id = "run_sentinel_route";
        state
            .autopilot()
            .create_run_if_absent(run_id, &company.id, &document.id, "manual", MODE_AUTOPILOT)
            .expect("create run")
            .expect("run created");
        let run = state.autopilot().get_run(run_id).expect("get run");

        // Extraction itself is expected to fail downstream (Gemini has no
        // credential in this test environment) — that failure is exercised by
        // `extract_stage_fails_when_kpi_extraction_fails` below. What this test
        // pins is that the KPI extraction job the stage creates is already
        // routed through the sentinel before that failure happens.
        let _ = stage_extract(&state, &run);

        let jobs = state
            .list_kpi_extraction_jobs_by_document(&document.id)
            .expect("jobs should list");
        let job = jobs
            .first()
            .expect("stage_extract should have created a KPI extraction job");
        assert_eq!(job.provider_id, CAPABILITY_ROUTED_PROVIDER_ID);
        assert_eq!(job.model, CAPABILITY_ROUTED_PROVIDER_ID);
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

    /// ADR 0061 decision 1b: on a disclosure-date TIE for the same statement
    /// type, the structured xhtml document wins over a PDF sibling, since the
    /// deterministic structured-extraction pipeline prefers the xhtml input.
    #[test]
    fn newest_per_type_prefers_xhtml_on_disclosure_tie() {
        let pdf = report_doc(
            "doc_ssf_pdf",
            "https://bonnier.pl/static/att/emitent/2026-05/20260520_ssf.pdf",
            "Cyber Folks 2026 Q1 SSF",
            "2026-06-15T16:49:36.000Z",
        );
        let xhtml = report_doc(
            "doc_ssf_xhtml",
            "https://bonnier.pl/static/att/emitent/2026-05/20260520_ssf.xhtml",
            "Cyber Folks 2026 Q1 SSF",
            "2026-06-15T16:49:36.100Z",
        );

        let picked = newest_periodic_reports_per_type(vec![pdf, xhtml]);
        assert_eq!(picked.len(), 1);
        assert_eq!(
            picked[0].id, "doc_ssf_xhtml",
            "xhtml must win a disclosure-date tie over its pdf sibling"
        );
    }

    /// The tie-break preference must never override a genuinely newer
    /// disclosure date: a strictly-newer PDF still beats an older xhtml.
    #[test]
    fn newest_per_type_strictly_newer_pdf_still_beats_older_xhtml() {
        let older_xhtml = report_doc(
            "doc_ssf_xhtml_old",
            "https://bonnier.pl/static/att/emitent/2026-03/20260320_ssf.xhtml",
            "Cyber Folks 2025 Q4 SSF",
            "2026-04-01T10:00:00.000Z",
        );
        let newer_pdf = report_doc(
            "doc_ssf_pdf_new",
            "https://bonnier.pl/static/att/emitent/2026-05/20260520_ssf.pdf",
            "Cyber Folks 2026 Q1 SSF",
            "2026-06-15T10:00:00.000Z",
        );

        let picked = newest_periodic_reports_per_type(vec![older_xhtml, newer_pdf]);
        assert_eq!(picked.len(), 1);
        assert_eq!(
            picked[0].id, "doc_ssf_pdf_new",
            "a strictly newer disclosure date must win regardless of format"
        );
    }
}
