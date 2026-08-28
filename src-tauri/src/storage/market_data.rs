//! Market-data domain store (Architecture v2 / ADR 0050): the `daily_quotes`
//! EOD price series (ADR 0067 / ADR 0082). Append-only; corrections upsert by
//! `(company_id, date)`, never a wholesale history rewrite. Reach it via
//! `AppState::market_data()`.

use super::database::Database;
use super::*;
use crate::source_adapters::yahoo_eod::DailyQuote;

/// One persisted EOD bar read back from `daily_quotes`.
#[derive(Debug, Clone, PartialEq)]
pub struct QuoteBar {
    pub date: String,
    pub open: f64,
    pub high: f64,
    pub low: f64,
    pub close: f64,
    pub volume: i64,
    pub fetched_at: String,
}

#[derive(Clone)]
pub struct MarketDataStore {
    db: Database,
}

impl MarketDataStore {
    pub(super) fn new(db: Database) -> Self {
        Self { db }
    }

    /// Upsert a batch of bars for one company. Returns the number of rows
    /// written. Idempotent: re-running with identical bars changes nothing;
    /// a correction (same key, new values) overwrites in place.
    pub fn upsert_quotes(
        &self,
        company_id: &str,
        quotes: &[DailyQuote],
        source_adapter_id: &str,
        fetched_at: &str,
    ) -> StorageResult<usize> {
        let mut connection = self.db.checkout()?;
        upsert_quotes(
            &mut connection,
            company_id,
            quotes,
            source_adapter_id,
            fetched_at,
        )
    }

    /// The most recent bar for a company (highest `date`), if any.
    pub fn latest_quote(&self, company_id: &str) -> StorageResult<Option<QuoteBar>> {
        let connection = self.db.checkout()?;
        latest_quote(&connection, company_id)
    }

    /// All bars for a company on or after `from_date` (inclusive), ascending —
    /// the 52-week / own-history percentile range scan (PK-indexed).
    pub fn quotes_since(&self, company_id: &str, from_date: &str) -> StorageResult<Vec<QuoteBar>> {
        let connection = self.db.checkout()?;
        quotes_since(&connection, company_id, from_date)
    }

    /// Stamp a completed pull/backfill sweep onto the adapter's mutable runtime
    /// row (`last_success_at` + per-run item counters) so the Sources UI shows
    /// the refresh — the same outcome record every feed adapter writes.
    pub fn record_pull_outcome(
        &self,
        adapter_id: &str,
        fetched_at: &str,
        items_fetched: usize,
        items_created: usize,
        items_unmatched: usize,
    ) -> StorageResult<()> {
        let connection = self.db.checkout()?;
        super::ingestion::record_source_outcome(
            &connection,
            adapter_id,
            fetched_at,
            items_fetched,
            items_created,
            items_created,
            items_unmatched,
        )
    }
}

fn upsert_quotes(
    connection: &mut Connection,
    company_id: &str,
    quotes: &[DailyQuote],
    source_adapter_id: &str,
    fetched_at: &str,
) -> StorageResult<usize> {
    let tx = connection.transaction_with_behavior(rusqlite::TransactionBehavior::Immediate)?;
    {
        let mut stmt = tx.prepare(
            "INSERT INTO daily_quotes
                (company_id, date, open, high, low, close, volume, source_adapter_id, fetched_at)
             VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9)
             ON CONFLICT(company_id, date) DO UPDATE SET
                open = excluded.open,
                high = excluded.high,
                low = excluded.low,
                close = excluded.close,
                volume = excluded.volume,
                source_adapter_id = excluded.source_adapter_id,
                fetched_at = excluded.fetched_at",
        )?;
        for q in quotes {
            stmt.execute((
                company_id,
                &q.date,
                q.open,
                q.high,
                q.low,
                q.close,
                q.volume,
                source_adapter_id,
                fetched_at,
            ))?;
        }
    }
    tx.commit()?;
    Ok(quotes.len())
}

/// The most recent stored bar for a company, callable with a borrowed
/// connection (used by inline attention-rule price evaluation, `storage::attention`).
pub(super) fn latest_quote_for(
    connection: &Connection,
    company_id: &str,
) -> StorageResult<Option<QuoteBar>> {
    latest_quote(connection, company_id)
}

