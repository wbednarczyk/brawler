//! Borrowed-connection reads for the Activity read model (ADR 0109 dec. 3/D3,
//! `commands::activity`). One function per composed-view section, each taking
//! an already checked-out `&Connection` so `compute_activity` can check out
//! ONE connection for the whole call (the `company_view_reads.rs` pattern).

use rusqlite::Connection;

use super::*;
use crate::jobs::activity_identity::{ActivityFamily, ActivityTarget};

/// One `job_runs` row, decoded. `None` from the row mapper (never a panic)
/// when `family`/`target_json` fail to parse — schema drift, not a caller bug;
/// the read model skips such a row rather than guessing.
#[derive(Debug, Clone)]
pub(crate) struct OccurrenceRow {
    pub id: i64,
    pub activity_key: String,
    pub family: ActivityFamily,
    pub company_id: Option<String>,
    pub subject: String,
    pub target: ActivityTarget,
    pub status: String,
    pub attempt: i64,
    pub started_at: String,
    pub finished_at: Option<String>,
    pub error: Option<String>,
}

/// The raw columns of one `job_runs` row, before `family`/`target_json` are
/// decoded — grouped so the row mapper stays under clippy's argument limit.
struct RawOccurrenceRow {
    id: i64,
    activity_key: String,
    family_token: String,
    company_id: Option<String>,
    subject: String,
    target_json: String,
    status: String,
    attempt: i64,
    started_at: String,
    finished_at: Option<String>,
    error: Option<String>,
}

/// `None` (never a panic) when `family`/`target_json` fail to parse — schema
/// drift, not a caller bug; the read model skips such a row rather than
/// guessing.
fn decode_occurrence(raw: RawOccurrenceRow) -> Option<OccurrenceRow> {
    let family = job_runs::parse_family_token(&raw.family_token)?;
    let target: ActivityTarget = serde_json::from_str(&raw.target_json).ok()?;
    Some(OccurrenceRow {
        id: raw.id,
        activity_key: raw.activity_key,
        family,
        company_id: raw.company_id,
        subject: raw.subject,
        target,
        status: raw.status,
        attempt: raw.attempt,
        started_at: raw.started_at,
        finished_at: raw.finished_at,
        error: raw.error,
    })
}

const OCCURRENCE_COLUMNS: &str = "id, activity_key, family, company_id, subject, target_json, \
     status, attempt, started_at, finished_at, error";

fn map_occurrence_row(row: &rusqlite::Row<'_>) -> rusqlite::Result<Option<OccurrenceRow>> {
    Ok(decode_occurrence(RawOccurrenceRow {
        id: row.get(0)?,
        activity_key: row.get(1)?,
        family_token: row.get(2)?,
        company_id: row.get(3)?,
        subject: row.get(4)?,
        target_json: row.get(5)?,
        status: row.get(6)?,
        attempt: row.get(7)?,
        started_at: row.get(8)?,
        finished_at: row.get(9)?,
        error: row.get(10)?,
    }))
}

/// `job_runs` rows currently `id IN (ids)` — the caller composes `ids` from
/// the queue-running-with-occurrence set ∪ the live direct-activity registry
/// (D3's `active` definition; done by the caller since the registry itself is
/// an `AppState` concern, not a connection-scoped read).
pub(crate) fn occurrences_by_id(
    connection: &Connection,
    ids: &[i64],
) -> StorageResult<Vec<OccurrenceRow>> {
    if ids.is_empty() {
        return Ok(Vec::new());
    }
    let placeholders = vec!["?"; ids.len()].join(", ");
    let sql = format!("SELECT {OCCURRENCE_COLUMNS} FROM job_runs WHERE id IN ({placeholders})");
    let mut statement = connection.prepare(&sql)?;
    let rows = statement.query_map(rusqlite::params_from_iter(ids.iter()), map_occurrence_row)?;
    let mut out = Vec::new();
    for row in rows {
        if let Some(row) = row? {
            out.push(row);
        }
    }
    Ok(out)
}

