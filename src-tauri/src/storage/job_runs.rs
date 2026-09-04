//! `job_runs` occurrence-history storage (ADR 0109 dec. 2, migration `0153`).
//!
//! One row per attempt of background work — queue jobs and the direct-activity
//! registry's awaited work alike. Identity is never reused; the only legal
//! update is `running` → one terminal status. Every function here takes an
//! already-checked-out `&Connection` so the two writers (the queue's single
//! dispatch seam, `jobs::queue`; the direct-activity registry,
//! `storage::activity_registry`) can compose `begin_attempt`/`settle`/`prune`
//! into ONE `BEGIN IMMEDIATE` transaction alongside their own domain write —
//! never a separate checkout, never a window where the two writes disagree.
//! [`JobRunsStore`] is a thin per-call-checkout facade for standalone reads
//! (the `company_view_reads.rs` / `JobQueueStore` pattern).

use super::database::Database;
use super::*;
use crate::jobs::activity_identity::{ActivityFamily, ActivityTarget};

/// A fresh attempt to record via [`begin_attempt`].
pub struct NewJobRun {
    pub activity_key: String,
    pub run_key: String,
    pub kind: String,
    pub family: ActivityFamily,
    pub company_id: Option<String>,
    pub subject: String,
    pub target: ActivityTarget,
    pub attempt: i64,
}

/// The terminal (or retry) outcome [`settle`] records.
pub enum JobRunOutcome<'a> {
    Succeeded,
    Failed { error: &'a str },
    RetryScheduled { error: &'a str },
    Interrupted,
}

impl JobRunOutcome<'_> {
    fn status(&self) -> &'static str {
        match self {
            Self::Succeeded => "succeeded",
            Self::Failed { .. } => "failed",
            Self::RetryScheduled { .. } => "retry_scheduled",
            Self::Interrupted => "interrupted",
        }
    }

    fn error(&self) -> Option<&str> {
        match self {
            Self::Failed { error } | Self::RetryScheduled { error } => Some(error),
            Self::Succeeded | Self::Interrupted => None,
        }
    }
}

/// Family token as stored in `job_runs.family` — the same camelCase string
/// the DTO serializes (so a raw SQL scan and the wire format agree).
pub(crate) fn family_token(family: ActivityFamily) -> &'static str {
    match family {
        ActivityFamily::SourceRefresh => "sourceRefresh",
        ActivityFamily::CompanyRefresh => "companyRefresh",
        ActivityFamily::RegistryRefresh => "registryRefresh",
        ActivityFamily::FxPull => "fxPull",
        ActivityFamily::FundamentalsPull => "fundamentalsPull",
        ActivityFamily::Briefing => "briefing",
        ActivityFamily::HistoryFetch => "historyFetch",
        ActivityFamily::ReportSweep => "reportSweep",
        ActivityFamily::Reextraction => "reextraction",
        ActivityFamily::ReportReading => "reportReading",
        ActivityFamily::OwnershipReading => "ownershipReading",
        ActivityFamily::ManagementReading => "managementReading",
        ActivityFamily::PriceHistory => "priceHistory",
        ActivityFamily::KpiIngest => "kpiIngest",
        ActivityFamily::Transcript => "transcript",
        ActivityFamily::Corrupted => "corrupted",
    }
}

/// Begin one attempt: inserts a `running` row with `started_at = now` and
/// returns its id (the settle handle — never the reusable queue/run key).
pub(crate) fn begin_attempt(connection: &Connection, new_run: NewJobRun) -> StorageResult<i64> {
    let target_json = serde_json::to_string(&new_run.target)?;
    connection.execute(
        "
        INSERT INTO job_runs
            (activity_key, run_key, kind, family, company_id, subject, target_json,
             status, attempt, started_at)
        VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, 'running', ?8, strftime('%Y-%m-%dT%H:%M:%fZ', 'now'))
        ",
        params![
            new_run.activity_key,
            new_run.run_key,
            new_run.kind,
            family_token(new_run.family),
            new_run.company_id,
            new_run.subject,
            target_json,
            new_run.attempt.max(1),
        ],
    )?;
    Ok(connection.last_insert_rowid())
}

/// Settle one occurrence: the ONE legal `running` → terminal update, stamping
/// `finished_at` + `error`. A no-op (0 rows) if `id` is unknown or already
/// terminal — settling is idempotent, never a panic on a double-call.
pub(crate) fn settle(
    connection: &Connection,
    id: i64,
    outcome: &JobRunOutcome<'_>,
) -> StorageResult<()> {
    connection.execute(
        "
        UPDATE job_runs
        SET status = ?2,
            error = ?3,
            finished_at = strftime('%Y-%m-%dT%H:%M:%fZ', 'now')
        WHERE id = ?1 AND status = 'running'
        ",
        params![id, outcome.status(), outcome.error()],
    )?;
    Ok(())
}

