//! Evidence-specifics mapping for one `attention_events` row (v0.60 D6):
//! the concrete WHAT-happened fields from the joined columns, by `evidence_type`.

use rusqlite::Row;

use super::{
    EVIDENCE_AUTOPILOT_RUN, EVIDENCE_COMPANY_SIGNAL, EVIDENCE_JOB, EVIDENCE_SOURCE_RECONCILIATION,
};

/// `(evidence_title, evidence_detail)` for one row, given the resolved
/// `evidence_type` and the fire-time snapshot title (preferred over every
/// live join). Reads the row's join columns at their FIXED positions — see
/// the `SELECT` in `list_attention_events`: 10 `signal_title`, 11
/// `recon_title`, 12 `recon_adapter`, 13 `run_document_title`, 14
/// `run_status`, 16 `job_kind`, 17 `job_last_error`.
pub(super) fn evidence_specifics(
    row: &Row<'_>,
    evidence_type: &str,
    snapshot_title: Option<String>,
) -> rusqlite::Result<(Option<String>, Option<String>)> {
    Ok(match evidence_type {
        EVIDENCE_COMPANY_SIGNAL => (snapshot_title.or(row.get::<_, Option<String>>(10)?), None),
        EVIDENCE_SOURCE_RECONCILIATION => {
            let title: Option<String> = snapshot_title.or(row.get(11)?);
            let adapter: Option<String> = row.get(12)?;
            (title, adapter.map(|id| super::adapter_display_name(&id)))
        }
        EVIDENCE_AUTOPILOT_RUN => (
            snapshot_title.or(row.get::<_, Option<String>>(13)?),
            row.get(14)?,
        ),
        // A terminally failed background job (ADR 0091 dec. 1): the fire-time
        // SUBJECT snapshot when the handler had one, else the queue's own
        // `last_error`; the detail carries the raw job `kind`.
        EVIDENCE_JOB => (
            snapshot_title.or(row.get::<_, Option<String>>(17)?),
            row.get(16)?,
        ),
        _ => (None, None),
    })
}
