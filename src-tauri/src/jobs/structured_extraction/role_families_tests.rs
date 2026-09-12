//! #511 (ADR 0100 decision 3, amended 2026-09-12): the presentation-role
//! family tests — job-level packages per family, the no-linkbase fallback
//! boundary, and Layer 1 extractor-version freshness. A child of the
//! sibling of the colocated `tests` module (declared externally: the coverage
//! summary forbids a nested `mod x;` inside an inline test module) so the
//! pinned file stays under its size ratchet.

use super::tests::{esef_package_bytes, seed_esef_package};
use super::*;
use crate::storage::MODE_AUTOPILOT;
use crate::test_support::{
    esef_presentation_linkbase_xml as presentation_linkbase_xml, minimal_zip,
    seed_document_with_bytes,
};

/// Same balance-sheet trio as [`esef_package_bytes`], but the
/// presentation linkbase classifies each concept under the CALLER's
/// chosen role names (#511, ADR 0100 dec. 3 amendment) — proves a role
/// family routes to the `esef` tier end to end through
/// `run_structured_extraction`/`compute_layer1_generation`, not only
/// through `classify_role`'s own unit tests.
fn esef_package_bytes_with_roles(roles: &[(&str, &str)]) -> Vec<u8> {
    let instance = r#"<html xmlns:ix="http://www.xbrl.org/2013/inlineXBRL"
  xmlns:ifrs-full="https://xbrl.ifrs.org/taxonomy/2024-03-27/ifrs-full"
  xmlns:xbrli="http://www.xbrl.org/2003/instance"
  xmlns:iso4217="http://www.xbrl.org/2003/iso4217">
  <xbrli:context id="i"><xbrli:period><xbrli:instant>2025-12-31</xbrli:instant></xbrli:period></xbrli:context>
  <xbrli:unit id="pln"><xbrli:measure>iso4217:PLN</xbrli:measure></xbrli:unit>
  <ix:nonFraction name="ifrs-full:Assets" contextRef="i" unitRef="pln" scale="3">45 000</ix:nonFraction>
  <ix:nonFraction name="ifrs-full:Liabilities" contextRef="i" unitRef="pln" scale="3">20 000</ix:nonFraction>
  <ix:nonFraction name="ifrs-full:Equity" contextRef="i" unitRef="pln" scale="3">25 000</ix:nonFraction>
</html>"#;
    let pre_xml = presentation_linkbase_xml(roles);
    minimal_zip(&[
        ("pkg/reports/instance.xhtml", instance.as_bytes()),
        ("pkg/www/instance_pre.xml", pre_xml.as_bytes()),
    ])
}

/// Reads back `(metric_key, value)` for exactly the produced fact ids —
/// Test D's "exact emitted pairs" assertion, joined through
/// `list_financial_facts`'s `metric_key` (one hop from `definition_id`).
/// Compares as `Decimal` (never a formatted string) so the assertion
/// pins the VALUE, not `Decimal::to_string`'s scale/formatting choice.
fn produced_metric_values(
    state: &AppState,
    company_id: &str,
    produced_fact_ids: &[String],
) -> Vec<(String, rust_decimal::Decimal)> {
    let mut got: Vec<(String, rust_decimal::Decimal)> = state
        .list_financial_facts(crate::storage::ListFinancialFactsInput {
            company_id: Some(company_id.to_owned()),
            period_id: None,
            definition_id: None,
        })
        .expect("list facts")
        .into_iter()
        .filter(|f| produced_fact_ids.contains(&f.id))
        .map(|f| {
            (
                f.metric_key.clone(),
                f.value_numeric
                    .parse::<rust_decimal::Decimal>()
                    .expect("a produced financial fact's value_numeric must parse as Decimal"),
            )
        })
        .collect();
    got.sort_by(|a, b| a.0.cmp(&b.0));
    got
}

/// Test D (#511, ADR 0100 dec. 3 amendment): a package whose `_pre.xml`
/// uses ONLY the English vendor role names must reach the `esef` tier —
/// on master these role names classify `other`, every fact fails Layer
/// 2's primary-statement filter, and the run ends `no_deterministic_tier`.
#[test]
fn a_package_using_only_the_english_role_family_reaches_the_esef_tier() {
    let english_roles: &[(&str, &str)] = &[
        ("Assets", "BalanceSheet"),
        ("Liabilities", "BalanceSheet"),
        ("Equity", "BalanceSheet"),
    ];
    let bytes = esef_package_bytes_with_roles(english_roles);
    let (state, company_id, document_id) = seed_document_with_bytes(
        "english-family",
        "ENG",
        "English family annual 2025",
        "report.xbri",
        &bytes,
    );
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
    .expect("structured extraction runs");

    assert_eq!(
        result.tier,
        Some(SourceTier::Esef),
        "red on master: the English role names classify `other`, so this ends \
         no_deterministic_tier"
    );
    assert_eq!(result.acceptance, Acceptance::Accepted);
    assert_eq!(
        produced_metric_values(&state, &company_id, &result.produced_fact_ids),
        vec![
            (
                "total_assets".to_owned(),
                rust_decimal::Decimal::from(45_000_000)
            ),
            (
                "total_equity".to_owned(),
                rust_decimal::Decimal::from(25_000_000)
            ),
            (
                "total_liabilities".to_owned(),
                rust_decimal::Decimal::from(20_000_000)
            ),
        ]
    );
}

