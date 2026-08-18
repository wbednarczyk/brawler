//! End-to-end pipeline test (ADR 0061 S5, closes the ADR 0049 "no e2e ingestion
//! pipeline test" gap): the surviving chain — ESEF source-of-truth, the HTML
//! aggregator resort, the completeness downgrade and the comparative cross-check
//! — exercised deterministically over sample bytes.
//!
//! ADR 0100 (epic #398): the ESEF tier's candidate set is now a projection over
//! Layer 1 rows (`crate::storage::NewTaggedFact`), not a `parse_esef` pass over
//! raw bytes — these tests build `NewTaggedFact` fixtures directly (the same
//! layer `esef::projection`'s own tests exercise), with a presentation role
//! attached to every fact: role-less rows never project (decision 3), so a
//! fixture with no role would silently make every test below assert nothing.

use super::*;
use crate::fundamentals::extraction::SourceTier;
use crate::storage::NewTaggedFactRole;
use rust_decimal::Decimal;
use std::collections::BTreeSet;

const END: &str = "2026-03-31";
const PRIOR_END: &str = "2025-03-31";

fn d(v: i64) -> Decimal {
    Decimal::from(v)
}

fn balance_role() -> NewTaggedFactRole {
    NewTaggedFactRole {
        role_uri: "http://x/role/ias_1_role-210000".to_owned(),
        role_kind: "balance".to_owned(),
    }
}

/// A minimal dimensionless instant (balance-sheet) fact, classified `balance`.
fn balance_fact(concept: &str, identity: &str, period_end: &str, value: i64) -> NewTaggedFact {
    NewTaggedFact {
        fact_identity: identity.to_owned(),
        concept_namespace_uri: "https://xbrl.ifrs.org/taxonomy/2024-03-27/ifrs-full".to_owned(),
        concept_local_name: concept.to_owned(),
        period_type: "instant".to_owned(),
        period_end: period_end.to_owned(),
        unit_measure: Some("PLN".to_owned()),
        value_numeric: Some(value.to_string()),
        roles: vec![balance_role()],
        ..Default::default()
    }
}

/// A minimal balanced ESEF balance sheet at `END` (45m = 20m + 25m).
fn balanced_layer1_facts() -> Vec<NewTaggedFact> {
    vec![
        balance_fact("Assets", "assets", END, 45_000_000),
        balance_fact("Liabilities", "liabilities", END, 20_000_000),
        balance_fact("Equity", "equity", END, 25_000_000),
    ]
}

/// The same balance sheet, PLUS a comparative-period column (40m = 18m + 22m
/// at `PRIOR_END`) — the real shape of a tagged ESEF filing's comparative.
fn balanced_layer1_facts_with_prior() -> Vec<NewTaggedFact> {
    let mut facts = balanced_layer1_facts();
    facts.push(balance_fact(
        "Assets",
        "assets_prior",
        PRIOR_END,
        40_000_000,
    ));
    facts.push(balance_fact(
        "Liabilities",
        "liabilities_prior",
        PRIOR_END,
        18_000_000,
    ));
    facts.push(balance_fact(
        "Equity",
        "equity_prior",
        PRIOR_END,
        22_000_000,
    ));
    facts
}

fn stored(pairs: &[(&str, i64)]) -> FactSet {
    pairs.iter().map(|(k, v)| (k.to_string(), d(*v))).collect()
}

