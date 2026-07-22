//! End-to-end pipeline test (ADR 0061 S5, closes the ADR 0049 "no e2e ingestion
//! pipeline test" gap): the surviving chain — ESEF source-of-truth, the HTML
//! aggregator resort, the completeness downgrade and the comparative cross-check
//! — exercised deterministically over sample bytes. The PDF fact arm (parse +
//! profile + drift) is retired (ADR 0086 dec. 1), so those tiers no longer exist.

use super::*;
use crate::fundamentals::extraction::SourceTier;
use rust_decimal::Decimal;
use std::collections::BTreeSet;

const END: &str = "2026-03-31";

// Minimal balanced ESEF instance (45m = 20m + 25m at the period end).
const ESEF: &str = r#"<html xmlns:ix="http://www.xbrl.org/2013/inlineXBRL"
  xmlns:xbrli="http://www.xbrl.org/2003/instance"
  xmlns:iso4217="http://www.xbrl.org/2003/iso4217">
  <xbrli:context id="c"><xbrli:period><xbrli:instant>2026-03-31</xbrli:instant></xbrli:period></xbrli:context>
  <xbrli:unit id="pln"><xbrli:measure>iso4217:PLN</xbrli:measure></xbrli:unit>
  <ix:nonFraction name="ifrs-full:Assets" contextRef="c" unitRef="pln" scale="3">45 000</ix:nonFraction>
  <ix:nonFraction name="ifrs-full:Liabilities" contextRef="c" unitRef="pln" scale="3">20 000</ix:nonFraction>
  <ix:nonFraction name="ifrs-full:Equity" contextRef="c" unitRef="pln" scale="3">25 000</ix:nonFraction>
</html>"#;

fn d(v: i64) -> Decimal {
    Decimal::from(v)
}

// A prior-period ESEF instance carrying a second, distinct context (comparative
// column of a tagged ESEF filing). 40m = 18m + 22m at the prior period end.
const ESEF_WITH_PRIOR: &str = r#"<html xmlns:ix="http://www.xbrl.org/2013/inlineXBRL"
  xmlns:xbrli="http://www.xbrl.org/2003/instance"
  xmlns:iso4217="http://www.xbrl.org/2003/iso4217">
  <xbrli:context id="c"><xbrli:period><xbrli:instant>2026-03-31</xbrli:instant></xbrli:period></xbrli:context>
  <xbrli:context id="p"><xbrli:period><xbrli:instant>2025-03-31</xbrli:instant></xbrli:period></xbrli:context>
  <xbrli:unit id="pln"><xbrli:measure>iso4217:PLN</xbrli:measure></xbrli:unit>
  <ix:nonFraction name="ifrs-full:Assets" contextRef="c" unitRef="pln" scale="3">45 000</ix:nonFraction>
  <ix:nonFraction name="ifrs-full:Liabilities" contextRef="c" unitRef="pln" scale="3">20 000</ix:nonFraction>
  <ix:nonFraction name="ifrs-full:Equity" contextRef="c" unitRef="pln" scale="3">25 000</ix:nonFraction>
  <ix:nonFraction name="ifrs-full:Assets" contextRef="p" unitRef="pln" scale="3">40 000</ix:nonFraction>
  <ix:nonFraction name="ifrs-full:Liabilities" contextRef="p" unitRef="pln" scale="3">18 000</ix:nonFraction>
  <ix:nonFraction name="ifrs-full:Equity" contextRef="p" unitRef="pln" scale="3">22 000</ix:nonFraction>
</html>"#;

const PRIOR_END: &str = "2025-03-31";

fn stored(pairs: &[(&str, i64)]) -> FactSet {
    pairs.iter().map(|(k, v)| (k.to_string(), d(*v))).collect()
}

#[test]
fn esef_present_is_source_of_truth() {
    let out = run_pipeline(&PipelineInput {
        period_end: END,
        esef_bytes: Some(ESEF.as_bytes()),
        ..Default::default()
    });
    assert_eq!(out.acceptance, Acceptance::Accepted);
    assert_eq!(out.tier, Some(SourceTier::Esef));
    assert_eq!(out.acceptance.validation_status(), "passed");
}

#[test]
fn nothing_available_is_empty() {
    let out = run_pipeline(&PipelineInput {
        period_end: END,
        ..Default::default()
    });
    assert_eq!(out.acceptance, Acceptance::Empty);
    assert!(!out.acceptance.emits());
}

#[test]
fn corrupt_esef_is_an_honest_empty_gap() {
    // With the PDF fact arm and the aggregator-fallback arm retired (ADR 0086),
    // an unparsable ESEF leaves nothing to source — an honest empty gap (the
    // BR-primary pull fills the period from its own job, never this pipeline).
    let out = run_pipeline(&PipelineInput {
        period_end: END,
        esef_bytes: Some(b"<html>not xbrl</html>"),
        ..Default::default()
    });
    assert_eq!(out.acceptance, Acceptance::Empty);
    assert!(!out.acceptance.emits());
}

// ---------------------------------------------------------------------------
// Comparative-period cross-check (ADR 0061 dec. 4b) lives in the pipeline.
// ---------------------------------------------------------------------------

#[test]
fn esef_comparative_cross_check_matching_stored_prior_stays_accepted() {
    let stored_prior = stored(&[
        ("total_assets", 40_000_000),
        ("total_liabilities", 18_000_000),
        ("total_equity", 22_000_000),
    ]);
    let out = run_pipeline(&PipelineInput {
        period_end: END,
        esef_bytes: Some(ESEF_WITH_PRIOR.as_bytes()),
        prior_period_end: Some(PRIOR_END),
        prior: Some(&stored_prior),
        ..Default::default()
    });
    assert_eq!(out.acceptance, Acceptance::Accepted);
    assert_eq!(out.tier, Some(SourceTier::Esef));
    assert!(out.facts.iter().all(|f| f.period.end_date() == END));
}

// ---------------------------------------------------------------------------
// Completeness gate (ADR 0061 dec. 4d): report-only, downgrades acceptance
// only on zero overlap — never blocks emission.
// ---------------------------------------------------------------------------

#[test]
fn zero_overlap_expected_keys_downgrades_to_accepted_unreviewed() {
    // The ESEF balance sheet validates cleanly, but none of the expected primary
    // KPIs ("revenue") showed up — likely the wrong table — so the acceptance is
    // downgraded even though nothing contradicts.
    let expected: BTreeSet<String> = ["revenue".to_string()].into_iter().collect();
    let out = run_pipeline(&PipelineInput {
        period_end: END,
        esef_bytes: Some(ESEF.as_bytes()),
        expected_keys: Some(&expected),
        ..Default::default()
    });
    assert_eq!(out.acceptance, Acceptance::AcceptedUnreviewed);
    assert_eq!(out.status, Status::Passed, "validation itself still passed");
    assert!(
        out.acceptance.emits(),
        "a downgrade must never block emission"
    );
    assert!(!out.facts.is_empty());
}

#[test]
fn overlapping_expected_keys_keeps_full_accepted() {
    let expected: BTreeSet<String> = ["total_assets".to_string()].into_iter().collect();
    let out = run_pipeline(&PipelineInput {
        period_end: END,
        esef_bytes: Some(ESEF.as_bytes()),
        expected_keys: Some(&expected),
        ..Default::default()
    });
    assert_eq!(out.acceptance, Acceptance::Accepted);
}
