//! Health-score engine tests (ADR 0083): exact per-signal / per-ratio
//! assertions on hand-built multi-period samples, strict-completeness states,
//! the F5/F9 averaging-basis fallback, and proptest totality/determinism.

use super::*;
use crate::fundamentals::metrics::{Computation, MetricDef, MetricsContext, PeriodFacts};
use rust_decimal::Decimal;
use std::collections::HashMap;

fn d(n: i64) -> Decimal {
    Decimal::from(n)
}

fn def_reported() -> MetricDef {
    MetricDef {
        computation: Computation::Reported,
        formula: None,
        value_kind: "monetary".to_owned(),
        unit: None,
    }
}

fn def_derived(formula: &str) -> MetricDef {
    MetricDef {
        computation: Computation::Derived,
        formula: crate::fundamentals::metrics::parse_formula(formula),
        value_kind: "ratio".to_owned(),
        unit: None,
    }
}

/// Catalog with every reported input the scores read, plus the two derived
/// metrics migration 0089 seeds (`working_capital`, `current_ratio`).
fn defs() -> HashMap<String, MetricDef> {
    let reported = [
        "net_profit",
        "total_assets",
        "operating_cash_flow",
        "long_term_debt",
        "current_assets",
        "current_liabilities",
        "shares_outstanding",
        "gross_profit",
        "revenue",
        "retained_earnings",
        "operating_profit",
        "total_equity",
        "total_liabilities",
    ];
    let mut m: HashMap<String, MetricDef> = reported
        .into_iter()
        .map(|k| (k.to_owned(), def_reported()))
        .collect();
    m.insert(
        "working_capital".to_owned(),
        def_derived("current_assets - current_liabilities"),
    );
    m.insert(
        "current_ratio".to_owned(),
        def_derived("current_assets / current_liabilities"),
    );
    m
}

const REPORTED_KEYS: [&str; 13] = [
    "net_profit",
    "total_assets",
    "operating_cash_flow",
    "long_term_debt",
    "current_assets",
    "current_liabilities",
    "shares_outstanding",
    "gross_profit",
    "revenue",
    "retained_earnings",
    "operating_profit",
    "total_equity",
    "total_liabilities",
];

/// Re-materialize a context's FY periods, keeping only the first `take` of them
/// (newest-first) and dropping any `drop` keys — for the missing-input and
/// fewer-period variants.
fn rebuild(src: &MetricsContext, take: usize, drop: &[&str]) -> MetricsContext {
    let mut periods = Vec::new();
    for y in src.fy_period_refs().iter().take(take) {
        let mut facts = HashMap::new();
        for key in REPORTED_KEYS {
            if drop.contains(&key) {
                continue;
            }
            if let Some(v) = src.value_for(key, y.idx) {
                facts.insert(key.to_string(), v);
            }
        }
        periods.push(PeriodFacts {
            period_id: y.period_id.clone(),
            fiscal_year: y.fiscal_year,
            period_type: "FY".to_owned(),
            reported: facts,
        });
    }
    MetricsContext::new(defs(), periods)
}

fn rebuilt_without(src: &MetricsContext, drop: &[&str]) -> MetricsContext {
    rebuild(src, usize::MAX, drop)
}

fn period(year: i64, facts: &[(&str, i64)]) -> PeriodFacts {
    PeriodFacts {
        period_id: format!("p{year}"),
        fiscal_year: year,
        period_type: "FY".to_owned(),
        reported: facts.iter().map(|(k, v)| (k.to_string(), d(*v))).collect(),
    }
}

