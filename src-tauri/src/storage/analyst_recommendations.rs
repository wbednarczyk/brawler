//! Analyst-recommendations storage (ADR 0073, plan v0.58 A1).
//!
//! Tracks sell-side recommendations (rating, target price, issuing firm) for
//! tracked GPW companies strictly as attributed third-party opinions — never as
//! advice. The source page carries no "rating before", so `direction`,
//! `rating_prev` and `target_prev` are DERIVED at ingest by comparing each entry
//! against the latest prior stored recommendation from the SAME firm (ordered by
//! `published_at`; no prior → `initiate`).
//!
//! Each NEWLY inserted recommendation emits one `feed_items` row + a
//! `recommendation_change` `company_signals` row and fires matching alert rules —
//! the same path as the KNF short-position ingester ([`super::short_positions`]).
//! Re-ingesting the same page is a no-op (ADR 0073 decision 4): the natural key
//! `(company, firm, published_at, rating, target)` dedupes rows and signals.

use rust_decimal::Decimal;
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use std::str::FromStr;

use super::database::Database;
use super::*;
use crate::source_adapters::biznesradar_recommendations::{ADAPTER_ID, ATTRIBUTION, DISPLAY_NAME};

/// The typed signal category emitted for new recommendations (seeded by migration
/// 0100; empty patterns, so the rule classifier never matches it by text — the
/// signal is written directly here).
const SIGNAL_CATEGORY: &str = "recommendation_change";
/// Feed-item display type, matching the other official-report sources.
const FEED_ITEM_TYPE: &str = "Official report";

/// A parsed recommendation for one company, as produced by the adapter (slice A2).
/// `direction`, `rating_prev` and `target_prev` are NOT part of the input — they
/// are derived at ingest.
#[derive(Debug, Clone, Deserialize)]
pub struct AnalystRecommendationEntry {
    /// Issuing brokerage/firm (e.g. "DM BOŚ"). Verbatim.
    pub firm: String,
    /// Analyst name, when the page lists one separately.
    pub analyst: Option<String>,
    /// Rating in the source vocabulary, verbatim (e.g. "akumuluj").
    pub rating: String,
    /// Target price as decimal-exact TEXT, or `None` when the page omits it.
    pub target_price: Option<String>,
    /// Target currency (e.g. "PLN"), when disclosed.
    pub target_currency: Option<String>,
    /// The share price at the report's issue date, decimal-exact TEXT.
    pub price_at_issue: Option<String>,
    /// Publication timestamp (ISO-8601).
    pub published_at: String,
    /// Canonical page URL for attribution.
    pub source_url: String,
    /// Broker PDF URL, when linked.
    pub report_url: Option<String>,
}

/// One stored recommendation, newest-first, for the cockpit panel (slice A3).
#[derive(Debug, Clone, Serialize)]
#[cfg_attr(feature = "ts-export", derive(ts_rs::TS))]
#[cfg_attr(
    feature = "ts-export",
    ts(export, export_to = "../../src/api/generated/")
)]
#[serde(rename_all = "camelCase")]
pub struct AnalystRecommendationRow {
    pub firm: String,
    pub analyst: Option<String>,
    /// Rating verbatim in the source vocabulary.
    pub rating: String,
    /// The same-firm prior rating this entry was compared against (`None` on an
    /// `initiate`).
    pub rating_prev: Option<String>,
    /// Coarse normalized direction: `upgrade | downgrade | initiate | reiterate`.
    pub direction: String,
    pub target_price: Option<String>,
    pub target_currency: Option<String>,
    pub target_prev: Option<String>,
    pub price_at_issue: Option<String>,
    pub published_at: String,
    pub report_url: Option<String>,
    pub source_url: String,
}

