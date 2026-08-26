//! Company View composed read model (F3a #429, ADR 0107 dec. 3;
//! `docs/contracts.md` § "Company View (Spółka)"). Everything the Spółka
//! screen's glance bar and core render, composed in ONE read: counters
//! (signals/claims/shorts/events), KPI trend, feed strip, price chart,
//! coverage, and analyst recommendations — six independent sections, each
//! degrading via `sectionErrors` instead of failing the whole read (ADR 0081
//! Partial), mirroring [`crate::commands::today`] /
//! [`crate::commands::company_context`]. Top-level `Err` ONLY for an unknown
//! company or a failed read establishment. Offloaded off the UI thread via
//! `spawn_blocking`.

use std::collections::{BTreeSet, HashMap};

use serde::Serialize;

use crate::app_state::AppState;
use crate::commands::fundamentals_coverage::CoveragePeriodRow;
use crate::commands::today::SectionErrorKind;
use crate::commands::{fundamentals_coverage, market_data};
use crate::storage::{
    AnalystRecommendationRow, CompanyEventListInput, FinancialFact, ListFinancialFactsInput,
    ListFinancialPeriodsInput, PresentationKind,
};

/// Feed strip cap (contracts.md § Company View — feed cap = 6).
const FEED_LIMIT: i64 = 6;
/// Recommendations panel cap (contracts.md § Company View — "3 latest").
const RECOMMENDATIONS_LIMIT: usize = 3;
/// KPI trend window (contracts.md § Company View — last 4 fiscal years).
const KPI_YEARS: usize = 4;
/// Chart window (plan `frontend-v2-f3a.md` § S1 — last 66 sessions present).
const PRICE_SESSIONS: usize = 66;
/// delta1M lookback (plan S1 — close vs close 21 sessions back).
const DELTA_1M_SESSIONS_BACK: usize = 21;
/// Upcoming-events window (contracts.md § Company View — closed
/// `[today, today+30d]`).
const EVENTS_WINDOW_DAYS: i64 = 30;
/// The three FY metrics the KPI trend row set covers, in display order.
const KPI_METRIC_KEYS: [&str; 3] = ["revenue", "operating_profit", "net_profit"];

// ============================================================================
// DTOs (ts-rs export → ../../src/api/generated/)
// ============================================================================

/// One red-flag category's share of the unacknowledged count.
#[derive(Debug, Clone, PartialEq, Serialize)]
#[cfg_attr(feature = "ts-export", derive(ts_rs::TS))]
#[cfg_attr(
    feature = "ts-export",
    ts(export, export_to = "../../src/api/generated/")
)]
#[serde(rename_all = "camelCase")]
pub struct CompanyViewCategoryCount {
    pub category: String,
    pub count: i64,
}

/// Unacknowledged company signals (full history), by category.
#[derive(Debug, Clone, Serialize)]
#[cfg_attr(feature = "ts-export", derive(ts_rs::TS))]
#[cfg_attr(
    feature = "ts-export",
    ts(export, export_to = "../../src/api/generated/")
)]
#[serde(rename_all = "camelCase")]
pub struct CompanyViewSignalCounts {
    pub unacked: i64,
    pub by_category: Vec<CompanyViewCategoryCount>,
}

/// Open (`pending`/`partially_delivered`) management claims.
#[derive(Debug, Clone, Serialize)]
#[cfg_attr(feature = "ts-export", derive(ts_rs::TS))]
#[cfg_attr(
    feature = "ts-export",
    ts(export, export_to = "../../src/api/generated/")
)]
#[serde(rename_all = "camelCase")]
pub struct CompanyViewClaimCounts {
    pub open: i64,
    #[serde(skip_serializing_if = "Option::is_none")]
    #[cfg_attr(feature = "ts-export", ts(optional))]
    pub nearest_due: Option<String>,
}

/// Active KNF short-position registry rows.
#[derive(Debug, Clone, Serialize)]
#[cfg_attr(feature = "ts-export", derive(ts_rs::TS))]
#[cfg_attr(
    feature = "ts-export",
    ts(export, export_to = "../../src/api/generated/")
)]
#[serde(rename_all = "camelCase")]
pub struct CompanyViewShortCounts {
    pub active_sum_pct: f64,
    #[serde(skip_serializing_if = "Option::is_none")]
    #[cfg_attr(feature = "ts-export", ts(optional))]
    pub largest_holder: Option<String>,
}

