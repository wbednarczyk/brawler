//! Tests for the deterministic fundamentals validation gate (ADR 0061, ADR 0049
//! test architecture: behavioural units + property invariants + a golden
//! snapshot).

use super::*;
use rust_decimal::Decimal;
use std::collections::BTreeSet;

/// Builds a `FactSet` from `(key, value)` literals.
fn facts(pairs: &[(&str, Decimal)]) -> FactSet {
    pairs.iter().map(|(k, v)| (k.to_string(), *v)).collect()
}

/// Whole-unit `Decimal` literal helper (all test amounts are integers).
fn d(v: i64) -> Decimal {
    Decimal::from(v)
}

// ---------------------------------------------------------------------------
// Balance-sheet identity
// ---------------------------------------------------------------------------

#[test]
fn balance_sheet_passes_when_exact() {
    let f = facts(&[
        (TOTAL_ASSETS, d(1000)),
        (TOTAL_LIABILITIES, d(600)),
        (TOTAL_EQUITY, d(400)),
    ]);
    assert_eq!(
        balance_sheet(&f, &Tolerance::default()).outcome,
        Outcome::Pass
    );
}

#[test]
fn balance_sheet_passes_within_rounding_tolerance() {
    // 0.02% off a 1bn balance sheet — pure reporting rounding, must pass.
    let f = facts(&[
        (TOTAL_ASSETS, d(1_000_000_000)),
        (TOTAL_LIABILITIES, d(600_000_000)),
        (TOTAL_EQUITY, d(400_200_000)),
    ]);
    assert_eq!(
        balance_sheet(&f, &Tolerance::default()).outcome,
        Outcome::Pass
    );
}

#[test]
fn balance_sheet_fails_when_off_beyond_tolerance() {
    // 10% off — a missing line or a swapped column, must fail.
    let f = facts(&[
        (TOTAL_ASSETS, d(1000)),
        (TOTAL_LIABILITIES, d(600)),
        (TOTAL_EQUITY, d(500)),
    ]);
    let out = balance_sheet(&f, &Tolerance::default()).outcome;
    assert!(out.is_fail(), "expected fail, got {out:?}");
    if let Outcome::Fail { residual, .. } = out {
        assert_eq!(residual, d(-100));
    }
}

#[test]
fn balance_sheet_not_applicable_when_leg_missing() {
    // No liabilities fact — absence is not a contradiction.
    let f = facts(&[(TOTAL_ASSETS, d(1000)), (TOTAL_EQUITY, d(400))]);
    match balance_sheet(&f, &Tolerance::default()).outcome {
        Outcome::NotApplicable { missing } => {
            assert_eq!(missing, vec![TOTAL_LIABILITIES.to_string()])
        }
        other => panic!("expected NotApplicable, got {other:?}"),
    }
}

// ---------------------------------------------------------------------------
// Free cash flow
// ---------------------------------------------------------------------------

#[test]
fn free_cash_flow_passes_when_matches_definition() {
    let f = facts(&[
        (FREE_CASH_FLOW, d(700)),
        (OPERATING_CASH_FLOW, d(1000)),
        (CAPEX, d(300)),
    ]);
    assert_eq!(
        free_cash_flow(&f, &Tolerance::default()).outcome,
        Outcome::Pass
    );
}

#[test]
fn free_cash_flow_not_applicable_when_derived_only() {
    // FCF not reported (the common case) → skipped, not failed.
    let f = facts(&[(OPERATING_CASH_FLOW, d(1000)), (CAPEX, d(300))]);
    assert!(matches!(
        free_cash_flow(&f, &Tolerance::default()).outcome,
        Outcome::NotApplicable { .. }
    ));
}

// ---------------------------------------------------------------------------
// Cash-flow tie (cross-period)
// ---------------------------------------------------------------------------

#[test]
fn cash_flow_tie_passes_when_flows_reconcile() {
    let cur = facts(&[
        (CASH, d(1500)),
        (OPERATING_CASH_FLOW, d(800)),
        (INVESTING_CASH_FLOW, d(-300)),
        (FINANCING_CASH_FLOW, d(0)),
    ]);
    let prior = facts(&[(CASH, d(1000))]); // Δcash = 500 = 800 - 300 + 0
    assert_eq!(
        cash_flow_tie(&cur, Some(&prior), &Tolerance::default()).outcome,
        Outcome::Pass
    );
}