fn latest_quote(connection: &Connection, company_id: &str) -> StorageResult<Option<QuoteBar>> {
    let bar = connection
        .query_row(
            "SELECT date, open, high, low, close, volume, fetched_at
             FROM daily_quotes WHERE company_id = ?1
             ORDER BY date DESC LIMIT 1",
            [company_id],
            map_quote_bar,
        )
        .optional()?;
    Ok(bar)
}

pub(super) fn quotes_since(
    connection: &Connection,
    company_id: &str,
    from_date: &str,
) -> StorageResult<Vec<QuoteBar>> {
    let mut stmt = connection.prepare(
        "SELECT date, open, high, low, close, volume, fetched_at
         FROM daily_quotes WHERE company_id = ?1 AND date >= ?2
         ORDER BY date ASC",
    )?;
    let rows = stmt
        .query_map((company_id, from_date), map_quote_bar)?
        .collect::<Result<Vec<_>, _>>()?;
    Ok(rows)
}

fn map_quote_bar(row: &rusqlite::Row<'_>) -> rusqlite::Result<QuoteBar> {
    Ok(QuoteBar {
        date: row.get(0)?,
        open: row.get(1)?,
        high: row.get(2)?,
        low: row.get(3)?,
        close: row.get(4)?,
        volume: row.get(5)?,
        fetched_at: row.get(6)?,
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::storage::migrations::open_in_memory_database;
    use crate::storage::AppState;

    fn bar(date: &str, close: f64) -> DailyQuote {
        DailyQuote {
            date: date.to_owned(),
            open: close - 1.0,
            high: close + 2.0,
            low: close - 2.0,
            close,
            volume: 1000,
        }
    }

    fn seed_company(state: &AppState) -> String {
        let company = state
            .create_company(NewCompany {
                exchange: "gpw".to_owned(),
                ticker: "cdr".to_owned(),
                display_name: "CD PROJEKT S.A.".to_owned(),
                isin: Some("PLOPTTC00011".to_owned()),
                cik: None,
                lei: None,
            })
            .expect("company should create");
        company.id
    }

    #[test]
    fn upsert_is_idempotent_and_reads_back_ascending() {
        let state = AppState::new(open_in_memory_database().expect("db"));
        let cid = seed_company(&state);
        let quotes = vec![bar("2026-07-13", 231.0), bar("2026-07-14", 233.0)];

        let store = state.market_data();
        store
            .upsert_quotes(&cid, &quotes, "yahoo-eod", "2026-07-14T18:00:00Z")
            .expect("first upsert");
        // Re-run identical: idempotent, no duplicate rows.
        store
            .upsert_quotes(&cid, &quotes, "yahoo-eod", "2026-07-14T18:00:00Z")
            .expect("second upsert");

        let series = store.quotes_since(&cid, "2000-01-01").expect("read");
        assert_eq!(series.len(), 2);
        assert_eq!(series[0].date, "2026-07-13");
        assert_eq!(series[1].date, "2026-07-14");
        assert_eq!(series[1].close, 233.0);
    }

    #[test]
    fn correction_overwrites_in_place_by_key() {
        let state = AppState::new(open_in_memory_database().expect("db"));
        let cid = seed_company(&state);
        let store = state.market_data();

        store
            .upsert_quotes(&cid, &[bar("2026-07-14", 233.0)], "yahoo-eod", "t1")
            .expect("insert");
        // Same (company, date), corrected close re-upserted by key.
        store
            .upsert_quotes(&cid, &[bar("2026-07-14", 231.5)], "yahoo-eod", "t2")
            .expect("correction");

        let series = store.quotes_since(&cid, "2000-01-01").expect("read");
        assert_eq!(series.len(), 1, "correction must not add a row");
        assert_eq!(series[0].close, 231.5);
    }

    #[test]
    fn latest_quote_returns_the_highest_date() {
        let state = AppState::new(open_in_memory_database().expect("db"));
        let cid = seed_company(&state);
        let store = state.market_data();
        store
            .upsert_quotes(
                &cid,
                &[
                    bar("2026-07-13", 231.0),
                    bar("2026-07-15", 240.0),
                    bar("2026-07-14", 233.0),
                ],
                "yahoo-eod",
                "t",
            )
            .expect("insert");

        let latest = store.latest_quote(&cid).expect("latest").expect("some");
        assert_eq!(latest.date, "2026-07-15");
        assert_eq!(latest.close, 240.0);

        assert!(store
            .latest_quote("company_gpw_missing")
            .expect("none")
            .is_none());
    }
}
