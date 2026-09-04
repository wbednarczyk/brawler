//! Startup reconciliation for the Activity ledger (ADR 0109 dec. 4).
//!
//! Runs once at startup, AFTER the existing KPI-run reclaim, generic queue
//! reclaim, and KPI queue reconciliation, and BEFORE any worker lane starts
//! (`lib.rs`) — so no command or worker can begin a run while a crash's
//! residue is still ambiguous. Idempotent: every rule only touches rows that
//! are still in the state it looks for, so a second call is a no-op.

use crate::app_state::AppState;
use crate::jobs::history_sweep::HISTORY_SWEEP_KIND;
use crate::jobs::pipeline_reextraction::PIPELINE_REEXTRACTION_KIND;
use crate::storage::{job_runs, JobRunOutcome};

/// Terminalize everything a crash could have left ambiguous (ADR 0109 dec. 4).
/// Best-effort per rule: one rule's storage error is logged and does not stop
/// the others (never blocks startup).
///
/// DELIBERATE (sol diff R1 #12): reconciliation failing does NOT refuse to
/// start the worker pools. The ledger is a read-model convenience over the
/// app's real work, never a gate on it — a startup that hard-failed because
/// one occurrence's `interrupted` write errored would turn an activity-panel
/// cosmetic issue into the app not starting at all. Every failure here logs
/// at `error` level (loud, since it means a durable ledger row is now
/// silently wrong) and reconciliation simply moves on.
pub fn reconcile_on_startup(state: &AppState) {
    if let Err(error) = interrupt_open_occurrences(state) {
        log::error!("activity reconcile: open occurrences: {error}");
    }
    if let Err(error) = fail_running_transcripts(state) {
        log::warn!("activity reconcile: transcripts: {error}");
    }
    if let Err(error) = fail_orphaned_autopilot_runs(state) {
        log::warn!("activity reconcile: autopilot runs: {error}");
    }
    if let Err(error) = fail_orphaned_domain_rows(state, "history_sweeps", HISTORY_SWEEP_KIND) {
        log::warn!("activity reconcile: history sweeps: {error}");
    }
    if let Err(error) = fail_orphaned_domain_rows(
        state,
        "pipeline_reextraction_batches",
        PIPELINE_REEXTRACTION_KIND,
    ) {
        log::warn!("activity reconcile: reextraction batches: {error}");
    }
    // KPI ingest runs are NEVER terminalized here: their own reclaim
    // (`kpi_ingest_runs().reclaim_ingest_runs_on_startup`, run earlier in
    // `lib.rs`) owns `committing`; an expired/NULL lease reads as waiting in
    // the read model, never as a stranded run.
}

/// Every occurrence still `running` when the app starts could not have
/// survived the crash — terminalize it `interrupted` (ADR 0109 dec. 4).
/// Bulk, in ONE `BEGIN IMMEDIATE` transaction (sol diff R1 #12): the
/// previous per-id loop ran each settle as its own autocommit statement, so
/// a failure partway left an arbitrary split between interrupted and still-
/// `running` rows. Atomic now: either every open occurrence settles, or none
/// does (and the caller logs it loudly and moves on regardless).
fn interrupt_open_occurrences(state: &AppState) -> Result<(), String> {
    let mut connection = state.checkout().map_err(|e| e.to_string())?;
    let tx = connection
        .transaction_with_behavior(rusqlite::TransactionBehavior::Immediate)
        .map_err(|e| e.to_string())?;
    let ids = job_runs::running_ids(&tx).map_err(|e| e.to_string())?;
    for id in ids {
        job_runs::settle(&tx, id, &JobRunOutcome::Interrupted).map_err(|e| e.to_string())?;
    }
    job_runs::prune(&tx).map_err(|e| e.to_string())?;
    tx.commit().map_err(|e| e.to_string())
}