/// The newest recommendation that carries a target price, for the "vs target"
/// readout beside market data — always attributed by firm + date (ADR 0073 D4).
#[derive(Debug, Clone, Serialize)]
#[cfg_attr(feature = "ts-export", derive(ts_rs::TS))]
#[cfg_attr(
    feature = "ts-export",
    ts(export, export_to = "../../src/api/generated/")
)]
#[serde(rename_all = "camelCase")]
pub struct AnalystRecommendationTarget {
    pub firm: String,
    pub target_price: String,
    pub target_currency: Option<String>,
    pub published_at: String,
}

/// Coarse rating rank for the Polish sell-side vocabulary, higher = more bullish.
/// `None` for a rating outside the known vocabulary (case/whitespace-insensitive).
fn rating_rank(rating: &str) -> Option<i32> {
    match rating.trim().to_lowercase().as_str() {
        "kupuj" => Some(5),
        "akumuluj" => Some(4),
        "trzymaj" | "neutralnie" => Some(3),
        "redukuj" => Some(2),
        "sprzedaj" => Some(1),
        _ => None,
    }
}

/// Parse a decimal-exact TEXT figure for numeric comparison, tolerating a Polish
/// decimal comma. `None` when absent or unparseable (comparison then falls back
/// to the rating-only rule).
fn parse_decimal(value: Option<&str>) -> Option<Decimal> {
    let raw = value?.trim().replace(',', ".");
    if raw.is_empty() {
        return None;
    }
    Decimal::from_str(&raw).ok()
}

/// Derive `(direction, rating_prev, target_prev)` for a new entry against the
/// latest prior stored entry of the same firm.
///
/// Rules (documented so ingest stays deterministic):
/// - No prior → `initiate` (prev fields `None`).
/// - Known ratings on both sides → compare rank: higher → `upgrade`, lower →
///   `downgrade`, equal → `reiterate` unless the target moved (then follow the
///   target: up → `upgrade`, down → `downgrade`).
/// - Rating outside the known vocabulary on either side → fall back to the target
///   direction (up → `upgrade`, down → `downgrade`); when the target is unchanged
///   or unknown → `reiterate`.
fn derive_direction(
    prior: Option<&PriorRecommendation>,
    rating: &str,
    target_price: Option<&str>,
) -> (String, Option<String>, Option<String>) {
    let Some(prior) = prior else {
        return ("initiate".to_owned(), None, None);
    };
    let rating_prev = Some(prior.rating.clone());
    let target_prev = prior.target_price.clone();

    let target_dir = match (
        parse_decimal(prior.target_price.as_deref()),
        parse_decimal(target_price),
    ) {
        (Some(prev), Some(cur)) if cur > prev => Some(true), // target up
        (Some(prev), Some(cur)) if cur < prev => Some(false), // target down
        _ => None,
    };

    let direction = match (rating_rank(&prior.rating), rating_rank(rating)) {
        (Some(prev), Some(cur)) if cur > prev => "upgrade",
        (Some(prev), Some(cur)) if cur < prev => "downgrade",
        (Some(_), Some(_)) => match target_dir {
            Some(true) => "upgrade",
            Some(false) => "downgrade",
            None => "reiterate",
        },
        _ => match target_dir {
            Some(true) => "upgrade",
            Some(false) => "downgrade",
            None => "reiterate",
        },
    };

    (direction.to_owned(), rating_prev, target_prev)
}

/// The latest prior stored recommendation for a (company, firm), for derivation.
struct PriorRecommendation {
    rating: String,
    target_price: Option<String>,
}

/// The natural key string (ADR 0073 decision 4). Unit-separated so distinct
/// components never blur together; `''` for an absent target.
fn natural_key(company_id: &str, entry: &AnalystRecommendationEntry) -> String {
    format!(
        "analyst-rec:{}\u{1f}{}\u{1f}{}\u{1f}{}\u{1f}{}",
        company_id,
        entry.firm,
        entry.published_at,
        entry.rating,
        entry.target_price.as_deref().unwrap_or("")
    )
}

