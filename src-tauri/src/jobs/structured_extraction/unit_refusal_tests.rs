//! #509 (ADR 0100 decision 4 amendment): a non-currency unit never reaches
//! `currency`, and a fact-local store refusal never aborts the run. A child
//! of the sibling of the colocated `tests` module (declared externally —
//! `role_families_tests.rs`'s precedent: the coverage summary forbids a
//! nested `mod x;` inside an inline test module), so the pinned file stays
//! under its size ratchet.

use super::*;
use crate::storage::MODE_AUTOPILOT;
use crate::test_support::{
    esef_presentation_linkbase_xml as presentation_linkbase_xml, minimal_zip,
    seed_document_with_bytes,
};

/// One primary-role package tagging `Revenue` (`PLN`, monetary) and
/// `WeightedAverageShares` (`shares`, count) — the shape #509's acceptance-
/// corpus package carried. Both concepts share the income role (the real
/// corpus shape: a share count sits in the income statement's per-share
/// section).
fn esef_package_revenue_and_share_count(shares_value: &str) -> Vec<u8> {
    let instance = format!(
        r#"<html xmlns:ix="http://www.xbrl.org/2013/inlineXBRL"
  xmlns:ifrs-full="https://xbrl.ifrs.org/taxonomy/2024-03-27/ifrs-full"
  xmlns:xbrli="http://www.xbrl.org/2003/instance"
  xmlns:iso4217="http://www.xbrl.org/2003/iso4217">
  <xbrli:context id="d">
    <xbrli:period><xbrli:startDate>2025-01-01</xbrli:startDate><xbrli:endDate>2025-12-31</xbrli:endDate></xbrli:period>
  </xbrli:context>
  <xbrli:unit id="pln"><xbrli:measure>iso4217:PLN</xbrli:measure></xbrli:unit>
  <xbrli:unit id="shares"><xbrli:measure>xbrli:shares</xbrli:measure></xbrli:unit>
  <ix:nonFraction name="ifrs-full:Revenue" contextRef="d" unitRef="pln">1000000</ix:nonFraction>
  <ix:nonFraction name="ifrs-full:WeightedAverageShares" contextRef="d" unitRef="shares">{shares_value}</ix:nonFraction>
</html>"#
    );
    let roles: &[(&str, &str)] = &[
        ("Revenue", "ias_1_role-310000"),
        ("WeightedAverageShares", "ias_1_role-310000"),
    ];
    let pre_xml = presentation_linkbase_xml(roles);
    minimal_zip(&[
        ("pkg/reports/instance.xhtml", instance.as_bytes()),
        ("pkg/www/instance_pre.xml", pre_xml.as_bytes()),
    ])
}