/// `job_runs.id` values whose `run_key` is a live `job_queue` row literally
/// `running` — the queue half of `active` (D3): a `running` queue row WITHOUT
/// a matching open occurrence is excluded (the panic-containment invariant
/// means that should not happen in a live app; it degrades to invisible here
/// rather than a fabricated `active` entry).
pub(crate) fn queue_running_occurrence_ids(connection: &Connection) -> StorageResult<Vec<i64>> {
    let mut statement = connection.prepare(
        "SELECT job_runs.id
         FROM job_runs
         JOIN job_queue ON job_queue.id = job_runs.run_key
         WHERE job_queue.status = 'running' AND job_runs.status = 'running'",
    )?;
    let rows = statement.query_map([], |row| row.get::<_, i64>(0))?;
    let mut out = Vec::new();
    for row in rows {
        out.push(row?);
    }
    Ok(out)
}

/// `job_queue` rows `pending` (incl. retry backoff), oldest first — the raw
/// material for `queued` (D3); identity resolution (family/subject/target)
/// happens in `commands::activity` since it needs `AppState`, not just a
/// connection.
pub(crate) struct PendingJobRow {
    pub id: String,
    pub kind: String,
    pub payload: String,
    pub last_error: Option<String>,
    pub created_at: String,
}

pub(crate) fn pending_jobs(connection: &Connection) -> StorageResult<Vec<PendingJobRow>> {
    let mut statement = connection.prepare(
        "SELECT id, kind, payload, last_error, created_at
         FROM job_queue
         WHERE status = 'pending'
         ORDER BY available_at, created_at",
    )?;
    let rows = statement.query_map([], |row| {
        Ok(PendingJobRow {
            id: row.get(0)?,
            kind: row.get(1)?,
            payload: row.get(2)?,
            last_error: row.get(3)?,
            created_at: row.get(4)?,
        })
    })?;
    let mut out = Vec::new();
    for row in rows {
        out.push(row?);
    }
    Ok(out)
}

/// A `transcript_jobs` row still `queued` (not yet awaited/started — no
/// `job_runs`/registry entry exists for it yet, ADR 0109 dec. 3): the raw
/// material for `queued`'s transcript family.
pub(crate) struct QueuedTranscriptRow {
    pub id: String,
    pub company_id: Option<String>,
    pub source_label: Option<String>,
    pub source_url: String,
    pub created_at: String,
}

pub(crate) fn queued_transcript_jobs(
    connection: &Connection,
) -> StorageResult<Vec<QueuedTranscriptRow>> {
    let mut statement = connection.prepare(
        "SELECT id, company_id, source_label, source_url, created_at
         FROM transcript_jobs
         WHERE status = 'queued'
         ORDER BY created_at",
    )?;
    let rows = statement.query_map([], |row| {
        Ok(QueuedTranscriptRow {
            id: row.get(0)?,
            company_id: row.get(1)?,
            source_label: row.get(2)?,
            source_url: row.get(3)?,
            created_at: row.get(4)?,
        })
    })?;
    let mut out = Vec::new();
    for row in rows {
        out.push(row?);
    }
    Ok(out)
}

/// The latest occurrence per `activity_key` among TERMINAL `job_runs` rows
/// (`succeeded`/`failed`/`interrupted` — `retry_scheduled` is not terminal,
/// the job runs again) within `window_days` of `now`, newest first, capped at
/// `limit` AFTER the per-key collapse (D3/ADR 0109 dec. 2). `now` is caller-
/// supplied so tests can control the window boundary.
pub(crate) fn recent_occurrences(
    connection: &Connection,
    now: &str,
    window_days: i64,
    limit: i64,
) -> StorageResult<Vec<OccurrenceRow>> {
    let sql = format!(
        "SELECT {OCCURRENCE_COLUMNS} FROM job_runs
         WHERE id IN (
             SELECT MAX(id) FROM job_runs
             WHERE status IN ('succeeded', 'failed', 'interrupted')
                 AND finished_at IS NOT NULL
                 AND finished_at >= datetime(?1, ?2)
             GROUP BY activity_key
         )
         ORDER BY finished_at DESC, id DESC
         LIMIT ?3"
    );
    let mut statement = connection.prepare(&sql)?;
    let rows = statement.query_map(
        params![now, format!("-{window_days} days"), limit],
        map_occurrence_row,
    )?;
    let mut out = Vec::new();
    for row in rows {
        if let Some(row) = row? {
            out.push(row);
        }
    }
    Ok(out)
}

