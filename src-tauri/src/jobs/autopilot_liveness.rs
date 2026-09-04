//! Liveness of an automatic report-reading run's stage jobs (ADR 0109 dec. 4).
//! Shared by startup reconcile and the Activity read model's stalled-row
//! detection so both use the SAME definition of "live". Lives beside
//! `autopilot.rs` (pinned in `file-size-baseline.json`) rather than inside it.

use crate::jobs::autopilot::{
    stage_job_id, AUTOPILOT_STAGE_KIND, STAGE_CROSS_REFERENCE, STAGE_DIFF, STAGE_EXTRACT,
    STAGE_FETCH, STAGE_NOTIFY,
};

/// The five deterministic stage names, in pipeline order.
pub const STAGE_NAMES: [&str; 5] = [
    STAGE_FETCH,
    STAGE_EXTRACT,
    STAGE_DIFF,
    STAGE_CROSS_REFERENCE,
    STAGE_NOTIFY,
];

/// Whether `run_id` has a live (`pending`/`running`) stage job right now — an
/// exact `IN` match over the five deterministic stage ids (sol diff R1 #12:
/// the previous `id LIKE 'autopilot:' || run_id || ':%'` pattern is
/// exploitable by a `run_id` containing `_`, a SQLite LIKE wildcard for "any
/// single char" — an unrelated row can then false-positive-match, making a
/// genuinely stranded run look live). Shared by startup reconcile and the
/// Activity read model's stalled-row detection so both use the SAME,
/// correct definition of "live".
pub(crate) fn has_live_stage_job(
    connection: &rusqlite::Connection,
    run_id: &str,
) -> crate::storage::StorageResult<bool> {
    let ids: Vec<String> = STAGE_NAMES
        .iter()
        .map(|stage| stage_job_id(run_id, stage))
        .collect();
    let placeholders = vec!["?"; ids.len()].join(", ");
    let sql = format!(
        "SELECT EXISTS(
            SELECT 1 FROM job_queue
            WHERE kind = ? AND status IN ('pending', 'running') AND id IN ({placeholders})
        )"
    );
    let mut params: Vec<&str> = vec![AUTOPILOT_STAGE_KIND];
    params.extend(ids.iter().map(String::as_str));
    Ok(connection.query_row(&sql, rusqlite::params_from_iter(params), |row| row.get(0))?)
}
