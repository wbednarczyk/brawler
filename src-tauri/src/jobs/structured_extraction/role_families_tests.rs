//! #511 (ADR 0100 decision 3, amended 2026-09-12): the presentation-role
//! family tests — job-level packages per family, the no-linkbase fallback
//! boundary, and Layer 1 extractor-version freshness. A child of the
//! sibling of the colocated `tests` module (declared externally: the coverage
//! summary forbids a nested `mod x;` inside an inline test module) so the
//! pinned file stays under its size ratchet.

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

/// Test C (#511, ADR 0100 dec. 3 amendment; astra r1 fix 2): freshness is
/// `(hash, extractor_version)` (decision 8) — a version bump must not just
/// re-timestamp a stored generation, it must REPLACE the stored role
/// classification. Seeds a REAL fact + role row through the store API at
/// the stale version 2, whose role reads `other` for an English-family role
/// URI (`BalanceSheet`) — exactly what a pre-#511 extractor actually wrote
/// for this filing, since the amendment is what taught `classify_role` this
/// name. A rebuild over the SAME (unchanged) bytes must both bump the
/// version AND reclassify that role to `balance` through the CURRENT
/// classifier, minting a fresh fact id for it; a second capture at the
/// now-current version must then skip, keeping those same ids.
#[test]
fn a_stale_extractor_version_replaces_the_stored_role_classification_then_skips_when_current() {
    let english_roles: &[(&str, &str)] = &[
        ("Assets", "BalanceSheet"),
        ("Liabilities", "BalanceSheet"),
        ("Equity", "BalanceSheet"),
    ];
    let bytes = esef_package_bytes_with_roles(english_roles);
    let (state, company_id, document_id) = seed_document_with_bytes(
        "stale-role-replacement",
        "SRR",
        "Stale role replacement annual 2025",
        "report.xbri",
        &bytes,
    );
    let hash = crate::report_documents_capture::content_hash_hex(&bytes);

    // The version-2 generation a pre-#511 extractor would have written for
    // this exact package: a real fact row whose ONLY role (`BalanceSheet`)
    // reads `other`, since the amendment is what taught the classifier this
    // English name.
    let stale_fact = crate::storage::NewTaggedFact {
        package_entry_path: "pkg/reports/instance.xhtml".to_owned(),
        fact_identity: "stale_assets".to_owned(),
        identity_kind: "occurrence".to_owned(),
        concept_namespace_uri: "https://xbrl.ifrs.org/taxonomy/2024-03-27/ifrs-full".to_owned(),
        concept_local_name: "Assets".to_owned(),
        context_ref: "i".to_owned(),
        period_type: "instant".to_owned(),
        period_end: "2025-12-31".to_owned(),
        unit_measure: Some("PLN".to_owned()),
        value_raw: "45 000".to_owned(),
        value_numeric: Some("45000000".to_owned()),
        parse_status: "ok".to_owned(),
        roles: vec![crate::storage::NewTaggedFactRole {
            role_uri: "http://x/role/BalanceSheet".to_owned(),
            role_kind: "other".to_owned(),
        }],
        ..Default::default()
    };
    state
        .report_tagged_facts()
        .replace_tagged_facts(
            &document_id,
            &company_id,
            &crate::storage::TaggedFactExtraction {
                source_content_hash: Some(hash),
                extractor_version: 2,
                state: "extracted".to_owned(),
                encountered_count: 1,
                stored_count: 1,
                dimensional_count: 0,
                no_linkbase_fallback_count: 0,
                facts: vec![stale_fact],
            },
        )
        .expect("seed a stale version-2 generation with a real fact + role row");
    let stale_ids: Vec<String> = state
        .report_tagged_facts()
        .facts(&document_id)
        .expect("facts")
        .into_iter()
        .map(|f| f.id)
        .collect();
    assert_eq!(stale_ids.len(), 1);

    capture_layer1_tagged_facts(
        &state,
        &company_id,
        &document_id,
        &bytes,
        DocumentRoute::ZipPackage,
    );

    let extraction = state
        .report_tagged_facts()
        .extraction(&document_id)
        .expect("read extraction")
        .expect("extraction row exists");
    assert_eq!(
        extraction.extractor_version, TAGGED_FACT_EXTRACTOR_VERSION,
        "a stale version-2 generation must be rebuilt at the current version, \
         even though the bytes never changed"
    );

    let rebuilt_facts = state
        .report_tagged_facts()
        .facts(&document_id)
        .expect("facts");
    assert_eq!(rebuilt_facts.len(), 3, "the package's 3 real occurrences");
    let assets = rebuilt_facts
        .iter()
        .find(|f| f.concept_local_name == "Assets")
        .expect("Assets fact exists after rebuild");
    let role_kinds: Vec<&str> = assets.roles.iter().map(|r| r.role_kind.as_str()).collect();
    assert_eq!(
        role_kinds,
        vec!["balance"],
        "the version bump must reclassify the stored role through the CURRENT \
         classifier, not keep the stale `other`"
    );
    assert!(
        !rebuilt_facts.iter().any(|f| f.id == stale_ids[0]),
        "the stale fact id must be gone, not reused"
    );

    // Unchanged bytes, now already at the current version: must skip.
    let rebuilt_ids: std::collections::BTreeSet<String> =
        rebuilt_facts.iter().map(|f| f.id.clone()).collect();
    capture_layer1_tagged_facts(
        &state,
        &company_id,
        &document_id,
        &bytes,
        DocumentRoute::ZipPackage,
    );
    let unchanged_ids: std::collections::BTreeSet<String> = state
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

/// #511 astra r1 fix 1: `_pre.xml` lists `ProfitLoss` under TWO newly
/// supported role URIs — the `R0n_` abbreviation and the English vendor
/// name — and the instance tags TWO real `ProfitLoss` occurrences on
/// distinct contexts (the real XTB shape: the income-statement line and the
/// cash-flow reconciliation's opening line tag the same concept). Built and
/// read through the REAL Layer 1 path, never a fabricated role vector.
fn two_role_profit_loss_package(value_ctx1: &str, value_ctx2: &str) -> Vec<u8> {
    let instance = format!(
        r#"<html xmlns:ix="http://www.xbrl.org/2013/inlineXBRL"
  xmlns:ifrs-full="https://xbrl.ifrs.org/taxonomy/2024-03-27/ifrs-full"
  xmlns:xbrli="http://www.xbrl.org/2003/instance"
  xmlns:iso4217="http://www.xbrl.org/2003/iso4217">
  <xbrli:context id="ctx1">
    <xbrli:period><xbrli:startDate>2025-01-01</xbrli:startDate><xbrli:endDate>2025-12-31</xbrli:endDate></xbrli:period>
  </xbrli:context>
  <xbrli:context id="ctx2">
    <xbrli:period><xbrli:startDate>2025-01-01</xbrli:startDate><xbrli:endDate>2025-12-31</xbrli:endDate></xbrli:period>
  </xbrli:context>
  <xbrli:unit id="pln"><xbrli:measure>iso4217:PLN</xbrli:measure></xbrli:unit>
  <ix:nonFraction name="ifrs-full:ProfitLoss" contextRef="ctx1" unitRef="pln">{value_ctx1}</ix:nonFraction>
  <ix:nonFraction name="ifrs-full:ProfitLoss" contextRef="ctx2" unitRef="pln">{value_ctx2}</ix:nonFraction>
</html>"#
    );
    let roles: &[(&str, &str)] = &[
        ("ProfitLoss", "R01_RZiS"),
        ("ProfitLoss", "ComprehensiveIncome"),
    ];
    let pre_xml = presentation_linkbase_xml(roles);
    minimal_zip(&[
        ("pkg/reports/instance.xhtml", instance.as_bytes()),
        ("pkg/www/instance_pre.xml", pre_xml.as_bytes()),
    ])
}

/// Parser→projection coverage (replaces the plan's fabricated-role-vector
/// B1): equal values across the two REAL, newly-recognised roles resolve as
/// one re-observed fact — same doctrine as `esef/projection.rs`'s own B1,
/// but the role membership comes from the real classifier/parser this time.
#[test]
fn two_newly_recognised_roles_from_real_parsing_project_as_one_fact_when_values_agree() {
    let bytes = two_role_profit_loss_package("100", "100");
    let generation = compute_layer1_generation(&bytes, DocumentRoute::ZipPackage);

    let profit_loss: Vec<_> = generation
        .facts
        .iter()
        .filter(|f| f.concept_local_name == "ProfitLoss")
        .collect();
    assert_eq!(
        profit_loss.len(),
        2,
        "both real ProfitLoss occurrences must be captured"
    );
    for fact in &profit_loss {
        let mut kinds: Vec<&str> = fact.roles.iter().map(|r| r.role_kind.as_str()).collect();
        kinds.sort_unstable();
        assert_eq!(
            kinds,
            vec!["comprehensive_income", "income"],
            "both occurrences of one concept must carry the identical role \
             membership the REAL classifier assigned to R01_RZiS + ComprehensiveIncome"
        );
    }

    let projected = crate::fundamentals::extraction::esef::projection::project_period(
        &generation.facts,
        "2025-12-31",
        generation.has_presentation_linkbase,
    );
    assert_eq!(projected.facts.len(), 1);
    assert_eq!(projected.facts[0].fact.metric_key, "net_profit");
    assert_eq!(
        projected.facts[0].fact.value,
        rust_decimal::Decimal::from(100)
    );
    assert!(projected.conflicts.is_empty());
}

/// Parser→projection coverage (replaces the plan's fabricated-role-vector
/// B2): divergent values across the two REAL, newly-recognised roles are a
/// typed conflict, never a pick — same real-parser evidence as the sibling
/// test above.
#[test]
fn two_newly_recognised_roles_from_real_parsing_yield_a_conflict_when_values_diverge() {
    let bytes = two_role_profit_loss_package("100", "-100");
    let generation = compute_layer1_generation(&bytes, DocumentRoute::ZipPackage);

    let projected = crate::fundamentals::extraction::esef::projection::project_period(
        &generation.facts,
        "2025-12-31",
        generation.has_presentation_linkbase,
    );
    assert!(projected.facts.is_empty());
    assert_eq!(projected.conflicts.len(), 1);
    assert_eq!(projected.conflicts[0].metric_key, "net_profit");
}