#[test]
fn cash_flow_tie_not_applicable_without_prior_cash() {
    let cur = facts(&[
        (CASH, d(1500)),
        (OPERATING_CASH_FLOW, d(800)),
        (INVESTING_CASH_FLOW, d(-300)),
        (FINANCING_CASH_FLOW, d(0)),
    ]);
    assert!(matches!(
        cash_flow_tie(&cur, None, &Tolerance::default()).outcome,
        Outcome::NotApplicable { .. }
    ));
}

#[test]
fn cash_flow_tie_fails_when_flows_dont_reconcile() {
    let cur = facts(&[
        (CASH, d(2000)),
        (OPERATING_CASH_FLOW, d(800)),
        (INVESTING_CASH_FLOW, d(-300)),
        (FINANCING_CASH_FLOW, d(0)),
    ]);
    let prior = facts(&[(CASH, d(1000))]); // Δ=1000 but flows sum to 500
    assert!(cash_flow_tie(&cur, Some(&prior), &Tolerance::default())
        .outcome
        .is_fail());
}

// ---------------------------------------------------------------------------
// Comparative cross-check
// ---------------------------------------------------------------------------

#[test]
fn cross_check_flags_column_misalignment() {
    // The prior column read from the report disagrees with what we stored for
    // that period → the extractor read the wrong column.
    let extracted_prior = facts(&[(TOTAL_ASSETS, d(950)), (TOTAL_EQUITY, d(400))]);
    let stored_prior = facts(&[(TOTAL_ASSETS, d(500)), (TOTAL_EQUITY, d(400))]);
    let checks = cross_check_prior(&extracted_prior, &stored_prior, &Tolerance::default());
    let assets = checks
        .iter()
        .find(|c| c.metric_key == TOTAL_ASSETS)
        .unwrap();
    assert!(assets.outcome.is_fail());
    let equity = checks
        .iter()
        .find(|c| c.metric_key == TOTAL_EQUITY)
        .unwrap();
    assert_eq!(equity.outcome, Outcome::Pass);
}

#[test]
fn uniform_scale_error_passes_balance_sheet_but_the_comparative_cross_check_catches_it() {
    // Guard for the class the same-period identities are BLIND to (card 22ac70c
    // item 4): a document read a uniform ×1000 too small (a dropped "w tys."
    // multiplier) is INTERNALLY consistent — assets, liabilities and equity are
    // all off by the same factor, so `total_assets = total_liabilities +
    // total_equity` STILL HOLDS. The balance-sheet identity therefore passes and
    // cannot flag it; only a check against an independently-read number can.
    let unscaled = facts(&[
        (TOTAL_ASSETS, d(1_000)),
        (TOTAL_LIABILITIES, d(600)),
        (TOTAL_EQUITY, d(400)),
    ]);
    assert_eq!(
        balance_sheet(&unscaled, &Tolerance::default()).outcome,
        Outcome::Pass,
        "a uniformly-unscaled statement is internally consistent — the identity cannot see the error"
    );

    // The comparative cross-check IS the guard: the prior-period column read out
    // of this unscaled report (×1 base units) disagrees ~1000× with the value
    // already stored for that period (correct base units) → Fail.
    let extracted_prior = facts(&[(TOTAL_ASSETS, d(950)), (TOTAL_EQUITY, d(380))]);
    let stored_prior = facts(&[(TOTAL_ASSETS, d(950_000)), (TOTAL_EQUITY, d(380_000))]);
    let checks = cross_check_prior(&extracted_prior, &stored_prior, &Tolerance::default());
    assert!(
        checks.iter().all(|c| c.outcome.is_fail()),
        "every comparative metric must flag the ~1000× scale mismatch"
    );
}

