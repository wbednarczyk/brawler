//! NBP FX daily pull + full-history backfill (ADR 0089 dec. 2).
//!
//! A durable-queue job on the **market-data lane** (the `sources` lane, ADR 0059
//! — both are small, latency-tolerant external pulls; deliberate lane share),
//! serializing on the `nbp-fx` source lock so it never races itself. One code
//! path serves both modes: it fetches every needed currency's mids from the
//! earliest date any of them is missing through today, chunked in <=90-day
//! windows (NBP's 93-day range limit), upserts them into `fx_rates`, and stamps
//! the adapter's `last_success_at` (DoD §C).
//!
//! - **Backfill on first need**: a currency with no rows drags the fetch start
//!   back to [`NBP_HISTORY_START`], so its full available history lands the first
//!   time it is requested ([`ensure_fx_backfilled`]).
//! - **Daily recent**: when every needed currency is already current, the start
//!   is the latest stored date, so a steady-state pull is a single small window.
//!
//! Needed currencies are table-driven: the defaults ∪ any already in `fx_rates`
//! ∪ any named in the payload — never a hardcoded enum.

use std::collections::BTreeSet;

use serde_json::json;
use time::{format_description::well_known::Rfc3339, OffsetDateTime};

use crate::source_adapters::nbp_fx::{
    self, HttpNbpFxFetcher, NbpFetchError, NbpFxFetcher, ADAPTER_ID, NBP_HISTORY_START,
    NBP_MAX_RANGE_DAYS,
};
use crate::storage::AppState;

/// Job kind: NBP FX daily pull / full-history backfill. Payload optional
/// `{"currencies": ["USD", ...]}` — extra codes to include beyond the defaults
/// and the currencies already stored.
pub const FX_DAILY_PULL_KIND: &str = "fx_daily_pull";

/// Enqueue the recurring daily FX pull under a stable singleton id (idempotent:
/// a re-plan resets a still-pending row and never disturbs one already running).
/// Called by the Rust scheduler's daily cadence.
pub fn enqueue_fx_daily_pull(state: &AppState) {
    if let Err(error) = state
        .jobs()
        .reschedule(FX_DAILY_PULL_KIND, FX_DAILY_PULL_KIND, "{}", 3)
    {
        log::warn!("module=fx stage=plan_failed job=fx_daily_pull error={error}");
    }
}

/// Ensure a currency's history is present: if `fx_rates` has no row for it, plan
/// a backfill pull that includes it (idempotent per currency via a stable job
/// id). A no-op when the currency is already stored. The comparison read model
/// calls this on first need (ADR 0089 dec. 2).
pub fn ensure_fx_backfilled(state: &AppState, currency: &str) -> Result<(), String> {
    let code = currency.trim().to_uppercase();
    if code.is_empty() || code == crate::fx::PLN {
        return Ok(());
    }
    let already = state
        .fx_rates()
        .latest_date_for(&code)
        .map_err(|error| error.to_string())?
        .is_some();
    if already {
        return Ok(());
    }
    let job_id = format!("{FX_DAILY_PULL_KIND}:backfill:{code}");
    let payload = json!({ "currencies": [code] }).to_string();
    if let Err(error) = state
        .jobs()
        .reschedule(&job_id, FX_DAILY_PULL_KIND, &payload, 3)
    {
        log::warn!("module=fx stage=plan_failed job=fx_backfill currency={code} error={error}");
    }
    Ok(())
}

/// Queue entry point: run the pull with the live HTTP fetcher.
pub fn run_fx_daily_pull(state: &AppState, payload: &str) -> Result<(), String> {
    run_fx_pull_with(state, payload, &HttpNbpFxFetcher)
}

