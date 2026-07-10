//! History sweep storage operations (ADR 0077 §3, T3.2).
//!
//! One durable row per history sweep ([`history_sweeps`]): the backfill/manual
//! counterpart to the refresh-time detection sweep. A sweep enqueues a full
//! autopilot run for every canonical periodic report whose period still lacks
//! accepted facts, through the shared `enqueue_extraction_run`; this store is the
//! record behind sweep progress (docs swept, runs enqueued, budget) and the
//! coverage panel's status line. Reach the store via `AppState::history_sweeps()`.

use super::database::Database;
use super::*;

/// A history sweep record (read model). `enqueued_run_ids` is parsed from the
/// JSON column so the progress command can derive per-run status without a
/// parallel query.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[cfg_attr(feature = "ts-export", derive(ts_rs::TS))]
#[cfg_attr(
    feature = "ts-export",
    ts(export, export_to = "../../src/api/generated/")
)]
#[serde(rename_all = "camelCase")]
pub struct HistorySweep {
    pub id: String,
    pub company_id: String,
    /// `backfill` (chained from `run_backfill`) | `manual` ("Extract missing periods").
    pub trigger: String,
    /// `queued` | `running` | `completed` | `failed`.
    pub status: String,
    /// Canonical periods that needed extracting when the sweep ran.
    pub candidates_total: i64,
    /// Runs freshly created or re-armed (`Created` | `Rearmed`).
    pub runs_enqueued: i64,
    /// Candidates whose extraction run was already terminal (`DedupedTerminal`).
    pub skipped_existing: i64,
    /// Candidates a storage error prevented enqueuing (`Failed`).
    pub runs_failed: i64,
    /// Why the sweep enqueued nothing, when it did (e.g. `automation_off` for a
    /// company in mode `off`; ADR 0077 §3 amendment (c) — never a silent skip).
    pub skipped_reason: Option<String>,
    /// The `autopilot_run` ids this sweep enqueued.
    pub enqueued_run_ids: Vec<String>,
    /// Tier-4 AI call units this sweep has spent so far (ADR 0077 §6). Bumped
    /// atomically as each document enters tier-4; one unit = one invocation.
    pub ai_calls_used: i64,
    /// The tier-4 budget snapshotted onto this sweep at creation (ADR 0077 §6);
    /// `0` means unlimited. A mid-sweep settings change never moves this gate.
    pub ai_call_limit: i64,
    /// A storage-level abort that failed the whole sweep.
    pub error: Option<String>,
    pub created_at: String,
    pub updated_at: String,
}

/// The counted outcome of one sweep pass, written when the sweep completes.
#[derive(Debug, Clone, Default)]
pub struct HistorySweepOutcome {
    pub candidates_total: i64,
    pub runs_enqueued: i64,
    pub skipped_existing: i64,
    pub runs_failed: i64,
    pub skipped_reason: Option<String>,
    pub enqueued_run_ids: Vec<String>,
}

/// History sweep domain store.
#[derive(Clone)]
pub struct HistorySweepStore {
    db: Database,
}

impl HistorySweepStore {
    pub(super) fn new(db: Database) -> Self {
        Self { db }
    }

    /// Create a queued sweep for a company. The id is unique per sweep
    /// (`history_sweep:{company}:{nanos}`), collision-checked so two sweeps in the
    /// same instant never share a row.
    pub fn create_history_sweep(
        &self,
        company_id: &str,
        trigger: &str,
    ) -> StorageResult<HistorySweep> {
        let connection = self.db.checkout()?;
        let id = next_sweep_id(&connection, company_id)?;
        // Snapshot the tier-4 budget onto the row at creation (ADR 0077 §6): the
        // gate this sweep enforces is fixed here, so a mid-sweep settings change
        // never moves it. A settings read error (a database that predates the
        // key still reads tolerantly, so this is rare) falls back to the default.
        let ai_call_limit = super::settings::get_settings(&connection)
            .map(|s| s.history_sweep_ai_call_limit)
            .unwrap_or(super::settings::HISTORY_SWEEP_AI_CALL_LIMIT_DEFAULT);
        connection.execute(
            "
            INSERT INTO history_sweeps (id, company_id, trigger, ai_call_limit)
            VALUES (?1, ?2, ?3, ?4)
            ",
            params![id, company_id, trigger, ai_call_limit],
        )?;
        drop(connection);
        self.get_history_sweep(&id)
    }

