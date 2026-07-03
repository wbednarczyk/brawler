//! End-to-end pipeline test (ADR 0061 S5, closes the ADR 0049 "no e2e ingestion
//! pipeline test" gap): the whole chain — tier order, the "good" gate, witness
//! fallback, drift — exercised deterministically over sample bytes/text.

use super::*;
use crate::fundamentals::extraction::pdf::parse_pdf_text;
use crate::fundamentals::extraction::profile::ExtractionProfile;
use crate::fundamentals::extraction::{ExtractedFact, FactPeriod, SourceTier};
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

// A balanced PDF (45m = 20m + 25m).
const PDF_OK: &str = "\
dane w tys. zł
Aktywa razem  45 000  40 000
Zobowiązania razem  20 000  18 000
Kapitał własny  25 000  22 000
";

// A PDF that does NOT balance (equity misread as 30m): 45 ≠ 20 + 30.
const PDF_BROKEN: &str = "\
dane w tys. zł
Aktywa razem  45 000  40 000
Zobowiązania razem  20 000  18 000
Kapitał własny  30 000  22 000
";

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

fn witness(pairs: &[(&str, i64)]) -> Vec<ExtractedFact> {
    pairs
        .iter()
        .map(|(k, v)| ExtractedFact {
            metric_key: k.to_string(),
            value: d(*v),
            period: FactPeriod::Instant(END.to_string()),
            basis: None,
            currency: None,
            tier: SourceTier::HtmlAggregator,
            citation: k.to_string(),
        })
        .collect()
}

#[test]
fn esef_present_is_source_of_truth() {
    let out = run_pipeline(&PipelineInput {
        period_end: END,
        esef_bytes: Some(ESEF.as_bytes()),
        pdf_text: Some(PDF_OK),
        ..Default::default()
    });
    assert_eq!(out.acceptance, Acceptance::Accepted);
    assert_eq!(out.tier, Some(SourceTier::Esef));
    assert_eq!(out.acceptance.validation_status(), "passed");
}

#[test]
fn pdf_clean_path_auto_accepts() {
    let out = run_pipeline(&PipelineInput {
        period_end: END,
        pdf_text: Some(PDF_OK),
        ..Default::default()
    });
    assert_eq!(out.acceptance, Acceptance::Accepted);
    assert_eq!(out.tier, Some(SourceTier::Pdf));
    assert!(out.facts.iter().any(|f| f.metric_key == "total_assets"));
}

#[test]
fn pdf_failing_validation_without_witness_is_flagged() {
    let out = run_pipeline(&PipelineInput {
        period_end: END,
        pdf_text: Some(PDF_BROKEN),
        ..Default::default()
    });
    assert_eq!(out.acceptance, Acceptance::Flagged);
    assert!(out.facts.is_empty(), "flagged facts must not be emitted");
    assert!(!out.acceptance.emits());
}

#[test]
fn pdf_failing_but_witness_corroborates_is_accepted_via_witness() {
    // PDF self-fails the balance identity, but the aggregator agrees with the
    // PDF's figures → accept, witness-confirmed. (Witness carries the same
    // numbers the PDF read; agreement is the signal, not re-validation.)
    let w = witness(&[
        ("total_assets", 45_000_000),
        ("total_liabilities", 20_000_000),
        ("total_equity", 30_000_000),
    ]);
    let out = run_pipeline(&PipelineInput {
        period_end: END,
        pdf_text: Some(PDF_BROKEN),
        witness: Some(&w),
        ..Default::default()
    });
    // Balance identity fails (45≠20+30) even with agreement → still flagged,
    // because the witness cannot rescue a self-contradiction.
    assert_eq!(out.acceptance, Acceptance::Flagged);
}

#[test]
fn pdf_drift_with_agreeing_witness_accepts_via_witness() {
    // Profile expects a different layout (drift), but the parse validates and
    // the witness agrees → accept via witness, carrying the drift diff.
    let profile = ExtractionProfile::bootstrap(
        "cmp",
        &parse_pdf_text(
            "dane w tys. zł\nAktywa razem  1  1\nZobowiązania razem  1  1\nKapitał własny  1  1\nŚrodki pieniężne i ich ekwiwalenty  1  1\n",
            END,
            None,
        ),
    );
    let w = witness(&[
        ("total_assets", 45_000_000),
        ("total_liabilities", 20_000_000),
        ("total_equity", 25_000_000),
    ]);
    let out = run_pipeline(&PipelineInput {
        period_end: END,
        pdf_text: Some(PDF_OK),
        profile: Some(&profile),
        witness: Some(&w),
        ..Default::default()
    });
    assert_eq!(out.acceptance, Acceptance::AcceptedViaWitness);
    assert!(
        out.drift.is_some(),
        "drift diff should accompany the acceptance"
    );
}

