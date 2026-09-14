//! #508 (ADR 0100 decisions 2/3/4): one statement basis per ESEF document,
//! job-level through `run_structured_extraction`. A child of the sibling of
//! the colocated `tests` module — declared externally (the
//! `role_families_tests`/`unit_refusal_tests` precedent: the coverage
//! summary forbids a nested `mod x;` inside an inline test module), so the
//! pinned parent stays under its file-size ratchet.

use super::*;
use crate::storage::MODE_AUTOPILOT;
use crate::test_support::{
    esef_presentation_linkbase_xml as presentation_linkbase_xml, minimal_zip,
    seed_document_with_bytes,
};

const BALANCE_ROLES: &[(&str, &str)] = &[
    ("Assets", "ias_1_role-210000"),
    ("Liabilities", "ias_1_role-210000"),
    ("Equity", "ias_1_role-210000"),
    ("CurrentLiabilities", "ias_1_role-210000"),
];

/// One instant (balance-sheet) instance tagging `entries` at `2025-12-31`.
fn instant_instance(entries: &[(&str, &str)]) -> String {
    let facts: String = entries
        .iter()
        .map(|(concept, value)| {
            format!(
                r#"  <ix:nonFraction name="ifrs-full:{concept}" contextRef="i" unitRef="pln">{value}</ix:nonFraction>"#
            )
        })
        .collect::<Vec<_>>()
        .join("\n");
    format!(
        r#"<html xmlns:ix="http://www.xbrl.org/2013/inlineXBRL"
  xmlns:ifrs-full="https://xbrl.ifrs.org/taxonomy/2024-03-27/ifrs-full"
  xmlns:xbrli="http://www.xbrl.org/2003/instance"
  xmlns:iso4217="http://www.xbrl.org/2003/iso4217">
  <xbrli:context id="i"><xbrli:period><xbrli:instant>2025-12-31</xbrli:instant></xbrli:period></xbrli:context>
  <xbrli:unit id="pln"><xbrli:measure>iso4217:PLN</xbrli:measure></xbrli:unit>
{facts}
</html>"#
    )
}

/// The contract's asymmetric fixture (#508): a consolidated instance
/// carrying the balanced identity (`Assets 100 = Liabilities 60 + Equity
/// 40`) plus a standalone instance duplicating `Assets` (50) AND carrying
/// its own exclusive `CurrentLiabilities` (30) — one shared presentation
/// linkbase covering both instances' concepts.
fn mixed_basis_package() -> Vec<u8> {
    let consolidated = instant_instance(&[
        ("Assets", "100000000"),
        ("Liabilities", "60000000"),
        ("Equity", "40000000"),
    ]);
    let standalone =
        instant_instance(&[("Assets", "50000000"), ("CurrentLiabilities", "30000000")]);
    let pre_xml = presentation_linkbase_xml(BALANCE_ROLES);
    minimal_zip(&[
        (
            "pkg/reports/skonsolidowane/instance.xhtml",
            consolidated.as_bytes(),
        ),
        (
            "pkg/reports/jednostkowe/instance.xhtml",
            standalone.as_bytes(),
        ),
        ("pkg/www/instance_pre.xml", pre_xml.as_bytes()),
    ])
}

/// A standalone-ONLY package — no consolidated instance at all (Test F).
fn standalone_only_package() -> Vec<u8> {
    let standalone = instant_instance(&[
        ("Assets", "50000000"),
        ("Liabilities", "20000000"),
        ("Equity", "30000000"),
    ]);
    let pre_xml = presentation_linkbase_xml(BALANCE_ROLES);
    minimal_zip(&[
        (
            "pkg/reports/jednostkowe/instance.xhtml",
            standalone.as_bytes(),
        ),
        ("pkg/www/instance_pre.xml", pre_xml.as_bytes()),
    ])
}

fn stored_facts(state: &AppState, company_id: &str) -> Vec<crate::storage::FinancialFact> {
    state
        .list_financial_facts(crate::storage::ListFinancialFactsInput {
            company_id: Some(company_id.to_owned()),
            period_id: None,
            definition_id: None,
        })
        .expect("list facts")
}