#[test]
fn esef_present_is_source_of_truth() {
    let facts = balanced_layer1_facts();
    let out = run_pipeline(&PipelineInput {
        period_end: END,
        layer1_facts: Some(&facts),
        has_presentation_linkbase: true,
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
fn no_projectable_layer1_rows_is_an_honest_empty_gap() {
    // An empty generation (e.g. a corrupt/unreadable instance upstream in
    // `esef::layer1` yielded nothing) must be an honest empty, never a panic.
    let out = run_pipeline(&PipelineInput {
        period_end: END,
        layer1_facts: Some(&[]),
        has_presentation_linkbase: true,
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
    let facts = balanced_layer1_facts_with_prior();
    let out = run_pipeline(&PipelineInput {
        period_end: END,
        layer1_facts: Some(&facts),
        has_presentation_linkbase: true,
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
    let facts = balanced_layer1_facts();
    let out = run_pipeline(&PipelineInput {
        period_end: END,
        layer1_facts: Some(&facts),
        has_presentation_linkbase: true,
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
    let facts = balanced_layer1_facts();
    let out = run_pipeline(&PipelineInput {
        period_end: END,
        layer1_facts: Some(&facts),
        has_presentation_linkbase: true,
        expected_keys: Some(&expected),
        ..Default::default()
    });
    assert_eq!(out.acceptance, Acceptance::Accepted);
}

// ---------------------------------------------------------------------------
// ADR 0100 decision 5 (epic #398): projected candidates pass the SAME
// validation gate as before — a failing set writes no facts.
// ---------------------------------------------------------------------------

#[test]
fn a_candidate_set_that_fails_validation_writes_no_facts() {
    // An unbalanced balance sheet (45m assets != 20m liabilities + 20m equity)
    // fails the balance-sheet identity — `Flagged`, and `facts` must be empty:
    // the gate genuinely ran on the PROJECTED candidates, not bypassed.
    let facts = vec![
        balance_fact("Assets", "assets", END, 45_000_000),
        balance_fact("Liabilities", "liabilities", END, 20_000_000),
        balance_fact("Equity", "equity", END, 20_000_000),
    ];
    let out = run_pipeline(&PipelineInput {
        period_end: END,
        layer1_facts: Some(&facts),
        has_presentation_linkbase: true,
        ..Default::default()
    });
    assert_eq!(out.acceptance, Acceptance::Flagged);
    assert!(!out.acceptance.emits());
    assert!(
        out.facts.is_empty(),
        "a flagged set must write no facts — the validation gate genuinely ran"
    );
}

// ---------------------------------------------------------------------------
// `has_presentation_linkbase = false` regression fix (epic #398): a bare
// iXBRL instance carries no linkbase, so its facts arrive with no roles
// attached at all — the strict path would classify every one of them as
// "not a primary statement line" and the document would silently project
// zero facts.
// ---------------------------------------------------------------------------

#[test]
fn a_bare_instance_with_no_linkbase_still_projects_its_crosswalked_facts() {
    let facts = vec![
        NewTaggedFact {
            fact_identity: "assets".to_owned(),
            concept_namespace_uri: "https://xbrl.ifrs.org/taxonomy/2024-03-27/ifrs-full".to_owned(),
            concept_local_name: "Assets".to_owned(),
            period_type: "instant".to_owned(),
            period_end: END.to_owned(),
            unit_measure: Some("PLN".to_owned()),
            value_numeric: Some("45000000".to_owned()),
            roles: Vec::new(),
            ..Default::default()
        },
        NewTaggedFact {
            fact_identity: "liabilities".to_owned(),
            concept_namespace_uri: "https://xbrl.ifrs.org/taxonomy/2024-03-27/ifrs-full".to_owned(),
            concept_local_name: "Liabilities".to_owned(),
            period_type: "instant".to_owned(),
            period_end: END.to_owned(),
            unit_measure: Some("PLN".to_owned()),
            value_numeric: Some("20000000".to_owned()),
            roles: Vec::new(),
            ..Default::default()
        },
        NewTaggedFact {
            fact_identity: "equity".to_owned(),
            concept_namespace_uri: "https://xbrl.ifrs.org/taxonomy/2024-03-27/ifrs-full".to_owned(),
            concept_local_name: "Equity".to_owned(),
            period_type: "instant".to_owned(),
            period_end: END.to_owned(),
            unit_measure: Some("PLN".to_owned()),
            value_numeric: Some("25000000".to_owned()),
            roles: Vec::new(),
            ..Default::default()
        },
    ];
    let out = run_pipeline(&PipelineInput {
        period_end: END,
        layer1_facts: Some(&facts),
        has_presentation_linkbase: false,
        ..Default::default()
    });

    assert_eq!(
        out.acceptance,
        Acceptance::Accepted,
        "the fallback selection must still pass the balance-sheet identity"
    );
    assert_eq!(out.tier, Some(SourceTier::Esef));
    assert_eq!(
        out.facts.len(),
        3,
        "a realistic count > 0 — every dimensionless, crosswalked fact projects"
    );
}