/// Upcoming `scheduled` calendar events within the 30-day window.
#[derive(Debug, Clone, Serialize)]
#[cfg_attr(feature = "ts-export", derive(ts_rs::TS))]
#[cfg_attr(
    feature = "ts-export",
    ts(export, export_to = "../../src/api/generated/")
)]
#[serde(rename_all = "camelCase")]
pub struct CompanyViewEventCounts {
    pub upcoming: i64,
}

/// The glance bar's four counter groups.
#[derive(Debug, Clone, Serialize)]
#[cfg_attr(feature = "ts-export", derive(ts_rs::TS))]
#[cfg_attr(
    feature = "ts-export",
    ts(export, export_to = "../../src/api/generated/")
)]
#[serde(rename_all = "camelCase")]
pub struct CompanyViewCounters {
    pub signals: CompanyViewSignalCounts,
    pub claims: CompanyViewClaimCounts,
    pub shorts: CompanyViewShortCounts,
    pub events: CompanyViewEventCounts,
}

/// One fiscal year's value for one KPI row.
#[derive(Debug, Clone, Serialize)]
#[cfg_attr(feature = "ts-export", derive(ts_rs::TS))]
#[cfg_attr(
    feature = "ts-export",
    ts(export, export_to = "../../src/api/generated/")
)]
#[serde(rename_all = "camelCase")]
pub struct CompanyViewKpiCell {
    pub fiscal_year: i64,
    /// Verbatim from `FinancialFact.value_numeric` — never re-parsed.
    #[serde(skip_serializing_if = "Option::is_none")]
    #[cfg_attr(feature = "ts-export", ts(optional))]
    pub value_numeric: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    #[cfg_attr(feature = "ts-export", ts(optional))]
    pub source_document_ref: Option<String>,
}

/// One metric's FY trend row (revenue / operating profit / net profit).
#[derive(Debug, Clone, Serialize)]
#[cfg_attr(feature = "ts-export", derive(ts_rs::TS))]
#[cfg_attr(
    feature = "ts-export",
    ts(export, export_to = "../../src/api/generated/")
)]
#[serde(rename_all = "camelCase")]
pub struct CompanyViewKpiRow {
    pub metric_key: String,
    pub cells: Vec<CompanyViewKpiCell>,
    #[serde(skip_serializing_if = "Option::is_none")]
    #[cfg_attr(feature = "ts-export", ts(optional))]
    pub yoy_pct: Option<f64>,
}

/// The KPI trend section: last 4 FY, revenue/operating-profit/net-profit.
#[derive(Debug, Clone, Serialize)]
#[cfg_attr(feature = "ts-export", derive(ts_rs::TS))]
#[cfg_attr(
    feature = "ts-export",
    ts(export, export_to = "../../src/api/generated/")
)]
#[serde(rename_all = "camelCase")]
pub struct CompanyViewKpi {
    #[serde(skip_serializing_if = "Option::is_none")]
    #[cfg_attr(feature = "ts-export", ts(optional))]
    pub currency: Option<String>,
    pub rows: Vec<CompanyViewKpiRow>,
    pub years: Vec<i64>,
}

/// One feed item in the glance strip (newest 6).
#[derive(Debug, Clone, Serialize)]
#[cfg_attr(feature = "ts-export", derive(ts_rs::TS))]
#[cfg_attr(
    feature = "ts-export",
    ts(export, export_to = "../../src/api/generated/")
)]
#[serde(rename_all = "camelCase")]
pub struct CompanyViewFeedItem {
    pub feed_item_id: String,
    pub title: String,
    pub published_at: String,
    pub read: bool,
    /// `"Official report" | "Public media"` (the feed's closed item-type set
    /// this section filters to).
    pub item_type: String,
    pub source_name: String,
    pub presentation_kind: PresentationKind,
}

/// One daily OHLC session.
#[derive(Debug, Clone, Serialize)]
#[cfg_attr(feature = "ts-export", derive(ts_rs::TS))]
#[cfg_attr(
    feature = "ts-export",
    ts(export, export_to = "../../src/api/generated/")
)]
#[serde(rename_all = "camelCase")]
pub struct CompanyViewCandle {
    pub date: String,
    pub open: f64,
    pub high: f64,
    pub low: f64,
    pub close: f64,
}