    /// Atomically charge one tier-4 unit against this sweep's budget (ADR 0077 §6,
    /// decision 3). Returns `true` when the unit was granted (`changes() == 1`),
    /// `false` when the budget is exhausted. A single guarded `UPDATE` is the whole
    /// check-and-consume: the `WHERE` clause admits the write only while a unit
    /// remains (`ai_call_limit = 0` ⇒ unlimited), so two concurrent callers can
    /// never both succeed past the last unit. Deterministic outcomes never call
    /// this — only a run actually entering tier-4 spends budget (decision 4).
    pub fn try_consume_sweep_ai_budget(&self, sweep_id: &str) -> StorageResult<bool> {
        let connection = self.db.checkout()?;
        let changed = connection.execute(
            "
            UPDATE history_sweeps
            SET ai_calls_used = ai_calls_used + 1,
                updated_at = strftime('%Y-%m-%dT%H:%M:%fZ', 'now')
            WHERE id = ?1
                AND (ai_call_limit = 0 OR ai_calls_used < ai_call_limit)
            ",
            params![sweep_id],
        )?;
        Ok(changed == 1)
    }

    /// Fetch one sweep by id.
    pub fn get_history_sweep(&self, id: &str) -> StorageResult<HistorySweep> {
        let connection = self.db.checkout()?;
        connection
            .query_row(
                "SELECT * FROM history_sweeps WHERE id = ?1",
                [id],
                map_sweep_row,
            )
            .map_err(StorageError::from)
    }

    /// The most recent sweep for a company (newest by `created_at`), or `None` when
    /// the company has never been swept (reads tolerate a missing row).
    pub fn get_latest_history_sweep(
        &self,
        company_id: &str,
    ) -> StorageResult<Option<HistorySweep>> {
        let connection = self.db.checkout()?;
        connection
            .query_row(
                "
                SELECT * FROM history_sweeps
                WHERE company_id = ?1
                ORDER BY created_at DESC, id DESC
                LIMIT 1
                ",
                [company_id],
                map_sweep_row,
            )
            .optional()
            .map_err(StorageError::from)
    }

    /// Move a sweep to `running`.
    pub fn mark_history_sweep_running(&self, id: &str) -> StorageResult<HistorySweep> {
        let connection = self.db.checkout()?;
        connection.execute(
            "
            UPDATE history_sweeps
            SET status = 'running',
                updated_at = strftime('%Y-%m-%dT%H:%M:%fZ', 'now')
            WHERE id = ?1
            ",
            params![id],
        )?;
        drop(connection);
        self.get_history_sweep(id)
    }

    /// Finalize a sweep as `completed`, recording its counters, the enqueued run
    /// ids, and any `skipped_reason` (e.g. `automation_off`). A completed sweep
    /// with `runs_failed > 0` still completes — the count records the partial
    /// failure honestly rather than aborting the whole sweep.
    pub fn complete_history_sweep(
        &self,
        id: &str,
        outcome: &HistorySweepOutcome,
    ) -> StorageResult<HistorySweep> {
        let run_ids_json =
            serde_json::to_string(&outcome.enqueued_run_ids).unwrap_or_else(|_| "[]".to_owned());
        let connection = self.db.checkout()?;
        connection.execute(
            "
            UPDATE history_sweeps
            SET status = 'completed',
                candidates_total = ?2,
                runs_enqueued = ?3,
                skipped_existing = ?4,
                runs_failed = ?5,
                skipped_reason = ?6,
                enqueued_run_ids_json = ?7,
                updated_at = strftime('%Y-%m-%dT%H:%M:%fZ', 'now')
            WHERE id = ?1
            ",
            params![
                id,
                outcome.candidates_total,
                outcome.runs_enqueued,
                outcome.skipped_existing,
                outcome.runs_failed,
                outcome.skipped_reason,
                run_ids_json,
            ],
        )?;
        drop(connection);
        self.get_history_sweep(id)
    }

