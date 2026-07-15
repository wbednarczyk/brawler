//! Full-history EOD quote backfill (ADR 0082, v0.53 T2).
//!
//! One-shot per company: full history (`period1=0`) from the primary
//! (`yahoo-eod`) source, run on company add / on the adapter being enabled.
//! Idempotent — `MarketDataStore::upsert_quotes` upserts by `(company_id,
//! date)`, so a re-run of the same history adds nothing new. Serializes on
//! the `yahoo-eod` source lock (ADR 0059) via [`serialization_key`], so
//! concurrent backfills never race the daily pull or each other for the same
//! source. `// migrates to Fetcher (ADR 0069)`.

use serde_json::json;

use crate::source_adapters::market_data_fetch::MarketDataFetcher;
use crate::source_adapters::yahoo_eod::parse_yahoo_chart;
use crate::storage::AppState;

/// Job kind: full-history quote backfill for one company. Payload
/// `{"companyId": ...}`.
pub const QUOTE_BACKFILL_KIND: &str = "quote_backfill";

/// The source-adapter id whose lock this job's kind serializes on (ADR 0059) —
/// shared with the daily pull so backfill and daily pull for the primary
/// source never run concurrently.
pub const YAHOO_ADAPTER_ID: &str = "yahoo-eod";

/// Enqueue a full-history backfill for one company (idempotent under the
/// stable per-company job id: a re-plan resets a still-pending row and never
/// disturbs one already running). Call on company add and on `yahoo-eod`
/// being enabled.
pub fn enqueue_backfill_for_company(state: &AppState, company_id: &str) {
    let job_id = format!("{QUOTE_BACKFILL_KIND}:{company_id}");
    let payload = json!({ "companyId": company_id }).to_string();
    if let Err(error) = state
        .jobs()
        .reschedule(&job_id, QUOTE_BACKFILL_KIND, &payload, 3)
    {
        log::warn!(
            "module=market_data stage=plan_failed job=quote_backfill companyId={company_id} error={error}"
        );
    }
}

/// Queue entry point: resolve `companyId` from the payload and run the
/// backfill with the live HTTP fetcher.
pub fn run_quote_backfill_job(state: &AppState, payload: &str) -> Result<(), String> {
    let fetcher = crate::source_adapters::market_data_fetch::HttpMarketDataFetcher;
    run_quote_backfill_job_with(state, payload, &fetcher)
}

/// Core backfill routine, generic over the fetcher so tests inject a
/// deterministic one (no network).
pub fn run_quote_backfill_job_with(
    state: &AppState,
    payload: &str,
    fetcher: &impl MarketDataFetcher,
) -> Result<(), String> {
    let parsed: serde_json::Value =
        serde_json::from_str(payload).map_err(|error| error.to_string())?;
    let company_id = parsed
        .get("companyId")
        .and_then(|value| value.as_str())
        .ok_or("quote backfill missing companyId")?;

    let company = state
        .companies()
        .list_companies()
        .map_err(|error| error.to_string())?
        .into_iter()
        .find(|company| company.id == company_id)
        .ok_or_else(|| format!("no tracked company for id {company_id}"))?;

    let ticker_wa = format!("{}.WA", company.ticker.to_uppercase());
    let json = fetcher
        .fetch_yahoo_full_history(&ticker_wa)
        .map_err(|error| error.to_string())?;
    let series = parse_yahoo_chart(&json).map_err(|error| error.to_string())?;

    let fetched_at = now_rfc3339();
    state
        .market_data()
        .upsert_quotes(company_id, &series.quotes, YAHOO_ADAPTER_ID, &fetched_at)
        .map_err(|error| error.to_string())?;

    // Mark the deep history as fetched so the daily pull's self-heal stops
    // re-planning this company (owner report 2026-07-14: companies tracked
    // before v0.53 had a FRESH latest bar but no deep history, which the
    // gap-only trigger never caught).
    state
        .record_source_adapter_state(
            YAHOO_ADAPTER_ID,
            &backfill_done_key(company_id),
            &fetched_at,
        )
        .map_err(|error| error.to_string())?;

    Ok(())
}

