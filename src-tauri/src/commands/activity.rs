//! Activity read model (ADR 0109, #133; `docs/contracts.md` § Activity): one
//! composed view over the durable queue, the `job_runs` occurrence history,
//! and the direct-activity registry. UI-only reads (not exposed as MCP
//! tools), off the UI thread via `spawn_blocking`, ONE pool checkout per call.

use rusqlite::OptionalExtension;
use serde::Serialize;

use crate::app_state::AppState;
use crate::jobs::activity_identity::{identity_for_job, ActivityFamily, ActivityTarget};
use crate::storage::activity_reads as reads;
use crate::storage::activity_reads::OccurrenceRow;
use crate::storage::activity_registry;
use crate::storage::StorageResult;

/// Progress counters for a parent task (sweep/batch/backfill) — `None` for a
/// leaf item.
#[derive(Debug, Clone, Serialize)]
#[cfg_attr(feature = "ts-export", derive(ts_rs::TS))]
#[cfg_attr(
    feature = "ts-export",
    ts(export, export_to = "../../src/api/generated/")
)]
#[serde(rename_all = "camelCase")]
pub struct ActivityProgress {
    pub done: i64,
    pub total: i64,
    pub failed: i64,
}

/// One row of the Activity panel (contracts.md § Activity — field names are
/// normative).
#[derive(Debug, Clone, Serialize)]
#[cfg_attr(feature = "ts-export", derive(ts_rs::TS))]
#[cfg_attr(
    feature = "ts-export",
    ts(export, export_to = "../../src/api/generated/")
)]
#[serde(rename_all = "camelCase")]
pub struct ActivityItem {
    pub id: String,
    pub activity_key: String,
    pub family: ActivityFamily,
    /// `queued | running | stalled | succeeded | failed | partial | interrupted`.
    #[cfg_attr(
        feature = "ts-export",
        ts(
            type = "\"queued\" | \"running\" | \"stalled\" | \"succeeded\" | \"failed\" | \"partial\" | \"interrupted\""
        )
    )]
    pub status: String,
    pub subject: String,
    pub company_id: Option<String>,
    pub qualified_ticker: Option<String>,
    pub progress: Option<ActivityProgress>,
    pub in_flight: Option<i64>,
    pub attempt: i64,
    pub started_at: String,
    pub finished_at: Option<String>,
    pub error: Option<String>,
    /// Bounded (≤ 10) raw subjects of a parent task's members (a sweep's
    /// documents) — the expanded row lists them (contract § 5, sol diff R1 #14).
    pub members: Vec<String>,
    pub target: ActivityTarget,
}

#[derive(Debug, Clone, Serialize)]
#[cfg_attr(feature = "ts-export", derive(ts_rs::TS))]
#[cfg_attr(
    feature = "ts-export",
    ts(export, export_to = "../../src/api/generated/")
)]
#[serde(rename_all = "camelCase")]
pub struct ActivityView {
    pub active: Vec<ActivityItem>,
    pub queued: Vec<ActivityItem>,
    pub recent: Vec<ActivityItem>,
    pub generated_at: String,
}

#[derive(Debug, Clone, Serialize)]
#[cfg_attr(feature = "ts-export", derive(ts_rs::TS))]
#[cfg_attr(
    feature = "ts-export",
    ts(export, export_to = "../../src/api/generated/")
)]
#[serde(rename_all = "camelCase")]
pub struct ActivitySummary {
    pub active: i64,
    pub queued: i64,
    pub last_finished_at: Option<String>,
}

/// Recent window (contracts.md § Activity).
const RECENT_WINDOW_DAYS: i64 = 7;
/// Cap applied AFTER the per-`activityKey` collapse (contracts.md § Activity).
const RECENT_CAP: i64 = 40;
/// Bounded member-subject list per parent (sol diff R1 #14).
const MEMBER_LIMIT: usize = 10;

fn now_iso() -> String {
    time::OffsetDateTime::now_utc()
        .format(&time::format_description::well_known::Rfc3339)
        .unwrap_or_else(|_| "1970-01-01T00:00:00Z".to_owned())
}

