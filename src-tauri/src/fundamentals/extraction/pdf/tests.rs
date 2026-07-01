//! Tests for the deterministic PDF parser (ADR 0061 tier 2; ADR 0049).

use super::*;
use crate::fundamentals::extraction::fact_set_for_period;
use crate::fundamentals::validation::{validate_period, Status, Tolerance};

// Columns are separated by 2+ spaces (the tabular-PDF convention); a single
// interior space is a thousands separator.
const REPORT: &str = "\
Skonsolidowane sprawozdanie finansowe (dane w tys. zł)
Aktywa razem  45 000  40 000
Zobowiązania razem  20 000  18 000
Kapitał własny  12  25 000  22 000
Przychody netto ze sprzedaży  12 000  10 500
Zysk z działalności operacyjnej  1 500  1 200
Zysk netto  1 100  900
Środki pieniężne i ich ekwiwalenty  8 000  6 000
Przepływy pieniężne netto z działalności operacyjnej  6 000  5 000
Zysk na akcję  2,45  2,01
";

fn d(v: i64) -> Decimal {
    Decimal::from(v)
}

fn facts_map(text: &str) -> std::collections::BTreeMap<String, Decimal> {
    let parse = parse_pdf_text(text, "2026-03-31", None);
    parse
        .facts
        .into_iter()
        .map(|f| (f.metric_key, f.value))
        .collect()
}

// ---------------------------------------------------------------------------
// Unit scale
// ---------------------------------------------------------------------------

#[test]
fn detects_thousands_and_millions() {
    assert_eq!(detect_unit_scale("dane w tys. zł"), UnitScale::Thousands);
    assert_eq!(detect_unit_scale("wartości w mln zł"), UnitScale::Millions);
    // Default when nothing stated: thousands (the dominant convention).
    assert_eq!(detect_unit_scale("bez jednostki"), UnitScale::Thousands);
}

// ---------------------------------------------------------------------------
// Core extraction
// ---------------------------------------------------------------------------

#[test]
fn extracts_core_metrics_scaled_to_base_units() {
    let m = facts_map(REPORT);
    assert_eq!(m.get("total_assets"), Some(&d(45_000_000)));
    assert_eq!(m.get("total_liabilities"), Some(&d(20_000_000)));
    assert_eq!(m.get("total_equity"), Some(&d(25_000_000)));
    assert_eq!(m.get("revenue"), Some(&d(12_000_000)));
    assert_eq!(m.get("operating_profit"), Some(&d(1_500_000)));
    assert_eq!(m.get("net_profit"), Some(&d(1_100_000)));
    assert_eq!(m.get("cash"), Some(&d(8_000_000)));
    assert_eq!(m.get("operating_cash_flow"), Some(&d(6_000_000)));
}

#[test]
fn note_reference_is_skipped() {
    // "Kapitał własny 12 25 000 22 000" — 12 is a note ref, 25 000 the value.
    let m = facts_map(REPORT);
    assert_eq!(m.get("total_equity"), Some(&d(25_000_000)));
}

#[test]
fn current_period_column_is_first() {
    // Value is the first column (45 000), never the comparative (40 000).
    let m = facts_map(REPORT);
    assert_eq!(m.get("total_assets"), Some(&d(45_000_000)));
}

#[test]
fn per_share_is_not_scaled() {
    let m = facts_map(REPORT);
    assert_eq!(
        m.get("eps_basic"),
        Some(&Decimal::from_str_exact("2.45").unwrap())
    );
}

#[test]
fn parentheses_make_negative() {
    let text = "dane w tys. zł\nZysk netto  (1 100)  900\n";
    let m = facts_map(text);
    assert_eq!(m.get("net_profit"), Some(&d(-1_100_000)));
}

#[test]
fn millions_unit_scales_by_1e6() {
    let text = "dane w mln zł\nAktywa razem  45  40\n";
    let m = facts_map(text);
    assert_eq!(m.get("total_assets"), Some(&d(45_000_000)));
}

#[test]
fn extracted_set_passes_validation_gate() {
    let parse = parse_pdf_text(REPORT, "2026-03-31", None);
    let set = fact_set_for_period(&parse.facts, "2026-03-31");
    // 45m = 20m + 25m.
    assert_eq!(
        validate_period(&set, &Tolerance::default()).status,
        Status::Passed
    );
}

#[test]
fn columns_split_on_double_space_not_thousands_space() {
    // "1 100  900" → the single space groups thousands, the double space
    // separates columns: [1100, 900], never a single 1 100 900.
    let (_, tokens) = split_label_and_values("Zysk netto  1 100  900").unwrap();
    let values: Vec<Decimal> = tokens.iter().map(|t| t.value).collect();
    assert_eq!(values, vec![d(1100), d(900)]);
}

#[test]
fn ungrouped_multidigit_numbers_are_whole() {
    // Some extractions drop thousands spaces: "45000  40000" stays two whole
    // numbers, not truncated to 3 digits.
    let (_, tokens) = split_label_and_values("Aktywa razem  45000  40000").unwrap();
    let values: Vec<Decimal> = tokens.iter().map(|t| t.value).collect();
    assert_eq!(values, vec![d(45000), d(40000)]);
}

#[test]
fn golden_facts_snapshot() {
    let parse = parse_pdf_text(REPORT, "2026-03-31", None);
    insta::assert_debug_snapshot!("golden_pdf_facts", parse.facts);
}

mod properties {
    use super::*;
    use proptest::prelude::*;

    proptest! {
        // The parser must never panic on arbitrary text — a garbled extraction
        // yields no facts, it does not crash.
        #[test]
        fn parse_never_panics(s in ".{0,2000}") {
            let _ = parse_pdf_text(&s, "2026-03-31", None);
        }
    }
}
