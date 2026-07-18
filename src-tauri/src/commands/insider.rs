//! Insider overview read model (ADR 0083 Decision 7, plan v0.57 T6): the parsed
//! MAR art. 19 transaction timeline + latest management holdings + rolling
//! count-based net-direction aggregates over a 90-day and a 12-month window.
//!
//! A computed read model (no stored projection), mirroring the short-positions
//! view: it loads the parsed insider substrate ([`crate::storage::InsiderStore`] +
//! [`crate::storage::ManagementHoldingsStore`]) and folds it per window off the UI
//! thread via `spawn_blocking`. Decision support only — counts, volumes, and who,
//! never "bullish/bearish" language (ADR 0083 D7). Until T4b fills the attachment
//! PDFs, volumes/dates are known for only some transactions, so every aggregate
//! carries an explicit coverage note and directionless transactions land in a
//! separate `undetermined` bucket rather than being hidden.
//!
//! Window inclusivity (documented rule): a transaction counts toward a window when
//! its **effective date** (its `tx_date`, or the filing `signal_date` when the
//! cover note omitted the transaction date) is on or after the window's lower
//! bound **and** on or before the read date — **both boundaries inclusive**. So a
//! transaction dated *exactly* 90 days (resp. 12 months) before the read date is
//! IN the window; 91 days is out. A transaction with no effective date at all is
//! listed in the timeline but excluded from every window aggregate (never guessed
//! into one).
//!
//! Minimum-2 rule (ADR 0083 D7): a window renders an aggregate ONLY with ≥ 2
//! in-window transactions. Below that the aggregate is the tagged
//! [`WindowAggregate::BelowMinimum`] state carrying just the count — the UI shows
//! the transactions but no net readout.

use std::collections::BTreeMap;
use std::str::FromStr;

use rust_decimal::Decimal;
use serde::Serialize;
use time::{Date, Duration, Month};

use crate::app_state::AppState;
use crate::storage::{InsiderOverviewSource, ManagementHoldingRow};

// ============================================================================
// DTOs (ts-exported)
// ============================================================================

/// Everything the Ownership-area "Insiderzy" block renders for one company.
#[derive(Debug, Clone, PartialEq, Serialize)]
#[cfg_attr(feature = "ts-export", derive(ts_rs::TS))]
#[cfg_attr(
    feature = "ts-export",
    ts(export, export_to = "../../src/api/generated/")
)]
#[serde(rename_all = "camelCase")]
pub struct InsiderOverview {
    pub company_id: String,
    /// Parsed transactions, newest effective-date first (the timeline).
    pub transactions: Vec<InsiderTransactionEntry>,
    /// Latest disclosed holding per management/supervisory person (newest first).
    pub holdings: Vec<ManagementHoldingEntry>,
    /// Rolling aggregate over the trailing 90 days (both boundaries inclusive).
    pub window90d: WindowAggregate,
    /// Rolling aggregate over the trailing 12 months (both boundaries inclusive).
    pub window12m: WindowAggregate,
}

/// One parsed transaction for the timeline. All figure fields are nullable — the
/// cover note omits volume/price/date for most filings (T4b fills them), and
/// nothing is ever fabricated.
#[derive(Debug, Clone, PartialEq, Serialize)]
#[cfg_attr(feature = "ts-export", derive(ts_rs::TS))]
#[cfg_attr(
    feature = "ts-export",
    ts(export, export_to = "../../src/api/generated/")
)]
#[serde(rename_all = "camelCase")]
pub struct InsiderTransactionEntry {
    pub id: String,
    /// Display name of the person filing (raw, as disclosed).
    pub person: String,
    /// `management | supervisory | closely_associated`, or null when unqualified.
    pub role: Option<String>,
    /// The anchoring PDMR for a `closely_associated` filing, when disclosed.
    pub related_pdmr: Option<String>,
    /// `buy | sell | other`, or null when the cover note omitted the direction.
    pub direction: Option<String>,
    /// `shares | subscription_warrants | other`, or null.
    pub instrument: Option<String>,
    pub volume: Option<String>,
    pub price: Option<String>,
    pub currency: Option<String>,
    /// The disclosed transaction date, when the cover note stated one.
    pub tx_date: Option<String>,
    /// The date used for window placement (`txDate`, else the filing signal date),
    /// or null when neither is known.
    pub effective_date: Option<String>,
    /// How `effectiveDate` was derived — `transaction | filing | unknown` — so the
    /// UI can label a filing-date fallback honestly.
    pub date_source: String,
    /// Provenance to the classified filing.
    pub feed_item_id: String,
    /// The filing's source URL (the timeline's "link to the filing"), when known.
    pub source_url: Option<String>,
}