/// Core pull routine, generic over the fetcher (no network in tests).
pub fn run_fx_pull_with(
    state: &AppState,
    payload: &str,
    fetcher: &impl NbpFxFetcher,
) -> Result<(), String> {
    let store = state.fx_rates();

    // Table-driven needed set: defaults ∪ already-stored ∪ payload extras.
    let payload_extras = parse_payload_currencies(payload);
    let present = store
        .distinct_currencies()
        .map_err(|error| error.to_string())?;
    let extras: Vec<String> = payload_extras.into_iter().chain(present).collect();
    let needed = nbp_fx::currency_set(&extras);

    let end = today_utc();
    let start = earliest_needed_start(state, &needed)?;

    let mut rates = Vec::new();
    let mut skipped_windows = 0usize;
    for (chunk_start, chunk_end) in nbp_fx::date_chunks(&start, &end, NBP_MAX_RANGE_DAYS) {
        match fetcher.fetch_range(&chunk_start, &chunk_end) {
            Ok(body) => {
                let days = nbp_fx::parse_nbp_table_a(&body).map_err(|error| error.to_string())?;
                rates.extend(nbp_fx::extract_fx_rates(&days, &needed));
            }
            // 404 = no publication days in the window (weekend/holiday-only, or a
            // window entirely before history exists) — not a failure.
            Err(NbpFetchError::Status(404)) => skipped_windows += 1,
            Err(error) => {
                let message = format!("{chunk_start}..{chunk_end}: {error}");
                let _ = state.record_source_adapter_error(ADAPTER_ID, &message);
                return Err(message);
            }
        }
    }

    let fetched_at = now_rfc3339();
    let written = store
        .upsert_rates(&rates, ADAPTER_ID, &fetched_at)
        .map_err(|error| error.to_string())?;

    // Stamp the adapter runtime row so the Sources health reflects the pull
    // (DoD §C: a refresh path MUST set last_success_at).
    store
        .record_pull_outcome(ADAPTER_ID, &fetched_at, written, written, skipped_windows)
        .map_err(|error| error.to_string())?;

    Ok(())
}

/// The earliest date the pull must start from: the minimum over the needed
/// currencies of (its latest stored date, or [`NBP_HISTORY_START`] when it has
/// none). A currency missing entirely pulls the start back to history; when all
/// are current the start is the latest stored date (a small recent window).
fn earliest_needed_start(state: &AppState, needed: &BTreeSet<String>) -> Result<String, String> {
    let store = state.fx_rates();
    let mut start = today_utc();
    for currency in needed {
        let currency_start = store
            .latest_date_for(currency)
            .map_err(|error| error.to_string())?
            .unwrap_or_else(|| NBP_HISTORY_START.to_owned());
        if currency_start < start {
            start = currency_start;
        }
    }
    Ok(start)
}

fn parse_payload_currencies(payload: &str) -> Vec<String> {
    serde_json::from_str::<serde_json::Value>(payload)
        .ok()
        .and_then(|value| {
            value
                .get("currencies")
                .and_then(|c| c.as_array())
                .map(|arr| {
                    arr.iter()
                        .filter_map(|v| v.as_str().map(str::to_owned))
                        .collect()
                })
        })
        .unwrap_or_default()
}

fn today_utc() -> String {
    OffsetDateTime::now_utc()
        .format(&Rfc3339)
        .map(|s| s[..10].to_owned())
        .unwrap_or_else(|_| "1970-01-01".to_owned())
}

