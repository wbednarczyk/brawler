//! Sector-percentile read model (ADR 0089 dec. 3, v0.61 §B1).
//!
//! `get_sector_percentiles` answers "where does this company stand against its
//! sector peers?" — for each level-0 market ratio and selected canonical KPI, the
//! company's rank-based percentile among the **tracked** companies sharing its
//! `companies.sector`. The peer set is derived at read time (no stored
//! projection, no new tables); the percentile math lives in the pure
//! [`crate::fundamentals::sector_percentiles`] transform. This command is the
//! thin DB seam: it derives the peer set, evaluates each peer's metrics through
//! the **existing** derived-metrics paths (`compute_price_context` for the
//! level-0 market ratios — no ratio-formula re-derivation — and the metrics
//! resolver for the canonical KPIs), and offloads the assembly off the UI thread.
//!
//! Every value read is confirmed/validated data (the resolver and price-context
//! paths read `confirmation_state = 'confirmed'` facts only). Work is bounded:
//! one indexed sector scan plus indexed per-peer reads — never a corpus recompute.

use std::collections::BTreeMap;

use rust_decimal::prelude::FromPrimitive;
use rust_decimal::Decimal;

use crate::app_state;
use crate::fundamentals::expr::MetricResolver;
use crate::fundamentals::sector_percentiles::{
    compute_sector_percentiles, MetricSpec, PeerSample, SectorMetricKind, SectorPercentiles,
};

/// Level-0 market ratios ranked (ADR 0089 dec. 3), sourced from the existing
/// `compute_price_context` path — the ratio formulas are NOT re-derived here.
const MARKET_RATIO_KEYS: &[&str] = &[
    "pe_ratio",
    "pbv_ratio",
    "ev_ebitda",
    "dividend_yield",
    "fcf_yield",
];

/// Selected canonical KPIs ranked, sourced from the derived-metrics resolver:
/// profitability/returns + one leverage ratio, all cross-company comparable.
const CANONICAL_KPI_KEYS: &[&str] = &["roe", "roa", "roic", "fcf_margin", "net_debt_to_ebitda"];

/// Round a sampled value to a stable display precision (kills `f64→Decimal`
/// noise on the market ratios; a no-op on the already-exact resolver decimals),
/// so equal values compare equal in the mid-rank tie count.
fn sample_value(value: Decimal) -> Decimal {
    value.round_dp(6).normalize()
}

/// The ordered metric set: market ratios first, then canonical KPIs.
fn metric_specs() -> Vec<MetricSpec> {
    MARKET_RATIO_KEYS
        .iter()
        .map(|key| MetricSpec::new(key, SectorMetricKind::MarketRatio))
        .chain(
            CANONICAL_KPI_KEYS
                .iter()
                .map(|key| MetricSpec::new(key, SectorMetricKind::CanonicalKpi)),
        )
        .collect()
}

/// Case-insensitive sector fold (registry vs manual taxonomies differ in casing).
fn fold(sector: &str) -> String {
    sector.trim().to_lowercase()
}

/// Evaluate one peer's ranked metrics through the existing derived-metrics paths.
fn peer_sample(state: &app_state::AppState, company_id: &str) -> Result<PeerSample, String> {
    let mut values: BTreeMap<String, Decimal> = BTreeMap::new();

    // Level-0 market ratios via the shared price-context read model (reuse, no
    // formula re-derivation). An empty context (unmapped exchange / no quotes)
    // simply contributes no ratios.
    let context = crate::commands::market_data::compute_price_context(state, company_id)?;
    if context.empty_reason.is_none() {
        let ratios = &context.ratios;
        for (key, value) in [
            ("pe_ratio", ratios.pe),
            ("pbv_ratio", ratios.pbv),
            ("ev_ebitda", ratios.ev_ebitda),
            ("dividend_yield", ratios.div_yield),
            ("fcf_yield", ratios.fcf_yield),
        ] {
            if let Some(decimal) = value.and_then(Decimal::from_f64) {
                values.insert(key.to_owned(), sample_value(decimal));
            }
        }
    }

    // Selected canonical KPIs via the derived-metrics resolver (latest period,
    // confirmed facts only).
    let metrics_context = state
        .quality_frameworks()
        .metrics_context(company_id)
        .map_err(|error| error.to_string())?;
    let resolver = metrics_context.resolver();
    for key in CANONICAL_KPI_KEYS {
        if let Some(decimal) = resolver.value(key) {
            values.insert((*key).to_owned(), sample_value(decimal));
        }
    }

    Ok(PeerSample {
        company_id: company_id.to_owned(),
        values,
    })
}