/// Parent (sweep/batch/backfill) member progress (D1: `{done,total,failed}`),
/// only for the families that have one — `None` for every leaf item.
/// `historyFetch` reads its LIVE counters from `BackfillProgress` (sol diff
/// R1 #14) rather than a `job_runs`-adjacent table, since a backfill has no
/// domain parent row of its own.
fn parent_progress_for(
    connection: &rusqlite::Connection,
    state: &AppState,
    family: ActivityFamily,
    activity_key: &str,
    company_id: Option<&str>,
) -> Option<ActivityProgress> {
    if family == ActivityFamily::HistoryFetch {
        return history_fetch_progress(state, company_id);
    }
    let (table, prefix) = match family {
        ActivityFamily::ReportSweep => ("history_sweeps", "report-sweep:"),
        ActivityFamily::Reextraction => ("pipeline_reextraction_batches", "reextraction:"),
        _ => return None,
    };
    let id = activity_key.strip_prefix(prefix)?;
    let (done, total, failed) = reads::parent_progress(connection, table, id)
        .ok()
        .flatten()?;
    Some(ActivityProgress {
        done,
        total,
        failed,
    })
}

/// A running backfill's LIVE progress (sol diff R1 #14): `done` = documents
/// durably stored so far, `total` = items ingested so far (items seen but
/// not yet necessarily stored), `failed` = detail-fetch errors — all real
/// `BackfillProgress` counters, never a fabricated target (a backfill has no
/// known page count ahead of time). `None` once the backfill is no longer
/// `running` (a leaf/terminal backfill carries no live progress).
fn history_fetch_progress(state: &AppState, company_id: Option<&str>) -> Option<ActivityProgress> {
    let progress = state.get_backfill_progress(company_id?)?;
    if progress.status != "running" {
        return None;
    }
    Some(ActivityProgress {
        done: progress.documents_stored as i64,
        total: progress.items_ingested as i64,
        failed: progress.detail_errors as i64,
    })
}

fn in_flight_for(progress: &Option<ActivityProgress>) -> Option<i64> {
    progress
        .as_ref()
        .map(|p| (p.total - p.done - p.failed).max(0))
}

/// Bounded (≤ [`MEMBER_LIMIT`]) raw member subjects for a sweep/batch parent
/// (sol diff R1 #14) — `[]` for every family without members.
fn members_for(
    connection: &rusqlite::Connection,
    family: ActivityFamily,
    activity_key: &str,
) -> Vec<String> {
    let (table, prefix) = match family {
        ActivityFamily::ReportSweep => ("history_sweeps", "report-sweep:"),
        ActivityFamily::Reextraction => ("pipeline_reextraction_batches", "reextraction:"),
        _ => return Vec::new(),
    };
    let Some(id) = activity_key.strip_prefix(prefix) else {
        return Vec::new();
    };
    reads::member_subjects(connection, table, id, MEMBER_LIMIT)
}

/// Resolve the domain-truth `(status, error)` for a TERMINAL occurrence row
/// headed into `recent` (sol diff R1 #4): `job_runs.status` alone can never
/// represent `partial` (the schema has no such status), nor a fan-out job
/// that finished its OWN attempt while its domain row — and its members —
/// are still running. `None` leaves the occurrence's own ledger status.
fn domain_status_override(
    connection: &rusqlite::Connection,
    family: ActivityFamily,
    activity_key: &str,
) -> Option<(String, Option<String>)> {
    match family {
        ActivityFamily::ReportReading => {
            let run_id = activity_key.strip_prefix("report-reading:")?;
            let (status, last_error) = reads::autopilot_run_status(connection, run_id)?;
            match status.as_str() {
                "failed" => Some(("failed".to_owned(), last_error)),
                "partial" => Some(("partial".to_owned(), last_error)),
                _ => None,
            }
        }
        ActivityFamily::ReportSweep | ActivityFamily::Reextraction => {
            let (table, prefix) = match family {
                ActivityFamily::ReportSweep => ("history_sweeps", "report-sweep:"),
                _ => ("pipeline_reextraction_batches", "reextraction:"),
            };
            let id = activity_key.strip_prefix(prefix)?;
            let (status, error) = reads::parent_status(connection, table, id)?;
            match status.as_str() {
                // The fan-out's OWN job finished, but its domain row is not
                // terminal — members are still running. Stays active, never
                // demoted to `recent`.
                "queued" | "running" => Some(("running".to_owned(), None)),
                "failed" => {
                    let (done, _total, failed) = reads::parent_progress(connection, table, id)
                        .ok()
                        .flatten()
                        .unwrap_or((0, 0, 0));
                    if done > 0 && failed > 0 {
                        Some(("partial".to_owned(), error))
                    } else {
                        Some(("failed".to_owned(), error))
                    }
                }
                _ => None,
            }
        }
        ActivityFamily::KpiIngest => {
            let run_id = activity_key.strip_prefix("kpi-ingest:")?;
            let (status, last_error) = reads::kpi_run_status(connection, run_id)?;
            match status.as_str() {
                "failed" => Some(("failed".to_owned(), last_error)),
                "partial" => Some(("partial".to_owned(), last_error)),
                // No `cancelled` slot in the DTO's status vocabulary — the
                // closest honest fit without fabricating a new one.
                "cancelled" => Some(("failed".to_owned(), last_error)),
                _ => None,
            }
        }
        _ => None,
    }
}

