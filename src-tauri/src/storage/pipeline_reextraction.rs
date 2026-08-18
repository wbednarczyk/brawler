//! Version-aware re-extraction batch storage (epic #398 Item B, migration
//! `0146`).
//!
//! One durable row per batch ([`pipeline_reextraction_batches`]): the record
//! behind batch progress and the Coverage panel's status line, mirroring
//! `history_sweeps.rs`'s shape exactly — but a SEPARATE table, because a
//! batch's candidate selector (successful ESEF-tier runs with a stale stored
//! pipeline version) is unrelated to the history sweep's own (periods lacking
//! facts) and must never share its "never re-arm an emitted run" rule. Reach
//! the store via `AppState::pipeline_reextraction()`.

use super::database::Database;
use super::*;

/// A version-aware re-extraction batch record (read model). `enqueued_run_ids`
/// is parsed from the JSON column so the progress command can derive per-run
/// status without a parallel query.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[cfg_attr(feature = "ts-export", derive(ts_rs::TS))]
#[cfg_attr(
    feature = "ts-export",
    ts(export, export_to = "../../src/api/generated/")
)]
#[serde(rename_all = "camelCase")]
pub struct PipelineReextractionBatch {
    pub id: String,
    pub company_id: String,
    /// `queued` | `running` | `completed` | `failed`.
    pub status: String,
    /// Successful ESEF-tier runs found with a stale stored pipeline version.
    pub candidates_total: i64,
    /// Candidates successfully re-armed (`rearm_run` + `enqueue_first_stage`).
    pub runs_enqueued: i64,
    /// Candidates a storage error prevented re-arming.
    pub runs_failed: i64,
    /// The `autopilot_run` ids this batch re-armed.
    pub enqueued_run_ids: Vec<String>,
    /// A storage-level abort that failed the whole batch.
    pub error: Option<String>,
    pub created_at: String,
    pub updated_at: String,
}

/// The counted outcome of one batch pass, written when the batch completes.
#[derive(Debug, Clone, Default)]
pub struct PipelineReextractionOutcome {
    pub candidates_total: i64,
    pub runs_enqueued: i64,
    pub runs_failed: i64,
    pub enqueued_run_ids: Vec<String>,
}

/// Version-aware re-extraction batch domain store.
#[derive(Clone)]
pub struct PipelineReextractionStore {
    db: Database,
}

impl PipelineReextractionStore {
    pub(super) fn new(db: Database) -> Self {
        Self { db }
    }

    /// Create a queued batch for a company. The id is unique per batch
    /// (`pipeline_reextraction:{company}:{nanos}`), collision-checked so two
    /// batches in the same instant never share a row.
    pub fn create_batch(&self, company_id: &str) -> StorageResult<PipelineReextractionBatch> {
        let connection = self.db.checkout()?;
        let id = next_batch_id(&connection, company_id)?;
        connection.execute(
            "INSERT INTO pipeline_reextraction_batches (id, company_id) VALUES (?1, ?2)",
            params![id, company_id],
        )?;
        drop(connection);
        self.get_batch(&id)
    }

    /// Fetch one batch by id.
    pub fn get_batch(&self, id: &str) -> StorageResult<PipelineReextractionBatch> {
        let connection = self.db.checkout()?;
        connection
            .query_row(
                "SELECT * FROM pipeline_reextraction_batches WHERE id = ?1",
                [id],
                map_batch_row,
            )
            .map_err(StorageError::from)
    }

    /// The most recent batch for a company (newest by `created_at`), or `None`
    /// when the company has never had one (reads tolerate a missing row).
    pub fn get_latest_batch(
        &self,
        company_id: &str,
    ) -> StorageResult<Option<PipelineReextractionBatch>> {
        let connection = self.db.checkout()?;
        connection
            .query_row(
                "
                SELECT * FROM pipeline_reextraction_batches
                WHERE company_id = ?1
                ORDER BY created_at DESC, id DESC
                LIMIT 1
                ",
                [company_id],
                map_batch_row,
            )
            .optional()
            .map_err(StorageError::from)
    }

    /// Move a batch to `running`.
    pub fn mark_batch_running(&self, id: &str) -> StorageResult<PipelineReextractionBatch> {
        let connection = self.db.checkout()?;
        connection.execute(
            "
            UPDATE pipeline_reextraction_batches
            SET status = 'running',
                updated_at = strftime('%Y-%m-%dT%H:%M:%fZ', 'now')
            WHERE id = ?1
            ",
            params![id],
        )?;
        drop(connection);
        self.get_batch(id)
    }

    /// Finalize a batch as `completed`, recording its counters and the
    /// re-armed run ids. A completed batch with `runs_failed > 0` still
    /// completes — the count records the partial failure honestly rather
    /// than aborting the whole batch.
    pub fn complete_batch(
        &self,
        id: &str,
        outcome: &PipelineReextractionOutcome,
    ) -> StorageResult<PipelineReextractionBatch> {
        let run_ids_json =
            serde_json::to_string(&outcome.enqueued_run_ids).unwrap_or_else(|_| "[]".to_owned());
        let connection = self.db.checkout()?;
        connection.execute(
            "
            UPDATE pipeline_reextraction_batches
            SET status = 'completed',
                candidates_total = ?2,
                runs_enqueued = ?3,
                runs_failed = ?4,
                enqueued_run_ids_json = ?5,
                updated_at = strftime('%Y-%m-%dT%H:%M:%fZ', 'now')
            WHERE id = ?1
            ",
            params![
                id,
                outcome.candidates_total,
                outcome.runs_enqueued,
                outcome.runs_failed,
                run_ids_json,
            ],
        )?;
        drop(connection);
        self.get_batch(id)
    }

