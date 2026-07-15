//! Price context read model (v0.53 T5, ADR 0067/0082).
//!
//! Answers "what does today's price/valuation picture look like for this
//! company?" for the fundamentals-adjacent Price context section
//! (`src/screens/Companies/PriceContextSection.tsx`): latest close + change,
//! 52-week range, level-0 market ratios, and a chart-ready close history.
//! Computed read model (ADR 0044 pattern) over `daily_quotes`
//! (`MarketDataStore`) plus the derived-metrics engine
//! (`fundamentals::metrics`) — no stored projection, so it can never go stale.
//!
//! Empty states (never silent, ADR 0082): a company whose exchange is not yet
//! mapped to a market-data provider (v1: GPW only, via `yahoo-eod`) reports `emptyReason: "unmapped_ticker"`; a mapped
//! company with no quotes yet (backfill pending/failed) reports
//! `emptyReason: "no_quotes"`. Decision-support only: no cheap/expensive or
//! buy/sell wording anywhere in this module.

use rust_decimal::prelude::*;
use rust_decimal::Decimal;
use serde::Serialize;
use time::macros::format_description;
use time::Date;

use crate::app_state;
use crate::fundamentals::expr::MetricResolver;
use crate::fundamentals::metrics::QuoteFacts;

/// Trailing window read for the 52-week high/low/percentile plus the chart —
/// a bit more than 52 weeks of slack so a session gap never drops the true
/// 52-week boundary bar.
const HISTORY_WINDOW_DAYS: i64 = 371;

/// GPW is the only market a v1 quote provider is mapped to (ADR 0082, T1):
/// the `yahoo-eod` registry entry declares `markets: GPW`.
/// A company on any other exchange has no path to a quote yet.
const MAPPED_EXCHANGE: &str = "GPW";

/// Currency for every quote in this store: v1 covers GPW only, and GPW quotes
/// are PLN-denominated (Yahoo's own chart `meta.currency` for every `.WA`
/// ticker, `source_adapters::yahoo_eod`). `daily_quotes` does not persist a
/// per-bar currency column (ADR 0067 migration 0071), so this is a documented
/// v1 assumption, not a per-row read; revisit when a non-PLN market is added.
const QUOTE_CURRENCY: &str = "PLN";

// ============================================================================
// DTOs (ts-rs export → ../../src/api/generated/; MUST match the frontend's
// hand-authored `PriceContext` type in
// src/screens/Companies/PriceContextSection.tsx exactly)
// ============================================================================

#[derive(Debug, Clone, Serialize)]
#[cfg_attr(feature = "ts-export", derive(ts_rs::TS))]
#[cfg_attr(
    feature = "ts-export",
    ts(export, export_to = "../../src/api/generated/")
)]
#[serde(rename_all = "camelCase")]
pub struct PriceContext {
    pub last_close: f64,
    pub last_date: String,
    /// Percentage points already (e.g. `-1.2` renders as "-1.2 %"), matching
    /// the shared `formatFinancialValue` percentage contract — never a 0..1
    /// fraction.
    pub change_abs: f64,
    pub change_pct: f64,
    pub currency: String,
    pub week52_high: f64,
    pub week52_low: f64,
    pub week52_high_dist_pct: f64,
    pub week52_low_dist_pct: f64,
    pub market_cap: Option<f64>,
    pub ratios: PriceContextRatios,
    pub history: Vec<PriceContextHistoryPoint>,
    pub fetched_at: String,
    /// Present only when there is nothing to render; every other field then
    /// carries an inert default (`0.0` / `""` / `[]`) and must be ignored —
    /// the section renders its empty state instead of the readout.
    #[serde(skip_serializing_if = "Option::is_none")]
    #[cfg_attr(
        feature = "ts-export",
        ts(optional, type = "\"no_quotes\" | \"unmapped_ticker\"")
    )]
    pub empty_reason: Option<String>,
}

#[derive(Debug, Clone, Serialize)]
#[cfg_attr(feature = "ts-export", derive(ts_rs::TS))]
#[cfg_attr(
    feature = "ts-export",
    ts(export, export_to = "../../src/api/generated/")
)]
#[serde(rename_all = "camelCase")]
pub struct PriceContextRatios {
    pub pe: Option<f64>,
    pub pbv: Option<f64>,
    pub ev_ebitda: Option<f64>,
    /// Percentage points (see `PriceContext::change_pct`).
    pub div_yield: Option<f64>,
    /// Percentage points (see `PriceContext::change_pct`).
    pub fcf_yield: Option<f64>,
    /// Raw `[0, 1]` fraction — the frontend scales it to percentage points
    /// itself (`PriceContextSection.tsx`), unlike every other percent field.
    pub own_hist_percentile: Option<f64>,
}

