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
use std::sync::atomic::{AtomicI64, Ordering};
use std::sync::{Arc, Mutex};

use serde::Serialize;

use super::{AppState, StorageResult};
use crate::jobs::activity_identity::ActivityIdentity;
use crate::storage::job_runs::{JobRunOutcome, NewJobRun};

fn now_iso() -> String {
    time::OffsetDateTime::now_utc()
        .format(&time::format_description::well_known::Rfc3339)
        .unwrap_or_else(|_| "1970-01-01T00:00:00Z".to_owned())
}

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

    pub(crate) fn activity_registry_map(&self) -> &Arc<Mutex<HashMap<i64, RegistryEntry>>> {
        &self.activity_registry
    }
}

/// One live direct-activity registry entry (ADR 0109 dec. 3, sol diff R1
/// #3). The map is keyed by a unique per-guard HANDLE, never by
/// `activity_key` — two concurrent guards under the SAME key each get their
/// own entry (a per-key set, since a key never appears more than once as a
/// map KEY but can back several live handles), so one guard's removal can
/// never evict a sibling's. The handle is the real `job_runs.id` once
/// `begin_attempt` succeeds, or a synthetic negative id (a disjoint,
/// process-local counter — real rowids are always ≥ 1) when `begin_attempt`
/// itself failed and no row exists to key by.
pub(crate) enum RegistryEntry {
    /// A `job_runs` row exists (the map key IS its id). `settle_failed`
    /// becomes `true` if a settle attempt against it errored: the entry is
    /// KEPT, never silently dropped, so the read model can surface it as
    /// `stalled` (the row is still durably `running`) rather than hide a
    /// live occurrence. Only a future session's startup reconcile clears it.
    Recorded { settle_failed: bool },
    /// `begin_attempt` failed — no row exists, but the work is genuinely
    /// running. Carries enough identity to render without a row to join.
    Unrecorded {
        identity: ActivityIdentity,
        started_at: String,
    },
}

/// Disjoint negative key space for [`RegistryEntry::Unrecorded`] handles
/// (real `job_runs.id` rowids are always positive).
static NEXT_UNRECORDED_HANDLE: AtomicI64 = AtomicI64::new(-1);

/// RAII guard for one direct-activity occurrence (ADR 0109 dec. 3): holds the
/// registry handle [`start`] opened. Call [`ActivityGuard::settle`]
/// with the work's outcome; if the guard is dropped without settling (an
/// unwind), the occurrence settles `interrupted` — a panic in awaited work can
/// never leave an open occurrence or a live registry entry behind.
pub struct ActivityGuard {
    state: AppState,
    handle: i64,
    settled: bool,
}

