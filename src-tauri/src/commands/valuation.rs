//! Comparative valuation L1 (ADR 0089 dec. 4-5, v0.61 §B2).
//!
//! `compute_comparative_valuation` computes the peer-multiple implied fair-value
//! ranges + confidence grade for one company and **appends** a `valuation_runs`
//! row per method whose input signature differs from that method's latest stored
//! run (never a row per render). `list_valuation_runs` reads the append-only
//! history newest-first by the domain `data_as_of` date. The valuation math lives
//! in the pure [`crate::valuation`] transform; this command is the DB seam:
//!
//! - **peer multiples** come from the existing level-0 ratio path
//!   (`compute_price_context` per tracked sector peer — no ratio-formula
//!   re-derivation, the B1 reuse pattern);
//! - **target drivers** (`net_profit_ttm`, `ebitda_ttm`, `total_equity`,
//!   `net_debt`, `shares_outstanding`) come from the derived-metrics resolver
//!   over confirmed facts;
//! - **current price** + the **`data_as_of`** domain date come from the target's
//!   own price context (falling back to the latest fundamentals period end when
//!   no quote resolves).
//!
//! Work is offloaded off the UI thread (`spawn_blocking`) and bounded: one
//! indexed sector scan plus indexed per-peer reads. Decision support only — the
//! output is ranges, percentiles, and a grade; never buy/sell/hold language.

use rust_decimal::prelude::FromPrimitive;
use rust_decimal::Decimal;
use serde::Serialize;

use crate::app_state;
use crate::commands::market_data::compute_price_context;
use crate::fundamentals::expr::MetricResolver;
use crate::storage::{ListFinancialPeriodsInput, NewValuationRun, StoredValuationRun};
use crate::valuation::{
    compute_comparative_valuation as compute_pure, ComparativeValuation, PeerMultiples,
    ValuationDrivers, ValuationMethodResult,
};

/// The five per-share drivers whose provenance backs the `validation` grade
/// component (all resolve from confirmed data only).
const DRIVER_COUNT: usize = 5;

/// Round a sampled f64 ratio to a stable precision (kills `f64→Decimal` noise so
/// equal peer multiples compare equal), then normalize.
fn sample_value(value: f64) -> Option<Decimal> {
    Decimal::from_f64(value).map(|d| d.round_dp(6).normalize())
}

/// Case-insensitive sector fold (registry vs manual taxonomies differ in casing).
fn fold(sector: &str) -> String {
    sector.trim().to_lowercase()
}

/// One peer's level-0 multiples via the shared price-context read model (reuse,
/// no formula re-derivation). An empty context contributes no multiples.
fn peer_multiples(state: &app_state::AppState, company_id: &str) -> Result<PeerMultiples, String> {
    let mut out = PeerMultiples::new(company_id);
    let context = compute_price_context(state, company_id)?;
    if context.empty_reason.is_none() {
        out.pe = context.ratios.pe.and_then(sample_value);
        out.pbv = context.ratios.pbv.and_then(sample_value);
        out.ev_ebitda = context.ratios.ev_ebitda.and_then(sample_value);
    }
    Ok(out)
}

/// The target's per-share value drivers from the derived-metrics resolver
/// (confirmed facts only). Returns the drivers plus the `validation` ratio: the
/// share of the five driver facts that resolve from confirmed data.
fn target_drivers(
    state: &app_state::AppState,
    company_id: &str,
) -> Result<(ValuationDrivers, Decimal), String> {
    let context = state
        .quality_frameworks()
        .metrics_context(company_id)
        .map_err(|error| error.to_string())?;
    let resolver = context.resolver();
    let drivers = ValuationDrivers {
        shares_outstanding: resolver.value("shares_outstanding"),
        net_profit_ttm: resolver.value("net_profit_ttm"),
        total_equity: resolver.value("total_equity"),
        ebitda_ttm: resolver.value("ebitda_ttm"),
        net_debt: resolver.value("net_debt"),
    };
    let present = [
        drivers.shares_outstanding,
        drivers.net_profit_ttm,
        drivers.total_equity,
        drivers.ebitda_ttm,
        drivers.net_debt,
    ]
    .iter()
    .filter(|v| v.is_some())
    .count();
    let validation = Decimal::from(present as i64) / Decimal::from(DRIVER_COUNT as i64);
    Ok((drivers, validation))
}

/// The latest fundamentals period-end date (max), for the `data_as_of` fallback
/// when the target has no resolvable quote.
fn latest_period_end(state: &app_state::AppState, company_id: &str) -> Option<String> {
    state
        .financials()
        .list_financial_periods(ListFinancialPeriodsInput {
            company_id: company_id.to_owned(),
            fiscal_year: None,
        })
        .ok()?
        .into_iter()
        .filter_map(|period| period.period_end_date)
        .max()
}