#[derive(Debug, Clone, Serialize)]
#[cfg_attr(feature = "ts-export", derive(ts_rs::TS))]
#[cfg_attr(
    feature = "ts-export",
    ts(export, export_to = "../../src/api/generated/")
)]
#[serde(rename_all = "camelCase")]
pub struct PriceContextHistoryPoint {
    pub date: String,
    /// Full OHLC per session (candlestick rendering, owner request 2026-07-14).
    /// `daily_quotes` stores all four NOT NULL, so these are never optional.
    pub open: f64,
    pub high: f64,
    pub low: f64,
    pub close: f64,
}

fn empty_ratios() -> PriceContextRatios {
    PriceContextRatios {
        pe: None,
        pbv: None,
        ev_ebitda: None,
        div_yield: None,
        fcf_yield: None,
        own_hist_percentile: None,
    }
}

fn empty_price_context(reason: &str) -> PriceContext {
    PriceContext {
        last_close: 0.0,
        last_date: String::new(),
        change_abs: 0.0,
        change_pct: 0.0,
        currency: QUOTE_CURRENCY.to_owned(),
        week52_high: 0.0,
        week52_low: 0.0,
        week52_high_dist_pct: 0.0,
        week52_low_dist_pct: 0.0,
        market_cap: None,
        ratios: empty_ratios(),
        history: Vec::new(),
        fetched_at: String::new(),
        empty_reason: Some(reason.to_owned()),
    }
}

/// Shift an ISO `YYYY-MM-DD` date back `delta_days` days, saturating rather
/// than panicking on an out-of-range result. Falls back to the epoch (the
/// widest possible window — never a panic, never a narrower-than-intended
/// read) when `date` fails to parse.
fn shift_iso_date_back(date: &str, delta_days: i64) -> String {
    let format = format_description!("[year]-[month]-[day]");
    let Ok(parsed) = Date::parse(date, &format) else {
        return "0000-01-01".to_owned();
    };
    let shifted = parsed.saturating_sub(time::Duration::days(delta_days));
    shifted
        .format(&format)
        .unwrap_or_else(|_| "0000-01-01".to_owned())
}

fn decimal_to_f64(value: Decimal) -> f64 {
    value.to_f64().unwrap_or(0.0)
}