fn occurrence_to_item(
    connection: &rusqlite::Connection,
    state: &AppState,
    row: OccurrenceRow,
    status: &str,
) -> ActivityItem {
    let qualified_ticker = row
        .company_id
        .as_deref()
        .and_then(|id| reads::qualified_ticker(connection, id));
    let progress = parent_progress_for(
        connection,
        state,
        row.family,
        &row.activity_key,
        row.company_id.as_deref(),
    );
    let in_flight = in_flight_for(&progress);
    let members = members_for(connection, row.family, &row.activity_key);
    ActivityItem {
        id: format!("job_runs:{}", row.id),
        activity_key: row.activity_key,
        family: row.family,
        status: status.to_owned(),
        subject: row.subject,
        company_id: row.company_id,
        qualified_ticker,
        progress,
        in_flight,
        attempt: row.attempt,
        started_at: row.started_at,
        finished_at: row.finished_at,
        error: row.error,
        members,
        target: row.target,
    }
}

/// The active job_runs ids: a queue row literally `running` WITH an open
/// occurrence, ∪ the live direct-activity registry entries (ADR 0109 dec. 4).
fn active_ids(connection: &rusqlite::Connection, state: &AppState) -> StorageResult<Vec<i64>> {
    let mut ids = reads::queue_running_occurrence_ids(connection)?;
    ids.extend(activity_registry::live_run_ids(state)?);
    ids.sort_unstable();
    ids.dedup();
    Ok(ids)
}

fn queued_items(
    connection: &rusqlite::Connection,
    state: &AppState,
) -> StorageResult<Vec<ActivityItem>> {
    Ok(reads::pending_jobs(connection)?
        .into_iter()
        .filter_map(|job| queue_row_item(connection, state, job, "queued"))
        .collect())
}

/// A `job_queue` row literally `running` with no matching open `job_runs`
/// occurrence — `stalled`, never fabricated `active` (D3/ADR 0109 dec. 4).
fn stalled_queue_items(
    connection: &rusqlite::Connection,
    state: &AppState,
) -> StorageResult<Vec<ActivityItem>> {
    Ok(reads::stalled_queue_rows(connection)?
        .into_iter()
        .filter_map(|job| queue_row_item(connection, state, job, "stalled"))
        .collect())
}

fn queue_row_item(
    connection: &rusqlite::Connection,
    state: &AppState,
    job: reads::PendingJobRow,
    status: &str,
) -> Option<ActivityItem> {
    let identity = identity_for_job(&job.kind, &job.id, &job.payload, connection)?;
    let qualified_ticker = identity
        .company_id
        .as_deref()
        .and_then(|id| reads::qualified_ticker(connection, id));
    let progress = parent_progress_for(
        connection,
        state,
        identity.family,
        &identity.activity_key,
        identity.company_id.as_deref(),
    );
    let in_flight = in_flight_for(&progress);
    let members = members_for(connection, identity.family, &identity.activity_key);
    Some(ActivityItem {
        id: format!("job_queue:{}", job.id),
        activity_key: identity.activity_key,
        family: identity.family,
        status: status.to_owned(),
        subject: identity.subject,
        company_id: identity.company_id,
        qualified_ticker,
        progress,
        in_flight,
        // sol diff R1 #14: the real `job_queue.attempts` count, never 0.
        attempt: job.attempts,
        started_at: job.created_at,
        finished_at: None,
        error: job.last_error,
        members,

        target: identity.target,
    })
}