/// A `transcript_jobs` row left `running` by a crash: no queue and no
/// worker survive process death, so it is honestly `failed` with
/// `error_code = "interrupted"`.
fn fail_running_transcripts(state: &AppState) -> Result<(), String> {
    let connection = state.checkout().map_err(|e| e.to_string())?;
    connection
        .execute(
            "
            UPDATE transcript_jobs
            SET status = 'failed',
                error_code = 'interrupted',
                error = COALESCE(error, 'Interrupted by app restart'),
                finished_at = strftime('%Y-%m-%dT%H:%M:%fZ', 'now')
            WHERE status = 'running'
            ",
            [],
        )
        .map_err(|e| e.to_string())?;
    Ok(())
}

/// A non-terminal `autopilot_run` (`pending`/`running`) with NO live stage job
/// (pending/running `autopilot_stage` naming its `run_id`) cannot resume — the
/// queue's own crash-residue reclaim already ran, so a run with no live stage
/// job left is provably stranded. Terminalized `failed`. A run WITH a live
/// stage job (including one in retry backoff) is left alone.
fn fail_orphaned_autopilot_runs(state: &AppState) -> Result<(), String> {
    let connection = state.checkout().map_err(|e| e.to_string())?;
    let mut statement = connection
        .prepare("SELECT id FROM autopilot_run WHERE status IN ('pending', 'running')")
        .map_err(|e| e.to_string())?;
    let run_ids: Vec<String> = statement
        .query_map([], |row| row.get::<_, String>(0))
        .map_err(|e| e.to_string())?
        .collect::<Result<_, _>>()
        .map_err(|e| e.to_string())?;
    drop(statement);

    for run_id in run_ids {
        // sol diff R1 #12: exact `IN` over the five deterministic stage ids
        // (`jobs::autopilot::has_live_stage_job`) — never a `LIKE` prefix
        // match, which a `run_id` containing `_` (a SQLite LIKE wildcard)
        // could exploit into a false-positive "still live" read.
        let live_stage = crate::jobs::autopilot_liveness::has_live_stage_job(&connection, &run_id)
            .map_err(|e| e.to_string())?;
        if live_stage {
            continue;
        }
        connection
            .execute(
                "
                UPDATE autopilot_run
                SET status = 'failed',
                    last_error = COALESCE(last_error, 'Interrupted by app restart: no live stage job'),
                    updated_at = strftime('%Y-%m-%dT%H:%M:%fZ', 'now')
                WHERE id = ?1
                ",
                [&run_id],
            )
            .map_err(|e| e.to_string())?;
    }
    Ok(())
}

/// Shared rule for `history_sweeps` / `pipeline_reextraction_batches`: both
/// enqueue their parent job under an id EQUAL to the domain row's own id
/// (`enqueue_history_sweep`, `pipeline_reextraction`'s batch enqueue). A
/// non-terminal row (`queued`/`running`) with no live (`pending`/`running`)
/// parent job of that id is stranded — terminalized `failed`.
fn fail_orphaned_domain_rows(state: &AppState, table: &str, job_kind: &str) -> Result<(), String> {
    let connection = state.checkout().map_err(|e| e.to_string())?;
    let select_sql = format!("SELECT id FROM {table} WHERE status IN ('queued', 'running')");
    let mut statement = connection.prepare(&select_sql).map_err(|e| e.to_string())?;
    let ids: Vec<String> = statement
        .query_map([], |row| row.get::<_, String>(0))
        .map_err(|e| e.to_string())?
        .collect::<Result<_, _>>()
        .map_err(|e| e.to_string())?;
    drop(statement);

    for id in ids {
        let live_job: bool = connection
            .query_row(
                "SELECT EXISTS(
                    SELECT 1 FROM job_queue
                    WHERE id = ?1 AND kind = ?2 AND status IN ('pending', 'running')
                )",
                rusqlite::params![id, job_kind],
                |row| row.get(0),
            )
            .map_err(|e| e.to_string())?;
        if live_job {
            continue;
        }
        let update_sql = format!(
            "UPDATE {table} SET status = 'failed', error = COALESCE(error, 'Interrupted by app restart: no live job'), updated_at = strftime('%Y-%m-%dT%H:%M:%fZ', 'now') WHERE id = ?1"
        );
        connection
            .execute(&update_sql, [&id])
            .map_err(|e| e.to_string())?;
    }
    Ok(())
}

#[cfg(test)]
mod tests;
