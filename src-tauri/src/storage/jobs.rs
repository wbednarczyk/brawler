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

/// A single queue row's lifecycle fields, for a narrow "how is this job doing?"
/// read (e.g. the qualitative-assessment panel poll). `status` is the raw
/// `job_queue.status` string (`pending`/`running`/`succeeded`/`failed`);
/// `last_error` is the most recent failure message the queue recorded, `None`
/// once the row has succeeded or has never failed.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct JobStatusRow {
    pub status: String,
    pub attempts: i64,
    pub max_attempts: i64,
    pub last_error: Option<String>,
}

/// Suffix marking a **follow-up** row: one that must never run while its base
/// sibling (the same id without this suffix) runs, and vice-versa. Used by the
/// qualitative-assessment re-arm (`commands::quality_frameworks`), where a re-run
/// arriving mid-run parks in `<main-id>{FOLLOWUP_SUFFIX}`. The ai worker lane has
/// more than one worker ([`crate::jobs::pool_layout`], ADR 0059), so two rows of
/// the same kind — a running job and its parked re-arm — could otherwise be
/// claimed at once and double-run a paid AI request. The claim guard below keeps
/// the pair serialized in both directions.
pub const FOLLOWUP_SUFFIX: &str = ":followup";

/// SQL predicate over an aliased `candidate` row: true unless the row's sibling is
/// currently `running`. Pairs `<id>` with `<id>{FOLLOWUP_SUFFIX}` in both
/// directions (a base row is held while its follow-up runs, and the follow-up is
/// held while its base runs), so neither is ever claimed while the other runs.
/// Constant SQL derived from [`FOLLOWUP_SUFFIX`] — no user input is interpolated.
fn sibling_not_running_guard() -> String {
    let suffix_len = FOLLOWUP_SUFFIX.len();
    format!(
        "NOT EXISTS (\
             SELECT 1 FROM job_queue sibling \
             WHERE sibling.status = 'running' AND (\
                 sibling.id = candidate.id || '{FOLLOWUP_SUFFIX}' \
                 OR (candidate.id LIKE '%{FOLLOWUP_SUFFIX}' \
                     AND sibling.id = substr(candidate.id, 1, length(candidate.id) - {suffix_len}))\
             )\
         )"
    )
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
    /// The reset row follows the caller's `kind`/`payload`/`max_attempts` — the id
    /// IS the job's identity, so a row whose stored kind drifted (raw tamper)
    /// heals on the next re-arm instead of sitting invisible to every lane.
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
                kind = excluded.kind,
                payload = excluded.payload,
                max_attempts = excluded.max_attempts,
                updated_at = strftime('%Y-%m-%dT%H:%M:%fZ', 'now')
            WHERE job_queue.status != 'running'
            ",
            params![id, kind, payload, max_attempts.max(1)],
        )?;
        Ok(changed > 0)
    }

    /// Read the opaque payload of a job that is still `pending` (not yet claimed),
    /// or `None` if the id is absent, already `running`, or terminal. Callers use
    /// this to merge a superseding enqueue into a not-yet-started row (e.g. union a
    /// single-criterion re-run into a pending framework-wide assessment) rather
    /// than racing a second, duplicate job.
    pub fn pending_payload(&self, id: &str) -> StorageResult<Option<String>> {
        let connection = self.db.checkout()?;
        connection
            .query_row(
                "SELECT payload FROM job_queue WHERE id = ?1 AND status = 'pending'",
                params![id],
                |row| row.get(0),
            )
            .optional()
            .map_err(StorageError::from)
    }

    /// Atomically claim the next runnable job: the oldest `pending` row whose
    /// `available_at` has passed and whose sibling (see [`FOLLOWUP_SUFFIX`]) is not
    /// currently `running`. The claim and the `attempts` increment happen in one
    /// statement, so two workers can never claim the same row and a crash mid-run
    /// still counts as an attempt. Returns `None` when nothing is runnable.
    pub fn claim_next(&self) -> StorageResult<Option<ClaimedJob>> {
        let connection = self.db.checkout()?;
        let guard = sibling_not_running_guard();
        let sql = format!(
            "
                UPDATE job_queue
                SET status = 'running',
                    attempts = attempts + 1,
                    locked_at = strftime('%Y-%m-%dT%H:%M:%fZ', 'now'),
                    updated_at = strftime('%Y-%m-%dT%H:%M:%fZ', 'now')
                WHERE id = (
                    SELECT candidate.id
                    FROM job_queue candidate
                    WHERE candidate.status = 'pending'
                        AND candidate.available_at <= strftime('%Y-%m-%dT%H:%M:%fZ', 'now')
                        AND {guard}
                    ORDER BY candidate.available_at, candidate.created_at
                    LIMIT 1
                )
                RETURNING id, kind, payload, attempts, max_attempts
                "
        );
        connection
            .query_row(&sql, [], |row| {
                Ok(ClaimedJob {
                    id: row.get(0)?,
                    kind: row.get(1)?,
                    payload: row.get(2)?,
                    attempts: row.get(3)?,
                    max_attempts: row.get(4)?,
                })
            })
            .optional()
            .map_err(StorageError::from)
    }

    /// Like [`claim_next`], but only claims rows whose `kind` is in `kinds`. This
    /// is the split point for isolated worker pools (ADR 0059): each pool's threads
    /// claim only their lane's kinds, so a slow refresh cannot starve autopilot.
    /// Returns `None` when nothing in those kinds is runnable (or `kinds` is empty).
    pub fn claim_next_for_kinds(&self, kinds: &[&str]) -> StorageResult<Option<ClaimedJob>> {
        if kinds.is_empty() {
            return Ok(None);
        }
        let connection = self.db.checkout()?;
        let placeholders = vec!["?"; kinds.len()].join(", ");
        let guard = sibling_not_running_guard();
        let sql = format!(
            "
            UPDATE job_queue
            SET status = 'running',
                attempts = attempts + 1,
                locked_at = strftime('%Y-%m-%dT%H:%M:%fZ', 'now'),
                updated_at = strftime('%Y-%m-%dT%H:%M:%fZ', 'now')
            WHERE id = (
                SELECT candidate.id
                FROM job_queue candidate
                WHERE candidate.status = 'pending'
                    AND candidate.available_at <= strftime('%Y-%m-%dT%H:%M:%fZ', 'now')
                    AND candidate.kind IN ({placeholders})
                    AND {guard}
                ORDER BY candidate.available_at, candidate.created_at
                LIMIT 1
            )
            RETURNING id, kind, payload, attempts, max_attempts
            "
        );
        connection
            .query_row(
                &sql,
                rusqlite::params_from_iter(kinds.iter().copied()),
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

    /// Defer a claimed job back to `pending` after `backoff_seconds` **without
    /// counting it as an attempt** (ADR 0059). Unlike [`mark_failed`], this is not a
    /// failure: it is a "resource busy, try later" requeue used when a worker cannot
    /// acquire the per-source serialization lock. It undoes the claim's attempt
    /// increment so contention never exhausts a job's retry budget.
    pub fn defer(&self, id: &str, backoff_seconds: i64) -> StorageResult<()> {
        let connection = self.db.checkout()?;
        connection.execute(
            "
            UPDATE job_queue
            SET status = 'pending',
                attempts = MAX(attempts - 1, 0),
                available_at = strftime('%Y-%m-%dT%H:%M:%fZ', 'now', ?2),
                locked_at = NULL,
                updated_at = strftime('%Y-%m-%dT%H:%M:%fZ', 'now')
            WHERE id = ?1
            ",
            params![id, format!("+{} seconds", backoff_seconds.max(0))],
        )?;
        Ok(())
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

    /// Settle a claimed job succeeded AND its exact `job_runs` occurrence in ONE
    /// `BEGIN IMMEDIATE` transaction (ADR 0109 dec. 2), then run retention.
    /// `run_id` is `None` when [`begin_attempt`](super::job_runs::begin_attempt)
    /// itself failed or the kind has no resolved identity — the queue row still
    /// settles normally, just with no occurrence to close.
    pub fn mark_succeeded_with_run(&self, id: &str, run_id: Option<i64>) -> StorageResult<()> {
        let mut connection = self.db.checkout()?;
        let tx = connection.transaction_with_behavior(rusqlite::TransactionBehavior::Immediate)?;
        tx.execute(
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
        if let Some(run_id) = run_id {
            super::job_runs::settle(&tx, run_id, &super::job_runs::JobRunOutcome::Succeeded)?;
            super::job_runs::prune(&tx)?;
        }
        tx.commit()?;
        Ok(())
    }

    /// Settle a claimed job's failure AND its exact `job_runs` occurrence in ONE
    /// `BEGIN IMMEDIATE` transaction (ADR 0109 dec. 2): `Failed` when the queue
    /// row goes terminally `failed`, `RetryScheduled` when retries remain. Same
    /// return contract as [`mark_failed`](Self::mark_failed) (`true` = will retry).
    pub fn mark_failed_with_run(
        &self,
        id: &str,
        error: &str,
        backoff_seconds: i64,
        run_id: Option<i64>,
    ) -> StorageResult<bool> {
        let mut connection = self.db.checkout()?;
        let tx = connection.transaction_with_behavior(rusqlite::TransactionBehavior::Immediate)?;

        let attempts_and_max: Option<(i64, i64)> = tx
            .query_row(
                "SELECT attempts, max_attempts FROM job_queue WHERE id = ?1",
                [id],
                |row| Ok((row.get(0)?, row.get(1)?)),
            )
            .optional()?;
        let Some((attempts, max_attempts)) = attempts_and_max else {
            tx.commit()?;
            return Ok(false);
        };

        let will_retry = attempts < max_attempts;
        if will_retry {
            tx.execute(
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
        } else {
            tx.execute(
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
        }

        if let Some(run_id) = run_id {
            let outcome = if will_retry {
                super::job_runs::JobRunOutcome::RetryScheduled { error }
            } else {
                super::job_runs::JobRunOutcome::Failed { error }
            };
            super::job_runs::settle(&tx, run_id, &outcome)?;
            super::job_runs::prune(&tx)?;
        }

        tx.commit()?;
        Ok(will_retry)
    }

    /// Requeue crash-residue `running` rows on startup (no worker is alive yet).
    /// A row that already **exhausted its attempts** is dead-lettered instead of
    /// resurrected: it is a job that keeps getting reclaimed and re-run without
    /// ever reaching [`mark_failed`] (e.g. it hangs), so resuming it forever
    /// starves the queue. This is the poison-job guard from ADR 0059 (the bankier
    /// refresh with `attempts=15 > max_attempts=2` monopolized the worker across
    /// every restart). Rows with attempts left are resumed as before. Returns how
    /// many were resumed (dead-lettered rows are not counted).
    pub fn reclaim_stale_running(&self) -> StorageResult<usize> {
        let connection = self.db.checkout()?;
        connection.execute(
            "
            UPDATE job_queue
            SET status = 'failed',
                last_error = COALESCE(last_error, 'dead-lettered on reclaim: attempts exhausted'),
                locked_at = NULL,
                updated_at = strftime('%Y-%m-%dT%H:%M:%fZ', 'now')
            WHERE status = 'running' AND attempts >= max_attempts
            ",
            [],
        )?;
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

    /// Read one job row's lifecycle fields by id, or `None` if the id is absent.
    /// A narrow, id-scoped status read (unlike [`counts`](Self::counts), which
    /// aggregates the whole queue): the qualitative-assessment panel polls the
    /// primary `qualitative_assessment:<company>:<framework>` row this way to
    /// surface a terminal failure (`status = 'failed'`, `last_error`) instead of
    /// silently clearing its "queued" hint.
    pub fn status(&self, id: &str) -> StorageResult<Option<JobStatusRow>> {
        let connection = self.db.checkout()?;
        connection
            .query_row(
                "SELECT status, attempts, max_attempts, last_error FROM job_queue WHERE id = ?1",
                params![id],
                |row| {
                    Ok(JobStatusRow {
                        status: row.get(0)?,
                        attempts: row.get(1)?,
                        max_attempts: row.get(2)?,
                        last_error: row.get(3)?,
                    })
                },
            )
            .optional()
            .map_err(StorageError::from)
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
