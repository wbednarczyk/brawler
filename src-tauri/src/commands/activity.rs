//! Activity read model (ADR 0109, #133; `docs/contracts.md` § Activity): one
//! composed view over the durable queue, the `job_runs` occurrence history,
//! and the direct-activity registry. UI-only reads (not exposed as MCP
//! tools), off the UI thread via `spawn_blocking`, ONE pool checkout per call.

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

fn now_iso() -> String {
    time::OffsetDateTime::now_utc()
        .format(&time::format_description::well_known::Rfc3339)
        .unwrap_or_else(|_| "1970-01-01T00:00:00Z".to_owned())
}

/// Parent (sweep/batch) member progress (D1: `{done,total,failed}`), only for
/// the families that have one — `None` for every leaf item.
fn parent_progress_for(
    connection: &rusqlite::Connection,
    family: ActivityFamily,
    activity_key: &str,
) -> Option<ActivityProgress> {
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

fn in_flight_for(progress: &Option<ActivityProgress>) -> Option<i64> {
    progress
        .as_ref()
        .map(|p| (p.total - p.done - p.failed).max(0))
}

fn occurrence_to_item(
    connection: &rusqlite::Connection,
    row: OccurrenceRow,
    status: &str,
) -> ActivityItem {
    let qualified_ticker = row
        .company_id
        .as_deref()
        .and_then(|id| reads::qualified_ticker(connection, id));
    let progress = parent_progress_for(connection, row.family, &row.activity_key);
    let in_flight = in_flight_for(&progress);
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
        members: Vec::new(),

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

fn queued_items(connection: &rusqlite::Connection) -> Vec<ActivityItem> {
    reads::pending_jobs(connection)
        .unwrap_or_default()
        .into_iter()
        .filter_map(|job| queue_row_item(connection, job, "queued"))
        .collect()
}

/// A `job_queue` row literally `running` with no matching open `job_runs`
/// occurrence — `stalled`, never fabricated `active` (D3/ADR 0109 dec. 4).
fn stalled_queue_items(connection: &rusqlite::Connection) -> Vec<ActivityItem> {
    reads::stalled_queue_rows(connection)
        .unwrap_or_default()
        .into_iter()
        .filter_map(|job| queue_row_item(connection, job, "stalled"))
        .collect()
}

fn queue_row_item(
    connection: &rusqlite::Connection,
    job: reads::PendingJobRow,
    status: &str,
) -> Option<ActivityItem> {
    let identity = identity_for_job(&job.kind, &job.id, &job.payload, connection)?;
    let qualified_ticker = identity
        .company_id
        .as_deref()
        .and_then(|id| reads::qualified_ticker(connection, id));
    let progress = parent_progress_for(connection, identity.family, &identity.activity_key);
    let in_flight = in_flight_for(&progress);
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
        attempt: 0,
        started_at: job.created_at,
        finished_at: None,
        error: job.last_error,
        members: Vec::new(),

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

/// Non-terminal domain rows (`autopilot_run`, `history_sweeps`,
/// `pipeline_reextraction_batches`) with no live backing job — `stalled`
/// (D3/ADR 0109 dec. 4). Rare once startup reconciliation has run; the read
/// model still states it honestly rather than hiding it.
fn stalled_domain_items(connection: &rusqlite::Connection) -> Vec<ActivityItem> {
    let mut items = Vec::new();

    for (row, document_id) in reads::stalled_autopilot_runs(connection).unwrap_or_default() {
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
        for row in reads::stalled_parent_rows(connection, table, job_kind).unwrap_or_default() {
            let progress = reads::parent_progress(connection, table, &row.id)
                .ok()
                .flatten()
                .map(|(done, total, failed)| ActivityProgress {
                    done,
                    total,
                    failed,
                });
            let in_flight = progress
                .as_ref()
                .map(|p| (p.total - p.done - p.failed).max(0));
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
                members: Vec::new(),

                target: ActivityTarget::Company {
                    company_id: row.company_id,
                    tool: Some(crate::jobs::activity_identity::ActivityTool::Pokrycie),
                },
            });
        }
    }

    items
}

/// Collapse items sharing an `activityKey` down to one (D1: a sweep/batch
/// parent's own item plus its suppressed children — e.g. several member
/// `autopilot_stage` jobs pending/running at once for one sweep — all resolve
/// to the SAME key; `active`/`queued` must render exactly one task, matching
/// what `recent`'s SQL collapse already does for terminal occurrences).
/// Stable: the first-seen item wins (its `progress` is freshly computed from
/// the domain row regardless of which duplicate carried it in).
fn collapse_by_activity_key(items: Vec<ActivityItem>) -> Vec<ActivityItem> {
    let mut seen = std::collections::HashSet::new();
    items
        .into_iter()
        .filter(|item| seen.insert(item.activity_key.clone()))
        .collect()
}

/// Compose the Activity view (D3): `active` (registry ∪ queue-running-with-
/// occurrence ∪ leased KPI runs ∪ stalled rows), `queued` (pending queue rows
/// ∪ unleased KPI runs, identity-resolved), `recent` (latest terminal
/// occurrence per `activityKey`, 7-day window, cap 40 after collapse). ONE
/// pool checkout for the whole call.
pub(crate) fn compute_activity(state: &AppState) -> StorageResult<ActivityView> {
    let connection = state.checkout()?;
    let now = now_iso();

    let active_ids = active_ids(&connection, state)?;
    let mut active: Vec<ActivityItem> = reads::occurrences_by_id(&connection, &active_ids)?
        .into_iter()
        .map(|row| occurrence_to_item(&connection, row, "running"))
        .collect();

    let (kpi_leased, kpi_unleased) = reads::kpi_runs_by_lease(&connection, &now)?;
    active.extend(
        kpi_leased
            .into_iter()
            .map(|run| kpi_run_item(&connection, run, "running")),
    );
    active.extend(stalled_queue_items(&connection));
    active.extend(stalled_domain_items(&connection));

    let mut queued = queued_items(&connection);
    queued.extend(
        kpi_unleased
            .into_iter()
            .map(|run| kpi_run_item(&connection, run, "queued")),
    );
    queued.extend(
        reads::queued_transcript_jobs(&connection)
            .unwrap_or_default()
            .into_iter()
            .map(|row| queued_transcript_item(&connection, row)),
    );

    let recent = reads::recent_occurrences(&connection, &now, RECENT_WINDOW_DAYS, RECENT_CAP)?
        .into_iter()
        .map(|row| {
            let status = row.status.clone();
            occurrence_to_item(&connection, row, &status)
        })
        .collect();

    Ok(ActivityView {
        active: collapse_by_activity_key(active),
        queued: collapse_by_activity_key(queued),
        recent,
        generated_at: now,
    })
}

/// Compose the Activity summary (D3): a handful of indexed counts + one max —
/// no full item construction (no document/company-title joins), matching
/// contracts.md's "two indexed counts + one max" intent. ONE pool checkout
/// for the whole call.
pub(crate) fn compute_activity_summary(state: &AppState) -> StorageResult<ActivitySummary> {
    let connection = state.checkout()?;
    let now = now_iso();

    let active_ids = active_ids(&connection, state)?;
    let counts = reads::summary_counts(&connection, &active_ids)?;
    let (kpi_leased, kpi_unleased) = reads::kpi_runs_by_lease(&connection, &now)?;
    let stalled = reads::stalled_queue_rows(&connection)?.len()
        + reads::stalled_autopilot_runs(&connection)?.len()
        + reads::stalled_parent_rows(
            &connection,
            "history_sweeps",
            crate::jobs::history_sweep::HISTORY_SWEEP_KIND,
        )?
        .len()
        + reads::stalled_parent_rows(
            &connection,
            "pipeline_reextraction_batches",
            crate::jobs::pipeline_reextraction::PIPELINE_REEXTRACTION_KIND,
        )?
        .len();

    let queued_transcripts = reads::queued_transcript_jobs(&connection)?.len();

    Ok(ActivitySummary {
        active: counts.active + kpi_leased.len() as i64 + stalled as i64,
        queued: counts.queued + kpi_unleased.len() as i64 + queued_transcripts as i64,
        last_finished_at: counts.last_finished_at,
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
