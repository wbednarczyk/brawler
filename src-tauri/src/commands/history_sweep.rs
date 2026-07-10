//! Tauri commands for the history sweep (ADR 0077 §3, T3.2).
//!
//! A history sweep enqueues a full autopilot run for every canonical periodic
//! report whose period still lacks accepted facts. It is chained automatically at
//! the end of a backfill and available manually here ("Extract missing periods" —
//! the CBF case where the documents already exist and only extraction is needed).
//! Both commands offload to a blocking task; the sweep itself runs on the durable
//! queue (`crate::jobs::history_sweep`).

use serde::Serialize;

use crate::{app_state, jobs, storage};

/// Per-run progress derived from a sweep's `enqueued_run_ids`, alongside the sweep
/// record itself (`null` when the company has never been swept).
#[derive(Debug, Serialize)]
#[cfg_attr(feature = "ts-export", derive(ts_rs::TS))]
#[cfg_attr(
    feature = "ts-export",
    ts(export, export_to = "../../src/api/generated/")
)]
#[serde(rename_all = "camelCase")]
pub struct HistorySweepProgress {
    pub sweep: Option<storage::HistorySweep>,
    /// How many runs the sweep enqueued.
    pub runs_total: i64,
    /// Enqueued runs that have reached a terminal state (`succeeded`/`partial`/`failed`).
    pub runs_done: i64,
    /// Enqueued runs that ended `failed`.
    pub runs_failed: i64,
}

/// Start a manual history sweep for a company ("Extract missing periods"). Gates:
/// the company must exist and be opted into automation (mode ≠ `off`) — the UI
/// disables the button in mode `off`, and this keeps the command honest for a
/// direct call. Creates a `manual` sweep row and enqueues its durable job.
#[tauri::command]
pub async fn run_history_sweep(
    company_id: String,
    state: tauri::State<'_, app_state::AppState>,
) -> Result<storage::HistorySweep, String> {
    let state = state.inner().clone();
    jobs::scheduler::run_blocking_task(move || start_history_sweep(&state, &company_id)).await
}

/// The company's latest sweep plus per-run progress derived from its enqueued run
/// ids (terminal = `succeeded`/`partial`/`failed`). Returns a null sweep when the
/// company has never been swept.
#[tauri::command]
pub async fn get_history_sweep_progress(
    company_id: String,
    state: tauri::State<'_, app_state::AppState>,
) -> Result<HistorySweepProgress, String> {
    let state = state.inner().clone();
    jobs::scheduler::run_blocking_task(move || compute_history_sweep_progress(&state, &company_id))
        .await
}

/// The gated sweep-start core the `run_history_sweep` command offloads (extracted
/// so the mock-fidelity corpus replays the same logic the wrapper delegates to).
pub(crate) fn start_history_sweep(
    state: &app_state::AppState,
    company_id: &str,
) -> Result<storage::HistorySweep, String> {
    if !company_exists(state, company_id)? {
        return Err("company_not_found".to_owned());
    }
    let mode = state
        .autopilot()
        .get_mode(company_id)
        .map_err(|error| error.to_string())?;
    if mode == storage::MODE_OFF {
        return Err("automation_off".to_owned());
    }
    jobs::history_sweep::enqueue_history_sweep(state, company_id, "manual")
}

/// The progress read model the `get_history_sweep_progress` command offloads
/// (extracted so the mock-fidelity corpus replays the same assembly).
pub(crate) fn compute_history_sweep_progress(
    state: &app_state::AppState,
    company_id: &str,
) -> Result<HistorySweepProgress, String> {
    let sweep = state
        .history_sweeps()
        .get_latest_history_sweep(company_id)
        .map_err(|error| error.to_string())?;

    let mut runs_total = 0;
    let mut runs_done = 0;
    let mut runs_failed = 0;
    if let Some(sweep) = sweep.as_ref() {
        runs_total = sweep.enqueued_run_ids.len() as i64;
        for run_id in &sweep.enqueued_run_ids {
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

    Ok(HistorySweepProgress {
        sweep,
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
    use crate::storage::{open_in_memory_database, AppState, NewCompany};

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
    fn run_history_sweep_rejects_an_unknown_company() {
        let s = state();
        let error =
            start_history_sweep(&s, "missing").expect_err("unknown company must be rejected");
        assert_eq!(error, "company_not_found");
    }

    #[test]
    fn run_history_sweep_rejects_off_mode() {
        let s = state();
        let c = company(&s);
        // Mode defaults to `off`.
        let error = start_history_sweep(&s, &c).expect_err("off-mode must be rejected");
        assert_eq!(error, "automation_off");
    }

    #[test]
    fn run_history_sweep_creates_a_manual_sweep_and_queues_its_job() {
        let s = state();
        let c = company(&s);
        s.autopilot().set_mode(&c, "assist").expect("set mode");

        let sweep = start_history_sweep(&s, &c).expect("sweep should start");
        assert_eq!(sweep.trigger, "manual");
        assert_eq!(sweep.status, "queued");
        // The command returns the sweep's id so the coverage panel can poll THIS
        // sweep specifically (never "the latest sweep") until it settles.
        assert!(
            !sweep.id.is_empty(),
            "the manual sweep command returns its id"
        );

        // The durable job is queued under the sweep id.
        let payload = s
            .jobs()
            .pending_payload(&sweep.id)
            .expect("pending payload query")
            .expect("a history_sweep job must be queued");
        assert!(payload.contains(&sweep.id));
    }

    #[test]
    fn progress_is_empty_before_any_sweep_then_reports_the_latest() {
        let s = state();
        let c = company(&s);

        let empty = compute_history_sweep_progress(&s, &c).expect("progress");
        assert!(empty.sweep.is_none(), "no sweep before one runs");
        assert_eq!(empty.runs_total, 0);

        // A completed sweep with two enqueued runs, one of which has terminated.
        let created = s
            .history_sweeps()
            .create_history_sweep(&c, "manual")
            .expect("create");
        let run_a = format!("autopilot_run:{c}:doc_a");
        let run_b = format!("autopilot_run:{c}:doc_b");
        s.autopilot()
            .create_run_if_absent(&run_a, &c, "doc_a", "history_sweep", "assist", None)
            .expect("run a");
        s.autopilot()
            .create_run_if_absent(&run_b, &c, "doc_b", "history_sweep", "assist", None)
            .expect("run b");
        s.autopilot()
            .finalize_run(&run_a, "succeeded", "notify", None, None)
            .expect("finalize a");
        let outcome = storage::HistorySweepOutcome {
            candidates_total: 2,
            runs_enqueued: 2,
            enqueued_run_ids: vec![run_a, run_b],
            ..Default::default()
        };
        s.history_sweeps()
            .complete_history_sweep(&created.id, &outcome)
            .expect("complete");

        let progress = compute_history_sweep_progress(&s, &c).expect("progress");
        let sweep = progress.sweep.expect("a sweep exists");
        assert_eq!(sweep.id, created.id);
        assert_eq!(progress.runs_total, 2);
        assert_eq!(progress.runs_done, 1, "one of the two runs has terminated");
        assert_eq!(progress.runs_failed, 0);
    }
}
