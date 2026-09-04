//! Direct-activity registry for awaited (non-queue) work (ADR 0109 dec. 3):
//! per-adapter source refresh, the aggregator pull, direct history backfill,
//! the direct/stale-checked registry refresh, the transcript runner. An
//! in-memory RAII guard writes the `job_runs` start/finish rows for the ONE
//! writer the queue itself is not — Drop-on-unwind settles `interrupted` so a
//! panic in awaited work can never leave an open occurrence.
//!
//! Also home to [`BackfillProgress`] (ADR 0036) — moved here (with its
//! `AppState` accessors) from `storage::mod` to keep that file's pinned line
//! count (`file-size-baseline.json`, ADR 0103) unchanged while this module's
//! new `AppState` field/accessors land.

use std::collections::HashMap;
use std::sync::{Arc, Mutex};

use serde::Serialize;

use super::{AppState, StorageResult};
use crate::jobs::activity_identity::ActivityIdentity;
use crate::storage::job_runs::{JobRunOutcome, NewJobRun};

/// Live progress/diagnostics for an on-track history backfill (ADR 0036). Held in shared
/// memory (not persisted): backfill is an explicit, app-open-only action, and idempotent
/// re-runs mean a lost in-flight status is never harmful.
#[derive(Clone, Debug, Serialize)]
#[cfg_attr(feature = "ts-export", derive(ts_rs::TS))]
#[cfg_attr(
    feature = "ts-export",
    ts(export, export_to = "../../src/api/generated/")
)]
#[serde(rename_all = "camelCase")]
pub struct BackfillProgress {
    pub company_id: String,
    /// `running` | `completed` | `failed`.
    #[cfg_attr(
        feature = "ts-export",
        ts(type = "\"running\" | \"completed\" | \"failed\"")
    )]
    pub status: String,
    pub pages_fetched: usize,
    pub items_ingested: usize,
    pub documents_stored: usize,
    pub detail_errors: usize,
    /// True when the page cap ended the fetch before the configured backfill
    /// cutoff was reached (ADR 0077 §3) — older filings may be missing. Surfaced
    /// as an explicit warning in the coverage panel, never silently dropped.
    pub truncated: bool,
    /// The chained history sweep's id, when a completed backfill auto-chained one
    /// (ADR 0077 §3). The sweep row is created **eagerly** at enqueue time, so this
    /// id is known before the command returns — the coverage panel polls THIS sweep
    /// specifically (never "the latest sweep", which could be a stale/other one) so
    /// its status line and AI-budget footer settle on the sweep the backfill
    /// started, never a false-settle. `None` when nothing was chained (a chain
    /// failure is best-effort, or the backfill itself failed).
    pub chained_sweep_id: Option<String>,
    pub error: Option<String>,
    pub started_at: String,
    pub updated_at: String,
}

impl AppState {
    /// Replace the stored backfill progress for a company.
    pub fn set_backfill_progress(&self, progress: BackfillProgress) {
        let mut guard = self
            .backfill_progress
            .lock()
            .expect("backfill progress mutex poisoned");
        guard.insert(progress.company_id.clone(), progress);
    }

    /// Read the latest backfill progress for a company, if any run has been recorded.
    pub fn get_backfill_progress(&self, company_id: &str) -> Option<BackfillProgress> {
        let guard = self
            .backfill_progress
            .lock()
            .expect("backfill progress mutex poisoned");
        guard.get(company_id).cloned()
    }

    pub(crate) fn activity_registry_map(&self) -> &Arc<Mutex<HashMap<String, i64>>> {
        &self.activity_registry
    }
}

/// RAII guard for one direct-activity occurrence (ADR 0109 dec. 3): holds the
/// `job_runs.id` [`ActivityRegistry::start`] opened. Call [`ActivityGuard::settle`]
/// with the work's outcome; if the guard is dropped without settling (an
/// unwind), the occurrence settles `interrupted` — a panic in awaited work can
/// never leave an open occurrence or a live registry entry behind.
pub struct ActivityGuard {
    state: AppState,
    run_id: i64,
    map_key: String,
    settled: bool,
}

impl ActivityGuard {
    /// Settle the occurrence: `Ok(())` -> `succeeded`, `Err(message)` ->
    /// `failed`. Runs the settle + retention prune in ONE transaction, then
    /// removes the live registry entry.
    pub fn settle(mut self, outcome: Result<(), &str>) {
        self.settle_and_remove(match outcome {
            Ok(()) => JobRunOutcome::Succeeded,
            Err(error) => JobRunOutcome::Failed { error },
        });
    }

