//! KNF short-selling registry storage (ADR 0069 decision 3, plan v0.55 T4).
//!
//! The adapter ([`crate::source_adapters::knf_short_selling`]) fetches the current
//! net-short-position register; this module diffs it against stored state and
//! records the outcome:
//!
//! - `short_positions` — current-state mirror, one row per (company, holder).
//! - `short_position_events` — append-only history (`entered | increased |
//!   decreased | exited`).
//! - one `feed_items` row + one `short_position_change` `company_signals` row per
//!   detected change, firing the v0.54 alert rules on the same path as the ESPI
//!   classifier ([`super::signals`] / [`super::attention`]).
//!
//! Entries are matched to companies by ISIN; unmatched issuers are skipped (never
//! auto-created). Re-polling an unchanged snapshot detects zero changes, so it
//! produces zero events / feed items / signals (idempotence is a hard requirement).

use std::collections::HashSet;

use serde::Deserialize;

use super::database::Database;
use super::*;
use crate::source_adapters::knf_short_selling::{
    KnfShortEntry, ADAPTER_ID, ATTRIBUTION, DISPLAY_NAME,
};

/// The typed signal category emitted for register changes (seeded by migration
/// 0080; empty patterns, so the rule classifier never matches it by text — the
/// signal is written directly here).
const SIGNAL_CATEGORY: &str = "short_position_change";
/// Feed-item display type, matching the other official-report sources.
const FEED_ITEM_TYPE: &str = "Official report";
/// Float tolerance below which two percentages are treated as unchanged.
const PCT_EPSILON: f64 = 1e-9;

/// Per-company short-selling read model (v0.55 T4b) — the cockpit
/// "Krótka sprzedaż (KNF)" panel. Current active positions, the change history,
/// the last remembered exit (empty-state "Ostatnia obecność"), the aggregate net
/// short %, and the 30-day change in pp.
#[derive(Debug, Clone, serde::Serialize)]
#[cfg_attr(feature = "ts-export", derive(ts_rs::TS))]
#[cfg_attr(
    feature = "ts-export",
    ts(export, export_to = "../../src/api/generated/")
)]
#[serde(rename_all = "camelCase")]
pub struct ShortPositionsView {
    /// Active positions (register mirror rows with `exited_at IS NULL`), largest
    /// first. A holder appears at most once (UNIQUE(company, holder)).
    pub positions: Vec<ShortPositionRow>,
    /// Change history, newest-first by the DOMAIN date (`position_date`), capped.
    pub events: Vec<ShortPositionEventRow>,
    /// The most recent remembered exit, if any — powers the empty state's
    /// "Ostatnia obecność" line even when no position is currently active.
    pub last_exit: Option<ShortPositionExit>,
    /// Sum of active `net_position_pct` (the aggregate net short position).
    pub aggregate_pct: f64,
    /// 30-day change in percentage points: the signed sum of register-change
    /// deltas dated within the last 30 days (entered = +to, increased/decreased =
    /// to−from, exited = −from). A clean "aggregate 30 days ago" is not derivable
    /// from the current mirror alone (only the latest per-holder value is stored),
    /// so this signed-event-delta sum is the definition — it equals
    /// `aggregate_now − aggregate_30d_ago` whenever every in-window change is
    /// represented by an event, which the ingester guarantees (one event per
    /// detected change).
    pub delta_30d_pp: f64,
    /// The register's last successful refresh (`source_adapters.last_success_at`
    /// for `knf-short-selling`), for the panel's attribution line. `None` until
    /// the first successful pull.
    pub register_updated_at: Option<String>,
}

#[derive(Debug, Clone, serde::Serialize)]
#[cfg_attr(feature = "ts-export", derive(ts_rs::TS))]
#[cfg_attr(
    feature = "ts-export",
    ts(export, export_to = "../../src/api/generated/")
)]
#[serde(rename_all = "camelCase")]
pub struct ShortPositionRow {
    pub holder_name: String,
    pub net_position_pct: f64,
    /// The register's calculation date (`YYYY-MM-DD`) for this position.
    pub position_date: String,
    /// True when this holder had a register change dated within the last 30 days
    /// (drives the "zmiana" chip on recently-changed rows).
    pub recently_changed: bool,
}