impl ActivityGuard {
    /// Settle the occurrence: `Ok(())` -> `succeeded`, `Err(message)` ->
    /// `failed`. Runs the settle + retention prune in ONE transaction, then
    /// removes the live registry entry (its own handle only) — UNLESS the
    /// settle attempt itself fails, in which case the entry is kept, marked
    /// `settle_failed` (sol diff R1 #3), never silently forgotten.
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
        if self.handle < 0 {
            // Unrecorded: no `job_runs` row exists to settle — just clear
            // the live entry (never a sibling's, since the handle is unique).
            if let Ok(mut map) = self.state.activity_registry_map().lock() {
                map.remove(&self.handle);
            }
            return;
        }
        match self.state.job_runs().settle(self.handle, outcome) {
            Ok(()) => {
                if let Ok(mut map) = self.state.activity_registry_map().lock() {
                    map.remove(&self.handle);
                }
            }
            Err(error) => {
                log::error!(
                    "activity registry: failed to settle occurrence {} — keeping it live as \
                     settle_failed (sol diff R1 #3), surfaced by the read model as stalled, \
                     cleared by the next startup reconcile: {error}",
                    self.handle
                );
                if let Ok(mut map) = self.state.activity_registry_map().lock() {
                    map.insert(
                        self.handle,
                        RegistryEntry::Recorded {
                            settle_failed: true,
                        },
                    );
                }
            }
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
/// inserts the live registry entry (ADR 0109 dec. 3). `run_key`/`kind` are
/// synthesized as `direct:<activity_key>` — used only for the `job_runs`
/// row, never written as a queue row.
///
/// A `begin_attempt` storage failure never blocks the caller's work — but
/// (sol diff R1 #3) it also never disappears the work from the ledger: the
/// registry still gets a live [`RegistryEntry::Unrecorded`] entry carrying
/// the identity itself, so the read model can render it as active even with
/// no backing row. `start` therefore always returns a guard.
pub fn start(state: &AppState, identity: ActivityIdentity) -> ActivityGuard {
    let run_key = format!("direct:{}", identity.activity_key);
    let new_run = NewJobRun {
        activity_key: identity.activity_key.clone(),
        run_key: run_key.clone(),
        kind: run_key,
        family: identity.family,
        company_id: identity.company_id.clone(),
        subject: identity.subject.clone(),
        target: identity.target.clone(),
        attempt: 1,
    };
    match state.job_runs().begin_attempt(new_run) {
        Ok(run_id) => {
            if let Ok(mut map) = state.activity_registry_map().lock() {
                map.insert(
                    run_id,
                    RegistryEntry::Recorded {
                        settle_failed: false,
                    },
                );
            }
            ActivityGuard {
                state: state.clone(),
                handle: run_id,
                settled: false,
            }
        }
        Err(error) => {
            log::warn!(
                "activity registry: begin_attempt failed: {error}; recording an unrecorded live \
                 entry so the work stays visible (sol diff R1 #3)"
            );
            let handle = NEXT_UNRECORDED_HANDLE.fetch_sub(1, Ordering::SeqCst);
            if let Ok(mut map) = state.activity_registry_map().lock() {
                map.insert(
                    handle,
                    RegistryEntry::Unrecorded {
                        identity,
                        started_at: now_iso(),
                    },
                );
            }
            ActivityGuard {
                state: state.clone(),
                handle,
                settled: false,
            }
        }
    }
}

/// A snapshot of the live direct-activity registry, split by how the read
/// model must render each entry (D3/ADR 0109 dec. 4, sol diff R1 #3):
/// `running` occurrences with a real row, `stalled` occurrences whose settle
/// attempt failed (still a real row, just not `active`), and `stalled`
/// entries with no row at all (a `begin_attempt` failure).
pub(crate) struct RegistrySnapshot {
    pub running_run_ids: Vec<i64>,
    pub stalled_run_ids: Vec<i64>,
    pub stalled_unrecorded: Vec<(ActivityIdentity, String)>,
}

/// Read the current live direct-activity entries, split for the read model.
/// Used by `commands::activity`'s `active`/summary composition.
pub(crate) fn snapshot(state: &AppState) -> RegistrySnapshot {
    let Ok(map) = state.activity_registry_map().lock() else {
        return RegistrySnapshot {
            running_run_ids: Vec::new(),
            stalled_run_ids: Vec::new(),
            stalled_unrecorded: Vec::new(),
        };
    };
    let mut running_run_ids = Vec::new();
    let mut stalled_run_ids = Vec::new();
    let mut stalled_unrecorded = Vec::new();
    for (handle, entry) in map.iter() {
        match entry {
            RegistryEntry::Recorded {
                settle_failed: false,
            } => running_run_ids.push(*handle),
            RegistryEntry::Recorded {
                settle_failed: true,
            } => stalled_run_ids.push(*handle),
            RegistryEntry::Unrecorded {
                identity,
                started_at,
            } => stalled_unrecorded.push((identity.clone(), started_at.clone())),
        }
    }
    RegistrySnapshot {
        running_run_ids,
        stalled_run_ids,
        stalled_unrecorded,
    }
}

/// Just the running handles — kept for call sites that only need `active`
/// (e.g. the summary's count), never the stalled split.
pub(crate) fn live_run_ids(state: &AppState) -> StorageResult<Vec<i64>> {
    Ok(snapshot(state).running_run_ids)
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

    fn occurrence_status(state: &AppState, run_id: i64) -> String {
        let connection = state.checkout_for_tests().expect("checkout");
        connection
            .query_row(
                "SELECT status FROM job_runs WHERE id = ?1",
                [run_id],
                |row| row.get(0),
            )
            .expect("status")
    }

    #[test]
    fn start_writes_a_running_row_and_a_live_entry() {
        let state = AppState::new(open_in_memory_database().expect("db"));
        let guard = start(&state, identity());
        assert_eq!(live_run_ids(&state).expect("live").len(), 1);
        assert_eq!(occurrence_status(&state, guard.handle), "running");
    }

    #[test]
    fn settle_ok_terminalizes_succeeded_and_clears_the_entry() {
        let state = AppState::new(open_in_memory_database().expect("db"));
        let guard = start(&state, identity());
        let run_id = guard.handle;
        guard.settle(Ok(()));

        assert!(live_run_ids(&state).expect("live").is_empty());
        assert_eq!(occurrence_status(&state, run_id), "succeeded");
    }

    #[test]
    fn direct_activity_panic_settles_interrupted() {
        // A guard dropped WITHOUT an explicit settle (the shape a panic mid-work
        // leaves behind, since the guard is never reached to call `.settle()`)
        // must still terminalize the occurrence (`interrupted`) and clear the
        // live entry, never leaving either dangling.
        let state = AppState::new(open_in_memory_database().expect("db"));
        let run_id = {
            let guard = start(&state, identity());
            guard.handle
            // `guard` drops here, never settled.
        };

        assert!(live_run_ids(&state).expect("live").is_empty());
        assert_eq!(occurrence_status(&state, run_id), "interrupted");
    }

    #[test]
    fn two_concurrent_same_key_guards_do_not_evict_each_other() {
        // sol diff R1 #3: the map used to be keyed by `activity_key` alone,
        // so two concurrent guards under the SAME key overwrote each other's
        // entry — either guard's removal could evict the OTHER's. Keyed by
        // handle (occurrence id) instead, both stay live independently and
        // each guard removes only its OWN entry.
        let state = AppState::new(open_in_memory_database().expect("db"));
        let first = start(&state, identity());
        let second = start(&state, identity());
        assert_ne!(
            first.handle, second.handle,
            "distinct handles even under the same key"
        );
        assert_eq!(
            live_run_ids(&state).expect("live").len(),
            2,
            "both concurrent attempts under the same activity_key stay visible"
        );

        let first_run_id = first.handle;
        let second_run_id = second.handle;
        first.settle(Ok(()));
        assert_eq!(
            live_run_ids(&state).expect("live"),
            vec![second_run_id],
            "settling the first guard must remove ONLY its own entry"
        );
        assert_eq!(occurrence_status(&state, first_run_id), "succeeded");
        assert_eq!(occurrence_status(&state, second_run_id), "running");

        second.settle(Ok(()));
        assert!(live_run_ids(&state).expect("live").is_empty());
    }

    #[test]
    fn a_forced_settle_failure_keeps_the_entry_live_as_settle_failed() {
        // sol diff R1 #3: settling the occurrence used to be logged-and-
        // ignored on failure, then the entry was removed anyway — a durable
        // `running` row could vanish from the live registry, hiding it from
        // the read model entirely. The entry must instead be KEPT, marked
        // `settle_failed`, so the read model can still surface it (as
        // stalled, via `snapshot`), never silently forgotten.
        let state = AppState::new(open_in_memory_database().expect("db"));
        let guard = start(&state, identity());
        let run_id = guard.handle;

        // Poison the settle UPDATE specifically (a BEFORE UPDATE trigger),
        // so `begin_attempt`'s earlier INSERT already succeeded fine.
        state
            .checkout_for_tests()
            .expect("checkout")
            .execute_batch(
                "CREATE TRIGGER poison_settle BEFORE UPDATE ON job_runs
                 BEGIN SELECT RAISE(ABORT, 'settle poisoned for test'); END;",
            )
            .expect("install poison trigger");

        guard.settle(Ok(()));

        // The occurrence itself never actually settled (the trigger aborted
        // the transaction) — still `running` in storage.
        assert_eq!(occurrence_status(&state, run_id), "running");
        // But the entry stays LIVE in the registry (never silently dropped),
        // surfaced by `snapshot` as a stalled run id, never `running`.
        let snapshot = snapshot(&state);
        assert!(
            snapshot.running_run_ids.is_empty(),
            "a settle-failed entry must never still count as running"
        );
        assert_eq!(snapshot.stalled_run_ids, vec![run_id]);
    }

    #[test]
    fn a_begin_attempt_failure_still_yields_a_live_unrecorded_entry() {
        // sol diff R1 #3: `start` used to return `None` on a `begin_attempt`
        // failure, and callers just ran the work unrecorded — invisible to
        // the read model. It must instead stay visible via an `Unrecorded`
        // registry entry carrying the identity itself (no row to join).
        let state = AppState::new(open_in_memory_database().expect("db"));
        state
            .checkout_for_tests()
            .expect("checkout")
            .execute("DROP TABLE job_runs", [])
            .expect("poison job_runs so begin_attempt fails");

        let guard = start(&state, identity());
        assert!(
            guard.handle < 0,
            "an unrecorded entry uses the synthetic negative key space"
        );

        let snapshot = snapshot(&state);
        assert!(snapshot.running_run_ids.is_empty());
        assert!(snapshot.stalled_run_ids.is_empty());
        assert_eq!(snapshot.stalled_unrecorded.len(), 1);
        assert_eq!(
            snapshot.stalled_unrecorded[0].0.activity_key,
            identity().activity_key
        );

        // Settling still clears the entry (no row to update, just removal).
        guard.settle(Ok(()));
        assert!(super::snapshot(&state).stalled_unrecorded.is_empty());
    }
}