/// A KPI ingest run (ADR 0109 dec. 3: never a `job_runs` writer — its live
/// lease IS the activity signal). `status` is `"running"` for a leased run,
/// `"queued"` for an unleased one (D3/ADR 0109 dec. 4).
fn kpi_run_item(
    connection: &rusqlite::Connection,
    run: reads::KpiRunRow,
    status: &str,
) -> ActivityItem {
    ActivityItem {
        id: format!("kpi_ingest_runs:{}", run.id),
        activity_key: format!("kpi-ingest:{}", run.id),
        family: ActivityFamily::KpiIngest,
        status: status.to_owned(),
        subject: reads::document_title_bound(connection, &run.report_document_id),
        qualified_ticker: reads::qualified_ticker(connection, &run.company_id),
        company_id: Some(run.company_id.clone()),
        progress: None,
        in_flight: None,
        attempt: 0,
        started_at: run.created_at,
        finished_at: None,
        error: run.last_error,
        members: Vec::new(),

        target: ActivityTarget::Company {
            company_id: run.company_id,
            tool: Some(crate::jobs::activity_identity::ActivityTool::Dokumenty {
                document_id: run.report_document_id,
            }),
        },
    }
}

/// A `transcript_jobs` row still `queued` — not yet awaited/started, so no
/// `job_runs`/registry entry exists for it yet (ADR 0109 dec. 3).
fn queued_transcript_item(
    connection: &rusqlite::Connection,
    row: reads::QueuedTranscriptRow,
) -> ActivityItem {
    let subject = row
        .source_label
        .filter(|label| !label.trim().is_empty())
        .unwrap_or(row.source_url);
    ActivityItem {
        id: format!("transcript_jobs:{}", row.id),
        activity_key: format!("transcript:{}", row.id),
        family: ActivityFamily::Transcript,
        status: "queued".to_owned(),
        subject,
        qualified_ticker: row
            .company_id
            .as_deref()
            .and_then(|id| reads::qualified_ticker(connection, id)),
        company_id: row.company_id,
        progress: None,
        in_flight: None,
        attempt: 0,
        started_at: row.created_at,
        finished_at: None,
        error: None,
        members: Vec::new(),

        target: ActivityTarget::Transcripts,
    }
}

/// A `transcript_jobs` row literally `running` with no matching open
/// `job_runs` occurrence — `stalled` (sol diff R1 #6: the transcript
/// runner's finalizer means this should not exist while the app is alive,
/// but the read model states it honestly rather than hiding it, mirroring
/// [`stalled_queue_items`]).
fn stalled_transcript_items(connection: &rusqlite::Connection) -> StorageResult<Vec<ActivityItem>> {
    Ok(reads::stalled_transcript_rows(connection)?
        .into_iter()
        .map(|row| {
            let subject = row
                .source_label
                .filter(|label| !label.trim().is_empty())
                .unwrap_or(row.source_url);
            ActivityItem {
                id: format!("transcript_jobs:{}", row.id),
                activity_key: format!("transcript:{}", row.id),
                family: ActivityFamily::Transcript,
                status: "stalled".to_owned(),
                subject,
                qualified_ticker: row
                    .company_id
                    .as_deref()
                    .and_then(|id| reads::qualified_ticker(connection, id)),
                company_id: row.company_id,
                progress: None,
                in_flight: None,
                attempt: 0,
                started_at: row.started_at,
                finished_at: None,
                error: None,
                members: Vec::new(),

                target: ActivityTarget::Transcripts,
            }
        })
        .collect())
}