/// The canonical per-method input signature (the persisted `inputs_json`): a
/// deterministic serialization of everything that determines the method's
/// output. Two computes with an identical signature append no new row.
#[derive(Serialize)]
struct MethodSignature<'a> {
    method: &'a str,
    driver_key: &'a str,
    driver_value: Option<&'a str>,
    peer_multiple_low: Option<&'a str>,
    peer_multiple_base: Option<&'a str>,
    peer_multiple_high: Option<&'a str>,
    peer_sample_size: u32,
    data_as_of: &'a str,
}

fn method_signature(method: &ValuationMethodResult, data_as_of: &str) -> String {
    let sig = MethodSignature {
        method: method.method.as_str(),
        driver_key: &method.driver_key,
        driver_value: method.driver_value.as_deref(),
        peer_multiple_low: method.peer_multiple_low.as_deref(),
        peer_multiple_base: method.peer_multiple_base.as_deref(),
        peer_multiple_high: method.peer_multiple_high.as_deref(),
        peer_sample_size: method.peer_sample_size,
        data_as_of,
    };
    serde_json::to_string(&sig).unwrap_or_default()
}

/// Compute the comparative valuation AND persist a run per method whose input
/// signature changed — the shared helper the command wrapper offloads and the
/// MCP act handler calls (so no path diverges).
pub fn compute_and_persist_comparative_valuation(
    state: &app_state::AppState,
    company_id: &str,
) -> Result<ComparativeValuation, String> {
    // The company's own sector (registry or manual). No sector ⇒ typed empty.
    let (sector, _source) = state
        .companies()
        .get_company_sector(company_id)
        .map_err(|error| error.to_string())?;
    let sector = sector
        .map(|s| s.trim().to_owned())
        .filter(|s| !s.is_empty());

    // Peer set: tracked companies sharing the sector (case-insensitive), the
    // company itself included — one indexed scan. Empty when unclassified.
    let peer_ids: Vec<String> = match &sector {
        Some(sector) => {
            let target_fold = fold(sector);
            state
                .companies()
                .list_companies_with_sector()
                .map_err(|error| error.to_string())?
                .into_iter()
                .filter_map(|(id, company_sector)| {
                    company_sector
                        .filter(|value| fold(value) == target_fold)
                        .map(|_| id)
                })
                .collect()
        }
        None => Vec::new(),
    };
    let peer_count = peer_ids.len() as u32;

    let mut multiples = Vec::with_capacity(peer_ids.len());
    for id in &peer_ids {
        multiples.push(peer_multiples(state, id)?);
    }

    let (drivers, validation) = target_drivers(state, company_id)?;

    // Current price + the domain as-of date from the target's own price context;
    // fall back to the latest fundamentals period end when no quote resolves.
    let price = compute_price_context(state, company_id)?;
    let (current_price, price_date) = if price.empty_reason.is_none() {
        (sample_value(price.last_close), Some(price.last_date))
    } else {
        (None, None)
    };
    let data_as_of = price_date
        .or_else(|| latest_period_end(state, company_id))
        .unwrap_or_default();

    let valuation = compute_pure(
        company_id,
        sector.as_deref(),
        current_price,
        &data_as_of,
        peer_count,
        &drivers,
        &multiples,
        validation,
    );

    persist_runs(state, &valuation)?;
    Ok(valuation)
}

/// Append a run per method that produced a fair-value range, when its input
/// signature differs from that method's latest stored run (ADR 0089 dec. 5).
fn persist_runs(
    state: &app_state::AppState,
    valuation: &ComparativeValuation,
) -> Result<(), String> {
    let store = state.valuation_runs();
    let grade = valuation.confidence.grade.as_str().to_owned();
    for method in &valuation.methods {
        // Only a method that produced a range is worth a history row.
        if method.fair_base.is_none() {
            continue;
        }
        let method_str = method.method.as_str();
        let inputs_json = method_signature(method, &valuation.data_as_of);

        let latest = store
            .latest_run_for_method(&valuation.company_id, method_str)
            .map_err(|error| error.to_string())?;
        if latest.map(|run| run.inputs_json) == Some(inputs_json.clone()) {
            continue; // unchanged signature — no append (no row per render).
        }

        store
            .insert_run(&NewValuationRun {
                company_id: valuation.company_id.clone(),
                method: method_str.to_owned(),
                inputs_json,
                fair_low: method.fair_low.clone(),
                fair_base: method.fair_base.clone(),
                fair_high: method.fair_high.clone(),
                data_as_of: valuation.data_as_of.clone(),
                confidence_grade: grade.clone(),
            })
            .map_err(|error| error.to_string())?;
    }
    Ok(())
}

/// Comparative valuation for one company (ADR 0089 dec. 4). Offloaded off the UI
/// thread: it derives the sector peer set, reads each peer's confirmed multiples,
/// resolves the target's drivers, and persists a run per changed method.
#[tauri::command]
pub async fn compute_comparative_valuation(
    company_id: String,
    state: tauri::State<'_, app_state::AppState>,
) -> Result<ComparativeValuation, String> {
    let state = state.inner().clone();
    tauri::async_runtime::spawn_blocking(move || {
        compute_and_persist_comparative_valuation(&state, &company_id)
    })
    .await
    .map_err(|error| format!("comparative valuation task failed: {error}"))?
}

