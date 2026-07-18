//! Parsed MAR art. 19 insider-transaction storage (ADR 0083 Decision 6, plan
//! v0.57 T4).
//!
//! Mirrors the ownership ESPI-stake seam ([`super::ownership::update_stakes_from_major_holdings`]):
//! a confirmed `insider_transaction` signal's cover note is parsed by the
//! deterministic [`fundamentals::insider`](crate::fundamentals::insider) parser
//! into per-notification units, each upserted in place by a **deterministic id**
//! from `(feed_item_id, unit_index)` so a re-parse never duplicates. A filing
//! whose cover note yields no writable unit parks once in `insider_espi_unparsed`
//! (never guessed — the buy/sell/volume detail lives in the attachment PDF, T4b).

use std::collections::BTreeSet;

use rusqlite::OptionalExtension;

use super::*;
use crate::fundamentals::insider::attachment::AttachmentTxUnit;
use crate::fundamentals::insider::{
    parse_insider_notification, InsiderNotificationParse, InsiderUnit, ParsedInsiderUnit,
};

/// A stored parsed insider-transaction row (headless in T4 — the TS DTO + read
/// command land with the insider overview in T6, avoiding an orphaned export).
#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize)]
#[serde(rename_all = "camelCase")]
pub struct InsiderTransactionRow {
    pub id: String,
    pub company_id: String,
    pub feed_item_id: String,
    pub unit_index: i64,
    pub person_name_raw: String,
    pub person_normalized: String,
    pub role: Option<String>,
    pub related_pdmr_raw: Option<String>,
    pub related_pdmr_normalized: Option<String>,
    pub related_pdmr_role: Option<String>,
    pub direction: Option<String>,
    pub instrument: Option<String>,
    pub volume: Option<String>,
    pub price: Option<String>,
    pub currency: Option<String>,
    pub tx_date: Option<String>,
    pub created_at: String,
}

const TX_COLUMNS: &str = "id, company_id, feed_item_id, unit_index, person_name_raw, \
     person_normalized, role, related_pdmr_raw, related_pdmr_normalized, related_pdmr_role, \
     direction, instrument, volume, price, currency, tx_date, created_at";

fn row_from(row: &rusqlite::Row<'_>) -> rusqlite::Result<InsiderTransactionRow> {
    Ok(InsiderTransactionRow {
        id: row.get(0)?,
        company_id: row.get(1)?,
        feed_item_id: row.get(2)?,
        unit_index: row.get(3)?,
        person_name_raw: row.get(4)?,
        person_normalized: row.get(5)?,
        role: row.get(6)?,
        related_pdmr_raw: row.get(7)?,
        related_pdmr_normalized: row.get(8)?,
        related_pdmr_role: row.get(9)?,
        direction: row.get(10)?,
        instrument: row.get(11)?,
        volume: row.get(12)?,
        price: row.get(13)?,
        currency: row.get(14)?,
        tx_date: row.get(15)?,
        created_at: row.get(16)?,
    })
}

/// Deterministic transaction id: one row per `(feed_item_id, unit_index)`, so a
/// re-parse of the same filing upserts each unit in place.
fn transaction_id(feed_item_id: &str, unit_index: usize) -> String {
    format!("insidertx_{}_{}", slug_part(feed_item_id), unit_index)
}