#[test]
fn history_plausibility_flags_a_uniform_scale_outlier_but_spares_normal_moves() {
    // Card 22ac70c item 4 / harvest: Atrem cash 19 000 against sibling periods
    // of ~3.8 M / 2.3 M / 8.5 M — ~200× below the median, the footprint of a
    // note ref read as the value with no comparative column to catch it.
    let siblings = [d(3_841_000), d(2_315_000), d(8_529_000)];
    assert!(implausible_against_history("cash", d(19_000), &siblings));
    // A ~1000× over-scale (a dropped multiplier) is flagged the same way.
    assert!(implausible_against_history(
        "revenue",
        d(207_763_000_000),
        &[d(83_411_000), d(134_113_000), d(130_074_000),]
    ));
    // A normal period-over-period value is NOT flagged.
    assert!(!implausible_against_history(
        "cash",
        d(3_500_000),
        &siblings
    ));
    // Split-sensitive metrics are exempt (a split legitimately moves them >100×).
    assert!(!implausible_against_history(
        "eps_basic",
        d(1),
        &[d(200), d(210)]
    ));
    // Too little history to judge (< 2 values) abstains.
    assert!(!implausible_against_history(
        "cash",
        d(19_000),
        &[d(3_841_000)]
    ));
    // A zero on either side is left to the identities, never flagged as a scale error.
    assert!(!implausible_against_history("cash", d(0), &siblings));
}

#[test]
fn plausibility_verdict_names_why_a_fact_was_not_flagged() {
    // The typed twin of `implausible_against_history`: same cases, but every
    // non-implausible reason is a distinct, honest `Abstained` — never
    // collapsed into an unlabeled pass (ADR 0093 dec. 6).
    let siblings = [d(3_841_000), d(2_315_000), d(8_529_000)];
    assert_eq!(
        plausibility_verdict("cash", d(19_000), &siblings),
        PlausibilityVerdict::Implausible {
            history_median: d(3_841_000)
        }
    );
    assert_eq!(
        plausibility_verdict("cash", d(3_500_000), &siblings),
        PlausibilityVerdict::Plausible
    );
    // Split-sensitive: abstains regardless of the swing.
    assert_eq!(
        plausibility_verdict("eps_basic", d(1), &[d(200), d(210)]),
        PlausibilityVerdict::Abstained {
            reason: AbstentionReason::SplitSensitive
        }
    );
    // Thin history (< 2 values): abstains.
    assert_eq!(
        plausibility_verdict("cash", d(19_000), &[d(3_841_000)]),
        PlausibilityVerdict::Abstained {
            reason: AbstentionReason::ThinHistory
        }
    );
    // Zero value: abstains.
    assert_eq!(
        plausibility_verdict("cash", d(0), &siblings),
        PlausibilityVerdict::Abstained {
            reason: AbstentionReason::ZeroValue
        }
    );
}

#[test]
fn identity_check_metric_keys_names_each_identitys_inputs() {
    assert_eq!(
        identity_check_metric_keys("balance_sheet_identity"),
        &[TOTAL_ASSETS, TOTAL_LIABILITIES, TOTAL_EQUITY]
    );
    assert_eq!(
        identity_check_metric_keys("free_cash_flow_definition"),
        &[FREE_CASH_FLOW, OPERATING_CASH_FLOW, CAPEX]
    );
    assert_eq!(
        identity_check_metric_keys("cash_flow_tie"),
        &[
            CASH,
            OPERATING_CASH_FLOW,
            INVESTING_CASH_FLOW,
            FINANCING_CASH_FLOW
        ]
    );
    assert_eq!(identity_check_metric_keys("unknown_check"), &[] as &[&str]);
}

#[test]
fn history_median_is_the_upper_middle_magnitude_and_needs_two_values() {
    // The figure the quarantine outcome cites. Upper-middle element (index len/2),
    // magnitudes only, zeros excluded, and `None` below two non-zero values so the
    // gate has no median to judge against.
    assert_eq!(history_median(&[d(1), d(3), d(2)]), Some(d(2)));
    assert_eq!(
        history_median(&[d(-8_529_000), d(3_841_000)]),
        Some(d(8_529_000))
    );
    assert_eq!(
        history_median(&[d(0), d(5)]),
        None,
        "a lone non-zero is not a median"
    );
    assert_eq!(history_median(&[d(5)]), None);
    assert_eq!(history_median(&[]), None);
}