#[test]
fn pdf_drift_without_witness_is_flagged() {
    let profile = ExtractionProfile::bootstrap(
        "cmp",
        &parse_pdf_text(
            "dane w tys. zł\nŚrodki pieniężne i ich ekwiwalenty  1  1\n",
            END,
            None,
        ),
    );
    let out = run_pipeline(&PipelineInput {
        period_end: END,
        pdf_text: Some(PDF_OK),
        profile: Some(&profile),
        ..Default::default()
    });
    assert_eq!(out.acceptance, Acceptance::Flagged);
    assert!(out.drift.is_some());
}

#[test]
fn inconclusive_pdf_without_witness_is_accepted_unreviewed() {
    // Only revenue present: no identity is checkable, nothing contradicts.
    let out = run_pipeline(&PipelineInput {
        period_end: END,
        pdf_text: Some("dane w tys. zł\nPrzychody netto ze sprzedaży  12 000  10 500\n"),
        ..Default::default()
    });
    assert_eq!(out.acceptance, Acceptance::AcceptedUnreviewed);
    assert_eq!(out.acceptance.validation_status(), "unreviewed");
    assert!(out.facts.iter().any(|f| f.metric_key == "revenue"));
}

#[test]
fn html_only_fallback_is_used_when_no_filing() {
    let w = witness(&[
        ("total_assets", 45_000_000),
        ("total_liabilities", 20_000_000),
        ("total_equity", 25_000_000),
    ]);
    let out = run_pipeline(&PipelineInput {
        period_end: END,
        witness: Some(&w),
        ..Default::default()
    });
    assert_eq!(out.acceptance, Acceptance::Accepted);
    assert_eq!(out.tier, Some(SourceTier::HtmlAggregator));
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
fn corrupt_esef_falls_through_to_pdf() {
    let out = run_pipeline(&PipelineInput {
        period_end: END,
        esef_bytes: Some(b"<html>not xbrl</html>"),
        pdf_text: Some(PDF_OK),
        ..Default::default()
    });
    assert_eq!(out.tier, Some(SourceTier::Pdf));
    assert_eq!(out.acceptance, Acceptance::Accepted);
}

// ---------------------------------------------------------------------------
// Comparative-period cross-check (ADR 0061 dec. 4b) live in the pipeline.
// ---------------------------------------------------------------------------

#[test]
fn pdf_comparative_cross_check_matching_stored_prior_stays_accepted() {
    // PDF_OK's second column (40m/18m/22m) matches exactly what is already
    // stored for the prior period — the cross-check corroborates, stays
    // Accepted (not merely uncontradicted).
    let stored_prior = stored(&[
        ("total_assets", 40_000_000),
        ("total_liabilities", 18_000_000),
        ("total_equity", 22_000_000),
    ]);
    let out = run_pipeline(&PipelineInput {
        period_end: END,
        pdf_text: Some(PDF_OK),
        prior_period_end: Some(PRIOR_END),
        prior: Some(&stored_prior),
        ..Default::default()
    });
    assert_eq!(out.acceptance, Acceptance::Accepted);
    assert_eq!(out.tier, Some(SourceTier::Pdf));
    // Only current-period facts are emitted — the comparative rows never
    // pollute the persisted set.
    assert!(out.facts.iter().all(|f| f.period.end_date() == END));
}

#[test]
fn pdf_comparative_cross_check_mismatch_is_flagged_without_witness() {
    // The report's comparative column (40m) disagrees with what is already
    // stored for that period (999m) — a column-misalignment signal — and no
    // witness is available to corroborate, so it must flag, never emit.
    let stored_prior = stored(&[
        ("total_assets", 999_000_000),
        ("total_liabilities", 18_000_000),
        ("total_equity", 22_000_000),
    ]);
    let out = run_pipeline(&PipelineInput {
        period_end: END,
        pdf_text: Some(PDF_OK),
        prior_period_end: Some(PRIOR_END),
        prior: Some(&stored_prior),
        ..Default::default()
    });
    assert_eq!(out.acceptance, Acceptance::Flagged);
    assert!(out.facts.is_empty());
    assert!(!out.acceptance.emits());
}

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
    // PDF_OK's balance sheet validates cleanly, but none of the expected
    // primary KPIs ("revenue") showed up — the parse likely read the wrong
    // table, so the acceptance is downgraded even though nothing contradicts.
    let expected: BTreeSet<String> = ["revenue".to_string()].into_iter().collect();
    let out = run_pipeline(&PipelineInput {
        period_end: END,
        pdf_text: Some(PDF_OK),
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
        pdf_text: Some(PDF_OK),
        expected_keys: Some(&expected),
        ..Default::default()
    });
    assert_eq!(out.acceptance, Acceptance::Accepted);
}
