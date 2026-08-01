//! Cross-company KPI comparison read model (ADR 0089 dec. 1, v0.61 §A2).
//!
//! `get_kpi_comparison` answers "the same canonical KPI across these companies,
//! aligned on a shared period axis, with QoQ/YoY deltas and PLN conversion" —
//! the substrate behind both the Compare screen (N companies) and the
//! Fundamentals periods×deltas table (N=1). The pure alignment/labeling/delta
//! transform lives in [`crate::fundamentals::comparison`]; this command is the
//! thin DB seam: it selects the canonical confirmed fact per slot (reusing the
//! `load_period_facts` preference), fetches per-metric value kinds, wires the
//! NBP FX substrate, and offloads the assembly off the UI thread. It **reads
//! confirmed facts only** — it never re-parses report tables (rejected in the
//! v0.47 scope note; ADR 0089).

use std::collections::HashMap;

use rust_decimal::prelude::FromStr;
use rust_decimal::Decimal;
use serde::Deserialize;

use crate::app_state;
use crate::fundamentals::comparison::{
    compute_comparison, ComparisonFact, FxLookup, Granularity, KpiComparison, MetricMeta,
};
use crate::fx::{FxBasis, MeasureWindow};

/// Input for [`get_kpi_comparison`] (ADR 0089 dec. 1). `companyIds` of length 1
/// serves the Fundamentals table (same read model, N=1).
#[derive(Debug, Clone, Deserialize)]
#[cfg_attr(feature = "ts-export", derive(ts_rs::TS))]
#[cfg_attr(
    feature = "ts-export",
    ts(export, export_to = "../../src/api/generated/", optional_fields)
)]
#[serde(rename_all = "camelCase")]
pub struct KpiComparisonInput {
    /// Internal company ids to compare (1..N).
    pub company_ids: Vec<String>,
    /// Canonical `metric_key`s to align (1..N).
    pub metric_keys: Vec<String>,
    /// `"annual"` (FY periods) or `"quarterly"` (Q1–Q4). Unknown ⇒ annual.
    pub granularity: String,
}

/// The period types populating the axis for a granularity.
fn period_types(granularity: Granularity) -> &'static [&'static str] {
    match granularity {
        Granularity::Annual => &["FY"],
        Granularity::Quarterly => &["Q1", "Q2", "Q3", "Q4"],
    }
}

/// FX resolution backed by the `fx_rates` store (NBP Table-A mids, ADR 0089
/// dec. 2). A store error is treated as a missing rate (the cell flags
/// `fx_missing`) rather than failing the whole comparison.
struct StoreFx {
    fx: crate::storage::FxRatesStore,
}

impl FxLookup for StoreFx {
    fn resolve(
        &self,
        currency: &str,
        window: MeasureWindow,
        start: &str,
        end: &str,
    ) -> Option<(Decimal, FxBasis)> {
        match window {
            MeasureWindow::Flow => self
                .fx
                .period_average_mid(currency, start, end)
                .ok()
                .flatten()
                .map(|mid| (mid, FxBasis::PeriodAverage)),
            MeasureWindow::Stock => self
                .fx
                .latest_mid_on_or_before(currency, end)
                .ok()
                .flatten()
                .map(|mid| (mid, FxBasis::LatestOnOrBefore)),
        }
    }
}

