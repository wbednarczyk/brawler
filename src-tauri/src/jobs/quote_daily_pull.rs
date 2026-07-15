//! Post-session daily EOD quote pull (Yahoo, ADR 0082 as amended 2026-07-14 —
//! the Twelve Data fallback was removed: GPW quotes are paid-plan-only there,
//! so a free key could never serve; a free degraded fallback is card ee81afe).
//!
//! Not a standalone job kind: driven by the existing Rust-side scheduler
//! through the normal `scheduled_source_refresh` dispatch for the `yahoo-eod`
//! runtime adapter (mirrors the `RefreshBehavior::Feed` arms in
//! `jobs::source_refresh::runtime_adapters`), so enabling `yahoo-eod` alone is
//! what wires the cadence — no separate scheduler entry needed.
//!
//! Watchlist-only (every tracked GPW company); skips non-session days
//! silently (no bar newer than what is already stored). Each run upserts the
//! WHOLE fetched window — filling any session days missed inside it and
//! correcting the current day's intraday partial to the final close — and
//! self-heals bigger gaps (no history at all / outage longer than the window)
//! by enqueueing the company's full `quote_backfill`. On success the run
//! stamps the adapter's `last_success_at`. A Yahoo 429/999, parse failure, or
//! unmapped ticker is recorded as a source-health diagnostic and that company
//! is skipped for the run (best-effort — one company's failure never aborts
//! the sweep; the self-heal backfill catches history up after an outage).
//! `// migrates to Fetcher (ADR 0069)`.

use time::{format_description::well_known::Rfc3339, OffsetDateTime};

use crate::source_adapters::market_data_fetch::{MarketDataFetchError, MarketDataFetcher};
use crate::source_adapters::yahoo_eod::{parse_yahoo_chart, QuoteSeries};
use crate::storage::{AppState, DiagnosticScope, NewDiagnosticEvent, SourceIngestionResult};

pub const YAHOO_ADAPTER_ID: &str = "yahoo-eod";

/// Default witness-divergence tolerance (ADR 0082): a percent difference from
/// the stored close beyond this raises a source-health diagnostic.
pub const DEFAULT_WITNESS_TOLERANCE_PCT: f64 = 1.0;

/// Queue entry point (via `jobs::source_refresh`'s `yahoo-eod` dispatch arm):
/// pull the latest session bar for every tracked GPW company using the live
/// HTTP fetcher and the real market-data credential lookup.
pub fn run_daily_pull_for_trigger(
    state: &AppState,
    trigger: &str,
) -> Result<SourceIngestionResult, String> {
    let fetcher = crate::source_adapters::market_data_fetch::HttpMarketDataFetcher;
    run_daily_pull_with(state, trigger, &fetcher)
}

/// Core daily-pull routine, generic over the fetcher (no network in tests).
pub fn run_daily_pull_with(
    state: &AppState,
    _trigger: &str,
    fetcher: &impl MarketDataFetcher,
) -> Result<SourceIngestionResult, String> {
    let companies = state
        .companies()
        .list_companies()
        .map_err(|error| error.to_string())?;

    let mut items_fetched = 0usize;
    let mut items_created = 0usize;
    let mut items_unmatched = 0usize;

    for company in companies
        .into_iter()
        .filter(|company| company.exchange.eq_ignore_ascii_case("GPW"))
    {
        items_fetched += 1;
        match pull_one_company(state, &company.id, &company.ticker, fetcher) {
            Ok(true) => items_created += 1,
            Ok(false) => {}
            Err(_) => items_unmatched += 1,
        }
    }

    // Stamp the adapter's runtime row (last_success_at + counters) so the
    // Sources UI reflects the pull — without this the source reads as
    // never-refreshed forever (review 2026-07-14).
    let fetched_at = now_rfc3339();
    state
        .market_data()
        .record_pull_outcome(
            YAHOO_ADAPTER_ID,
            &fetched_at,
            items_fetched,
            items_created,
            items_unmatched,
        )
        .map_err(|error| error.to_string())?;

    Ok(SourceIngestionResult {
        adapter_id: YAHOO_ADAPTER_ID.to_owned(),
        items_fetched,
        items_created,
        items_matched: items_created,
        items_unmatched,
        detail_items_attempted: 0,
        detail_items_stored: 0,
        detail_items_failed: 0,
        fetched_at: Some(fetched_at),
    })
}