/// Upsert one parsed unit. Idempotent by deterministic id; `created_at` and the
/// domain key are never rewritten.
pub(super) fn upsert_transaction(
    connection: &Connection,
    company_id: &str,
    feed_item_id: &str,
    unit_index: usize,
    unit: &ParsedInsiderUnit,
) -> StorageResult<InsiderTransactionRow> {
    let id = transaction_id(feed_item_id, unit_index);
    connection.execute(
        "
        INSERT INTO insider_transactions (
            id, company_id, feed_item_id, unit_index, person_name_raw, person_normalized,
            role, related_pdmr_raw, related_pdmr_normalized, related_pdmr_role,
            direction, instrument, volume, price, currency, tx_date
        ) VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11, ?12, ?13, ?14, ?15, ?16)
        ON CONFLICT(id) DO UPDATE SET
            person_name_raw = excluded.person_name_raw,
            person_normalized = excluded.person_normalized,
            role = excluded.role,
            related_pdmr_raw = excluded.related_pdmr_raw,
            related_pdmr_normalized = excluded.related_pdmr_normalized,
            related_pdmr_role = excluded.related_pdmr_role,
            direction = excluded.direction,
            instrument = excluded.instrument,
            volume = excluded.volume,
            price = excluded.price,
            currency = excluded.currency,
            tx_date = excluded.tx_date,
            updated_at = strftime('%Y-%m-%dT%H:%M:%fZ', 'now')
        ",
        params![
            id,
            company_id,
            feed_item_id,
            unit_index as i64,
            unit.person_raw,
            unit.person_normalized,
            unit.role.map(|r| r.as_str()),
            unit.related_pdmr_raw,
            unit.related_pdmr_normalized,
            unit.related_pdmr_role.map(|r| r.as_str()),
            unit.direction.map(|d| d.as_str()),
            unit.instrument.map(|i| i.as_str()),
            unit.volume,
            unit.price,
            unit.currency,
            unit.tx_date,
        ],
    )?;
    connection
        .query_row(
            &format!("SELECT {TX_COLUMNS} FROM insider_transactions WHERE id = ?1"),
            [&id],
            row_from,
        )
        .map_err(StorageError::from)
}

/// Record a classified insider filing whose cover note yielded no writable unit
/// (idempotent per feed item). NO transaction row is written — never guess.
fn record_unparsed(
    connection: &Connection,
    feed_item_id: &str,
    company_id: &str,
    reason: &str,
) -> StorageResult<()> {
    connection.execute(
        "
        INSERT INTO insider_espi_unparsed (feed_item_id, company_id, reason)
        VALUES (?1, ?2, ?3)
        ON CONFLICT(feed_item_id) DO UPDATE SET
            company_id = excluded.company_id,
            reason = excluded.reason,
            updated_at = strftime('%Y-%m-%dT%H:%M:%fZ', 'now')
        ",
        params![feed_item_id, company_id, reason],
    )?;
    Ok(())
}

/// All parsed transactions for a company, newest filing first (T6 read model input).
pub(super) fn list_by_company(
    connection: &Connection,
    company_id: &str,
) -> StorageResult<Vec<InsiderTransactionRow>> {
    let mut statement = connection.prepare(&format!(
        "SELECT {TX_COLUMNS} FROM insider_transactions WHERE company_id = ?1 \
         ORDER BY tx_date DESC, feed_item_id DESC, unit_index ASC"
    ))?;
    let rows = statement.query_map([company_id], row_from)?;
    rows.collect::<Result<Vec<_>, _>>()
        .map_err(StorageError::from)
}

/// A parsed transaction enriched with the two provenance fields the insider
/// overview read model (T6) needs: the filing's `signal_date` (the windowing
/// fallback when `tx_date` is NULL — the figures/date live in the attachment PDF
/// for most cover notes) and the feed item's `source_url` (the timeline's link to
/// the filing). Both are LEFT-joined so a transaction never vanishes when its
/// signal/feed row is absent.
#[derive(Debug, Clone)]
pub struct InsiderOverviewSource {
    pub tx: InsiderTransactionRow,
    pub signal_date: Option<String>,
    pub source_url: Option<String>,
}

/// Every parsed transaction for a company, enriched for the overview read model
/// (T6). Ordering is only a stable default — the read model re-sorts by the
/// effective (tx-or-filing) date it computes.
pub(super) fn list_for_overview(
    connection: &Connection,
    company_id: &str,
) -> StorageResult<Vec<InsiderOverviewSource>> {
    let mut statement = connection.prepare(
        "
        SELECT it.id, it.company_id, it.feed_item_id, it.unit_index, it.person_name_raw,
               it.person_normalized, it.role, it.related_pdmr_raw, it.related_pdmr_normalized,
               it.related_pdmr_role, it.direction, it.instrument, it.volume, it.price,
               it.currency, it.tx_date, it.created_at,
               cs.signal_date, fi.source_url
        FROM insider_transactions it
        LEFT JOIN feed_items fi ON fi.id = it.feed_item_id
        LEFT JOIN company_signals cs
          ON cs.feed_item_id = it.feed_item_id AND cs.category = 'insider_transaction'
        WHERE it.company_id = ?1
        ORDER BY it.tx_date DESC, it.feed_item_id DESC, it.unit_index ASC
        ",
    )?;
    let rows = statement.query_map([company_id], |row| {
        Ok(InsiderOverviewSource {
            tx: row_from(row)?,
            signal_date: row.get(17)?,
            source_url: row.get(18)?,
        })
    })?;
    rows.collect::<Result<Vec<_>, _>>()
        .map_err(StorageError::from)
}