/// The price chart section: last 66 sessions + the two deltas, sliced from
/// `compute_price_context`'s series (plan S1 exact semantics).
#[derive(Debug, Clone, Serialize)]
#[cfg_attr(feature = "ts-export", derive(ts_rs::TS))]
#[cfg_attr(
    feature = "ts-export",
    ts(export, export_to = "../../src/api/generated/")
)]
#[serde(rename_all = "camelCase")]
pub struct CompanyViewPrice {
    pub candles: Vec<CompanyViewCandle>,
    pub last_close: f64,
    pub as_of: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    #[cfg_attr(feature = "ts-export", ts(optional))]
    pub delta_1m_pct: Option<f64>,
    /// vs the last session of the prior calendar year; `None` (never `0.0`)
    /// when no such session exists.
    #[serde(skip_serializing_if = "Option::is_none")]
    #[cfg_attr(feature = "ts-export", ts(optional))]
    pub delta_ytd_pct: Option<f64>,
    pub currency: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    #[cfg_attr(
        feature = "ts-export",
        ts(optional, type = "\"no_quotes\" | \"unmapped_ticker\"")
    )]
    pub empty_reason: Option<String>,
}

/// Per-section degradation (ADR 0081 Partial, the F2/F1 pattern): a storage
/// error in one section fills its slot instead of failing the whole read.
#[derive(Debug, Clone, Default, Serialize)]
#[cfg_attr(feature = "ts-export", derive(ts_rs::TS))]
#[cfg_attr(
    feature = "ts-export",
    ts(export, export_to = "../../src/api/generated/")
)]
#[serde(rename_all = "camelCase")]
pub struct CompanyViewSectionErrors {
    #[serde(skip_serializing_if = "Option::is_none")]
    #[cfg_attr(feature = "ts-export", ts(optional))]
    pub counters: Option<SectionErrorKind>,
    #[serde(skip_serializing_if = "Option::is_none")]
    #[cfg_attr(feature = "ts-export", ts(optional))]
    pub kpi: Option<SectionErrorKind>,
    #[serde(skip_serializing_if = "Option::is_none")]
    #[cfg_attr(feature = "ts-export", ts(optional))]
    pub feed: Option<SectionErrorKind>,
    #[serde(skip_serializing_if = "Option::is_none")]
    #[cfg_attr(feature = "ts-export", ts(optional))]
    pub price: Option<SectionErrorKind>,
    #[serde(skip_serializing_if = "Option::is_none")]
    #[cfg_attr(feature = "ts-export", ts(optional))]
    pub coverage: Option<SectionErrorKind>,
    #[serde(skip_serializing_if = "Option::is_none")]
    #[cfg_attr(feature = "ts-export", ts(optional))]
    pub recommendations: Option<SectionErrorKind>,
}

/// Everything the Spółka screen's glance bar and core render, composed in one
/// read (ADR 0107 dec. 3, F3a #429).
#[derive(Debug, Clone, Serialize)]
#[cfg_attr(feature = "ts-export", derive(ts_rs::TS))]
#[cfg_attr(
    feature = "ts-export",
    ts(export, export_to = "../../src/api/generated/")
)]
#[serde(rename_all = "camelCase")]
pub struct CompanyView {
    pub company_id: String,
    pub qualified_ticker: String,
    pub display_name: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    #[cfg_attr(feature = "ts-export", ts(optional))]
    pub isin: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    #[cfg_attr(feature = "ts-export", ts(optional))]
    pub counters: Option<CompanyViewCounters>,
    #[serde(skip_serializing_if = "Option::is_none")]
    #[cfg_attr(feature = "ts-export", ts(optional))]
    pub kpi: Option<CompanyViewKpi>,
    pub feed: Vec<CompanyViewFeedItem>,
    #[serde(skip_serializing_if = "Option::is_none")]
    #[cfg_attr(feature = "ts-export", ts(optional))]
    pub price: Option<CompanyViewPrice>,
    pub coverage: Vec<CoveragePeriodRow>,
    pub recommendations: Vec<AnalystRecommendationRow>,
    pub section_errors: CompanyViewSectionErrors,
}

// ============================================================================
// Helpers
// ============================================================================

fn today_date() -> time::Date {
    time::OffsetDateTime::now_utc().date()
}

fn format_date(date: time::Date) -> String {
    let format = time::macros::format_description!("[year]-[month]-[day]");
    date.format(&format)
        .unwrap_or_else(|_| "1970-01-01".to_owned())
}

/// `"{period_type} {fiscal_year}"` (e.g. `"FY 2026"`) — the same label shape
/// as `company_context::period_label`.
fn claim_period_label(fiscal_year: i64, period_type: &str) -> String {
    format!("{period_type} {fiscal_year}")
}

