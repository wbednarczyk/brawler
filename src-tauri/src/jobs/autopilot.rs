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
use crate::storage;

/// Durable-queue job kind for one pipeline stage.
pub const AUTOPILOT_STAGE_KIND: &str = "autopilot_stage";

/// `autopilot_run.trigger` value for a run started by the history sweep (ADR 0077
/// §3). Distinct from the detection sweep so `stage_extract` can gate sweep runs
/// to determinism-only until F3b (the tier-4 AI budget counter, T5.2).
pub const TRIGGER_HISTORY_SWEEP: &str = "history_sweep";

const STAGE_FETCH: &str = "fetch";
const STAGE_EXTRACT: &str = "extract";
const STAGE_DIFF: &str = "diff";
const STAGE_CROSS_REFERENCE: &str = "cross_reference";
const STAGE_NOTIFY: &str = "notify";

/// Queue attempts per stage (ADR 0055 dec. 2: "each stage retries with backoff
/// independently"). Only a **transient** stage failure (a network-level fetch
/// error) consumes retries by returning `Err` to the queue; a fatal domain
/// failure still finalizes the run on the first attempt (#189).
const STAGE_MAX_ATTEMPTS: i64 = 3;

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
        .reschedule(&job_id, AUTOPILOT_STAGE_KIND, &payload, STAGE_MAX_ATTEMPTS)
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

/// A stage failure, split by whether the queue should retry it. Only the fetch
/// stage produces `transient` failures today (network-level errors); every
/// other stage failure is a domain verdict and stays `fatal`.
struct StageFailure {
    transient: bool,
    message: String,
}

impl StageFailure {
    fn fatal(message: impl Into<String>) -> Self {
        Self {
            transient: false,
            message: message.into(),
        }
    }
}

/// Whether a transient failure should still finalize the run: it is the stage
/// job's **last** allowed attempt, so returning `Err` would strand the run
/// `running` forever with a terminally-failed job under it.
fn last_attempt_exhausted(attempts: i64, max_attempts: i64) -> bool {
    attempts >= max_attempts
}

/// Run one pipeline stage (the `autopilot_stage` handler entry point). On success
/// enqueues the next stage; on a fatal domain failure finalizes the run as
/// `failed` (still notified). A **transient** failure (network blip in the fetch
/// stage, #189 / ADR 0055 dec. 2) returns `Err` so the durable queue retries it
/// with backoff — until the stage job's last attempt, which finalizes like a
/// fatal failure so no run is left dangling.
pub fn run_stage(state: &AppState, payload: &str) -> Result<(), String> {
    run_stage_with_fetcher(
        state,
        payload,
        &crate::document_fetcher::HttpDocumentFetcher::new(),
    )
}

/// [`run_stage`] with the fetch stage's document fetcher injectable for tests.
fn run_stage_with_fetcher(
    state: &AppState,
    payload: &str,
    fetcher: &dyn crate::document_fetcher::DocumentFetcher,
) -> Result<(), String> {
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
        STAGE_FETCH => stage_fetch(state, fetcher, &run),
        STAGE_EXTRACT => stage_extract(state, &run).map_err(StageFailure::fatal),
        STAGE_DIFF => stage_diff(state, &run).map_err(StageFailure::fatal),
        STAGE_CROSS_REFERENCE => stage_cross_reference(state, &run).map_err(StageFailure::fatal),
        STAGE_NOTIFY => return finalize_notify(state, &run),
        other => Err(StageFailure::fatal(format!(
            "unknown autopilot stage: {other}"
        ))),
    };

    match outcome {
        Ok(()) => {
            if let Some(next) = next_stage(&payload.stage) {
                enqueue_stage(state, &run.id, next);
            }
            Ok(())
        }
        Err(failure) => {
            if failure.transient {
                // Let the durable queue retry with backoff — unless this was the
                // stage job's last attempt (the claim already incremented
                // `attempts`), in which case fall through to finalize so the run
                // never dangles `running` over a terminally-failed job. A missing
                // job row (should not happen) also falls through — finalizing is
                // the safe end state.
                let job_id = stage_job_id(&run.id, &payload.stage);
                let job = state.jobs().status(&job_id).ok().flatten();
                let exhausted = job
                    .map(|row| last_attempt_exhausted(row.attempts, row.max_attempts))
                    .unwrap_or(true);
                if !exhausted {
                    log::warn!(
                        "autopilot run {} stage {} transient failure, queue will retry: {}",
                        run.id,
                        payload.stage,
                        failure.message
                    );
                    return Err(failure.message);
                }
            }
            // Fatal for this run (or transient with attempts exhausted): finalize
            // as failed, but still surface a notification describing how far it
            // got. Returning Ok keeps the job from looping; the user can
            // re-trigger.
            let _ = state.autopilot().finalize_run(
                &run.id,
                "failed",
                &payload.stage,
                Some(&format!("Autopilot stopped at {} stage.", payload.stage)),
                Some(&failure.message),
            );
            log::warn!(
                "autopilot run {} failed at {} stage: {}",
                run.id,
                payload.stage,
                failure.message
            );
            Ok(())
        }
    }
}

/// Stage 1 — ensure the detected report document's file is downloaded. Idempotent
/// (an already-fetched document is a no-op). Reuses the shared fetch path; a
/// network-level error is transient (the queue retries it, #189).
fn stage_fetch(
    state: &AppState,
    fetcher: &dyn crate::document_fetcher::DocumentFetcher,
    run: &storage::AutopilotRun,
) -> Result<(), StageFailure> {
    crate::report_documents_capture::fetch_report_document(state, fetcher, &run.report_document_id)
        .map_err(|error| StageFailure {
            transient: error.transient,
            message: error.message,
        })?;
    Ok(())
}

/// How many of a structured-extraction result's produced facts are recorded
/// (`confirmed`). Facts are review-free (ADR 0086 dec. 5): every emitted fact
/// lands `confirmed` in **both** modes, so every produced fact is counted. Bug
/// e77a1a2 context: the run's `kpi_delta_json` used to carry only `produced` (a
/// raw fact count) with no honest "confirmed" figure — the Today card then read a
/// *different* branch's key (always absent here) and silently showed "0 of 0" for
/// a run that had just committed dozens of facts; this figure keeps the count
/// honest. `acceptance`/`mode` no longer change the answer (every emit confirms)
/// but stay in the signature so callers pass them uniformly. `Flagged`/`Empty`
/// never reach this (nothing was emitted).
fn structured_facts_auto_confirmed(
    _acceptance: crate::fundamentals::extraction::pipeline::Acceptance,
    produced: usize,
    _mode: &str,
) -> usize {
    produced
}

/// Stage 2 — extract the report's KPIs through the **deterministic** pipeline
/// (ADR 0061 dec. 3/8/9). The tier-4 OCR fallback is retired with the in-app AI
/// layer (ADR 0084 decision 4), so this stage has no AI branch at all: either a
/// deterministic tier emits validated facts, or the run records an honest
/// `extractionAvailable:false` delta carrying a typed
/// [`KpiUnavailableReason`] — flagged for the user via the run's notification,
/// never guessed and never silently absent.
fn stage_extract(state: &AppState, run: &storage::AutopilotRun) -> Result<(), String> {
    let no_tier = KpiUnavailableReason::NoDeterministicTier.as_str();
    let reason = match try_structured_extraction(state, run) {
        Ok(Some(result)) if result.emitted => {
            if !result.produced_fact_ids.is_empty() {
                state
                    .autopilot()
                    .add_produced_facts(&run.id, &result.produced_fact_ids)
                    .map_err(|e| e.to_string())?;
            }
            let delta = emitted_extract_delta(&result, &run.mode);
            let _ = state
                .autopilot()
                .set_kpi_delta_json(&run.id, &delta.to_string());
            return Ok(());
        }
        // A deterministic tier ran but produced no ISSUER emit — an honest gap.
        // A raw-PDF document is the EXPECTED gap (machine fact-reading retired,
        // ADR 0086 dec. 1: core KPIs arrive from the BR-primary pull), reported
        // with its own reason so the Today card never frames it as a failure.
        // (The aggregator-fallback reason is retired with ADR 0086; stored
        // `witness_fallback` deltas stay readable as legacy.)
        Ok(Some(_result)) => gap_reason(state, run, no_tier),
        // Not eligible for the deterministic path (no derivable period, unparsable).
        Ok(None) => gap_reason(state, run, no_tier),
        Err(error) => {
            log::info!(
                "autopilot run {}: structured extraction skipped: {error}",
                run.id
            );
            no_tier
        }
    };

    // No issuer tier could read this document. Record the gap with its typed
    // reason so the notification says what actually happened — and, for a witness
    // fallback, so the re-arm logic keeps the period retryable.
    // Stamp the pipeline version that produced this couldn't-extract verdict so
    // the re-arm gate (`terminal_run_should_rearm`) retries the period exactly
    // once per capability upgrade, not on every sweep pass. No schema migration:
    // `pipelineVersion` is a JSON field, tolerantly read (missing = version 0).
    let delta = serde_json::json!({
        "extractionAvailable": false,
        "reason": reason,
        "pipelineVersion": crate::jobs::structured_extraction::EXTRACTION_PIPELINE_VERSION,
    });
    let _ = state
        .autopilot()
        .set_kpi_delta_json(&run.id, &delta.to_string());
    Ok(())
}

