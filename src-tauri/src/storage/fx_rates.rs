//! FX-rate domain store (ADR 0089 dec. 2): the `fx_rates` NBP Table-A mid
//! series. Append-only; corrections upsert by `(currency, date)`, never a
//! wholesale history rewrite — the exact idiom `daily_quotes` uses. Mids are
//! stored decimal-exact as TEXT and read back into `rust_decimal::Decimal`.
//! Reach it via `AppState::fx_rates()`.

use rust_decimal::prelude::FromStr;
use rust_decimal::Decimal;

use super::database::Database;
use super::*;
use crate::fx::{self, FxRate};

#[derive(Clone)]
pub struct FxRatesStore {
    db: Database,
}

impl FxRatesStore {
    pub(super) fn new(db: Database) -> Self {
        Self { db }
    }

    /// Upsert a batch of mids. Idempotent: re-running identical rows changes
    /// nothing; a correction (same key, new mid) overwrites in place. Returns the
    /// number of rows written.
    pub fn upsert_rates(
        &self,
        rates: &[FxRate],
        source_adapter_id: &str,
        fetched_at: &str,
    ) -> StorageResult<usize> {
        let mut connection = self.db.checkout()?;
        let tx = connection.transaction_with_behavior(rusqlite::TransactionBehavior::Immediate)?;
        {
            let mut stmt = tx.prepare(
                "INSERT INTO fx_rates (currency, date, mid_rate, source_adapter_id, fetched_at)
                 VALUES (?1, ?2, ?3, ?4, ?5)
                 ON CONFLICT(currency, date) DO UPDATE SET
                    mid_rate = excluded.mid_rate,
                    source_adapter_id = excluded.source_adapter_id,
                    fetched_at = excluded.fetched_at",
            )?;
            for rate in rates {
                stmt.execute(params![
                    rate.currency,
                    rate.date,
                    rate.mid.to_string(),
                    source_adapter_id,
                    fetched_at,
                ])?;
            }
        }
        tx.commit()?;
        Ok(rates.len())
    }

    /// The most recent stored date for one currency (highest `date`), if any.
    pub fn latest_date_for(&self, currency: &str) -> StorageResult<Option<String>> {
        let connection = self.db.checkout()?;
        let date = connection
            .query_row(
                "SELECT date FROM fx_rates WHERE currency = ?1 ORDER BY date DESC LIMIT 1",
                [currency],
                |row| row.get::<_, String>(0),
            )
            .optional()?;
        Ok(date)
    }

    /// Every distinct currency code with at least one stored rate (ascending).
    pub fn distinct_currencies(&self) -> StorageResult<Vec<String>> {
        let connection = self.db.checkout()?;
        let mut stmt =
            connection.prepare("SELECT DISTINCT currency FROM fx_rates ORDER BY currency ASC")?;
        let rows = stmt
            .query_map([], |row| row.get::<_, String>(0))?
            .collect::<Result<Vec<_>, _>>()?;
        Ok(rows)
    }

    /// All `(date, mid)` for a currency within `[from, to]` inclusive, ascending.
    pub fn rates_in_range(
        &self,
        currency: &str,
        from: &str,
        to: &str,
    ) -> StorageResult<Vec<(String, Decimal)>> {
        let connection = self.db.checkout()?;
        let mut stmt = connection.prepare(
            "SELECT date, mid_rate FROM fx_rates
             WHERE currency = ?1 AND date >= ?2 AND date <= ?3
             ORDER BY date ASC",
        )?;
        let rows = stmt
            .query_map(params![currency, from, to], map_rate_row)?
            .collect::<Result<Vec<_>, _>>()?;
        Ok(rows)
    }

    /// The flow-KPI basis: the period-average mid for a currency over
    /// `[start, end]`. Reads the range and delegates to the pure
    /// [`fx::period_average_mid`]. `None` when no mid falls in the window.
    pub fn period_average_mid(
        &self,
        currency: &str,
        start: &str,
        end: &str,
    ) -> StorageResult<Option<Decimal>> {
        let rates = self.rates_in_range(currency, start, end)?;
        Ok(fx::period_average_mid(&rates, start, end))
    }

    /// The stock-KPI basis: the last mid `<= end` for a currency. `None` when
    /// every stored mid is after `end` (or the currency has no rows).
    pub fn latest_mid_on_or_before(
        &self,
        currency: &str,
        end: &str,
    ) -> StorageResult<Option<Decimal>> {
        let connection = self.db.checkout()?;
        let mid = connection
            .query_row(
                "SELECT mid_rate FROM fx_rates
                 WHERE currency = ?1 AND date <= ?2
                 ORDER BY date DESC LIMIT 1",
                params![currency, end],
                |row| row.get::<_, String>(0),
            )
            .optional()?;
        match mid {
            Some(text) => Ok(Some(parse_mid(&text)?)),
            None => Ok(None),
        }
    }