/// A confirmed `insider_transaction` filing awaiting a cover-note parse: not yet
/// turned into any transaction row, nor parked as unparsed.
struct PendingInsiderFiling {
    feed_item_id: String,
    company_id: String,
    title: String,
    body_text: String,
}

fn load_pending(connection: &Connection) -> StorageResult<Vec<PendingInsiderFiling>> {
    let mut statement = connection.prepare(
        "
        SELECT cs.feed_item_id, cs.company_id, fi.title, COALESCE(fi.body_text, '')
        FROM company_signals cs
        JOIN feed_items fi ON fi.id = cs.feed_item_id
        WHERE cs.category = 'insider_transaction'
          AND cs.status = 'confirmed'
          AND fi.body_text IS NOT NULL AND TRIM(fi.body_text) <> ''
          AND NOT EXISTS (
            SELECT 1 FROM insider_transactions it WHERE it.feed_item_id = cs.feed_item_id
          )
          AND NOT EXISTS (
            SELECT 1 FROM insider_espi_unparsed u WHERE u.feed_item_id = cs.feed_item_id
          )
        ORDER BY cs.created_at DESC
        ",
    )?;
    let rows = statement.query_map([], |row| {
        Ok(PendingInsiderFiling {
            feed_item_id: row.get(0)?,
            company_id: row.get(1)?,
            title: row.get(2)?,
            body_text: row.get(3)?,
        })
    })?;
    rows.collect::<Result<Vec<_>, _>>()
        .map_err(StorageError::from)
}

/// Parse every pending confirmed `insider_transaction` filing's cover note into
/// transaction rows. Idempotent end-to-end: a filing is attempted exactly once
/// (a clean parse writes rows; a no-unit parse parks), so re-running creates zero
/// new rows. Returns the number of transaction rows written.
pub(super) fn parse_insider_transactions(connection: &Connection) -> StorageResult<usize> {
    let pending = load_pending(connection)?;
    let mut written = 0usize;
    for filing in pending {
        match parse_insider_notification(&filing.title, &filing.body_text) {
            InsiderNotificationParse::Units(units) => {
                let mut wrote_any = false;
                for (index, unit) in units.iter().enumerate() {
                    if let InsiderUnit::Clean(parsed) = unit {
                        upsert_transaction(
                            connection,
                            &filing.company_id,
                            &filing.feed_item_id,
                            index,
                            parsed,
                        )?;
                        written += 1;
                        wrote_any = true;
                    }
                }
                if !wrote_any {
                    record_unparsed(
                        connection,
                        &filing.feed_item_id,
                        &filing.company_id,
                        "person_unresolved",
                    )?;
                }
            }
            InsiderNotificationParse::NotFound => {
                record_unparsed(
                    connection,
                    &filing.feed_item_id,
                    &filing.company_id,
                    "not_found",
                )?;
            }
        }
    }
    Ok(written)
}

/// Whether a filing has been parked as unparsed (test/diagnostic helper).
pub(super) fn is_parked(connection: &Connection, feed_item_id: &str) -> StorageResult<bool> {
    let found: Option<String> = connection
        .query_row(
            "SELECT feed_item_id FROM insider_espi_unparsed WHERE feed_item_id = ?1",
            [feed_item_id],
            |row| row.get(0),
        )
        .optional()?;
    Ok(found.is_some())
}

// ===========================================================================
// Attachment-PDF tier (T4b): merge parsed notification-document units into the
// insider substrate, filling the NULLs the cover note left.
// ===========================================================================

/// A field the attachment tier declined to change because the existing value was
/// already non-NULL and disagreed with the document (never overwritten — recorded).
#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize)]
#[serde(rename_all = "camelCase")]
pub struct AttachmentConflict {
    pub feed_item_id: String,
    pub unit_index: i64,
    pub field: &'static str,
    pub existing: String,
    pub incoming: String,
}