/// Why a run could not produce KPI facts, as a **typed code** rather than an
/// English sentence (ADR 0084 decision 6).
///
/// `compose_summary` used to hardcode "KPI extraction unavailable (no AI
/// provider configured)" for every cause, which misdiagnosed a real quota
/// exhaustion as a missing configuration during owner dogfooding (2026-07-19).
/// The backend now emits the code and the frontend renders it through the
/// translation layer, so distinct causes can never collapse into one wrong
/// sentence again.
///
/// After the AI retirement the only cause this app can still produce is
/// [`Self::NoDeterministicTier`]; the provider-shaped variants remain so that
/// **stored** AI-era deltas (which are user data and stay readable, ADR 0084
/// decision 5) keep reporting their original, distinguishable cause.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum KpiUnavailableReason {
    /// A third-party quota/rate limit was exhausted (historical runs only).
    QuotaExhausted,
    /// No provider was configured for the capability (historical runs only).
    ProviderNotConfigured,
    /// The provider was reached but failed (historical runs only).
    ProviderError,
    /// No deterministic tier could parse the document — a live cause.
    NoDeterministicTier,
    /// The aggregator witness sourced this period's figures because no issuer
    /// tier could read the filing — a live cause, distinct from a plain
    /// no-tier gap so the notification names the real reason (ADR 0085 / C1).
    WitnessFallback,
    /// The document is a raw PDF — machine fact-reading is retired by design
    /// (ADR 0086 dec. 1), so the gap is EXPECTED: core KPIs arrive from the
    /// BiznesRadar-primary daily pull. Distinct from `NoDeterministicTier`
    /// so the Today card never frames a by-design gap as a per-report failure
    /// (review 2026-07-22).
    PdfDocument,
}

impl KpiUnavailableReason {
    pub(crate) fn as_str(self) -> &'static str {
        match self {
            Self::QuotaExhausted => "quota_exhausted",
            Self::ProviderNotConfigured => "provider_not_configured",
            Self::ProviderError => "provider_error",
            Self::NoDeterministicTier => "no_deterministic_tier",
            Self::WitnessFallback => "witness_fallback",
            Self::PdfDocument => "pdf_document",
        }
    }

    /// Map a `reason` string stored in a run's `kpi_delta_json` onto a typed
    /// code. Covers the AI-era vocabulary so historical runs keep an honest,
    /// distinguishable diagnosis instead of being re-labelled after the fact.
    pub(crate) fn from_delta_reason(reason: &str) -> Self {
        match reason {
            "quota_exhausted" => Self::QuotaExhausted,
            "provider_not_configured" | "no_vision_provider" => Self::ProviderNotConfigured,
            // The aggregator sourced this period; distinct from a plain no-tier gap
            // so the run reports its real cause and re-arm stays honest (C1).
            "witness_fallback" => Self::WitnessFallback,
            "pdf_document" => Self::PdfDocument,
            // AI-era codes were stored as `provider_error:<code>`.
            other if other.starts_with("provider_error") => Self::ProviderError,
            _ => Self::NoDeterministicTier,
        }
    }
}

/// Compose the `extractionAvailable:true` `kpi_delta_json` for an emitting run —
/// facts produced by a deterministic tier. The caller records produced facts +
/// merges any structure-changed flag; this only builds the counts (bug e77a1a2:
/// normalized so every tier reports `factsProposed`/`factsAutoConfirmed`
/// identically).
fn emitted_extract_delta(
    result: &crate::jobs::structured_extraction::StructuredExtractionResult,
    mode: &str,
) -> serde_json::Value {
    let produced = result.produced_fact_ids.len();
    let auto_confirmed = structured_facts_auto_confirmed(result.acceptance, produced, mode);
    serde_json::json!({
        "extractionAvailable": true,
        // Every emitting tier is deterministic now (ADR 0084 decision 4).
        "structured": true,
        "tier": result.tier.map(|t| t.as_str()),
        "produced": produced,
        "factsProposed": produced,
        "factsAutoConfirmed": auto_confirmed,
        "mode": mode,
    })
}

/// Attempts structured-first extraction for a document eligible for it (ADR
/// 0061 dec. 3/8/9): a tagged ESEF/iXBRL `.xhtml` filing, or a PDF whose
/// reporting period can be derived from its title/URL. Returns `Ok(None)` when
/// the document is not eligible (unparsable ESEF, or a PDF whose period can't
/// be classified) — an honest gap the caller flags. Runs in **both**
/// trust-ladder modes — [`crate::jobs::structured_extraction::
/// run_structured_extraction`] derives the per-fact confirmation state from
/// `run.mode` and the pipeline's acceptance. The period derivation (ESEF vs
/// PDF title/URL) lives in [`crate::jobs::structured_extraction::
/// derive_report_period`], shared with the on-demand "Extract data" command.
fn try_structured_extraction(
    state: &AppState,
    run: &storage::AutopilotRun,
) -> Result<Option<crate::jobs::structured_extraction::StructuredExtractionResult>, String> {
    let document = state
        .get_report_document(&run.report_document_id)
        .map_err(|e| e.to_string())?;
    // Period derivation is shared with the on-demand "Extract data" command so
    // the two paths never drift (`derive_report_period`). `None` → not eligible
    // for the deterministic path.
    let Some((fiscal_year, period_type, period_end)) =
        crate::jobs::structured_extraction::derive_report_period(state, &document)
    else {
        return Ok(None);
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

    // J4 (ADR 0071): a frozen, unresolved expectation for this occurrence means
    // the user wrote down what they expected and can now review it vs actuals.
    // Listing freezes-on-read (facts already landed in stage_extract), so a
    // frozen+unresolved row is precisely the reviewable set. Decision-support
    // only — a count to nudge review, never a score of the user's judgment.
    let expectations_to_review = state
        .report_expectations()
        .list_report_expectations(storage::ListReportExpectationsInput {
            company_id: Some(run.company_id.clone()),
        })
        .map(|expectations| {
            expectations
                .iter()
                .filter(|e| e.frozen_at.is_some() && e.resolved_at.is_none())
                .count()
        })
        .unwrap_or(0);

    let cross_refs = serde_json::json!({
        "claimsOverdue": claims.overdue.len(),
        "claimsDue": claims.due.len(),
        "openQuestions": open_questions,
        "expectationsToReview": expectations_to_review,
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

    // Inline attention-rule evaluation (ADR 0068 / plan §T2): a completed run
    // fires any `autopilot_run_completed` alert rule scoped to the company. No
    // new worker lane; best-effort — a failure never fails the run finalize.
    if let Err(error) = state
        .attention()
        .evaluate_autopilot_completion(&run.company_id, &run.id)
    {
        log::warn!(
            "module=attention stage=autopilot_eval runId={} error={error}",
            run.id
        );
    }
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
            // ADR 0084 decision 6: emit the typed reason code, never a guessed
            // English diagnosis. The frontend renders the code through the
            // translation layer.
            let reason = delta
                .get("reason")
                .and_then(|v| v.as_str())
                .unwrap_or("no_deterministic_tier");
            parts.push(format!(
                "kpi_extraction_unavailable:{}",
                KpiUnavailableReason::from_delta_reason(reason).as_str()
            ));
        } else {
            // Normalized counts (bug e77a1a2): both extraction branches write
            // these keys now. Only emit a KPI count token when the delta actually
            // carries the counts — never fabricate "0 of 0" for a shape that never
            // reported them (that is bug e77a1a2's symptom).
            // Review-free (ADR 0086 dec. 5): facts land `confirmed` in BOTH
            // modes, so both emit the same `kpi_confirmed` token — there is no
            // `kpi_pending`/awaiting-confirmation semantics anymore.
            let proposed = delta.get("factsProposed").and_then(|v| v.as_u64());
            let confirmed = delta.get("factsAutoConfirmed").and_then(|v| v.as_u64());
            if let (Some(confirmed), Some(proposed)) = (confirmed, proposed) {
                parts.push(format!("kpi_confirmed:{confirmed}:{proposed}"));
            }
        }
    }

    if run.report_diff_ref.is_some() {
        parts.push("report_diff_available".to_owned());
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
            parts.push(format!("claims_to_verify:{to_verify}"));
        }
        if questions > 0 {
            parts.push(format!("research_questions:{questions}"));
        }
        // J4 (ADR 0071): the user recorded expectations for this occurrence —
        // nudge them to review vs actuals (they record their own verdict).
        if refs
            .get("expectationsToReview")
            .and_then(|v| v.as_u64())
            .unwrap_or(0)
            > 0
        {
            parts.push("expectations_to_review".to_owned());
        }
    }

    // ADR 0084 decision 6 (completed 2026-07-21): the stored summary is a typed
    // token stream — NO user-visible English prose. The frontend translates each
    // token through the locale layer (`renderAutopilotSummaryTokens`); an
    // unrecognized/legacy summary passes through verbatim. Tokens join with "; ".
    if parts.is_empty() {
        "report_processed".to_owned()
    } else {
        parts.join("; ")
    }
}