/// One person's latest disclosed management holding.
#[derive(Debug, Clone, PartialEq, Serialize)]
#[cfg_attr(feature = "ts-export", derive(ts_rs::TS))]
#[cfg_attr(
    feature = "ts-export",
    ts(export, export_to = "../../src/api/generated/")
)]
#[serde(rename_all = "camelCase")]
pub struct ManagementHoldingEntry {
    pub person: String,
    /// `management | supervisory`, or null.
    pub role: Option<String>,
    /// Decimal-exact share count as disclosed; null when stated-but-unreadable or a
    /// `-`/`nd.` cell (never coerced to zero). An explicit `"0"` is a real zero.
    pub shares: Option<String>,
    /// The vehicle a founder holds through ("pośrednio poprzez …"), when disclosed.
    pub indirect_via: Option<String>,
    pub as_of: String,
}

/// A rolling window aggregate. A tagged union so an aggregate can never render
/// below the 2-transaction minimum (ADR 0083 D7): `belowMinimum` carries only the
/// count; `computed` carries the count-based net direction with volume-where-known.
#[derive(Debug, Clone, PartialEq, Serialize)]
#[cfg_attr(feature = "ts-export", derive(ts_rs::TS))]
#[cfg_attr(
    feature = "ts-export",
    ts(export, export_to = "../../src/api/generated/")
)]
#[serde(tag = "state", rename_all = "camelCase")]
pub enum WindowAggregate {
    /// Fewer than 2 in-window transactions: no aggregate, just the count.
    BelowMinimum { count: i64 },
    /// Count-based net direction (ADR 0083 D7 amendment: until T4b, counts are the
    /// honest aggregate; volumes are summed only where known).
    // Variant-level rename: the enum-level `rename_all` only camelCases the variant
    // tags, not the struct-variant fields (same reason `AltmanScore` renames its
    // `zScore` field explicitly).
    #[serde(rename_all = "camelCase")]
    Computed {
        /// Total in-window transactions (`= buys + sells + undetermined`).
        count: i64,
        buys: i64,
        sells: i64,
        /// Neither a buy nor a sell (direction NULL or `other`) — nothing hidden.
        undetermined: i64,
        /// `buys − sells` (directionless transactions never move the net).
        net: i64,
        /// Sum of buy volumes across in-window buys **with a known volume**; null
        /// when no in-window buy disclosed a volume.
        buy_volume: Option<String>,
        /// Sum of sell volumes across in-window sells with a known volume.
        sell_volume: Option<String>,
        /// Coverage note numerator: in-window transactions with a known volume.
        volume_known: i64,
        /// Coverage note denominator: in-window transactions (`= count`).
        volume_total: i64,
    },
}

// ============================================================================
// Pure window math (unit-tested below)
// ============================================================================

/// Which rolling window, for [`window_start`].
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum WindowKind {
    Days90,
    Months12,
}

/// The inclusive lower bound of a window anchored at `anchor` (the read date).
fn window_start(anchor: Date, kind: WindowKind) -> Date {
    match kind {
        WindowKind::Days90 => anchor.saturating_sub(Duration::days(90)),
        // 12 months = the same calendar date one year earlier; a Feb-29 anchor in a
        // non-leap prior year clamps to Feb 28 (never panics, never widens).
        WindowKind::Months12 => {
            let year = anchor.year() - 1;
            anchor.replace_year(year).unwrap_or_else(|_| {
                Date::from_calendar_date(year, Month::February, 28).unwrap_or(anchor)
            })
        }
    }
}