/// The result of merging one filing's parsed notification document(s) into its
/// `insider_transactions` rows.
#[derive(Debug, Default, Clone, PartialEq, Eq)]
pub struct AttachmentMergeOutcome {
    /// NULL fields filled across matched units.
    pub filled: usize,
    /// New units appended (a PDF transaction row that matched no existing unit).
    pub appended: usize,
    /// Typed conflicts (nothing overwritten).
    pub conflicts: Vec<AttachmentConflict>,
}

/// One existing `insider_transactions` row's fields the merge reads/fills.
struct ExistingUnit {
    id: String,
    unit_index: i64,
    person_normalized: String,
    role: Option<String>,
    related_pdmr_raw: Option<String>,
    related_pdmr_normalized: Option<String>,
    related_pdmr_role: Option<String>,
    direction: Option<String>,
    instrument: Option<String>,
    volume: Option<String>,
    price: Option<String>,
    currency: Option<String>,
    tx_date: Option<String>,
}

/// A lenient person key (set of 4-char folded token prefixes) so an attachment's
/// NOMINATIVE name matches the cover note's genitive-recovered key across residual
/// declension differences (mirrors the real-data harness matcher). Two names match
/// when the smaller key is a subset of the larger.
fn person_key(name: &str) -> BTreeSet<String> {
    name.chars()
        .map(|c| match c {
            'ą' | 'Ą' => 'a',
            'ć' | 'Ć' => 'c',
            'ę' | 'Ę' => 'e',
            'ł' | 'Ł' => 'l',
            'ń' | 'Ń' => 'n',
            'ó' | 'Ó' => 'o',
            'ś' | 'Ś' => 's',
            'ź' | 'Ź' | 'ż' | 'Ż' => 'z',
            other => other.to_ascii_lowercase(),
        })
        .collect::<String>()
        .split(|c: char| !c.is_alphanumeric())
        .filter(|t| t.len() >= 2)
        .map(|t| t.chars().take(4).collect::<String>())
        .collect()
}

fn names_match(a: &BTreeSet<String>, b: &BTreeSet<String>) -> bool {
    !a.is_empty() && !b.is_empty() && (a.is_subset(b) || b.is_subset(a))
}

/// A NULL body field matches anything; two non-NULL values match only when equal.
fn field_compatible(existing: &Option<String>, incoming: &Option<String>) -> bool {
    match (existing, incoming) {
        (Some(a), Some(b)) => a == b,
        _ => true,
    }
}

fn load_existing_units(
    connection: &Connection,
    feed_item_id: &str,
) -> StorageResult<Vec<ExistingUnit>> {
    let mut statement = connection.prepare(
        "SELECT id, unit_index, person_normalized, role, related_pdmr_raw, \
         related_pdmr_normalized, related_pdmr_role, direction, instrument, volume, price, \
         currency, tx_date FROM insider_transactions WHERE feed_item_id = ?1 ORDER BY unit_index ASC",
    )?;
    let rows = statement.query_map([feed_item_id], |row| {
        Ok(ExistingUnit {
            id: row.get(0)?,
            unit_index: row.get(1)?,
            person_normalized: row.get(2)?,
            role: row.get(3)?,
            related_pdmr_raw: row.get(4)?,
            related_pdmr_normalized: row.get(5)?,
            related_pdmr_role: row.get(6)?,
            direction: row.get(7)?,
            instrument: row.get(8)?,
            volume: row.get(9)?,
            price: row.get(10)?,
            currency: row.get(11)?,
            tx_date: row.get(12)?,
        })
    })?;
    rows.collect::<Result<Vec<_>, _>>()
        .map_err(StorageError::from)
}

