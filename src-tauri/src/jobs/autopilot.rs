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
use crate::storage::{self, MODE_AUTOPILOT};

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
    use crate::jobs::structured_extraction::{StructuredExtractionResult, Tier4Gate};

    // Structured-first (ADR 0061 dec. 3/8/9), then the tier-4 OCR fallback inside
    // `run_structured_extraction` (ADR 0077 §4). Caller gate: detection/manual
    // triggers permit tier-4; a history-sweep run is denied until F3b (also
    // enforced by the explicit early-return below, for defense in depth).
    let gate = if run.trigger == TRIGGER_HISTORY_SWEEP {
        Tier4Gate::DeniedSweep
    } else {
        Tier4Gate::Allowed
    };

    let mut carried_structure_changed = false;
    let mut carried_drift_json: Option<String> = None;
    // The non-emitting structured result (with any tier-4 outcome folded in),
    // held for the delta composition below.
    let mut non_emitting: Option<StructuredExtractionResult> = None;

    match try_structured_extraction(state, run, gate) {
        Ok(Some(result)) if result.emitted => {
            // Facts emitted — by a structured tier, or by the tier-4 OCR fallback
            // (whose verdict is folded into `result` as the `ai_text` tier).
            if !result.produced_fact_ids.is_empty() {
                state
                    .autopilot()
                    .add_produced_facts(&run.id, &result.produced_fact_ids)
                    .map_err(|e| e.to_string())?;
            }
            let mut delta = emitted_extract_delta(&result, &run.mode);
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
        // Determinism did not emit (flagged/empty). The tier-4 fallback (if the
        // gate allowed it) already ran inside `run_structured_extraction`; hold
        // the result for the delta below. Carry the structure-changed signal.
        Ok(Some(result)) => {
            carried_structure_changed = result.structure_changed;
            carried_drift_json = result.drift_json.clone();
            non_emitting = Some(result);
        }
        // Not eligible for structured extraction (no derivable period, unparsable).
        Ok(None) => {}
        Err(error) => log::info!(
            "autopilot run {}: structured extraction skipped: {error}",
            run.id
        ),
    }

    // F3b (ADR 0077 §3 amendment (b) lifted, §6 budget governs): a sweep run may
    // now use tier-4 — but only under the per-sweep AI budget snapshotted on its
    // sweep row. Reached ONLY when the deterministic pipeline did not emit; a
    // deterministic Accepted returned at the emit branch above, so it never spends
    // budget (decision 4). Never a silent skip.
    if run.trigger == TRIGGER_HISTORY_SWEEP {
        // Not structured-eligible (no derivable period) → the PDF+period-only
        // tier-4 cannot run either. Record the gap; spend no budget.
        if non_emitting.is_none() {
            let mut delta =
                serde_json::json!({ "extractionAvailable": false, "reason": "not_extractable" });
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

        // No VisionExtraction pool member → tier-4 has nothing to invoke. Degrade
        // BEFORE consuming: a budget unit buys a provider invocation, not a
        // configuration check — charging here would burn the whole sweep budget
        // on nothing and mislabel later periods `skipped_budget`. (The same check
        // inside `run_tier4_extraction` stays as defense in depth; a settings
        // change racing this pre-check at worst misdirects one run's reason.)
        if !has_vision_provider(state) {
            let mut delta = serde_json::json!({
                "extractionAvailable": false,
                "reason": "no_vision_provider",
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

        // Statically tier-4-ineligible document → degrade BEFORE consuming (G-4
        // class, ADR 0077 §6). A budget unit buys a provider invocation; tier-4
        // can only degrade on a statically-decidable document — no stored file, or
        // the ESEF/iXBRL route (tier-4 is PDF-only). Replicates
        // `run_tier4_extraction`'s early degrades (`no_stored_file` / `not_pdf`)
        // BEFORE the budget consume, mirroring the no-vision-provider pre-check
        // above; charging here would burn the whole sweep budget on a document no
        // invocation could read and mislabel later periods `skipped_budget`. (The
        // same checks inside `run_tier4_extraction` stay as defense in depth.)
        let document = state
            .get_report_document(&run.report_document_id)
            .map_err(|e| e.to_string())?;
        let static_ineligible_reason = match document.local_path.as_deref() {
            None => Some("no_stored_file"),
            Some(local_path)
                if crate::jobs::structured_extraction::is_esef_route(
                    document.content_type.as_deref(),
                    local_path,
                ) =>
            {
                Some("not_pdf")
            }
            Some(_) => None,
        };
        if let Some(reason) = static_ineligible_reason {
            let mut delta = serde_json::json!({
                "extractionAvailable": false,
                "reason": reason,
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

        // Budget gate (decision 3): a legacy run with no sweep_id, or an exhausted
        // budget, skips honestly — never a silent drop. One granted unit lets this
        // run enter tier-4.
        let granted = match run.sweep_id.as_deref() {
            Some(sweep_id) => state
                .history_sweeps()
                .try_consume_sweep_ai_budget(sweep_id)
                .unwrap_or(false),
            None => false,
        };
        if !granted {
            let mut delta =
                serde_json::json!({ "extractionAvailable": false, "reason": "skipped_budget" });
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

        // Budget granted → enter tier-4 exactly like a detection run
        // (Tier4Gate::Allowed). The deterministic-only first pass emitted nothing,
        // so re-running it is side-effect-free apart from the tier-4 fallback we
        // now want. A transient provider failure propagates as Err so the queue's
        // capped backoff retries (a retry re-charges a unit — one budget unit is
        // one tier-4 invocation, ADR 0077 §6).
        let rerun = try_structured_extraction(state, run, Tier4Gate::Allowed)?;
        let mut delta = match &rerun {
            Some(result) if result.emitted => {
                if !result.produced_fact_ids.is_empty() {
                    state
                        .autopilot()
                        .add_produced_facts(&run.id, &result.produced_fact_ids)
                        .map_err(|e| e.to_string())?;
                }
                emitted_extract_delta(result, &run.mode)
            }
            Some(result) => tier4_extract_delta(result, &run.mode),
            // Deterministic eligibility cannot vanish between two passes over the
            // same document, but stay total.
            None => {
                serde_json::json!({ "extractionAvailable": false, "reason": "not_extractable" })
            }
        };
        // The fresh pass carries the same drift as the first; merge it so a
        // structure change is never dropped.
        let (structure_changed, drift_json) = match &rerun {
            Some(result) => (result.structure_changed, result.drift_json.clone()),
            None => (carried_structure_changed, carried_drift_json.clone()),
        };
        merge_structure_flag(&mut delta, structure_changed, drift_json.as_deref());
        let _ = state
            .autopilot()
            .set_kpi_delta_json(&run.id, &delta.to_string());
        return Ok(());
    }

    // Detection/manual, determinism did not emit: record the tier-4 OCR fallback's
    // honest outcome (proposals landed for confirmation, or a benign degradation).
    // The old text-AI number-reading path is retired (ADR 0077 §4: the LLM must
    // never read numbers) — tier-4 is now the single AI fallback, and it lives in
    // `run_structured_extraction`.
    let mut delta = match &non_emitting {
        Some(result) => tier4_extract_delta(result, &run.mode),
        // Not eligible for the deterministic path (no derivable period / unparsable):
        // tier-4 is PDF+period-only, so nothing can extract this document.
        None => serde_json::json!({ "extractionAvailable": false, "reason": "not_extractable" }),
    };
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

/// Compose the `extractionAvailable:true` `kpi_delta_json` for an emitting run —
/// facts produced by a structured tier or by the tier-4 OCR fallback (whose
/// verdict is folded into `result` as the `ai_text` tier). Shared by the
/// detection/manual emit path and the sweep's budget-granted tier-4 re-run so the
/// two compose one honest shape. The caller records produced facts + merges any
/// structure-changed flag; this only builds the counts (bug e77a1a2: normalized
/// so both tiers report `factsProposed`/`factsAutoConfirmed` identically).
fn emitted_extract_delta(
    result: &crate::jobs::structured_extraction::StructuredExtractionResult,
    mode: &str,
) -> serde_json::Value {
    let produced = result.produced_fact_ids.len();
    let auto_confirmed = structured_facts_auto_confirmed(result.acceptance, produced, mode);
    serde_json::json!({
        "extractionAvailable": true,
        // `structured` distinguishes a deterministic tier from the tier-4 OCR
        // fallback for the Today card / diagnostics.
        "structured": result.tier4.is_none(),
        "tier": result.tier.map(|t| t.as_str()),
        "tier4": result.tier4,
        "produced": produced,
        "factsProposed": produced,
        "factsAutoConfirmed": auto_confirmed,
        "mode": mode,
    })
}

/// Compose the `kpi_delta_json` for a non-emitting detection/manual run from the
/// tier-4 fallback's outcome (ADR 0077 §4): proposals landed for confirmation
/// report `extractionAvailable:true` with the proposal count; a benign
/// degradation (`no_vision_provider`, `not_pdf`, a terminal provider error)
/// reports `extractionAvailable:false` with the honest reason.
fn tier4_extract_delta(
    result: &crate::jobs::structured_extraction::StructuredExtractionResult,
    mode: &str,
) -> serde_json::Value {
    match result.tier4.as_deref() {
        Some("bootstrap_proposals") | Some("proposals_flagged") => serde_json::json!({
            "extractionAvailable": true,
            "tier4": result.tier4,
            "factsProposed": result.tier4_proposals,
            "factsAutoConfirmed": 0,
            "mode": mode,
        }),
        Some(reason) => serde_json::json!({
            "extractionAvailable": false,
            "reason": reason,
            "tier4": result.tier4,
        }),
        // The gate denied tier-4 (should not happen on the detection/manual path)
        // or nothing ran: an honest, benign no-op.
        None => serde_json::json!({ "extractionAvailable": false, "reason": "no_extraction" }),
    }
}

/// Attempts structured-first extraction for a document eligible for it (ADR
/// 0061 dec. 3/8/9): a tagged ESEF/iXBRL `.xhtml` filing, or a PDF whose
/// reporting period can be derived from its title/URL. Returns `Ok(None)` when
/// the document is not eligible (unparsable ESEF, or a PDF whose period can't
/// be classified), so the caller falls back to the AI path. Runs in **both**
/// trust-ladder modes — [`crate::jobs::structured_extraction::
/// run_structured_extraction`] derives the per-fact confirmation state from
/// `run.mode` and the pipeline's acceptance. The period derivation (ESEF vs
/// PDF title/URL) lives in [`crate::jobs::structured_extraction::
/// derive_report_period`], shared with the on-demand "Extract data" command.
fn try_structured_extraction(
    state: &AppState,
    run: &storage::AutopilotRun,
    tier4: crate::jobs::structured_extraction::Tier4Gate,
) -> Result<Option<crate::jobs::structured_extraction::StructuredExtractionResult>, String> {
    let document = state
        .get_report_document(&run.report_document_id)
        .map_err(|e| e.to_string())?;
    // Period derivation is shared with the on-demand "Extract data" command so
    // the two paths never drift (`derive_report_period`). `None` → not eligible
    // for the deterministic path (nor the PDF-only tier-4 fallback).
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
        tier4,
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

    reenqueue_qualitative_assessments(state, run);
    Ok(())
}

/// §T6d (ADR 0075 Decision 5): on a new report, re-enqueue qualitative assessment
/// for each framework that already has a prior agent assessment for this company
/// — the bounded, meaningful "refresh existing judgment on new evidence" set (no
/// explicit framework↔company assignment model exists). The assessment runs as
/// its own durable job; this only re-arms it. Idempotent per `company:framework`
/// via [`enqueue_assessment`] — a re-arm arriving while a run is in flight is parked
/// in that pair's follow-up row, never dropped. Best-effort: a storage hiccup here
/// must never fail the pipeline (the report is already processed).
fn reenqueue_qualitative_assessments(state: &AppState, run: &storage::AutopilotRun) {
    let frameworks = match state.frameworks_with_qualitative_assessments(&run.company_id) {
        Ok(frameworks) => frameworks,
        Err(error) => {
            log::warn!(
                "autopilot cross_reference: list assessed frameworks failed for {}: {error}",
                run.company_id
            );
            return;
        }
    };
    for framework in frameworks {
        // `None` criterion set ⇒ refresh all qualitative criteria (§T6d, D3).
        if let Err(error) = crate::commands::quality_frameworks::enqueue_assessment(
            state,
            &run.company_id,
            &framework.framework_id,
            None,
        ) {
            log::warn!(
                "autopilot cross_reference: re-enqueue assessment failed for {}:{}: {error}",
                run.company_id,
                framework.framework_id
            );
        }
    }
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
        // J4 (ADR 0071): the user recorded expectations for this occurrence —
        // nudge them to review vs actuals (they record their own verdict).
        if refs
            .get("expectationsToReview")
            .and_then(|v| v.as_u64())
            .unwrap_or(0)
            > 0
        {
            parts.push("expectations recorded — review vs actuals".to_owned());
        }
    }

    if parts.is_empty() {
        "New report processed.".to_owned()
    } else {
        format!("New report processed — {}.", parts.join("; "))
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
/// - `not_extractable` / `not_pdf` / `no_stored_file` — a couldn't-extract verdict.
///   Re-armed **iff the document is now extractable** ([`history_sweep::
///   document_is_extractable`], reused not duplicated): a capability upgrade (the
///   tier-3b positional tier) can now read documents a prior version couldn't. A
///   still-dead file (unreadable/zero-byte) stays deduped — no per-sweep re-parse.
/// - `skipped_budget` — a budget-denied period. A **fresh sweep carries fresh
///   budget**, so re-attack it (still gated on extractability); the new sweep's own
///   budget naturally caps how many actually re-invoke tier-4 (G-4).
/// - `no_vision_provider` — tier-4 had no OCR provider. Re-armed only once one is
///   configured ([`has_vision_provider`], a cheap settings check the extractability
///   test can't see), so we never loop re-arming a period nothing can yet read.
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
    match reason.as_str() {
        "not_extractable" | "not_pdf" | "no_stored_file" | "skipped_budget" => {
            document_now_extractable(state, &run.report_document_id)
        }
        "no_vision_provider" => {
            document_now_extractable(state, &run.report_document_id) && has_vision_provider(state)
        }
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

/// Whether the run's document could now be read by SOME tier — the shared
/// `history_sweep::document_is_extractable` test. A load failure means the document
/// is gone/unreadable → not extractable (stay deduped).
fn document_now_extractable(state: &AppState, report_document_id: &str) -> bool {
    match state.get_report_document(report_document_id) {
        Ok(document) => crate::jobs::history_sweep::document_is_extractable(state, &document),
        Err(_) => false,
    }
}

/// Whether a `VisionExtraction` pool member is configured — the cheap settings
/// check deciding if tier-4 (OCR) could invoke a provider at all. Shared by
/// [`stage_extract`]'s pre-check and the [`terminal_run_should_rearm`] gate (a
/// `no_vision_provider` run is worth re-arming only once a provider exists).
fn has_vision_provider(state: &AppState) -> bool {
    state
        .get_settings()
        .map(|settings| {
            settings
                .capability_providers
                .get(crate::providers::analysis::capabilities::AiCapability::VisionExtraction.key())
                .is_some_and(|entries| !entries.is_empty())
        })
        .unwrap_or(false)
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

/// Whether a report document's format is a structured ESEF/iXBRL (xhtml)
/// statement rather than a PDF, resolved from its content type and/or
/// URL/local path. Used to break a disclosure-date tie in [`prefers_candidate`]
/// and, via the coverage read model (ADR 0077 §2), the canonical-report
/// structured tie-break. `pub(crate)` so the coverage command reuses this exact
/// definition instead of re-deriving it (F3 decides the final home).
pub(crate) fn is_structured_document(document: &storage::ReportDocument) -> bool {
    use crate::report_diff::extraction::SourceFormat;

    SourceFormat::resolve(document.content_type.as_deref(), &document.url) == SourceFormat::Xhtml
}

/// A sortable **disclosure-date** key (`YYYY-MM-DD`) for ranking report recency —
/// the domain date, not `created_at`/ingestion order ([data-model.md] Model
/// Principles; guardrail `d60305c`). The accepted ESPI/EBI attachment sources embed
/// the disclosure month in the URL as `/emitent/YYYY-MM/`; use it (day `01`, which
/// is enough for the quarterly cadence detection ranks). Falls back to `fetched_at`,
/// then `created_at` only as a last resort (a non-`emitent`, never-fetched doc).
/// `pub(crate)` so the coverage read model (ADR 0077 §2) ranks canonical-report
/// revisions with the identical disclosure semantics (F3 decides the final home).
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
            .create_run_if_absent(
                run_id,
                &company_id,
                &document_id,
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
                .create_run_if_absent(&run_id, &company_id, &document_id, "manual", mode, None)
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

    /// ADR 0061 dec. 3/8/9 + ADR 0077 §4: a flagged (drifted) structured attempt
    /// must not blind the company — it falls through to the tier-4 OCR fallback —
    /// and the "structure changed" signal is carried into the composed delta
    /// (here, the `no_vision_provider` degrade, since no vision provider is
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
            .create_run_if_absent(
                run_id,
                &company_id,
                &document_id,
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
            after.produced_fact_ids.is_empty(),
            "a flagged drift must not commit structured facts"
        );
        let delta = after.kpi_delta_json.expect("kpi delta recorded");
        assert!(delta.contains("no_vision_provider"), "delta: {delta}");
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
    /// F3b (ADR 0077 §3 amendment (b) lifted, §6 budget governs) — re-expected
    /// from the T3.2 "deterministic-only" pin: a `history_sweep` run under budget
    /// now ENTERS the tier-4 OCR fallback exactly like a detection run, and NEVER
    /// the retired text-AI `kpi_extraction` path. With no `VisionExtraction` pool
    /// member configured the run degrades `no_vision_provider` (a benign terminal
    /// reason) — still zero text-AI calls, zero `kpi_extraction` jobs — and the
    /// budget is NOT charged: without a provider there is nothing a unit could
    /// buy, and charging would burn the whole sweep budget on nothing while
    /// mislabeling later periods `skipped_budget` (QG re-expectation of the
    /// subagent's charge-on-entry variant). Once a provider exists, entering
    /// tier-4 charges BEFORE the invocation (upper-bound counter, ADR 0077 §6) —
    /// that half is G-4 below.
    #[test]
    fn history_sweep_run_enters_tier4_under_budget_never_text_ai() {
        let dir = unique_temp_dir("pdf-sweep-gate");
        std::fs::create_dir_all(&dir).expect("temp dir");
        let connection = open_in_memory_database().expect("db");
        // A real (non-test-sample) general provider is configured to prove the
        // retired text-AI path is gone: even so, a sweep run makes zero text-AI
        // calls — tier-4 (OCR) is the only AI fallback, and it needs a
        // `VisionExtraction` member (absent here), so it degrades cleanly.
        connection
            .execute(
                "UPDATE settings SET value = 'provider_gemini' WHERE key = 'general_analysis_provider'",
                [],
            )
            .expect("configure general provider");
        let state = AppState::with_data_dir(connection, dir.clone());
        // A structured-eligible PDF (period derivable from the title) whose body
        // is prose-only → determinism emits nothing → the run reaches the budget
        // gate → tier-4.
        let (company_id, document_id) =
            seed_pdf_report(&state, &dir, "CD PROJEKT 2026 Q1 SSF", &["prose only"]);

        // A real sweep row carries the default budget (30) snapshotted at creation;
        // the run charges it.
        let sweep = state
            .history_sweeps()
            .create_history_sweep(&company_id, "manual")
            .expect("create sweep");
        let run_id = "run_sweep_gate";
        state
            .autopilot()
            .create_run_if_absent(
                run_id,
                &company_id,
                &document_id,
                TRIGGER_HISTORY_SWEEP,
                MODE_AUTOPILOT,
                Some(&sweep.id),
            )
            .expect("create run")
            .expect("run created");
        let run = state.autopilot().get_run(run_id).expect("get run");

        stage_extract(&state, &run).expect("sweep extract stage completes");

        let after = state.autopilot().get_run(run_id).expect("get run");
        let delta = after.kpi_delta_json.expect("kpi delta recorded");
        assert!(
            delta.contains("no_vision_provider"),
            "a budgeted sweep run enters tier-4 and degrades no_vision_provider, got: {delta}"
        );
        assert!(
            delta.contains("\"extractionAvailable\":false"),
            "extraction is unavailable (no OCR provider), got: {delta}"
        );

        // Entering tier-4 spent one budget unit even though it degraded.
        let sweep = state
            .history_sweeps()
            .get_history_sweep(&sweep.id)
            .expect("reload sweep");
        assert_eq!(
            sweep.ai_calls_used, 0,
            "with no vision provider configured, no budget unit is spent — \
             a unit buys a provider invocation, not a configuration check"
        );

        // The retired text-AI path is never taken — no kpi_extraction job exists.
        let jobs = state
            .list_kpi_extraction_jobs_by_document(&document_id)
            .expect("jobs should list");
        assert!(
            jobs.is_empty(),
            "a history_sweep run must not create a text-AI kpi_extraction job, got: {jobs:?}"
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
            doc_kind: None,
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
    fn framework_with_prior_assessment(
        state: &AppState,
        company_id: &str,
        name: &str,
    ) -> storage::QualityFramework {
        let framework = state
            .create_quality_framework(storage::NewQualityFramework {
                name: name.to_owned(),
                description: None,
            })
            .expect("framework");
        let criterion = state
            .create_framework_criterion(storage::NewFrameworkCriterion {
                framework_id: framework.id.clone(),
                label: "Wide moat".to_owned(),
                expression: String::new(),
                weight: None,
                partial_band: None,
                ordinal: None,
                kind: Some("qualitative".to_owned()),
                assessment_guidance: Some("Assess moat.".to_owned()),
            })
            .expect("criterion");
        state
            .persist_qualitative_assessment(storage::PersistQualitativeAssessmentInput {
                framework_id: framework.id.clone(),
                company_id: company_id.to_owned(),
                results: vec![storage::QualitativeCriterionResult {
                    criterion_id: criterion.id,
                    ordinal: 0,
                    label: "Wide moat".to_owned(),
                    verdict: "pass".to_owned(),
                    reasoning: "Durable.".to_owned(),
                    citations_json: "[]".to_owned(),
                    confidence: "medium".to_owned(),
                    prompt_version: "qualitative_assessment_v1".to_owned(),
                }],
            })
            .expect("prior assessment");
        framework
    }

    /// §T6d (ADR 0075 Decision 5): on a new report, the cross-reference stage
    /// re-enqueues qualitative assessment for each framework that already has a
    /// prior agent assessment for the company — and only those (bounded).
    #[test]
    fn cross_reference_reenqueues_qualitative_assessment_for_assessed_frameworks() {
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
        let assessed = framework_with_prior_assessment(&state, &company.id, "Kroeze");
        // A framework never assessed for this company must NOT be re-enqueued.
        let unassessed = state
            .create_quality_framework(storage::NewQualityFramework {
                name: "Unassessed".to_owned(),
                description: None,
            })
            .expect("framework");

        let run_id = "run_xref_qual";
        state
            .autopilot()
            .create_run_if_absent(
                run_id,
                &company.id,
                "doc1",
                "manual",
                storage::MODE_ASSIST,
                None,
            )
            .expect("create run")
            .expect("run created");
        let run = state.autopilot().get_run(run_id).expect("get run");

        stage_cross_reference(&state, &run).expect("cross_reference stage");

        let assessed_job = format!("qualitative_assessment:{}:{}", company.id, assessed.id);
        assert!(
            state
                .jobs()
                .pending_payload(&assessed_job)
                .expect("pending lookup")
                .is_some(),
            "an assessed framework should be re-enqueued"
        );
        let unassessed_job = format!("qualitative_assessment:{}:{}", company.id, unassessed.id);
        assert!(
            state
                .jobs()
                .pending_payload(&unassessed_job)
                .expect("pending lookup")
                .is_none(),
            "a framework with no prior assessment must not be re-enqueued"
        );
    }

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
            summary.contains("expectations recorded — review vs actuals"),
            "summary should nudge the review: {summary}"
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
            !summary.contains("expectations recorded"),
            "no expectations line when none exist: {summary}"
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
    fn seed_test_sample_vision(connection: &rusqlite::Connection) {
        use crate::providers::analysis::capabilities::AiCapability;
        use crate::providers::analysis::{
            TEST_SAMPLE_ANALYSIS_MODEL, TEST_SAMPLE_ANALYSIS_PROVIDER_ID,
        };
        let json = serde_json::json!({
            AiCapability::VisionExtraction.key(): [
                { "provider": TEST_SAMPLE_ANALYSIS_PROVIDER_ID, "model": TEST_SAMPLE_ANALYSIS_MODEL }
            ]
        })
        .to_string();
        connection
            .execute(
                "INSERT INTO settings (key, value, value_type) VALUES ('capability_providers', ?1, 'json') \
                 ON CONFLICT(key) DO UPDATE SET value = excluded.value",
                [json],
            )
            .expect("seed vision provider");
    }

    /// G-4 (ADR 0077 §6, budget-stop): a sweep with `ai_call_limit=2` over three
    /// tier-4-eligible candidates spends exactly two units, budget-skips the third
    /// (`skipped_budget`, zero facts), and never silently drops a run.
    #[test]
    fn g4_history_sweep_stops_at_the_budget_never_silently_dropping() {
        let dir = unique_temp_dir("g4-budget");
        std::fs::create_dir_all(&dir).expect("temp dir");
        let connection = open_in_memory_database().expect("db");
        seed_test_sample_vision(&connection);
        let state = AppState::with_data_dir(connection, dir.clone());
        state
            .update_settings(storage::SettingsUpdate {
                history_sweep_ai_call_limit: Some(2),
                ..Default::default()
            })
            .expect("set budget to 2");
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
        for (year, file) in [(2023, "r23.pdf"), (2024, "r24.pdf"), (2025, "r25.pdf")] {
            seed_periodic_pdf(
                &state,
                &dir,
                &company.id,
                &format!("Skonsolidowany raport roczny {year} SSF"),
                file,
            );
        }

        // Drive the whole sweep end-to-end through the durable queue.
        let sweep =
            crate::jobs::history_sweep::enqueue_history_sweep(&state, &company.id, "manual")
                .expect("enqueue sweep");
        crate::jobs::handlers::build_worker(state.clone())
            .run_until_idle()
            .expect("drain the queue");

        let sweep = state
            .history_sweeps()
            .get_history_sweep(&sweep.id)
            .expect("reload sweep");
        assert_eq!(sweep.candidates_total, 3);
        assert_eq!(sweep.runs_enqueued, 3, "all three runs enqueued");
        assert_eq!(
            sweep.ai_calls_used, 2,
            "exactly two tier-4 invocations are charged"
        );

        let runs = state
            .autopilot()
            .list_runs(&crate::storage::ListAutopilotRunsInput {
                company_id: Some(company.id.clone()),
                limit: Some(500),
                ..Default::default()
            })
            .expect("list runs");
        assert_eq!(runs.len(), 3, "no run is silently dropped");
        let skipped: Vec<_> = runs
            .iter()
            .filter(|r| {
                r.kpi_delta_json
                    .as_deref()
                    .is_some_and(|d| d.contains("skipped_budget"))
            })
            .collect();
        assert_eq!(skipped.len(), 1, "exactly one run is budget-skipped");
        assert!(
            skipped[0].produced_fact_ids.is_empty(),
            "the budget-skipped run produces no facts"
        );
        // The other two runs each entered tier-4 (their delta carries the `tier4`
        // outcome field; the budget-skipped run's delta does not).
        let entered = runs
            .iter()
            .filter(|r| {
                r.kpi_delta_json
                    .as_deref()
                    .is_some_and(|d| d.contains("\"tier4\"") && !d.contains("skipped_budget"))
            })
            .count();
        assert_eq!(entered, 2, "the other two runs entered tier-4 under budget");
    }

    /// G-4 static-ineligibility (ADR 0077 §6): a sweep run whose canonical
    /// document is XHTML (the ESEF route — non-PDF) is **statically** ineligible
    /// for the PDF-only tier-4 OCR fallback; it can ONLY degrade `not_pdf`. A
    /// budget unit buys a PROVIDER INVOCATION, so this must be caught BEFORE
    /// `try_consume_sweep_ai_budget` — exactly like the no-vision-provider
    /// pre-check. Regression (live MDV): the run consumed 1 unit, THEN degraded
    /// `not_pdf` inside `run_tier4_extraction`, burning the unit on nothing and
    /// mislabeling later periods `skipped_budget`. Vision provider IS configured
    /// here to prove the charge is skipped for static ineligibility, not for a
    /// missing provider.
    #[test]
    fn history_sweep_statically_ineligible_xhtml_charges_no_budget() {
        let dir = unique_temp_dir("sweep-xhtml-ineligible");
        std::fs::create_dir_all(&dir).expect("temp dir");
        let connection = open_in_memory_database().expect("db");
        seed_test_sample_vision(&connection);
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
        // A *valid* iXBRL instance (period self-derives → structured-eligible) but
        // UNBALANCED (45 ≠ 20 + 30) → the ESEF tier flags it and emits nothing →
        // the sweep run reaches the budget gate. Because it is the ESEF route it is
        // statically tier-4-ineligible: it can only degrade `not_pdf`.
        let unbalanced_esef = r#"<html xmlns:ix="http://www.xbrl.org/2013/inlineXBRL"
          xmlns:xbrli="http://www.xbrl.org/2003/instance"
          xmlns:iso4217="http://www.xbrl.org/2003/iso4217">
          <xbrli:context id="c"><xbrli:period><xbrli:instant>2026-03-31</xbrli:instant></xbrli:period></xbrli:context>
          <xbrli:unit id="pln"><xbrli:measure>iso4217:PLN</xbrli:measure></xbrli:unit>
          <ix:nonFraction name="ifrs-full:Assets" contextRef="c" unitRef="pln" scale="3">45 000</ix:nonFraction>
          <ix:nonFraction name="ifrs-full:Liabilities" contextRef="c" unitRef="pln" scale="3">20 000</ix:nonFraction>
          <ix:nonFraction name="ifrs-full:Equity" contextRef="c" unitRef="pln" scale="3">30 000</ix:nonFraction>
        </html>"#;
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
        std::fs::write(dir.join("report.xhtml"), unbalanced_esef.as_bytes()).expect("write esef");
        state
            .mark_report_document_fetched(
                &document.id,
                Some("report.xhtml"),
                Some("application/xhtml+xml"),
                None,
                Some(unbalanced_esef.len() as i64),
            )
            .expect("mark fetched");

        let sweep = state
            .history_sweeps()
            .create_history_sweep(&company.id, "manual")
            .expect("create sweep");
        let run_id = "run_sweep_xhtml";
        state
            .autopilot()
            .create_run_if_absent(
                run_id,
                &company.id,
                &document.id,
                TRIGGER_HISTORY_SWEEP,
                MODE_AUTOPILOT,
                Some(&sweep.id),
            )
            .expect("create run")
            .expect("run created");
        let run = state.autopilot().get_run(run_id).expect("get run");

        stage_extract(&state, &run).expect("sweep extract stage completes");

        let after = state.autopilot().get_run(run_id).expect("get run");
        let delta = after.kpi_delta_json.expect("kpi delta recorded");
        assert!(
            delta.contains("not_pdf"),
            "an XHTML (ESEF-route) document degrades not_pdf, got: {delta}"
        );
        assert!(
            delta.contains("\"extractionAvailable\":false"),
            "extraction is unavailable for a non-PDF document, got: {delta}"
        );

        let sweep = state
            .history_sweeps()
            .get_history_sweep(&sweep.id)
            .expect("reload sweep");
        assert_eq!(
            sweep.ai_calls_used, 0,
            "a statically-ineligible (non-PDF/ESEF-route) document must not consume \
             a budget unit — a unit buys a provider invocation, not a format check"
        );
    }

    /// (d) A detection-triggered run never charges any sweep budget — only a
    /// `history_sweep`-trigger run spends units.
    #[test]
    fn detection_run_never_charges_a_sweep_budget() {
        let dir = unique_temp_dir("detection-nobudget");
        std::fs::create_dir_all(&dir).expect("temp dir");
        let connection = open_in_memory_database().expect("db");
        let state = AppState::with_data_dir(connection, dir.clone());
        let (company_id, document_id) =
            seed_pdf_report(&state, &dir, "CD PROJEKT 2026 Q1 SSF", &["prose only"]);
        let sweep = state
            .history_sweeps()
            .create_history_sweep(&company_id, "manual")
            .expect("sweep");
        let run_id = "det_run";
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
            .expect("create")
            .expect("created");
        let run = state.autopilot().get_run(run_id).expect("run");
        stage_extract(&state, &run).expect("extract");
        assert_eq!(
            state
                .history_sweeps()
                .get_history_sweep(&sweep.id)
                .expect("reload")
                .ai_calls_used,
            0,
            "a detection run charges no sweep budget"
        );
    }

    /// (e) A sweep run whose deterministic pipeline Accepts (and emits) returns
    /// before the budget gate — a deterministic outcome spends no budget
    /// (ADR 0077 §6 decision 4).
    #[test]
    fn sweep_run_whose_determinism_accepts_spends_no_budget() {
        let dir = unique_temp_dir("accept-nobudget");
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
        state
            .fundamentals_provenance()
            .upsert_profile(&ascii_profile(
                &company_id,
                &[
                    ("total assets line", "total_assets"),
                    ("total liabilities line", "total_liabilities"),
                    ("total equity line", "total_equity"),
                ],
            ))
            .expect("seed profile");
        let sweep = state
            .history_sweeps()
            .create_history_sweep(&company_id, "manual")
            .expect("sweep");
        let run_id = "sweep_accept";
        state
            .autopilot()
            .create_run_if_absent(
                run_id,
                &company_id,
                &document_id,
                TRIGGER_HISTORY_SWEEP,
                MODE_AUTOPILOT,
                Some(&sweep.id),
            )
            .expect("create")
            .expect("created");
        let run = state.autopilot().get_run(run_id).expect("run");
        stage_extract(&state, &run).expect("extract");

        let after = state.autopilot().get_run(run_id).expect("run");
        let delta = after.kpi_delta_json.expect("delta");
        assert!(
            delta.contains("\"extractionAvailable\":true"),
            "determinism should have emitted: {delta}"
        );
        assert_eq!(
            state
                .history_sweeps()
                .get_history_sweep(&sweep.id)
                .expect("reload")
                .ai_calls_used,
            0,
            "a deterministic Accept spends no budget"
        );
    }
}