/// Assemble the price context for one company (v0.53 T5). Reads
/// `daily_quotes` (bounded, indexed: a `latest_quote` point read plus one
/// `quotes_since` range scan — never a corpus recompute) and the
/// derived-metrics engine for the level-0 market ratios (migration 0072).
pub fn compute_price_context(
    state: &app_state::AppState,
    company_id: &str,
) -> Result<PriceContext, String> {
    let company = state
        .list_companies()
        .map_err(|error| error.to_string())?
        .into_iter()
        .find(|company| company.id == company_id)
        .ok_or_else(|| format!("no tracked company for id {company_id}"))?;

    if !company.exchange.eq_ignore_ascii_case(MAPPED_EXCHANGE) {
        return Ok(empty_price_context("unmapped_ticker"));
    }

    let market_data = state.market_data();
    let Some(latest) = market_data
        .latest_quote(company_id)
        .map_err(|error| error.to_string())?
    else {
        return Ok(empty_price_context("no_quotes"));
    };

    let from_date = shift_iso_date_back(&latest.date, HISTORY_WINDOW_DAYS);
    let history_bars = market_data
        .quotes_since(company_id, &from_date)
        .map_err(|error| error.to_string())?;

    let previous_close = if history_bars.len() >= 2 {
        Some(history_bars[history_bars.len() - 2].close)
    } else {
        None
    };
    let (change_abs, change_pct) = match previous_close {
        Some(previous) if previous != 0.0 => {
            let abs = latest.close - previous;
            (abs, (abs / previous) * 100.0)
        }
        _ => (0.0, 0.0),
    };

    let as_of_close = Decimal::from_f64(latest.close).unwrap_or_default();
    let close_history: Vec<Decimal> = history_bars
        .iter()
        .filter_map(|bar| Decimal::from_f64(bar.close))
        .collect();
    let quote_facts = QuoteFacts::from_close_history(as_of_close, &close_history);

    let context = state
        .quality_frameworks()
        .metrics_context(company_id)
        .map_err(|error| error.to_string())?
        .with_quotes(quote_facts);
    let resolver = context.resolver();

    let market_cap = resolver.value("market_cap").map(decimal_to_f64);
    let pe = resolver.value("pe_ratio").map(decimal_to_f64);
    let pbv = resolver.value("pbv_ratio").map(decimal_to_f64);
    let ev_ebitda = resolver.value("ev_ebitda").map(decimal_to_f64);
    let div_yield = resolver
        .value("dividend_yield")
        .map(|d| decimal_to_f64(d) * 100.0);
    let fcf_yield = resolver
        .value("fcf_yield")
        .map(|d| decimal_to_f64(d) * 100.0);
    let week52_high_dist_pct = resolver
        .value("distance_from_52w_high")
        .map(|d| decimal_to_f64(d) * 100.0)
        .unwrap_or(0.0);
    let week52_low_dist_pct = resolver
        .value("distance_from_52w_low")
        .map(|d| decimal_to_f64(d) * 100.0)
        .unwrap_or(0.0);
    // Honesty floor (owner question 2026-07-14): a percentile over a handful
    // of sessions is noise, not context — suppress it until the 52w window has
    // meaningful coverage (~1 month of sessions; fills in as backfill lands).
    const MIN_PERCENTILE_SESSIONS: usize = 20;
    let own_hist_percentile = if history_bars.len() >= MIN_PERCENTILE_SESSIONS {
        resolver
            .value("valuation_percentile_52w")
            .map(decimal_to_f64)
    } else {
        None
    };

    Ok(PriceContext {
        last_close: latest.close,
        last_date: latest.date,
        change_abs,
        change_pct,
        currency: QUOTE_CURRENCY.to_owned(),
        week52_high: decimal_to_f64(resolver.value("week52_high").unwrap_or(as_of_close)),
        week52_low: decimal_to_f64(resolver.value("week52_low").unwrap_or(as_of_close)),
        week52_high_dist_pct,
        week52_low_dist_pct,
        market_cap,
        ratios: PriceContextRatios {
            pe,
            pbv,
            ev_ebitda,
            div_yield,
            fcf_yield,
            own_hist_percentile,
        },
        history: history_bars
            .into_iter()
            .map(|bar| PriceContextHistoryPoint {
                date: bar.date,
                open: bar.open,
                high: bar.high,
                low: bar.low,
                close: bar.close,
            })
            .collect(),
        fetched_at: latest.fetched_at,
        empty_reason: None,
    })
}