/// Pull one company's latest session bar. Returns `Ok(true)` if a new bar was
/// written, `Ok(false)` if there was nothing new (non-session day / already
/// current), `Err` when the pull failed (the failure is also recorded as a
/// source-health diagnostic before returning; the company is skipped for the
/// run, never the whole sweep).
fn pull_one_company(
    state: &AppState,
    company_id: &str,
    ticker: &str,
    fetcher: &impl MarketDataFetcher,
) -> Result<bool, String> {
    let ticker_wa = format!("{}.WA", ticker.to_uppercase());

    let series = match fetcher
        .fetch_yahoo_recent(&ticker_wa)
        .map_err(PrimaryFailure::from)
        .and_then(|body| parse_yahoo_chart(&body).map_err(PrimaryFailure::Parse))
    {
        Ok(series) => series,
        Err(failure) => {
            record_source_health_diagnostic(
                state,
                YAHOO_ADAPTER_ID,
                Some(company_id),
                &format!("{ticker_wa}: {failure}"),
            );
            return Err(failure.to_string());
        }
    };

    upsert_window_bars(state, company_id, &series, YAHOO_ADAPTER_ID)
}

/// A primary-fetch failure: either the HTTP fetch itself failed (network,
/// 429/999, other non-2xx) or the response parsed but was unusable (provider
/// error payload, no data — the "unmapped ticker" case, ADR 0082).
#[derive(Debug)]
enum PrimaryFailure {
    Fetch(MarketDataFetchError),
    Parse(crate::source_adapters::yahoo_eod::QuoteParseError),
}

impl std::fmt::Display for PrimaryFailure {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Fetch(error) => write!(f, "{error}"),
            Self::Parse(error) => write!(f, "{error}"),
        }
    }
}

impl From<MarketDataFetchError> for PrimaryFailure {
    fn from(error: MarketDataFetchError) -> Self {
        Self::Fetch(error)
    }
}

/// Upsert the WHOLE fetched window for one company (review 2026-07-14):
/// filling every session day the window carries — not just the newest bar —
/// so a multi-day outage inside the window never leaves a permanent hole, and
/// re-upserting the current day corrects a mid-session partial price to the
/// final close (upsert by `(company_id, date)`). Returns `Ok(true)` only when
/// a bar NEWER than the previously stored latest was written (the
/// "one new row per session day, skip non-session days silently" counter).
///
/// Self-heal: a company with no stored history, or whose stored history ends
/// before the fetched window even begins (outage longer than the window),
/// additionally enqueues its full `quote_backfill` (idempotent stable job id)
/// — this is also what backfills companies tracked before v0.53 shipped.
fn upsert_window_bars(
    state: &AppState,
    company_id: &str,
    series: &QuoteSeries,
    source_adapter_id: &str,
) -> Result<bool, String> {
    let store = state.market_data();
    let Some(first_fetched) = series.quotes.first() else {
        return Ok(false);
    };

    let latest_stored = store
        .latest_quote(company_id)
        .map_err(|error| error.to_string())?;

    // Self-heal triggers: no history at all, a gap beyond the fetched window,
    // or a company whose deep backfill has never completed (fresh latest bar
    // but shallow history — companies tracked before v0.53; the marker is set
    // by a successful `quote_backfill`).
    let backfill_done = state
        .get_source_adapter_state(
            YAHOO_ADAPTER_ID,
            &crate::jobs::quote_backfill::backfill_done_key(company_id),
        )
        .map_err(|error| error.to_string())?
        .is_some();
    let needs_full_backfill = !backfill_done
        || match &latest_stored {
            None => true,
            Some(latest) => latest.date.as_str() < first_fetched.date.as_str(),
        };
    if needs_full_backfill {
        crate::jobs::quote_backfill::enqueue_backfill_for_company(state, company_id);
    }

    let wrote_new_bar = match &latest_stored {
        None => true,
        Some(latest) => series
            .quotes
            .iter()
            .any(|bar| bar.date.as_str() > latest.date.as_str()),
    };

    let fetched_at = now_rfc3339();
    store
        .upsert_quotes(company_id, &series.quotes, source_adapter_id, &fetched_at)
        .map_err(|error| error.to_string())?;

    Ok(wrote_new_bar)
}