/// A full-input three-FY sample (FY2025 / FY2024 / FY2023) with hand-chosen
/// figures so every F signal and the Z″ value are hand-computable.
fn full_sample() -> MetricsContext {
    let fy2025 = period(
        2025,
        &[
            ("net_profit", 100),
            ("total_assets", 1000),
            ("operating_cash_flow", 150),
            ("long_term_debt", 200),
            ("current_assets", 500),
            ("current_liabilities", 250),
            ("shares_outstanding", 1000),
            ("gross_profit", 350),
            ("revenue", 1000),
            ("retained_earnings", 300),
            ("operating_profit", 120),
            ("total_equity", 600),
            ("total_liabilities", 400),
        ],
    );
    let fy2024 = period(
        2024,
        &[
            ("net_profit", 80),
            ("total_assets", 900),
            ("operating_cash_flow", 120),
            ("long_term_debt", 250),
            ("current_assets", 400),
            ("current_liabilities", 250),
            ("shares_outstanding", 1000),
            ("gross_profit", 270),
            ("revenue", 900),
            ("retained_earnings", 200),
            ("operating_profit", 100),
            ("total_equity", 500),
            ("total_liabilities", 450),
        ],
    );
    let fy2023 = period(
        2023,
        &[
            ("net_profit", 60),
            ("total_assets", 800),
            ("operating_cash_flow", 90),
            ("long_term_debt", 300),
            ("current_assets", 300),
            ("current_liabilities", 200),
            ("shares_outstanding", 1000),
            ("gross_profit", 220),
            ("revenue", 820),
            ("retained_earnings", 120),
            ("operating_profit", 80),
            ("total_equity", 420),
            ("total_liabilities", 380),
        ],
    );
    MetricsContext::new(defs(), vec![fy2025, fy2024, fy2023])
}

fn coords_latest(ctx: &MetricsContext) -> FyCoords {
    let fys = ctx.fy_period_refs();
    FyCoords {
        idx_t: fys[0].idx,
        year_t: fys[0].fiscal_year,
        idx_tm1: fys[1].idx,
        year_tm1: fys[1].fiscal_year,
        tm2: fys.get(2).map(|p| (p.idx, p.fiscal_year)),
    }
}

fn signal<'a>(score: &'a PiotroskiScore, code: &str) -> &'a PiotroskiSignal {
    let signals = match score {
        PiotroskiScore::Headline { signals, .. } => signals,
        PiotroskiScore::InsufficientData { signals, .. } => signals,
        PiotroskiScore::NotApplicable { .. } => panic!("not applicable"),
    };
    signals
        .iter()
        .find(|s| s.code == code)
        .expect("signal present")
}

// ---- full-input company: exact points + exact Z ---------------------------

#[test]
fn full_input_piotroski_scores_each_signal_exactly() {
    let ctx = full_sample();
    let score = piotroski_f(&ctx, &coords_latest(&ctx), INDUSTRIAL_STATEMENT_TYPE);

    // F9 fails (turnover declined on the two-year-average basis); all others pass.
    assert!(signal(&score, "F1").passed, "F1 ROA>0");
    assert!(signal(&score, "F2").passed, "F2 CFO>0");
    assert!(signal(&score, "F3").passed, "F3 ΔROA>0");
    assert!(signal(&score, "F4").passed, "F4 accruals");
    assert!(signal(&score, "F5").passed, "F5 leverage");
    assert!(signal(&score, "F6").passed, "F6 liquidity");
    assert!(signal(&score, "F7").passed, "F7 no dilution");
    assert!(signal(&score, "F8").passed, "F8 margin");
    assert!(!signal(&score, "F9").passed, "F9 turnover declined");

    match score {
        PiotroskiScore::Headline { score, signals } => {
            assert_eq!(score, 8);
            assert_eq!(signals.len(), 9);
        }
        other => panic!("expected headline, got {other:?}"),
    }
}

#[test]
fn full_input_altman_z_and_band_exact() {
    let ctx = full_sample();
    let fys = ctx.fy_period_refs();
    let score = altman_z(
        &ctx,
        fys[0].idx,
        fys[0].fiscal_year,
        INDUSTRIAL_STATEMENT_TYPE,
    );
    match score {
        AltmanScore::Headline {
            z_score,
            band,
            components,
        } => {
            // 6.56·0.25 + 3.26·0.30 + 6.72·0.12 + 1.05·1.5 = 4.9994
            assert_eq!(z_score, "4.9994");
            assert_eq!(band, AltmanBand::Safe);
            assert_eq!(components.len(), 4);
        }
        other => panic!("expected headline, got {other:?}"),
    }
}

#[test]
fn altman_band_thresholds() {
    // >2.6 safe, 1.1..=2.6 grey (boundary inclusive at 1.1), <1.1 distress.
    assert_eq!(band_of(Decimal::new(27, 1)), AltmanBand::Safe);
    assert_eq!(band_of(Decimal::new(26, 1)), AltmanBand::Grey); // exactly 2.6
    assert_eq!(band_of(Decimal::new(11, 1)), AltmanBand::Grey); // exactly 1.1
    assert_eq!(band_of(Decimal::new(10, 1)), AltmanBand::Distress);
}

