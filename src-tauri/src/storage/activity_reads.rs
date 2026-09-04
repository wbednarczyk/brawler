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
    /// The real `job_queue.attempts` count (sol diff R1 #14) — a retry-
    /// pending row's attempt count, never a hardcoded 0.
    pub attempts: i64,
}

pub(crate) fn pending_jobs(connection: &Connection) -> StorageResult<Vec<PendingJobRow>> {
    let mut statement = connection.prepare(
        "SELECT id, kind, payload, last_error, created_at, attempts
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
            attempts: row.get(5)?,
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

/// The intended access path for `recent_candidates_sql()` (sol diff R1
/// #16) — a `finished_at DESC` range scan that early-terminates once past
/// the window, never `idx_job_runs_status`: `status IN (…)` alone is not
/// selective at scale (most rows are terminal), so leading with it would
/// visit close to the WHOLE table before the date filter ever applies.
const RECENT_INDEX: &str = "idx_job_runs_finished_at";

/// Generous cap on rows FETCHED before the per-`activity_key` collapse (sol
/// diff R1 #16) — comfortably above how many attempts of the SAME key could
/// realistically land inside one 7-day window, so a burst of retries for one
/// key can never crowd a different key's newest occurrence out of the
/// result before the collapse runs.
const RECENT_FETCH_CAP: i64 = 500;

/// The candidate-selection query `recent_occurrences` runs against
/// `job_runs` — factored out as its own constant (sol diff R1 #16) so the
/// 100k-row performance gate can `EXPLAIN QUERY PLAN` the EXACT production
/// query, never a hand-copied stand-in that can drift from what actually
/// runs.
///
/// Deliberately NOT a `GROUP BY activity_key` aggregate (sol diff R1 #16):
/// grouping forces SQLite into `USE TEMP B-TREE FOR GROUP BY` over
/// everything the `status` filter matches — which, since most terminal rows
/// share one of three statuses, is close to the WHOLE table, not the recent
/// window. A plain range scan ordered `finished_at DESC` with `LIMIT` uses
/// [`RECENT_INDEX`] directly and early-terminates; the "one newest row per
/// key, cap AFTER the collapse" rule (D3/ADR 0109 dec. 2) then runs in Rust
/// over this already-small, already-ordered fetch.
///
/// sol diff R1 #10: `finished_at` is stored `YYYY-MM-DDTHH:MM:SS.SSSZ`
/// (`strftime('%Y-%m-%dT%H:%M:%fZ', ...)`), but `datetime()` returns
/// `YYYY-MM-DD HH:MM:SS` (a space, no fractional seconds). A lexical `>=`
/// comparison of those two formats is wrong: 'T' (0x54) sorts after ' '
/// (0x20), so ANY stored row sharing the cutoff's calendar date passes
/// regardless of its actual time-of-day — up to nearly a day too old.
/// Fix: format the cutoff with `strftime` IDENTICALLY to how `finished_at`
/// itself is stored, so the comparison stays plain TEXT `>=` — correct AND
/// still able to use [`RECENT_INDEX`] (wrapping the COLUMN in `julianday()`
/// instead would have been correct too, but forces a full index/table scan
/// since the index is built on the raw text ordering).
fn recent_candidates_sql() -> String {
    // `INDEXED BY` forces `RECENT_INDEX` (sol diff R1 #16): without ANALYZE
    // statistics on a fresh/small database, SQLite's planner prefers
    // `idx_job_runs_status` (an IN-list over 3 literal values LOOKS cheap)
    // and then sorts/scans everything it matches — which at scale is close
    // to the WHOLE table, since most terminal rows share one of 3 statuses.
    // The date-bounded index is the one whose ORDER already matches this
    // query and can early-terminate; pin it rather than hope the planner's
    // heuristic agrees.
    format!(
        "SELECT {OCCURRENCE_COLUMNS}
         FROM job_runs INDEXED BY {RECENT_INDEX}
         WHERE status IN ('succeeded', 'failed', 'interrupted')
             AND finished_at IS NOT NULL
             AND finished_at >= strftime('%Y-%m-%dT%H:%M:%fZ', ?1, ?2)
         ORDER BY finished_at DESC, id DESC
         LIMIT ?3"
    )
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
    let mut statement = connection.prepare(&recent_candidates_sql())?;
    let rows = statement.query_map(
        params![now, format!("-{window_days} days"), RECENT_FETCH_CAP],
        map_occurrence_row,
    )?;
    let mut seen = std::collections::HashSet::new();
    let mut out = Vec::new();
    for row in rows {
        let Some(row) = row? else { continue };
        if seen.insert(row.activity_key.clone()) {
            out.push(row);
            if out.len() as i64 >= limit {
                break;
            }
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
        "SELECT job_queue.id, job_queue.kind, job_queue.payload, job_queue.last_error, job_queue.created_at, job_queue.attempts
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
            attempts: row.get(5)?,
        })
    })?;
    let mut out = Vec::new();
    for row in rows {
        out.push(row?);
    }
    Ok(out)
}