#[test]
fn cross_check_only_compares_overlapping_metrics() {
    let extracted_prior = facts(&[(TOTAL_ASSETS, d(950))]);
    let stored_prior = facts(&[(TOTAL_EQUITY, d(400))]);
    assert!(cross_check_prior(&extracted_prior, &stored_prior, &Tolerance::default()).is_empty());
}

// ---------------------------------------------------------------------------
// Overall status
// ---------------------------------------------------------------------------

#[test]
fn status_inconclusive_when_nothing_evaluable() {
    let f = facts(&[("revenue", d(1000))]); // no identity has its inputs
    assert_eq!(
        validate_period(&f, &Tolerance::default()).status,
        Status::Inconclusive
    );
}

#[test]
fn status_passed_when_an_identity_holds() {
    let f = facts(&[
        (TOTAL_ASSETS, d(1000)),
        (TOTAL_LIABILITIES, d(600)),
        (TOTAL_EQUITY, d(400)),
    ]);
    assert_eq!(
        validate_period(&f, &Tolerance::default()).status,
        Status::Passed
    );
}

#[test]
fn status_failed_dominates_a_passing_identity() {
    // Balance sheet holds, but the cash-flow tie fails → overall Failed.
    let cur = facts(&[
        (TOTAL_ASSETS, d(1000)),
        (TOTAL_LIABILITIES, d(600)),
        (TOTAL_EQUITY, d(400)),
        (CASH, d(2000)),
        (OPERATING_CASH_FLOW, d(800)),
        (INVESTING_CASH_FLOW, d(-300)),
        (FINANCING_CASH_FLOW, d(0)),
    ]);
    let prior = facts(&[(CASH, d(1000))]);
    assert_eq!(
        validate(&cur, Some(&prior), None, None, &Tolerance::default()).status,
        Status::Failed
    );
}

#[test]
fn empty_factset_does_not_panic() {
    assert_eq!(
        validate_period(&FactSet::new(), &Tolerance::default()).status,
        Status::Inconclusive
    );
}

// ---------------------------------------------------------------------------
// Completeness (ADR 0061 dec. 4d) — report-only, never flips Status.
// ---------------------------------------------------------------------------

fn keys(names: &[&str]) -> BTreeSet<String> {
    names.iter().map(|s| s.to_string()).collect()
}

#[test]
fn completeness_counts_present_and_missing() {
    let cur = facts(&[(TOTAL_ASSETS, d(1000)), ("revenue", d(500))]);
    let expected = keys(&[TOTAL_ASSETS, "revenue", "net_profit"]);
    let c = completeness(&cur, &expected);
    assert_eq!(c.expected, 3);
    assert_eq!(c.present, 2);
    assert_eq!(c.missing, vec!["net_profit".to_string()]);
}

#[test]
fn completeness_zero_present_when_nothing_overlaps() {
    let cur = facts(&[("some_other_metric", d(1))]);
    let expected = keys(&[TOTAL_ASSETS, "revenue"]);
    let c = completeness(&cur, &expected);
    assert_eq!(c.expected, 2);
    assert_eq!(c.present, 0);
    // BTreeSet iterates sorted: "revenue" < "total_assets".
    assert_eq!(
        c.missing,
        vec!["revenue".to_string(), TOTAL_ASSETS.to_string()]
    );
}

#[test]
fn completeness_full_when_all_expected_present() {
    let cur = facts(&[(TOTAL_ASSETS, d(1000)), ("revenue", d(500))]);
    let expected = keys(&[TOTAL_ASSETS, "revenue"]);
    let c = completeness(&cur, &expected);
    assert_eq!(c.expected, 2);
    assert_eq!(c.present, 2);
    assert!(c.missing.is_empty());
}

#[test]
fn validate_never_sets_completeness_itself() {
    // `validate`/`validate_period` don't take `expected_keys` — completeness is
    // wired by the pipeline, which owns the report afterward. The gate itself
    // never computes it, so it must default to `None`.
    let f = facts(&[
        (TOTAL_ASSETS, d(1000)),
        (TOTAL_LIABILITIES, d(600)),
        (TOTAL_EQUITY, d(400)),
    ]);
    assert_eq!(
        validate_period(&f, &Tolerance::default()).completeness,
        None
    );
}