/// Collision-safe deterministic id from the natural key (sha256 hex, truncated).
fn recommendation_id(natural_key: &str) -> String {
    format!("analystrec_{}", &sha256_hex(natural_key)[..32])
}

fn feed_item_id_for(natural_key: &str) -> String {
    format!("feed_analystrec_{}", &sha256_hex(natural_key)[..32])
}

fn sha256_hex(value: &str) -> String {
    let digest = Sha256::digest(value.as_bytes());
    let mut out = String::with_capacity(digest.len() * 2);
    for byte in digest {
        out.push_str(&format!("{byte:02x}"));
    }
    out
}

fn now_iso() -> String {
    time::OffsetDateTime::now_utc()
        .format(&time::format_description::well_known::Rfc3339)
        .unwrap_or_else(|_| "1970-01-01T00:00:00Z".to_owned())
}

/// Ingest a company's parsed recommendations. Derives `direction`/`rating_prev`/
/// `target_prev`, inserts new rows (dedupe on the natural key), and emits one
/// feed item + `recommendation_change` signal per newly inserted row. Runs in one
/// immediate transaction so a page lands atomically. Records the run outcome so
/// the adapter's `last_success_at` is set.
pub(super) fn ingest_analyst_recommendations(
    connection: &mut Connection,
    company_id: &str,
    entries: &[AnalystRecommendationEntry],
) -> StorageResult<SourceIngestionResult> {
    let transaction =
        connection.transaction_with_behavior(rusqlite::TransactionBehavior::Immediate)?;
    let fetched_at = now_iso();

    // Process oldest-first so each entry derives against prior entries already
    // stored (including earlier entries in this same batch).
    let mut ordered: Vec<&AnalystRecommendationEntry> = entries.iter().collect();
    ordered.sort_by(|a, b| a.published_at.cmp(&b.published_at));

    let qualified_ticker = company_ticker(&transaction, company_id)?;
    let mut items_created = 0;

    for entry in ordered {
        let prior = latest_prior(&transaction, company_id, &entry.firm, &entry.published_at)?;
        let (direction, rating_prev, target_prev) =
            derive_direction(prior.as_ref(), &entry.rating, entry.target_price.as_deref());

        let key = natural_key(company_id, entry);
        let id = recommendation_id(&key);
        let inserted = transaction.execute(
            "
            INSERT INTO analyst_recommendations (
                id, company_id, firm, analyst, rating, rating_prev, direction,
                target_price, target_currency, target_prev, price_at_issue,
                published_at, source_url, report_url
            )
            VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11, ?12, ?13, ?14)
            ON CONFLICT(id) DO NOTHING
            ",
            params![
                id,
                company_id,
                entry.firm,
                entry.analyst,
                entry.rating,
                rating_prev,
                direction,
                entry.target_price,
                entry.target_currency,
                target_prev,
                entry.price_at_issue,
                entry.published_at,
                entry.source_url,
                entry.report_url,
            ],
        )?;

        if inserted == 0 {
            continue; // already stored — no feed item, no signal (idempotent)
        }
        items_created += 1;

        emit_feed_item_and_signal(
            &transaction,
            &key,
            company_id,
            &qualified_ticker,
            entry,
            &direction,
            rating_prev.as_deref(),
            &fetched_at,
        )?;
    }

    // Best-effort run-outcome so the catalog row reflects the successful pull.
    let _ = super::ingestion::record_source_outcome(
        &transaction,
        ADAPTER_ID,
        &fetched_at,
        entries.len(),
        items_created,
        entries.len(),
        0,
    );

    transaction.commit()?;

    Ok(SourceIngestionResult {
        adapter_id: ADAPTER_ID.to_owned(),
        items_fetched: entries.len(),
        items_created,
        items_matched: entries.len(),
        items_unmatched: 0,
        detail_items_attempted: 0,
        detail_items_stored: 0,
        detail_items_failed: 0,
        fetched_at: Some(fetched_at),
    })
}