/// Merge parsed notification-document units into a filing's transaction rows.
///
/// Matching: each PDF unit is matched (greedy, one-to-one) to an existing unit by
/// **(person, direction, tx_date)** with NULL-tolerant fields (a NULL existing
/// field matches anything). On a match, only still-NULL existing fields are filled;
/// a disagreement with an existing non-NULL value changes nothing and is recorded
/// as a typed [`AttachmentConflict`]. A PDF unit that matches no existing unit is
/// appended as a new unit whose index extends `max(existing.unit_index)`.
pub(super) fn merge_attachment_units(
    connection: &Connection,
    company_id: &str,
    feed_item_id: &str,
    units: &[AttachmentTxUnit],
) -> StorageResult<AttachmentMergeOutcome> {
    let existing = load_existing_units(connection, feed_item_id)?;
    let mut consumed = vec![false; existing.len()];
    let mut outcome = AttachmentMergeOutcome::default();
    let mut next_index = existing.iter().map(|u| u.unit_index).max().unwrap_or(-1) + 1;

    for unit in units {
        let pdf_direction = unit.direction.map(|d| d.as_str().to_owned());
        let pdf_tx_date = unit.tx_date.clone();
        let pdf_key = person_key(&unit.person_normalized);

        let matched = existing.iter().enumerate().position(|(i, e)| {
            !consumed[i]
                && names_match(&pdf_key, &person_key(&e.person_normalized))
                && field_compatible(&e.direction, &pdf_direction)
                && field_compatible(&e.tx_date, &pdf_tx_date)
        });

        match matched {
            Some(i) => {
                consumed[i] = true;
                fill_matched_unit(connection, &existing[i], unit, feed_item_id, &mut outcome)?;
            }
            None => {
                // Append: a personless unit is never written (never guess).
                if unit.person_normalized.trim().is_empty() {
                    continue;
                }
                let parsed = ParsedInsiderUnit {
                    person_raw: unit.person_raw.clone(),
                    person_normalized: unit.person_normalized.clone(),
                    role: unit.role,
                    related_pdmr_raw: unit.related_pdmr_raw.clone(),
                    related_pdmr_normalized: unit.related_pdmr_normalized.clone(),
                    related_pdmr_role: None,
                    direction: unit.direction,
                    instrument: unit.instrument,
                    volume: unit.volume.clone(),
                    price: unit.price.clone(),
                    currency: unit.currency.clone(),
                    tx_date: unit.tx_date.clone(),
                };
                upsert_transaction(
                    connection,
                    company_id,
                    feed_item_id,
                    next_index as usize,
                    &parsed,
                )?;
                next_index += 1;
                outcome.appended += 1;
            }
        }
    }

    Ok(outcome)
}