/// Assemble the comparison read model (the same helper the command wrapper
/// offloads and the fidelity corpus / MCP handler call, so no path diverges).
pub fn compute_kpi_comparison(
    state: &app_state::AppState,
    input: &KpiComparisonInput,
) -> Result<KpiComparison, String> {
    let granularity = Granularity::parse(&input.granularity);
    let financials = state.financials();

    let rows = financials
        .comparison_facts(
            &input.company_ids,
            &input.metric_keys,
            period_types(granularity),
        )
        .map_err(|error| error.to_string())?;

    // A value_numeric that does not parse is skipped rather than failing the
    // whole read (same tolerance as load_period_facts).
    let facts: Vec<ComparisonFact> = rows
        .into_iter()
        .filter_map(|row| {
            let value = Decimal::from_str(row.value_numeric.trim()).ok()?;
            Some(ComparisonFact {
                company_id: row.company_id,
                metric_key: row.metric_key,
                fiscal_year: row.fiscal_year,
                period_type: row.period_type,
                period_end_date: row.period_end_date,
                fact_id: row.fact_id,
                value,
                currency: row.currency,
                measure_window: row.measure_window,
                validation_status: row.validation_status,
            })
        })
        .collect();

    let value_kinds = financials
        .metric_value_kinds(&input.metric_keys)
        .map_err(|error| error.to_string())?;
    let metric_meta: HashMap<String, MetricMeta> = value_kinds
        .into_iter()
        .map(|(key, kind)| {
            (
                key,
                MetricMeta {
                    value_kind: Some(kind),
                },
            )
        })
        .collect();

    let fx = StoreFx {
        fx: state.fx_rates(),
    };

    Ok(compute_comparison(
        granularity,
        &input.company_ids,
        &input.metric_keys,
        &facts,
        &metric_meta,
        &fx,
    ))
}

