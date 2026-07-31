//! Tests for the caller-supplied fact set validation service (ADR 0093 dec. 6,
//! epic #285 T6).

use rust_decimal::Decimal;

use super::*;
use crate::storage::{open_in_memory_database, AppState, NewCompany, StructuredFactInput};

fn d(v: i64) -> Decimal {
    Decimal::from(v)
}

fn state_with_company() -> (AppState, String) {
    let state = AppState::new(open_in_memory_database().expect("db"));
    let company = state
        .create_company(NewCompany {
            exchange: "GPW".to_owned(),
            ticker: "XTB".to_owned(),
            display_name: "XTB S.A.".to_owned(),
            isin: None,
            cik: None,
            lei: None,
        })
        .expect("company");
    (state, company.id)
}

/// Seeds one historical fact directly (bypassing the extraction pipeline) so
/// `metric_histories` has something to read for the metric/company under
/// test.
fn seed_history_fact(
    state: &AppState,
    company_id: &str,
    metric_key: &str,
    fiscal_year: i64,
    value: i64,
) {
    let value = value.to_string();
    state
        .kpi_extraction()
        .record_structured_fact(StructuredFactInput {
            company_id,
            fiscal_year,
            period_type: "FY",
            period_end: Some(&format!("{fiscal_year}-12-31")),
            report_document_id: "seed-doc",
            metric_key,
            value_numeric: &value,
            currency: Some("PLN"),
            confirmation_state: "confirmed",
            source_tier: "esef",
            extraction_method: "api",
            validation_status: "passed",
            drift_json: None,
            citation: Some("seed"),
            data_quality: None,
        })
        .expect("seed history fact");
}

fn candidate(metric_key: &str, value: i64) -> CandidateFact {
    CandidateFact {
        metric_key: metric_key.to_owned(),
        value: d(value),
    }
}

#[test]
fn pass_verdict_for_a_plausible_set() {
    let (state, company_id) = state_with_company();
    seed_history_fact(&state, &company_id, "revenue", 2024, 1_000_000);
    seed_history_fact(&state, &company_id, "revenue", 2025, 1_200_000);

    let facts = [candidate("revenue", 1_100_000)];
    let verdict = validate_supplied_set(&state, &company_id, 2026, "FY", &facts).expect("verdict");

    assert_eq!(verdict.facts, vec![FactVerdict::Pass]);
}

#[test]
fn implausible_verdict_carries_the_history_median() {
    let (state, company_id) = state_with_company();
    seed_history_fact(&state, &company_id, "cash", 2024, 100_000);
    seed_history_fact(&state, &company_id, "cash", 2025, 120_000);

    // ~167× the median of [100_000, 120_000] (120_000) — a dropped multiplier.
    let facts = [candidate("cash", 20_000_000)];
    let verdict = validate_supplied_set(&state, &company_id, 2026, "FY", &facts).expect("verdict");

    assert_eq!(
        verdict.facts,
        vec![FactVerdict::Implausible {
            history_median: d(120_000)
        }]
    );
}

#[test]
fn abstains_with_thin_history() {
    let (state, company_id) = state_with_company();
    // Exactly one stored point — no stable median (ADR 0093 dec. 6: XTB
    // total_equity's real shape).
    seed_history_fact(&state, &company_id, "total_equity", 2024, 500_000);

    let facts = [candidate("total_equity", 550_000)];
    let verdict = validate_supplied_set(&state, &company_id, 2026, "FY", &facts).expect("verdict");

    assert_eq!(verdict.facts, vec![FactVerdict::AbstainedThinHistory]);
}

#[test]
fn abstains_for_split_sensitive_metric_regardless_of_history_size() {
    let (state, company_id) = state_with_company();
    seed_history_fact(&state, &company_id, "eps_basic", 2024, 1);
    seed_history_fact(&state, &company_id, "eps_basic", 2025, 50);

    // Would be flagged implausible if eps_basic were not split-sensitive.
    let facts = [candidate("eps_basic", 500)];
    let verdict = validate_supplied_set(&state, &company_id, 2026, "FY", &facts).expect("verdict");

    assert_eq!(verdict.facts, vec![FactVerdict::AbstainedThinHistory]);
}

#[test]
fn abstains_for_a_zero_value() {
    let (state, company_id) = state_with_company();
    seed_history_fact(&state, &company_id, "ebitda", 2024, 1_000_000);
    seed_history_fact(&state, &company_id, "ebitda", 2025, 1_200_000);

    let facts = [candidate("ebitda", 0)];
    let verdict = validate_supplied_set(&state, &company_id, 2026, "FY", &facts).expect("verdict");

    assert_eq!(verdict.facts, vec![FactVerdict::AbstainedThinHistory]);
}

#[test]
fn identity_violation_attributed_to_every_fact_involved() {
    let (state, company_id) = state_with_company();

    // 200 + 200 = 400 != 1000 — well beyond tolerance.
    let facts = [
        candidate("total_assets", 1_000_000),
        candidate("total_liabilities", 200_000),
        candidate("total_equity", 200_000),
    ];
    let verdict = validate_supplied_set(&state, &company_id, 2026, "FY", &facts).expect("verdict");

    assert_eq!(
        verdict.facts,
        vec![
            FactVerdict::IdentityViolation {
                check: "balance_sheet_identity"
            },
            FactVerdict::IdentityViolation {
                check: "balance_sheet_identity"
            },
            FactVerdict::IdentityViolation {
                check: "balance_sheet_identity"
            },
        ]
    );
}

#[test]
fn completeness_is_informational_against_the_expected_primary_set() {
    let (state, company_id) = state_with_company();
    // A fresh company is core-seeded with 5 expected primary keys (migration
    // 0106): net_profit, operating_profit, revenue, total_assets,
    // total_equity. Supply two of them plus one non-core metric.
    let facts = [
        candidate("revenue", 1_000_000),
        candidate("total_assets", 2_000_000),
        candidate("some_other_metric", 42),
    ];
    let verdict = validate_supplied_set(&state, &company_id, 2026, "FY", &facts).expect("verdict");

    let completeness = verdict
        .completeness
        .expect("a core-seeded company has an expected set");
    assert_eq!(completeness.expected, 5);
    assert_eq!(completeness.present, 2);
    assert_eq!(completeness.missing.len(), 3);
    // Informational only — it never turns into a verdict entry.
    assert_eq!(verdict.facts.len(), facts.len());
}

#[test]
fn empty_input_is_a_no_op() {
    let (state, company_id) = state_with_company();
    let verdict = validate_supplied_set(&state, &company_id, 2026, "FY", &[]).expect("verdict");
    assert!(verdict.facts.is_empty());
    assert!(verdict.completeness.is_none());
}