fn now_rfc3339() -> String {
    OffsetDateTime::now_utc()
        .format(&Rfc3339)
        .unwrap_or_else(|_| "1970-01-01T00:00:00Z".to_owned())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::fx::FxRate;
    use crate::jobs::handlers::pool_layout;
    use crate::source_adapters::nbp_fx::NbpParseError;
    use crate::storage::open_in_memory_database;
    use rust_decimal::prelude::FromStr;
    use rust_decimal::Decimal;
    use std::cell::RefCell;

    /// A trimmed NBP body for one day carrying the four default currencies.
    fn sample_body(date: &str) -> String {
        json!([{
            "table": "A",
            "no": "010/A/NBP/2024",
            "effectiveDate": date,
            "rates": [
                {"currency": "dolar amerykański", "code": "USD", "mid": 3.9963},
                {"currency": "euro", "code": "EUR", "mid": 4.3748},
                {"currency": "frank szwajcarski", "code": "CHF", "mid": 4.6797},
                {"currency": "funt szterling", "code": "GBP", "mid": 5.0861}
            ]
        }])
        .to_string()
    }

    /// Records every requested `[start, end]` window; returns the sample body for
    /// any window containing 2024-01-15, and an empty `[]` table otherwise.
    struct RecordingFetcher {
        windows: RefCell<Vec<(String, String)>>,
    }

    impl NbpFxFetcher for RecordingFetcher {
        fn fetch_range(&self, start: &str, end: &str) -> Result<String, NbpFetchError> {
            self.windows
                .borrow_mut()
                .push((start.to_owned(), end.to_owned()));
            if start <= "2024-01-15" && "2024-01-15" <= end {
                Ok(sample_body("2024-01-15"))
            } else {
                Ok("[]".to_owned())
            }
        }
    }

    fn empty_store_fetcher() -> RecordingFetcher {
        RecordingFetcher {
            windows: RefCell::new(Vec::new()),
        }
    }

    #[test]
    fn fx_daily_pull_is_on_the_market_data_sources_lane() {
        // ADR 0059 deliberate lane share: fx_daily_pull drains on the same lane as
        // the market-data (quote) jobs — the "sources" lane.
        let state = AppState::new(open_in_memory_database().expect("db"));
        let sources = pool_layout(state.queue_config())
            .into_iter()
            .find(|pool| pool.name == "sources")
            .expect("a sources lane exists");
        assert!(
            sources.kinds.contains(&FX_DAILY_PULL_KIND),
            "fx_daily_pull must be assigned to the market-data (sources) lane, got {:?}",
            sources.kinds
        );
    }

    #[test]
    fn empty_store_backfills_from_history_start_and_writes_needed_currencies() {
        let state = AppState::new(open_in_memory_database().expect("db"));
        let fetcher = empty_store_fetcher();

        run_fx_pull_with(&state, "{}", &fetcher).expect("pull");

        // First requested window starts at NBP history (full backfill on first need).
        let windows = fetcher.windows.borrow();
        assert_eq!(
            windows.first().map(|(s, _)| s.as_str()),
            Some(NBP_HISTORY_START),
            "an empty store backfills from history start, got {windows:?}"
        );
        assert!(
            windows.len() > 1,
            "a multi-decade span chunks into many windows"
        );

        // The default currencies for the sample date landed, decimal-exact.
        let store = state.fx_rates();
        assert_eq!(
            store
                .latest_mid_on_or_before("EUR", "2024-01-31")
                .expect("q"),
            Some(Decimal::from_str("4.3748").unwrap())
        );
        let mut present = store.distinct_currencies().expect("q");
        present.sort();
        assert_eq!(present, vec!["CHF", "EUR", "GBP", "USD"]);
    }

    #[test]
    fn recent_pull_starts_from_latest_stored_not_history() {
        let state = AppState::new(open_in_memory_database().expect("db"));
        // Seed every default currency current as of a recent date.
        let seed: Vec<FxRate> = ["CHF", "EUR", "GBP", "USD"]
            .iter()
            .map(|c| FxRate {
                currency: (*c).to_owned(),
                date: "2026-07-20".to_owned(),
                mid: Decimal::from_str("4.0").unwrap(),
            })
            .collect();
        state
            .fx_rates()
            .upsert_rates(&seed, ADAPTER_ID, "2026-07-20T12:00:00Z")
            .expect("seed");

        let fetcher = empty_store_fetcher();
        run_fx_pull_with(&state, "{}", &fetcher).expect("pull");

        let windows = fetcher.windows.borrow();
        assert_eq!(
            windows.first().map(|(s, _)| s.as_str()),
            Some("2026-07-20"),
            "a current store pulls only the recent window, got {windows:?}"
        );
        // A recent window (a few days) is a single chunk.
        assert_eq!(windows.len(), 1);
    }

    #[test]
    fn pull_stamps_last_success_at_and_records_outcome() {
        let state = AppState::new(open_in_memory_database().expect("db"));
        state
            .fx_rates()
            .upsert_rates(
                &[FxRate {
                    currency: "EUR".to_owned(),
                    date: "2026-07-20".to_owned(),
                    mid: Decimal::from_str("4.0").unwrap(),
                }],
                ADAPTER_ID,
                "2026-07-20T12:00:00Z",
            )
            .expect("seed so the pull is a small recent window");

        run_fx_pull_with(&state, "{}", &empty_store_fetcher()).expect("pull");

        let adapter = state
            .list_source_adapters_with_developer(true)
            .expect("adapters")
            .into_iter()
            .find(|a| a.id == ADAPTER_ID)
            .expect("nbp-fx row exists");
        assert!(
            adapter.last_success_at.is_some(),
            "a successful pull stamps last_success_at (DoD §C)"
        );
    }

    #[test]
    fn a_404_window_is_skipped_not_failed() {
        struct AllMissing;
        impl NbpFxFetcher for AllMissing {
            fn fetch_range(&self, _s: &str, _e: &str) -> Result<String, NbpFetchError> {
                Err(NbpFetchError::Status(404))
            }
        }
        let state = AppState::new(open_in_memory_database().expect("db"));
        state
            .fx_rates()
            .upsert_rates(
                &[FxRate {
                    currency: "EUR".to_owned(),
                    date: "2026-07-20".to_owned(),
                    mid: Decimal::from_str("4.0").unwrap(),
                }],
                ADAPTER_ID,
                "t",
            )
            .expect("seed");
        // A weekend-only recent window returns 404 for every chunk — the pull
        // still succeeds (no new rate that day is not an error).
        run_fx_pull_with(&state, "{}", &AllMissing).expect("404 window is not a failure");
    }

    #[test]
    fn a_non_404_error_fails_the_run_for_retry() {
        struct Failing;
        impl NbpFxFetcher for Failing {
            fn fetch_range(&self, _s: &str, _e: &str) -> Result<String, NbpFetchError> {
                Err(NbpFetchError::Status(500))
            }
        }
        let state = AppState::new(open_in_memory_database().expect("db"));
        state
            .fx_rates()
            .upsert_rates(
                &[FxRate {
                    currency: "EUR".to_owned(),
                    date: "2026-07-20".to_owned(),
                    mid: Decimal::from_str("4.0").unwrap(),
                }],
                ADAPTER_ID,
                "t",
            )
            .expect("seed");
        assert!(
            run_fx_pull_with(&state, "{}", &Failing).is_err(),
            "a 5xx must fail the run so the queue retries with backoff"
        );
    }

    #[test]
    fn enqueue_fx_daily_pull_is_idempotent_under_the_singleton_id() {
        let state = AppState::new(open_in_memory_database().expect("db"));
        enqueue_fx_daily_pull(&state);
        enqueue_fx_daily_pull(&state);
        let counts = state.jobs().counts().expect("counts");
        assert_eq!(counts.pending, 1, "one recurring row, not one per fire");

        let job = state.jobs().claim_next().expect("claim").expect("a job");
        assert_eq!(job.kind, FX_DAILY_PULL_KIND);
    }

    #[test]
    fn ensure_backfilled_enqueues_only_for_an_absent_currency() {
        let state = AppState::new(open_in_memory_database().expect("db"));

        // Absent -> a backfill job is planned naming the currency.
        ensure_fx_backfilled(&state, "nok").expect("ensure");
        let counts = state.jobs().counts().expect("counts");
        assert_eq!(counts.pending, 1);
        let job = state.jobs().claim_next().expect("claim").expect("a job");
        assert_eq!(job.kind, FX_DAILY_PULL_KIND);
        assert!(
            job.payload.contains("NOK"),
            "payload names the currency, got {}",
            job.payload
        );

        // PLN is never backfilled (it is the base currency).
        ensure_fx_backfilled(&state, "PLN").expect("ensure pln");
        assert_eq!(
            state.jobs().counts().expect("counts").pending,
            0,
            "PLN needs no FX row"
        );

        // A present currency is a no-op.
        state
            .fx_rates()
            .upsert_rates(
                &[FxRate {
                    currency: "USD".to_owned(),
                    date: "2024-01-15".to_owned(),
                    mid: Decimal::from_str("3.99").unwrap(),
                }],
                ADAPTER_ID,
                "t",
            )
            .expect("seed usd");
        ensure_fx_backfilled(&state, "USD").expect("ensure usd");
        assert_eq!(
            state.jobs().counts().expect("counts").pending,
            0,
            "an already-stored currency plans no backfill"
        );
    }

    #[test]
    fn parse_payload_currencies_tolerates_junk() {
        assert!(parse_payload_currencies("{}").is_empty());
        assert!(parse_payload_currencies("not json").is_empty());
        assert_eq!(
            parse_payload_currencies(r#"{"currencies":["USD","NOK"]}"#),
            vec!["USD".to_owned(), "NOK".to_owned()]
        );
    }

    // Compile-time proof the parse error type is reachable from the job module
    // (keeps the adapter/job seam explicit).
    #[allow(dead_code)]
    fn _parse_error_is_typed(_: NbpParseError) {}
}
