//! Tests for the HTML aggregator adapter (ADR 0061 tier 3; ADR 0049).

use super::*;
use crate::fundamentals::extraction::{fact_set_for_period, SourceTier};
use crate::fundamentals::validation::{cross_check_prior, Outcome, Tolerance};
use rust_decimal::Decimal;

fn d(v: i64) -> Decimal {
    Decimal::from(v)
}

// A BiznesRadar-like multi-period table (values in tys. zł).
const MULTI: &str = r#"
<html><body>
<p>Dane w tys. zł</p>
<table>
  <tr><th>Pozycja</th><th>2025 Q1</th><th>2026 Q1</th></tr>
  <tr><td>Aktywa razem</td><td>40 000</td><td>45 000</td></tr>
  <tr><td>Zobowiązania razem</td><td>18 000</td><td>20 000</td></tr>
  <tr><td>Kapitał własny</td><td>22 000</td><td>25 000</td></tr>
</table>
</body></html>
"#;

#[test]
fn reads_requested_year_column() {
    let facts = parse_html_financials(
        MULTI,
        &AggregatorColumn {
            period_end: "2026-03-31",
            fiscal_year: 2026,
            period_hint: Some("Q1"),
        },
    );
    let map: std::collections::BTreeMap<_, _> = facts
        .iter()
        .map(|f| (f.metric_key.clone(), f.value))
        .collect();
    assert_eq!(map.get("total_assets"), Some(&d(45_000_000)));
    assert_eq!(map.get("total_liabilities"), Some(&d(20_000_000)));
    assert_eq!(map.get("total_equity"), Some(&d(25_000_000)));
    assert!(facts.iter().all(|f| f.tier == SourceTier::HtmlAggregator));
}

#[test]
fn selects_the_other_year_column() {
    let facts = parse_html_financials(
        MULTI,
        &AggregatorColumn {
            period_end: "2025-03-31",
            fiscal_year: 2025,
            period_hint: Some("Q1"),
        },
    );
    let map: std::collections::BTreeMap<_, _> = facts
        .iter()
        .map(|f| (f.metric_key.clone(), f.value))
        .collect();
    assert_eq!(map.get("total_assets"), Some(&d(40_000_000)));
}

#[test]
fn single_period_table_uses_sole_column() {
    let html = r#"<html><body><p>w tys. zł</p><table>
      <tr><th>Pozycja</th><th>2026</th></tr>
      <tr><td>Aktywa razem</td><td>45 000</td></tr>
    </table></body></html>"#;
    let facts = parse_html_financials(
        html,
        &AggregatorColumn {
            period_end: "2026-12-31",
            fiscal_year: 2099, // no header match → falls back to the sole column
            period_hint: None,
        },
    );
    assert_eq!(facts.len(), 1);
    assert_eq!(facts[0].value, d(45_000_000));
}

#[test]
fn witness_agrees_with_primary_filing() {
    // The aggregator confirms the primary (ESEF/PDF) figures → all cross-checks
    // pass, which is the ~100%-confidence agreement path.
    let agg = parse_html_financials(
        MULTI,
        &AggregatorColumn {
            period_end: "2026-03-31",
            fiscal_year: 2026,
            period_hint: Some("Q1"),
        },
    );
    let agg_set = fact_set_for_period(&agg, "2026-03-31");
    let mut primary = crate::fundamentals::validation::FactSet::new();
    primary.insert("total_assets".into(), d(45_000_000));
    primary.insert("total_equity".into(), d(25_000_000));

    let checks = cross_check_prior(&agg_set, &primary, &Tolerance::default());
    assert!(!checks.is_empty());
    assert!(checks.iter().all(|c| c.outcome == Outcome::Pass));
}

#[test]
fn witness_flags_disagreement_with_primary() {
    let agg = parse_html_financials(
        MULTI,
        &AggregatorColumn {
            period_end: "2026-03-31",
            fiscal_year: 2026,
            period_hint: Some("Q1"),
        },
    );
    let agg_set = fact_set_for_period(&agg, "2026-03-31");
    let mut primary = crate::fundamentals::validation::FactSet::new();
    // Primary read a wildly different assets figure → the witness flags it.
    primary.insert("total_assets".into(), d(9_000_000));

    let checks = cross_check_prior(&agg_set, &primary, &Tolerance::default());
    let assets = checks
        .iter()
        .find(|c| c.metric_key == "total_assets")
        .unwrap();
    assert!(assets.outcome.is_fail());
}

#[test]
fn empty_or_garbage_html_yields_no_facts() {
    assert!(parse_html_financials("", &col()).is_empty());
    assert!(parse_html_financials("<html><body>no tables here</body></html>", &col()).is_empty());
}

fn col() -> AggregatorColumn<'static> {
    AggregatorColumn {
        period_end: "2026-03-31",
        fiscal_year: 2026,
        period_hint: None,
    }
}
