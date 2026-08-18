//! Version-aware re-extraction — durable batch job (epic #398 Item B, ADR
//! 0100 consequence: "existing tagged filings do not widen by themselves").
//!
//! A widened crosswalk/projection (Item A's fix, and the epic generally)
//! never re-reads an already-landed report on its own: `terminal_run_should_
//! rearm` in [`crate::jobs::autopilot`] deliberately never re-arms a run that
//! emitted facts — that rule is correct and MUST stay (it stops the history
//! sweep from re-milling every finished report on every pass). Widening
//! coverage for existing filings therefore needs an EXPLICIT, user-triggered
//! action, reachable from the Coverage panel: this module selects the
//! company's successful ESEF-tier runs whose stored `pipelineVersion` (epic
//! #398 Item B blocker 2 fix — now stamped on an emitted-success delta too,
//! not only a gap) is below the current build, and re-arms each through the
//! SAME primitives the sweep's own re-arm uses (`AutopilotStore::rearm_run` +
//! `autopilot::enqueue_first_stage`) — but through a SEPARATE candidate
//! selector and a SEPARATE durable record ([`PipelineReextractionBatch`],
//! migration `0146`), so the sweep's own skip rule is never touched.
//!
//! Idempotent by construction: a second invocation re-lists the company's
//! runs and finds none below the (now-current) stamped version, so it
//! completes with zero candidates — a genuine no-op, not a special case.

use serde::{Deserialize, Serialize};

use crate::app_state::AppState;
use crate::jobs::autopilot::{enqueue_first_stage, stored_pipeline_version};
use crate::jobs::structured_extraction::EXTRACTION_PIPELINE_VERSION;
use crate::storage::{
    AutopilotRun, ListAutopilotRunsInput, PipelineReextractionBatch, PipelineReextractionOutcome,
};

/// Durable-queue job kind for one re-extraction batch.
pub const PIPELINE_REEXTRACTION_KIND: &str = "pipeline_reextraction";

/// A batch re-arms terminal runs idempotently (re-listing finds nothing left
/// once every candidate is current, and a re-listed candidate re-arms as a
/// harmless reset-to-pending even if a prior attempt already touched it), so
/// **2** attempts: the first covers the normal run, the second lets a
/// genuine process crash (job left `running`, reclaimed on the next startup —
/// see `reclaim_stale_running`'s attempts-exhausted dead-letter guard,
/// ADR 0059) actually resume and complete instead of dead-lettering with the
/// batch row stuck at `running` forever.
const PIPELINE_REEXTRACTION_MAX_ATTEMPTS: i64 = 2;

/// Payload for a `pipeline_reextraction` job: which batch row to drive.
#[derive(Debug, Serialize, Deserialize)]
pub struct PipelineReextractionPayload {
    pub batch_id: String,
}

/// `autopilot_run.trigger` value re-armed runs are stamped with. Reuses the
/// existing generic "user explicitly triggered this" bucket (`rerun_
/// extraction_outcome`'s "Try again", the manual history sweep) rather than
/// widening the `autopilot_run.trigger` CHECK for one more value with
/// identical semantics.
const REEXTRACTION_TRIGGER: &str = "manual";

/// Start a version-aware re-extraction batch for a company: create a queued
/// batch row + enqueue its durable job.
///
/// **Idempotent while one is in flight, race-free** (sol review finding 12,
/// both rounds): the check-and-create runs in ONE immediate transaction
/// (`create_batch_if_none_active`), so two concurrent MCP calls can never
/// both mint a batch. A failed job enqueue marks the fresh batch `failed`
/// instead of stranding a permanently-`queued` batch that every later call
/// would return as if it were making progress.
pub fn enqueue_pipeline_reextraction(
    state: &AppState,
    company_id: &str,
) -> Result<PipelineReextractionBatch, String> {
    let (batch, created) = state
        .pipeline_reextraction()
        .create_batch_if_none_active(company_id)
        .map_err(|error| error.to_string())?;
    if !created {
        return Ok(batch);
    }
    let payload = serde_json::to_string(&PipelineReextractionPayload {
        batch_id: batch.id.clone(),
    })
    .map_err(|error| error.to_string())?;
    if let Err(error) = state.jobs().enqueue(
        &batch.id,
        PIPELINE_REEXTRACTION_KIND,
        &payload,
        PIPELINE_REEXTRACTION_MAX_ATTEMPTS,
    ) {
        // Never strand an inert `queued` batch (sol round 2): mark it failed
        // so the next call may start a real one.
        let _ = state
            .pipeline_reextraction()
            .fail_batch(&batch.id, &format!("job enqueue failed: {error}"));
        return Err(error.to_string());
    }
    Ok(batch)
}