/// Non-terminal domain rows (`autopilot_run`, `history_sweeps`,
/// `pipeline_reextraction_batches`) with no live backing job — `stalled`
/// (D3/ADR 0109 dec. 4). Rare once startup reconciliation has run; the read
/// model still states it honestly rather than hiding it.
fn stalled_domain_items(connection: &rusqlite::Connection) -> StorageResult<Vec<ActivityItem>> {
    let mut items = Vec::new();

    for (row, document_id) in reads::stalled_autopilot_runs(connection)? {
        items.push(ActivityItem {
            id: format!("autopilot_run:{}", row.id),
            activity_key: format!("report-reading:{}", row.id),
            family: ActivityFamily::ReportReading,
            status: "stalled".to_owned(),
            subject: reads::document_title_bound(connection, &document_id),
            qualified_ticker: reads::qualified_ticker(connection, &row.company_id),
            company_id: Some(row.company_id.clone()),
            progress: None,
            in_flight: None,
            attempt: 0,
            started_at: row.updated_at,
            finished_at: None,
            error: row.error,
            members: Vec::new(),

            target: ActivityTarget::Company {
                company_id: row.company_id,
                tool: Some(crate::jobs::activity_identity::ActivityTool::Dokumenty { document_id }),
            },
        });
    }

    let parents: [(&str, &str, ActivityFamily, &str); 2] = [
        (
            "history_sweeps",
            "report-sweep",
            ActivityFamily::ReportSweep,
            crate::jobs::history_sweep::HISTORY_SWEEP_KIND,
        ),
        (
            "pipeline_reextraction_batches",
            "reextraction",
            ActivityFamily::Reextraction,
            crate::jobs::pipeline_reextraction::PIPELINE_REEXTRACTION_KIND,
        ),
    ];
    for (table, prefix, family, job_kind) in parents {
        for row in reads::stalled_parent_rows(connection, table, job_kind)? {
            let progress = reads::parent_progress(connection, table, &row.id)
                .ok()
                .flatten()
                .map(|(done, total, failed)| ActivityProgress {
                    done,
                    total,
                    failed,
                });
            let in_flight = in_flight_for(&progress);
            let members = members_for(connection, family, &format!("{prefix}:{}", row.id));
            items.push(ActivityItem {
                id: format!("{table}:{}", row.id),
                activity_key: format!("{prefix}:{}", row.id),
                family,
                status: "stalled".to_owned(),
                subject: reads::ticker(connection, &row.company_id),
                qualified_ticker: reads::qualified_ticker(connection, &row.company_id),
                company_id: Some(row.company_id.clone()),
                progress,
                in_flight,
                attempt: 0,
                started_at: row.updated_at,
                finished_at: None,
                error: row.error,
                members,

                target: ActivityTarget::Company {
                    company_id: row.company_id,
                    tool: Some(crate::jobs::activity_identity::ActivityTool::Pokrycie),
                },
            });
        }
    }

    Ok(items)
}

/// Registry-backed stalled items (sol diff R1 #3): a settle-failed occurrence
/// (a real `job_runs` row, still durably `running`, but the registry could
/// not confirm settling it) and an unrecorded live entry (`begin_attempt`
/// itself failed — no row at all, rendered directly from its identity).
fn registry_stalled_items(
    connection: &rusqlite::Connection,
    state: &AppState,
) -> StorageResult<Vec<ActivityItem>> {
    let snapshot = activity_registry::snapshot(state);
    let mut items: Vec<ActivityItem> =
        reads::occurrences_by_id(connection, &snapshot.stalled_run_ids)?
            .into_iter()
            .map(|row| occurrence_to_item(connection, state, row, "stalled"))
            .collect();
    for (identity, started_at) in snapshot.stalled_unrecorded {
        let qualified_ticker = identity
            .company_id
            .as_deref()
            .and_then(|id| reads::qualified_ticker(connection, id));
        items.push(ActivityItem {
            id: format!("unrecorded:{}", identity.activity_key),
            activity_key: identity.activity_key,
            family: identity.family,
            status: "stalled".to_owned(),
            subject: identity.subject,
            company_id: identity.company_id,
            qualified_ticker,
            progress: None,
            in_flight: None,
            attempt: 0,
            started_at,
            finished_at: None,
            error: None,
            members: Vec::new(),

            target: identity.target,
        });
    }
    Ok(items)
}

/// Collapse items sharing an `activityKey` down to one, keeping the FIRST
/// occurrence — callers append sources in precedence order (running, then
/// stalled, then queued: sol diff R1 #5) so the first-seen item is always
/// the highest-precedence one. Also used within one precedence tier (a
/// sweep/batch parent's own item plus its suppressed children collapsing to
/// the SAME key).
fn collapse_by_activity_key(items: Vec<ActivityItem>) -> Vec<ActivityItem> {
    let mut seen = std::collections::HashSet::new();
    items
        .into_iter()
        .filter(|item| seen.insert(item.activity_key.clone()))
        .collect()
}