// ---- missing current_liabilities: strict completeness withholds both -------
// F5's leverage input is non-current liabilities (`total_liabilities −
// current_liabilities`, ADR 0083 D4 amendment). A missing `current_liabilities`
// breaks F5 (leverage) + F6 (current_ratio) and Altman X1 (working_capital) —
// strict completeness must yield `insufficient_data` for both, never a silent
// zero.
#[test]
fn missing_current_liabilities_yields_insufficient_both() {
    let ctx = rebuilt_without(&full_sample(), &["current_liabilities"]);

    let piotroski = piotroski_f(&ctx, &coords_latest(&ctx), INDUSTRIAL_STATEMENT_TYPE);
    match &piotroski {
        PiotroskiScore::InsufficientData { signals, missing } => {
            assert!(
                missing.iter().any(|m| m.metric == "current_liabilities"),
                "missing lists current_liabilities (F5 leverage input): {missing:?}"
            );
            // F5 (leverage) and F6 (current_ratio) could not compute.
            assert!(signals.iter().all(|s| s.code != "F5" && s.code != "F6"));
        }
        other => panic!("expected insufficient_data, got {other:?}"),
    }

    let fys = ctx.fy_period_refs();
    let altman = altman_z(
        &ctx,
        fys[0].idx,
        fys[0].fiscal_year,
        INDUSTRIAL_STATEMENT_TYPE,
    );
    // X1 (working_capital = current_assets − current_liabilities) can't resolve.
    match &altman {
        AltmanScore::InsufficientData { missing, .. } => assert!(
            missing.iter().any(|m| m.metric == "working_capital"),
            "Altman missing lists working_capital (X1): {missing:?}"
        ),
        other => panic!("expected insufficient_data, got {other:?}"),
    }
}

// ---- `long_term_debt` does not withhold F (basis swap) ----------
#[test]
fn missing_long_term_debt_still_computes_headline() {
    // long_term_debt is retained in the catalog but is not a health input.
    let ctx = rebuilt_without(&full_sample(), &["long_term_debt"]);
    let piotroski = piotroski_f(&ctx, &coords_latest(&ctx), INDUSTRIAL_STATEMENT_TYPE);
    assert!(
        matches!(piotroski, PiotroskiScore::Headline { .. }),
        "F5 reads non-current liabilities now, so missing LTD is immaterial: {piotroski:?}"
    );
}

// ---- financial statement type: NotApplicable -------------------------------

#[test]
fn financial_statement_type_is_not_applicable() {
    let ctx = full_sample();
    let piotroski = piotroski_f(&ctx, &coords_latest(&ctx), "bank");
    let fys = ctx.fy_period_refs();
    let altman = altman_z(&ctx, fys[0].idx, fys[0].fiscal_year, "insurance");
    assert!(matches!(piotroski, PiotroskiScore::NotApplicable { .. }));
    assert!(matches!(altman, AltmanScore::NotApplicable { .. }));

    // And through the read model.
    let scores = company_health(&ctx, "broker");
    assert!(scores.iter().all(
        |s| matches!(s.piotroski, PiotroskiScore::NotApplicable { .. })
            && matches!(s.altman, AltmanScore::NotApplicable { .. })
    ));
}

// ---- single period: no-prior insufficient, Altman still headline -----------

#[test]
fn single_period_has_no_prior_piotroski_but_altman_headline() {
    let ctx = MetricsContext::new(
        defs(),
        vec![period(
            2025,
            &[
                ("net_profit", 100),
                ("total_assets", 1000),
                ("operating_cash_flow", 150),
                ("long_term_debt", 200),
                ("current_assets", 500),
                ("current_liabilities", 250),
                ("shares_outstanding", 1000),
                ("gross_profit", 350),
                ("revenue", 1000),
                ("retained_earnings", 300),
                ("operating_profit", 120),
                ("total_equity", 600),
                ("total_liabilities", 400),
            ],
        )],
    );
    let scores = company_health(&ctx, INDUSTRIAL_STATEMENT_TYPE);
    assert_eq!(scores.len(), 1);
    match &scores[0].piotroski {
        PiotroskiScore::InsufficientData { signals, missing } => {
            assert!(signals.is_empty());
            assert_eq!(missing.len(), 1);
            assert_eq!(missing[0].metric, "prior_fy_period");
        }
        other => panic!("expected no-prior insufficient_data, got {other:?}"),
    }
    assert!(matches!(scores[0].altman, AltmanScore::Headline { .. }));
}

