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

// ---------------------------------------------------------------------------
// Real BiznesRadar markup: the value figure lives in `<span class="value">`,
// alongside a `<div class="changeyy">` carrying r/r & ~branża change percentages
// in the SAME cell. Reading whole-cell text yields "7 203 r/r -80.94% …" which
// is not a parseable number — the parser must select the value span. (ADR 0086.)
// ---------------------------------------------------------------------------

const BR_REAL_BILANS: &str = include_str!("../../../../samples/biznesradar_bilans_cdr.html");
const BR_REAL_PRZEPLYWY: &str = include_str!("../../../../samples/biznesradar_przeplywy_cdr.html");

#[test]
fn reads_value_span_ignoring_in_cell_change_percentages() {
    let facts = parse_html_financials(
        BR_REAL_BILANS,
        &AggregatorColumn {
            period_end: "2008-12-31",
            fiscal_year: 2008,
            period_hint: None,
        },
    );
    let map: std::collections::BTreeMap<_, _> = facts
        .iter()
        .map(|f| (f.metric_key.clone(), f.value))
        .collect();
    // 1 373 (tys. zł) → 1 373 000 base units, read from the 2008 column's value
    // span, NOT the "-80.94%" change figure sharing the cell.
    assert_eq!(map.get("inventories"), Some(&d(1_373_000)));
    assert_eq!(map.get("current_assets"), Some(&d(16_815_000)));
}

#[test]
fn parse_all_financials_reads_every_column_of_every_period() {
    let all = parse_all_financials(BR_REAL_BILANS);
    // One (period, facts) entry per annual column.
    assert!(
        all.len() >= 5,
        "expected >=5 period columns, got {}",
        all.len()
    );
    let p2008 = all
        .iter()
        .find(|(p, _)| p.fiscal_year == 2008)
        .map(|(_, f)| f)
        .expect("2008 column");
    let map: std::collections::BTreeMap<_, _> = p2008
        .iter()
        .map(|f| (f.metric_key.clone(), f.value))
        .collect();
    // 2008 column: Zapasy 1 373, Aktywa obrotowe 16 815 (tys. zł → base units).
    assert_eq!(map.get("inventories"), Some(&d(1_373_000)));
    assert_eq!(map.get("current_assets"), Some(&d(16_815_000)));
    // Facts carry the column's period end, not a single hard-coded one.
    assert!(p2008.iter().all(|f| f.period.end_date() == "2008-12-31"));
    assert!(p2008.iter().all(|f| f.tier == SourceTier::HtmlAggregator));
}

#[test]
fn parse_all_financials_reads_cash_flow_labels_without_netto() {
    // Real BR cash-flow labels omit "netto" ("Przepływy pieniężne z działalności
    // operacyjnej") — the dictionary must map them for the aggregator to source
    // cash flow at all.
    let all = parse_all_financials(BR_REAL_PRZEPLYWY);
    let has_ocf = all
        .iter()
        .any(|(_, facts)| facts.iter().any(|f| f.metric_key == "operating_cash_flow"));
    assert!(
        has_ocf,
        "operating_cash_flow must be read from real BR cash-flow labels"
    );
}

// BiznesRadar's BANK schema (verified live Bank Pekao / PEO pages, epic #277
// T1): net interest/fee income on the income statement, customer loans/
// deposits + a plural "Kapitały własne/razem" equity pair on the balance
// sheet — a different row set than the non-financial CDR samples above.
const BR_PEO_RZIS: &str = include_str!("../../../../samples/biznesradar_rzis_peo.html");
const BR_PEO_BILANS: &str = include_str!("../../../../samples/biznesradar_bilans_peo.html");