/// Assemble the sector-percentile read model (the same helper the command
/// wrapper offloads and the fidelity corpus / MCP handler call, so no path
/// diverges).
pub fn compute_company_sector_percentiles(
    state: &app_state::AppState,
    company_id: &str,
) -> Result<SectorPercentiles, String> {
    let specs = metric_specs();

    // The company's own sector (registry or manual). No sector ⇒ typed empty.
    let (sector, _source) = state
        .companies()
        .get_company_sector(company_id)
        .map_err(|error| error.to_string())?;
    let sector = sector
        .map(|s| s.trim().to_owned())
        .filter(|s| !s.is_empty());
    let Some(sector) = sector else {
        return Ok(compute_sector_percentiles(company_id, None, &specs, &[]));
    };

    // Peer set: tracked companies sharing the sector (case-insensitive), the
    // company itself included — one indexed scan.
    let target_fold = fold(&sector);
    let peer_ids: Vec<String> = state
        .companies()
        .list_companies_with_sector()
        .map_err(|error| error.to_string())?
        .into_iter()
        .filter_map(|(id, company_sector)| {
            company_sector
                .filter(|value| fold(value) == target_fold)
                .map(|_| id)
        })
        .collect();

    let mut samples = Vec::with_capacity(peer_ids.len());
    for id in &peer_ids {
        samples.push(peer_sample(state, id)?);
    }

    Ok(compute_sector_percentiles(
        company_id,
        Some(&sector),
        &specs,
        &samples,
    ))
}