/// A `transcript_jobs` row literally `running` with no matching open
/// `job_runs` occurrence (`run_key = 'direct:' || activity_key`) — the
/// transcript-runner finalizer (sol diff R1 #6) means this should not exist
/// while the app is alive, but the read model surfaces it honestly as
/// `stalled` rather than hiding it, mirroring [`stalled_queue_rows`].
pub(crate) struct StalledTranscriptRow {
    pub id: String,
    pub company_id: Option<String>,
    pub source_label: Option<String>,
    pub source_url: String,
    pub started_at: String,
}

pub(crate) fn stalled_transcript_rows(
    connection: &Connection,
) -> StorageResult<Vec<StalledTranscriptRow>> {
    let mut statement = connection.prepare(
        "SELECT transcript_jobs.id, transcript_jobs.company_id, transcript_jobs.source_label,
                transcript_jobs.source_url, transcript_jobs.started_at
         FROM transcript_jobs
         LEFT JOIN job_runs
             ON job_runs.run_key = 'direct:transcript:' || transcript_jobs.id
             AND job_runs.status = 'running'
         WHERE transcript_jobs.status = 'running' AND job_runs.id IS NULL",
    )?;
    let rows = statement.query_map([], |row| {
        Ok(StalledTranscriptRow {
            id: row.get(0)?,
            company_id: row.get(1)?,
            source_label: row.get(2)?,
            source_url: row.get(3)?,
            started_at: row.get(4)?,
        })
    })?;
    let mut out = Vec::new();
    for row in rows {
        out.push(row?);
    }
    Ok(out)
}

/// One domain row's terminal-ish `(status, error)` — the raw material for
/// the recent-occurrence domain-status override (sol diff R1 #4): the
/// ledger's own `job_runs.status` alone cannot represent `partial`, nor a
/// fan-out job that finished its own attempt while its members still run —
/// only the domain row knows either.
///
/// sol diff R3 #3: `StorageResult<Option<_>>`, never a bare `Option` that
/// collapsed a genuine query failure (a dropped table, a schema/type error)
/// into the SAME "no row" the caller reads for a genuinely absent run — which
/// let the recent item silently keep the ledger's stale `succeeded` while the
/// domain lookup had actually failed. `.optional()?` keeps `None` for a truly
/// absent row; any other error propagates with `?`.
pub(crate) fn autopilot_run_status(
    connection: &Connection,
    run_id: &str,
) -> StorageResult<Option<(String, Option<String>)>> {
    connection
        .query_row(
            "SELECT status, last_error FROM autopilot_run WHERE id = ?1",
            [run_id],
            |row| Ok((row.get(0)?, row.get(1)?)),
        )
        .optional()
        .map_err(StorageError::from)
}

/// sol diff R3 #3: same fail-closed fix as [`autopilot_run_status`].
pub(crate) fn kpi_run_status(
    connection: &Connection,
    run_id: &str,
) -> StorageResult<Option<(String, Option<String>)>> {
    connection
        .query_row(
            "SELECT status, last_error FROM kpi_ingest_runs WHERE id = ?1",
            [run_id],
            |row| Ok((row.get(0)?, row.get(1)?)),
        )
        .optional()
        .map_err(StorageError::from)
}