/// Price context for the company workspace (v0.53 T5). Offloaded off the UI
/// thread: assembling it reads the quote range plus the derived-metrics
/// engine's period/definition tables.
#[tauri::command]
pub async fn get_price_context(
    company_id: String,
    state: tauri::State<'_, app_state::AppState>,
) -> Result<PriceContext, String> {
    let state = state.inner().clone();
    tauri::async_runtime::spawn_blocking(move || compute_price_context(&state, &company_id))
        .await
        .map_err(|error| format!("price context task failed: {error}"))?
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::source_adapters::yahoo_eod::DailyQuote;
    use crate::storage::{
        open_in_memory_database, AppState, NewCompany, NewFinancialFact, NewFinancialPeriod,
    };

    fn state() -> AppState {
        AppState::new(open_in_memory_database().expect("in-memory db"))
    }

    fn company(state: &AppState, exchange: &str) -> String {
        state
            .create_company(NewCompany {
                exchange: exchange.to_owned(),
                ticker: "TST".to_owned(),
                display_name: "Test S.A.".to_owned(),
                isin: None,
                cik: None,
                lei: None,
            })
            .expect("company")
            .id
    }

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

    #[test]
    fn unmapped_exchange_reports_unmapped_ticker() {
        let s = state();
        let c = company(&s, "NASDAQ");

        let context = compute_price_context(&s, &c).expect("compute");
        assert_eq!(context.empty_reason.as_deref(), Some("unmapped_ticker"));
    }

    #[test]
    fn gpw_company_with_no_quotes_reports_no_quotes() {
        let s = state();
        let c = company(&s, "GPW");

        let context = compute_price_context(&s, &c).expect("compute");
        assert_eq!(context.empty_reason.as_deref(), Some("no_quotes"));
    }

    #[test]
    fn gpw_company_with_quotes_assembles_close_change_and_range() {
        let s = state();
        let c = company(&s, "GPW");
        let quotes = vec![
            bar("2026-01-02", 200.0),
            bar("2026-06-01", 250.0),
            bar("2026-07-13", 231.0),
            bar("2026-07-14", 233.0),
        ];
        s.market_data()
            .upsert_quotes(&c, &quotes, "yahoo-eod", "2026-07-14T18:00:00Z")
            .expect("upsert");

        let context = compute_price_context(&s, &c).expect("compute");
        assert!(context.empty_reason.is_none());
        assert_eq!(context.last_close, 233.0);
        assert_eq!(context.last_date, "2026-07-14");
        assert_eq!(context.currency, "PLN");
        assert_eq!(context.fetched_at, "2026-07-14T18:00:00Z");
        assert!((context.change_abs - 2.0).abs() < 1e-9);
        assert_eq!(context.week52_high, 250.0);
        assert_eq!(context.week52_low, 200.0);
        assert_eq!(context.history.len(), 4);
        // No shares_outstanding fact yet: market_cap and every ratio that
        // depends on it must be None, never a panic or an invented value.
        assert!(context.market_cap.is_none());
        assert!(context.ratios.pe.is_none());
    }

    #[test]
    fn market_cap_resolves_once_shares_outstanding_is_confirmed() {
        let s = state();
        let c = company(&s, "GPW");
        s.market_data()
            .upsert_quotes(
                &c,
                &[bar("2026-07-14", 200.0)],
                "yahoo-eod",
                "2026-07-14T18:00:00Z",
            )
            .expect("upsert");
        let period = s
            .financials()
            .create_financial_period(NewFinancialPeriod {
                company_id: c.clone(),
                fiscal_year: 2025,
                period_type: "FY".to_owned(),
                period_end_date: None,
                report_evidence_ref: None,
            })
            .expect("period");
        s.financials()
            .create_financial_fact(NewFinancialFact {
                company_id: c.clone(),
                period_id: period.id,
                definition_id: "kpidef_shares_outstanding".to_owned(),
                value_numeric: "100".to_owned(),
                currency: Some("PLN".to_owned()),
                statement_basis: None,
                attribution: None,
                variant: None,
                measure_window: None,
                data_quality: None,
                as_reported_value: None,
                as_reported_scale: None,
                reporting_standard: None,
                extraction_method: None,
                confidence: None,
                confirmation_state: None,
                supersedes_id: None,
                source_document_ref: None,
            })
            .expect("shares_outstanding fact");

        let context = compute_price_context(&s, &c).expect("compute");
        assert_eq!(context.market_cap, Some(20_000.0));
    }

    /// Cross-language populated-path contract (review 2026-07-14, M1): the
    /// exact wire-format JSON of a POPULATED `PriceContext` is pinned in a
    /// shared fixture that the frontend contract test renders through the real
    /// component. Any DTO drift (field name/casing, percent scaling, null
    /// handling) reddens here; any renderer drift reddens on the vitest side —
    /// closing the gap the mock-fidelity corpus (empty-states only) leaves.
    #[test]
    fn populated_price_context_matches_the_shared_wire_fixture() {
        let s = state();
        let c = company(&s, "GPW");
        s.market_data()
            .upsert_quotes(
                &c,
                &[
                    bar("2026-01-02", 200.0),
                    bar("2026-06-01", 250.0),
                    bar("2026-07-13", 231.0),
                    bar("2026-07-14", 233.0),
                ],
                "yahoo-eod",
                "2026-07-14T18:00:00Z",
            )
            .expect("upsert");
        let period = s
            .financials()
            .create_financial_period(NewFinancialPeriod {
                company_id: c.clone(),
                fiscal_year: 2025,
                period_type: "FY".to_owned(),
                period_end_date: None,
                report_evidence_ref: None,
            })
            .expect("period");
        s.financials()
            .create_financial_fact(NewFinancialFact {
                company_id: c.clone(),
                period_id: period.id,
                definition_id: "kpidef_shares_outstanding".to_owned(),
                value_numeric: "100".to_owned(),
                currency: Some("PLN".to_owned()),
                statement_basis: None,
                attribution: None,
                variant: None,
                measure_window: None,
                data_quality: None,
                as_reported_value: None,
                as_reported_scale: None,
                reporting_standard: None,
                extraction_method: None,
                confidence: None,
                confirmation_state: None,
                supersedes_id: None,
                source_document_ref: None,
            })
            .expect("shares_outstanding fact");

        let context = compute_price_context(&s, &c).expect("compute");
        let actual = serde_json::to_value(&context).expect("serialize");
        let fixture: serde_json::Value = serde_json::from_str(include_str!(
            "../../../src/test/scenarios/priceContextPopulated.json"
        ))
        .expect("fixture parses");
        assert_eq!(
            actual, fixture,
            "populated PriceContext wire format drifted from the shared fixture \
             (src/test/scenarios/priceContextPopulated.json) — update BOTH sides deliberately; \
             actual: {actual}"
        );
    }
}
