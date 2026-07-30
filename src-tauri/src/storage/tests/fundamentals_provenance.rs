//! Fundamentals provenance reads (epic #229 T5): the flagged-FACTS surface and
//! the witness-corroboration stamp.

use super::*;

fn state_with_company(ticker: &str) -> (AppState, String) {
    let state = AppState::new(open_in_memory_database().expect("db"));
    let company = state
        .create_company(NewCompany {
            exchange: "GPW".to_owned(),
            ticker: ticker.to_owned(),
            display_name: format!("{ticker} S.A."),
            isin: None,
            cik: None,
            lei: None,
        })
        .expect("company");
    (state, company.id)
}

fn record(
    state: &AppState,
    company_id: &str,
    metric_key: &str,
    value: &str,
    validation_status: &str,
) {
    record_as(
        state,
        company_id,
        metric_key,
        value,
        validation_status,
        "esef",
    );
}

fn record_as(
    state: &AppState,
    company_id: &str,
    metric_key: &str,
    value: &str,
    validation_status: &str,
    source_tier: &str,
) {
    state
        .kpi_extraction()
        .record_structured_fact(StructuredFactInput {
            company_id,
            fiscal_year: 2025,
            period_type: "FY",
            period_end: Some("2025-12-31"),
            report_document_id: "doc1",
            metric_key,
            value_numeric: value,
            currency: Some("PLN"),
            confirmation_state: "confirmed",
            source_tier,
            extraction_method: "api",
            validation_status,
            drift_json: None,
            citation: Some("ifrs-full:Assets"),
        })
        .expect("record fact");
}

/// The flagged-facts review surface must be answerable PER COMPANY and must
/// carry what a human needs to judge the flag — the metric, the value, the tier
/// and the citation. Returning bare fact ids (the pre-T5 shape) made the read
/// unusable in a company-scoped surface, which is why it had no UI consumer.
#[test]
fn flagged_facts_read_is_company_scoped_and_carries_the_value() {
    let (state, company_id) = state_with_company("CDR");
    record(&state, &company_id, "total_assets", "45000000", "flagged");
    record(&state, &company_id, "revenue", "12000000", "passed");

    let flagged = state
        .fundamentals_provenance()
        .list_flagged_facts(Some(&company_id))
        .expect("flagged facts");

    assert_eq!(
        flagged.len(),
        1,
        "only the flagged fact is listed: {flagged:?}"
    );
    let row = &flagged[0];
    assert_eq!(row.company_id, company_id);
    assert_eq!(row.metric_key, "total_assets");
    assert_eq!(row.value_numeric.trim(), "45000000");
    assert_eq!(row.currency.as_deref(), Some("PLN"));
    assert_eq!(row.fiscal_year, 2025);
    assert_eq!(row.period_type, "FY");
    assert_eq!(row.source_tier, "esef");
    assert_eq!(row.validation_status, "flagged");
    assert_eq!(row.citation.as_deref(), Some("ifrs-full:Assets"));
    assert!(
        !row.label.trim().is_empty(),
        "the catalog label travels with the row so the UI never shows a raw key"
    );
}

#[test]
fn flagged_facts_of_another_company_never_leak_into_a_company_scoped_read() {
    let (state, cdr) = state_with_company("CDR");
    let other = state
        .create_company(NewCompany {
            exchange: "GPW".to_owned(),
            ticker: "PKN".to_owned(),
            display_name: "PKN S.A.".to_owned(),
            isin: None,
            cik: None,
            lei: None,
        })
        .expect("company")
        .id;
    record(&state, &cdr, "total_assets", "45000000", "flagged");
    record(&state, &other, "total_assets", "91000000", "flagged");

    let scoped = state
        .fundamentals_provenance()
        .list_flagged_facts(Some(&cdr))
        .expect("scoped read");
    assert_eq!(scoped.len(), 1);
    assert_eq!(scoped[0].company_id, cdr);

    // The unscoped read (the MCP data-quality surface) still spans every company.
    let all = state
        .fundamentals_provenance()
        .list_flagged_facts(None)
        .expect("unscoped read");
    assert_eq!(
        all.len(),
        2,
        "the global review surface keeps both: {all:?}"
    );
}

/// A re-write of the fact's value clears the corroboration stamp: a witness
/// figure recorded against the OLD value must never read as agreement with the
/// new one. Exercised through the real rewrite path — a higher tier taking over
/// the slot (ADR 0086 dec. 3), the only way a stored value legitimately changes
/// under an automatic writer.
#[test]
fn rewriting_a_fact_clears_its_witness_corroboration() {
    let (state, company_id) = state_with_company("CDR");
    record_as(
        &state,
        &company_id,
        "total_assets",
        "45000000",
        "unreviewed",
        "html_aggregator",
    );
    let fact_id = state
        .list_financial_facts(ListFinancialFactsInput {
            company_id: Some(company_id.clone()),
            period_id: None,
            definition_id: None,
        })
        .expect("facts")
        .remove(0)
        .id;

    state
        .fundamentals_provenance()
        .corroborate_fact(WitnessCorroboration {
            fact_id: &fact_id,
            witness_value: "45000000",
            witness_page_url: "https://www.biznesradar.pl/raporty-finansowe-bilans/CDR",
            upgrade_validation_status: true,
        })
        .expect("corroborate");
    let stamped = state
        .fundamentals_provenance()
        .get_fact_provenance(&fact_id)
        .expect("read")
        .expect("provenance");
    assert_eq!(stamped.validation_status, "witness_confirmed");
    assert!(stamped.corroborated_at.is_some());

    // The issuer's own filing takes the slot over with a different value.
    record(&state, &company_id, "total_assets", "46000000", "passed");
    assert_eq!(
        state
            .list_financial_facts(ListFinancialFactsInput {
                company_id: Some(company_id.clone()),
                period_id: None,
                definition_id: None,
            })
            .expect("facts")
            .remove(0)
            .value_numeric
            .trim(),
        "46000000",
        "the higher tier must actually have rewritten the value"
    );
    let after = state
        .fundamentals_provenance()
        .get_fact_provenance(&fact_id)
        .expect("read")
        .expect("provenance");
    assert_eq!(
        after.witness_value, None,
        "a stale stamp must never survive the value it corroborated: {after:?}"
    );
    assert_eq!(after.corroborated_at, None);
}