/// A company's ticker, for a raw subject (never a silent blank — falls back
/// to the id if the company is somehow gone).
pub(crate) fn ticker(connection: &Connection, company_id: &str) -> String {
    connection
        .query_row(
            "SELECT ticker FROM companies WHERE id = ?1",
            [company_id],
            |row| row.get::<_, String>(0),
        )
        .unwrap_or_else(|_| company_id.to_owned())
}

/// A company's `qualified_ticker`, for the `qualifiedTicker` DTO field.
pub(crate) fn qualified_ticker(connection: &Connection, company_id: &str) -> Option<String> {
    connection
        .query_row(
            "SELECT qualified_ticker FROM companies WHERE id = ?1",
            [company_id],
            |row| row.get(0),
        )
        .ok()
}

/// One `kpi_ingest_runs` row's activity-relevant fields (ADR 0109 dec. 3: KPI
/// ingest never writes `job_runs` — its live lease IS the activity signal).
pub(crate) struct KpiRunRow {
    pub id: String,
    pub report_document_id: String,
    pub company_id: String,
    pub last_error: Option<String>,
    pub created_at: String,
}

/// Non-terminal `kpi_ingest_runs` rows split by live lease (D3/ADR 0109 dec. 4):
/// leased -> `active`; unleased (absent or expired lease) -> `queued`, never
/// active. `now` is caller-supplied so tests control the lease-expiry boundary.
pub(crate) fn kpi_runs_by_lease(
    connection: &Connection,
    now: &str,
) -> StorageResult<(Vec<KpiRunRow>, Vec<KpiRunRow>)> {
    let mut statement = connection.prepare(
        "SELECT id, report_document_id, company_id, last_error, created_at,
                (lease_holder IS NOT NULL AND lease_expires_at > ?1) AS leased
         FROM kpi_ingest_runs
         WHERE status NOT IN ('complete', 'partial', 'failed', 'cancelled')",
    )?;
    let rows = statement.query_map([now], |row| {
        Ok((
            KpiRunRow {
                id: row.get(0)?,
                report_document_id: row.get(1)?,
                company_id: row.get(2)?,
                last_error: row.get(3)?,
                created_at: row.get(4)?,
            },
            row.get::<_, bool>(5)?,
        ))
    })?;
    let mut leased = Vec::new();
    let mut unleased = Vec::new();
    for row in rows {
        let (run, is_leased) = row?;
        if is_leased {
            leased.push(run);
        } else {
            unleased.push(run);
        }
    }
    Ok((leased, unleased))
}

/// A report document's title, via the caller's own connection (never a second
/// checkout — the `AppState::get_report_document` accessor checks out its own).
pub(crate) fn document_title_bound(connection: &Connection, document_id: &str) -> String {
    connection
        .query_row(
            "SELECT title FROM report_documents WHERE id = ?1",
            [document_id],
            |row| row.get::<_, Option<String>>(0),
        )
        .ok()
        .flatten()
        .filter(|title| !title.trim().is_empty())
        .unwrap_or_else(|| document_id.to_owned())
}

/// A `job_queue` row literally `running` with NO matching open `job_runs`
/// occurrence — the panic-containment invariant (ADR 0109 dec. 2) means this
/// should not exist while the app is alive, but the read model surfaces it
/// honestly as `stalled` rather than fabricating `active` or hiding it (D3).
pub(crate) fn stalled_queue_rows(connection: &Connection) -> StorageResult<Vec<PendingJobRow>> {
    let mut statement = connection.prepare(
        "SELECT job_queue.id, job_queue.kind, job_queue.payload, job_queue.last_error, job_queue.created_at
         FROM job_queue
         LEFT JOIN job_runs ON job_runs.run_key = job_queue.id AND job_runs.status = 'running'
         WHERE job_queue.status = 'running' AND job_runs.id IS NULL",
    )?;
    let rows = statement.query_map([], |row| {
        Ok(PendingJobRow {
            id: row.get(0)?,
            kind: row.get(1)?,
            payload: row.get(2)?,
            last_error: row.get(3)?,
            created_at: row.get(4)?,
        })
    })?;
    let mut out = Vec::new();
    for row in rows {
        out.push(row?);
    }
    Ok(out)
}

