//! Durable job-queue storage operations (Architecture v2 / ADR 0050).
//!
//! The `job_queue` table is a persisted work list; this module is the domain
//! store ([`JobQueueStore`], AV1 pattern) that enqueues, atomically claims,
//! completes, retries, and reclaims rows. The in-process worker
//! (`crate::jobs::queue`) drives it. Local-first: a single worker runs only
//! while the app is open.

use super::database::Database;
use super::*;

/// A job claimed for execution. The handler deserializes [`payload`](Self::payload)
/// for its `kind`.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ClaimedJob {
    pub id: String,
    pub kind: String,
    pub payload: String,
    pub attempts: i64,
    pub max_attempts: i64,
}

/// Status tallies for observability/diagnostics.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct JobQueueCounts {
    pub pending: i64,
    pub running: i64,
    pub succeeded: i64,
    pub failed: i64,
}

/// Job-queue domain store. Reach it via `AppState::jobs()`.
#[derive(Clone)]
pub struct JobQueueStore {
    db: Database,
}

impl JobQueueStore {
    pub(super) fn new(db: Database) -> Self {
        Self { db }
    }

    /// Enqueue a job idempotently keyed by `id`. Returns `true` if a new row was
    /// inserted, `false` if a row with that id already exists (dedup) — so an
    /// idempotency key like `"content_embedding:feed_item:<id>"` collapses
    /// duplicate enqueues. `payload` is opaque JSON the handler deserializes.
    pub fn enqueue(
        &self,
        id: &str,
        kind: &str,
        payload: &str,
        max_attempts: i64,
    ) -> StorageResult<bool> {
        let connection = self.db.checkout()?;
        let inserted = connection.execute(
            "
            INSERT OR IGNORE INTO job_queue (id, kind, payload, max_attempts)
            VALUES (?1, ?2, ?3, ?4)
            ",
            params![id, kind, payload, max_attempts.max(1)],
        )?;
        Ok(inserted > 0)
    }

    /// Re-arm a recurring job under a **stable** id: insert it, or reset an
    /// existing terminal (`succeeded`/`failed`) or `pending` row back to `pending`
    /// for another run — while leaving a `running` row untouched (so an in-flight
    /// job is never disturbed or double-run). This is the scheduler primitive for
    /// periodic work (e.g. `source_refresh:<adapter>`): one row per recurring job,
    /// re-runnable on each tick, so the queue does not accumulate a row per fire.
    /// Returns `true` if the job is now runnable (newly inserted or reset).
    pub fn reschedule(
        &self,
        id: &str,
        kind: &str,
        payload: &str,
        max_attempts: i64,
    ) -> StorageResult<bool> {
        let connection = self.db.checkout()?;
        let changed = connection.execute(
            "
            INSERT INTO job_queue (id, kind, payload, max_attempts)
            VALUES (?1, ?2, ?3, ?4)
            ON CONFLICT(id) DO UPDATE SET
                status = 'pending',
                attempts = 0,
                available_at = strftime('%Y-%m-%dT%H:%M:%fZ', 'now'),
                last_error = NULL,
                payload = excluded.payload,
                updated_at = strftime('%Y-%m-%dT%H:%M:%fZ', 'now')
            WHERE job_queue.status != 'running'
            ",
            params![id, kind, payload, max_attempts.max(1)],
        )?;
        Ok(changed > 0)
    }

    /// Atomically claim the next runnable job: the oldest `pending` row whose
    /// `available_at` has passed. The claim and the `attempts` increment happen
    /// in one statement, so two workers can never claim the same row and a crash
    /// mid-run still counts as an attempt. Returns `None` when nothing is
    /// runnable.
    pub fn claim_next(&self) -> StorageResult<Option<ClaimedJob>> {
        let connection = self.db.checkout()?;
        connection
            .query_row(
                "
                UPDATE job_queue
                SET status = 'running',
                    attempts = attempts + 1,
                    locked_at = strftime('%Y-%m-%dT%H:%M:%fZ', 'now'),
                    updated_at = strftime('%Y-%m-%dT%H:%M:%fZ', 'now')
                WHERE id = (
                    SELECT id
                    FROM job_queue
                    WHERE status = 'pending'
                        AND available_at <= strftime('%Y-%m-%dT%H:%M:%fZ', 'now')
                    ORDER BY available_at, created_at
                    LIMIT 1
                )
                RETURNING id, kind, payload, attempts, max_attempts
                ",
                [],
                |row| {
                    Ok(ClaimedJob {
                        id: row.get(0)?,
                        kind: row.get(1)?,
                        payload: row.get(2)?,
                        attempts: row.get(3)?,
                        max_attempts: row.get(4)?,
                    })
                },
            )
            .optional()
            .map_err(StorageError::from)
    }