    /// Finalize a sweep as `failed` with a storage-level error (the sweep could
    /// not be driven at all — e.g. its candidates could not be listed).
    pub fn fail_history_sweep(&self, id: &str, error: &str) -> StorageResult<HistorySweep> {
        let connection = self.db.checkout()?;
        connection.execute(
            "
            UPDATE history_sweeps
            SET status = 'failed',
                error = ?2,
                updated_at = strftime('%Y-%m-%dT%H:%M:%fZ', 'now')
            WHERE id = ?1
            ",
            params![id, error],
        )?;
        drop(connection);
        self.get_history_sweep(id)
    }
}

/// A unique sweep id (`history_sweep:{company}:{nanos}`), bumped on the rare
/// same-instant collision so a rapid pair of sweeps never share a row.
fn next_sweep_id(connection: &Connection, company_id: &str) -> StorageResult<String> {
    let nanos = time::OffsetDateTime::now_utc().unix_timestamp_nanos();
    let base = format!("history_sweep:{company_id}:{nanos}");
    let mut candidate = base.clone();
    let mut suffix = 2;
    while sweep_exists(connection, &candidate)? {
        candidate = format!("{base}_{suffix}");
        suffix += 1;
    }
    Ok(candidate)
}

fn sweep_exists(connection: &Connection, id: &str) -> StorageResult<bool> {
    connection
        .query_row(
            "SELECT EXISTS(SELECT 1 FROM history_sweeps WHERE id = ?1)",
            [id],
            |row| row.get(0),
        )
        .map_err(StorageError::from)
}