#[test]
fn parses_bank_schema_rows_into_bank_specific_metrics() {
    let income_map: std::collections::BTreeMap<_, _> = parse_all_financials(BR_PEO_RZIS)
        .iter()
        .flat_map(|(_, facts)| facts.iter())
        .map(|f| (f.metric_key.clone(), f.value))
        .collect();
    let nii = income_map
        .get("net_interest_income")
        .expect("net_interest_income mapped from 'Wynik z tytułu odsetek'");
    // Real PEO net interest income is in the low tens of billions PLN
    // (base units, page states tys. PLN).
    assert!(
        *nii > d(1_000_000_000) && *nii < d(30_000_000_000),
        "implausible net_interest_income: {nii}"
    );
    let nfi = income_map
        .get("net_fee_commission_income")
        .expect("net_fee_commission_income mapped from 'Wynik z tytułu prowizji'");
    assert!(
        *nfi > d(500_000_000) && *nfi < d(10_000_000_000),
        "implausible net_fee_commission_income: {nfi}"
    );

    let balance_map: std::collections::BTreeMap<_, _> = parse_all_financials(BR_PEO_BILANS)
        .iter()
        .flat_map(|(_, facts)| facts.iter())
        .map(|f| (f.metric_key.clone(), f.value))
        .collect();
    let loans = balance_map
        .get("total_loans")
        .expect("total_loans mapped from 'Należności od klientów'");
    assert!(
        *loans > d(50_000_000_000) && *loans < d(500_000_000_000),
        "implausible total_loans: {loans}"
    );
    let deposits = balance_map
        .get("total_deposits")
        .expect("total_deposits mapped from 'Zobowiązania wobec klientów'");
    assert!(
        *deposits > d(50_000_000_000) && *deposits < d(500_000_000_000),
        "implausible total_deposits: {deposits}"
    );
    let equity_parent = balance_map
        .get("wdf_equity_parent")
        .expect("wdf_equity_parent mapped from bank-schema plural 'Kapitały własne'");
    assert!(
        *equity_parent > d(5_000_000_000) && *equity_parent < d(100_000_000_000),
        "implausible wdf_equity_parent: {equity_parent}"
    );
    let total_equity = balance_map
        .get("total_equity")
        .expect("total_equity mapped from bank-schema group total 'Kapitały razem'");
    assert!(
        *total_equity > d(5_000_000_000) && *total_equity < d(100_000_000_000),
        "implausible total_equity: {total_equity}"
    );
    // Group total (parent + non-controlling) must be >= the parent-only figure.
    assert!(
        total_equity >= equity_parent,
        "group total_equity ({total_equity}) must be >= parent-only wdf_equity_parent ({equity_parent})"
    );
}

#[test]
fn period_header_parsing_handles_shifted_fiscal_year_month() {
    // A June fiscal-year end column "2013 (cze 13)" → FY 2013 ending 2013-06-30,
    // never forced to a calendar December (card c465c54).
    let p = parse_period_header("2013 (cze 13)").expect("annual header parses");
    assert_eq!(p.fiscal_year, 2013);
    assert_eq!(p.period_type, "FY");
    assert_eq!(p.period_end, "2013-06-30");
}

#[test]
fn glued_quarter_headers_read_the_quarter_digit_not_the_years_leading_digit() {
    // In "3q2024" the digit AFTER 'q' is the year's leading '2', which must
    // never win over the quarter digit before the 'q'.
    let q1 = parse_period_header("1Q2024").expect("glued Q1 parses");
    assert_eq!(
        (q1.period_type, q1.period_end.as_str()),
        ("Q1", "2024-03-31")
    );
    let q3 = parse_period_header("3Q2024").expect("glued Q3 parses");
    assert_eq!(
        (q3.period_type, q3.period_end.as_str()),
        ("Q3", "2024-09-30")
    );
    // The spaced forms keep working in both orders.
    let spaced = parse_period_header("Q3 2024").expect("spaced parses");
    assert_eq!(spaced.period_type, "Q3");
    let reversed = parse_period_header("2024 Q3").expect("reversed parses");
    assert_eq!(reversed.period_type, "Q3");
}

#[test]
fn february_fiscal_year_end_respects_leap_years() {
    // Review finding (2026-07-22): month_end hardcoded (2, 28).
    let leap = parse_period_header("2028 (lut 28)").expect("leap Feb parses");
    assert_eq!(leap.period_end, "2028-02-29");
    let common = parse_period_header("2027 (lut 27)").expect("common Feb parses");
    assert_eq!(common.period_end, "2027-02-28");
}

// -- Golden snapshot + proptest invariants (ADR 0049, review 2026-07-22) -----

/// The full multi-period extraction over the checked-in real BR balance page is
/// pinned as a golden snapshot: any drift in period enumeration, label mapping,
/// or unit scaling shows up as a reviewable diff, not a silent change.
#[test]
fn parse_all_financials_golden_snapshot() {
    let mut rows: Vec<String> = parse_all_financials(BR_REAL_BILANS)
        .iter()
        .flat_map(|(period, facts)| {
            facts.iter().map(move |f| {
                format!(
                    "{}|{}|{}|{}",
                    period.fiscal_year, period.period_end, f.metric_key, f.value
                )
            })
        })
        .collect();
    rows.sort();
    insta::assert_debug_snapshot!("br_bilans_all_periods_golden", rows);
}