/// A non-terminal domain row (id, company_id, error, updated_at) with no
/// backing job — `stalled` (D3/ADR 0109 dec. 4). Mirrors the STRANDED
/// detection `jobs::activity_reconcile` uses (a read here, never a mutation);
/// rare once startup reconciliation has run.
pub(crate) struct StalledDomainRow {
    pub id: String,
    pub company_id: String,
    pub error: Option<String>,
    pub updated_at: String,
}

/// Stranded `autopilot_run` rows: non-terminal with no live (pending/running)
/// stage job. Stage job ids are `autopilot:{run_id}:{stage}`. Sweep-child runs
/// (`sweep_id IS NOT NULL`) are excluded — they collapse into their sweep's own
/// `report-sweep:<id>` item (D1), surfaced instead by [`stalled_parent_rows`]
/// over `history_sweeps` if the sweep itself is stranded.
pub(crate) fn stalled_autopilot_runs(
    connection: &Connection,
) -> StorageResult<Vec<(StalledDomainRow, String)>> {
    let mut statement = connection.prepare(
        "SELECT id, company_id, last_error, updated_at, report_document_id
         FROM autopilot_run
         WHERE status IN ('pending', 'running')
             AND sweep_id IS NULL
             AND NOT EXISTS (
                 SELECT 1 FROM job_queue
                 WHERE kind = 'autopilot_stage'
                     AND status IN ('pending', 'running')
                     AND id LIKE 'autopilot:' || autopilot_run.id || ':%'
             )",
    )?;
    let rows = statement.query_map([], |row| {
        Ok((
            StalledDomainRow {
                id: row.get(0)?,
                company_id: row.get(1)?,
                error: row.get(2)?,
                updated_at: row.get(3)?,
            },
            row.get::<_, String>(4)?,
        ))
    })?;
    let mut out = Vec::new();
    for row in rows {
        out.push(row?);
    }
    Ok(out)
}

/// Stranded `history_sweeps` / `pipeline_reextraction_batches` rows: a
/// non-terminal row (`queued`/`running`) whose parent job (id == the row's own
/// id) is not live.
pub(crate) fn stalled_parent_rows(
    connection: &Connection,
    table: &str,
    job_kind: &str,
) -> StorageResult<Vec<StalledDomainRow>> {
    let sql = format!(
        "SELECT id, company_id, error, updated_at
         FROM {table}
         WHERE status IN ('queued', 'running')
             AND NOT EXISTS (
                 SELECT 1 FROM job_queue
                 WHERE job_queue.id = {table}.id
                     AND job_queue.kind = ?1
                     AND job_queue.status IN ('pending', 'running')
             )"
    );
    let mut statement = connection.prepare(&sql)?;
    let rows = statement.query_map([job_kind], |row| {
        Ok(StalledDomainRow {
            id: row.get(0)?,
            company_id: row.get(1)?,
            error: row.get(2)?,
            updated_at: row.get(3)?,
        })
    })?;
    let mut out = Vec::new();
    for row in rows {
        out.push(row?);
    }
    Ok(out)
}

/// Member-run progress for a sweep/batch parent (D1: `progress {done,total,failed}`
/// from its drain counters). `total` = the row's own `candidates_total`; `done`/
/// `failed` are counted from its `enqueued_run_ids_json` member runs' current
/// `autopilot_run.status` — read fresh so an in-progress parent's counters move
/// as members complete, not just at sweep-finish time.
pub(crate) fn parent_progress(
    connection: &Connection,
    table: &str,
    id: &str,
) -> StorageResult<Option<(i64, i64, i64)>> {
    let sql = format!("SELECT candidates_total, enqueued_run_ids_json FROM {table} WHERE id = ?1");
    let row: Option<(i64, Option<String>)> = connection
        .query_row(&sql, [id], |row| Ok((row.get(0)?, row.get(1)?)))
        .optional()?;
    let Some((total, run_ids_json)) = row else {
        return Ok(None);
    };
    let run_ids: Vec<String> = run_ids_json
        .and_then(|json| serde_json::from_str(&json).ok())
        .unwrap_or_default();
    if run_ids.is_empty() {
        return Ok(Some((0, total, 0)));
    }
    let placeholders = vec!["?"; run_ids.len()].join(", ");
    let status_sql = format!("SELECT status FROM autopilot_run WHERE id IN ({placeholders})");
    let mut statement = connection.prepare(&status_sql)?;
    let statuses = statement.query_map(rusqlite::params_from_iter(run_ids.iter()), |row| {
        row.get::<_, String>(0)
    })?;
    let (mut done, mut failed) = (0i64, 0i64);
    for status in statuses {
        match status?.as_str() {
            "succeeded" | "partial" => done += 1,
            "failed" => failed += 1,
            _ => {}
        }
    }
    Ok(Some((done, total, failed)))
}