/// Retention GC (ADR 0109 dec. 2): keep the newest 500 FINISHED rows by
/// `(finished_at DESC, id DESC)`, drop anything finished more than 30 days
/// ago. Never touches a `running` row. Run in the same transaction as
/// [`settle`] (queue settle, registry guard settle) and after startup
/// reconciliation — never on insert.
pub(crate) fn prune(connection: &Connection) -> StorageResult<()> {
    connection.execute(
        "
        DELETE FROM job_runs
        WHERE finished_at IS NOT NULL
            AND finished_at < strftime('%Y-%m-%dT%H:%M:%fZ', 'now', '-30 days')
        ",
        [],
    )?;
    connection.execute(
        "
        DELETE FROM job_runs
        WHERE finished_at IS NOT NULL
            AND id NOT IN (
                SELECT id FROM job_runs
                WHERE finished_at IS NOT NULL
                ORDER BY finished_at DESC, id DESC
                LIMIT 500
            )
        ",
        [],
    )?;
    Ok(())
}

/// Reverse of [`family_token`]: parse a stored `job_runs.family` string back
/// into its [`ActivityFamily`]. `None` for an unrecognized token (schema
/// drift — the read model then skips the row rather than guessing).
pub(crate) fn parse_family_token(token: &str) -> Option<ActivityFamily> {
    Some(match token {
        "sourceRefresh" => ActivityFamily::SourceRefresh,
        "companyRefresh" => ActivityFamily::CompanyRefresh,
        "registryRefresh" => ActivityFamily::RegistryRefresh,
        "fxPull" => ActivityFamily::FxPull,
        "fundamentalsPull" => ActivityFamily::FundamentalsPull,
        "briefing" => ActivityFamily::Briefing,
        "historyFetch" => ActivityFamily::HistoryFetch,
        "reportSweep" => ActivityFamily::ReportSweep,
        "reextraction" => ActivityFamily::Reextraction,
        "reportReading" => ActivityFamily::ReportReading,
        "ownershipReading" => ActivityFamily::OwnershipReading,
        "managementReading" => ActivityFamily::ManagementReading,
        "priceHistory" => ActivityFamily::PriceHistory,
        "kpiIngest" => ActivityFamily::KpiIngest,
        "transcript" => ActivityFamily::Transcript,
        "corrupted" => ActivityFamily::Corrupted,
        _ => return None,
    })
}

/// Every occurrence still `running` — used by startup reconciliation to
/// terminalize open occurrences as `interrupted` (ADR 0109 dec. 4).
pub(crate) fn running_ids(connection: &Connection) -> StorageResult<Vec<i64>> {
    let mut statement = connection.prepare("SELECT id FROM job_runs WHERE status = 'running'")?;
    let rows = statement.query_map([], |row| row.get::<_, i64>(0))?;
    let mut ids = Vec::new();
    for row in rows {
        ids.push(row?);
    }
    Ok(ids)
}

/// Per-call-checkout facade (the `JobQueueStore` pattern) for callers that
/// hold no connection of their own. Reach it via `AppState::job_runs()`.
#[derive(Clone)]
pub struct JobRunsStore {
    db: Database,
}

impl JobRunsStore {
    pub(super) fn new(db: Database) -> Self {
        Self { db }
    }

    pub fn begin_attempt(&self, new_run: NewJobRun) -> StorageResult<i64> {
        let connection = self.db.checkout()?;
        begin_attempt(&connection, new_run)
    }

