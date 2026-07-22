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
    /// The tier-4 budget snapshotted onto this sweep at creation (ADR 0077 §6);
    /// `0` means unlimited. A mid-sweep settings change never moves this gate.
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
        // The tier-4 AI budget columns are orphaned in place (ADR 0084 decision
        // 4 retires tier-4; decision 5 forbids destructive migrations), so a new
        // sweep simply leaves them at their schema defaults.
        connection.execute(
            "
            INSERT INTO history_sweeps (id, company_id, trigger)
            VALUES (?1, ?2, ?3)
            ",
            params![id, company_id, trigger],
        )?;
        drop(connection);
        self.get_history_sweep(&id)
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
}