// ---- F5/F9 averaging-basis fallback ----------------------------------------

fn basis_input(sig: &PiotroskiSignal) -> &str {
    sig.inputs
        .iter()
        .find(|i| i.key == "basis")
        .map(|i| i.value.as_str())
        .expect("basis recorded")
}

#[test]
fn two_periods_use_period_end_basis_three_use_two_year_average() {
    // Three FY periods → t-2 present → two-year-average basis.
    let three = full_sample();
    let s3 = piotroski_f(&three, &coords_latest(&three), INDUSTRIAL_STATEMENT_TYPE);
    assert_eq!(basis_input(signal(&s3, "F5")), "two_year_average");
    assert_eq!(basis_input(signal(&s3, "F9")), "two_year_average");

    // Two FY periods only → no t-2 → period-end fallback, consistent across both.
    let two = rebuild(&three, 2, &[]);
    let s2 = piotroski_f(&two, &coords_latest(&two), INDUSTRIAL_STATEMENT_TYPE);
    assert_eq!(basis_input(signal(&s2, "F5")), "period_end");
    assert_eq!(basis_input(signal(&s2, "F9")), "period_end");
}

// ---- insta golden: the full read model -------------------------------------

#[test]
fn company_health_full_sample_golden() {
    let ctx = full_sample();
    let scores = company_health(&ctx, INDUSTRIAL_STATEMENT_TYPE);
    insta::assert_debug_snapshot!("company_health_full_sample", scores);
}

// ---- scalar injection: latest-FY headline / unavailable --------------------

#[test]
fn latest_scalars_are_headline_values_or_none() {
    let full = full_sample();
    let s = latest_fy_scalars(&full, INDUSTRIAL_STATEMENT_TYPE);
    assert_eq!(s.piotroski_f, Some(Decimal::from(8)));
    assert_eq!(s.altman_z, Some(Decimal::new(49994, 4)));

    // Financial company → both None (→ Unavailable at the resolver).
    let na = latest_fy_scalars(&full, "bank");
    assert_eq!(na.piotroski_f, None);
    assert_eq!(na.altman_z, None);
}

// ---- criterion integration: injected scalar resolves in the DSL ------------

#[test]
fn piotroski_scalar_criterion_passes_on_headline_and_unavailable_when_missing() {
    use crate::fundamentals::scorecard::evaluate_criterion;
    use crate::fundamentals::Verdict;

    // Full sample: latest-FY F = 8 → injected scalar 8 → `piotroski_f >= 7` passes.
    let full = full_sample();
    let full_ctx = full
        .clone()
        .with_health_scalars(latest_fy_scalars(&full, INDUSTRIAL_STATEMENT_TYPE));
    assert_eq!(
        evaluate_criterion("piotroski_f >= 7", None, &full_ctx).verdict,
        Verdict::Pass
    );

    // Missing-current-liabilities sample: latest FY is InsufficientData (F5/F6
    // can't compute) → scalar None → the key resolves Unavailable → the criterion
    // renders `unavailable`, not fail.
    let no_cl = rebuilt_without(&full_sample(), &["current_liabilities"]);
    let no_cl_ctx = no_cl
        .clone()
        .with_health_scalars(latest_fy_scalars(&no_cl, INDUSTRIAL_STATEMENT_TYPE));
    assert_eq!(
        evaluate_criterion("piotroski_f >= 7", None, &no_cl_ctx).verdict,
        Verdict::Unavailable
    );
}

// ---- proptest: determinism + totality + never-headline-when-missing --------

mod properties {
    use super::*;
    use proptest::prelude::*;

    /// Build a context from an optional value per reported key across up to
    /// three FY periods; `None` drops the fact.
    fn ctx_from(present: &[[Option<i64>; 13]]) -> MetricsContext {
        let keys = [
            "net_profit",
            "total_assets",
            "operating_cash_flow",
            "long_term_debt",
            "current_assets",
            "current_liabilities",
            "shares_outstanding",
            "gross_profit",
            "revenue",
            "retained_earnings",
            "operating_profit",
            "total_equity",
            "total_liabilities",
        ];
        let mut periods = Vec::new();
        for (i, year_facts) in present.iter().enumerate() {
            let year = 2025 - i as i64;
            let mut facts = HashMap::new();
            for (k, v) in keys.iter().zip(year_facts.iter()) {
                if let Some(val) = v {
                    facts.insert(k.to_string(), d(*val));
                }
            }
            periods.push(PeriodFacts {
                period_id: format!("p{year}"),
                fiscal_year: year,
                period_type: "FY".to_owned(),
                reported: facts,
            });
        }
        MetricsContext::new(defs(), periods)
    }

