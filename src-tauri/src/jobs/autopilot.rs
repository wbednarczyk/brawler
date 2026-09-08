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

pub(crate) const STAGE_FETCH: &str = "fetch";
pub(crate) const STAGE_EXTRACT: &str = "extract";
pub(crate) const STAGE_DIFF: &str = "diff";
pub(crate) const STAGE_CROSS_REFERENCE: &str = "cross_reference";
pub(crate) const STAGE_NOTIFY: &str = "notify";

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

pub(crate) fn stage_job_id(run_id: &str, stage: &str) -> String {
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

/// Enqueue the first stage of a run onto the durable queue. Self-healing
/// (issue #458): if the enqueue itself fails, the run is finalized `failed`
/// with a typed `last_error` naming the stage — never left `pending` with no
/// job to ever drive it (visible on the run card, ADR 0091 surface). `()`
/// return kept for this fn's external callers (a manual "run now" trigger),
/// which have nothing further to do with the outcome; `enqueue_extraction_run`
/// below calls `enqueue_stage` directly instead so it can report the failure
/// through its own `EnqueueExtractionOutcome`.
pub fn enqueue_first_stage(state: &AppState, run_id: &str) {
    if let Err(error) = enqueue_stage(state, run_id, STAGE_FETCH) {
        fail_run_stage_enqueue(state, run_id, STAGE_FETCH, &error);
    }
}

/// Finalize `run_id` as `failed` with a typed `last_error` naming the stage
/// that could not be enqueued (issue #458). Best-effort: a second storage
/// error here (finalizing the finalize) is logged, not propagated — the
/// caller is already on a failure path with nothing further to do.
fn fail_run_stage_enqueue(state: &AppState, run_id: &str, stage: &str, error: &str) {
    let message = format!("stage {stage} could not be enqueued: {error}");
    log::warn!("autopilot: run {run_id} failed — {message}");
    if let Err(finalize_error) =
        state
            .autopilot()
            .finalize_run(run_id, "failed", stage, None, Some(&message))
    {
        log::error!(
            "autopilot: failed to finalize run {run_id} as failed after a stage-enqueue failure: {finalize_error}"
        );
    }
}

/// Re-arm the durable-queue job driving `stage` of `run_id`. Returns `Err` iff
/// the queue write itself failed (issue #458: the pre-fix version swallowed
/// this as a warn-only log, so a crash/error here could strand a run
/// `pending`/`running` forever with no job under it — never observed as a
/// failure anywhere). `Ok(())` covers both a fresh/reset row (armed) and an
/// existing `running` row left untouched (already in flight) — the caller
/// does not need to distinguish those. `pub(super)` (visible throughout
/// `jobs::`): `activity_reconcile.rs`'s startup reconcile reuses this exact
/// primitive to reschedule a stranded run's last-started stage (issue #458)
/// rather than duplicating the reschedule/payload/kind wiring.
pub(super) fn enqueue_stage(state: &AppState, run_id: &str, stage: &str) -> Result<(), String> {
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
        Ok(true) => Ok(()),
        Ok(false) => {
            // The only way `reschedule` reports "not (re)armed" is an existing row
            // still `running` — expected when this stage is already in flight from
            // a prior life; logged so a silent no-op can never hide unnoticed again.
            log::info!(
                "autopilot: stage {stage} job {job_id} for run {run_id} already running, not re-armed"
            );
            Ok(())
        }
        Err(error) => {
            let message = error.to_string();
            log::warn!("autopilot: failed to enqueue stage {stage} for run {run_id}: {message}");
            Err(message)
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

    // Issue #458: was `let _ =`, swallowing a storage error here. Propagate —
    // the queue retries the stage attempt (STAGE_MAX_ATTEMPTS) rather than
    // silently proceeding to run the stage body against a run row whose
    // `stage`/`status` were never actually updated.
    state
        .autopilot()
        .set_run_stage(&run.id, &payload.stage, "running")
        .map_err(|e| e.to_string())?;

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
                // Issue #458: the hand-off enqueue is never swallowed — a
                // failure here used to be a warn-only log with the run left
                // `running` and no live job under it (permanently stranded,
                // no reconcile rule resurrected it). Finalize the run
                // `failed` with a typed `last_error` instead; the CURRENT
                // stage's own job genuinely succeeded, so its own outcome
                // stays `Ok`.
                if let Err(error) = enqueue_stage(state, &run.id, next) {
                    fail_run_stage_enqueue(state, &run.id, next, &error);
                }
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
/// lands `confirmed` in **both** modes, so every produced fact is counted.
/// `acceptance`/`mode` do not change the answer but stay in the signature so
/// callers pass them uniformly. `Flagged`/`Empty` never reach this (nothing
/// was emitted).
fn structured_facts_auto_confirmed(
    _acceptance: crate::fundamentals::extraction::pipeline::Acceptance,
    produced: usize,
    _mode: &str,
) -> usize {
    produced
}

/// Stage 2 — extract the report's KPIs through the **deterministic** pipeline
/// (ADR 0061 dec. 3/8/9; no AI branch, ADR 0084 decision 4): either a
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
/// English sentence (ADR 0084 decision 6). The backend emits the code and the
/// frontend renders it through the translation layer.
///
/// The only cause this app can still produce is [`Self::NoDeterministicTier`];
/// provider-shaped variants remain so stored AI-era deltas keep reporting
/// their original cause (ADR 0084 decision 5).
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
    /// so the Today card never frames a by-design gap as a per-report failure.
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
///
/// Stamps `pipelineVersion` (epic #398 Item B blocker 2): before this fix it was
/// stamped ONLY on the `extractionAvailable:false` gap delta, so an emitted
/// success left no record of which pipeline version produced it — the
/// version-aware re-extraction (`jobs::structured_extraction::rearm_stale_
/// pipeline_version_runs`) needs it on EVERY terminal delta, emitted or not, to
/// tell an already-current run apart from a stale one.
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
        "pipelineVersion": crate::jobs::structured_extraction::EXTRACTION_PIPELINE_VERSION,
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
            // Both extraction branches write these keys now. Only emit a KPI
            // count token when the delta actually carries the counts — never
            // fabricate "0 of 0" for a shape that never reported them.
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
    /// A storage error prevented enqueuing — either `create_run_if_absent`/
    /// `get_run`/`rearm_run` itself, or the run was created/re-armed but its
    /// stage could not be enqueued (issue #458: the run is finalized `failed`
    /// with a typed `last_error` in that case, never left dangling). Logged
    /// and skipped (best-effort, mirroring the detection sweep's
    /// warn-and-continue on a per-report failure).
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
            // Issue #458: called directly (not the pub `enqueue_first_stage`
            // wrapper) so a stage-enqueue failure is reported as `Failed`
            // here instead of `Created` — the run is still finalized failed
            // either way (`fail_run_stage_enqueue`).
            match enqueue_stage(state, &run.id, STAGE_FETCH) {
                Ok(()) => EnqueueExtractionOutcome::Created,
                Err(error) => {
                    fail_run_stage_enqueue(state, &run.id, STAGE_FETCH, &error);
                    EnqueueExtractionOutcome::Failed
                }
            }
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
                    match enqueue_stage(state, &existing.id, &existing.stage) {
                        Ok(()) => EnqueueExtractionOutcome::Rearmed,
                        Err(error) => {
                            fail_run_stage_enqueue(state, &existing.id, &existing.stage, &error);
                            EnqueueExtractionOutcome::Failed
                        }
                    }
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
                    match enqueue_stage(state, &existing.id, STAGE_FETCH) {
                        Ok(()) => EnqueueExtractionOutcome::Rearmed,
                        Err(error) => {
                            fail_run_stage_enqueue(state, &existing.id, STAGE_FETCH, &error);
                            EnqueueExtractionOutcome::Failed
                        }
                    }
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
/// forever. The version gate —`stored_pipeline_version(run) <
/// EXTRACTION_PIPELINE_VERSION`— is what makes a
/// capability upgrade (a newly landed/changed deterministic tier) retry a period
/// **once**: after the re-run stamps the current version the period settles. A
/// still-dead file (unreadable/zero-byte, or one no tier can parse) stays
/// deduped regardless. Manual per-period retry ("Try again" /
/// `rerun_extraction_outcome`) does NOT route through here — it calls
/// `run_structured_extraction` directly and stays unconditional.
///
/// The AI-era reasons (`no_vision_provider`, `skipped_budget`) remain readable
/// on stored runs (ADR 0084); they re-arm on the same extractability test as
/// any other gap.
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

/// The `EXTRACTION_PIPELINE_VERSION` a run stamped into its `kpi_delta_json` —
/// on EITHER a couldn't-extract verdict or an emitted-success delta (epic #398
/// Item B blocker 2: before that fix only the gap delta stamped it). A
/// missing/garbled delta or an absent `pipelineVersion` field reads as `0` —
/// the pre-versioning era — so a legacy run is eligible for exactly one re-arm
/// under the current build (see the version gate in
/// [`terminal_run_should_rearm`], reused by
/// [`crate::jobs::pipeline_reextraction`] for the emitted-success population).
pub(crate) fn stored_pipeline_version(kpi_delta_json: Option<&str>) -> u32 {
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
/// XML statements under a `.pdf` name — name alone would misrank them below a
/// companion PDF, handing the canonical slot to the document with less
/// extractable data. A ZIP counts as structured because the structured path
/// unpacks its inner iXBRL instance.
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
mod tests;