#[derive(Debug, Clone, serde::Serialize)]
#[cfg_attr(feature = "ts-export", derive(ts_rs::TS))]
#[cfg_attr(
    feature = "ts-export",
    ts(export, export_to = "../../src/api/generated/")
)]
#[serde(rename_all = "camelCase")]
pub struct ShortPositionEventRow {
    /// One of `entered | increased | decreased | exited`.
    pub kind: String,
    pub holder_name: String,
    pub from_pct: Option<f64>,
    pub to_pct: Option<f64>,
    pub position_date: String,
}

#[derive(Debug, Clone, serde::Serialize)]
#[cfg_attr(feature = "ts-export", derive(ts_rs::TS))]
#[cfg_attr(
    feature = "ts-export",
    ts(export, export_to = "../../src/api/generated/")
)]
#[serde(rename_all = "camelCase")]
pub struct ShortPositionExit {
    pub holder_name: String,
    /// Date (`YYYY-MM-DD`) the position dropped out of the register.
    pub exited_on: String,
}

#[derive(Debug, Clone, Deserialize)]
#[cfg_attr(feature = "ts-export", derive(ts_rs::TS))]
#[cfg_attr(
    feature = "ts-export",
    ts(export, export_to = "../../src/api/generated/")
)]
#[serde(rename_all = "camelCase")]
pub struct ShortPositionsInput {
    pub company_id: String,
}

/// A stored current-state position, loaded for diffing.
struct StoredPosition {
    company_id: String,
    holder_name: String,
    net_position_pct: f64,
    exited: bool,
}

/// Diff + persist the current KNF register (already parsed to typed entries).
/// Runs in a single immediate transaction so a snapshot lands atomically.
pub(super) fn ingest_knf_short_positions(
    connection: &mut Connection,
    entries: &[KnfShortEntry],
) -> StorageResult<SourceIngestionResult> {
    let transaction =
        connection.transaction_with_behavior(rusqlite::TransactionBehavior::Immediate)?;
    let fetched_at = now_iso();

    // 1. Resolve each entry to a tracked company by ISIN; unmatched issuers are
    //    skipped. `current_keys` is the complete set of live (company, holder)
    //    pairs in this snapshot — the basis for exit detection.
    struct Matched<'a> {
        company_id: String,
        qualified_ticker: String,
        entry: &'a KnfShortEntry,
    }
    let mut matched: Vec<Matched> = Vec::new();
    let mut current_keys: HashSet<(String, String)> = HashSet::new();
    let mut items_unmatched = 0;
    for entry in entries {
        match feed_matching::find_company_by_isin(&transaction, &entry.isin)? {
            Some(company) => {
                current_keys.insert((company.id.clone(), entry.holder_name.clone()));
                matched.push(Matched {
                    company_id: company.id,
                    qualified_ticker: company.qualified_ticker,
                    entry,
                });
            }
            None => items_unmatched += 1,
        }
    }

    let mut items_created = 0;

    // 2. Entered / increased / decreased for each matched entry.
    for row in &matched {
        let stored = load_stored_position(&transaction, &row.company_id, &row.entry.holder_name)?;
        let change = classify_change(stored.as_ref(), row.entry.net_position_pct);
        let Some((kind, from_pct)) = change else {
            continue; // unchanged — no event, no feed item, no signal
        };
        upsert_position(&transaction, &row.company_id, row.entry)?;
        record_event(
            &transaction,
            &row.company_id,
            &row.qualified_ticker,
            &row.entry.holder_name,
            kind,
            from_pct,
            Some(row.entry.net_position_pct),
            &row.entry.position_date,
            &fetched_at,
        )?;
        items_created += 1;
    }

    // 3. Exits: stored positions still marked live whose (company, holder) is no
    //    longer in the current register (register is complete for >= 0.5%).
    for stored in load_live_positions(&transaction)? {
        let key = (stored.company_id.clone(), stored.holder_name.clone());
        if current_keys.contains(&key) {
            continue;
        }
        let qualified_ticker = company_ticker(&transaction, &stored.company_id)?;
        mark_exited(
            &transaction,
            &stored.company_id,
            &stored.holder_name,
            &fetched_at,
        )?;
        record_event(
            &transaction,
            &stored.company_id,
            &qualified_ticker,
            &stored.holder_name,
            "exited",
            Some(stored.net_position_pct),
            None,
            &fetched_at[..10.min(fetched_at.len())],
            &fetched_at,
        )?;
        items_created += 1;
    }

    // Best-effort run-outcome (no-op when the catalog row is not yet seeded).
    let _ = super::ingestion::record_source_outcome(
        &transaction,
        ADAPTER_ID,
        &fetched_at,
        entries.len(),
        items_created,
        matched.len(),
        items_unmatched,
    );

    transaction.commit()?;

    Ok(SourceIngestionResult {
        adapter_id: ADAPTER_ID.to_owned(),
        items_fetched: entries.len(),
        items_created,
        items_matched: matched.len(),
        items_unmatched,
        detail_items_attempted: 0,
        detail_items_stored: 0,
        detail_items_failed: 0,
        fetched_at: Some(fetched_at),
    })
}