/// Adapter-state key marking one company's full-history backfill as completed.
pub fn backfill_done_key(company_id: &str) -> String {
    format!("backfill_done:{company_id}")
}

fn now_rfc3339() -> String {
    time::OffsetDateTime::now_utc()
        .format(&time::format_description::well_known::Rfc3339)
        .unwrap_or_else(|_| "1970-01-01T00:00:00Z".to_owned())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::source_adapters::market_data_fetch::MarketDataFetchError;
    use crate::storage::{open_in_memory_database, NewCompany};

    /// Deterministic fetcher: returns a fixed 3-bar Yahoo chart fixture for
    /// full history and panics if the daily-recent or Twelve Data paths are
    /// ever called (the backfill path must not touch them).
    struct FakeFullHistoryFetcher {
        body: String,
    }

    impl MarketDataFetcher for FakeFullHistoryFetcher {
        fn fetch_yahoo_full_history(
            &self,
            _ticker_wa: &str,
        ) -> Result<String, MarketDataFetchError> {
            Ok(self.body.clone())
        }
        fn fetch_yahoo_recent(&self, _ticker_wa: &str) -> Result<String, MarketDataFetchError> {
            panic!("quote_backfill must not call fetch_yahoo_recent");
        }
    }

    fn yahoo_fixture_json() -> String {
        json!({
            "chart": {
                "result": [{
                    "meta": { "symbol": "CDR.WA", "currency": "PLN" },
                    "timestamp": [1_752_400_000_i64, 1_752_486_400_i64, 1_752_572_800_i64],
                    "indicators": {
                        "quote": [{
                            "open": [230.0, 231.0, 232.0],
                            "high": [233.0, 234.0, 235.0],
                            "low": [229.0, 230.0, 231.0],
                            "close": [231.0, 233.0, 234.0],
                            "volume": [1000, 1100, 1200]
                        }]
                    }
                }],
                "error": null
            }
        })
        .to_string()
    }

    fn seed_company(state: &AppState) -> String {
        state
            .create_company(NewCompany {
                exchange: "GPW".to_owned(),
                ticker: "CDR".to_owned(),
                display_name: "CD PROJEKT S.A.".to_owned(),
                isin: Some("PLOPTTC00011".to_owned()),
                cik: None,
                lei: None,
            })
            .expect("company should create")
            .id
    }

    #[test]
    fn backfill_inserts_bars_from_fixture_then_rerun_adds_nothing() {
        let state = AppState::new(open_in_memory_database().expect("db"));
        let company_id = seed_company(&state);
        let fetcher = FakeFullHistoryFetcher {
            body: yahoo_fixture_json(),
        };
        let payload = json!({ "companyId": company_id }).to_string();

        run_quote_backfill_job_with(&state, &payload, &fetcher).expect("first backfill");
        let series = state
            .market_data()
            .quotes_since(&company_id, "2000-01-01")
            .expect("read back");
        assert_eq!(series.len(), 3, "all 3 fixture bars land");
        assert!(
            state
                .get_source_adapter_state(YAHOO_ADAPTER_ID, &backfill_done_key(&company_id))
                .expect("state read")
                .is_some(),
            "a successful backfill records the backfill_done marker"
        );

        // Idempotent re-run: same bars, same key -> row count unchanged.
        run_quote_backfill_job_with(&state, &payload, &fetcher).expect("second backfill");
        let series_after_rerun = state
            .market_data()
            .quotes_since(&company_id, "2000-01-01")
            .expect("read back again");
        assert_eq!(
            series_after_rerun.len(),
            3,
            "re-running the backfill adds no new rows (upsert by company_id+date)"
        );
    }

    #[test]
    fn enqueue_backfill_for_company_is_idempotent_under_a_stable_job_id() {
        let state = AppState::new(open_in_memory_database().expect("db"));
        let company_id = seed_company(&state);

        enqueue_backfill_for_company(&state, &company_id);
        enqueue_backfill_for_company(&state, &company_id);

        let counts = state.jobs().counts().expect("counts");
        assert_eq!(
            counts.pending, 1,
            "re-planning the same company's backfill does not accumulate duplicate rows"
        );
    }
}