mod period_header_invariants {
    use super::*;
    use proptest::prelude::*;

    proptest! {
        /// Never panics on arbitrary header text, and parsing is deterministic.
        #[test]
        fn never_panics_and_is_deterministic(header in "\\PC{0,60}") {
            let first = parse_period_header(&header);
            let second = parse_period_header(&header);
            prop_assert_eq!(first, second);
        }

        /// Every generated quarter header form maps to its canonical period and
        /// a period_end inside the fiscal year — glued, spaced, either order.
        #[test]
        fn quarter_headers_map_to_canonical_periods(
            year in 1990i64..=2099,
            quarter in 1u8..=4,
            form in 0u8..=3,
        ) {
            let header = match form {
                0 => format!("{quarter}Q{year}"),
                1 => format!("Q{quarter} {year}"),
                2 => format!("{year} Q{quarter}"),
                _ => format!("{quarter}q {year}"),
            };
            let parsed = parse_period_header(&header).expect("quarter header parses");
            prop_assert_eq!(parsed.fiscal_year, year);
            let expected = match quarter {
                1 => "Q1",
                2 => "H1",
                3 => "Q3",
                _ => "FY",
            };
            prop_assert_eq!(parsed.period_type, expected);
            let prefix = format!("{year}-");
            prop_assert!(parsed.period_end.starts_with(&prefix));
        }

        /// A bare-year annual header always resolves to FY ending in-year, and
        /// the period_end is a real calendar date (Feb 29 only in leap years).
        #[test]
        fn annual_headers_end_inside_the_fiscal_year(year in 1990i64..=2099) {
            let parsed = parse_period_header(&format!("{year}")).expect("annual parses");
            prop_assert_eq!(parsed.period_type, "FY");
            prop_assert_eq!(parsed.period_end, format!("{year}-12-31"));
        }
    }
}

/// Guardrail G2 (review 2026-07-22, finding-1 class A): the SOURCE-VOCABULARY
/// CONTRACT. Every statement row of every checked-in real BiznesRadar page is
/// pinned as `(data-field, Polish label, mapping outcome)` — mapped to its
/// metric, or EXPLICITLY unmapped. Any change reddens this snapshot and forces
/// a human decision:
/// - BR renames/adds a row → new line in the diff;
/// - a new dictionary entry's prefix silently captures someone else's row (the
///   exact mechanism that filed parent equity under group `total_equity`) →
///   that row's outcome flips in the diff.
///
/// An unmapped row here is a REVIEWED skip, not an accident.
#[test]
fn source_vocabulary_contract_golden() {
    use scraper::{Html, Selector};

    let samples: [(&str, &str); 5] = [
        (
            "rzis",
            include_str!("../../../../samples/biznesradar_rzis_cdr.html"),
        ),
        (
            "bilans",
            include_str!("../../../../samples/biznesradar_bilans_cdr.html"),
        ),
        (
            "przeplywy",
            include_str!("../../../../samples/biznesradar_przeplywy_cdr.html"),
        ),
        (
            "rzis-bank",
            include_str!("../../../../samples/biznesradar_rzis_peo.html"),
        ),
        (
            "bilans-bank",
            include_str!("../../../../samples/biznesradar_bilans_peo.html"),
        ),
    ];
    let row_sel = Selector::parse("tr[data-field]").unwrap();
    let cell_sel = Selector::parse("td").unwrap();

    let mut lines: Vec<String> = Vec::new();
    for (page, html) in samples {
        let document = Html::parse_document(html);
        for row in document.select(&row_sel) {
            let field = row.value().attr("data-field").unwrap_or_default();
            let Some(first_cell) = row.select(&cell_sel).next() else {
                continue;
            };
            let label =
                super::super::text_numbers::normalize_label(&first_cell.text().collect::<String>());
            if label.is_empty() {
                continue;
            }
            let outcome = super::super::text_numbers::match_dictionary_label(&label)
                .unwrap_or("(unmapped — reviewed skip)");
            lines.push(format!("{page} | {field} | {label} -> {outcome}"));
        }
    }
    assert!(
        lines.len() >= 30,
        "the vocabulary sweep must see the statement rows; got {lines:?}"
    );
    insta::assert_debug_snapshot!("source_vocabulary_contract", lines);
}