/// Test E (#508): the mixed-basis fixture through `run_structured_extraction`
/// stores EXACTLY the three consolidated metrics, all `statement_basis ==
/// "consolidated"`; no `current_liabilities` row at all (the standalone
/// occurrence is dropped, never merged/mislabeled); Layer 1 rows for BOTH
/// instances are retained (`report_tagged_facts` coverage still counts all 5
/// raw rows, 2 of them `other_basis`); a second identical run re-observes
/// the same fact ids, no new rows.
#[test]
fn mixed_basis_package_stores_only_the_consolidated_slots() {
    let bytes = mixed_basis_package();
    let (state, company_id, document_id) =
        seed_document_with_bytes("basis-e", "BSE", "Basis E", "report.xbri", &bytes);
    let document = state.get_report_document(&document_id).expect("document");
    let (fiscal_year, period_type, period_end) =
        derive_report_period(&state, &document).expect("period derives");

    let result = run_structured_extraction(
        &state,
        &company_id,
        &document_id,
        fiscal_year,
        period_type,
        &period_end,
        MODE_AUTOPILOT,
    )
    .expect("run must not abort");
    assert_eq!(result.tier, Some(SourceTier::Esef));

    let facts = stored_facts(&state, &company_id);
    let mut got: Vec<(String, String)> = facts
        .iter()
        .map(|f| (f.metric_key.clone(), f.statement_basis.clone()))
        .collect();
    got.sort();
    assert_eq!(
        got,
        vec![
            ("total_assets".to_owned(), "consolidated".to_owned()),
            ("total_equity".to_owned(), "consolidated".to_owned()),
            ("total_liabilities".to_owned(), "consolidated".to_owned()),
        ]
    );
    assert!(
        !facts.iter().any(|f| f.metric_key == "current_liabilities"),
        "the standalone-exclusive metric must never be stored: {facts:?}"
    );

    let coverage = state
        .report_tagged_facts()
        .coverage_counts(&company_id)
        .expect("coverage counts");
    assert_eq!(
        coverage.raw_stored, 5,
        "Layer 1 keeps BOTH instances' occurrences, never narrowed by basis"
    );
    assert_eq!(coverage.other_basis, 2);

    let mut first_ids = result.produced_fact_ids.clone();
    first_ids.sort();

    let second = run_structured_extraction(
        &state,
        &company_id,
        &document_id,
        fiscal_year,
        period_type,
        &period_end,
        MODE_AUTOPILOT,
    )
    .expect("second run must not abort");
    let mut second_ids = second.skipped_fact_ids.clone();
    second_ids.sort();
    assert!(
        second.produced_fact_ids.is_empty(),
        "a repeat run must re-observe, never create: {second:?}"
    );
    assert_eq!(
        second_ids, first_ids,
        "a repeat run re-observes the SAME fact ids the first run produced"
    );
}

