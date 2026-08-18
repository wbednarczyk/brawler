//! Tauri commands for version-aware re-extraction (epic #398 Item B).
//!
//! Reachable from the Coverage panel, beside "Backfill history" / "Extract
//! missing periods": re-arms the company's successful ESEF-tier runs whose
//! stored pipeline version is stale, through the durable `pipeline_reextraction`
//! job (`crate::jobs::pipeline_reextraction`). Both commands offload to a
//! blocking task, mirroring `commands::history_sweep`.

use serde::Serialize;

use crate::{app_state, jobs, storage};

/// Per-run progress derived from a batch's `enqueued_run_ids`, alongside the
/// batch record itself (`null` when the company has never had one).
#[derive(Debug, Serialize)]
#[cfg_attr(feature = "ts-export", derive(ts_rs::TS))]
#[cfg_attr(
    feature = "ts-export",
    ts(export, export_to = "../../src/api/generated/")
)]
#[serde(rename_all = "camelCase")]
pub struct PipelineReextractionProgress {
    pub batch: Option<storage::PipelineReextractionBatch>,
    /// How many runs the batch re-armed.
    pub runs_total: i64,
    /// Re-armed runs that have reached a terminal state (`succeeded`/`partial`/`failed`).
    pub runs_done: i64,
    /// Re-armed runs that ended `failed`.
    pub runs_failed: i64,
}

/// Start a version-aware re-extraction batch for a company. Creates a queued
/// batch row and enqueues its durable job; the batch selects and re-arms
/// candidates on the queue, never inline.
#[tauri::command]
pub async fn run_pipeline_reextraction(
    company_id: String,
    state: tauri::State<'_, app_state::AppState>,
) -> Result<storage::PipelineReextractionBatch, String> {
    let state = state.inner().clone();
    jobs::scheduler::run_blocking_task(move || start_pipeline_reextraction(&state, &company_id))
        .await
}

/// The company's latest re-extraction batch plus per-run progress derived
/// from its re-armed run ids. Returns a null batch when the company has never
/// had one.
#[tauri::command]
pub async fn get_pipeline_reextraction_progress(
    company_id: String,
    state: tauri::State<'_, app_state::AppState>,
) -> Result<PipelineReextractionProgress, String> {
    let state = state.inner().clone();
    jobs::scheduler::run_blocking_task(move || {
        compute_pipeline_reextraction_progress(&state, &company_id)
    })
    .await
}

/// The gated batch-start core the `run_pipeline_reextraction` command offloads
/// (extracted so the mock-fidelity corpus replays the same logic the wrapper
/// delegates to).
pub(crate) fn start_pipeline_reextraction(
    state: &app_state::AppState,
    company_id: &str,
) -> Result<storage::PipelineReextractionBatch, String> {
    if !company_exists(state, company_id)? {
        return Err("company_not_found".to_owned());
    }
    jobs::pipeline_reextraction::enqueue_pipeline_reextraction(state, company_id)
}

/// The progress read model the `get_pipeline_reextraction_progress` command
/// offloads (extracted so the mock-fidelity corpus replays the same
/// assembly).
pub(crate) fn compute_pipeline_reextraction_progress(
    state: &app_state::AppState,
    company_id: &str,
) -> Result<PipelineReextractionProgress, String> {
    let batch = state
        .pipeline_reextraction()
        .get_latest_batch(company_id)
        .map_err(|error| error.to_string())?;

    let mut runs_total = 0;
    let mut runs_done = 0;
    let mut runs_failed = 0;
    if let Some(batch) = batch.as_ref() {
        runs_total = batch.enqueued_run_ids.len() as i64;
        for run_id in &batch.enqueued_run_ids {
            // A run id may be absent (e.g. undone) — a missing run counts as
            // neither done nor failed, never an error.
            if let Ok(run) = state.autopilot().get_run(run_id) {
                if matches!(run.status.as_str(), "succeeded" | "partial" | "failed") {
                    runs_done += 1;
                }
                if run.status == "failed" {
                    runs_failed += 1;
                }
            }
        }
    }

    Ok(PipelineReextractionProgress {
        batch,
        runs_total,
        runs_done,
        runs_failed,
    })
}

fn company_exists(state: &app_state::AppState, company_id: &str) -> Result<bool, String> {
    Ok(state
        .list_companies()
        .map_err(|error| error.to_string())?
        .iter()
        .any(|company| company.id == company_id))
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

    #[test]
    fn run_pipeline_reextraction_rejects_an_unknown_company() {
        let s = state();
        let error = start_pipeline_reextraction(&s, "missing")
            .expect_err("unknown company must be rejected");
        assert_eq!(error, "company_not_found");
    }

    #[test]
    fn run_pipeline_reextraction_creates_a_batch_and_queues_its_job() {
        let s = state();
        let c = company(&s);

        let batch = start_pipeline_reextraction(&s, &c).expect("batch should start");
        assert_eq!(batch.company_id, c);
        assert_eq!(batch.status, "queued");
        assert!(!batch.id.is_empty());

        let payload = s
            .jobs()
            .pending_payload(&batch.id)
            .expect("pending payload query")
            .expect("a pipeline_reextraction job must be queued");
        assert!(payload.contains(&batch.id));
    }

    #[test]
    fn progress_is_empty_before_any_batch_then_reports_the_latest() {
        let s = state();
        let c = company(&s);

        let empty = compute_pipeline_reextraction_progress(&s, &c).expect("progress");
        assert!(empty.batch.is_none(), "no batch before one runs");
        assert_eq!(empty.runs_total, 0);

        // A completed batch with two re-armed runs, one of which has terminated.
        let created = s.pipeline_reextraction().create_batch(&c).expect("create");
        let run_a = format!("autopilot_run:{c}:doc_a");
        let run_b = format!("autopilot_run:{c}:doc_b");
        s.autopilot()
            .create_run_if_absent(&run_a, &c, "doc_a", "manual", MODE_AUTOPILOT, None)
            .expect("run a");
        s.autopilot()
            .create_run_if_absent(&run_b, &c, "doc_b", "manual", MODE_AUTOPILOT, None)
            .expect("run b");
        s.autopilot()
            .finalize_run(&run_a, "succeeded", "notify", None, None)
            .expect("finalize a");
        let outcome = storage::PipelineReextractionOutcome {
            candidates_total: 2,
            runs_enqueued: 2,
            runs_failed: 0,
            enqueued_run_ids: vec![run_a, run_b],
        };
        s.pipeline_reextraction()
            .complete_batch(&created.id, &outcome)
            .expect("complete");

        let progress = compute_pipeline_reextraction_progress(&s, &c).expect("progress");
        let batch = progress.batch.expect("a batch exists");
        assert_eq!(batch.id, created.id);
        assert_eq!(progress.runs_total, 2);
        assert_eq!(progress.runs_done, 1, "one of the two runs has terminated");
        assert_eq!(progress.runs_failed, 0);
    }
}