/// Decide the change kind for a matched entry against its stored row. `None` means
/// "no change" (new value equals the stored live value). A re-entry after an exit
/// is an `entered`.
fn classify_change(
    stored: Option<&StoredPosition>,
    new_pct: f64,
) -> Option<(&'static str, Option<f64>)> {
    match stored {
        None => Some(("entered", None)),
        Some(row) if row.exited => Some(("entered", None)),
        Some(row) if new_pct > row.net_position_pct + PCT_EPSILON => {
            Some(("increased", Some(row.net_position_pct)))
        }
        Some(row) if new_pct < row.net_position_pct - PCT_EPSILON => {
            Some(("decreased", Some(row.net_position_pct)))
        }
        Some(_) => None,
    }
}

fn load_stored_position(
    connection: &Connection,
    company_id: &str,
    holder_name: &str,
) -> StorageResult<Option<StoredPosition>> {
    connection
        .query_row(
            "
            SELECT company_id, holder_name, net_position_pct, exited_at
            FROM short_positions
            WHERE company_id = ?1 AND holder_name = ?2
            ",
            params![company_id, holder_name],
            |row| {
                Ok(StoredPosition {
                    company_id: row.get(0)?,
                    holder_name: row.get(1)?,
                    net_position_pct: row.get(2)?,
                    exited: row.get::<_, Option<String>>(3)?.is_some(),
                })
            },
        )
        .optional()
        .map_err(StorageError::from)
}

fn load_live_positions(connection: &Connection) -> StorageResult<Vec<StoredPosition>> {
    let mut statement = connection.prepare(
        "
        SELECT company_id, holder_name, net_position_pct
        FROM short_positions
        WHERE exited_at IS NULL
        ",
    )?;
    let rows = statement.query_map([], |row| {
        Ok(StoredPosition {
            company_id: row.get(0)?,
            holder_name: row.get(1)?,
            net_position_pct: row.get(2)?,
            exited: false,
        })
    })?;
    rows.collect::<Result<Vec<_>, _>>()
        .map_err(StorageError::from)
}

fn company_ticker(connection: &Connection, company_id: &str) -> StorageResult<String> {
    connection
        .query_row(
            "SELECT qualified_ticker FROM companies WHERE id = ?1",
            [company_id],
            |row| row.get::<_, String>(0),
        )
        .optional()
        .map(|value| value.unwrap_or_else(|| company_id.to_owned()))
        .map_err(StorageError::from)
}

/// Insert or update the current-state row for (company, holder), clearing any
/// prior `exited_at` (a re-entry becomes live again).
fn upsert_position(
    connection: &Connection,
    company_id: &str,
    entry: &KnfShortEntry,
) -> StorageResult<()> {
    let id = position_id(company_id, &entry.holder_name);
    connection.execute(
        "
        INSERT INTO short_positions (
            id, company_id, holder_name, isin, net_position_pct, position_date, modify_date, exited_at
        )
        VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, NULL)
        ON CONFLICT(id) DO UPDATE SET
            holder_name = excluded.holder_name,
            isin = excluded.isin,
            net_position_pct = excluded.net_position_pct,
            position_date = excluded.position_date,
            modify_date = excluded.modify_date,
            exited_at = NULL,
            updated_at = strftime('%Y-%m-%dT%H:%M:%fZ', 'now')
        ",
        params![
            id,
            company_id,
            entry.holder_name,
            entry.isin,
            entry.net_position_pct,
            entry.position_date,
            entry.modify_date,
        ],
    )?;
    Ok(())
}