/// Compare-only witness cross-check (ADR 0082): a divergence beyond
/// `tolerance_pct` between the stored latest close and an independently
/// sourced witness close raises a source-health diagnostic. Never writes to
/// `daily_quotes` — callers pass whatever witness close they obtained
/// (BiznesRadar/Bankier/StockWatch parsing is a separate concern, not part of
/// this job). Returns whether a divergence was raised.
pub fn check_witness_divergence(
    state: &AppState,
    company_id: &str,
    stored_close: f64,
    witness_close: f64,
    tolerance_pct: f64,
) -> bool {
    if stored_close <= 0.0 {
        return false;
    }
    let diff_pct = ((stored_close - witness_close).abs() / stored_close) * 100.0;
    let diverged = diff_pct > tolerance_pct;
    if diverged {
        record_source_health_diagnostic(
            state,
            YAHOO_ADAPTER_ID,
            Some(company_id),
            &format!(
                "witness divergence: stored close {stored_close:.4} vs witness {witness_close:.4} ({diff_pct:.2}% > {tolerance_pct:.2}%)"
            ),
        );
    }
    diverged
}

fn record_source_health_diagnostic(
    state: &AppState,
    adapter_id: &str,
    company_id: Option<&str>,
    message: &str,
) {
    let scope = company_id
        .map(|company_id| DiagnosticScope {
            scope_type: "company".to_owned(),
            id: Some(company_id.to_owned()),
        })
        .unwrap_or(DiagnosticScope {
            scope_type: "source_adapter".to_owned(),
            id: Some(adapter_id.to_owned()),
        });
    let _ = state.record_diagnostic_event(NewDiagnosticEvent {
        occurred_at: None,
        module: "market_data".to_owned(),
        scope: Some(scope),
        stage: "warning".to_owned(),
        severity: "warning".to_owned(),
        message: message.to_owned(),
        metadata: None,
    });
}