/// Compose the Activity view (D3, sol diff R1 #5): every source collapses
/// into ONE keyed task map with precedence `running > stalled > queued >
/// recent` — so a task live in more than one source (a sweep's own
/// occurrence plus its pending member, a KPI run's queue step plus its run
/// row) still renders as exactly one item, in the highest-precedence
/// section. `recent` (terminal occurrences, domain-status-overridden per
/// sol diff R1 #4) excludes any key already live elsewhere. ONE pool
/// checkout for the whole call (sol diff R1 #13: every read propagates its
/// `StorageResult` with `?`, never `unwrap_or_default` swallowing a storage
/// failure into a fake empty state).
pub(crate) fn compute_activity(state: &AppState) -> StorageResult<ActivityView> {
    let connection = state.checkout()?;
    let now = now_iso();

    // ---- running (precedence 0) ----
    let active_ids = active_ids(&connection, state)?;
    let mut running: Vec<ActivityItem> = reads::occurrences_by_id(&connection, &active_ids)?
        .into_iter()
        .map(|row| occurrence_to_item(&connection, state, row, "running"))
        .collect();
    let (kpi_leased, kpi_unleased) = reads::kpi_runs_by_lease(&connection, &now)?;
    running.extend(
        kpi_leased
            .into_iter()
            .map(|run| kpi_run_item(&connection, run, "running")),
    );

    // ---- stalled (precedence 1) ----
    let mut stalled = stalled_queue_items(&connection, state)?;
    stalled.extend(stalled_domain_items(&connection)?);
    stalled.extend(stalled_transcript_items(&connection)?);
    stalled.extend(registry_stalled_items(&connection, state)?);

    // ---- queued (precedence 2) ----
    let mut queued = queued_items(&connection, state)?;
    queued.extend(
        kpi_unleased
            .into_iter()
            .map(|run| kpi_run_item(&connection, run, "queued")),
    );
    queued.extend(
        reads::queued_transcript_jobs(&connection)?
            .into_iter()
            .map(|row| queued_transcript_item(&connection, row)),
    );

    // Collapse running/stalled/queued into ONE keyed set — first-seen wins,
    // and sources were appended in precedence order above.
    let mut live = Vec::with_capacity(running.len() + stalled.len() + queued.len());
    live.extend(running);
    live.extend(stalled);
    live.extend(queued);
    let live = collapse_by_activity_key(live);
    let live_keys: std::collections::HashSet<String> =
        live.iter().map(|item| item.activity_key.clone()).collect();

    let active: Vec<ActivityItem> = live
        .iter()
        .filter(|item| item.status == "running" || item.status == "stalled")
        .cloned()
        .collect();
    let queued: Vec<ActivityItem> = live
        .into_iter()
        .filter(|item| item.status == "queued")
        .collect();

    // ---- recent (precedence 3: only keys not already live) ----
    let recent = reads::recent_occurrences(&connection, &now, RECENT_WINDOW_DAYS, RECENT_CAP)?
        .into_iter()
        .filter(|row| !live_keys.contains(&row.activity_key))
        .map(|row| {
            let override_status =
                domain_status_override(&connection, row.family, &row.activity_key);
            let (status, error_override) = match &override_status {
                Some((status, error)) => (status.clone(), Some(error.clone())),
                None => (row.status.clone(), None),
            };
            let mut item = occurrence_to_item(&connection, state, row, &status);
            if let Some(error) = error_override {
                item.error = error;
            }
            item
        })
        .collect();

    Ok(ActivityView {
        active,
        queued,
        recent,
        generated_at: now,
    })
}