/// The latest stored recommendation from the same firm published strictly before
/// this entry — the basis for direction/prev derivation.
fn latest_prior(
    connection: &Connection,
    company_id: &str,
    firm: &str,
    published_at: &str,
) -> StorageResult<Option<PriorRecommendation>> {
    connection
        .query_row(
            "
            SELECT rating, target_price
            FROM analyst_recommendations
            WHERE company_id = ?1 AND firm = ?2 AND published_at < ?3
            ORDER BY published_at DESC, created_at DESC
            LIMIT 1
            ",
            params![company_id, firm, published_at],
            |row| {
                Ok(PriorRecommendation {
                    rating: row.get(0)?,
                    target_price: row.get(1)?,
                })
            },
        )
        .optional()
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

/// Create the feed item + `feed_item_companies` link and the typed signal for a
/// newly inserted recommendation, then fire matching alert rules.
#[allow(clippy::too_many_arguments)]
fn emit_feed_item_and_signal(
    connection: &Connection,
    natural_key: &str,
    company_id: &str,
    qualified_ticker: &str,
    entry: &AnalystRecommendationEntry,
    direction: &str,
    rating_prev: Option<&str>,
    fetched_at: &str,
) -> StorageResult<()> {
    let feed_item_id = feed_item_id_for(natural_key);
    let title = feed_item_title(entry, qualified_ticker, direction, rating_prev);

    super::ingestion::upsert_feed_item(
        connection,
        &super::ingestion::NormalizedFeedItem {
            id: &feed_item_id,
            item_type: FEED_ITEM_TYPE,
            source_adapter_id: ADAPTER_ID,
            source_name: DISPLAY_NAME,
            source_url: &entry.source_url,
            title: &title,
            summary: None,
            body_text: None,
            language: "pl",
            published_at: Some(&entry.published_at),
            fetched_at,
            dedupe_key: natural_key,
            attribution: ATTRIBUTION,
            display_company: qualified_ticker,
            duplicate_signature: None,
        },
    )?;

    connection.execute(
        "
        INSERT OR IGNORE INTO feed_item_companies (feed_item_id, company_id, match_type)
        VALUES (?1, ?2, 'ticker')
        ",
        params![feed_item_id, company_id],
    )?;

    store_signal(connection, &feed_item_id, company_id, &entry.published_at)?;
    Ok(())
}

fn feed_item_title(
    entry: &AnalystRecommendationEntry,
    qualified_ticker: &str,
    direction: &str,
    rating_prev: Option<&str>,
) -> String {
    let verb = match direction {
        "upgrade" => "podwyższa",
        "downgrade" => "obniża",
        "initiate" => "rozpoczyna",
        _ => "podtrzymuje",
    };
    match rating_prev {
        Some(prev) if direction == "upgrade" || direction == "downgrade" => format!(
            "Rekomendacja: {} {verb} {} → {} ({qualified_ticker})",
            entry.firm, prev, entry.rating
        ),
        _ => format!(
            "Rekomendacja: {} {verb} {} ({qualified_ticker})",
            entry.firm, entry.rating
        ),
    }
}

/// Write the `recommendation_change` signal for a recommendation's feed item and
/// fire any matching `signal_category` alert rules (ADR 0068). Idempotent on
/// `(feed_item_id, category)`. Best-effort alert firing (mirrors
/// [`super::short_positions`]).
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
            log::warn!(
                "module=analyst_recommendations stage=signal_eval signalId={id} error={error}"
            );
        }
    }
    Ok(())
}