/// The result of a single [`enqueue_extraction_run`] call, for callers that want
/// to count outcomes (e.g. the history sweep's budget/coverage bookkeeping, T3.2).
/// The detection sweep ignores it — its behavior is the side effects alone.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum EnqueueExtractionOutcome {
    /// A fresh run was inserted and its first stage armed.
    Created,
    /// An existing non-terminal run was found and its current stage re-armed.
    Rearmed,
    /// An existing terminal run (`succeeded`/`partial`/`failed`-with-facts) — no
    /// action; re-extracting a finished report would be wasted work.
    DedupedTerminal,
    /// A storage error prevented enqueuing; logged and skipped (best-effort,
    /// mirroring the detection sweep's warn-and-continue on a per-report failure).
    Failed,
}

/// Enqueue a full autopilot run for one `(company, report document)`, idempotently
/// (ADR 0077 §3). This is the shared entry point both the detection sweep and the
/// history sweep (T3.2) drive, so the two can never build a run differently: the
/// run id is deterministic (`autopilot_run:{company}:{document}`),
/// `create_run_if_absent` dedups, and a fresh run has its first stage armed.
///
/// `trigger` is passed straight through to the run row; the function is
/// trigger-agnostic (the DB CHECK constraint on `autopilot_run.trigger` is what
/// bounds the allowed values). `mode` records the run's trust-ladder mode.
pub(crate) fn enqueue_extraction_run(
    state: &AppState,
    company_id: &str,
    document_id: &str,
    trigger: &str,
    mode: &str,
    sweep_id: Option<&str>,
) -> EnqueueExtractionOutcome {
    let run_id = format!("autopilot_run:{company_id}:{document_id}");
    match state.autopilot().create_run_if_absent(
        &run_id,
        company_id,
        document_id,
        trigger,
        mode,
        sweep_id,
    ) {
        Ok(Some(run)) => {
            log::info!(
                "autopilot: enqueuing extraction run {} for {company_id} (trigger={trigger})",
                run.id
            );
            enqueue_first_stage(state, &run.id);
            EnqueueExtractionOutcome::Created
        }
        Ok(None) => {
            // Already has a run for this (company, document) -- dedup. But a
            // non-terminal run's current-stage job may never have actually been
            // armed (bug dce9ce8): a stale `job_queue` row left `succeeded` by
            // an unrelated prior life of the same deterministic stage id made
            // the original `enqueue_stage` call a silent no-op, so `run_stage`
            // was never invoked and the run stuck at pending/fetch forever with
            // no later event to retry it. Re-arm on every enqueue instead of only
            // at creation: safe even for a genuinely in-flight run, since
            // `enqueue_stage`/`reschedule` leaves a `running` row untouched and
            // resetting an already-`pending` row to `pending` is a no-op.
            match state.autopilot().get_run(&run_id) {
                Ok(existing) if matches!(existing.status.as_str(), "pending" | "running") => {
                    enqueue_stage(state, &existing.id, &existing.stage);
                    EnqueueExtractionOutcome::Rearmed
                }
                // A capability upgrade (the tier-3b positional tier) or a fresh
                // sweep budget can now reach a document a prior pipeline version
                // concluded it could not extract (ADR 0077 §3, 2026-07-10). Re-arm
                // the terminal run instead of skipping it forever — otherwise the
                // dedup makes such a period permanently blind to every later
                // pipeline version.
                Ok(existing) if terminal_run_should_rearm(state, &existing) => {
                    if let Err(error) = state.autopilot().rearm_run(&existing.id, trigger, sweep_id)
                    {
                        log::warn!("autopilot: re-arm terminal run failed for {run_id}: {error}");
                        return EnqueueExtractionOutcome::Failed;
                    }
                    log::info!(
                        "autopilot: re-arming terminal run {run_id} — now extractable (trigger={trigger})"
                    );
                    enqueue_first_stage(state, &existing.id);
                    EnqueueExtractionOutcome::Rearmed
                }
                Ok(_) => EnqueueExtractionOutcome::DedupedTerminal,
                Err(error) => {
                    log::warn!("autopilot: get run failed for {run_id}: {error}");
                    EnqueueExtractionOutcome::Failed
                }
            }
        }
        Err(error) => {
            log::warn!("autopilot: create run failed for {company_id}: {error}");
            EnqueueExtractionOutcome::Failed
        }
    }
}

/// Whether a TERMINAL run should be RE-ARMED for a fresh extraction attack rather
/// than deduped (ADR 0077 §3, 2026-07-10). Only a **succeeded** run that recorded
/// `extractionAvailable:false` with a re-arm-class reason qualifies — a run that
/// emitted facts (`extractionAvailable:true`), or is `partial`/`failed`, is never
/// re-armed. The re-arm classes:
///
/// A couldn't-extract verdict is re-armed **iff (a) the pipeline's capability
/// version advanced since the run recorded its verdict AND (b) the document is
/// now extractable** ([`history_sweep::document_is_extractable`], reused not
/// duplicated). `document_is_extractable` is constant-true for any well-formed
/// PDF, so on its own it re-armed every flagged period on every sweep pass,
/// forever (the extraction storm, owner dogfooding 2026-07-21). The version gate
/// —`stored_pipeline_version(run) < EXTRACTION_PIPELINE_VERSION`— is what makes a
/// capability upgrade (a newly landed/changed deterministic tier) retry a period
/// **once**: after the re-run stamps the current version the period settles. A
/// still-dead file (unreadable/zero-byte, or one no tier can parse) stays
/// deduped regardless. Manual per-period retry ("Try again" /
/// `rerun_extraction_outcome`) does NOT route through here — it calls
/// `run_structured_extraction` directly and stays unconditional.
///
/// The AI-era reasons (`no_vision_provider`, `skipped_budget`) are no longer
/// produced (ADR 0084) but remain readable on stored runs; they re-arm on the
/// same extractability test as any other gap — the retired provider budget no
/// longer gates anything.
///
/// Deterministic-emitted outcomes (`extractionAvailable:true`) return `None` from
/// [`extraction_unavailable_reason`] and are therefore never re-armed.
fn terminal_run_should_rearm(state: &AppState, run: &storage::AutopilotRun) -> bool {
    if run.status != "succeeded" {
        return false;
    }
    let Some(reason) = extraction_unavailable_reason(run.kpi_delta_json.as_deref()) else {
        return false;
    };
    // VERSION GATE (owner dogfooding 2026-07-21): a couldn't-extract verdict only
    // becomes stale when the pipeline gains the ability to read a document it
    // previously could not — signalled by a bump of
    // `EXTRACTION_PIPELINE_VERSION`. Re-arm ONLY when this build's version is
    // newer than the one the run recorded; once the re-run stamps the current
    // version, the next enqueue dedups. Without this gate,
    // `document_is_extractable` is constant-true for any well-formed PDF, so
    // every sweep pass re-armed every flagged period forever (attempt_count
    // reached ~1100+ in a day). Missing field = version 0 (pre-versioning era) →
    // eligible for exactly one re-arm under the current build.
    if stored_pipeline_version(run.kpi_delta_json.as_deref())
        >= crate::jobs::structured_extraction::EXTRACTION_PIPELINE_VERSION
    {
        return false;
    }
    match reason.as_str() {
        // `witness_fallback`: the aggregator sourced this period but no issuer
        // tier could read the filing. Re-arm on the same extractability test as
        // any other gap, so a later parser fix re-extracts it with real issuer
        // data instead of leaving the period permanently on third-party numbers
        // (ADR 0085 amendment / C1).
        "no_deterministic_tier"
        | "witness_fallback"
        | "not_extractable"
        | "not_pdf"
        | "no_stored_file"
        | "skipped_budget"
        | "no_vision_provider" => document_now_extractable(state, &run.report_document_id),
        // `pdf_document` is the BY-DESIGN gap (ADR 0086 dec. 1): machine
        // fact-reading of PDFs is retired, so no capability upgrade ever makes
        // the document extractable — never re-armed (falls through to false).
        _ => false,
    }
}