/// Test D (#511), the `R0n_` abbreviation family: same package shape,
/// classified under `R03_Bilans` instead — red on master for the same
/// reason (`R0n_` roles classify `other` today).
#[test]
fn a_package_using_only_the_abbreviation_role_family_reaches_the_esef_tier() {
    let abbreviation_roles: &[(&str, &str)] = &[
        ("Assets", "R03_Bilans"),
        ("Liabilities", "R03_Bilans"),
        ("Equity", "R03_Bilans"),
    ];
    let bytes = esef_package_bytes_with_roles(abbreviation_roles);
    let (state, company_id, document_id) = seed_document_with_bytes(
        "abbreviation-family",
        "ABR",
        "Abbreviation family annual 2025",
        "report.xbri",
        &bytes,
    );
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
    .expect("structured extraction runs");

    assert_eq!(
        result.tier,
        Some(SourceTier::Esef),
        "red on master: R0n_ roles classify `other`, so this ends no_deterministic_tier"
    );
    assert_eq!(result.acceptance, Acceptance::Accepted);
    assert_eq!(
        produced_metric_values(&state, &company_id, &result.produced_fact_ids),
        vec![
            (
                "total_assets".to_owned(),
                rust_decimal::Decimal::from(45_000_000)
            ),
            (
                "total_equity".to_owned(),
                rust_decimal::Decimal::from(25_000_000)
            ),
            (
                "total_liabilities".to_owned(),
                rust_decimal::Decimal::from(20_000_000)
            ),
        ]
    );
}

/// Test E (#511, preservation, ADR 0100 decision 4): a package whose
/// linkbase PARSES but classifies every role `other` still HAS linkbase
/// evidence (`has_presentation_linkbase` is `!roles.is_empty()` — any
/// parsed concept->role pair, regardless of kind) — the no-linkbase
/// fallback is reserved for a document with NO linkbase at all, never
/// for one whose roles are merely unrecognised.
#[test]
fn a_linkbase_with_only_other_roles_does_not_take_the_no_linkbase_fallback() {
    let other_roles: &[(&str, &str)] = &[
        ("Assets", "NotesToBalanceSheet"),
        ("Liabilities", "NotesToBalanceSheet"),
        ("Equity", "NotesToBalanceSheet"),
    ];
    let bytes = esef_package_bytes_with_roles(other_roles);

    let generation = compute_layer1_generation(&bytes, DocumentRoute::ZipPackage);
    assert!(
        generation.has_presentation_linkbase,
        "a parsed linkbase counts as evidence even when every role classifies other"
    );

    let projected = crate::fundamentals::extraction::esef::projection::project_period(
        &generation.facts,
        "2025-12-31",
        generation.has_presentation_linkbase,
    );
    assert!(
        projected.facts.is_empty(),
        "no primary-statement role recognised -> nothing projected, \
         the no-linkbase fallback must NOT have run"
    );
    assert_eq!(projected.non_primary_statement_skipped, 3);
}

/// Test C (#511, ADR 0100 dec. 3 amendment): freshness is `(hash,
/// extractor_version)`, never hash alone (decision 8) — a generation
/// stored at the STALE version 2 must be rebuilt at the current version
/// even though the bytes never changed (the role-family fix needs every
/// already-captured document's roles reclassified); only THEN does an
/// unchanged version-3 generation skip.
#[test]
fn a_stale_extractor_version_forces_a_rebuild_then_an_unchanged_current_version_skips() {
    let (state, company_id, document_id) = seed_esef_package();
    let bytes = esef_package_bytes();
    let hash = crate::report_documents_capture::content_hash_hex(&bytes);
    {
        let connection = state.checkout_for_tests().expect("raw");
        connection
            .execute(
                "INSERT INTO report_tagged_fact_extractions
                    (report_document_id, source_content_hash, extractor_version, state,
                     encountered_count, stored_count, dimensional_count)
                 VALUES (?1, ?2, 2, 'extracted', 0, 0, 0)",
                rusqlite::params![document_id, hash],
            )
            .expect("seed a stale version-2 generation");
    }

    capture_layer1_tagged_facts(
        &state,
        &company_id,
        &document_id,
        &bytes,
        DocumentRoute::ZipPackage,
    );
    let rebuilt = state
        .report_tagged_facts()
        .extraction(&document_id)
        .expect("read extraction")
        .expect("extraction row exists");
    assert_eq!(
        rebuilt.extractor_version, TAGGED_FACT_EXTRACTOR_VERSION,
        "a stale version-2 generation must be rebuilt at the current version, \
         even though the bytes never changed"
    );
    let rebuilt_ids: Vec<String> = state
        .report_tagged_facts()
        .facts(&document_id)
        .expect("facts")
        .into_iter()
        .map(|f| f.id)
        .collect();
    assert_eq!(rebuilt_ids.len(), 4, "the package's 4 real occurrences");

    // Unchanged bytes, now already at the current version: must skip.
    capture_layer1_tagged_facts(
        &state,
        &company_id,
        &document_id,
        &bytes,
        DocumentRoute::ZipPackage,
    );
    let unchanged = state
        .report_tagged_facts()
        .extraction(&document_id)
        .expect("read extraction")
        .expect("extraction row exists");
    assert_eq!(unchanged.extractor_version, TAGGED_FACT_EXTRACTOR_VERSION);
    let unchanged_ids: Vec<String> = state
        .report_tagged_facts()
        .facts(&document_id)
        .expect("facts")
        .into_iter()
        .map(|f| f.id)
        .collect();
    assert_eq!(
        unchanged_ids, rebuilt_ids,
        "unchanged bytes at the current version must skip, not mint a new generation"
    );
}