/// The append-only valuation-run history for one company (ADR 0089 dec. 5),
/// newest-first by the domain `data_as_of` date.
#[tauri::command]
pub async fn list_valuation_runs(
    company_id: String,
    state: tauri::State<'_, app_state::AppState>,
) -> Result<Vec<StoredValuationRun>, String> {
    let state = state.inner().clone();
    tauri::async_runtime::spawn_blocking(move || {
        state
            .valuation_runs()
            .list_runs(&company_id)
            .map_err(|error| error.to_string())
    })
    .await
    .map_err(|error| format!("list valuation runs task failed: {error}"))?
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::source_adapters::yahoo_eod::DailyQuote;
    use crate::storage::{
        open_in_memory_database, AppState, NewCompany, NewFinancialFact, NewFinancialPeriod,
    };
    use crate::valuation::ValuationMethod;

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

    /// Seed a company so its P/BV resolves (one quote, shares, equity), in a
    /// sector, and return its id. P/BV = (close × shares) / total_equity.
    fn seed_peer(state: &AppState, ticker: &str, sector: &str, close: f64, equity: &str) -> String {
        let id = company(state, ticker);
        state.set_company_sector(&id, Some(sector)).expect("sector");
        state
            .market_data()
            .upsert_quotes(
                &id,
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
            .expect("quote");
        let period = state
            .financials()
            .create_financial_period(NewFinancialPeriod {
                company_id: id.clone(),
                fiscal_year: 2025,
                period_type: "FY".to_owned(),
                period_end_date: Some("2025-12-31".to_owned()),
                report_evidence_ref: None,
            })
            .expect("period")
            .id;
        for (definition_id, value) in [
            ("kpidef_shares_outstanding", "100"),
            ("kpidef_total_equity", equity),
        ] {
            state
                .financials()
                .create_financial_fact(NewFinancialFact {
                    company_id: id.clone(),
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
        id
    }

    #[test]
    fn company_without_a_sector_is_typed_empty_and_persists_nothing() {
        let s = state();
        let c = company(&s, "TST");
        let result = compute_and_persist_comparative_valuation(&s, &c).expect("compute");
        assert_eq!(
            result.empty_reason,
            Some(crate::valuation::ValuationEmptyReason::NoSector)
        );
        assert!(s.valuation_runs().list_runs(&c).expect("list").is_empty());
    }

    #[test]
    fn a_pbv_valuation_persists_one_run_and_is_signature_gated() {
        let s = state();
        // Four peers with distinct P/BV in one sector; the target is peer A.
        let target = seed_peer(&s, "AAA", "oprogramowanie", 200.0, "10000");
        seed_peer(&s, "BBB", "oprogramowanie", 200.0, "20000");
        seed_peer(&s, "CCC", "oprogramowanie", 200.0, "40000");
        seed_peer(&s, "DDD", "oprogramowanie", 200.0, "80000");

        let first = compute_and_persist_comparative_valuation(&s, &target).expect("compute");
        let pbv = crate::valuation::method_of(&first, ValuationMethod::PbvMultiple).expect("pbv");
        assert_eq!(pbv.absent_reason, None, "P/BV should compute over 3 peers");
        assert!(pbv.fair_base.is_some());

        let runs = s.valuation_runs().list_runs(&target).expect("list");
        let pbv_runs = runs.iter().filter(|r| r.method == "pbv_multiple").count();
        assert_eq!(pbv_runs, 1, "one P/BV run persisted");

        // Re-running with identical inputs appends nothing (signature unchanged).
        compute_and_persist_comparative_valuation(&s, &target).expect("recompute");
        let runs2 = s.valuation_runs().list_runs(&target).expect("list");
        assert_eq!(
            runs2.iter().filter(|r| r.method == "pbv_multiple").count(),
            1,
            "identical signature must not append a second P/BV run"
        );
    }

    #[test]
    fn a_changed_input_appends_a_second_run() {
        let s = state();
        let target = seed_peer(&s, "AAA", "oprogramowanie", 200.0, "10000");
        seed_peer(&s, "BBB", "oprogramowanie", 200.0, "20000");
        seed_peer(&s, "CCC", "oprogramowanie", 200.0, "40000");
        compute_and_persist_comparative_valuation(&s, &target).expect("first");

        // Change a peer's price → its P/BV multiple shifts → the target's peer
        // dispersion (and thus the signature) changes → a second run appends.
        let ddd = seed_peer(&s, "DDD", "oprogramowanie", 400.0, "20000");
        let _ = ddd;
        compute_and_persist_comparative_valuation(&s, &target).expect("second");

        let runs = s.valuation_runs().list_runs(&target).expect("list");
        assert!(
            runs.iter().filter(|r| r.method == "pbv_multiple").count() >= 2,
            "a changed input signature appends a second run"
        );
    }
}