/// Run one re-extraction batch (the `pipeline_reextraction` handler entry
/// point). Loads the batch row, selects stale-version successful ESEF-tier
/// runs for its company, re-arms each through `rearm_run` +
/// `enqueue_first_stage`, and records the counted outcome. A storage-level
/// abort listing candidates fails the batch with the error and returns `Err`
/// (mirrors `run_history_sweep_job`); per-candidate re-arm failures are
/// counted (`runs_failed`), never abort the whole batch.
pub fn run_pipeline_reextraction_job(state: &AppState, payload: &str) -> Result<(), String> {
    let payload: PipelineReextractionPayload =
        serde_json::from_str(payload).map_err(|error| error.to_string())?;
    let batch = state
        .pipeline_reextraction()
        .get_batch(&payload.batch_id)
        .map_err(|error| error.to_string())?;

    state
        .pipeline_reextraction()
        .mark_batch_running(&batch.id)
        .map_err(|error| error.to_string())?;

    let runs = match state.autopilot().list_runs(&ListAutopilotRunsInput {
        company_id: Some(batch.company_id.clone()),
        notification_state: None,
        limit: Some(500),
    }) {
        Ok(runs) => runs,
        Err(error) => {
            let error = error.to_string();
            let _ = state.pipeline_reextraction().fail_batch(&batch.id, &error);
            return Err(error);
        }
    };

    let candidates: Vec<AutopilotRun> = runs.into_iter().filter(is_stale_esef_success).collect();
    let candidates_total = candidates.len() as i64;

    let mut enqueued_run_ids = Vec::new();
    let mut runs_failed = 0i64;
    for run in candidates {
        match state
            .autopilot()
            .rearm_run(&run.id, REEXTRACTION_TRIGGER, None)
        {
            Ok(()) => {
                log::info!(
                    "pipeline_reextraction: re-arming run {} — stored pipeline version below current",
                    run.id
                );
                enqueue_first_stage(state, &run.id);
                enqueued_run_ids.push(run.id);
            }
            Err(error) => {
                log::warn!(
                    "pipeline_reextraction: re-arm failed for {}: {error}",
                    run.id
                );
                runs_failed += 1;
            }
        }
    }

    state
        .pipeline_reextraction()
        .complete_batch(
            &batch.id,
            &PipelineReextractionOutcome {
                candidates_total,
                runs_enqueued: enqueued_run_ids.len() as i64,
                runs_failed,
                enqueued_run_ids,
            },
        )
        .map_err(|error| error.to_string())?;
    Ok(())
}

