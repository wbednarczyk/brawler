//! Aggregator fundamentals page cache (ADR 0085 → ADR 0086).
//!
//! ADR 0085's witness fetched ONE page per tracked company per day (the income
//! statement) and cached it one row per company. ADR 0086 promotes BiznesRadar
//! to the PRIMARY core-KPI source and fetches THREE robots-allowed pages per
//! company per day — income, balance, cash flow — so the cache is keyed by
//! `(company_id, page_kind)` in `fundamentals_aggregator_pages` (migration 0110).
//!
//! One row per (company, page kind), upserted in place — a cache, not history. A
//! failed or uncovered attempt is cached too (`status` says which), so a dead
//! slug or a flaky host costs one request per page per day rather than one per
//! document. Reads are tolerant: a missing row simply means "a fetch is due".
//!
//! The per-kind API (`get_fresh_kind`/`put_kind`) serves the three-page primary
//! pull; the free `get_fresh_witness_page` (income kind, raw `&Connection`)
//! serves the ingest-time cover-note seam. The pre-0110 single-page wrappers
//! were removed with the review sweep (2026-07-22) — nothing called them.
//!
//! Reach the store via `AppState::fundamentals_witness_cache()`.

use super::database::Database;
use super::*;

/// Why a cached aggregator attempt landed where it did. Cached alongside the body
/// so a degraded attempt (`NoCoverage`/`FetchFailed`) can never be replayed as
/// agreement — ADR 0085 decision 5.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum WitnessPageStatus {
    /// The page was fetched and carries a financial report table.
    Ok,
    /// The host served the generic landing (unresolvable slug / no coverage).
    NoCoverage,
    /// The request failed (network, timeout, non-2xx).
    FetchFailed,
}

impl WitnessPageStatus {
    pub fn as_str(self) -> &'static str {
        match self {
            WitnessPageStatus::Ok => "ok",
            WitnessPageStatus::NoCoverage => "no_coverage",
            WitnessPageStatus::FetchFailed => "fetch_failed",
        }
    }

    fn from_str(value: &str) -> Option<Self> {
        match value {
            "ok" => Some(WitnessPageStatus::Ok),
            "no_coverage" => Some(WitnessPageStatus::NoCoverage),
            "fetch_failed" => Some(WitnessPageStatus::FetchFailed),
            _ => None,
        }
    }
}

/// Which of the three robots-allowed BiznesRadar report pages a cache row holds
/// (ADR 0086 decision 2). The `as_str` values are the `page_kind` CHECK vocabulary
/// of `fundamentals_aggregator_pages` (migration 0110).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum AggregatorPageKind {
    /// `raporty-finansowe-rachunek-zyskow-i-strat` — the income statement.
    Income,
    /// `raporty-finansowe-bilans` — the balance sheet.
    Balance,
    /// `raporty-finansowe-przeplywy-pieniezne` — the cash-flow statement.
    Cashflow,
}

impl AggregatorPageKind {
    pub fn as_str(self) -> &'static str {
        match self {
            AggregatorPageKind::Income => "income",
            AggregatorPageKind::Balance => "balance",
            AggregatorPageKind::Cashflow => "cashflow",
        }
    }

    /// The three kinds, in fetch order.
    pub const ALL: [AggregatorPageKind; 3] = [
        AggregatorPageKind::Income,
        AggregatorPageKind::Balance,
        AggregatorPageKind::Cashflow,
    ];
}

/// One cached aggregator attempt for a (company, page kind).
#[derive(Debug, Clone)]
pub struct CachedWitnessPage {
    pub page_url: String,
    pub html: Option<String>,
    pub status: WitnessPageStatus,
    pub fetched_at: String,
}