/// Test C (#509): `Revenue` (PLN) + `WeightedAverageShares` (shares) both
/// persist — the share count with `currency` NULL — and the run succeeds
/// (red on master: `Err("invalid financials value for currency: shares")"`,
/// since the projection copied the raw unit into `currency` and the store's
/// #93 guard refused the whole write, aborting before any fact committed).
#[test]
fn a_share_count_alongside_a_monetary_fact_persists_with_currency_null() {
    let bytes = esef_package_revenue_and_share_count("500000");
    let (state, company_id, document_id) = seed_document_with_bytes(
        "unit-refusal-c",
        "URC",
        "Unit refusal C",
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
    .expect("run_structured_extraction must not abort on a non-monetary unit");

    assert_eq!(result.tier, Some(SourceTier::Esef));
    // Acceptance per the normal gate (never hardcoded here — this test proves
    // the run completes and persists correctly, not a specific gate grade):
    // both facts committed, so the run is at least an emit, never
    // Flagged/Empty.
    assert!(
        matches!(
            result.acceptance,
            Acceptance::Accepted | Acceptance::AcceptedUnreviewed
        ),
        "unexpected acceptance grade: {:?}",
        result.acceptance
    );

    let facts = state
        .list_financial_facts(crate::storage::ListFinancialFactsInput {
            company_id: Some(company_id.clone()),
            period_id: None,
            definition_id: None,
        })
        .expect("list facts");
    let revenue = facts
        .iter()
        .find(|f| f.metric_key == "revenue")
        .expect("revenue persisted");
    assert_eq!(revenue.currency.as_deref(), Some("PLN"));
    let shares = facts
        .iter()
        .find(|f| f.metric_key == "weighted_average_shares")
        .expect("share count persisted");
    assert_eq!(
        shares.currency, None,
        "a count-kind fact must persist with currency NULL, never the raw unit"
    );
}

/// A package tagging a MONETARY concept (`AdministrativeExpense`) whose
/// Layer 1 unit is `xbrli:pure` — the projection (decision 1) still yields
/// `currency: Some("pure")` since the concept genuinely IS monetary; only the
/// unit itself is wrong. `administrative_expense` < `revenue` lexicographically,
/// so the projection's `(basis, metric_key)` `BTreeMap` (`by_slot`,
/// `esef/projection.rs`) — and therefore `outcome.facts`' order — places it
/// BEFORE the valid `Revenue`/PLN sibling; asserted directly below rather
/// than assumed.
fn esef_package_bad_unit_before_valid_sibling() -> Vec<u8> {
    let instance = r#"<html xmlns:ix="http://www.xbrl.org/2013/inlineXBRL"
  xmlns:ifrs-full="https://xbrl.ifrs.org/taxonomy/2024-03-27/ifrs-full"
  xmlns:xbrli="http://www.xbrl.org/2003/instance"
  xmlns:iso4217="http://www.xbrl.org/2003/iso4217">
  <xbrli:context id="d">
    <xbrli:period><xbrli:startDate>2025-01-01</xbrli:startDate><xbrli:endDate>2025-12-31</xbrli:endDate></xbrli:period>
  </xbrli:context>
  <xbrli:unit id="pln"><xbrli:measure>iso4217:PLN</xbrli:measure></xbrli:unit>
  <xbrli:unit id="pure"><xbrli:measure>xbrli:pure</xbrli:measure></xbrli:unit>
  <ix:nonFraction name="ifrs-full:AdministrativeExpense" contextRef="d" unitRef="pure">42</ix:nonFraction>
  <ix:nonFraction name="ifrs-full:Revenue" contextRef="d" unitRef="pln">1000000</ix:nonFraction>
</html>"#;
    let roles: &[(&str, &str)] = &[
        ("AdministrativeExpense", "ias_1_role-310000"),
        ("Revenue", "ias_1_role-310000"),
    ];
    let pre_xml = presentation_linkbase_xml(roles);
    minimal_zip(&[
        ("pkg/reports/instance.xhtml", instance.as_bytes()),
        ("pkg/www/instance_pre.xml", pre_xml.as_bytes()),
    ])
}

/// Test D (#509 decision 2): still red after the projection fix ALONE — a
/// monetary concept with a bad (non-currency-shaped) unit is a fact-local
/// store refusal, never a run abort. The sibling continues to persist, and
/// the outcome is `flagged`/`validation_failed` naming the rejection.
#[test]
fn a_monetary_fact_with_a_bad_unit_is_rejected_not_aborting_the_run() {
    assert!(
        "administrative_expense" < "revenue",
        "the ordering claim this test rests on: the (basis, metric_key) \
         BTreeMap slot key sorts admin_expense before revenue"
    );
    let bytes = esef_package_bad_unit_before_valid_sibling();
    let (state, company_id, document_id) = seed_document_with_bytes(
        "unit-refusal-d",
        "URD",
        "Unit refusal D",
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
    .expect("a fact-local store refusal must never abort the run");

    assert_eq!(result.tier, Some(SourceTier::Esef));
    assert_eq!(result.acceptance, Acceptance::Flagged);

    let facts = state
        .list_financial_facts(crate::storage::ListFinancialFactsInput {
            company_id: Some(company_id.clone()),
            period_id: None,
            definition_id: None,
        })
        .expect("list facts");
    assert!(
        facts.iter().any(|f| f.metric_key == "revenue"),
        "the valid sibling must persist (continuation, not abort): {facts:?}"
    );
    assert!(
        !facts
            .iter()
            .any(|f| f.metric_key == "administrative_expense"),
        "the rejected fact must never persist: {facts:?}"
    );

    let flagged = state
        .fundamentals_provenance()
        .list_flagged_extraction_outcomes(&company_id)
        .expect("flagged outcomes");
    let outcome = flagged
        .iter()
        .find(|o| o.reason_code == "validation_failed")
        .expect("a validation_failed outcome row exists");
    let detail: serde_json::Value =
        serde_json::from_str(outcome.detail_json.as_deref().expect("detail_json"))
            .expect("detail parses");
    assert_eq!(
        detail["rejectedFacts"],
        serde_json::json!([{
            "metricKey": "administrative_expense",
            "field": "currency",
            "value": "pure",
        }])
    );
}

/// A package tagging ONLY a bad-unit concept — no valid sibling at all.
/// `BasicEarningsLossPerShare` (never `AdministrativeExpense`): the legacy
/// `esef::concept_to_metric_key` 22-concept map `derive_report_period` still
/// consults for its OWN period self-derivation only recognises a fixed
/// concept set — an unrecognised-alone concept would leave that map's
/// `facts` empty and `derive_report_period` would return `None` before
/// `run_structured_extraction` is even reached, which is not what this test
/// means to exercise.
fn esef_package_only_bad_unit() -> Vec<u8> {
    let instance = r#"<html xmlns:ix="http://www.xbrl.org/2013/inlineXBRL"
  xmlns:ifrs-full="https://xbrl.ifrs.org/taxonomy/2024-03-27/ifrs-full"
  xmlns:xbrli="http://www.xbrl.org/2003/instance"
  xmlns:iso4217="http://www.xbrl.org/2003/iso4217">
  <xbrli:context id="d">
    <xbrli:period><xbrli:startDate>2025-01-01</xbrli:startDate><xbrli:endDate>2025-12-31</xbrli:endDate></xbrli:period>
  </xbrli:context>
  <xbrli:unit id="pure"><xbrli:measure>xbrli:pure</xbrli:measure></xbrli:unit>
  <ix:nonFraction name="ifrs-full:BasicEarningsLossPerShare" contextRef="d" unitRef="pure">1.23</ix:nonFraction>
</html>"#;
    let roles: &[(&str, &str)] = &[("BasicEarningsLossPerShare", "ias_1_role-310000")];
    let pre_xml = presentation_linkbase_xml(roles);
    minimal_zip(&[
        ("pkg/reports/instance.xhtml", instance.as_bytes()),
        ("pkg/www/instance_pre.xml", pre_xml.as_bytes()),
    ])
}

/// Test D2 (#509): every fact in the set rejected → `emitted == false`,
/// `fact_count == 0`, and the flagged outcome row still exists — a durable,
/// reviewable trace, never a silent no-op (the `:1398`/`issuer_emitted` rule
/// counts created OR reobserved facts only; an all-rejected set has neither).
#[test]
fn all_facts_rejected_still_records_a_durable_flagged_outcome_with_zero_emitted() {
    let bytes = esef_package_only_bad_unit();
    let (state, company_id, document_id) = seed_document_with_bytes(
        "unit-refusal-d2",
        "URD2",
        "Unit refusal D2",
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
    .expect("an all-rejected set must still return Ok, never abort");

    assert!(
        !result.emitted,
        "nothing committed anywhere — a fully rejected set is a gap, never an emit"
    );
    assert!(result.produced_fact_ids.is_empty());
    assert!(result.skipped_fact_ids.is_empty());
    assert_eq!(result.acceptance, Acceptance::Flagged);

    let flagged = state
        .fundamentals_provenance()
        .list_flagged_extraction_outcomes(&company_id)
        .expect("flagged outcomes");
    let outcome = flagged
        .iter()
        .find(|o| o.reason_code == "validation_failed")
        .expect("a durable flagged outcome row exists");
    assert_eq!(outcome.fact_count, 0);
}

/// Test D3 (#509): a rejection (currency guard) AND a history-plausibility
/// quarantine in the SAME set both surface — `rejected_detail`/
/// `quarantine_detail` chain onto one payload rather than one clobbering the
/// other.
#[test]
fn a_rejection_and_a_quarantine_in_one_set_both_appear_in_the_outcome_detail() {
    let instance = r#"<html xmlns:ix="http://www.xbrl.org/2013/inlineXBRL"
  xmlns:ifrs-full="https://xbrl.ifrs.org/taxonomy/2024-03-27/ifrs-full"
  xmlns:xbrli="http://www.xbrl.org/2003/instance"
  xmlns:iso4217="http://www.xbrl.org/2003/iso4217">
  <xbrli:context id="d">
    <xbrli:period><xbrli:startDate>2025-01-01</xbrli:startDate><xbrli:endDate>2025-12-31</xbrli:endDate></xbrli:period>
  </xbrli:context>
  <xbrli:unit id="pln"><xbrli:measure>iso4217:PLN</xbrli:measure></xbrli:unit>
  <xbrli:unit id="pure"><xbrli:measure>xbrli:pure</xbrli:measure></xbrli:unit>
  <ix:nonFraction name="ifrs-full:AdministrativeExpense" contextRef="d" unitRef="pure">42</ix:nonFraction>
  <ix:nonFraction name="ifrs-full:Revenue" contextRef="d" unitRef="pln">200000000</ix:nonFraction>
</html>"#;
    let roles: &[(&str, &str)] = &[
        ("AdministrativeExpense", "ias_1_role-310000"),
        ("Revenue", "ias_1_role-310000"),
    ];
    let pre_xml = presentation_linkbase_xml(roles);
    let bytes = minimal_zip(&[
        ("pkg/reports/instance.xhtml", instance.as_bytes()),
        ("pkg/www/instance_pre.xml", pre_xml.as_bytes()),
    ]);
    let (state, company_id, document_id) = seed_document_with_bytes(
        "unit-refusal-d3",
        "URD3",
        "Unit refusal D3",
        "report.xbri",
        &bytes,
    );
    // Two prior FY periods at revenue=1_000_000 (median 1_000_000) — this
    // run's 200_000_000 is 200x off, well past the 100x plausibility ratio.
    super::tests::seed_prior_period(&state, &company_id, 2023, "FY", &[("revenue", "1000000")]);
    super::tests::seed_prior_period(&state, &company_id, 2024, "FY", &[("revenue", "1000000")]);
    let document = state.get_report_document(&document_id).expect("document");
    let (fiscal_year, period_type, period_end) =
        derive_report_period(&state, &document).expect("period derives");
    assert_eq!(
        fiscal_year, 2025,
        "the target period must not collide with the seeded priors"
    );

    let result = run_structured_extraction(
        &state,
        &company_id,
        &document_id,
        fiscal_year,
        period_type,
        &period_end,
        MODE_AUTOPILOT,
    )
    .expect("a rejection plus a quarantine must still return Ok");
    assert_eq!(result.acceptance, Acceptance::Flagged);

    let flagged = state
        .fundamentals_provenance()
        .list_flagged_extraction_outcomes(&company_id)
        .expect("flagged outcomes");
    let outcome = flagged
        .iter()
        .find(|o| o.reason_code == "validation_failed" && o.report_document_id == document_id)
        .expect("a validation_failed outcome row exists");
    let detail: serde_json::Value =
        serde_json::from_str(outcome.detail_json.as_deref().expect("detail_json"))
            .expect("detail parses");
    assert_eq!(
        detail["rejectedFacts"].as_array().map(Vec::len),
        Some(1),
        "the rejection must still appear: {detail}"
    );
    assert_eq!(
        detail["quarantinedFacts"].as_array().map(Vec::len),
        Some(1),
        "the quarantine must still appear: {detail}"
    );
}

/// Test D4 (#509, repeat-run idempotence): the outcome slot is a deterministic
/// upsert (`extraction_outcome_id` hashes company+document+fiscal_year+
/// period_type+period_end — `record_outcome`/`fundamentals_provenance.rs`), so
/// a second identical run UPDATES the same row (never appends a second) —
/// `attempt_count` increments, the recorded counts stay the same.
#[test]
fn a_repeat_run_upserts_the_same_outcome_row_with_the_same_counts() {
    let bytes = esef_package_bad_unit_before_valid_sibling();
    let (state, company_id, document_id) = seed_document_with_bytes(
        "unit-refusal-d4",
        "URD4",
        "Unit refusal D4",
        "report.xbri",
        &bytes,
    );
    let document = state.get_report_document(&document_id).expect("document");
    let (fiscal_year, period_type, period_end) =
        derive_report_period(&state, &document).expect("period derives");

    for _ in 0..2 {
        run_structured_extraction(
            &state,
            &company_id,
            &document_id,
            fiscal_year,
            period_type,
            &period_end,
            MODE_AUTOPILOT,
        )
        .expect("each run must return Ok");
    }

    let flagged = state
        .fundamentals_provenance()
        .list_flagged_extraction_outcomes(&company_id)
        .expect("flagged outcomes");
    let matches: Vec<_> = flagged
        .iter()
        .filter(|o| o.reason_code == "validation_failed")
        .collect();
    assert_eq!(
        matches.len(),
        1,
        "the deterministic slot id upserts one row, never a second: {flagged:?}"
    );
    assert!(
        matches[0].attempt_count >= 2,
        "a repeated attempt is visibly repeated: {:?}",
        matches[0]
    );
    assert_eq!(
        matches[0].fact_count, 1,
        "the same sibling re-observes each run"
    );
}

// Test E (#509, preservation) is NOT added: verified NOT reachable through
// `run_structured_extraction`. `record_structured_fact`'s write path resolves
// the period through `kpi_extraction::ensure_period` (`INSERT OR IGNORE`, no
// validation) — never `financials::create_financial_period`, the ONLY
// function that raises `InvalidFinancialsValue{key: "period_type"}` (the
// manual-entry/MCP `ensure_financial_period` path). An empty `period_type`
// passed to `run_structured_extraction` was measured to return `Ok`, not
// `Err` — reported in the contract handover rather than asserted here as a
// misleading red/green pair. The commit loop's "any other key/variant is
// still fatal" branch stays as written (the honest, defensive default for a
// StorageError this call site cannot currently produce).