fn now_rfc3339() -> String {
    OffsetDateTime::now_utc()
        .format(&Rfc3339)
        .unwrap_or_else(|_| "1970-01-01T00:00:00Z".to_owned())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::storage::{open_in_memory_database, NewCompany};
    use serde_json::json;
    use std::cell::RefCell;

    fn seed_company(state: &AppState, ticker: &str) -> String {
        state
            .create_company(NewCompany {
                exchange: "GPW".to_owned(),
                ticker: ticker.to_owned(),
                display_name: format!("{ticker} S.A."),
                isin: None,
                cik: None,
                lei: None,
            })
            .expect("company should create")
            .id
    }

    fn yahoo_ok_body_multi(bars: &[(i64, f64)]) -> String {
        json!({
            "chart": {
                "result": [{
                    "meta": { "symbol": "CDR.WA", "currency": "PLN" },
                    "timestamp": bars.iter().map(|(ts, _)| *ts).collect::<Vec<_>>(),
                    "indicators": {
                        "quote": [{
                            "open": bars.iter().map(|(_, c)| c - 1.0).collect::<Vec<_>>(),
                            "high": bars.iter().map(|(_, c)| c + 1.0).collect::<Vec<_>>(),
                            "low": bars.iter().map(|(_, c)| c - 2.0).collect::<Vec<_>>(),
                            "close": bars.iter().map(|(_, c)| *c).collect::<Vec<_>>(),
                            "volume": bars.iter().map(|_| 1000).collect::<Vec<_>>()
                        }]
                    }
                }],
                "error": null
            }
        })
        .to_string()
    }

    fn yahoo_ok_body(date_unix: i64, close: f64) -> String {
        json!({
            "chart": {
                "result": [{
                    "meta": { "symbol": "CDR.WA", "currency": "PLN" },
                    "timestamp": [date_unix],
                    "indicators": {
                        "quote": [{
                            "open": [close - 1.0],
                            "high": [close + 1.0],
                            "low": [close - 2.0],
                            "close": [close],
                            "volume": [1000]
                        }]
                    }
                }],
                "error": null
            }
        })
        .to_string()
    }

    /// A fetcher whose Yahoo-recent response is configurable per test.
    struct ScriptedFetcher {
        yahoo_recent: RefCell<Result<String, MarketDataFetchError>>,
    }

    impl MarketDataFetcher for ScriptedFetcher {
        fn fetch_yahoo_full_history(
            &self,
            _ticker_wa: &str,
        ) -> Result<String, MarketDataFetchError> {
            panic!("quote_daily_pull must not call fetch_yahoo_full_history");
        }

        fn fetch_yahoo_recent(&self, _ticker_wa: &str) -> Result<String, MarketDataFetchError> {
            match &*self.yahoo_recent.borrow() {
                Ok(body) => Ok(body.clone()),
                Err(MarketDataFetchError::Status(status)) => {
                    Err(MarketDataFetchError::Status(*status))
                }
                Err(_) => unreachable!("test only scripts Status errors"),
            }
        }
    }

    #[test]
    fn daily_pull_appends_exactly_one_row_for_a_new_session_day() {
        let state = AppState::new(open_in_memory_database().expect("db"));
        let company_id = seed_company(&state, "CDR");
        let fetcher = ScriptedFetcher {
            yahoo_recent: RefCell::new(Ok(yahoo_ok_body(1_752_400_000, 240.0))),
        };

        let result =
            run_daily_pull_with(&state, "scheduler", &fetcher).expect("daily pull should succeed");

        assert_eq!(result.items_fetched, 1);
        assert_eq!(result.items_created, 1, "one new session-day row written");

        let series = state
            .market_data()
            .quotes_since(&company_id, "2000-01-01")
            .expect("read back");
        assert_eq!(series.len(), 1);
        assert_eq!(series[0].close, 240.0);

        // Re-running with the same (now stale) bar writes nothing new — the
        // "skip non-session days silently" / idempotence contract.
        let second = run_daily_pull_with(&state, "scheduler", &fetcher)
            .expect("second daily pull should succeed");
        assert_eq!(second.items_created, 0, "no newer bar -> nothing written");
        let series_after = state
            .market_data()
            .quotes_since(&company_id, "2000-01-01")
            .expect("read back again");
        assert_eq!(series_after.len(), 1, "still exactly one row");
    }

    // Unix seconds at 00:00 UTC for a run of consecutive dates (Yahoo GPW
    // daily bars land at midnight UTC): 2026-07-06 .. 2026-07-10.
    const D_JUL06: i64 = 1_783_296_000;
    const D_JUL07: i64 = D_JUL06 + 86_400;
    const D_JUL08: i64 = D_JUL06 + 2 * 86_400;
    const D_JUL09: i64 = D_JUL06 + 3 * 86_400;
    const D_JUL10: i64 = D_JUL06 + 4 * 86_400;

    fn stored_bar(state: &AppState, company_id: &str, date: &str, close: f64) {
        state
            .market_data()
            .upsert_quotes(
                company_id,
                &[crate::source_adapters::yahoo_eod::DailyQuote {
                    date: date.to_owned(),
                    open: close - 1.0,
                    high: close + 1.0,
                    low: close - 2.0,
                    close,
                    volume: 1000,
                }],
                YAHOO_ADAPTER_ID,
                "2026-07-06T18:00:00Z",
            )
            .expect("seed bar");
    }

    #[test]
    fn daily_pull_fills_a_multi_day_gap_within_the_fetched_window() {
        let state = AppState::new(open_in_memory_database().expect("db"));
        let company_id = seed_company(&state, "CDR");
        stored_bar(&state, &company_id, "2026-07-06", 230.0);

        // The app was off for three sessions; the 5d window carries all of them.
        let fetcher = ScriptedFetcher {
            yahoo_recent: RefCell::new(Ok(yahoo_ok_body_multi(&[
                (D_JUL07, 231.0),
                (D_JUL08, 232.0),
                (D_JUL09, 233.0),
                (D_JUL10, 234.0),
            ]))),
        };

        run_daily_pull_with(&state, "scheduler", &fetcher).expect("pull");

        let series = state
            .market_data()
            .quotes_since(&company_id, "2000-01-01")
            .expect("read back");
        assert_eq!(
            series.len(),
            5,
            "every session day in the fetched window is filled, not just the newest"
        );
        assert_eq!(series[4].date, "2026-07-10");
        assert_eq!(series[4].close, 234.0);
    }

    #[test]
    fn daily_pull_corrects_the_current_days_intraday_bar() {
        let state = AppState::new(open_in_memory_database().expect("db"));
        let company_id = seed_company(&state, "CDR");
        // A mid-session pull stored a partial price as today's bar.
        stored_bar(&state, &company_id, "2026-07-10", 230.0);

        // The post-session pull returns the same date with the final close.
        let fetcher = ScriptedFetcher {
            yahoo_recent: RefCell::new(Ok(yahoo_ok_body_multi(&[(D_JUL10, 234.5)]))),
        };

        run_daily_pull_with(&state, "scheduler", &fetcher).expect("pull");

        let series = state
            .market_data()
            .quotes_since(&company_id, "2000-01-01")
            .expect("read back");
        assert_eq!(
            series.len(),
            1,
            "correction overwrites by key, never appends"
        );
        assert_eq!(
            series[0].close, 234.5,
            "the same-day bar is corrected to the final close"
        );
    }

    #[test]
    fn daily_pull_enqueues_backfill_when_the_company_has_no_history() {
        let state = AppState::new(open_in_memory_database().expect("db"));
        let company_id = seed_company(&state, "CDR");
        let fetcher = ScriptedFetcher {
            yahoo_recent: RefCell::new(Ok(yahoo_ok_body(D_JUL10, 240.0))),
        };

        run_daily_pull_with(&state, "scheduler", &fetcher).expect("pull");

        let counts = state.jobs().counts().expect("counts");
        assert_eq!(
            counts.pending, 1,
            "a company with no stored history self-heals via an enqueued quote_backfill"
        );
        let series = state
            .market_data()
            .quotes_since(&company_id, "2000-01-01")
            .expect("read back");
        assert_eq!(
            series.len(),
            1,
            "the window bar is still written immediately"
        );
    }

    #[test]
    fn daily_pull_enqueues_backfill_when_the_gap_exceeds_the_fetched_window() {
        let state = AppState::new(open_in_memory_database().expect("db"));
        let company_id = seed_company(&state, "CDR");
        stored_bar(&state, &company_id, "2026-01-02", 200.0);

        // The stored history ends far before the fetched window's first bar.
        let fetcher = ScriptedFetcher {
            yahoo_recent: RefCell::new(Ok(yahoo_ok_body_multi(&[
                (D_JUL09, 233.0),
                (D_JUL10, 234.0),
            ]))),
        };

        run_daily_pull_with(&state, "scheduler", &fetcher).expect("pull");

        let counts = state.jobs().counts().expect("counts");
        assert_eq!(
            counts.pending, 1,
            "a gap beyond the fetched window self-heals via an enqueued quote_backfill"
        );
    }

    #[test]
    fn daily_pull_enqueues_backfill_for_shallow_history_without_the_done_marker() {
        let state = AppState::new(open_in_memory_database().expect("db"));
        let company_id = seed_company(&state, "CDR");
        // Fresh latest bar (no gap!) — but the deep backfill never ran.
        stored_bar(&state, &company_id, "2026-07-09", 233.0);

        let fetcher = ScriptedFetcher {
            yahoo_recent: RefCell::new(Ok(yahoo_ok_body_multi(&[
                (D_JUL09, 233.0),
                (D_JUL10, 234.0),
            ]))),
        };

        run_daily_pull_with(&state, "scheduler", &fetcher).expect("pull");

        let counts = state.jobs().counts().expect("counts");
        assert_eq!(
            counts.pending, 1,
            "shallow history (no backfill_done marker) self-heals via quote_backfill"
        );
    }

    #[test]
    fn daily_pull_skips_backfill_once_the_done_marker_is_set() {
        let state = AppState::new(open_in_memory_database().expect("db"));
        let company_id = seed_company(&state, "CDR");
        stored_bar(&state, &company_id, "2026-07-09", 233.0);
        state
            .record_source_adapter_state(
                YAHOO_ADAPTER_ID,
                &crate::jobs::quote_backfill::backfill_done_key(&company_id),
                "2026-07-09T18:00:00Z",
            )
            .expect("marker");

        let fetcher = ScriptedFetcher {
            yahoo_recent: RefCell::new(Ok(yahoo_ok_body_multi(&[
                (D_JUL09, 233.0),
                (D_JUL10, 234.0),
            ]))),
        };

        run_daily_pull_with(&state, "scheduler", &fetcher).expect("pull");

        let counts = state.jobs().counts().expect("counts");
        assert_eq!(
            counts.pending, 0,
            "a completed backfill (marker set) must not be re-planned on every pull"
        );
    }

    #[test]
    fn daily_pull_stamps_the_adapter_outcome() {
        let state = AppState::new(open_in_memory_database().expect("db"));
        seed_company(&state, "CDR");
        let fetcher = ScriptedFetcher {
            yahoo_recent: RefCell::new(Ok(yahoo_ok_body(D_JUL10, 240.0))),
        };

        run_daily_pull_with(&state, "scheduler", &fetcher).expect("pull");

        let adapters = state.list_source_adapters().expect("adapters should list");
        let yahoo = adapters
            .iter()
            .find(|adapter| adapter.id == YAHOO_ADAPTER_ID)
            .expect("yahoo-eod should exist");
        assert!(
            yahoo.last_success_at.is_some(),
            "a successful pull stamps last_success_at so the Sources UI shows the refresh"
        );
    }

    #[test]
    fn yahoo_failure_records_a_diagnostic_skips_the_company_and_writes_nothing() {
        // The Twelve Data fallback was removed (ADR 0082 amendment 2026-07-14):
        // a Yahoo 429/999/parse failure now records a source-health diagnostic,
        // returns Err for that company (counted as unmatched), and writes no
        // bar — the sweep continues. This is the ONLY coverage of the failure
        // branch (the deleted fallback test used to exercise it).
        let state = AppState::new(open_in_memory_database().expect("db"));
        state
            .set_developer_mode_enabled(true)
            .expect("developer mode should enable (diagnostics are dev-mode gated)");
        let company_id = seed_company(&state, "CDR");
        let fetcher = ScriptedFetcher {
            yahoo_recent: RefCell::new(Err(MarketDataFetchError::Status(429))),
        };

        let result =
            run_daily_pull_with(&state, "scheduler", &fetcher).expect("the sweep itself succeeds");

        assert_eq!(result.items_fetched, 1);
        assert_eq!(result.items_created, 0, "a failed pull writes no bar");
        assert_eq!(
            result.items_unmatched, 1,
            "a Yahoo failure counts the company as unmatched, not silently skipped"
        );

        let series = state
            .market_data()
            .quotes_since(&company_id, "2000-01-01")
            .expect("read back");
        assert!(series.is_empty(), "no bar is stored on a failed pull");

        let events = state
            .list_diagnostic_events(10)
            .expect("diagnostic events should list");
        assert!(
            events.iter().any(|event| event.module == "market_data"
                && event.message.contains("CDR.WA")
                && event.message.contains("429")),
            "a source-health diagnostic naming the ticker and cause was recorded, got: {events:?}"
        );
    }

    #[test]
    fn witness_divergence_raises_a_diagnostic_and_never_changes_the_stored_close() {
        let state = AppState::new(open_in_memory_database().expect("db"));
        state
            .set_developer_mode_enabled(true)
            .expect("developer mode should enable (diagnostics are dev-mode gated)");
        let company_id = seed_company(&state, "CDR");
        let fetcher = ScriptedFetcher {
            yahoo_recent: RefCell::new(Ok(yahoo_ok_body(1_752_400_000, 240.0))),
        };
        run_daily_pull_with(&state, "scheduler", &fetcher).expect("seed a stored close");

        let diverged = check_witness_divergence(
            &state,
            &company_id,
            240.0,
            300.0,
            DEFAULT_WITNESS_TOLERANCE_PCT,
        );
        assert!(diverged, "25% divergence exceeds the 1% tolerance");

        let stored = state
            .market_data()
            .latest_quote(&company_id)
            .expect("read")
            .expect("a stored bar");
        assert_eq!(
            stored.close, 240.0,
            "the witness check is compare-only and never overwrites the stored close"
        );

        let events = state
            .list_diagnostic_events(10)
            .expect("diagnostic events should list");
        assert!(
            events.iter().any(|event| event.module == "market_data"
                && event.message.contains("witness divergence")),
            "a source-health diagnostic was recorded, got: {events:?}"
        );
    }

    #[test]
    fn witness_within_tolerance_raises_no_diagnostic() {
        let state = AppState::new(open_in_memory_database().expect("db"));
        state.set_developer_mode_enabled(true).expect("dev mode");
        let company_id = seed_company(&state, "CDR");

        let diverged = check_witness_divergence(
            &state,
            &company_id,
            240.0,
            240.5,
            DEFAULT_WITNESS_TOLERANCE_PCT,
        );

        assert!(!diverged, "0.2% divergence is within the 1% tolerance");
        let events = state.list_diagnostic_events(10).expect("list");
        assert!(
            events.is_empty(),
            "no diagnostic for an in-tolerance witness"
        );
    }
}