    pub fn settle(&self, id: i64, outcome: JobRunOutcome<'_>) -> StorageResult<()> {
        let connection = self.db.checkout()?;
        settle(&connection, id, &outcome)?;
        prune(&connection)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::storage::open_in_memory_database;

    fn new_run(activity_key: &str, run_key: &str) -> NewJobRun {
        NewJobRun {
            activity_key: activity_key.to_owned(),
            run_key: run_key.to_owned(),
            kind: "scheduled_source_refresh".to_owned(),
            family: ActivityFamily::SourceRefresh,
            company_id: None,
            subject: "GPW ESPI/EBI".to_owned(),
            target: ActivityTarget::Sources,
            attempt: 1,
        }
    }

    #[test]
    fn begin_attempt_writes_a_running_row_settle_terminalizes_it() {
        let connection = open_in_memory_database().expect("db");
        let id = begin_attempt(&connection, new_run("source-refresh:x", "job-1")).expect("begin");

        let status: String = connection
            .query_row("SELECT status FROM job_runs WHERE id = ?1", [id], |row| {
                row.get(0)
            })
            .expect("status");
        assert_eq!(status, "running");

        settle(&connection, id, &JobRunOutcome::Succeeded).expect("settle");
        let (status, finished_at): (String, Option<String>) = connection
            .query_row(
                "SELECT status, finished_at FROM job_runs WHERE id = ?1",
                [id],
                |row| Ok((row.get(0)?, row.get(1)?)),
            )
            .expect("row");
        assert_eq!(status, "succeeded");
        assert!(finished_at.is_some());
    }

    #[test]
    fn settle_is_a_noop_once_already_terminal() {
        let connection = open_in_memory_database().expect("db");
        let id = begin_attempt(&connection, new_run("source-refresh:x", "job-1")).expect("begin");
        settle(&connection, id, &JobRunOutcome::Succeeded).expect("settle once");
        // A second settle call (e.g. a defensive double-call) must not panic or
        // overwrite the terminal status.
        settle(&connection, id, &JobRunOutcome::Failed { error: "late" })
            .expect("settle twice is a noop");
        let status: String = connection
            .query_row("SELECT status FROM job_runs WHERE id = ?1", [id], |row| {
                row.get(0)
            })
            .expect("status");
        assert_eq!(status, "succeeded", "the first terminal write wins");
    }

    #[test]
    fn prune_keeps_newest_500_finished_and_drops_stale_ones() {
        let connection = open_in_memory_database().expect("db");
        // A row finished 40 days ago must be dropped by the age rule.
        connection
            .execute(
                "INSERT INTO job_runs
                    (activity_key, run_key, kind, family, subject, target_json, status,
                     attempt, started_at, finished_at)
                 VALUES ('old', 'old', 'k', 'sourceRefresh', 's', '{}', 'succeeded', 1,
                     strftime('%Y-%m-%dT%H:%M:%fZ', 'now', '-40 days'),
                     strftime('%Y-%m-%dT%H:%M:%fZ', 'now', '-40 days'))",
                [],
            )
            .expect("seed old row");

        // 501 fresh finished rows: only the newest 500 survive the count rule.
        for i in 0..501 {
            let id = begin_attempt(&connection, new_run(&format!("k:{i}"), &format!("job:{i}")))
                .expect("begin");
            settle(&connection, id, &JobRunOutcome::Succeeded).expect("settle");
        }

        prune(&connection).expect("prune");

        let total: i64 = connection
            .query_row("SELECT COUNT(*) FROM job_runs", [], |row| row.get(0))
            .expect("count");
        assert_eq!(total, 500, "newest 500 finished rows survive");

        let old_survives: i64 = connection
            .query_row(
                "SELECT COUNT(*) FROM job_runs WHERE activity_key = 'old'",
                [],
                |row| row.get(0),
            )
            .expect("count old");
        assert_eq!(old_survives, 0, "a 40-day-old finished row is pruned");
    }

    #[test]
    fn retention_runs_at_settlement_not_on_insert() {
        // ADR 0109 dec. 2: retention GC runs at settlement (and after startup
        // reconciliation), NEVER on `begin_attempt` (insert) — so the queue's
        // dispatch seam can never trip a retention scan mid-attempt. Seed 500
        // already-finished rows, then `begin_attempt` (insert only, no settle)
        // for a 501st: the total must grow to 501 (no prune ran); settling
        // that one row's occurrence must then bring it back down to 500.
        let connection = open_in_memory_database().expect("db");
        for i in 0..500 {
            let id = begin_attempt(&connection, new_run(&format!("k:{i}"), &format!("job:{i}")))
                .expect("begin");
            settle(&connection, id, &JobRunOutcome::Succeeded).expect("settle");
        }
        let total_before: i64 = connection
            .query_row("SELECT COUNT(*) FROM job_runs", [], |row| row.get(0))
            .expect("count");
        assert_eq!(total_before, 500);

        let extra_id =
            begin_attempt(&connection, new_run("k:extra", "job:extra")).expect("begin, no settle");
        let total_after_insert: i64 = connection
            .query_row("SELECT COUNT(*) FROM job_runs", [], |row| row.get(0))
            .expect("count");
        assert_eq!(
            total_after_insert, 501,
            "begin_attempt (insert) must never run retention itself"
        );

        settle(&connection, extra_id, &JobRunOutcome::Succeeded).expect("settle");
        prune(&connection).expect("prune (as settle's caller composes it)");
        let total_after_settle: i64 = connection
            .query_row("SELECT COUNT(*) FROM job_runs", [], |row| row.get(0))
            .expect("count");
        assert_eq!(
            total_after_settle, 500,
            "settlement's retention pass brings the count back to the newest 500"
        );
    }

    #[test]
    fn prune_never_touches_a_running_row() {
        let connection = open_in_memory_database().expect("db");
        let running_id =
            begin_attempt(&connection, new_run("running-key", "job-running")).expect("begin");
        for i in 0..501 {
            let id = begin_attempt(&connection, new_run(&format!("k:{i}"), &format!("job:{i}")))
                .expect("begin");
            settle(&connection, id, &JobRunOutcome::Succeeded).expect("settle");
        }
        prune(&connection).expect("prune");
        let status: String = connection
            .query_row(
                "SELECT status FROM job_runs WHERE id = ?1",
                [running_id],
                |row| row.get(0),
            )
            .expect("status");
        assert_eq!(status, "running");
    }
}