/// The direction/volume/date a transaction contributes to a window fold.
#[derive(Debug, Clone)]
struct AggRow {
    date: Option<Date>,
    /// Lowercased `direction` as stored (`buy | sell | other`), or None.
    direction: Option<String>,
    volume: Option<Decimal>,
}

/// Fold the in-window rows into a [`WindowAggregate`], applying the min-2 rule.
fn window_aggregate(rows: &[AggRow], anchor: Date, kind: WindowKind) -> WindowAggregate {
    let start = window_start(anchor, kind);
    let in_window: Vec<&AggRow> = rows
        .iter()
        .filter(|r| matches!(r.date, Some(d) if d >= start && d <= anchor))
        .collect();

    let count = in_window.len() as i64;
    if count < 2 {
        return WindowAggregate::BelowMinimum { count };
    }

    let mut buys = 0i64;
    let mut sells = 0i64;
    let mut undetermined = 0i64;
    let mut buy_volume: Option<Decimal> = None;
    let mut sell_volume: Option<Decimal> = None;
    let mut volume_known = 0i64;

    for row in &in_window {
        if row.volume.is_some() {
            volume_known += 1;
        }
        match row.direction.as_deref() {
            Some("buy") => {
                buys += 1;
                if let Some(v) = row.volume {
                    buy_volume = Some(buy_volume.unwrap_or(Decimal::ZERO) + v);
                }
            }
            Some("sell") => {
                sells += 1;
                if let Some(v) = row.volume {
                    sell_volume = Some(sell_volume.unwrap_or(Decimal::ZERO) + v);
                }
            }
            // direction NULL or `other`: counted, but never in the net.
            _ => undetermined += 1,
        }
    }

    WindowAggregate::Computed {
        count,
        buys,
        sells,
        undetermined,
        net: buys - sells,
        buy_volume: buy_volume.map(fmt_decimal),
        sell_volume: sell_volume.map(fmt_decimal),
        volume_known,
        volume_total: count,
    }
}

fn fmt_decimal(d: Decimal) -> String {
    d.normalize().to_string()
}

/// Parse an ISO `YYYY-MM-DD` (optionally with a time suffix) into a [`Date`].
fn parse_iso_date(raw: &str) -> Option<Date> {
    let fmt = time::macros::format_description!("[year]-[month]-[day]");
    let head: String = raw.chars().take(10).collect();
    Date::parse(&head, &fmt).ok()
}

// ============================================================================
// Assembly (storage rows → read model)
// ============================================================================