/// Per-company recommendation history, newest-first by `published_at` (never
/// `created_at`).
pub(super) fn list_analyst_recommendations(
    connection: &Connection,
    company_id: &str,
) -> StorageResult<Vec<AnalystRecommendationRow>> {
    let mut statement = connection.prepare(
        "
        SELECT firm, analyst, rating, rating_prev, direction, target_price,
               target_currency, target_prev, price_at_issue, published_at,
               report_url, source_url
        FROM analyst_recommendations
        WHERE company_id = ?1
        ORDER BY published_at DESC, created_at DESC
        ",
    )?;
    let rows = statement.query_map([company_id], |row| {
        Ok(AnalystRecommendationRow {
            firm: row.get(0)?,
            analyst: row.get(1)?,
            rating: row.get(2)?,
            rating_prev: row.get(3)?,
            direction: row.get(4)?,
            target_price: row.get(5)?,
            target_currency: row.get(6)?,
            target_prev: row.get(7)?,
            price_at_issue: row.get(8)?,
            published_at: row.get(9)?,
            report_url: row.get(10)?,
            source_url: row.get(11)?,
        })
    })?;
    rows.collect::<Result<Vec<_>, _>>()
        .map_err(StorageError::from)
}

/// The newest recommendation carrying a target price, for the attributed "vs
/// target" readout.
pub(super) fn latest_target(
    connection: &Connection,
    company_id: &str,
) -> StorageResult<Option<AnalystRecommendationTarget>> {
    connection
        .query_row(
            "
            SELECT firm, target_price, target_currency, published_at
            FROM analyst_recommendations
            WHERE company_id = ?1 AND target_price IS NOT NULL AND TRIM(target_price) <> ''
            ORDER BY published_at DESC, created_at DESC
            LIMIT 1
            ",
            [company_id],
            |row| {
                Ok(AnalystRecommendationTarget {
                    firm: row.get(0)?,
                    target_price: row.get(1)?,
                    target_currency: row.get(2)?,
                    published_at: row.get(3)?,
                })
            },
        )
        .optional()
        .map_err(StorageError::from)
}

/// The adapter's last successful refresh (`source_adapters.last_success_at`), for
/// the panel footer's honest "last refresh" line. `None` before the adapter has
/// ever run for this install (the fresh/empty state) — the footer then omits the
/// timestamp rather than faking one (ADR 0073, storyboard frame 2/4 footer).
pub(super) fn last_refreshed_at(connection: &Connection) -> StorageResult<Option<String>> {
    connection
        .query_row(
            "SELECT last_success_at FROM source_adapters WHERE id = ?1",
            [ADAPTER_ID],
            |row| row.get::<_, Option<String>>(0),
        )
        .optional()
        .map(Option::flatten)
        .map_err(StorageError::from)
}

/// Analyst-recommendations domain store (Architecture v2 / ADR 0050). Reached via
/// `AppState::analyst_recommendations()`.
#[derive(Clone)]
pub struct AnalystRecommendationStore {
    db: Database,
}

impl AnalystRecommendationStore {
    pub(super) fn new(db: Database) -> Self {
        Self { db }
    }

    /// Ingest a company's parsed recommendations (dedupe + derive + emit signals).
    pub fn ingest_analyst_recommendations(
        &self,
        company_id: &str,
        entries: &[AnalystRecommendationEntry],
    ) -> StorageResult<SourceIngestionResult> {
        let mut connection = self.db.checkout()?;
        ingest_analyst_recommendations(&mut connection, company_id, entries)
    }

    /// Per-company recommendation history, newest-first by `published_at`.
    pub fn list_analyst_recommendations(
        &self,
        company_id: &str,
    ) -> StorageResult<Vec<AnalystRecommendationRow>> {
        let connection = self.db.checkout()?;
        list_analyst_recommendations(&connection, company_id)
    }

    /// The newest recommendation carrying a target price.
    pub fn latest_target(
        &self,
        company_id: &str,
    ) -> StorageResult<Option<AnalystRecommendationTarget>> {
        let connection = self.db.checkout()?;
        latest_target(&connection, company_id)
    }

    /// The adapter's last successful refresh timestamp, for the panel footer.
    pub fn last_refreshed_at(&self) -> StorageResult<Option<String>> {
        let connection = self.db.checkout()?;
        last_refreshed_at(&connection)
    }
}