/// Whether a run is a re-extraction candidate: **terminal-succeeded**, an
/// **emitted ESEF-tier success** (`extractionAvailable:true`, `tier:"esef"` —
/// never a gap, and never a lower-trust tier this batch has no business
/// touching), with a **stored pipeline version below the current build**.
/// Reuses [`stored_pipeline_version`]'s "missing = 0" convention, so every
/// run that emitted before epic #398 Item B blocker 2's fix (which never
/// stamped a version on a success delta at all) is a candidate exactly once —
/// after this batch re-runs it, the fresh delta stamps the current version
/// and a later batch finds it settled.
fn is_stale_esef_success(run: &AutopilotRun) -> bool {
    if run.status != "succeeded" {
        return false;
    }
    let Some(delta) = run
        .kpi_delta_json
        .as_deref()
        .and_then(|json| serde_json::from_str::<serde_json::Value>(json).ok())
    else {
        return false;
    };
    if delta.get("extractionAvailable").and_then(|v| v.as_bool()) != Some(true) {
        return false;
    }
    if delta.get("tier").and_then(|v| v.as_str())
        != Some(crate::fundamentals::extraction::SourceTier::Esef.as_str())
    {
        return false;
    }
    stored_pipeline_version(run.kpi_delta_json.as_deref()) < EXTRACTION_PIPELINE_VERSION
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::storage::{open_in_memory_database, AppState, NewCompany, MODE_AUTOPILOT};

    fn state() -> AppState {
        AppState::new(open_in_memory_database().expect("in-memory db"))
    }

    fn company(state: &AppState) -> String {
        state
            .create_company(NewCompany {
                exchange: "GPW".to_owned(),
                ticker: "TST".to_owned(),
                display_name: "Test S.A.".to_owned(),
                isin: None,
                cik: None,
                lei: None,
            })
            .expect("company")
            .id
    }

    /// A terminal-succeeded run with the given `kpi_delta_json`.
    fn succeeded_run(state: &AppState, company_id: &str, doc: &str, delta_json: &str) -> String {
        let run_id = format!("autopilot_run:{company_id}:{doc}");
        state
            .autopilot()
            .create_run_if_absent(&run_id, company_id, doc, "detection", MODE_AUTOPILOT, None)
            .expect("create run")
            .expect("run created");
        state
            .autopilot()
            .set_kpi_delta_json(&run_id, delta_json)
            .expect("set delta");
        state
            .autopilot()
            .finalize_run(&run_id, "succeeded", "notify", None, None)
            .expect("finalize");
        run_id
    }

    #[test]
    fn a_stale_emitted_esef_run_is_rearmed_and_the_batch_records_it() {
        let s = state();
        let c = company(&s);
        s.autopilot().set_mode(&c, "assist").expect("set mode");
        // No `pipelineVersion` at all — the pre-blocker-2-fix shape every
        // existing successful ESEF run has (stored_pipeline_version = 0).
        let run_id = succeeded_run(
            &s,
            &c,
            "doc1",
            r#"{"extractionAvailable":true,"tier":"esef","produced":3}"#,
        );

        let batch = enqueue_pipeline_reextraction(&s, &c).expect("enqueue batch");
        assert_eq!(batch.status, "queued");

        let payload = s
            .jobs()
            .pending_payload(&batch.id)
            .expect("pending payload query")
            .expect("a pipeline_reextraction job must be queued");
        assert!(payload.contains(&batch.id));

        run_pipeline_reextraction_job(&s, &payload).expect("batch runs");

        let completed = s
            .pipeline_reextraction()
            .get_batch(&batch.id)
            .expect("get batch");
        assert_eq!(completed.status, "completed");
        assert_eq!(completed.candidates_total, 1);
        assert_eq!(completed.runs_enqueued, 1);
        assert_eq!(completed.enqueued_run_ids, vec![run_id.clone()]);

        let run = s.autopilot().get_run(&run_id).expect("get run");
        assert_eq!(run.status, "pending", "rearm_run resets to pending/fetch");
        assert_eq!(run.stage, "fetch");
        assert_eq!(run.trigger, "manual");
    }

    #[test]
    fn a_run_at_the_current_pipeline_version_is_never_selected() {
        let s = state();
        let c = company(&s);
        s.autopilot().set_mode(&c, "assist").expect("set mode");
        let current = succeeded_run(
            &s,
            &c,
            "doc1",
            &format!(
                r#"{{"extractionAvailable":true,"tier":"esef","pipelineVersion":{EXTRACTION_PIPELINE_VERSION}}}"#
            ),
        );

        let batch = enqueue_pipeline_reextraction(&s, &c).expect("enqueue batch");
        let payload = s
            .jobs()
            .pending_payload(&batch.id)
            .expect("payload")
            .expect("job queued");
        run_pipeline_reextraction_job(&s, &payload).expect("batch runs");

        let completed = s
            .pipeline_reextraction()
            .get_batch(&batch.id)
            .expect("get batch");
        assert_eq!(completed.candidates_total, 0);
        assert_eq!(completed.runs_enqueued, 0);

        // Untouched — the current-version run stays exactly as it was.
        let run = s.autopilot().get_run(&current).expect("get run");
        assert_eq!(run.status, "succeeded");
    }

    #[test]
    fn a_gap_run_and_a_non_esef_tier_run_are_never_selected() {
        let s = state();
        let c = company(&s);
        s.autopilot().set_mode(&c, "assist").expect("set mode");
        succeeded_run(
            &s,
            &c,
            "doc_gap",
            r#"{"extractionAvailable":false,"reason":"no_deterministic_tier"}"#,
        );
        succeeded_run(
            &s,
            &c,
            "doc_other_tier",
            r#"{"extractionAvailable":true,"tier":"html_aggregator"}"#,
        );

        let batch = enqueue_pipeline_reextraction(&s, &c).expect("enqueue batch");
        let payload = s
            .jobs()
            .pending_payload(&batch.id)
            .expect("payload")
            .expect("job queued");
        run_pipeline_reextraction_job(&s, &payload).expect("batch runs");

        let completed = s
            .pipeline_reextraction()
            .get_batch(&batch.id)
            .expect("get batch");
        assert_eq!(
            completed.candidates_total, 0,
            "a gap run and a non-ESEF tier run are the history sweep's or a \
             different tier's business, never this batch's"
        );
    }

    /// A second invocation, after the first already re-armed + settled every
    /// stale run (its fresh delta re-stamps the current version), finds
    /// nothing left — a genuine no-op.
    #[test]
    fn a_second_invocation_after_settling_is_a_no_op() {
        let s = state();
        let c = company(&s);
        s.autopilot().set_mode(&c, "assist").expect("set mode");
        let run_id = succeeded_run(
            &s,
            &c,
            "doc1",
            r#"{"extractionAvailable":true,"tier":"esef"}"#,
        );

        let first_batch = enqueue_pipeline_reextraction(&s, &c).expect("first batch");
        let first_payload = s
            .jobs()
            .pending_payload(&first_batch.id)
            .expect("payload")
            .expect("job queued");
        run_pipeline_reextraction_job(&s, &first_payload).expect("first batch runs");
        let first_completed = s
            .pipeline_reextraction()
            .get_batch(&first_batch.id)
            .expect("get batch");
        assert_eq!(first_completed.runs_enqueued, 1);

        // Simulate the re-armed run settling with the current version stamped
        // (what `stage_extract` does on its next real run).
        s.autopilot()
            .set_kpi_delta_json(
                &run_id,
                &format!(
                    r#"{{"extractionAvailable":true,"tier":"esef","pipelineVersion":{EXTRACTION_PIPELINE_VERSION}}}"#
                ),
            )
            .expect("set delta");
        s.autopilot()
            .finalize_run(&run_id, "succeeded", "notify", None, None)
            .expect("finalize");

        let second_batch = enqueue_pipeline_reextraction(&s, &c).expect("second batch");
        let second_payload = s
            .jobs()
            .pending_payload(&second_batch.id)
            .expect("payload")
            .expect("job queued");
        run_pipeline_reextraction_job(&s, &second_payload).expect("second batch runs");

        let second_completed = s
            .pipeline_reextraction()
            .get_batch(&second_batch.id)
            .expect("get batch");
        assert_eq!(second_completed.candidates_total, 0);
        assert_eq!(second_completed.runs_enqueued, 0);
    }

    /// Restart recovery: a batch job left `running` by a crash (claimed but
    /// never completed) is reclaimed by the generic durable-queue startup
    /// sweep (`reclaim_stale_running`) — same mechanism every job kind
    /// shares — and resumes to completion on the next drain, never stuck.
    #[test]
    fn a_crashed_batch_job_is_reclaimed_and_completes_on_restart() {
        let s = state();
        let c = company(&s);
        s.autopilot().set_mode(&c, "assist").expect("set mode");
        succeeded_run(
            &s,
            &c,
            "doc1",
            r#"{"extractionAvailable":true,"tier":"esef"}"#,
        );
        let batch = enqueue_pipeline_reextraction(&s, &c).expect("enqueue batch");

        // Simulate a crash mid-job: claim the job (as a worker would) but
        // never run the handler — it is left `running`.
        let claimed = s
            .jobs()
            .claim_next_for_kinds(&[PIPELINE_REEXTRACTION_KIND])
            .expect("claim")
            .expect("the batch job is claimable");
        assert_eq!(claimed.id, batch.id);

        // "Restart": the generic startup reclaim runs, then the worker drains.
        let reclaimed = s.jobs().reclaim_stale_running().expect("reclaim");
        assert_eq!(reclaimed, 1);

        let worker = crate::jobs::handlers::build_worker(s.clone());
        let processed = worker.run_until_idle().expect("drain");
        assert!(
            processed >= 1,
            "the reclaimed batch job (and the autopilot stage job it chains \
             on re-arm) must actually run, not stay stuck: {processed}"
        );

        let completed = s
            .pipeline_reextraction()
            .get_batch(&batch.id)
            .expect("get batch");
        assert_eq!(completed.status, "completed");
        assert_eq!(completed.runs_enqueued, 1);
    }
}