/// Fill the NULL fields of one matched existing row from a PDF unit; record (never
/// apply) any conflict with a non-NULL existing value.
fn fill_matched_unit(
    connection: &Connection,
    existing: &ExistingUnit,
    unit: &AttachmentTxUnit,
    feed_item_id: &str,
    outcome: &mut AttachmentMergeOutcome,
) -> StorageResult<()> {
    // (field name, existing value, incoming value) for every fillable column.
    let incoming_role = unit.role.map(|r| r.as_str().to_owned());
    let incoming_direction = unit.direction.map(|d| d.as_str().to_owned());
    let incoming_instrument = unit.instrument.map(|i| i.as_str().to_owned());
    let plan: [(&'static str, &Option<String>, &Option<String>); 9] = [
        ("role", &existing.role, &incoming_role),
        ("direction", &existing.direction, &incoming_direction),
        ("instrument", &existing.instrument, &incoming_instrument),
        ("volume", &existing.volume, &unit.volume),
        ("price", &existing.price, &unit.price),
        ("currency", &existing.currency, &unit.currency),
        ("tx_date", &existing.tx_date, &unit.tx_date),
        (
            "related_pdmr_raw",
            &existing.related_pdmr_raw,
            &unit.related_pdmr_raw,
        ),
        (
            "related_pdmr_normalized",
            &existing.related_pdmr_normalized,
            &unit.related_pdmr_normalized,
        ),
    ];

    let mut merged: std::collections::HashMap<&'static str, Option<String>> =
        std::collections::HashMap::new();
    let mut any_fill = false;
    for (field, existing_value, incoming) in plan {
        let resolved = match (existing_value, incoming) {
            (None, Some(new)) => {
                any_fill = true;
                outcome.filled += 1;
                Some(new.clone())
            }
            (Some(old), Some(new)) if old != new => {
                outcome.conflicts.push(AttachmentConflict {
                    feed_item_id: feed_item_id.to_owned(),
                    unit_index: existing.unit_index,
                    field,
                    existing: old.clone(),
                    incoming: new.clone(),
                });
                existing_value.clone()
            }
            _ => existing_value.clone(),
        };
        merged.insert(field, resolved);
    }

    if !any_fill {
        return Ok(());
    }

    // The attachment parser does not resolve `related_pdmr_role`, so the existing
    // value is always preserved (the cover-note parser is its only writer).
    let related_role = existing.related_pdmr_role.clone();

    connection.execute(
        "UPDATE insider_transactions SET \
             role = ?2, direction = ?3, instrument = ?4, volume = ?5, price = ?6, \
             currency = ?7, tx_date = ?8, related_pdmr_raw = ?9, related_pdmr_normalized = ?10, \
             related_pdmr_role = ?11, updated_at = strftime('%Y-%m-%dT%H:%M:%fZ', 'now') \
         WHERE id = ?1",
        params![
            existing.id,
            merged["role"],
            merged["direction"],
            merged["instrument"],
            merged["volume"],
            merged["price"],
            merged["currency"],
            merged["tx_date"],
            merged["related_pdmr_raw"],
            merged["related_pdmr_normalized"],
            related_role,
        ],
    )?;
    Ok(())
}

/// Record the once-per-filing attachment-tier attempt (mirrors `record_unparsed`),
/// carrying the merge diagnostics. Terminal outcomes only — a transient fetch
/// failure is not recorded, so it retries on the next sweep.
pub(super) fn record_attachment_attempt(
    connection: &Connection,
    feed_item_id: &str,
    company_id: &str,
    outcome: &str,
    diagnostics: &AttachmentMergeOutcome,
) -> StorageResult<()> {
    connection.execute(
        "INSERT INTO insider_attachment_attempts \
             (feed_item_id, company_id, outcome, filled, appended, conflicts) \
         VALUES (?1, ?2, ?3, ?4, ?5, ?6) \
         ON CONFLICT(feed_item_id) DO UPDATE SET \
             company_id = excluded.company_id, outcome = excluded.outcome, \
             filled = excluded.filled, appended = excluded.appended, \
             conflicts = excluded.conflicts, \
             updated_at = strftime('%Y-%m-%dT%H:%M:%fZ', 'now')",
        params![
            feed_item_id,
            company_id,
            outcome,
            diagnostics.filled as i64,
            diagnostics.appended as i64,
            diagnostics.conflicts.len() as i64,
        ],
    )?;
    Ok(())
}

/// Whether the attachment tier already terminally attempted a filing.
pub(super) fn is_attachment_attempted(
    connection: &Connection,
    feed_item_id: &str,
) -> StorageResult<bool> {
    let found: Option<String> = connection
        .query_row(
            "SELECT feed_item_id FROM insider_attachment_attempts WHERE feed_item_id = ?1",
            [feed_item_id],
            |row| row.get(0),
        )
        .optional()?;
    Ok(found.is_some())
}

/// A classified insider filing that still needs the attachment tier: it has ≥1
/// parsed transaction row (a cover-note unit to fill / extend) and has not yet been
/// terminally attempted. `company` scopes a backfill; `None` sweeps all.
#[derive(Debug, Clone)]
pub struct AttachmentPendingFiling {
    pub company_id: String,
    pub feed_item_id: String,
}

pub(super) fn filings_needing_attachment(
    connection: &Connection,
    company_id: Option<&str>,
) -> StorageResult<Vec<AttachmentPendingFiling>> {
    let mut sql = String::from(
        "SELECT DISTINCT it.company_id, it.feed_item_id \
         FROM insider_transactions it \
         WHERE NOT EXISTS ( \
             SELECT 1 FROM insider_attachment_attempts a WHERE a.feed_item_id = it.feed_item_id \
         )",
    );
    if company_id.is_some() {
        sql.push_str(" AND it.company_id = ?1");
    }
    sql.push_str(" ORDER BY it.feed_item_id DESC");

    let mut statement = connection.prepare(&sql)?;
    let map = |row: &rusqlite::Row<'_>| {
        Ok(AttachmentPendingFiling {
            company_id: row.get(0)?,
            feed_item_id: row.get(1)?,
        })
    };
    let rows = match company_id {
        Some(id) => statement
            .query_map([id], map)?
            .collect::<Result<Vec<_>, _>>(),
        None => statement.query_map([], map)?.collect::<Result<Vec<_>, _>>(),
    };
    rows.map_err(StorageError::from)
}