    fn settle_and_remove(&mut self, outcome: JobRunOutcome<'_>) {
        if self.settled {
            return;
        }
        self.settled = true;
        if let Err(error) = self.state.job_runs().settle(self.run_id, outcome) {
            log::warn!(
                "activity registry: failed to settle occurrence {}: {error}",
                self.run_id
            );
        }
        if let Ok(mut map) = self.state.activity_registry_map().lock() {
            map.remove(&self.map_key);
        }
    }
}

impl Drop for ActivityGuard {
    fn drop(&mut self) {
        if !self.settled {
            self.settle_and_remove(JobRunOutcome::Interrupted);
        }
    }
}

/// Begin one direct-activity occurrence: writes the `job_runs` start row and
/// inserts the live registry entry (ADR 0109 dec. 3). `kind` is the queue-kind
/// token this awaited path shares a family with (e.g. `scheduled_source_refresh`
/// for a manual per-adapter refresh) — used only to compose the `run_key`
/// (`direct:<activity_key>`), never written as a queue row. Best-effort: a
/// storage failure logs and returns `None` rather than blocking the caller's
/// work — an awaited command must never fail because its activity ledger row
/// could not be written.
pub fn start(state: &AppState, identity: ActivityIdentity) -> Option<ActivityGuard> {
    let map_key = format!("direct:{}", identity.activity_key);
    let new_run = NewJobRun {
        activity_key: identity.activity_key,
        run_key: map_key.clone(),
        kind: map_key.clone(),
        family: identity.family,
        company_id: identity.company_id,
        subject: identity.subject,
        target: identity.target,
        attempt: 1,
    };
    let run_id = match state.job_runs().begin_attempt(new_run) {
        Ok(run_id) => run_id,
        Err(error) => {
            log::warn!("activity registry: begin_attempt failed: {error}");
            return None;
        }
    };
    if let Ok(mut map) = state.activity_registry_map().lock() {
        map.insert(map_key.clone(), run_id);
    }
    Some(ActivityGuard {
        state: state.clone(),
        run_id,
        map_key,
        settled: false,
    })
}

/// Read the current live direct-activity entries: `job_runs.id` values still
/// open in the registry. Used by the read model's `active` composition.
pub(crate) fn live_run_ids(state: &AppState) -> StorageResult<Vec<i64>> {
    Ok(state
        .activity_registry_map()
        .lock()
        .map(|map| map.values().copied().collect())
        .unwrap_or_default())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::jobs::activity_identity::{ActivityFamily, ActivityTarget};
    use crate::storage::open_in_memory_database;

    fn identity() -> ActivityIdentity {
        ActivityIdentity {
            activity_key: "source-refresh:gpw-espi-ebi".to_owned(),
            family: ActivityFamily::SourceRefresh,
            company_id: None,
            subject: "GPW ESPI/EBI".to_owned(),
            target: ActivityTarget::Sources,
        }
    }

    #[test]
    fn start_writes_a_running_row_and_a_live_entry() {
        let state = AppState::new(open_in_memory_database().expect("db"));
        let guard = start(&state, identity()).expect("guard");
        assert_eq!(live_run_ids(&state).expect("live").len(), 1);

        let connection = state.checkout_for_tests().expect("checkout");
        let status: String = connection
            .query_row(
                "SELECT status FROM job_runs WHERE id = ?1",
                [guard.run_id],
                |row| row.get(0),
            )
            .expect("status");
        assert_eq!(status, "running");
    }

    #[test]
    fn settle_ok_terminalizes_succeeded_and_clears_the_entry() {
        let state = AppState::new(open_in_memory_database().expect("db"));
        let guard = start(&state, identity()).expect("guard");
        let run_id = guard.run_id;
        guard.settle(Ok(()));

        assert!(live_run_ids(&state).expect("live").is_empty());
        let connection = state.checkout_for_tests().expect("checkout");
        let status: String = connection
            .query_row(
                "SELECT status FROM job_runs WHERE id = ?1",
                [run_id],
                |row| row.get(0),
            )
            .expect("status");
        assert_eq!(status, "succeeded");
    }

    #[test]
    fn direct_activity_panic_settles_interrupted() {
        // A guard dropped WITHOUT an explicit settle (the shape a panic mid-work
        // leaves behind, since the guard is never reached to call `.settle()`)
        // must still terminalize the occurrence (`interrupted`) and clear the
        // live entry, never leaving either dangling.
        let state = AppState::new(open_in_memory_database().expect("db"));
        let run_id = {
            let guard = start(&state, identity()).expect("guard");
            guard.run_id
            // `guard` drops here, never settled.
        };

        assert!(live_run_ids(&state).expect("live").is_empty());
        let connection = state.checkout_for_tests().expect("checkout");
        let status: String = connection
            .query_row(
                "SELECT status FROM job_runs WHERE id = ?1",
                [run_id],
                |row| row.get(0),
            )
            .expect("status");
        assert_eq!(status, "interrupted");
    }
}