/// Build the read model from the raw substrate rows and an explicit read anchor.
/// Pure and deterministic — `pub(crate)` so the golden test can pin a full
/// overview against a fixed anchor.
pub(crate) fn build_overview(
    company_id: &str,
    anchor: Date,
    sources: Vec<InsiderOverviewSource>,
    holdings: Vec<ManagementHoldingRow>,
) -> InsiderOverview {
    // --- Transactions → timeline entries (with effective-date resolution) ---
    let mut entries: Vec<(Option<Date>, InsiderTransactionEntry)> = sources
        .into_iter()
        .map(|src| {
            let tx = src.tx;
            // tx_date wins; else the filing signal_date; else unknown.
            let (effective_date, date_source) = if tx.tx_date.is_some() {
                (tx.tx_date.clone(), "transaction")
            } else if src.signal_date.is_some() {
                (src.signal_date.clone(), "filing")
            } else {
                (None, "unknown")
            };
            let effective = effective_date.as_deref().and_then(parse_iso_date);
            let entry = InsiderTransactionEntry {
                id: tx.id,
                person: tx.person_name_raw,
                role: tx.role,
                related_pdmr: tx.related_pdmr_raw,
                direction: tx.direction,
                instrument: tx.instrument,
                volume: tx.volume,
                price: tx.price,
                currency: tx.currency,
                tx_date: tx.tx_date,
                effective_date,
                date_source: date_source.to_owned(),
                feed_item_id: tx.feed_item_id,
                source_url: src.source_url,
            };
            (effective, entry)
        })
        .collect();

    // Newest effective-date first; unknown-date entries sort last (stable id tie).
    entries.sort_by(|a, b| match (a.0, b.0) {
        (Some(x), Some(y)) => y.cmp(&x).then_with(|| b.1.id.cmp(&a.1.id)),
        (Some(_), None) => std::cmp::Ordering::Less,
        (None, Some(_)) => std::cmp::Ordering::Greater,
        (None, None) => b.1.id.cmp(&a.1.id),
    });

    // --- Window folds (share the parsed rows) ---
    let agg_rows: Vec<AggRow> = entries
        .iter()
        .map(|(date, entry)| AggRow {
            date: *date,
            direction: entry.direction.as_ref().map(|d| d.to_lowercase()),
            volume: entry
                .volume
                .as_deref()
                .and_then(|v| Decimal::from_str(v).ok()),
        })
        .collect();

    let window90d = window_aggregate(&agg_rows, anchor, WindowKind::Days90);
    let window12m = window_aggregate(&agg_rows, anchor, WindowKind::Months12);

    let transactions = entries.into_iter().map(|(_, entry)| entry).collect();

    InsiderOverview {
        company_id: company_id.to_owned(),
        transactions,
        holdings: latest_holdings(holdings),
        window90d,
        window12m,
    }
}

/// Latest disclosed holding per person (max `as_of`), newest first. Input already
/// excludes zero-aggregate sentinels (storage `list_by_company`).
fn latest_holdings(rows: Vec<ManagementHoldingRow>) -> Vec<ManagementHoldingEntry> {
    // Keep the newest as_of per normalized person.
    let mut latest: BTreeMap<String, ManagementHoldingRow> = BTreeMap::new();
    for row in rows {
        latest
            .entry(row.person_normalized.clone())
            .and_modify(|existing| {
                if row.as_of > existing.as_of {
                    *existing = row.clone();
                }
            })
            .or_insert(row);
    }
    let mut entries: Vec<ManagementHoldingEntry> = latest
        .into_values()
        .map(|row| ManagementHoldingEntry {
            person: row.person_name_raw,
            role: row.role,
            shares: row.shares,
            indirect_via: row.indirect_via_raw,
            as_of: row.as_of,
        })
        .collect();
    // Newest disclosure first, then by person for a stable order.
    entries.sort_by(|a, b| b.as_of.cmp(&a.as_of).then_with(|| a.person.cmp(&b.person)));
    entries
}

// ============================================================================
// Command
// ============================================================================

/// Read anchor: today (UTC). A parse failure would only ever come from a broken
/// clock; fall back to the epoch (an empty-window read), never a panic.
fn today_utc() -> Date {
    time::OffsetDateTime::now_utc().date()
}

/// Compute the insider overview for one company (sync core, unit-testable).
pub fn compute_insider_overview(
    state: &AppState,
    company_id: &str,
) -> Result<InsiderOverview, String> {
    let sources = state
        .insider()
        .list_for_overview(company_id)
        .map_err(|error| error.to_string())?;
    let holdings = state
        .management_holdings()
        .list_by_company(company_id)
        .map_err(|error| error.to_string())?;
    Ok(build_overview(company_id, today_utc(), sources, holdings))
}

/// Insider transaction timeline + management holdings + rolling net-direction
/// aggregates for the Ownership area (ADR 0083 D7). Offloaded off the UI thread.
#[tauri::command]
pub async fn get_insider_overview(
    company_id: String,
    state: tauri::State<'_, AppState>,
) -> Result<InsiderOverview, String> {
    let state = state.inner().clone();
    tauri::async_runtime::spawn_blocking(move || compute_insider_overview(&state, &company_id))
        .await
        .map_err(|error| format!("insider overview task failed: {error}"))?
}

#[cfg(test)]
mod tests;