fn mark_exited(
    connection: &Connection,
    company_id: &str,
    holder_name: &str,
    exited_at: &str,
) -> StorageResult<()> {
    connection.execute(
        "
        UPDATE short_positions
        SET exited_at = ?3,
            updated_at = strftime('%Y-%m-%dT%H:%M:%fZ', 'now')
        WHERE company_id = ?1 AND holder_name = ?2
        ",
        params![company_id, holder_name, exited_at],
    )?;
    Ok(())
}

/// Persist one change: append the history event, create the feed item and the
/// typed signal, and fire matching alert rules. Idempotent on identity keys.
#[allow(clippy::too_many_arguments)]
fn record_event(
    connection: &Connection,
    company_id: &str,
    qualified_ticker: &str,
    holder_name: &str,
    kind: &str,
    from_pct: Option<f64>,
    to_pct: Option<f64>,
    position_date: &str,
    fetched_at: &str,
) -> StorageResult<()> {
    let dedupe_key = event_dedupe_key(company_id, holder_name, kind, position_date, to_pct);

    // Append-only history row (deterministic id => idempotent on re-runs).
    connection.execute(
        "
        INSERT OR IGNORE INTO short_position_events (
            id, company_id, holder_name, kind, from_pct, to_pct, position_date
        )
        VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7)
        ",
        params![
            event_id(&dedupe_key),
            company_id,
            holder_name,
            kind,
            from_pct,
            to_pct,
            position_date,
        ],
    )?;

    // Feed item.
    let feed_item_id = super::feed_item_id(&dedupe_key);
    let source_url = format!(
        "{}#{}",
        crate::source_adapters::knf_short_selling::SOURCE_URL,
        dedupe_key
    );
    let title = event_title(kind, holder_name, qualified_ticker, from_pct, to_pct);
    super::ingestion::upsert_feed_item(
        connection,
        &super::ingestion::NormalizedFeedItem {
            id: &feed_item_id,
            item_type: FEED_ITEM_TYPE,
            source_adapter_id: ADAPTER_ID,
            source_name: DISPLAY_NAME,
            source_url: &source_url,
            title: &title,
            summary: None,
            body_text: None,
            language: "pl",
            published_at: Some(position_date),
            fetched_at,
            dedupe_key: &dedupe_key,
            attribution: ATTRIBUTION,
            display_company: qualified_ticker,
            duplicate_signature: None,
        },
    )?;

    connection.execute(
        "
        INSERT OR IGNORE INTO feed_item_companies (feed_item_id, company_id, match_type)
        VALUES (?1, ?2, 'isin')
        ",
        params![feed_item_id, company_id],
    )?;

    // Typed signal + alert-rule evaluation (same path as the ESPI classifier).
    store_signal(connection, &feed_item_id, company_id, position_date)?;
    Ok(())
}

/// Write the `short_position_change` signal for a change's feed item and fire any
/// matching `signal_category` alert rules (ADR 0068). Idempotent on
/// `(feed_item_id, category)`. Best-effort alert firing — a failure is logged and
/// never rolls back the signal (mirrors [`super::signals`]).
fn store_signal(
    connection: &Connection,
    feed_item_id: &str,
    company_id: &str,
    signal_date: &str,
) -> StorageResult<()> {
    let id = super::signals::signal_id(feed_item_id, SIGNAL_CATEGORY);
    let normalized_date: String = signal_date.chars().take(10).collect();
    let affected = connection.execute(
        "
        INSERT INTO company_signals (
            id, company_id, feed_item_id, category, confidence,
            classified_by, status, signal_date
        )
        VALUES (?1, ?2, ?3, ?4, 1.0, 'rule', 'confirmed', ?5)
        ON CONFLICT(feed_item_id, category) DO NOTHING
        ",
        params![
            id,
            company_id,
            feed_item_id,
            SIGNAL_CATEGORY,
            normalized_date
        ],
    )?;

    if affected > 0 {
        if let Err(error) = super::attention::evaluate_signal_rules(
            connection,
            company_id,
            SIGNAL_CATEGORY,
            &id,
            Some(&normalized_date),
        ) {
            log::warn!("module=short_positions stage=signal_eval signalId={id} error={error}");
        }
    }
    Ok(())
}

fn position_id(company_id: &str, holder_name: &str) -> String {
    format!(
        "shortpos_{}_{}",
        super::slug_part(company_id),
        super::slug_part(holder_name)
    )
}