/// Domain period order within a fiscal year (Q1 < H1/Q2 < 9M/Q3 < Q4/FY) —
/// alphabetical order gets this wrong (`FY` < `H1` < `Q1`).
fn period_type_rank(period_type: &str) -> u8 {
    match period_type {
        "Q1" => 1,
        "H1" | "Q2" => 2,
        "9M" | "Q3" => 3,
        "Q4" | "FY" => 4,
        _ => 5,
    }
}

/// `(last − previous)/|previous|×100` over the two newest cells that both
/// carry a parseable value; `None` (never a divide-by-zero panic/NaN) when
/// fewer than two cells have values or the older one is zero.
fn compute_yoy(cells: &[CompanyViewKpiCell]) -> Option<f64> {
    let n = cells.len();
    if n < 2 {
        return None;
    }
    let last: f64 = cells[n - 1].value_numeric.as_deref()?.trim().parse().ok()?;
    let previous: f64 = cells[n - 2].value_numeric.as_deref()?.trim().parse().ok()?;
    if previous == 0.0 {
        return None;
    }
    Some((last - previous) / previous.abs() * 100.0)
}

// ============================================================================
// Section assembly
// ============================================================================

fn compute_counters(
    state: &AppState,
    company_id: &str,
    today: &str,
    horizon: &str,
) -> Result<CompanyViewCounters, String> {
    let flags = state
        .red_flags()
        .red_flags_view(company_id)
        .map_err(|error| error.to_string())?;
    let mut counts: HashMap<String, i64> = HashMap::new();
    for flag in &flags.active {
        *counts.entry(flag.flag_type.clone()).or_insert(0) += 1;
    }
    let mut by_category: Vec<CompanyViewCategoryCount> = counts
        .into_iter()
        .map(|(category, count)| CompanyViewCategoryCount { category, count })
        .collect();
    by_category.sort_by(|a, b| {
        b.count
            .cmp(&a.count)
            .then_with(|| a.category.cmp(&b.category))
    });
    let signals = CompanyViewSignalCounts {
        unacked: flags.active.len() as i64,
        by_category,
    };

    let claims = state
        .list_management_claims(company_id)
        .map_err(|error| error.to_string())?;
    let open_claims: Vec<_> = claims
        .iter()
        .filter(|claim| claim.status == "pending" || claim.status == "partially_delivered")
        .collect();
    let nearest_due = open_claims
        .iter()
        .filter_map(|claim| {
            let fiscal_year = claim.due_fiscal_year?;
            let period_type = claim.due_period_type.clone()?;
            Some((fiscal_year, period_type))
        })
        .min_by(|a, b| {
            a.0.cmp(&b.0)
                .then_with(|| period_type_rank(&a.1).cmp(&period_type_rank(&b.1)))
                .then_with(|| a.1.cmp(&b.1))
        })
        .map(|(fiscal_year, period_type)| claim_period_label(fiscal_year, &period_type));
    let claim_counts = CompanyViewClaimCounts {
        open: open_claims.len() as i64,
        nearest_due,
    };

    let shorts_view = state
        .short_positions()
        .short_positions_view(company_id)
        .map_err(|error| error.to_string())?;
    // Largest active position, alphabetically-first holder on a percent tie
    // (contracts.md § Company View) — an explicit fold rather than
    // `Iterator::max_by` (which returns the LAST maximum on a tie, the wrong
    // direction here).
    let mut largest: Option<&crate::storage::ShortPositionRow> = None;
    for row in &shorts_view.positions {
        largest = match largest {
            None => Some(row),
            Some(current) if row.net_position_pct > current.net_position_pct => Some(row),
            Some(current)
                if row.net_position_pct == current.net_position_pct
                    && row.holder_name < current.holder_name =>
            {
                Some(row)
            }
            Some(current) => Some(current),
        };
    }
    let short_counts = CompanyViewShortCounts {
        active_sum_pct: shorts_view.aggregate_pct,
        largest_holder: largest.map(|row| row.holder_name.clone()),
    };

    let events = state
        .list_company_events(CompanyEventListInput {
            mode: None,
            company_id: Some(company_id.to_owned()),
            watchlist_id: None,
            event_type: None,
            status: Some("scheduled".to_owned()),
            date_from: Some(today.to_owned()),
            date_to: Some(horizon.to_owned()),
        })
        .map_err(|error| error.to_string())?;
    let event_counts = CompanyViewEventCounts {
        upcoming: events.len() as i64,
    };

    Ok(CompanyViewCounters {
        signals,
        claims: claim_counts,
        shorts: short_counts,
        events: event_counts,
    })
}