/// Connection-level fresh-cache read for the `income` page kind — the single SQL
/// body shared by the pooled [`FundamentalsWitnessStore::get_fresh`] and the
/// ingest-time cover-note witness path (which holds a raw `&Connection` post-
/// commit and must read the SAME cadence-windowed cache without ever fetching).
///
/// The window is evaluated in SQL against `now` so a clock read and the row
/// comparison cannot disagree, the same shape the source-scheduler due-check uses.
pub(crate) fn get_fresh_witness_page(
    connection: &rusqlite::Connection,
    company_id: &str,
    max_age_seconds: i64,
) -> StorageResult<Option<CachedWitnessPage>> {
    get_fresh_aggregator_page(
        connection,
        company_id,
        AggregatorPageKind::Income,
        max_age_seconds,
    )
}

/// Connection-level fresh-cache read for one (company, page kind). Tolerant: an
/// elapsed window or a missing row both yield `None` (a fetch is due); an
/// uninterpretable status marker is also read as "nothing cached" rather than
/// guessed into a verdict.
pub(crate) fn get_fresh_aggregator_page(
    connection: &rusqlite::Connection,
    company_id: &str,
    page_kind: AggregatorPageKind,
    max_age_seconds: i64,
) -> StorageResult<Option<CachedWitnessPage>> {
    let row = connection
        .query_row(
            "
            SELECT page_url, html, status, fetched_at
            FROM fundamentals_aggregator_pages
            WHERE company_id = ?1 AND page_kind = ?2
                AND ((julianday('now') - julianday(fetched_at)) * 86400.0) < ?3
            ",
            params![company_id, page_kind.as_str(), max_age_seconds],
            |row| {
                Ok((
                    row.get::<_, String>(0)?,
                    row.get::<_, Option<String>>(1)?,
                    row.get::<_, String>(2)?,
                    row.get::<_, String>(3)?,
                ))
            },
        )
        .optional()?;

    let Some((page_url, html, status, fetched_at)) = row else {
        return Ok(None);
    };
    Ok(
        WitnessPageStatus::from_str(status.as_str()).map(|status| CachedWitnessPage {
            page_url,
            html,
            status,
            fetched_at,
        }),
    )
}

/// Connection-level upsert for one (company, page kind). `html` is stored only
/// for [`WitnessPageStatus::Ok`].
pub(crate) fn put_aggregator_page(
    connection: &rusqlite::Connection,
    company_id: &str,
    page_kind: AggregatorPageKind,
    page_url: &str,
    status: WitnessPageStatus,
    html: Option<&str>,
) -> StorageResult<()> {
    let body = match status {
        WitnessPageStatus::Ok => html,
        _ => None,
    };
    connection.execute(
        "
        INSERT INTO fundamentals_aggregator_pages
            (company_id, page_kind, page_url, html, status, fetched_at)
        VALUES (?1, ?2, ?3, ?4, ?5, strftime('%Y-%m-%dT%H:%M:%fZ', 'now'))
        ON CONFLICT(company_id, page_kind) DO UPDATE SET
            page_url = excluded.page_url,
            html = excluded.html,
            status = excluded.status,
            fetched_at = excluded.fetched_at
        ",
        params![
            company_id,
            page_kind.as_str(),
            page_url,
            body,
            status.as_str()
        ],
    )?;
    Ok(())
}

pub struct FundamentalsWitnessStore {
    db: Database,
}

impl FundamentalsWitnessStore {
    pub(super) fn new(db: Database) -> Self {
        Self { db }
    }

    /// The cached attempt for one (company, page kind) inside the cadence window.
    pub fn get_fresh_kind(
        &self,
        company_id: &str,
        page_kind: AggregatorPageKind,
        max_age_seconds: i64,
    ) -> StorageResult<Option<CachedWitnessPage>> {
        let connection = self.db.checkout()?;
        get_fresh_aggregator_page(&connection, company_id, page_kind, max_age_seconds)
    }

    /// Record the outcome of one (company, page kind) attempt, replacing any
    /// previous row for that pair.
    pub fn put_kind(
        &self,
        company_id: &str,
        page_kind: AggregatorPageKind,
        page_url: &str,
        status: WitnessPageStatus,
        html: Option<&str>,
    ) -> StorageResult<()> {
        let connection = self.db.checkout()?;
        put_aggregator_page(&connection, company_id, page_kind, page_url, status, html)
    }
}