fn event_id(dedupe_key: &str) -> String {
    format!("shortevt_{}", super::slug_part(dedupe_key))
}

fn event_dedupe_key(
    company_id: &str,
    holder_name: &str,
    kind: &str,
    position_date: &str,
    to_pct: Option<f64>,
) -> String {
    format!(
        "knf-short:{}:{}:{}:{}:{}",
        company_id,
        holder_name,
        kind,
        position_date,
        to_pct.map(format_pct_value).unwrap_or_default()
    )
}

fn event_title(
    kind: &str,
    holder_name: &str,
    qualified_ticker: &str,
    from_pct: Option<f64>,
    to_pct: Option<f64>,
) -> String {
    let ticker = qualified_ticker;
    match kind {
        "entered" => format!(
            "Pozycja krótka: {holder_name} — nowa {} ({ticker})",
            fmt_pct(to_pct)
        ),
        "increased" => format!(
            "Pozycja krótka: {holder_name} {} → {} ({ticker})",
            fmt_pct(from_pct),
            fmt_pct(to_pct)
        ),
        "decreased" => format!(
            "Pozycja krótka: {holder_name} {} → {} ({ticker})",
            fmt_pct(from_pct),
            fmt_pct(to_pct)
        ),
        "exited" => format!(
            "Pozycja krótka: {holder_name} {} → zamknięta ({ticker})",
            fmt_pct(from_pct)
        ),
        _ => format!("Pozycja krótka: {holder_name} ({ticker})"),
    }
}

/// Render a percent for display: Polish decimal comma, two places, `%` suffix.
fn fmt_pct(pct: Option<f64>) -> String {
    match pct {
        Some(value) => format!("{}%", format_pct_value(value).replace('.', ",")),
        None => "—".to_owned(),
    }
}

/// Stable numeric rendering of a percent for identity keys (two places, `.`).
fn format_pct_value(value: f64) -> String {
    format!("{value:.2}")
}

/// Load the per-company short-selling view (v0.55 T4b). `today` (`YYYY-MM-DD`)
/// anchors the 30-day window — the store passes the real UTC date; tests pass a
/// fixed date for determinism.
pub(super) fn load_short_positions_view(
    connection: &Connection,
    company_id: &str,
    today: &str,
) -> StorageResult<ShortPositionsView> {
    let cutoff = date_minus_30_days(today);

    // Active positions (register mirror rows still in the register), largest first.
    let mut positions_stmt = connection.prepare(
        "
        SELECT holder_name, net_position_pct, position_date
        FROM short_positions
        WHERE company_id = ?1 AND exited_at IS NULL
        ORDER BY net_position_pct DESC, holder_name ASC
        ",
    )?;
    let raw_positions: Vec<(String, f64, String)> = positions_stmt
        .query_map([company_id], |row| {
            Ok((row.get(0)?, row.get(1)?, row.get(2)?))
        })?
        .collect::<Result<Vec<_>, _>>()?;

    // Holders with a register change dated within the last 30 days -> "zmiana" chip.
    let mut changed_stmt = connection.prepare(
        "
        SELECT DISTINCT holder_name
        FROM short_position_events
        WHERE company_id = ?1 AND position_date >= ?2
        ",
    )?;
    let recently_changed: HashSet<String> = changed_stmt
        .query_map(params![company_id, cutoff], |row| row.get::<_, String>(0))?
        .collect::<Result<HashSet<_>, _>>()?;

    let aggregate_pct = raw_positions.iter().map(|(_, pct, _)| *pct).sum();
    let positions = raw_positions
        .into_iter()
        .map(
            |(holder_name, net_position_pct, position_date)| ShortPositionRow {
                recently_changed: recently_changed.contains(&holder_name),
                holder_name,
                net_position_pct,
                position_date,
            },
        )
        .collect();

    // Change history, newest-first by the DOMAIN date (never created_at).
    let mut events_stmt = connection.prepare(
        "
        SELECT kind, holder_name, from_pct, to_pct, position_date
        FROM short_position_events
        WHERE company_id = ?1
        ORDER BY position_date DESC, id DESC
        LIMIT 50
        ",
    )?;
    let events = events_stmt
        .query_map([company_id], |row| {
            Ok(ShortPositionEventRow {
                kind: row.get(0)?,
                holder_name: row.get(1)?,
                from_pct: row.get(2)?,
                to_pct: row.get(3)?,
                position_date: row.get(4)?,
            })
        })?
        .collect::<Result<Vec<_>, _>>()?;

    // 30-day change (pp): signed sum of in-window event deltas (see the type doc).
    let mut delta_stmt = connection.prepare(
        "
        SELECT kind, from_pct, to_pct
        FROM short_position_events
        WHERE company_id = ?1 AND position_date >= ?2
        ",
    )?;
    let delta_30d_pp: f64 = delta_stmt
        .query_map(params![company_id, cutoff], |row| {
            let kind: String = row.get(0)?;
            let from_pct: Option<f64> = row.get(1)?;
            let to_pct: Option<f64> = row.get(2)?;
            Ok(signed_event_delta(&kind, from_pct, to_pct))
        })?
        .collect::<Result<Vec<f64>, _>>()?
        .into_iter()
        .sum();

    // Last remembered exit (empty-state "Ostatnia obecność"): most recent exit.
    let last_exit = connection
        .query_row(
            "
            SELECT holder_name, exited_at
            FROM short_positions
            WHERE company_id = ?1 AND exited_at IS NOT NULL
            ORDER BY exited_at DESC
            LIMIT 1
            ",
            [company_id],
            |row| {
                let holder_name: String = row.get(0)?;
                let exited_at: String = row.get(1)?;
                Ok(ShortPositionExit {
                    holder_name,
                    exited_on: exited_at.chars().take(10).collect(),
                })
            },
        )
        .optional()?;

    let register_updated_at: Option<String> = connection
        .query_row(
            "SELECT last_success_at FROM source_adapters WHERE id = ?1",
            [crate::source_adapters::knf_short_selling::ADAPTER_ID],
            |row| row.get(0),
        )
        .optional()?
        .flatten();

    Ok(ShortPositionsView {
        positions,
        events,
        last_exit,
        aggregate_pct,
        delta_30d_pp,
        register_updated_at,
    })
}