fn compute_kpi(state: &AppState, company_id: &str) -> Result<CompanyViewKpi, String> {
    let periods = state
        .financials()
        .list_financial_periods(ListFinancialPeriodsInput {
            company_id: company_id.to_owned(),
            fiscal_year: None,
        })
        .map_err(|error| error.to_string())?;

    let mut fy_years: Vec<i64> = periods
        .iter()
        .filter(|period| period.period_type == "FY")
        .map(|period| period.fiscal_year)
        .collect::<BTreeSet<_>>()
        .into_iter()
        .collect();
    if fy_years.len() > KPI_YEARS {
        fy_years = fy_years.split_off(fy_years.len() - KPI_YEARS);
    }
    let years = fy_years;

    let mut period_by_year: HashMap<i64, String> = HashMap::new();
    for period in &periods {
        if period.period_type == "FY" && years.contains(&period.fiscal_year) {
            period_by_year
                .entry(period.fiscal_year)
                .or_insert_with(|| period.id.clone());
        }
    }

    let mut facts_by_period: HashMap<String, Vec<FinancialFact>> = HashMap::new();
    for period_id in period_by_year.values() {
        if facts_by_period.contains_key(period_id) {
            continue;
        }
        let facts = state
            .financials()
            .list_financial_facts(ListFinancialFactsInput {
                company_id: Some(company_id.to_owned()),
                period_id: Some(period_id.clone()),
                definition_id: None,
            })
            .map_err(|error| error.to_string())?;
        facts_by_period.insert(period_id.clone(), facts);
    }

    let mut currency: Option<String> = None;
    let mut rows = Vec::new();
    for metric_key in KPI_METRIC_KEYS {
        let mut cells = Vec::new();
        for &year in &years {
            let facts = period_by_year
                .get(&year)
                .and_then(|period_id| facts_by_period.get(period_id));
            // Preferred fact for (metric, year): confirmed over any other
            // state, newest `created_at` among equally-preferred candidates.
            let best = facts.and_then(|facts| {
                facts
                    .iter()
                    .filter(|fact| fact.metric_key == metric_key)
                    .max_by(|a, b| {
                        let a_confirmed = a.confirmation_state == "confirmed";
                        let b_confirmed = b.confirmation_state == "confirmed";
                        a_confirmed
                            .cmp(&b_confirmed)
                            .then_with(|| a.created_at.cmp(&b.created_at))
                    })
            });
            let cell = match best {
                Some(fact) => {
                    if currency.is_none() {
                        currency = fact.currency.clone();
                    }
                    CompanyViewKpiCell {
                        fiscal_year: year,
                        value_numeric: Some(fact.value_numeric.clone()),
                        source_document_ref: fact.source_document_ref.clone(),
                    }
                }
                None => CompanyViewKpiCell {
                    fiscal_year: year,
                    value_numeric: None,
                    source_document_ref: None,
                },
            };
            cells.push(cell);
        }
        let yoy_pct = compute_yoy(&cells);
        rows.push(CompanyViewKpiRow {
            metric_key: metric_key.to_owned(),
            cells,
            yoy_pct,
        });
    }

    Ok(CompanyViewKpi {
        currency,
        rows,
        years,
    })
}

fn compute_feed(state: &AppState, company_id: &str) -> Result<Vec<CompanyViewFeedItem>, String> {
    let rows = state
        .feed()
        .list_company_feed_newest(company_id, FEED_LIMIT)
        .map_err(|error| error.to_string())?;
    Ok(rows
        .into_iter()
        .map(|row| CompanyViewFeedItem {
            feed_item_id: row.feed_item_id,
            title: row.title,
            published_at: row.published_at,
            read: row.read,
            item_type: row.item_type,
            source_name: row.source_name,
            presentation_kind: row.presentation_kind,
        })
        .collect())
}