    /// Mark a claimed job succeeded (terminal).
    pub fn mark_succeeded(&self, id: &str) -> StorageResult<()> {
        let connection = self.db.checkout()?;
        connection.execute(
            "
            UPDATE job_queue
            SET status = 'succeeded',
                last_error = NULL,
                locked_at = NULL,
                updated_at = strftime('%Y-%m-%dT%H:%M:%fZ', 'now')
            WHERE id = ?1
            ",
            [id],
        )?;
        Ok(())
    }

    /// Record a failed run. If the job has retries left it goes back to `pending`
    /// with `available_at` pushed out by `backoff_seconds` (backpressure);
    /// otherwise it becomes terminally `failed`. Returns `true` if the job will
    /// be retried.
    pub fn mark_failed(&self, id: &str, error: &str, backoff_seconds: i64) -> StorageResult<bool> {
        let connection = self.db.checkout()?;
        let attempts_and_max: Option<(i64, i64)> = connection
            .query_row(
                "SELECT attempts, max_attempts FROM job_queue WHERE id = ?1",
                [id],
                |row| Ok((row.get(0)?, row.get(1)?)),
            )
            .optional()?;

        let Some((attempts, max_attempts)) = attempts_and_max else {
            return Ok(false);
        };

        if attempts < max_attempts {
            connection.execute(
                "
                UPDATE job_queue
                SET status = 'pending',
                    available_at = strftime('%Y-%m-%dT%H:%M:%fZ', 'now', ?2),
                    last_error = ?3,
                    locked_at = NULL,
                    updated_at = strftime('%Y-%m-%dT%H:%M:%fZ', 'now')
                WHERE id = ?1
                ",
                params![id, format!("+{} seconds", backoff_seconds.max(0)), error],
            )?;
            Ok(true)
        } else {
            connection.execute(
                "
                UPDATE job_queue
                SET status = 'failed',
                    last_error = ?2,
                    locked_at = NULL,
                    updated_at = strftime('%Y-%m-%dT%H:%M:%fZ', 'now')
                WHERE id = ?1
                ",
                params![id, error],
            )?;
            Ok(false)
        }
    }

    /// Requeue every `running` row back to `pending`. Called on startup: a row
    /// left `running` is the residue of a crash (no worker is alive yet), so it
    /// must be resumed rather than stranded. Returns how many were reclaimed.
    pub fn reclaim_stale_running(&self) -> StorageResult<usize> {
        let connection = self.db.checkout()?;
        let reclaimed = connection.execute(
            "
            UPDATE job_queue
            SET status = 'pending',
                available_at = strftime('%Y-%m-%dT%H:%M:%fZ', 'now'),
                locked_at = NULL,
                updated_at = strftime('%Y-%m-%dT%H:%M:%fZ', 'now')
            WHERE status = 'running'
            ",
            [],
        )?;
        Ok(reclaimed)
    }

    /// Current status tallies.
    pub fn counts(&self) -> StorageResult<JobQueueCounts> {
        let connection = self.db.checkout()?;
        let mut statement =
            connection.prepare("SELECT status, COUNT(*) FROM job_queue GROUP BY status")?;
        let mut counts = JobQueueCounts::default();
        let rows = statement.query_map([], |row| {
            Ok((row.get::<_, String>(0)?, row.get::<_, i64>(1)?))
        })?;
        for row in rows {
            let (status, count) = row?;
            match status.as_str() {
                "pending" => counts.pending = count,
                "running" => counts.running = count,
                "succeeded" => counts.succeeded = count,
                "failed" => counts.failed = count,
                _ => {}
            }
        }
        Ok(counts)
    }
}
