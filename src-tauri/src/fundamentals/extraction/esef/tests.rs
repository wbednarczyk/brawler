//! Tests for the ESEF/iXBRL parser (ADR 0061 tier 1; ADR 0049 test
//! architecture: behavioural units + a golden snapshot + no-panic property).

use super::*;
use crate::fundamentals::extraction::fact_set_for_period;
use crate::fundamentals::validation::{validate_period, Status, Tolerance};

const SAMPLE: &str = r#"<?xml version="1.0" encoding="UTF-8"?>
<html xmlns:ix="http://www.xbrl.org/2013/inlineXBRL"
      xmlns:xbrli="http://www.xbrl.org/2003/instance"
      xmlns:ifrs-full="http://xbrl.ifrs.org/taxonomy/2023/ifrs-full"
      xmlns:iso4217="http://www.xbrl.org/2003/iso4217">
  <body>
    <ix:header><ix:resources>
      <xbrli:context id="c-cur-i">
        <xbrli:period><xbrli:instant>2026-03-31</xbrli:instant></xbrli:period>
      </xbrli:context>
      <xbrli:context id="c-prev-i">
        <xbrli:period><xbrli:instant>2025-03-31</xbrli:instant></xbrli:period>
      </xbrli:context>
      <xbrli:context id="c-cur-d">
        <xbrli:period>
          <xbrli:startDate>2026-01-01</xbrli:startDate>
          <xbrli:endDate>2026-03-31</xbrli:endDate>
        </xbrli:period>
      </xbrli:context>
      <xbrli:unit id="pln"><xbrli:measure>iso4217:PLN</xbrli:measure></xbrli:unit>
    </ix:resources></ix:header>
    <p>Aktywa: <ix:nonFraction name="ifrs-full:Assets" contextRef="c-cur-i" unitRef="pln" scale="3" format="ixt:num-space-comma-decimal">45 000</ix:nonFraction></p>
    <p>Zobowiązania: <ix:nonFraction name="ifrs-full:Liabilities" contextRef="c-cur-i" unitRef="pln" scale="3">20 000</ix:nonFraction></p>
    <p>Kapitał: <ix:nonFraction name="ifrs-full:Equity" contextRef="c-cur-i" unitRef="pln" scale="3">25 000</ix:nonFraction></p>
    <p>Przychody: <ix:nonFraction name="ifrs-full:Revenue" contextRef="c-cur-d" unitRef="pln" scale="3">12 000</ix:nonFraction></p>
    <p>Wynik operacyjny: <ix:nonFraction name="ifrs-full:ProfitLossFromOperatingActivities" contextRef="c-cur-d" unitRef="pln" scale="3" sign="-">1 500</ix:nonFraction></p>
    <p>Aktywa rok temu: <ix:nonFraction name="ifrs-full:Assets" contextRef="c-prev-i" unitRef="pln" scale="3">40 000</ix:nonFraction></p>
  </body>
</html>"#;

fn d(v: i64) -> Decimal {
    Decimal::from(v)
}

// ---------------------------------------------------------------------------
// Number parsing
// ---------------------------------------------------------------------------

#[test]
fn parses_space_comma_decimal() {
    assert_eq!(
        parse_ixbrl_number("1 234 567,89", Some("ixt:num-space-comma-decimal")),
        Some(Decimal::from_str_exact("1234567.89").unwrap())
    );
}

#[test]
fn parses_dot_decimal_with_comma_thousands() {
    assert_eq!(
        parse_ixbrl_number("1,234,567.89", Some("ixt:num-dot-decimal")),
        Some(Decimal::from_str_exact("1234567.89").unwrap())
    );
}

#[test]
fn parses_legacy_transform_names() {
    assert_eq!(
        parse_ixbrl_number("1.234,5", Some("ixt:numcommadecimal")),
        Some(Decimal::from_str_exact("1234.5").unwrap())
    );
}

#[test]
fn accounting_parentheses_are_negative() {
    assert_eq!(parse_ixbrl_number("(1 500)", None), Some(d(-1500)));
}

#[test]
fn nbsp_thousands_separator_is_stripped() {
    assert_eq!(parse_ixbrl_number("45\u{00A0}000", None), Some(d(45000)));
}

#[test]
fn empty_number_is_none() {
    assert_eq!(parse_ixbrl_number("   ", None), None);
}

#[test]
fn apply_scale_thousands() {
    assert_eq!(apply_scale(d(45), 3), Some(d(45000)));
    assert_eq!(apply_scale(d(45000), -3), Some(d(45)));
    assert_eq!(apply_scale(d(7), 0), Some(d(7)));
}

// ---------------------------------------------------------------------------
// Full parse
// ---------------------------------------------------------------------------