/// Test F (#508 decision 3): a standalone-ONLY package stores facts with
/// `statement_basis == "standalone"` (master: the `slot_dims` default
/// stamped every writer's facts `consolidated`, mislabeling the filing). A
/// pre-seeded manual `consolidated` fact for the SAME metric+period coexists
/// as a SEPARATE row (statement basis is a real slot dimension, ADR 0093) —
/// the canonical read (`list_financial_facts`'s `CANONICAL_FACT_PREFERENCE_
/// ORDER`, `fact_preference.rs`) still returns the consolidated one first,
/// preserving the display preference (astra r1 f2).
#[test]
fn standalone_only_package_stores_standalone_the_canonical_read_still_prefers_consolidated() {
    let bytes = standalone_only_package();
    let (state, company_id, document_id) =
        seed_document_with_bytes("basis-f", "BSF", "Basis F", "report.xbri", &bytes);
    let document = state.get_report_document(&document_id).expect("document");
    let (fiscal_year, period_type, period_end) =
        derive_report_period(&state, &document).expect("period derives");

    let result = run_structured_extraction(
        &state,
        &company_id,
        &document_id,
        fiscal_year,
        period_type,
        &period_end,
        MODE_AUTOPILOT,
    )
    .expect("run must not abort");
    assert_eq!(result.tier, Some(SourceTier::Esef));

    let facts = stored_facts(&state, &company_id);
    let standalone_assets = facts
        .iter()
        .find(|f| f.metric_key == "total_assets")
        .expect("total_assets stored");
    assert_eq!(standalone_assets.statement_basis, "standalone");

    state
        .financials()
        .create_financial_fact(crate::storage::NewFinancialFact {
            company_id: company_id.clone(),
            period_id: standalone_assets.period_id.clone(),
            definition_id: standalone_assets.definition_id.clone(),
            value_numeric: "999000000".to_owned(),
            currency: Some("PLN".to_owned()),
            statement_basis: Some("consolidated".to_owned()),
            attribution: None,
            variant: None,
            measure_window: None,
            data_quality: None,
            as_reported_value: None,
            as_reported_scale: None,
            reporting_standard: None,
            extraction_method: None,
            confidence: None,
            confirmation_state: Some("confirmed".to_owned()),
            supersedes_id: None,
            source_document_ref: None,
            annotation: None,
        })
        .expect("manual consolidated fact");

    let canonical = state
        .list_financial_facts(crate::storage::ListFinancialFactsInput {
            company_id: Some(company_id.clone()),
            period_id: None,
            definition_id: None,
        })
        .expect("list facts")
        .into_iter()
        .find(|f| f.metric_key == "total_assets")
        .expect("total_assets still resolves");
    assert_eq!(
        canonical.statement_basis, "consolidated",
        "the canonical read still prefers consolidated even with a standalone \
         sibling stored for the same metric+period"
    );
}

/// Test G (#508 decision 4): a stored prior period holding ONLY
/// `consolidated` facts never substitutes for a standalone run's cross-check
/// prior or plausibility history — both abstain (`None`/empty), never fall
/// back to the other basis; the unfiltered (`None`) read is unchanged
/// (preservation).
#[test]
fn a_consolidated_only_prior_period_abstains_under_a_standalone_basis_filter() {
    let bytes = mixed_basis_package();
    let (state, company_id, document_id) =
        seed_document_with_bytes("basis-g", "BSG", "Basis G", "report.xbri", &bytes);
    let document = state.get_report_document(&document_id).expect("document");
    let (fiscal_year, period_type, period_end) =
        derive_report_period(&state, &document).expect("period derives");
    run_structured_extraction(
        &state,
        &company_id,
        &document_id,
        fiscal_year,
        period_type,
        &period_end,
        MODE_AUTOPILOT,
    )
    .expect("seed run");

    let prior_unfiltered = state
        .financials()
        .stored_fact_set_for_cross_check(
            &company_id,
            fiscal_year,
            period_type,
            SourceTier::Esef,
            None,
        )
        .expect("unfiltered prior query")
        .expect("consolidated facts form the unfiltered prior");
    assert!(prior_unfiltered.contains_key("total_assets"));

    let prior_consolidated = state
        .financials()
        .stored_fact_set_for_cross_check(
            &company_id,
            fiscal_year,
            period_type,
            SourceTier::Esef,
            Some("consolidated"),
        )
        .expect("consolidated-filtered prior query")
        .expect("the stored facts ARE consolidated");
    assert!(prior_consolidated.contains_key("total_assets"));

    let prior_standalone = state
        .financials()
        .stored_fact_set_for_cross_check(
            &company_id,
            fiscal_year,
            period_type,
            SourceTier::Esef,
            Some("standalone"),
        )
        .expect("standalone-filtered prior query");
    assert!(
        prior_standalone.is_none(),
        "a consolidated-only prior must never substitute under a standalone filter"
    );

    let keys: BTreeSet<String> = ["total_assets".to_owned()].into_iter().collect();
    let histories_unfiltered = state
        .financials()
        .metric_histories(&company_id, &keys, fiscal_year + 1, period_type, None)
        .expect("unfiltered history");
    assert!(!histories_unfiltered["total_assets"].is_empty());

    let histories_standalone = state
        .financials()
        .metric_histories(
            &company_id,
            &keys,
            fiscal_year + 1,
            period_type,
            Some("standalone"),
        )
        .expect("standalone-filtered history");
    assert!(
        histories_standalone["total_assets"].is_empty(),
        "a standalone filter must never see the consolidated-only history"
    );
}