    /// Finalize a batch as `failed` with a storage-level error (the batch
    /// could not be driven at all — e.g. its candidates could not be listed).
    pub fn fail_batch(&self, id: &str, error: &str) -> StorageResult<PipelineReextractionBatch> {
        let connection = self.db.checkout()?;
        connection.execute(
            "
            UPDATE pipeline_reextraction_batches
            SET status = 'failed',
                error = ?2,
                updated_at = strftime('%Y-%m-%dT%H:%M:%fZ', 'now')
            WHERE id = ?1
            ",
            params![id, error],
        )?;
        drop(connection);
        self.get_batch(id)
    }
}

/// A unique batch id (`pipeline_reextraction:{company}:{nanos}`), bumped on
/// the rare same-instant collision so a rapid pair of batches never share a
/// row.
fn next_batch_id(connection: &Connection, company_id: &str) -> StorageResult<String> {
    let nanos = time::OffsetDateTime::now_utc().unix_timestamp_nanos();
    let base = format!("pipeline_reextraction:{company_id}:{nanos}");
    let mut candidate = base.clone();
    let mut suffix = 2;
    while batch_exists(connection, &candidate)? {
        candidate = format!("{base}_{suffix}");
        suffix += 1;
    }
    Ok(candidate)
}

fn batch_exists(connection: &Connection, id: &str) -> StorageResult<bool> {
    connection
        .query_row(
            "SELECT EXISTS(SELECT 1 FROM pipeline_reextraction_batches WHERE id = ?1)",
            [id],
            |row| row.get(0),
        )
        .map_err(StorageError::from)
}

fn map_batch_row(row: &rusqlite::Row<'_>) -> rusqlite::Result<PipelineReextractionBatch> {
    let run_ids_json: Option<String> = row.get("enqueued_run_ids_json")?;
    let enqueued_run_ids = run_ids_json
        .and_then(|json| serde_json::from_str::<Vec<String>>(&json).ok())
        .unwrap_or_default();
    Ok(PipelineReextractionBatch {
        id: row.get("id")?,
        company_id: row.get("company_id")?,
        status: row.get("status")?,
        candidates_total: row.get("candidates_total")?,
        runs_enqueued: row.get("runs_enqueued")?,
        runs_failed: row.get("runs_failed")?,
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
        let batch = s
            .pipeline_reextraction()
            .create_batch(&c)
            .expect("create batch");
        assert_eq!(batch.company_id, c);
        assert_eq!(batch.status, "queued");
        assert_eq!(batch.candidates_total, 0);
        assert_eq!(batch.runs_enqueued, 0);
        assert!(batch.enqueued_run_ids.is_empty());
    }

    #[test]
    fn latest_returns_none_then_the_newest_batch() {
        let s = state();
        let c = company(&s);
        assert!(s
            .pipeline_reextraction()
            .get_latest_batch(&c)
            .expect("latest")
            .is_none());

        let first = s.pipeline_reextraction().create_batch(&c).expect("first");
        let second = s.pipeline_reextraction().create_batch(&c).expect("second");
        assert_ne!(first.id, second.id);

        let latest = s
            .pipeline_reextraction()
            .get_latest_batch(&c)
            .expect("latest")
            .expect("a batch exists");
        assert_eq!(latest.id, second.id);
    }

    #[test]
    fn complete_records_counters_and_run_ids() {
        let s = state();
        let c = company(&s);
        let batch = s.pipeline_reextraction().create_batch(&c).expect("create");
        s.pipeline_reextraction()
            .mark_batch_running(&batch.id)
            .expect("running");

        let outcome = PipelineReextractionOutcome {
            candidates_total: 2,
            runs_enqueued: 2,
            runs_failed: 0,
            enqueued_run_ids: vec![
                "autopilot_run:c:d1".to_owned(),
                "autopilot_run:c:d2".to_owned(),
            ],
        };
        let completed = s
            .pipeline_reextraction()
            .complete_batch(&batch.id, &outcome)
            .expect("complete");
        assert_eq!(completed.status, "completed");
        assert_eq!(completed.candidates_total, 2);
        assert_eq!(completed.runs_enqueued, 2);
        assert_eq!(completed.enqueued_run_ids.len(), 2);
    }

    #[test]
    fn fail_records_the_error() {
        let s = state();
        let c = company(&s);
        let batch = s.pipeline_reextraction().create_batch(&c).expect("create");
        let failed = s
            .pipeline_reextraction()
            .fail_batch(&batch.id, "candidates unavailable")
            .expect("fail");
        assert_eq!(failed.status, "failed");
        assert_eq!(failed.error.as_deref(), Some("candidates unavailable"));
    }
}