fn map_sweep_row(row: &rusqlite::Row<'_>) -> rusqlite::Result<HistorySweep> {
    let run_ids_json: Option<String> = row.get("enqueued_run_ids_json")?;
    let enqueued_run_ids = run_ids_json
        .and_then(|json| serde_json::from_str::<Vec<String>>(&json).ok())
        .unwrap_or_default();
    Ok(HistorySweep {
        id: row.get("id")?,
        company_id: row.get("company_id")?,
        trigger: row.get("trigger")?,
        status: row.get("status")?,
        candidates_total: row.get("candidates_total")?,
        runs_enqueued: row.get("runs_enqueued")?,
        skipped_existing: row.get("skipped_existing")?,
        runs_failed: row.get("runs_failed")?,
        skipped_reason: row.get("skipped_reason")?,
        enqueued_run_ids,
        ai_calls_used: row.get("ai_calls_used")?,
        ai_call_limit: row.get("ai_call_limit")?,
        error: row.get("error")?,
        created_at: row.get("created_at")?,
        updated_at: row.get("updated_at")?,
    })
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
    fn create_starts_queued_with_zeroed_counters() {
        let s = state();
        let c = company(&s);
        let sweep = s
            .history_sweeps()
            .create_history_sweep(&c, "manual")
            .expect("create sweep");
        assert_eq!(sweep.company_id, c);
        assert_eq!(sweep.trigger, "manual");
        assert_eq!(sweep.status, "queued");
        assert_eq!(sweep.candidates_total, 0);
        assert_eq!(sweep.runs_enqueued, 0);
        assert!(sweep.enqueued_run_ids.is_empty());
        assert!(sweep.skipped_reason.is_none());
    }

    #[test]
    fn latest_returns_none_then_the_newest_sweep() {
        let s = state();
        let c = company(&s);
        assert!(s
            .history_sweeps()
            .get_latest_history_sweep(&c)
            .expect("latest")
            .is_none());

        let first = s
            .history_sweeps()
            .create_history_sweep(&c, "backfill")
            .expect("first");
        let second = s
            .history_sweeps()
            .create_history_sweep(&c, "manual")
            .expect("second");
        // Distinct ids even created back-to-back.
        assert_ne!(first.id, second.id);

        let latest = s
            .history_sweeps()
            .get_latest_history_sweep(&c)
            .expect("latest")
            .expect("a sweep exists");
        assert_eq!(latest.id, second.id);
    }

    #[test]
    fn complete_records_counters_and_run_ids() {
        let s = state();
        let c = company(&s);
        let sweep = s
            .history_sweeps()
            .create_history_sweep(&c, "backfill")
            .expect("create");
        s.history_sweeps()
            .mark_history_sweep_running(&sweep.id)
            .expect("running");

        let outcome = HistorySweepOutcome {
            candidates_total: 3,
            runs_enqueued: 2,
            skipped_existing: 1,
            runs_failed: 0,
            skipped_reason: None,
            enqueued_run_ids: vec![
                "autopilot_run:c:d1".to_owned(),
                "autopilot_run:c:d2".to_owned(),
            ],
        };
        let completed = s
            .history_sweeps()
            .complete_history_sweep(&sweep.id, &outcome)
            .expect("complete");
        assert_eq!(completed.status, "completed");
        assert_eq!(completed.candidates_total, 3);
        assert_eq!(completed.runs_enqueued, 2);
        assert_eq!(completed.skipped_existing, 1);
        assert_eq!(completed.enqueued_run_ids.len(), 2);
    }

    #[test]
    fn complete_with_skipped_reason_records_it() {
        let s = state();
        let c = company(&s);
        let sweep = s
            .history_sweeps()
            .create_history_sweep(&c, "manual")
            .expect("create");
        let outcome = HistorySweepOutcome {
            skipped_reason: Some("automation_off".to_owned()),
            ..Default::default()
        };
        let completed = s
            .history_sweeps()
            .complete_history_sweep(&sweep.id, &outcome)
            .expect("complete");
        assert_eq!(completed.status, "completed");
        assert_eq!(completed.skipped_reason.as_deref(), Some("automation_off"));
        assert_eq!(completed.runs_enqueued, 0);
    }

    #[test]
    fn fail_records_the_error() {
        let s = state();
        let c = company(&s);
        let sweep = s
            .history_sweeps()
            .create_history_sweep(&c, "manual")
            .expect("create");
        let failed = s
            .history_sweeps()
            .fail_history_sweep(&sweep.id, "candidates unavailable")
            .expect("fail");
        assert_eq!(failed.status, "failed");
        assert_eq!(failed.error.as_deref(), Some("candidates unavailable"));
    }

    // ---- tier-4 AI budget (ADR 0077 §6, T5.2) ------------------------------

    /// A `state()` whose `history_sweep_ai_call_limit` setting is `limit`, so a
    /// sweep created afterwards snapshots that ceiling.
    fn state_with_limit(limit: i64) -> AppState {
        let s = state();
        s.update_settings(crate::storage::SettingsUpdate {
            history_sweep_ai_call_limit: Some(limit),
            ..Default::default()
        })
        .expect("set limit");
        s
    }

    fn new_sweep(state: &AppState, company_id: &str) -> HistorySweep {
        state
            .history_sweeps()
            .create_history_sweep(company_id, "manual")
            .expect("create sweep")
    }

    #[test]
    fn create_snapshots_the_current_ai_call_limit() {
        let s = state_with_limit(7);
        let c = company(&s);
        let sweep = new_sweep(&s, &c);
        assert_eq!(sweep.ai_call_limit, 7, "limit snapshotted at creation");
        assert_eq!(sweep.ai_calls_used, 0);
    }

    #[test]
    fn try_consume_grants_exactly_the_limit_then_denies() {
        // G-4 at the storage layer: a limit of 2 grants two units, denies the
        // third; exactly two are recorded as spent.
        let s = state_with_limit(2);
        let c = company(&s);
        let sweep = new_sweep(&s, &c);
        let store = s.history_sweeps();
        assert!(store
            .try_consume_sweep_ai_budget(&sweep.id)
            .expect("unit 1"));
        assert!(store
            .try_consume_sweep_ai_budget(&sweep.id)
            .expect("unit 2"));
        assert!(
            !store
                .try_consume_sweep_ai_budget(&sweep.id)
                .expect("unit 3 denied"),
            "the third unit is over budget"
        );
        assert_eq!(
            store
                .get_history_sweep(&sweep.id)
                .expect("reload")
                .ai_calls_used,
            2,
            "exactly two units spent"
        );
    }

    #[test]
    fn try_consume_is_unlimited_when_limit_is_zero() {
        // (b) 0 = off: no cap, every consume is granted.
        let s = state_with_limit(0);
        let c = company(&s);
        let sweep = new_sweep(&s, &c);
        assert_eq!(sweep.ai_call_limit, 0);
        let store = s.history_sweeps();
        for unit in 0..5 {
            assert!(
                store
                    .try_consume_sweep_ai_budget(&sweep.id)
                    .expect("granted"),
                "unit {unit} must be granted with limit 0"
            );
        }
        assert_eq!(
            store
                .get_history_sweep(&sweep.id)
                .expect("reload")
                .ai_calls_used,
            5
        );
    }

    #[test]
    fn budget_snapshot_governs_over_a_later_settings_change() {
        // (f) The limit is fixed at creation: shrinking the setting mid-sweep does
        // not move an already-created sweep's gate.
        let s = state_with_limit(30);
        let c = company(&s);
        let sweep = new_sweep(&s, &c);
        assert_eq!(sweep.ai_call_limit, 30);
        s.update_settings(crate::storage::SettingsUpdate {
            history_sweep_ai_call_limit: Some(1),
            ..Default::default()
        })
        .expect("shrink setting to 1");
        let store = s.history_sweeps();
        assert!(store
            .try_consume_sweep_ai_budget(&sweep.id)
            .expect("unit 1"));
        assert!(
            store
                .try_consume_sweep_ai_budget(&sweep.id)
                .expect("unit 2"),
            "the snapshot (30), not the new setting (1), governs"
        );
    }

    #[test]
    fn try_consume_is_atomic_under_concurrency() {
        // (c) decision 3: two threads racing for the last unit — exactly one wins.
        // The in-memory test DB is a single mutex-guarded connection, which would
        // serialize the race away, so this uses a file-backed pool (real parallel
        // connections under WAL); the guarded UPDATE makes check-and-consume atomic.
        use std::sync::{Arc, Barrier};
        let dir = std::env::temp_dir().join(format!(
            "brawler-sweep-budget-conc-{}-{}",
            std::process::id(),
            time::OffsetDateTime::now_utc().unix_timestamp_nanos()
        ));
        std::fs::create_dir_all(&dir).expect("dir");
        let state = crate::storage::open_pool(dir.join("brawler.sqlite3"), dir.clone())
            .expect("open pooled db");
        state
            .update_settings(crate::storage::SettingsUpdate {
                history_sweep_ai_call_limit: Some(1),
                ..Default::default()
            })
            .expect("set limit 1");
        let c = state
            .create_company(NewCompany {
                exchange: "GPW".to_owned(),
                ticker: "TST".to_owned(),
                display_name: "Test S.A.".to_owned(),
                isin: None,
                cik: None,
                lei: None,
            })
            .expect("company")
            .id;
        let sweep = new_sweep(&state, &c);

        let barrier = Arc::new(Barrier::new(2));
        let handles: Vec<_> = (0..2)
            .map(|_| {
                let state = state.clone();
                let barrier = barrier.clone();
                let sweep_id = sweep.id.clone();
                std::thread::spawn(move || {
                    barrier.wait();
                    state
                        .history_sweeps()
                        .try_consume_sweep_ai_budget(&sweep_id)
                        .expect("consume")
                })
            })
            .collect();
        let wins = handles
            .into_iter()
            .map(|h| h.join().expect("thread"))
            .filter(|&won| won)
            .count();
        assert_eq!(wins, 1, "exactly one thread may claim the single unit");
        assert_eq!(
            state
                .history_sweeps()
                .get_history_sweep(&sweep.id)
                .expect("reload")
                .ai_calls_used,
            1
        );
    }
}