/// The `reason` a run recorded when it could not extract — `Some(reason)` only when
/// `kpi_delta_json` decodes to an object with `extractionAvailable == false`. An
/// emitting run, a missing/garbled delta, or a delta with no `reason` returns
/// `None` (never re-armed).
fn extraction_unavailable_reason(kpi_delta_json: Option<&str>) -> Option<String> {
    let delta = serde_json::from_str::<serde_json::Value>(kpi_delta_json?).ok()?;
    if delta.get("extractionAvailable").and_then(|v| v.as_bool()) != Some(false) {
        return None;
    }
    delta
        .get("reason")
        .and_then(|reason| reason.as_str())
        .map(str::to_owned)
}

/// The `EXTRACTION_PIPELINE_VERSION` a run stamped into its `kpi_delta_json` when
/// it recorded a couldn't-extract verdict. A missing/garbled delta or an absent
/// `pipelineVersion` field reads as `0` — the pre-versioning era — so a legacy
/// run is eligible for exactly one re-arm under the current build (see the
/// version gate in [`terminal_run_should_rearm`]).
fn stored_pipeline_version(kpi_delta_json: Option<&str>) -> u32 {
    kpi_delta_json
        .and_then(|json| serde_json::from_str::<serde_json::Value>(json).ok())
        .and_then(|delta| delta.get("pipelineVersion").and_then(|v| v.as_u64()))
        .map(|v| v.min(u64::from(u32::MAX)) as u32)
        .unwrap_or(0)
}

/// Whether the run's document could now be read by SOME tier — the shared
/// `history_sweep::document_is_extractable` test. A load failure means the document
/// is gone/unreadable → not extractable (stay deduped).
fn document_now_extractable(state: &AppState, report_document_id: &str) -> bool {
    match state.get_report_document(report_document_id) {
        Ok(document) => crate::jobs::history_sweep::document_is_extractable(state, &document),
        Err(_) => false,
    }
}