    fn opt_val() -> impl Strategy<Value = Option<i64>> {
        prop::option::of(1i64..100_000)
    }

    prop_compose! {
        fn arbitrary_year()(
            f in prop::array::uniform13(opt_val())
        ) -> [Option<i64>; 13] {
            f
        }
    }

    proptest! {
        /// Determinism: the same context yields byte-identical results.
        #[test]
        fn deterministic(years in prop::collection::vec(arbitrary_year(), 1..4)) {
            let a = company_health(&ctx_from(&years), INDUSTRIAL_STATEMENT_TYPE);
            let b = company_health(&ctx_from(&years), INDUSTRIAL_STATEMENT_TYPE);
            prop_assert_eq!(a, b);
        }

        /// Totality: never panics over any partial input, any period count.
        #[test]
        fn total_no_panic(years in prop::collection::vec(arbitrary_year(), 0..4)) {
            let _ = company_health(&ctx_from(&years), INDUSTRIAL_STATEMENT_TYPE);
            let _ = company_health(&ctx_from(&years), "bank");
        }

        /// A Piotroski headline implies every one of the nine signals is present
        /// and the sum is consistent — never a headline while inputs are absent.
        #[test]
        fn headline_only_when_complete(years in prop::collection::vec(arbitrary_year(), 1..4)) {
            for entry in company_health(&ctx_from(&years), INDUSTRIAL_STATEMENT_TYPE) {
                if let PiotroskiScore::Headline { score, signals } = &entry.piotroski {
                    prop_assert_eq!(signals.len(), 9);
                    let sum: u8 = signals.iter().map(|s| s.points).sum();
                    prop_assert_eq!(*score, sum);
                    prop_assert!(*score <= 9);
                }
                if let AltmanScore::Headline { components, .. } = &entry.altman {
                    prop_assert_eq!(components.len(), 4);
                }
            }
        }
    }
}

/// Every `statement_type` the app actually writes, pinned against the ADR 0083
/// D4 gate. The gate is an allow-list of ONE (`!= industrial`), so a new type
/// lands on the financial side automatically — this test exists so that stays
/// true: if the gate is ever rewritten as an explicit list, a forgotten member
/// reddens here instead of silently emitting a meaningless Altman Z″.
///
/// `brokerage` is the 2026-07-31 split of `specialty_finance` (owner decision):
/// it must sit on exactly the side `specialty_finance` sat on before it.
#[test]
fn every_financial_statement_type_is_not_applicable_and_names_itself() {
    let ctx = full_sample();
    let fys = ctx.fy_period_refs();

    for statement_type in [
        "banking",
        "insurance",
        "specialty_finance",
        "brokerage",
        "reit",
    ] {
        let piotroski = piotroski_f(&ctx, &coords_latest(&ctx), statement_type);
        match piotroski {
            PiotroskiScore::NotApplicable { ref reason } => {
                assert_eq!(
                    reason, statement_type,
                    "the reason echoes the type verbatim"
                )
            }
            other => panic!("{statement_type} must not score Piotroski F, got {other:?}"),
        }

        let altman = altman_z(&ctx, fys[0].idx, fys[0].fiscal_year, statement_type);
        match altman {
            AltmanScore::NotApplicable { ref reason } => {
                assert_eq!(
                    reason, statement_type,
                    "the reason echoes the type verbatim"
                )
            }
            other => panic!("{statement_type} must not score Altman Z\u{2033}, got {other:?}"),
        }

        assert!(
            company_health(&ctx, statement_type).iter().all(|s| {
                matches!(s.piotroski, PiotroskiScore::NotApplicable { .. })
                    && matches!(s.altman, AltmanScore::NotApplicable { .. })
            }),
            "{statement_type} must be NotApplicable through the read model too"
        );
    }

    // The control: the one type the scores DO apply to.
    assert!(!matches!(
        altman_z(
            &ctx,
            fys[0].idx,
            fys[0].fiscal_year,
            INDUSTRIAL_STATEMENT_TYPE
        ),
        AltmanScore::NotApplicable { .. }
    ));
}