#[test]
fn parses_all_tracked_facts() {
    let facts = parse_esef(SAMPLE.as_bytes()).expect("sample parses");
    // 3 balance-sheet + revenue + operating_profit (current) + prior assets = 6.
    assert_eq!(facts.len(), 6);
    assert!(facts.iter().all(|f| f.tier == SourceTier::Esef));
    assert!(facts.iter().all(|f| f.currency.as_deref() == Some("PLN")));
    assert!(facts
        .iter()
        .all(|f| f.basis == Some(StatementBasis::Consolidated)));
}

#[test]
fn applies_scale_and_sign() {
    let facts = parse_esef(SAMPLE.as_bytes()).unwrap();
    let assets = facts
        .iter()
        .find(|f| f.metric_key == "total_assets" && f.period.end_date() == "2026-03-31")
        .unwrap();
    assert_eq!(assets.value, d(45_000_000)); // 45 000 × 10^3

    let op = facts
        .iter()
        .find(|f| f.metric_key == "operating_profit")
        .unwrap();
    assert_eq!(op.value, d(-1_500_000)); // sign="-" applied
}

#[test]
fn current_period_set_satisfies_balance_sheet() {
    let facts = parse_esef(SAMPLE.as_bytes()).unwrap();
    let set = fact_set_for_period(&facts, "2026-03-31");
    assert_eq!(set.get("total_assets"), Some(&d(45_000_000)));
    assert_eq!(set.get("total_liabilities"), Some(&d(20_000_000)));
    assert_eq!(set.get("total_equity"), Some(&d(25_000_000)));
    // The extracted current-period set passes the deterministic gate end-to-end.
    let report = validate_period(&set, &Tolerance::default());
    assert_eq!(report.status, Status::Passed);
}

#[test]
fn comparative_period_is_isolated() {
    let facts = parse_esef(SAMPLE.as_bytes()).unwrap();
    let prev = fact_set_for_period(&facts, "2025-03-31");
    assert_eq!(prev.get("total_assets"), Some(&d(40_000_000)));
    assert_eq!(prev.len(), 1); // only the prior assets were tagged for that date
}

#[test]
fn untracked_concepts_are_ignored() {
    let xml = r#"<html xmlns:ix="http://www.xbrl.org/2013/inlineXBRL" xmlns:xbrli="http://www.xbrl.org/2003/instance">
      <xbrli:context id="c"><xbrli:period><xbrli:instant>2026-03-31</xbrli:instant></xbrli:period></xbrli:context>
      <ix:nonFraction name="ext:SomeCompanySpecificDisclosure" contextRef="c">999</ix:nonFraction>
    </html>"#;
    assert_eq!(parse_esef(xml.as_bytes()), Err(EsefError::NoTrackedFacts));
}

#[test]
fn dedup_keeps_longest_duration() {
    // Both a 3-month and a YTD revenue end on the report date; keep the YTD.
    let xml = r#"<html xmlns:ix="http://www.xbrl.org/2013/inlineXBRL" xmlns:xbrli="http://www.xbrl.org/2003/instance">
      <xbrli:context id="q"><xbrli:period><xbrli:startDate>2026-04-01</xbrli:startDate><xbrli:endDate>2026-06-30</xbrli:endDate></xbrli:period></xbrli:context>
      <xbrli:context id="ytd"><xbrli:period><xbrli:startDate>2026-01-01</xbrli:startDate><xbrli:endDate>2026-06-30</xbrli:endDate></xbrli:period></xbrli:context>
      <ix:nonFraction name="ifrs-full:Revenue" contextRef="q" scale="0">100</ix:nonFraction>
      <ix:nonFraction name="ifrs-full:Revenue" contextRef="ytd" scale="0">250</ix:nonFraction>
    </html>"#;
    let facts = parse_esef(xml.as_bytes()).unwrap();
    let set = fact_set_for_period(&facts, "2026-06-30");
    assert_eq!(set.get("revenue"), Some(&d(250))); // YTD, not the 100 quarter
}

// ---------------------------------------------------------------------------
// Golden snapshot
// ---------------------------------------------------------------------------

#[test]
fn golden_parsed_facts() {
    let facts = parse_esef(SAMPLE.as_bytes()).unwrap();
    insta::assert_debug_snapshot!("golden_parsed_facts", facts);
}

// ---------------------------------------------------------------------------
// Robustness
// ---------------------------------------------------------------------------

#[test]
fn malformed_xml_errors_without_panic() {
    let res = parse_esef(b"<html><ix:nonFraction name=");
    assert!(matches!(
        res,
        Err(EsefError::Xml(_)) | Err(EsefError::NoTrackedFacts)
    ));
}

mod properties {
    use super::*;
    use proptest::prelude::*;

    proptest! {
        // The parser must never panic on arbitrary bytes — a hostile or
        // truncated attachment falls to the next tier, it does not crash.
        #[test]
        fn parse_never_panics(bytes in prop::collection::vec(any::<u8>(), 0..2048)) {
            let _ = parse_esef(&bytes);
        }

        // Number parsing never panics on arbitrary text.
        #[test]
        fn number_parse_never_panics(s in ".{0,64}") {
            let _ = parse_ixbrl_number(&s, None);
        }
    }
}