/// Cross-company comparison (ADR 0089 dec. 1). Offloaded off the UI thread: it
/// scans the requested companies' confirmed facts + periods + the FX substrate.
#[tauri::command]
pub async fn get_kpi_comparison(
    input: KpiComparisonInput,
    state: tauri::State<'_, app_state::AppState>,
) -> Result<KpiComparison, String> {
    let state = state.inner().clone();
    tauri::async_runtime::spawn_blocking(move || compute_kpi_comparison(&state, &input))
        .await
        .map_err(|error| format!("kpi comparison task failed: {error}"))?
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::fundamentals::comparison::{cell_at, series_of, CellFlag};
    use crate::storage::{
        open_in_memory_database, AppState, NewCompany, NewFinancialFact, NewFinancialPeriod,
        NewKpiDefinition,
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

    /// The definition id for `metric_key`, reusing the migration-seeded canonical
    /// definition when present (the in-memory DB seeds the canonical KPI catalog),
    /// otherwise creating one at the requested `value_kind`.
    fn define(state: &AppState, metric_key: &str, value_kind: &str) -> String {
        let existing = state
            .financials()
            .list_kpi_definitions(crate::storage::ListKpiDefinitionsInput {
                scope: None,
                sector: None,
                company_id: None,
            })
            .expect("list definitions")
            .into_iter()
            .find(|d| d.metric_key == metric_key);
        if let Some(def) = existing {
            return def.id;
        }
        state
            .financials()
            .create_kpi_definition(NewKpiDefinition {
                scope: "canonical".to_owned(),
                company_id: None,
                sector: None,
                metric_key: metric_key.to_owned(),
                label: metric_key.to_owned(),
                value_kind: value_kind.to_owned(),
                unit: None,
                computation: "reported".to_owned(),
                formula: None,
                display_format: None,
                origin: None,
                statement_group: None,
            })
            .expect("definition")
            .id
    }

    fn period(state: &AppState, company_id: &str, year: i64) -> String {
        state
            .financials()
            .create_financial_period(NewFinancialPeriod {
                company_id: company_id.to_owned(),
                fiscal_year: year,
                period_type: "FY".to_owned(),
                period_end_date: Some(format!("{year:04}-12-31")),
                report_evidence_ref: None,
            })
            .expect("period")
            .id
    }

    #[allow(clippy::too_many_arguments)]
    fn fact(
        state: &AppState,
        company_id: &str,
        period_id: &str,
        definition_id: &str,
        value: &str,
        currency: Option<&str>,
        statement_basis: &str,
        variant: &str,
    ) -> String {
        state
            .financials()
            .create_financial_fact(NewFinancialFact {
                company_id: company_id.to_owned(),
                period_id: period_id.to_owned(),
                definition_id: definition_id.to_owned(),
                value_numeric: value.to_owned(),
                currency: currency.map(str::to_owned),
                statement_basis: Some(statement_basis.to_owned()),
                attribution: Some("total".to_owned()),
                variant: Some(variant.to_owned()),
                measure_window: Some("flow".to_owned()),
                data_quality: Some("final".to_owned()),
                as_reported_value: None,
                as_reported_scale: None,
                reporting_standard: None,
                extraction_method: Some("manual".to_owned()),
                confidence: None,
                confirmation_state: Some("confirmed".to_owned()),
                supersedes_id: None,
                source_document_ref: None,
                annotation: None,
            })
            .expect("fact")
            .id
    }

    #[test]
    fn empty_company_yields_empty_axis_and_one_series_per_metric() {
        let s = state();
        let c = company(&s, "TST");
        let out = compute_kpi_comparison(
            &s,
            &KpiComparisonInput {
                company_ids: vec![c.clone()],
                metric_keys: vec!["revenue".to_owned()],
                granularity: "annual".to_owned(),
            },
        )
        .expect("comparison");
        assert!(out.axis.is_empty());
        assert_eq!(out.series.len(), 1);
        assert_eq!(out.series[0].company_id, c);
        assert!(out.series[0].cells.is_empty());
    }

    #[test]
    fn canonical_slot_selection_prefers_consolidated_reported() {
        // Two variants in the same slot: a standalone/adjusted noise fact and the
        // canonical consolidated/reported one. The read model must pick the latter,
        // matching load_period_facts (so the two paths never disagree).
        let s = state();
        let c = company(&s, "TST");
        let def = define(&s, "revenue", "monetary");
        let p = period(&s, &c, 2024);
        fact(
            &s,
            &c,
            &p,
            &def,
            "111",
            Some("PLN"),
            "standalone",
            "adjusted",
        );
        let canonical = fact(
            &s,
            &c,
            &p,
            &def,
            "999",
            Some("PLN"),
            "consolidated",
            "reported",
        );

        let out = compute_kpi_comparison(
            &s,
            &KpiComparisonInput {
                company_ids: vec![c.clone()],
                metric_keys: vec!["revenue".to_owned()],
                granularity: "annual".to_owned(),
            },
        )
        .expect("comparison");
        let cell = cell_at(series_of(&out, &c, "revenue").unwrap(), "2024:FY").unwrap();
        assert_eq!(cell.value.as_deref(), Some("999"));
        assert_eq!(cell.fact_id.as_deref(), Some(canonical.as_str()));
    }

    #[test]
    fn n1_yoy_over_the_db_matches_ground_truth_math() {
        // N=1 Fundamentals surface: two consecutive FY facts → a real YoY %.
        let s = state();
        let c = company(&s, "ACP");
        let def = define(&s, "revenue", "monetary");
        let p04 = period(&s, &c, 2004);
        let p05 = period(&s, &c, 2005);
        fact(
            &s,
            &c,
            &p04,
            &def,
            "482013000",
            Some("PLN"),
            "consolidated",
            "reported",
        );
        fact(
            &s,
            &c,
            &p05,
            &def,
            "533234000",
            Some("PLN"),
            "consolidated",
            "reported",
        );

        let out = compute_kpi_comparison(
            &s,
            &KpiComparisonInput {
                company_ids: vec![c.clone()],
                metric_keys: vec!["revenue".to_owned()],
                granularity: "annual".to_owned(),
            },
        )
        .expect("comparison");
        let cell = cell_at(series_of(&out, &c, "revenue").unwrap(), "2005:FY").unwrap();
        assert_eq!(cell.delta_yo_y.as_deref(), Some("10.63"));
        assert_eq!(cell.value_pln.as_deref(), Some("533234000"));
        assert_eq!(cell.fx_basis.as_deref(), Some("native_pln"));
    }

    #[test]
    fn eur_fact_converts_when_a_rate_exists() {
        use crate::fx::FxRate;
        let s = state();
        let c = company(&s, "EUC");
        let def = define(&s, "revenue", "monetary");
        let p = period(&s, &c, 2024);
        fact(
            &s,
            &c,
            &p,
            &def,
            "100",
            Some("EUR"),
            "consolidated",
            "reported",
        );
        // Two mids inside 2024 → period-average 4.25 (flow basis).
        s.fx_rates()
            .upsert_rates(
                &[
                    FxRate {
                        currency: "EUR".to_owned(),
                        date: "2024-06-14".to_owned(),
                        mid: Decimal::from_str("4.0").unwrap(),
                    },
                    FxRate {
                        currency: "EUR".to_owned(),
                        date: "2024-06-28".to_owned(),
                        mid: Decimal::from_str("4.5").unwrap(),
                    },
                ],
                "nbp-fx",
                "2024-07-01T00:00:00Z",
            )
            .expect("upsert rates");

        let out = compute_kpi_comparison(
            &s,
            &KpiComparisonInput {
                company_ids: vec![c.clone()],
                metric_keys: vec!["revenue".to_owned()],
                granularity: "annual".to_owned(),
            },
        )
        .expect("comparison");
        let cell = cell_at(series_of(&out, &c, "revenue").unwrap(), "2024:FY").unwrap();
        assert_eq!(cell.value_pln.as_deref(), Some("425"));
        assert_eq!(cell.fx_basis.as_deref(), Some("period_average"));
        assert!(cell.flags.is_empty());
    }

    #[test]
    fn eur_fact_without_a_rate_flags_fx_missing_but_keeps_native() {
        let s = state();
        let c = company(&s, "EUX");
        let def = define(&s, "revenue", "monetary");
        let p = period(&s, &c, 2024);
        fact(
            &s,
            &c,
            &p,
            &def,
            "100",
            Some("EUR"),
            "consolidated",
            "reported",
        );

        let out = compute_kpi_comparison(
            &s,
            &KpiComparisonInput {
                company_ids: vec![c.clone()],
                metric_keys: vec!["revenue".to_owned()],
                granularity: "annual".to_owned(),
            },
        )
        .expect("comparison");
        let cell = cell_at(series_of(&out, &c, "revenue").unwrap(), "2024:FY").unwrap();
        assert_eq!(cell.value.as_deref(), Some("100"));
        assert_eq!(cell.value_pln, None);
        assert!(cell.flags.contains(&CellFlag::FxMissing));
    }

    /// Behavioral scale gate (ADR 0049 / DoD §C): a max-watchlist-sized dataset
    /// stays bounded and offloadable — the read model's structure is exactly
    /// companies × metrics series each aligned to the shared axis, with no
    /// unbounded growth. Not a wall-clock assertion.
    #[test]
    fn scale_gate_max_watchlist_dataset_stays_bounded() {
        let s = state();
        let def = define(&s, "revenue", "monetary");
        let mut company_ids = Vec::new();
        for i in 0..50 {
            let c = company(&s, &format!("C{i:02}"));
            for year in 1995..2025 {
                let p = period(&s, &c, year);
                fact(
                    &s,
                    &c,
                    &p,
                    &def,
                    &format!("{}", year * 1_000_000),
                    Some("PLN"),
                    "consolidated",
                    "reported",
                );
            }
            company_ids.push(c);
        }
        let out = compute_kpi_comparison(
            &s,
            &KpiComparisonInput {
                company_ids: company_ids.clone(),
                metric_keys: vec!["revenue".to_owned()],
                granularity: "annual".to_owned(),
            },
        )
        .expect("comparison");
        // One series per (company, metric); axis is the 30-year union; every
        // series aligns 1:1 with the axis — bounded, no cross-product blowup.
        assert_eq!(out.series.len(), 50);
        assert_eq!(out.axis.len(), 30);
        for series in &out.series {
            assert_eq!(series.cells.len(), out.axis.len());
        }
    }

    /// Real-data validation (docs/testing.md): reproduce the hand-labeled
    /// alignment ground truth against the maintainer's real DB. Inert in CI —
    /// `#[ignore]` + skips cleanly when `private/realdata/` is absent. Reads the
    /// gitignored ground truth (same schema as the committed sample) and asserts
    /// the DB-backed read model reproduces each pair's axis, cells, and verified
    /// YoY deltas exactly.
    ///
    /// ```text
    /// cp /mnt/d/Brawler/Builds/latest/data/brawler.sqlite3 private/realdata/v061_worktest.sqlite3
    /// BRAWLER_REAL_DB=private/realdata/v061_worktest.sqlite3 \
    ///   cargo nextest run -p brawler comparison::tests::real_data --run-ignored all --no-capture
    /// ```
    #[test]
    #[ignore = "real-data validation; needs BRAWLER_REAL_DB + private/realdata/v061-alignment-ground-truth.json"]
    fn real_data_alignment_matches_ground_truth() {
        use crate::fundamentals::comparison::{cells_by_year, testsupport::GroundTruth};
        use crate::storage::open_database;
        use std::path::PathBuf;

        let Ok(db_path) = std::env::var("BRAWLER_REAL_DB") else {
            eprintln!("SKIP real_data_alignment: set BRAWLER_REAL_DB to a throwaway copy");
            return;
        };
        let gt_path = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
            .join("..")
            .join("private/realdata/v061-alignment-ground-truth.json");
        let Ok(raw) = std::fs::read_to_string(&gt_path) else {
            eprintln!(
                "SKIP real_data_alignment: no ground truth at {}",
                gt_path.display()
            );
            return;
        };
        let gt = GroundTruth::parse(&raw);
        let state = AppState::new(open_database(&db_path).expect("open real db"));

        for pair in &gt.pairs {
            let out = compute_kpi_comparison(
                &state,
                &KpiComparisonInput {
                    company_ids: pair.company_ids(),
                    metric_keys: vec![pair.kpi.clone()],
                    granularity: pair.granularity.clone(),
                },
            )
            .expect("comparison");

            let axis_years: Vec<i64> = out.axis.iter().map(|p| p.fiscal_year).collect();
            assert_eq!(axis_years, pair.expected_axis, "[{}] axis", pair.id);

            for pc in &pair.companies {
                let series = series_of(&out, pair.company_id(&pc.ticker), &pair.kpi)
                    .unwrap_or_else(|| panic!("[{}] {} series", pair.id, pc.ticker));
                let cells = cells_by_year(series);
                let expected = pair.expected_cells.get(&pc.ticker).unwrap();
                for year in &pair.expected_axis {
                    let cell = cells.get(year).expect("axis cell");
                    match expected.get(&year.to_string()) {
                        Some(v) => assert_eq!(
                            cell.value.as_deref(),
                            Some(v.to_string().as_str()),
                            "[{}] {} {}",
                            pair.id,
                            pc.ticker,
                            year
                        ),
                        None => {
                            assert_eq!(cell.value, None, "[{}] {} {} gap", pair.id, pc.ticker, year)
                        }
                    }
                }
                for (label, exp) in &pair.expected_yoy_verified {
                    let Some(rest) = label.strip_prefix(&format!("{}_", pc.ticker)) else {
                        continue;
                    };
                    let year: i64 = rest.parse().expect("year");
                    let cell = cells.get(&year).expect("yoy cell");
                    match exp.as_str() {
                        "null" => assert_eq!(
                            cell.delta_yo_y, None,
                            "[{}] {} {} yoy",
                            pair.id, pc.ticker, year
                        ),
                        e if e.starts_with("delta_undefined") => {
                            assert_eq!(
                                cell.delta_yo_y, None,
                                "[{}] {} {} yoy undef",
                                pair.id, pc.ticker, year
                            );
                            assert!(cell.flags.contains(&CellFlag::DeltaYoyUndefined));
                        }
                        pct => assert_eq!(
                            cell.delta_yo_y.as_deref(),
                            Some(pct),
                            "[{}] {} {} yoy",
                            pair.id,
                            pc.ticker,
                            year
                        ),
                    }
                }
            }
        }
    }
}