// ---------------------------------------------------------------------------
// Golden snapshot (ADR 0049): a representative mixed-tier report's full report.
// ---------------------------------------------------------------------------

#[test]
fn golden_report_snapshot() {
    // A CBF-like consolidated set: balance sheet balances, FCF reported and
    // consistent, cash-flow tie reconciles against the prior period.
    let cur = facts(&[
        (TOTAL_ASSETS, d(45_000_000)),
        (TOTAL_LIABILITIES, d(20_000_000)),
        (TOTAL_EQUITY, d(25_000_000)),
        (CASH, d(8_000_000)),
        (OPERATING_CASH_FLOW, d(6_000_000)),
        (INVESTING_CASH_FLOW, d(-2_000_000)),
        (FINANCING_CASH_FLOW, d(-1_000_000)),
        (FREE_CASH_FLOW, d(4_000_000)),
        (CAPEX, d(2_000_000)),
    ]);
    let prior = facts(&[(CASH, d(5_000_000))]); // Δ=3m = 6 - 2 - 1
    let report = validate(&cur, Some(&prior), None, None, &Tolerance::default());
    insta::assert_debug_snapshot!("golden_report", report);
}

// ---------------------------------------------------------------------------
// Property invariants (ADR 0049)
// ---------------------------------------------------------------------------

mod properties {
    use super::*;
    use proptest::prelude::*;

    fn dec_from(i: i64) -> Decimal {
        Decimal::from(i)
    }

    proptest! {
        // Determinism: validating the same fact set twice yields the same report.
        #[test]
        fn validate_is_deterministic(
            a in -1_000_000i64..1_000_000,
            l in -1_000_000i64..1_000_000,
            e in -1_000_000i64..1_000_000,
        ) {
            let f = facts(&[
                (TOTAL_ASSETS, dec_from(a)),
                (TOTAL_LIABILITIES, dec_from(l)),
                (TOTAL_EQUITY, dec_from(e)),
            ]);
            let tol = Tolerance::default();
            prop_assert_eq!(validate_period(&f, &tol), validate_period(&f, &tol));
        }

        // Balance sheet built to balance always passes, at any magnitude/sign.
        #[test]
        fn constructed_balance_always_passes(
            l in -10_000_000i64..10_000_000,
            e in -10_000_000i64..10_000_000,
        ) {
            let f = facts(&[
                (TOTAL_ASSETS, dec_from(l) + dec_from(e)),
                (TOTAL_LIABILITIES, dec_from(l)),
                (TOTAL_EQUITY, dec_from(e)),
            ]);
            prop_assert_eq!(balance_sheet(&f, &Tolerance::default()).outcome, Outcome::Pass);
        }

        // Scale invariance: multiplying every value by a positive constant does
        // not change the balance-sheet verdict (relative tolerance is scale-free).
        #[test]
        fn balance_verdict_is_scale_invariant(
            a in 1i64..1_000_000,
            l in 1i64..1_000_000,
            e in 1i64..1_000_000,
            k in 2i64..1000,
        ) {
            let tol = Tolerance::default();
            let base = facts(&[
                (TOTAL_ASSETS, dec_from(a)),
                (TOTAL_LIABILITIES, dec_from(l)),
                (TOTAL_EQUITY, dec_from(e)),
            ]);
            let scaled = facts(&[
                (TOTAL_ASSETS, dec_from(a * k)),
                (TOTAL_LIABILITIES, dec_from(l * k)),
                (TOTAL_EQUITY, dec_from(e * k)),
            ]);
            // Compare only the pass/fail class (residual magnitudes differ by k).
            prop_assert_eq!(
                balance_sheet(&base, &tol).outcome.is_fail(),
                balance_sheet(&scaled, &tol).outcome.is_fail()
            );
        }

        // A fact set with no identity inputs is always Inconclusive, never a
        // panic and never a spurious failure.
        #[test]
        fn irrelevant_metrics_are_inconclusive(v in -1_000_000i64..1_000_000) {
            let f = facts(&[("revenue", dec_from(v)), ("net_profit", dec_from(v))]);
            prop_assert_eq!(validate_period(&f, &Tolerance::default()).status, Status::Inconclusive);
        }
    }
}