/// Price semantics (plan `frontend-v2-f3a.md` § S1, normative): reuse
/// `compute_price_context`'s series, then slice/compute here rather than
/// asking that read model for a differently-shaped answer.
fn compute_price(state: &AppState, company_id: &str) -> Result<CompanyViewPrice, String> {
    let context = market_data::compute_price_context(state, company_id)?;
    if context.empty_reason.is_some() {
        return Ok(CompanyViewPrice {
            candles: Vec::new(),
            last_close: 0.0,
            as_of: String::new(),
            delta_1m_pct: None,
            delta_ytd_pct: None,
            currency: context.currency,
            empty_reason: context.empty_reason,
        });
    }

    let history = context.history;
    let len = history.len();
    let last_close = history.last().map(|bar| bar.close).unwrap_or(0.0);
    let as_of = history
        .last()
        .map(|bar| bar.date.clone())
        .unwrap_or_default();

    let delta_1m_pct = if len > DELTA_1M_SESSIONS_BACK {
        let base = history[len - 1 - DELTA_1M_SESSIONS_BACK].close;
        (base != 0.0).then(|| (last_close - base) / base * 100.0)
    } else {
        None
    };

    let as_of_year = as_of.get(..4);
    let delta_ytd_pct = as_of_year.and_then(|as_of_year| {
        history
            .iter()
            .rev()
            .find(|bar| {
                bar.date
                    .get(..4)
                    .map(|year| year < as_of_year)
                    .unwrap_or(false)
            })
            .filter(|bar| bar.close != 0.0)
            .map(|bar| (last_close - bar.close) / bar.close * 100.0)
    });

    let take_n = len.min(PRICE_SESSIONS);
    let candles = history[len - take_n..]
        .iter()
        .map(|bar| CompanyViewCandle {
            date: bar.date.clone(),
            open: bar.open,
            high: bar.high,
            low: bar.low,
            close: bar.close,
        })
        .collect();

    Ok(CompanyViewPrice {
        candles,
        last_close,
        as_of,
        delta_1m_pct,
        delta_ytd_pct,
        currency: context.currency,
        empty_reason: None,
    })
}

/// Compute the Company View read model (sync core, unit-testable). `Err`
/// ONLY for an unknown company; every other section degrades independently
/// into `sectionErrors` (ADR 0081 Partial).
pub fn compute_company_view(state: &AppState, company_id: &str) -> Result<CompanyView, String> {
    let company = state
        .list_companies()
        .map_err(|error| error.to_string())?
        .into_iter()
        .find(|company| company.id == company_id)
        .ok_or_else(|| format!("no tracked company for id {company_id}"))?;

    let today = format_date(today_date());
    let horizon =
        format_date(today_date().saturating_add(time::Duration::days(EVENTS_WINDOW_DAYS)));

    let mut section_errors = CompanyViewSectionErrors::default();

    let counters = match compute_counters(state, company_id, &today, &horizon) {
        Ok(value) => Some(value),
        Err(_) => {
            section_errors.counters = Some(SectionErrorKind::Unavailable);
            None
        }
    };

    let kpi = match compute_kpi(state, company_id) {
        Ok(value) => Some(value),
        Err(_) => {
            section_errors.kpi = Some(SectionErrorKind::Unavailable);
            None
        }
    };

    let feed = match compute_feed(state, company_id) {
        Ok(value) => value,
        Err(_) => {
            section_errors.feed = Some(SectionErrorKind::Unavailable);
            Vec::new()
        }
    };

    let price = match compute_price(state, company_id) {
        Ok(value) => Some(value),
        Err(_) => {
            section_errors.price = Some(SectionErrorKind::Unavailable);
            None
        }
    };

    let coverage = match fundamentals_coverage::compute_fundamentals_coverage(state, company_id) {
        Ok(value) => value.periods,
        Err(_) => {
            section_errors.coverage = Some(SectionErrorKind::Unavailable);
            Vec::new()
        }
    };

    let recommendations = match state
        .analyst_recommendations()
        .list_analyst_recommendations(company_id)
    {
        Ok(rows) => rows.into_iter().take(RECOMMENDATIONS_LIMIT).collect(),
        Err(_) => {
            section_errors.recommendations = Some(SectionErrorKind::Unavailable);
            Vec::new()
        }
    };

    Ok(CompanyView {
        company_id: company.id,
        qualified_ticker: company.qualified_ticker,
        display_name: company.display_name,
        isin: company.isin,
        counters,
        kpi,
        feed,
        price,
        coverage,
        recommendations,
        section_errors,
    })
}

/// Composed Company View read model for the Spółka screen (F3a #429, ADR
/// 0107 dec. 3). Offloaded off the UI thread — composes six independent
/// domain stores.
#[tauri::command]
pub async fn get_company_view(
    company_id: String,
    state: tauri::State<'_, AppState>,
) -> Result<CompanyView, String> {
    let state = state.inner().clone();
    tauri::async_runtime::spawn_blocking(move || compute_company_view(&state, &company_id))
        .await
        .map_err(|error| format!("company view task failed: {error}"))?
}

#[cfg(test)]
#[path = "company_view_tests.rs"]
mod tests;