/// The typed reason for a run that produced no facts: a **genuine** PDF document
/// is the by-design `pdf_document` gap (ADR 0086 dec. 1 — never re-armed,
/// rendered as "core KPIs arrive from the aggregator"); anything else keeps the
/// caller's fallback (an honest `no_deterministic_tier`, which stays re-armable).
///
/// Resolution reads the stored `detected_container` (epic #229 T2), no byte read.
/// Only a document whose *bytes* are a PDF earns the never-re-armed verdict — an
/// XML or ZIP under a `.pdf` name has a deterministic tier that can still read it,
/// and burying it under `pdf_document` would retire it permanently by mistake.
fn gap_reason(
    state: &AppState,
    run: &storage::AutopilotRun,
    fallback: &'static str,
) -> &'static str {
    let is_pdf = state
        .get_report_document(&run.report_document_id)
        .ok()
        .is_some_and(|document| {
            document.local_path.is_some()
                && crate::report_documents_container::is_real_pdf(&document)
        });
    if is_pdf {
        KpiUnavailableReason::PdfDocument.as_str()
    } else {
        fallback
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
            // Shared enqueue path (T3.1): identical run-id, dedup, and dce9ce8
            // re-arm semantics whether the trigger is detection or the history
            // sweep. The sweep ignores the outcome — its behavior is the side
            // effects alone, exactly as before the extraction.
            // A detection run belongs to no sweep, so it charges no sweep budget.
            enqueue_extraction_run(state, &company_id, &document.id, "detection", &mode, None);
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

/// Whether a report document is a **structured** statement — an ESEF/iXBRL
/// markup instance or an ESEF/eSprawozdanie report package (ZIP) — rather than a
/// PDF. Used to break a disclosure-date tie in [`prefers_candidate`] and, via the
/// coverage read model (ADR 0077 §2), the canonical-report structured tie-break.
/// `pub(crate)` so the coverage command reuses this exact definition instead of
/// re-deriving it (F3 decides the final home).
///
/// Container truth decides it (epic #229 T2): the maintainer's corpus stores 38
/// XML statements under a `.pdf` name, and the old name-based resolution ranked
/// every one of them *below* a companion PDF — handing the canonical slot to the
/// document with less extractable data. A ZIP counts as structured because the
/// structured path unpacks its inner iXBRL instance.
pub(crate) fn is_structured_document(document: &storage::ReportDocument) -> bool {
    use crate::fundamentals::extraction::container::Container;
    use crate::report_documents_container::resolved_container_named;

    // The URL is this predicate's name carrier for a never-sniffed row: the
    // tie-break ranks candidate documents that may not be fetched yet, so it must
    // not depend on a stored file existing.
    matches!(
        resolved_container_named(document, &document.url),
        Container::Xml | Container::Html | Container::Zip
    )
}

/// A sortable **disclosure-date** key (`YYYY-MM-DD`) for ranking report recency —
/// the domain date, not `created_at`/ingestion order ([data-model.md] Model
/// Principles; guardrail `d60305c`). The accepted ESPI/EBI attachment sources embed
/// the disclosure month in the URL as `/emitent/YYYY-MM/`; use it (day `01`, which
/// is enough for the quarterly cadence detection ranks). Falls back to `fetched_at`,
/// then `created_at` only as a last resort (a non-`emitent`, never-fetched doc).
/// `pub(crate)` so the coverage read model (ADR 0077 §2) ranks canonical-report
/// revisions with the identical disclosure semantics (F3 decides the final home).
///
/// **The month segment survives a misleading slug** (epic #229 T3, #140). The
/// attachment host reuses one issuer's *filename* across unrelated filings, so a
/// slug can name a company that is not the owner — but the `/emitent/YYYY-MM/`
/// segment is the **article's** publication month, not the filename's, and stays
/// correct. Measured on the maintainer's corpus: all 53 rows whose slug names a
/// foreign tracked issuer carry the right month for their own filing (e.g.
/// cyber_Folks' H1-2024 statements under a `Vercom` filename at `/2024-09/`,
/// Orlen's Q3-2024 report under a `Grupy-Energa` filename at `/2024-11/`). The
/// distrust this epic ships therefore targets the **filename** — see
/// [`crate::fundamentals::extraction::classify::classify_doc_kind`] — and
/// deliberately NOT this date, whose only fallback is a bulk re-fetch timestamp
/// identical across every revision.
pub(crate) fn report_disclosure_key(document: &storage::ReportDocument) -> String {
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
    // MODE_AUTOPILOT is now test-only (production no longer branches on mode for
    // confirmation state — facts are review-free, ADR 0086 dec. 5).
    use crate::storage::{
        open_in_memory_database, CaptureReportDocumentInput, NewCompany, MODE_AUTOPILOT,
    };

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

    /// A fetcher failing with a **real** `reqwest` network error (the only way
    /// to construct the `Request` variant): an instant local connection
    /// failure — hermetic, no external network.
    struct TransientFailingFetcher;

    impl crate::document_fetcher::DocumentFetcher for TransientFailingFetcher {
        fn fetch(
            &self,
            _url: &str,
        ) -> Result<
            crate::document_fetcher::FetchedDocument,
            crate::document_fetcher::DocumentFetcherError,
        > {
            let error = reqwest::blocking::Client::builder()
                .timeout(std::time::Duration::from_millis(250))
                .build()
                .expect("client builds")
                .get("http://127.0.0.1:9/refused")
                .send()
                .expect_err("nothing listens on the discard port");
            Err(crate::document_fetcher::DocumentFetcherError::Request(
                error,
            ))
        }
    }

    /// A run whose document is still pending fetch (no stored file), so the
    /// fetch stage must go through the injected fetcher.
    fn pending_fetch_run(state: &AppState, run_id: &str) {
        let company = state
            .create_company(NewCompany {
                exchange: "GPW".to_owned(),
                ticker: "TST".to_owned(),
                display_name: "Transient Test S.A.".to_owned(),
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
                title: Some("Pending report".to_owned()),
                attribution: None,
            })
            .expect("document");
        state
            .autopilot()
            .create_run_if_absent(
                run_id,
                &company.id,
                &document.id,
                "manual",
                MODE_AUTOPILOT,
                None,
            )
            .expect("create run")
            .expect("run created");
    }

    fn fetch_payload(run_id: &str) -> String {
        format!(r#"{{"run_id":"{run_id}","stage":"{STAGE_FETCH}"}}"#)
    }

    /// #189 / ADR 0055 dec. 2: a network-level fetch failure with attempts left
    /// returns `Err` (the durable queue retries with backoff) and does NOT
    /// finalize the run — and the stage job is armed with retries at all.
    #[test]
    fn transient_fetch_failure_goes_back_to_the_queue_and_keeps_the_run_alive() {
        let state = AppState::new(open_in_memory_database().expect("db"));
        let run_id = "run_transient";
        pending_fetch_run(&state, run_id);
        enqueue_stage(&state, run_id, STAGE_FETCH);
        let job = state.jobs().claim_next().expect("claim").expect("a job");
        assert_eq!(job.kind, AUTOPILOT_STAGE_KIND);
        let row = state
            .jobs()
            .status(&stage_job_id(run_id, STAGE_FETCH))
            .expect("status")
            .expect("job row");
        assert_eq!(
            row.max_attempts, STAGE_MAX_ATTEMPTS,
            "stage jobs must arm queue retries (reddens on a revert to 1)"
        );

        let result =
            run_stage_with_fetcher(&state, &fetch_payload(run_id), &TransientFailingFetcher);

        assert!(
            result.is_err(),
            "transient failure must go back to the queue"
        );
        let run = state.autopilot().get_run(run_id).expect("run");
        assert_ne!(run.status, "failed", "run must stay alive for the retry");
    }

    /// #189: the last allowed attempt of a transient failure finalizes the run
    /// as failed (still notified) instead of stranding it over a dead job.
    #[test]
    fn transient_fetch_failure_on_the_last_attempt_finalizes_the_run() {
        let state = AppState::new(open_in_memory_database().expect("db"));
        let run_id = "run_exhausted";
        pending_fetch_run(&state, run_id);
        enqueue_stage(&state, run_id, STAGE_FETCH);
        // Burn all but the last attempt the way the worker would.
        for _ in 0..(STAGE_MAX_ATTEMPTS - 1) {
            let job = state.jobs().claim_next().expect("claim").expect("a job");
            assert!(
                state
                    .jobs()
                    .mark_failed(&job.id, "transient blip", 0)
                    .expect("mark failed"),
                "non-final attempts stay retryable"
            );
        }
        let _last = state.jobs().claim_next().expect("claim").expect("a job");

        let result =
            run_stage_with_fetcher(&state, &fetch_payload(run_id), &TransientFailingFetcher);

        assert!(result.is_ok(), "exhausted attempt must not loop the job");
        let run = state.autopilot().get_run(run_id).expect("run");
        assert_eq!(run.status, "failed", "the run is honestly finalized");
    }

    /// #189: a fatal (non-network) fetch failure still finalizes immediately,
    /// even with queue attempts remaining — retrying a domain failure is waste.
    #[test]
    fn fatal_fetch_failure_finalizes_immediately_despite_remaining_attempts() {
        let state = AppState::new(open_in_memory_database().expect("db"));
        let run_id = "run_fatal";
        pending_fetch_run(&state, run_id);
        enqueue_stage(&state, run_id, STAGE_FETCH);
        let _job = state.jobs().claim_next().expect("claim").expect("a job");

        let fetcher = crate::document_fetcher::FakeDocumentFetcher::new_error(
            crate::document_fetcher::DocumentFetcherError::InvalidContentType("boom".to_owned()),
        );
        let result = run_stage_with_fetcher(&state, &fetch_payload(run_id), &fetcher);

        assert!(result.is_ok(), "fatal failure must not be retried");
        let run = state.autopilot().get_run(run_id).expect("run");
        assert_eq!(run.status, "failed");
    }

    #[test]
    fn last_attempt_exhaustion_matrix() {
        assert!(!last_attempt_exhausted(1, STAGE_MAX_ATTEMPTS));
        assert!(!last_attempt_exhausted(2, STAGE_MAX_ATTEMPTS));
        assert!(last_attempt_exhausted(3, STAGE_MAX_ATTEMPTS));
    }

    /// ADR 0061: in autopilot mode a tagged ESEF filing is extracted
    /// deterministically before AI — facts land `confirmed` (review-free, ADR
    /// 0086 dec. 5) with `esef`/`passed` provenance, and the AI path is skipped.
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
            .create_run_if_absent(
                run_id,
                &company.id,
                &document.id,
                "manual",
                MODE_AUTOPILOT,
                None,
            )
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

        // Review-free (ADR 0086 dec. 5): every emitted fact lands `confirmed` —
        // no `auto_unreviewed`/`pending` awaiting-confirmation state survives.
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

        // The composed notification summary itself must reflect the honest count,
        // not "0 KPI auto-confirmed (unreviewed) of 0 extracted" — the exact
        // real-world symptom of bug e77a1a2.
        let summary = compose_summary(&after);
        assert!(
            summary.contains("kpi_confirmed:3:3"),
            "summary must carry the honest typed count token: {summary}"
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
            .create_run_if_absent(run_id, &company.id, "doc1", "manual", MODE_AUTOPILOT, None)
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
            summary.contains("kpi_confirmed:40:40"),
            "summary must carry the honest typed count token: {summary}"
        );
        // ADR 0084 dec 6: no user-visible English prose in the stored summary.
        assert!(
            !summary.contains("auto-confirmed"),
            "English prose leaked into the typed summary: {summary}"
        );
    }

    /// ADR 0084 decision 6 (completion) — `compose_summary` emits a **typed token
    /// stream** for every fragment, not just the extraction-unavailable branch.
    /// A run with KPI counts, claims-to-verify and open questions must serialize
    /// as machine tokens the frontend translates, with NO user-visible English
    /// prose reaching the stored summary (the misdiagnosis class the dogfooding
    /// screenshot exposed, generalized to every fragment).
    #[test]
    fn compose_summary_emits_only_typed_tokens_with_no_english_prose() {
        let connection = open_in_memory_database().expect("db");
        let state = AppState::new(connection);
        let company = state
            .create_company(NewCompany {
                exchange: "GPW".to_owned(),
                ticker: "TOK".to_owned(),
                display_name: "Tokens S.A.".to_owned(),
                isin: None,
                cik: None,
                lei: None,
            })
            .expect("company");
        let run_id = "run_all_tokens";
        state
            .autopilot()
            .create_run_if_absent(
                run_id,
                &company.id,
                "docTok",
                "manual",
                MODE_AUTOPILOT,
                None,
            )
            .expect("create run")
            .expect("run created");
        state
            .autopilot()
            .set_kpi_delta_json(
                run_id,
                &serde_json::json!({
                    "extractionAvailable": true,
                    "structured": true,
                    "factsProposed": 7,
                    "factsAutoConfirmed": 7,
                })
                .to_string(),
            )
            .expect("set delta");
        state
            .autopilot()
            .set_cross_refs_json(
                run_id,
                &serde_json::json!({
                    "claimsOverdue": 2,
                    "claimsDue": 1,
                    "openQuestions": 3,
                    "expectationsToReview": 0,
                })
                .to_string(),
            )
            .expect("set cross refs");
        let run = state.autopilot().get_run(run_id).expect("get run");

        let summary = compose_summary(&run);
        // Every fragment is a typed token.
        assert!(summary.contains("kpi_confirmed:7:7"), "summary: {summary}");
        assert!(summary.contains("claims_to_verify:3"), "summary: {summary}");
        assert!(
            summary.contains("research_questions:3"),
            "summary: {summary}"
        );
        // No user-visible English prose fragment may reach the stored summary.
        for prose in [
            "auto-confirmed",
            "to verify",
            "open research question",
            "New report processed",
            "extracted",
        ] {
            assert!(
                !summary.contains(prose),
                "English prose {prose:?} leaked into the typed summary: {summary}"
            );
        }
    }

    /// ADR 0085 / C1 — a witness-fallback gap (the aggregator sourced the period
    /// because no issuer tier could read the filing) must surface its OWN typed
    /// code, never collapse into `no_deterministic_tier`. Before this the run's
    /// notification lied about its cause.
    #[test]
    fn compose_summary_maps_witness_fallback_to_its_own_typed_code() {
        let connection = open_in_memory_database().expect("db");
        let state = AppState::new(connection);
        let company = state
            .create_company(NewCompany {
                exchange: "GPW".to_owned(),
                ticker: "WFB".to_owned(),
                display_name: "Witness Fallback S.A.".to_owned(),
                isin: None,
                cik: None,
                lei: None,
            })
            .expect("company");
        let run_id = "run_witness_fallback";
        state
            .autopilot()
            .create_run_if_absent(
                run_id,
                &company.id,
                "docWfb",
                "manual",
                MODE_AUTOPILOT,
                None,
            )
            .expect("create run")
            .expect("run created");
        state
            .autopilot()
            .set_kpi_delta_json(
                run_id,
                &serde_json::json!({
                    "extractionAvailable": false,
                    "reason": "witness_fallback",
                })
                .to_string(),
            )
            .expect("set delta");
        let run = state.autopilot().get_run(run_id).expect("get run");

        let summary = compose_summary(&run);
        assert!(
            summary.contains("kpi_extraction_unavailable:witness_fallback"),
            "witness fallback must surface its own typed code: {summary}"
        );
        assert!(
            !summary.contains("no_deterministic_tier"),
            "witness fallback must not collapse into no_deterministic_tier: {summary}"
        );
    }

    /// ADR 0084 decision 6 — honest failure reporting. `compose_summary` used to
    /// hardcode the English sentence "KPI extraction unavailable (no AI provider
    /// configured)" for *any* unavailability, which misdiagnosed a real quota
    /// exhaustion as a missing configuration during owner dogfooding
    /// (2026-07-19). The summary must now carry a **typed reason code** from the
    /// fixed vocabulary, and distinct causes must stay distinguishable — never
    /// collapsed into one. Rendering the code into a sentence is the frontend's
    /// job (the v0.60.0 Today seam); the backend emits typed data.
    #[test]
    fn compose_summary_emits_typed_reason_codes_that_stay_distinguishable() {
        let connection = open_in_memory_database().expect("db");
        let state = AppState::new(connection);
        let company = state
            .create_company(NewCompany {
                exchange: "GPW".to_owned(),
                ticker: "RSN".to_owned(),
                display_name: "Reason Codes S.A.".to_owned(),
                isin: None,
                cik: None,
                lei: None,
            })
            .expect("company");

        // (stored delta reason, expected typed code). The AI-era stored reasons
        // remain readable (ADR 0084 decision 5) and each maps to a typed code;
        // after the retirement the live one is `no_deterministic_tier`.
        let cases: [(&str, &str); 6] = [
            ("no_deterministic_tier", "no_deterministic_tier"),
            ("not_extractable", "no_deterministic_tier"),
            ("no_vision_provider", "provider_not_configured"),
            ("quota_exhausted", "quota_exhausted"),
            ("provider_error", "provider_error"),
            // The by-design PDF gap (ADR 0086 dec. 1) keeps its own code.
            ("pdf_document", "pdf_document"),
        ];

        let mut emitted: Vec<String> = Vec::new();
        for (index, (stored_reason, expected_code)) in cases.into_iter().enumerate() {
            let run_id = format!("run_reason_{index}");
            // Distinct document id per run: `create_run_if_absent` dedups on
            // (company, document), so reusing one id would return None after the
            // first insert.
            let document_id = format!("doc_{index}");
            state
                .autopilot()
                .create_run_if_absent(
                    &run_id,
                    &company.id,
                    &document_id,
                    "manual",
                    MODE_AUTOPILOT,
                    None,
                )
                .expect("create run")
                .expect("run created");
            let delta = serde_json::json!({
                "extractionAvailable": false,
                "reason": stored_reason,
            });
            state
                .autopilot()
                .set_kpi_delta_json(&run_id, &delta.to_string())
                .expect("set delta");
            let run = state.autopilot().get_run(&run_id).expect("get run");

            let summary = compose_summary(&run);
            assert!(
                summary.contains(expected_code),
                "stored reason {stored_reason} must surface the typed code \
                 {expected_code}, got: {summary}"
            );
            assert!(
                !summary.contains("no AI provider configured"),
                "the guessed English diagnosis must be gone, got: {summary}"
            );
            emitted.push(summary);
        }

        // The three distinct causes must not collapse into one string — that
        // collapse is the exact defect this test pins.
        assert_ne!(
            emitted[0], emitted[2],
            "a missing deterministic tier and an exhausted quota must stay distinguishable"
        );
        assert_ne!(
            emitted[2], emitted[3],
            "an exhausted quota and a provider error must stay distinguishable"
        );
        assert_ne!(
            emitted[1], emitted[3],
            "an unconfigured provider and a provider error must stay distinguishable"
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
            doc_kind: None,
            detected_container: None,
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

    /// Helper: a company × framework with one qualitative criterion already
    /// carrying a prior agent assessment (the §T6d re-enqueue precondition).
    /// Seed a financial period + one confirmed fact for `(company, 2026, H1)` so
    /// the occurrence's facts exist — the moment expectations freeze (mirrors the
    /// `storage::tests::report_expectations` helper).
    fn seed_h1_2026_facts(state: &AppState, company_id: &str) {
        let raw = state.checkout_for_tests().expect("raw connection");
        raw.execute(
            "INSERT INTO financial_periods (id, company_id, fiscal_year, period_type)
             VALUES ('p_h1', ?1, 2026, 'H1')",
            [company_id],
        )
        .expect("seed period");
        raw.execute(
            "INSERT INTO financial_facts (id, company_id, period_id, definition_id, value_numeric)
             VALUES ('f_np', ?1, 'p_h1', 'kpidef_net_profit', '120')",
            [company_id],
        )
        .expect("seed fact");
    }

    fn seed_expectation_company(state: &AppState) -> storage::Company {
        state
            .create_company(NewCompany {
                exchange: "GPW".to_owned(),
                ticker: "CDR".to_owned(),
                display_name: "CD PROJEKT S.A.".to_owned(),
                isin: None,
                cik: None,
                lei: None,
            })
            .expect("company")
    }

    /// J4 (ADR 0071): a frozen, unresolved expectation for the run's occurrence
    /// makes the cross-reference stage record an `expectationsToReview` count and
    /// the summary nudge the user to review vs actuals — decision-support, no
    /// scoring of the user's judgment (ADR 0042).
    #[test]
    fn cross_reference_links_expectation_review_when_expectations_exist() {
        let connection = open_in_memory_database().expect("db");
        let state = AppState::new(connection);
        let company = seed_expectation_company(&state);

        state
            .report_expectations()
            .create_report_expectation(storage::NewReportExpectation {
                company_id: company.id.clone(),
                event_key: "evt-h1-2026".to_owned(),
                fiscal_year: 2026,
                period_type: "H1".to_owned(),
                stance_md: "Margin recovery on the launch.".to_owned(),
                metrics: Vec::new(),
            })
            .expect("expectation");
        // The report lands: facts arrive → the expectation freezes.
        seed_h1_2026_facts(&state, &company.id);

        let run_id = "run_xref_expect";
        state
            .autopilot()
            .create_run_if_absent(run_id, &company.id, "doc1", "manual", MODE_AUTOPILOT, None)
            .expect("create run")
            .expect("run created");
        let run = state.autopilot().get_run(run_id).expect("get run");

        stage_cross_reference(&state, &run).expect("cross_reference stage");

        let run = state.autopilot().get_run(run_id).expect("get run");
        let refs: serde_json::Value =
            serde_json::from_str(run.cross_refs_json.as_deref().expect("cross_refs written"))
                .expect("cross_refs json");
        assert_eq!(
            refs.get("expectationsToReview").and_then(|v| v.as_u64()),
            Some(1),
            "a frozen, unresolved expectation for the occurrence is counted"
        );

        let summary = compose_summary(&run);
        assert!(
            summary.contains("expectations_to_review"),
            "summary should carry the typed expectations token: {summary}"
        );
    }

    /// Inverse: with no expectation for the occurrence, the cross-reference stage
    /// records a zero count and the summary carries no expectations line.
    #[test]
    fn cross_reference_omits_expectation_link_when_none_exist() {
        let connection = open_in_memory_database().expect("db");
        let state = AppState::new(connection);
        let company = seed_expectation_company(&state);

        let run_id = "run_xref_no_expect";
        state
            .autopilot()
            .create_run_if_absent(run_id, &company.id, "doc1", "manual", MODE_AUTOPILOT, None)
            .expect("create run")
            .expect("run created");
        let run = state.autopilot().get_run(run_id).expect("get run");

        stage_cross_reference(&state, &run).expect("cross_reference stage");

        let run = state.autopilot().get_run(run_id).expect("get run");
        let refs: serde_json::Value =
            serde_json::from_str(run.cross_refs_json.as_deref().expect("cross_refs written"))
                .expect("cross_refs json");
        assert_eq!(
            refs.get("expectationsToReview").and_then(|v| v.as_u64()),
            Some(0),
            "no expectation → zero count"
        );

        let summary = compose_summary(&run);
        assert!(
            !summary.contains("expectations_to_review"),
            "no expectations token when none exist: {summary}"
        );
    }

    // ---- F3b: history-sweep tier-4 budget (ADR 0077 §6) --------------------

    /// Seed a fetched, canonical periodic PDF report for `company_id` under `dir`,
    /// with prose-only content so determinism emits nothing (the run reaches
    /// tier-4). Returns the document id.
    fn seed_periodic_pdf(
        state: &AppState,
        dir: &std::path::Path,
        company_id: &str,
        title: &str,
        file: &str,
    ) -> String {
        let document = state
            .create_or_find_pending_report_document(CaptureReportDocumentInput {
                company_id: company_id.to_owned(),
                source_type: "user_url".to_owned(),
                url: format!("https://example.com/{file}"),
                period_id: None,
                origin_ref: None,
                title: Some(title.to_owned()),
                attribution: None,
            })
            .expect("document");
        let bytes = minimal_text_pdf(&["prose only"]);
        std::fs::write(dir.join(file), &bytes).expect("write pdf");
        state
            .mark_report_document_fetched(
                &document.id,
                Some(file),
                Some("application/pdf"),
                None,
                Some(bytes.len() as i64),
            )
            .expect("mark fetched");
        document.id
    }

    /// Configure the keyless `test_sample` provider as the `VisionExtraction` pool
    /// member, bypassing settings validation (test_sample is not a *selectable*
    /// provider) exactly like `build_capability_provider`'s own tests — so the full
    /// tier-4 path builds a deterministic offline OCR provider.
    /// ADR 0084 decision 4 — flagged, never silent. With the AI layer retired,
    /// a report document that **no deterministic tier can parse** must still
    /// produce a completed run carrying an unread notification whose summary
    /// names the typed `no_deterministic_tier` reason — never a silently absent
    /// run, never a guessed value, and with no AI branch anywhere in the path.
    /// Driven end-to-end through the real durable queue (no network).
    #[test]
    fn unparseable_report_is_flagged_with_a_notification_and_no_ai_branch() {
        let dir = unique_temp_dir("flagged-no-tier");
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
        state
            .autopilot()
            .set_mode(&company.id, "assist")
            .expect("assist mode");
        // A text PDF with no financial table: every deterministic tier declines.
        let document_id = seed_periodic_pdf(
            &state,
            &dir,
            &company.id,
            "Skonsolidowany raport roczny 2025 SSF",
            "unparseable.pdf",
        );

        let outcome = enqueue_extraction_run(
            &state,
            &company.id,
            &document_id,
            "detection",
            "assist",
            None,
        );
        assert_eq!(outcome, EnqueueExtractionOutcome::Created);
        crate::jobs::handlers::build_worker(state.clone())
            .run_until_idle()
            .expect("drain the queue");

        let runs = state
            .autopilot()
            .list_runs(&crate::storage::ListAutopilotRunsInput {
                company_id: Some(company.id.clone()),
                limit: Some(50),
                ..Default::default()
            })
            .expect("list runs");
        assert_eq!(runs.len(), 1, "the run must not be silently dropped");
        let run = &runs[0];

        assert_eq!(
            run.stage, STAGE_NOTIFY,
            "an unparseable document still reaches the notify stage"
        );
        assert_eq!(
            run.notification_state, "unread",
            "the gap must be surfaced as an unread notification, not swallowed"
        );
        assert!(
            run.produced_fact_ids.is_empty(),
            "nothing may be guessed for a document no tier parsed"
        );

        let delta = run
            .kpi_delta_json
            .as_deref()
            .expect("a non-emitting run still records its honest delta");
        assert!(
            delta.contains("\"extractionAvailable\":false"),
            "delta: {delta}"
        );
        assert!(
            // A raw-PDF document reports the BY-DESIGN `pdf_document` gap
            // (ADR 0086 dec. 1, review 2026-07-22) — the honest replacement for
            // the generic no_deterministic_tier this test originally pinned.
            delta.contains("pdf_document"),
            "the delta must carry the typed reason code, got: {delta}"
        );
        assert!(
            !delta.contains("tier4") && !delta.contains("vision"),
            "no AI/tier-4 branch may appear in the path, got: {delta}"
        );

        let summary = run
            .summary_text
            .as_deref()
            .expect("the notification must carry a summary");
        assert!(
            summary.contains("pdf_document"),
            "the notification must name the typed reason, got: {summary}"
        );
        assert!(
            !summary.contains("no AI provider configured"),
            "the retired guessed diagnosis must be gone, got: {summary}"
        );
    }

    /// Helper: seed a succeeded, couldn't-extract terminal run over a real
    /// (extractable-by-construction) PDF, with a caller-built `kpi_delta_json`.
    fn seed_terminal_unavailable_run(
        state: &AppState,
        dir: &std::path::Path,
        run_id: &str,
        delta: serde_json::Value,
    ) -> storage::AutopilotRun {
        let (company_id, document_id) = seed_pdf_report(
            state,
            dir,
            "Cyber Folks raport roczny 2025 SSF",
            &["Brak danych"],
        );
        state
            .autopilot()
            .create_run_if_absent(
                run_id,
                &company_id,
                &document_id,
                "detection",
                MODE_AUTOPILOT,
                None,
            )
            .expect("create run")
            .expect("run created");
        state
            .autopilot()
            .set_kpi_delta_json(run_id, &delta.to_string())
            .expect("set delta");
        state
            .autopilot()
            .finalize_run(run_id, "succeeded", STAGE_NOTIFY, Some("s"), None)
            .expect("finalize");
        state.autopilot().get_run(run_id).expect("get run")
    }

    /// THE STORM (owner dogfooding 2026-07-21): a couldn't-extract run whose
    /// delta already carries the CURRENT `pipelineVersion` must NOT re-arm on a
    /// subsequent enqueue — nothing about the pipeline's read capability changed,
    /// so re-running the full file IO + PDF parse with identical inputs is pure
    /// waste (attempt_count reached ~1100+ in one day). Before the version gate,
    /// `document_is_extractable` was constant-true for any PDF, so every sweep
    /// pass re-armed every flagged period forever.
    #[test]
    fn a_current_version_couldnt_extract_run_is_not_re_armed() {
        let dir = unique_temp_dir("rearm-current-version");
        std::fs::create_dir_all(&dir).expect("temp dir");
        let connection = open_in_memory_database().expect("db");
        let state = AppState::with_data_dir(connection, dir.clone());
        let delta = serde_json::json!({
            "extractionAvailable": false,
            "reason": crate::jobs::structured_extraction::reason::NO_DETERMINISTIC_TIER,
            "pipelineVersion": crate::jobs::structured_extraction::EXTRACTION_PIPELINE_VERSION,
        });
        let run = seed_terminal_unavailable_run(&state, &dir, "run_current_ver", delta);
        assert!(
            !terminal_run_should_rearm(&state, &run),
            "a run stamped with the current pipeline version must settle (no re-arm) — \
             this is the fix for the extraction storm"
        );
    }

    /// A legacy run (delta predates versioning, no `pipelineVersion`) re-arms
    /// ONCE under the new build; after the re-run records a delta stamped with
    /// the current version, the next enqueue dedups. Deterministic, no time-based
    /// backoff: the version is the only knob.
    #[test]
    fn a_legacy_run_re_arms_once_then_settles() {
        let dir = unique_temp_dir("rearm-legacy-once");
        std::fs::create_dir_all(&dir).expect("temp dir");
        let connection = open_in_memory_database().expect("db");
        let state = AppState::with_data_dir(connection, dir.clone());
        let legacy = serde_json::json!({
            "extractionAvailable": false,
            "reason": crate::jobs::structured_extraction::reason::NO_DETERMINISTIC_TIER,
        });
        let run = seed_terminal_unavailable_run(&state, &dir, "run_legacy", legacy);
        assert!(
            terminal_run_should_rearm(&state, &run),
            "a legacy (unstamped) couldn't-extract run must re-arm once under the new build"
        );

        // Simulate the re-run recording its new, stamped delta (what stage_extract
        // writes on the extractionAvailable:false path). The period must then settle.
        let stamped = serde_json::json!({
            "extractionAvailable": false,
            "reason": crate::jobs::structured_extraction::reason::NO_DETERMINISTIC_TIER,
            "pipelineVersion": crate::jobs::structured_extraction::EXTRACTION_PIPELINE_VERSION,
        });
        state
            .autopilot()
            .set_kpi_delta_json("run_legacy", &stamped.to_string())
            .expect("set stamped delta");
        let settled = state.autopilot().get_run("run_legacy").expect("get run");
        assert!(
            !terminal_run_should_rearm(&state, &settled),
            "after recording a stamped delta the period must dedup on the next enqueue"
        );
    }

    /// Regression: when the version gate is OPEN (a run stamped with a version
    /// LOWER than the current build — a genuine parser upgrade), a witness_fallback
    /// period still re-arms so the upgrade reaches it with real issuer data. The
    /// version gate must not close the door on legitimate capability upgrades.
    #[test]
    fn a_lower_version_witness_fallback_still_re_arms_on_upgrade() {
        let dir = unique_temp_dir("rearm-lower-version");
        std::fs::create_dir_all(&dir).expect("temp dir");
        let connection = open_in_memory_database().expect("db");
        let state = AppState::with_data_dir(connection, dir.clone());
        let older_version =
            crate::jobs::structured_extraction::EXTRACTION_PIPELINE_VERSION.saturating_sub(1);
        let delta = serde_json::json!({
            "extractionAvailable": false,
            "reason": "witness_fallback",
            "pipelineVersion": older_version,
        });
        let run = seed_terminal_unavailable_run(&state, &dir, "run_lower_ver", delta);
        assert!(
            terminal_run_should_rearm(&state, &run),
            "a parser upgrade (higher current version) must still reach a lower-version \
             witness_fallback period"
        );
    }

    /// ADR 0086 dec. 1 (review 2026-07-22): `pdf_document` is the BY-DESIGN gap —
    /// machine fact-reading of PDFs is retired, so no pipeline upgrade ever makes
    /// the document readable. Even a lower-version delta must never re-arm.
    #[test]
    fn a_pdf_document_gap_is_never_rearmed() {
        let dir = unique_temp_dir("rearm-pdf-doc");
        std::fs::create_dir_all(&dir).expect("temp dir");
        let connection = open_in_memory_database().expect("db");
        let state = AppState::with_data_dir(connection, dir.clone());
        let older_version =
            crate::jobs::structured_extraction::EXTRACTION_PIPELINE_VERSION.saturating_sub(1);
        let delta = serde_json::json!({
            "extractionAvailable": false,
            "reason": "pdf_document",
            "pipelineVersion": older_version,
        });
        let run = seed_terminal_unavailable_run(&state, &dir, "run_pdf_gap", delta);
        assert!(
            !terminal_run_should_rearm(&state, &run),
            "the by-design PDF gap must never re-arm — no upgrade makes a PDF readable"
        );
    }

    /// The gap-reason derivation: a raw-PDF document reports the by-design
    /// `pdf_document` reason; anything else keeps the caller's fallback.
    #[test]
    fn gap_reason_names_a_pdf_document_and_keeps_the_fallback_otherwise() {
        let dir = unique_temp_dir("gap-reason");
        std::fs::create_dir_all(&dir).expect("temp dir");
        let connection = open_in_memory_database().expect("db");
        let state = AppState::with_data_dir(connection, dir.clone());
        let company = state
            .create_company(NewCompany {
                exchange: "GPW".to_owned(),
                ticker: "GAP".to_owned(),
                display_name: "Gap Reason S.A.".to_owned(),
                isin: None,
                cik: None,
                lei: None,
            })
            .expect("company");

        let run_for = |suffix: &str, url: &str, local: &str, mime: &str| {
            let document = state
                .create_or_find_pending_report_document(CaptureReportDocumentInput {
                    company_id: company.id.clone(),
                    source_type: "user_url".to_owned(),
                    url: url.to_owned(),
                    period_id: None,
                    origin_ref: None,
                    title: Some(format!("doc {suffix}")),
                    attribution: None,
                })
                .expect("document");
            std::fs::write(dir.join(local), b"stub").expect("write file");
            state
                .mark_report_document_fetched(&document.id, Some(local), Some(mime), None, Some(4))
                .expect("mark fetched");
            let run_id = format!("run_gap_{suffix}");
            state
                .autopilot()
                .create_run_if_absent(
                    &run_id,
                    &company.id,
                    &document.id,
                    "manual",
                    MODE_AUTOPILOT,
                    None,
                )
                .expect("create run")
                .expect("run created");
            state.autopilot().get_run(&run_id).expect("get run")
        };

        let pdf_run = run_for(
            "pdf",
            "https://example.com/report.pdf",
            "report.pdf",
            "application/pdf",
        );
        assert_eq!(
            gap_reason(&state, &pdf_run, "no_deterministic_tier"),
            "pdf_document"
        );

        let xhtml_run = run_for(
            "xhtml",
            "https://example.com/report.xhtml",
            "report.xhtml",
            "application/xhtml+xml",
        );
        assert_eq!(
            gap_reason(&state, &xhtml_run, "no_deterministic_tier"),
            "no_deterministic_tier"
        );
    }

    /// Epic #229 T2: `pdf_document` is the **never-re-armed** verdict (ADR 0086
    /// dec. 1) — "core KPIs arrive from the aggregator". Only bytes that really are
    /// a PDF may earn it. 45 of the maintainer's stored `.pdf` files are XML or ZIP
    /// inside; stamping those `pdf_document` on the strength of their extension
    /// retired documents a deterministic tier can still read.
    #[test]
    fn gap_reason_earns_pdf_document_only_from_the_sniffed_container() {
        let dir = unique_temp_dir("gap-reason-container");
        std::fs::create_dir_all(&dir).expect("temp dir");
        let connection = open_in_memory_database().expect("db");
        let state = AppState::with_data_dir(connection, dir.clone());
        let company = state
            .create_company(NewCompany {
                exchange: "GPW".to_owned(),
                ticker: "GPC".to_owned(),
                display_name: "Gap Container S.A.".to_owned(),
                isin: None,
                cik: None,
                lei: None,
            })
            .expect("company");

        let run_for = |suffix: &str, container: &str| {
            let local = format!("report_{suffix}.pdf");
            let document = state
                .create_or_find_pending_report_document(CaptureReportDocumentInput {
                    company_id: company.id.clone(),
                    source_type: "user_url".to_owned(),
                    url: format!("https://example.com/{local}"),
                    period_id: None,
                    origin_ref: None,
                    title: Some(format!("doc {suffix}")),
                    attribution: None,
                })
                .expect("document");
            std::fs::write(dir.join(&local), b"stub").expect("write file");
            state
                .mark_report_document_fetched(
                    &document.id,
                    Some(&local),
                    Some("application/pdf"),
                    None,
                    Some(4),
                )
                .expect("mark fetched");
            state
                .set_report_document_detected_container(&document.id, container)
                .expect("stamp container");
            let run_id = format!("run_gapc_{suffix}");
            state
                .autopilot()
                .create_run_if_absent(
                    &run_id,
                    &company.id,
                    &document.id,
                    "manual",
                    MODE_AUTOPILOT,
                    None,
                )
                .expect("create run")
                .expect("run created");
            state.autopilot().get_run(&run_id).expect("get run")
        };

        // Everything below is named `.pdf` and served `application/pdf`.
        assert_eq!(
            gap_reason(&state, &run_for("real", "pdf"), "no_deterministic_tier"),
            "pdf_document",
            "a genuine PDF still earns the by-design reason"
        );
        assert_eq!(
            gap_reason(&state, &run_for("xml", "xml"), "no_deterministic_tier"),
            "no_deterministic_tier",
            "an XML statement under a .pdf name keeps a re-armable reason"
        );
        assert_eq!(
            gap_reason(&state, &run_for("zip", "zip"), "no_deterministic_tier"),
            "no_deterministic_tier",
            "an ESEF package under a .pdf name keeps a re-armable reason"
        );
    }

    /// Epic #229 T2: the canonical-report tie-break (`prefers_candidate`, reused by
    /// the coverage read model) prefers the **structured** document when two
    /// filings share a disclosure key. Resolving that from the URL alone ranked the
    /// corpus's 38 XML statements stored under a `.pdf` name below their companion
    /// PDF, handing the canonical slot to the document with less extractable data.
    #[test]
    fn structured_tie_break_prefers_the_markup_stored_under_a_pdf_name() {
        let mut markup = report_doc(
            "doc_markup",
            "https://bonnier.pl/static/att/emitent/2025-05/ssf_2025.pdf",
            "SSF 2025",
            "2025-05-02T00:00:00Z",
        );
        markup.detected_container = Some("xml".to_owned());
        let mut pdf = report_doc(
            "doc_pdf",
            "https://bonnier.pl/static/att/emitent/2025-05/ssf_2025_scan.pdf",
            "SSF 2025 scan",
            "2025-05-01T00:00:00Z",
        );
        pdf.detected_container = Some("pdf".to_owned());

        assert!(is_structured_document(&markup));
        assert!(!is_structured_document(&pdf));
        // Same disclosure month → the tie-break decides, and it must pick the
        // markup even though BOTH URLs end `.pdf`.
        assert!(
            prefers_candidate(&pdf, &markup),
            "the genuinely structured sibling must win the canonical slot"
        );
        assert!(
            !prefers_candidate(&markup, &pdf),
            "and the PDF must not displace it on a re-run"
        );

        // An ESEF report package is structured too — the structured path unpacks it.
        let mut package = report_doc(
            "doc_zip",
            "https://bonnier.pl/static/att/emitent/2025-05/ssf_2025_pkg.pdf",
            "SSF 2025 package",
            "2025-05-03T00:00:00Z",
        );
        package.detected_container = Some("zip".to_owned());
        assert!(is_structured_document(&package));
    }
}