/// Counts for `get_activity_summary` (two indexed counts + one max, no fan-out).
pub(crate) struct ActivitySummaryCounts {
    pub active: i64,
    pub queued: i64,
    pub last_finished_at: Option<String>,
}

pub(crate) fn summary_counts(
    connection: &Connection,
    active_ids: &[i64],
) -> StorageResult<ActivitySummaryCounts> {
    let queued: i64 = connection.query_row(
        "SELECT COUNT(*) FROM job_queue WHERE status = 'pending'",
        [],
        |row| row.get(0),
    )?;
    let last_finished_at: Option<String> = connection
        .query_row(
            "SELECT MAX(finished_at) FROM job_runs WHERE finished_at IS NOT NULL",
            [],
            |row| row.get(0),
        )
        .optional()?
        .flatten();
    Ok(ActivitySummaryCounts {
        active: active_ids.len() as i64,
        queued,
        last_finished_at,
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::storage::open_in_memory_database;

    /// ADR 0109 dec. 2/D3 volume gate: `recent_occurrences` reads a BOUNDED
    /// set at 100k finished rows — asserted via the query plan (it must use an
    /// index, never a full table scan), never wall-clock (ADR 0049).
    #[test]
    fn list_activity_reads_bounded_rows_at_100k() {
        let connection = open_in_memory_database().expect("db");
        let tx = rusqlite::Transaction::new_unchecked(
            &connection,
            rusqlite::TransactionBehavior::Immediate,
        )
        .expect("tx");
        for i in 0..100_000 {
            tx.execute(
                "INSERT INTO job_runs
                    (activity_key, run_key, kind, family, subject, target_json, status,
                     attempt, started_at, finished_at)
                 VALUES (?1, ?1, 'k', 'sourceRefresh', 's', '{\"kind\":\"sources\"}',
                     'succeeded', 1, strftime('%Y-%m-%dT%H:%M:%fZ', 'now'),
                     strftime('%Y-%m-%dT%H:%M:%fZ', 'now'))",
                [format!("k:{i}")],
            )
            .expect("seed row");
        }
        tx.commit().expect("commit");

        let sql = "SELECT id FROM job_runs
             WHERE status IN ('succeeded', 'failed', 'interrupted')
                 AND finished_at IS NOT NULL
                 AND finished_at >= datetime(?1, ?2)
             GROUP BY activity_key";
        let mut statement = connection
            .prepare(&format!("EXPLAIN QUERY PLAN {sql}"))
            .expect("prepare plan");
        let plan: Vec<String> = statement
            .query_map(params!["2026-09-03T00:00:00Z", "-7 days"], |row| {
                row.get::<_, String>(3)
            })
            .expect("plan rows")
            .map(|row| row.expect("plan row"))
            .collect();
        let plan_text = plan.join(" | ");
        assert!(
            plan_text.contains("USING INDEX") || plan_text.contains("USING COVERING INDEX"),
            "the candidate scan over job_runs must use an index, never an unindexed full scan: {plan_text}"
        );
        assert!(
            !plan_text.contains("SCAN job_runs USING TEMP B-TREE"),
            "grouping must not fall back to a full temp-b-tree materialization at this volume: {plan_text}"
        );

        let rows = recent_occurrences(&connection, "2026-09-03T00:00:00Z", 7, 40).expect("recent");
        assert!(
            rows.len() <= 40,
            "the cap is applied AFTER the collapse, never bypassed"
        );
    }
}
