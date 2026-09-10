//! The `report_delay` / Dziś non-arrival witness (ADR 0083 §8): a filing
//! witnesses only when it classifies as a periodic-report filing via
//! [`crate::source_adapters::periodic_filing::is_periodic_report_filing`].

use rusqlite::{params, Connection};

use crate::storage::StorageResult;

/// The "no witness" half of `report_delay` (ADR 0083 §8): true when no
/// `Official report` feed item for this company, published on/after
/// `event_date`, classifies as a periodic-report filing.
pub(in crate::storage) fn has_no_witnessing_report(
    connection: &Connection,
    company_id: &str,
    event_date: &str,
) -> StorageResult<bool> {
    let mut statement = connection.prepare(
        "SELECT fi.title, fi.body_text FROM feed_items fi JOIN feed_item_companies fic
            ON fic.feed_item_id = fi.id WHERE fic.company_id = ?1
            AND fi.type = 'Official report' AND fi.published_at >= ?2",
    )?;
    let items: Vec<(String, Option<String>)> = statement
        .query_map(params![company_id, event_date], |row| {
            Ok((row.get(0)?, row.get(1)?))
        })?
        .collect::<Result<Vec<_>, _>>()?;
    Ok(!items.iter().any(|(title, body)| {
        crate::source_adapters::periodic_filing::is_periodic_report_filing(title, body.as_deref())
    }))
}