/// Force a re-attempt of the attachment tier for one filing (test/backfill helper):
/// clears its terminal marker so the next sweep re-selects it.
pub(super) fn clear_attachment_attempt(
    connection: &Connection,
    feed_item_id: &str,
) -> StorageResult<()> {
    connection.execute(
        "DELETE FROM insider_attachment_attempts WHERE feed_item_id = ?1",
        [feed_item_id],
    )?;
    Ok(())
}

/// Clear every terminal attachment marker for one company (backfill re-attempt).
pub(super) fn clear_company_attachment_attempts(
    connection: &Connection,
    company_id: &str,
) -> StorageResult<()> {
    connection.execute(
        "DELETE FROM insider_attachment_attempts WHERE company_id = ?1",
        [company_id],
    )?;
    Ok(())
}

/// Insider-substrate domain store (ADR 0083). `AppState::insider()`.
#[derive(Clone)]
pub struct InsiderStore {
    db: Database,
}

impl InsiderStore {
    pub(super) fn new(db: Database) -> Self {
        Self { db }
    }

    /// Parse all pending confirmed insider filings' cover notes (idempotent).
    pub fn parse_pending(&self) -> StorageResult<usize> {
        let connection = self.db.checkout()?;
        parse_insider_transactions(&connection)
    }

    /// All parsed transactions for a company, newest first.
    pub fn list_by_company(&self, company_id: &str) -> StorageResult<Vec<InsiderTransactionRow>> {
        let connection = self.db.checkout()?;
        list_by_company(&connection, company_id)
    }

    /// Every parsed transaction enriched with `signal_date` + `source_url` for the
    /// insider overview read model (T6).
    pub fn list_for_overview(&self, company_id: &str) -> StorageResult<Vec<InsiderOverviewSource>> {
        let connection = self.db.checkout()?;
        list_for_overview(&connection, company_id)
    }

    /// Whether a filing was parked as unparsed.
    pub fn is_parked(&self, feed_item_id: &str) -> StorageResult<bool> {
        let connection = self.db.checkout()?;
        is_parked(&connection, feed_item_id)
    }

    // --- Attachment-PDF tier (T4b) ---

    /// Classified insider filings still needing the attachment tier (a cover-note
    /// row to fill, not yet terminally attempted). `company` scopes a backfill.
    pub fn filings_needing_attachment(
        &self,
        company_id: Option<&str>,
    ) -> StorageResult<Vec<AttachmentPendingFiling>> {
        let connection = self.db.checkout()?;
        filings_needing_attachment(&connection, company_id)
    }

    /// Merge parsed notification-document units into a filing's transaction rows
    /// (fill NULLs / append; conflicts recorded, never overwritten).
    pub fn merge_attachment_units(
        &self,
        company_id: &str,
        feed_item_id: &str,
        units: &[AttachmentTxUnit],
    ) -> StorageResult<AttachmentMergeOutcome> {
        let connection = self.db.checkout()?;
        merge_attachment_units(&connection, company_id, feed_item_id, units)
    }

    /// Record the once-per-filing terminal attachment attempt (with diagnostics).
    pub fn record_attachment_attempt(
        &self,
        feed_item_id: &str,
        company_id: &str,
        outcome: &str,
        diagnostics: &AttachmentMergeOutcome,
    ) -> StorageResult<()> {
        let connection = self.db.checkout()?;
        record_attachment_attempt(&connection, feed_item_id, company_id, outcome, diagnostics)
    }

    /// Whether the attachment tier already terminally attempted a filing.
    pub fn is_attachment_attempted(&self, feed_item_id: &str) -> StorageResult<bool> {
        let connection = self.db.checkout()?;
        is_attachment_attempted(&connection, feed_item_id)
    }

    /// Clear a filing's terminal attachment marker (backfill re-attempt).
    pub fn clear_attachment_attempt(&self, feed_item_id: &str) -> StorageResult<()> {
        let connection = self.db.checkout()?;
        clear_attachment_attempt(&connection, feed_item_id)
    }

    /// Clear all terminal attachment markers for one company (backfill re-attempt).
    pub fn clear_company_attachment_attempts(&self, company_id: &str) -> StorageResult<()> {
        let connection = self.db.checkout()?;
        clear_company_attachment_attempts(&connection, company_id)
    }
}

use super::database::Database;