/// A `job_runs` row's `activity_key` by id — the cheap summary path's way of
/// naming a registry `settle_failed` handle's key without the full
/// `occurrences_by_id` join (sol diff R1 #5).
///
/// sol diff R4 #1: `StorageResult<Option<_>>` — `.ok()` used to collapse a
/// genuine SQL/decode failure into the same `None` a truly absent row
/// produces, letting `resolved_key_statuses` silently drop the stalled key
/// (and a lower-precedence `queued` row with the same key could then leak
/// into the topbar count).
pub(crate) fn activity_key_for_occurrence(
    connection: &Connection,
    id: i64,
) -> StorageResult<Option<String>> {
    connection
        .query_row(
            "SELECT activity_key FROM job_runs WHERE id = ?1",
            [id],
            |row| row.get(0),
        )
        .optional()
        .map_err(StorageError::from)
}

/// Bounded (≤ `limit`) raw member subjects for a sweep/batch parent (sol
/// diff R1 #14) — the expanded row's evidence of WHAT it is processing. Each
/// member run's report document title, raw (never composed prose).
///
/// sol diff R2 #5: propagates a genuine SQL failure with `?` — `.ok()`/
/// `unwrap_or_default` used to collapse it into the same empty `Vec` a
/// parent with zero members legitimately produces, silently hiding the
/// difference from the caller.
///
/// sol diff R3 #1 (same class): a malformed `enqueued_run_ids_json` is a
/// `StorageError` too, not a silent empty member list — the same column
/// [`parent_progress`] now fails closed on.
pub(crate) fn member_subjects(
    connection: &Connection,
    table: &str,
    id: &str,
    limit: usize,
) -> StorageResult<Vec<String>> {
    let run_ids_json: Option<String> = connection
        .query_row(
            &format!("SELECT enqueued_run_ids_json FROM {table} WHERE id = ?1"),
            [id],
            |row| row.get(0),
        )
        .optional()?
        .flatten();
    let run_ids: Vec<String> = match run_ids_json {
        None => Vec::new(),
        Some(json) => serde_json::from_str(&json)?,
    };
    let mut subjects = Vec::with_capacity(limit.min(run_ids.len()));
    for run_id in run_ids.into_iter().take(limit) {
        let subject: Option<String> = connection
            .query_row(
                "SELECT report_documents.title FROM autopilot_run
                 JOIN report_documents ON report_documents.id = autopilot_run.report_document_id
                 WHERE autopilot_run.id = ?1",
                [&run_id],
                |row| row.get::<_, Option<String>>(0),
            )
            .optional()?
            .flatten();
        if let Some(subject) = subject.filter(|title| !title.trim().is_empty()) {
            subjects.push(subject);
        }
    }
    Ok(subjects)
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
    // sol diff R1 #12: candidates are every non-terminal, non-sweep-child
    // run; liveness itself is checked per-candidate via
    // `crate::jobs::autopilot_liveness::has_live_stage_job` — an exact `IN` match
    // over the five deterministic stage ids, never the previous `id LIKE
    // 'autopilot:' || autopilot_run.id || ':%'` (exploitable by a run id
    // containing `_`, a SQLite LIKE wildcard, into a false-positive "live"
    // read for an unrelated row).
    let mut statement = connection.prepare(
        "SELECT id, company_id, last_error, updated_at, report_document_id
         FROM autopilot_run
         WHERE status IN ('pending', 'running')
             AND sweep_id IS NULL",
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
    let mut candidates = Vec::new();
    for row in rows {
        candidates.push(row?);
    }
    drop(statement);

    let mut out = Vec::new();
    for (domain, document_id) in candidates {
        if !crate::jobs::autopilot_liveness::has_live_stage_job(connection, &domain.id)? {
            out.push((domain, document_id));
        }
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

/// A sweep/batch parent's full state (sol diff R3 #1): the row's OWN durable
/// counters (`candidates_total`, `runs_failed`, `skipped_existing`) AND every
/// enqueued member's current `autopilot_run.status`, read fresh so an
/// in-progress parent's counters move as members complete. Replaces an untyped
/// `(done, total, failed, enqueued)` tuple that folded `partial` members into
/// "done", ignored the row's own `runs_failed`/`skipped_existing`, and treated
/// a malformed `enqueued_run_ids_json` as an empty (rather than unreadable)
/// member list.
#[derive(Debug, Clone)]
pub(crate) struct ParentAggregate {
    pub parent_status: String,
    pub parent_error: Option<String>,
    /// How many candidates the parent's own drain counted when it ran.
    pub candidates_total: i64,
    /// `enqueued_run_ids_json`'s length — real member runs, never counting
    /// `skipped_existing`/`runs_failed` candidates that never became one.
    pub enqueued: i64,
    /// Enqueued members still `pending`/`running` — never counts
    /// `skipped_existing` (skipped candidates were never enqueued at all).
    pub live: i64,
    pub member_succeeded: i64,
    pub member_partial: i64,
    pub member_failed: i64,
    /// Candidates a storage error prevented enqueuing (the row's own column).
    pub runs_failed: i64,
    /// Candidates already terminal before this run, so skipped (`0` for
    /// `pipeline_reextraction_batches`, which has no such column).
    pub skipped_existing: i64,
}

impl ParentAggregate {
    /// `{done,total,failed}` for the DTO's `ActivityProgress` (D1): `done`
    /// counts a member `partial` as done (it finished, just not cleanly) and
    /// a skipped candidate as done (it needed no work); `failed` folds in the
    /// row's own enqueue failures alongside member failures.
    pub(crate) fn progress(&self) -> (i64, i64, i64) {
        (
            self.member_succeeded + self.member_partial + self.skipped_existing,
            self.candidates_total,
            self.member_failed + self.runs_failed,
        )
    }

    /// Derive the parent's resolved `(status, error)` from the row AND its
    /// members (sol diff R2 #3 lineage, tightened by sol diff R3 #1):
    ///
    /// - the parent's own row still `queued`/`running` (hasn't finished
    ///   dispatching) → trivially `running`, regardless of any member;
    /// - any enqueued member still non-terminal → `running` (`live` is
    ///   computed strictly from ENQUEUED members — a skipped candidate was
    ///   never enqueued, so it can never keep the parent "in flight");
    /// - a `failed` parent row with NO members ever enqueued → `failed`,
    ///   keeping the row's own error (a storage-level abort before any
    ///   candidate was even attempted);
    /// - no members enqueued and zero candidates on a terminal row →
    ///   `succeeded` (nothing to do, nothing failed);
    /// - otherwise: any member `partial`, or any failure (member or
    ///   row-level `runs_failed`) mixed with a success/skip → `partial`; all
    ///   outcomes failures → `failed`; all outcomes successes/skips →
    ///   `succeeded`.
    pub(crate) fn resolve(&self) -> (String, Option<String>) {
        if self.parent_status == "queued" || self.parent_status == "running" {
            return ("running".to_owned(), None);
        }
        if self.live > 0 {
            return ("running".to_owned(), None);
        }
        if self.enqueued == 0 {
            if self.parent_status == "failed" {
                return ("failed".to_owned(), self.parent_error.clone());
            }
            if self.candidates_total == 0 {
                return ("succeeded".to_owned(), self.parent_error.clone());
            }
        }
        let any_failure = self.member_failed > 0 || self.runs_failed > 0;
        let any_success_or_skip =
            self.member_succeeded > 0 || self.member_partial > 0 || self.skipped_existing > 0;
        if self.member_partial > 0 || (any_failure && any_success_or_skip) {
            return ("partial".to_owned(), self.parent_error.clone());
        }
        if any_failure {
            return ("failed".to_owned(), self.parent_error.clone());
        }
        ("succeeded".to_owned(), self.parent_error.clone())
    }
}

/// Read a sweep/batch parent row plus its member runs' current status (sol
/// diff R3 #1). `table` is `history_sweeps` or `pipeline_reextraction_batches`
/// — only the former has a `skipped_existing` column, so the other reports it
/// as `0` (never a schema drift, no such candidates exist for that table).
///
/// A malformed `enqueued_run_ids_json` is a `StorageError` (sol diff R3 #1):
/// silently treating it as an empty member list used to let a sweep that
/// actually enqueued work report as if it had none.
pub(crate) fn parent_progress(
    connection: &Connection,
    table: &str,
    id: &str,
) -> StorageResult<Option<ParentAggregate>> {
    let skipped_existing_column = if table == "history_sweeps" {
        "skipped_existing"
    } else {
        "0"
    };
    let sql = format!(
        "SELECT status, error, candidates_total, runs_failed, {skipped_existing_column}, \
         enqueued_run_ids_json FROM {table} WHERE id = ?1"
    );
    #[allow(clippy::type_complexity)]
    let row: Option<(String, Option<String>, i64, i64, i64, Option<String>)> = connection
        .query_row(&sql, [id], |row| {
            Ok((
                row.get(0)?,
                row.get(1)?,
                row.get(2)?,
                row.get(3)?,
                row.get(4)?,
                row.get(5)?,
            ))
        })
        .optional()?;
    let Some((
        parent_status,
        parent_error,
        candidates_total,
        runs_failed,
        skipped_existing,
        run_ids_json,
    )) = row
    else {
        return Ok(None);
    };

    let run_ids: Vec<String> = match run_ids_json {
        None => Vec::new(),
        Some(json) => serde_json::from_str(&json)?,
    };

    // sol diff R4 #2: reject a duplicate declared member id up front — left
    // unchecked, one real member row could satisfy two declared slots and
    // silently pass the "every declared id observed" check below.
    let mut seen_ids = std::collections::HashSet::with_capacity(run_ids.len());
    for run_id in &run_ids {
        if !seen_ids.insert(run_id.as_str()) {
            return Err(StorageError::ActivityParentMemberInvariant {
                table: table.to_owned(),
                id: id.to_owned(),
                reason: format!("duplicate declared member id {run_id}"),
            });
        }
    }

    let (member_succeeded, member_partial, member_failed, live) = if run_ids.is_empty() {
        (0i64, 0i64, 0i64, 0i64)
    } else {
        let placeholders = vec!["?"; run_ids.len()].join(", ");
        let status_sql =
            format!("SELECT id, status FROM autopilot_run WHERE id IN ({placeholders})");
        let mut statement = connection.prepare(&status_sql)?;
        let rows = statement.query_map(rusqlite::params_from_iter(run_ids.iter()), |row| {
            Ok((row.get::<_, String>(0)?, row.get::<_, String>(1)?))
        })?;
        let mut statuses: std::collections::HashMap<String, String> =
            std::collections::HashMap::with_capacity(run_ids.len());
        for row in rows {
            let (member_id, status) = row?;
            statuses.insert(member_id, status);
        }
        let (mut member_succeeded, mut member_partial, mut member_failed, mut live) =
            (0i64, 0i64, 0i64, 0i64);
        for run_id in &run_ids {
            // sol diff R4 #2: every declared id must yield exactly one
            // member row with a recognized status — a missing referenced run
            // or an unknown status used to contribute to none of
            // live/succeeded/partial/failed, letting a completed parent with
            // a broken reference silently resolve `succeeded`.
            let status = statuses.get(run_id).ok_or_else(|| {
                StorageError::ActivityParentMemberInvariant {
                    table: table.to_owned(),
                    id: id.to_owned(),
                    reason: format!("declared member {run_id} has no matching autopilot_run row"),
                }
            })?;
            match status.as_str() {
                "succeeded" => member_succeeded += 1,
                "partial" => member_partial += 1,
                "failed" => member_failed += 1,
                "pending" | "running" => live += 1,
                other => {
                    return Err(StorageError::ActivityParentMemberInvariant {
                        table: table.to_owned(),
                        id: id.to_owned(),
                        reason: format!(
                            "declared member {run_id} has unrecognized status '{other}'"
                        ),
                    });
                }
            }
        }
        (member_succeeded, member_partial, member_failed, live)
    };

    let enqueued = run_ids.len() as i64;
    // sol diff R4 #2: once the parent is terminal (its own row is no longer
    // queued/running AND no enqueued member is still live), the durable
    // candidate counters must add up — a drift means the producer's own
    // bookkeeping is corrupt, not something the aggregate should paper over.
    let terminal = parent_status != "queued" && parent_status != "running" && live == 0;
    if terminal && candidates_total != enqueued + runs_failed + skipped_existing {
        return Err(StorageError::ActivityParentMemberInvariant {
            table: table.to_owned(),
            id: id.to_owned(),
            reason: format!(
                "candidate counter drift: candidates_total={candidates_total} != \
                 enqueued({enqueued}) + runs_failed({runs_failed}) + \
                 skipped_existing({skipped_existing})"
            ),
        });
    }

    Ok(Some(ParentAggregate {
        parent_status,
        parent_error,
        candidates_total,
        enqueued,
        live,
        member_succeeded,
        member_partial,
        member_failed,
        runs_failed,
        skipped_existing,
    }))
}

#[cfg(test)]
#[path = "activity_reads_tests.rs"]
mod tests;