    /// Stamp a completed pull/backfill onto the adapter's runtime row
    /// (`last_success_at` + item counters) — the same outcome record every feed
    /// adapter writes (DoD §C: a refresh path must set `last_success_at`).
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

fn map_rate_row(row: &rusqlite::Row<'_>) -> rusqlite::Result<(String, Decimal)> {
    let date: String = row.get(0)?;
    let mid_text: String = row.get(1)?;
    let mid = Decimal::from_str(&mid_text).map_err(|error| {
        rusqlite::Error::FromSqlConversionFailure(
            1,
            rusqlite::types::Type::Text,
            format!("invalid stored mid {mid_text:?}: {error}").into(),
        )
    })?;
    Ok((date, mid))
}

fn parse_mid(text: &str) -> StorageResult<Decimal> {
    Decimal::from_str(text).map_err(|_| StorageError::InvalidSourceValue {
        key: "fx_mid_rate",
        value: text.to_owned(),
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::storage::migrations::open_in_memory_database;

    fn d(s: &str) -> Decimal {
        Decimal::from_str(s).expect("decimal")
    }

    fn rate(currency: &str, date: &str, mid: &str) -> FxRate {
        FxRate {
            currency: currency.to_owned(),
            date: date.to_owned(),
            mid: d(mid),
        }
    }

    #[test]
    fn upsert_is_idempotent_and_reads_back_decimal_exact() {
        let state = AppState::new(open_in_memory_database().expect("db"));
        let store = state.fx_rates();
        let rows = vec![
            rate("EUR", "2024-01-15", "4.3748"),
            rate("EUR", "2024-01-16", "4.4000"),
        ];
        store
            .upsert_rates(&rows, "nbp-fx", "2024-01-16T12:00:00Z")
            .expect("first upsert");
        // Idempotent re-run: no duplicate rows.
        store
            .upsert_rates(&rows, "nbp-fx", "2024-01-16T12:00:00Z")
            .expect("second upsert");

        let series = store
            .rates_in_range("EUR", "2000-01-01", "2100-01-01")
            .expect("read");
        assert_eq!(series.len(), 2);
        assert_eq!(series[0], ("2024-01-15".to_owned(), d("4.3748")));
    }

    #[test]
    fn correction_overwrites_in_place_by_key() {
        let state = AppState::new(open_in_memory_database().expect("db"));
        let store = state.fx_rates();
        store
            .upsert_rates(&[rate("USD", "2024-01-15", "3.9963")], "nbp-fx", "t1")
            .expect("insert");
        store
            .upsert_rates(&[rate("USD", "2024-01-15", "4.0100")], "nbp-fx", "t2")
            .expect("correction");
        let series = store
            .rates_in_range("USD", "2000-01-01", "2100-01-01")
            .expect("read");
        assert_eq!(series.len(), 1, "correction must not add a row");
        assert_eq!(series[0].1, d("4.0100"));
    }

    #[test]
    fn latest_date_and_distinct_currencies() {
        let state = AppState::new(open_in_memory_database().expect("db"));
        let store = state.fx_rates();
        store
            .upsert_rates(
                &[
                    rate("EUR", "2024-01-15", "4.37"),
                    rate("EUR", "2024-01-18", "4.40"),
                    rate("USD", "2024-01-15", "3.99"),
                ],
                "nbp-fx",
                "t",
            )
            .expect("insert");
        assert_eq!(
            store.latest_date_for("EUR").expect("q"),
            Some("2024-01-18".to_owned())
        );
        assert_eq!(store.latest_date_for("GBP").expect("q"), None);
        assert_eq!(
            store.distinct_currencies().expect("q"),
            vec!["EUR".to_owned(), "USD".to_owned()]
        );
    }

    #[test]
    fn period_average_and_stock_read_the_right_basis() {
        let state = AppState::new(open_in_memory_database().expect("db"));
        let store = state.fx_rates();
        store
            .upsert_rates(
                &[
                    rate("EUR", "2024-01-10", "4.00"),
                    rate("EUR", "2024-01-20", "4.50"),
                    rate("EUR", "2024-02-05", "9.99"), // outside Jan window
                ],
                "nbp-fx",
                "t",
            )
            .expect("insert");
        // Flow: average of the two January mids = 4.25.
        assert_eq!(
            store
                .period_average_mid("EUR", "2024-01-01", "2024-01-31")
                .expect("q"),
            Some(d("4.25"))
        );
        // Stock: last mid <= end-of-January = 4.50.
        assert_eq!(
            store
                .latest_mid_on_or_before("EUR", "2024-01-31")
                .expect("q"),
            Some(d("4.50"))
        );
        // Missing basis -> None (never a silent guess).
        assert_eq!(
            store
                .period_average_mid("EUR", "2023-01-01", "2023-12-31")
                .expect("q"),
            None
        );
    }

    #[test]
    fn record_pull_outcome_sets_last_success_at() {
        let state = AppState::new(open_in_memory_database().expect("db"));
        state
            .fx_rates()
            .record_pull_outcome("nbp-fx", "2024-01-16T12:00:00Z", 5, 5, 0)
            .expect("stamp");
        let adapter = state
            .list_source_adapters_with_developer(true)
            .expect("adapters")
            .into_iter()
            .find(|a| a.id == "nbp-fx")
            .expect("nbp-fx row exists");
        assert!(
            adapter.last_success_at.is_some(),
            "a pull stamps last_success_at so the Sources health reflects it"
        );
    }
}