/// Signed contribution of a register-change event to the aggregate net short %.
fn signed_event_delta(kind: &str, from_pct: Option<f64>, to_pct: Option<f64>) -> f64 {
    match kind {
        "entered" => to_pct.unwrap_or(0.0),
        "increased" | "decreased" => to_pct.unwrap_or(0.0) - from_pct.unwrap_or(0.0),
        "exited" => -from_pct.unwrap_or(0.0),
        _ => 0.0,
    }
}

/// `today` (`YYYY-MM-DD`) minus 30 days, same format. Falls back to `today`
/// unchanged if it is not a parseable date (a malformed cutoff would only widen
/// the window, never crash a read).
fn date_minus_30_days(today: &str) -> String {
    let fmt = time::macros::format_description!("[year]-[month]-[day]");
    match time::Date::parse(today, &fmt) {
        Ok(date) => date
            .saturating_sub(time::Duration::days(30))
            .format(&fmt)
            .unwrap_or_else(|_| today.to_owned()),
        Err(_) => today.to_owned(),
    }
}

fn now_iso() -> String {
    time::OffsetDateTime::now_utc()
        .format(&time::format_description::well_known::Rfc3339)
        .unwrap_or_else(|_| "1970-01-01T00:00:00Z".to_owned())
}

/// Short-selling domain store (Architecture v2 / ADR 0050). Reached via
/// `AppState::short_positions()`.
#[derive(Clone)]
pub struct ShortPositionStore {
    db: Database,
}

impl ShortPositionStore {
    pub(super) fn new(db: Database) -> Self {
        Self { db }
    }

    /// Diff + persist a parsed KNF register snapshot.
    pub fn ingest_knf_short_positions(
        &self,
        entries: &[KnfShortEntry],
    ) -> StorageResult<SourceIngestionResult> {
        let mut connection = self.db.checkout()?;

        ingest_knf_short_positions(&mut connection, entries)
    }

    /// The per-company cockpit read model (v0.55 T4b). The 30-day window is
    /// anchored to the real UTC date.
    pub fn short_positions_view(&self, company_id: &str) -> StorageResult<ShortPositionsView> {
        let connection = self.db.checkout()?;
        let today: String = now_iso().chars().take(10).collect();
        load_short_positions_view(&connection, company_id, &today)
    }
}