/// Cheap `(activityKey, status)` pairs for the summary (sol diff R1 #5): the
/// SAME precedence collapse `compute_activity` does for `running`/`stalled`/
/// `queued`, but skipping every per-item join (ticker/progress/members) —
/// contracts.md's "two indexed counts + one max, no fan-out" intent.
fn resolved_key_statuses(
    connection: &rusqlite::Connection,
    state: &AppState,
    now: &str,
) -> StorageResult<Vec<(String, String)>> {
    let mut running: Vec<(String, String)> = Vec::new();
    for row in reads::occurrences_by_id(connection, &active_ids(connection, state)?)? {
        running.push((row.activity_key, "running".to_owned()));
    }
    let (kpi_leased, kpi_unleased) = reads::kpi_runs_by_lease(connection, now)?;
    for run in &kpi_leased {
        running.push((format!("kpi-ingest:{}", run.id), "running".to_owned()));
    }

    let mut stalled: Vec<(String, String)> = Vec::new();
    for job in reads::stalled_queue_rows(connection)? {
        if let Some(identity) = identity_for_job(&job.kind, &job.id, &job.payload, connection) {
            stalled.push((identity.activity_key, "stalled".to_owned()));
        }
    }
    for (row, _document_id) in reads::stalled_autopilot_runs(connection)? {
        stalled.push((format!("report-reading:{}", row.id), "stalled".to_owned()));
    }
    let parents: [(&str, &str, &str); 2] = [
        (
            "history_sweeps",
            "report-sweep",
            crate::jobs::history_sweep::HISTORY_SWEEP_KIND,
        ),
        (
            "pipeline_reextraction_batches",
            "reextraction",
            crate::jobs::pipeline_reextraction::PIPELINE_REEXTRACTION_KIND,
        ),
    ];
    for (table, prefix, job_kind) in parents {
        for row in reads::stalled_parent_rows(connection, table, job_kind)? {
            stalled.push((format!("{prefix}:{}", row.id), "stalled".to_owned()));
        }
    }
    for row in reads::stalled_transcript_rows(connection)? {
        stalled.push((format!("transcript:{}", row.id), "stalled".to_owned()));
    }
    let snapshot = activity_registry::snapshot(state);
    for id in &snapshot.stalled_run_ids {
        if let Some(key) = reads::activity_key_for_occurrence(connection, *id) {
            stalled.push((key, "stalled".to_owned()));
        }
    }
    for (identity, _started_at) in &snapshot.stalled_unrecorded {
        stalled.push((identity.activity_key.clone(), "stalled".to_owned()));
    }

    let mut queued: Vec<(String, String)> = Vec::new();
    for job in reads::pending_jobs(connection)? {
        if let Some(identity) = identity_for_job(&job.kind, &job.id, &job.payload, connection) {
            queued.push((identity.activity_key, "queued".to_owned()));
        }
    }
    for run in &kpi_unleased {
        queued.push((format!("kpi-ingest:{}", run.id), "queued".to_owned()));
    }
    for row in reads::queued_transcript_jobs(connection)? {
        queued.push((format!("transcript:{}", row.id), "queued".to_owned()));
    }

    let mut seen = std::collections::HashSet::new();
    let mut out = Vec::new();
    for pair in running.into_iter().chain(stalled).chain(queued) {
        if seen.insert(pair.0.clone()) {
            out.push(pair);
        }
    }
    Ok(out)
}

/// Compose the Activity summary (D3, sol diff R1 #5): unique-key counts —
/// `active` counts `running` keys ONLY (a `stalled` item stays in the panel's
/// In-progress section with its own status, but never drives the topbar
/// spinner), `queued` counts `queued` keys. ONE pool checkout for the whole
/// call.
pub(crate) fn compute_activity_summary(state: &AppState) -> StorageResult<ActivitySummary> {
    let connection = state.checkout()?;
    let now = now_iso();

    let resolved = resolved_key_statuses(&connection, state, &now)?;
    let active = resolved
        .iter()
        .filter(|(_, status)| status == "running")
        .count() as i64;
    let queued = resolved
        .iter()
        .filter(|(_, status)| status == "queued")
        .count() as i64;

    let last_finished_at: Option<String> = connection
        .query_row(
            "SELECT MAX(finished_at) FROM job_runs WHERE finished_at IS NOT NULL",
            [],
            |row| row.get(0),
        )
        .optional()?
        .flatten();

    Ok(ActivitySummary {
        active,
        queued,
        last_finished_at,
    })
}

/// UI-only read: the current Activity view (`read` classification, not an MCP
/// tool — contracts.md § Activity).
#[tauri::command]
pub async fn list_activity(state: tauri::State<'_, AppState>) -> Result<ActivityView, String> {
    let state = state.inner().clone();
    tauri::async_runtime::spawn_blocking(move || {
        compute_activity(&state).map_err(|error| error.to_string())
    })
    .await
    .map_err(|error| format!("list_activity task failed: {error}"))?
}

/// UI-only read: the topbar summary (`read` classification, not an MCP tool).
#[tauri::command]
pub async fn get_activity_summary(
    state: tauri::State<'_, AppState>,
) -> Result<ActivitySummary, String> {
    let state = state.inner().clone();
    tauri::async_runtime::spawn_blocking(move || {
        compute_activity_summary(&state).map_err(|error| error.to_string())
    })
    .await
    .map_err(|error| format!("get_activity_summary task failed: {error}"))?
}

#[cfg(test)]
#[path = "activity_tests.rs"]
mod tests;