/// Sector percentiles for one company (ADR 0089 dec. 3). Offloaded off the UI
/// thread: it derives the sector peer set and reads each peer's confirmed facts +
/// quotes through the existing derived-metrics paths.
#[tauri::command]
pub async fn get_sector_percentiles(
    company_id: String,
    state: tauri::State<'_, app_state::AppState>,
) -> Result<SectorPercentiles, String> {
    let state = state.inner().clone();
    tauri::async_runtime::spawn_blocking(move || {
        compute_company_sector_percentiles(&state, &company_id)
    })
    .await
    .map_err(|error| format!("sector percentiles task failed: {error}"))?
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::fundamentals::sector_percentiles::{
        metric_of, MetricAbsentReason, SectorPercentileEmptyReason,
    };
    use crate::source_adapters::yahoo_eod::DailyQuote;
    use crate::storage::{
        open_in_memory_database, AppState, NewCompany, NewFinancialFact, NewFinancialPeriod,
    };

    fn state() -> AppState {
        AppState::new(open_in_memory_database().expect("in-memory db"))
    }

    fn company(state: &AppState, ticker: &str) -> String {
        state
            .create_company(NewCompany {
                exchange: "GPW".to_owned(),
                ticker: ticker.to_owned(),
                display_name: format!("{ticker} S.A."),
                isin: None,
                cik: None,
                lei: None,
            })
            .expect("company")
            .id
    }

    /// Seed a company so its P/BV ratio resolves: one quote (close), a
    /// `shares_outstanding` fact (→ market_cap) and a `total_equity` fact. P/BV =
    /// (close × shares) / total_equity, so distinct `total_equity` ⇒ distinct P/BV.
    fn seed_pbv(state: &AppState, company_id: &str, close: f64, shares: &str, total_equity: &str) {
        state
            .market_data()
            .upsert_quotes(
                company_id,
                &[DailyQuote {
                    date: "2026-07-14".to_owned(),
                    open: close,
                    high: close,
                    low: close,
                    close,
                    volume: 1000,
                }],
                "yahoo-eod",
                "2026-07-14T18:00:00Z",
            )
            .expect("upsert quote");
        let period = state
            .financials()
            .create_financial_period(NewFinancialPeriod {
                company_id: company_id.to_owned(),
                fiscal_year: 2025,
                period_type: "FY".to_owned(),
                period_end_date: Some("2025-12-31".to_owned()),
                report_evidence_ref: None,
            })
            .expect("period")
            .id;
        for (definition_id, value) in [
            ("kpidef_shares_outstanding", shares),
            ("kpidef_total_equity", total_equity),
        ] {
            state
                .financials()
                .create_financial_fact(NewFinancialFact {
                    company_id: company_id.to_owned(),
                    period_id: period.clone(),
                    definition_id: definition_id.to_owned(),
                    value_numeric: value.to_owned(),
                    currency: Some("PLN".to_owned()),
                    statement_basis: Some("consolidated".to_owned()),
                    attribution: Some("total".to_owned()),
                    variant: Some("reported".to_owned()),
                    measure_window: None,
                    data_quality: Some("final".to_owned()),
                    as_reported_value: None,
                    as_reported_scale: None,
                    reporting_standard: None,
                    extraction_method: Some("manual".to_owned()),
                    confidence: None,
                    confirmation_state: Some("confirmed".to_owned()),
                    supersedes_id: None,
                    source_document_ref: None,
                })
                .expect("fact");
        }
    }

    #[test]
    fn company_without_a_sector_reports_no_sector() {
        let s = state();
        let c = company(&s, "TST");
        let result = compute_company_sector_percentiles(&s, &c).expect("compute");
        assert_eq!(
            result.empty_reason,
            Some(SectorPercentileEmptyReason::NoSector)
        );
        assert_eq!(result.peer_count, 0);
        assert!(result.thin);
        assert!(result.metrics.is_empty());
    }

    #[test]
    fn four_peers_in_one_sector_rank_the_market_ratio() {
        let s = state();
        // Four companies in "oprogramowanie" with distinct total_equity → distinct
        // P/BV (close × shares = 200 × 100 = 20000 market cap for all).
        let equities = [
            ("A", "10000"),
            ("B", "20000"),
            ("C", "40000"),
            ("D", "80000"),
        ];
        let mut ids = Vec::new();
        for (ticker, equity) in equities {
            let id = company(&s, ticker);
            s.set_company_sector(&id, Some("oprogramowanie"))
                .expect("set sector");
            seed_pbv(&s, &id, 200.0, "100", equity);
            ids.push(id);
        }
        // Target = A: P/BV = 20000/10000 = 2.0, the HIGHEST of {2.0,1.0,0.5,0.25}.
        let result = compute_company_sector_percentiles(&s, &ids[0]).expect("compute");
        assert_eq!(result.peer_count, 4);
        assert!(!result.thin, "4 companies is not thin");
        assert_eq!(result.sector.as_deref(), Some("oprogramowanie"));

        let pbv = metric_of(&result, "pbv_ratio").expect("pbv row");
        assert_eq!(pbv.value.as_deref(), Some("2"));
        assert_eq!(pbv.sample_size, 4);
        assert_eq!(pbv.absent_reason, None);
        // sorted 0.25,0.5,1,2; target 2 highest → L=3,E=1,N=4 → (7)/(8)*100 = 87.5.
        assert_eq!(pbv.percentile.as_deref(), Some("87.5"));
    }

    #[test]
    fn three_peers_flag_thin_but_still_rank() {
        let s = state();
        let equities = [("A", "10000"), ("B", "20000"), ("C", "40000")];
        let mut ids = Vec::new();
        for (ticker, equity) in equities {
            let id = company(&s, ticker);
            s.set_company_sector(&id, Some("nowe technologie"))
                .expect("set sector");
            seed_pbv(&s, &id, 200.0, "100", equity);
            ids.push(id);
        }
        let result = compute_company_sector_percentiles(&s, &ids[0]).expect("compute");
        assert_eq!(result.peer_count, 3);
        assert!(result.thin, "3 companies is thin");
        // Still ranked (thin ≠ absent): pbv defined for all three.
        let pbv = metric_of(&result, "pbv_ratio").expect("pbv row");
        assert_eq!(pbv.sample_size, 3);
        assert_eq!(pbv.absent_reason, None);
    }

    #[test]
    fn a_metric_no_peer_reports_is_insufficient_or_no_company_value() {
        let s = state();
        // Two companies with P/BV, target present. Only the target + one peer have
        // it defined ⇒ InsufficientPeers for pbv; and no company has dividend data
        // ⇒ NoCompanyValue for dividend_yield.
        let a = company(&s, "A");
        let b = company(&s, "B");
        s.set_company_sector(&a, Some("banki")).expect("sector a");
        s.set_company_sector(&b, Some("banki")).expect("sector b");
        seed_pbv(&s, &a, 200.0, "100", "10000");
        seed_pbv(&s, &b, 200.0, "100", "20000");

        let result = compute_company_sector_percentiles(&s, &a).expect("compute");
        assert_eq!(result.peer_count, 2);
        let pbv = metric_of(&result, "pbv_ratio").expect("pbv row");
        assert_eq!(pbv.value.as_deref(), Some("2")); // company value surfaced
        assert_eq!(
            pbv.absent_reason,
            Some(MetricAbsentReason::InsufficientPeers)
        );
        let div = metric_of(&result, "dividend_yield").expect("dividend row");
        assert_eq!(div.absent_reason, Some(MetricAbsentReason::NoCompanyValue));
    }
}
