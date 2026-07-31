//! Tests for the `record_financial_facts` batch orchestration (ADR 0093 dec.
//! 6, epic #285 T7). The MCP wire boundary (schemars input, provenance gate,
//! registry wiring) is covered separately in `mcp::acts`/`mcp::registry`;
//! this module exercises the pure `AppState` orchestration seam directly.

use std::collections::HashMap;

use super::*;
use crate::commands::error::CommandErrorCode;
use crate::storage::{
    open_in_memory_database, AppState, CaptureReportDocumentInput, FinancialFact,
    ListFinancialFactsInput, NewCompany,
};

fn state_with_company_and_document() -> (AppState, String, String) {
    let state = AppState::new(open_in_memory_database().expect("db"));
    let company = state
        .create_company(NewCompany {
            exchange: "GPW".to_owned(),
            ticker: "XTB".to_owned(),
            display_name: "XTB S.A.".to_owned(),
            isin: None,
            cik: None,
            lei: None,
        })
        .expect("company");
    let document = state
        .create_or_find_pending_report_document(CaptureReportDocumentInput {
            company_id: company.id.clone(),
            source_type: "espi_attachment".to_owned(),
            url: "https://xtb.example/rb-18-2026.pdf".to_owned(),
            period_id: None,
            origin_ref: None,
            title: None,
            attribution: None,
        })
        .expect("document");
    (state, company.id, document.id)
}

fn fact<'a>(metric_key: &'a str, value: &'a str, citation: &'a str) -> BatchFactInput<'a> {
    BatchFactInput {
        metric_key,
        value_numeric: value,
        currency: Some("PLN"),
        attribution: None,
        measure_window: None,
        citation,
    }
}

fn company_facts(state: &AppState, company_id: &str) -> Vec<FinancialFact> {
    state
        .list_financial_facts(ListFinancialFactsInput {
            company_id: Some(company_id.to_owned()),
            period_id: None,
            definition_id: None,
        })
        .expect("list_financial_facts")
}

/// Seeds two prior annual `revenue` facts so the plausibility gate has
/// ≥2 history points to work with (otherwise every fresh-company write
/// abstains as thin-history — the correct behavior, but not what a
/// `created`/`reobserved`/`upgraded`/`divergent` outcome test wants to
/// exercise).
fn seed_revenue_history(state: &AppState, company_id: &str, document_id: &str) {
    for (year, value) in [(2024, "1000000"), (2025, "1100000")] {
        state
            .kpi_extraction()
            .record_structured_fact(StructuredFactInput {
                company_id,
                fiscal_year: year,
                period_type: "FY",
                period_end: Some(&format!("{year}-12-31")),
                report_document_id: document_id,
                metric_key: "revenue",
                value_numeric: value,
                currency: Some("PLN"),
                confirmation_state: "confirmed",
                source_tier: "esef",
                extraction_method: "api",
                validation_status: "passed",
                drift_json: None,
                citation: Some("history"),
                attribution: None,
                measure_window: None,
                data_quality: None,
            })
            .expect("seed revenue history");
    }
}

fn provenance_by_fact_id(
    state: &AppState,
    facts: &[FinancialFact],
) -> HashMap<String, crate::storage::FactProvenance> {
    let ids: Vec<String> = facts.iter().map(|f| f.id.clone()).collect();
    state
        .fundamentals_provenance()
        .get_many(&ids)
        .expect("get_many")
        .into_iter()
        .map(|entry| (entry.fact_id.clone(), entry))
        .collect()
}

// ---------------------------------------------------------------------------
// Outcomes: created / reobserved / upgraded / divergent / no_definition
// ---------------------------------------------------------------------------

#[test]
fn creates_a_new_fact_with_agent_tier_and_confirmed_state() {
    let (state, company_id, document_id) = state_with_company_and_document();
    seed_revenue_history(&state, &company_id, &document_id);
    let facts = [fact("revenue", "1200000", "p.3 H1 revenue")];
    let result = record_financial_facts(
        &state,
        RecordFinancialFactsInput {
            company_id: &company_id,
            report_document_id: &document_id,
            fiscal_year: 2026,
            period_type: "H1",
            period_end: Some("2026-06-30"),
            data_quality: None,
            facts: &facts,
        },
    )
    .expect("batch result");

    assert!(result.period_id.starts_with("finper_"));
    assert_eq!(result.outcomes.len(), 1);
    assert_eq!(result.outcomes[0].metric_key, "revenue");
    assert_eq!(result.outcomes[0].outcome, "created");
    assert!(result.outcomes[0].plausibility.is_none());

    let all_facts = company_facts(&state, &company_id);
    assert_eq!(all_facts.len(), 3); // 2 seeded history + 1 new
    let stored = all_facts
        .iter()
        .find(|f| f.value_numeric == "1200000")
        .expect("the H1 fact");
    assert_eq!(stored.confirmation_state, "confirmed");
    assert_eq!(stored.data_quality, "final");

    let provenance = provenance_by_fact_id(&state, &all_facts);
    let entry = &provenance[&stored.id];
    assert_eq!(entry.source_tier, "agent");
    assert_eq!(entry.validation_status, "passed");
    assert_eq!(entry.citation.as_deref(), Some("p.3 H1 revenue"));
}

#[test]
fn reobserves_an_identical_value_idempotently() {
    let (state, company_id, document_id) = state_with_company_and_document();
    let facts = [fact("revenue", "1200000", "p.3")];
    let input = RecordFinancialFactsInput {
        company_id: &company_id,
        report_document_id: &document_id,
        fiscal_year: 2026,
        period_type: "H1",
        period_end: Some("2026-06-30"),
        data_quality: None,
        facts: &facts,
    };
    record_financial_facts(&state, input).expect("first write");
    let result = record_financial_facts(&state, input).expect("second write");

    assert_eq!(result.outcomes[0].outcome, "reobserved");
    assert_eq!(company_facts(&state, &company_id).len(), 1);
}

#[test]
fn upgrades_an_aggregator_held_slot() {
    let (state, company_id, document_id) = state_with_company_and_document();
    // Seed an html_aggregator fact first (the ADR 0093 dec. 1 ordering: agent
    // outranks html_aggregator).
    state
        .kpi_extraction()
        .record_aggregator_fact(StructuredFactInput {
            company_id: &company_id,
            fiscal_year: 2026,
            period_type: "H1",
            period_end: Some("2026-06-30"),
            report_document_id: &document_id,
            metric_key: "revenue",
            value_numeric: "1000000",
            currency: Some("PLN"),
            confirmation_state: "confirmed",
            source_tier: "html_aggregator",
            extraction_method: "api",
            validation_status: "unreviewed",
            drift_json: None,
            citation: Some("aggregator"),
            attribution: None,
            measure_window: None,
            data_quality: None,
        })
        .expect("seed aggregator fact");

    let facts = [fact("revenue", "1200000", "p.3 H1 revenue")];
    let result = record_financial_facts(
        &state,
        RecordFinancialFactsInput {
            company_id: &company_id,
            report_document_id: &document_id,
            fiscal_year: 2026,
            period_type: "H1",
            period_end: Some("2026-06-30"),
            data_quality: None,
            facts: &facts,
        },
    )
    .expect("upgrade batch");

    assert_eq!(result.outcomes[0].outcome, "upgraded");
    let stored = company_facts(&state, &company_id);
    assert_eq!(stored.len(), 1);
    assert_eq!(stored[0].value_numeric, "1200000");
    let provenance = provenance_by_fact_id(&state, &stored);
    assert_eq!(provenance[&stored[0].id].source_tier, "agent");
}

#[test]
fn diverges_from_an_issuer_held_slot_without_overwriting() {
    let (state, company_id, document_id) = state_with_company_and_document();
    state
        .kpi_extraction()
        .record_structured_fact(StructuredFactInput {
            company_id: &company_id,
            fiscal_year: 2026,
            period_type: "H1",
            period_end: Some("2026-06-30"),
            report_document_id: &document_id,
            metric_key: "revenue",
            value_numeric: "1000000",
            currency: Some("PLN"),
            confirmation_state: "confirmed",
            source_tier: "esef",
            extraction_method: "api",
            validation_status: "passed",
            drift_json: None,
            citation: Some("issuer filing"),
            attribution: None,
            measure_window: None,
            data_quality: None,
        })
        .expect("seed issuer fact");

    let facts = [fact("revenue", "1200000", "p.3 H1 revenue")];
    let result = record_financial_facts(
        &state,
        RecordFinancialFactsInput {
            company_id: &company_id,
            report_document_id: &document_id,
            fiscal_year: 2026,
            period_type: "H1",
            period_end: Some("2026-06-30"),
            data_quality: None,
            facts: &facts,
        },
    )
    .expect("divergent batch");

    assert_eq!(result.outcomes[0].outcome, "divergent");
    let detail = result.outcomes[0].detail.as_ref().expect("detail");
    assert_eq!(detail["existing"], "1000000");
    assert_eq!(detail["incoming"], "1200000");

    // The stored (issuer) value is untouched — never silently overwritten.
    let stored = company_facts(&state, &company_id);
    assert_eq!(stored.len(), 1);
    assert_eq!(stored[0].value_numeric, "1000000");
    let provenance = provenance_by_fact_id(&state, &stored);
    assert_eq!(provenance[&stored[0].id].source_tier, "esef");
}

#[test]
fn no_definition_is_a_typed_skip_that_never_writes() {
    let (state, company_id, document_id) = state_with_company_and_document();
    let facts = [fact("totally_made_up_metric", "1", "p.1")];
    let result = record_financial_facts(
        &state,
        RecordFinancialFactsInput {
            company_id: &company_id,
            report_document_id: &document_id,
            fiscal_year: 2026,
            period_type: "H1",
            period_end: Some("2026-06-30"),
            data_quality: None,
            facts: &facts,
        },
    )
    .expect("batch result");

    assert_eq!(result.outcomes[0].outcome, "no_definition");
    assert!(company_facts(&state, &company_id).is_empty());
    // The period is still ensured even though nothing was written.
    assert!(result.period_id.starts_with("finper_"));
}

// ---------------------------------------------------------------------------
// Plausibility / identity verdicts
// ---------------------------------------------------------------------------

#[test]
fn implausible_facts_are_reported_and_never_written() {
    let (state, company_id, document_id) = state_with_company_and_document();
    state
        .kpi_extraction()
        .record_structured_fact(StructuredFactInput {
            company_id: &company_id,
            fiscal_year: 2024,
            period_type: "FY",
            period_end: Some("2024-12-31"),
            report_document_id: &document_id,
            metric_key: "cash",
            value_numeric: "100000",
            currency: Some("PLN"),
            confirmation_state: "confirmed",
            source_tier: "esef",
            extraction_method: "api",
            validation_status: "passed",
            drift_json: None,
            citation: Some("history"),
            attribution: None,
            measure_window: None,
            data_quality: None,
        })
        .expect("history 1");
    state
        .kpi_extraction()
        .record_structured_fact(StructuredFactInput {
            company_id: &company_id,
            fiscal_year: 2025,
            period_type: "FY",
            period_end: Some("2025-12-31"),
            report_document_id: &document_id,
            metric_key: "cash",
            value_numeric: "120000",
            currency: Some("PLN"),
            confirmation_state: "confirmed",
            source_tier: "esef",
            extraction_method: "api",
            validation_status: "passed",
            drift_json: None,
            citation: Some("history"),
            attribution: None,
            measure_window: None,
            data_quality: None,
        })
        .expect("history 2");

    // ~167x the history median (120_000) — a dropped-multiplier error.
    let facts = [fact("cash", "20000000", "p.9")];
    let result = record_financial_facts(
        &state,
        RecordFinancialFactsInput {
            company_id: &company_id,
            report_document_id: &document_id,
            fiscal_year: 2026,
            period_type: "FY",
            period_end: Some("2026-12-31"),
            data_quality: None,
            facts: &facts,
        },
    )
    .expect("batch result");

    assert_eq!(result.outcomes[0].outcome, "implausible");
    let detail = result.outcomes[0].detail.as_ref().expect("detail");
    assert_eq!(detail["historyMedian"], "120000");
    // Only the 2 seeded history facts exist — the implausible candidate never wrote.
    assert_eq!(company_facts(&state, &company_id).len(), 2);
}

#[test]
fn abstained_thin_history_is_written_and_marked_unreviewed() {
    let (state, company_id, document_id) = state_with_company_and_document();
    // No prior history at all for total_equity ⇒ thin-history abstention.
    let facts = [fact("total_equity", "550000", "p.4")];
    let result = record_financial_facts(
        &state,
        RecordFinancialFactsInput {
            company_id: &company_id,
            report_document_id: &document_id,
            fiscal_year: 2026,
            period_type: "H1",
            period_end: Some("2026-06-30"),
            data_quality: None,
            facts: &facts,
        },
    )
    .expect("batch result");

    assert_eq!(result.outcomes[0].outcome, "created");
    assert_eq!(
        result.outcomes[0].plausibility,
        Some("abstained_thin_history")
    );
    let stored = company_facts(&state, &company_id);
    assert_eq!(stored.len(), 1);
    let provenance = provenance_by_fact_id(&state, &stored);
    // Honest abstention — never a silent pass.
    assert_eq!(provenance[&stored[0].id].validation_status, "unreviewed");
}

#[test]
fn identity_violation_is_reported_and_never_written() {
    let (state, company_id, document_id) = state_with_company_and_document();
    // 200k + 200k != 1_000_000 — well beyond tolerance.
    let facts = [
        fact("total_assets", "1000000", "p.5"),
        fact("total_liabilities", "200000", "p.5"),
        fact("total_equity", "200000", "p.5"),
    ];
    let result = record_financial_facts(
        &state,
        RecordFinancialFactsInput {
            company_id: &company_id,
            report_document_id: &document_id,
            fiscal_year: 2026,
            period_type: "FY",
            period_end: Some("2026-12-31"),
            data_quality: None,
            facts: &facts,
        },
    )
    .expect("batch result");

    assert!(result
        .outcomes
        .iter()
        .all(|outcome| outcome.outcome == "identity_violation"));
    assert!(result.outcomes[0].detail.as_ref().unwrap()["check"]
        .as_str()
        .unwrap()
        .contains("balance_sheet"));
    // None of the three involved facts were persisted.
    assert!(company_facts(&state, &company_id).is_empty());
}

#[test]
fn batch_continues_past_a_bad_fact_and_commits_the_good_ones() {
    let (state, company_id, document_id) = state_with_company_and_document();
    let facts = [
        fact("revenue", "1200000", "p.3"),
        fact("totally_made_up_metric", "1", "p.1"),
    ];
    let result = record_financial_facts(
        &state,
        RecordFinancialFactsInput {
            company_id: &company_id,
            report_document_id: &document_id,
            fiscal_year: 2026,
            period_type: "H1",
            period_end: Some("2026-06-30"),
            data_quality: None,
            facts: &facts,
        },
    )
    .expect("batch result");

    assert_eq!(result.outcomes[0].outcome, "created");
    assert_eq!(result.outcomes[1].outcome, "no_definition");
    assert_eq!(company_facts(&state, &company_id).len(), 1);
}

// ---------------------------------------------------------------------------
// Refusals (whole-batch, before any write)
// ---------------------------------------------------------------------------

#[test]
fn empty_batch_is_a_typed_refusal() {
    let (state, company_id, document_id) = state_with_company_and_document();
    let error = record_financial_facts(
        &state,
        RecordFinancialFactsInput {
            company_id: &company_id,
            report_document_id: &document_id,
            fiscal_year: 2026,
            period_type: "H1",
            period_end: None,
            data_quality: None,
            facts: &[],
        },
    )
    .expect_err("empty batch must be refused");
    assert_eq!(error.code, CommandErrorCode::InvalidInput);
}

#[test]
fn oversized_batch_is_a_typed_refusal_and_writes_nothing() {
    let (state, company_id, document_id) = state_with_company_and_document();
    let owned: Vec<(String, String)> = (0..101)
        .map(|i| (format!("metric_{i}"), format!("p.{i}")))
        .collect();
    let facts: Vec<BatchFactInput<'_>> = owned
        .iter()
        .map(|(_, citation)| fact("revenue", "1", citation))
        .collect();

    let error = record_financial_facts(
        &state,
        RecordFinancialFactsInput {
            company_id: &company_id,
            report_document_id: &document_id,
            fiscal_year: 2026,
            period_type: "H1",
            period_end: None,
            data_quality: None,
            facts: &facts,
        },
    )
    .expect_err("101 facts must be refused");
    assert_eq!(error.code, CommandErrorCode::InvalidInput);
    assert!(company_facts(&state, &company_id).is_empty());
}

#[test]
fn garbage_attribution_is_a_typed_refusal_before_any_write() {
    let (state, company_id, document_id) = state_with_company_and_document();
    let facts = [BatchFactInput {
        metric_key: "revenue",
        value_numeric: "1200000",
        currency: Some("PLN"),
        attribution: Some("this is a citation, not a slot"),
        measure_window: None,
        citation: "p.3",
    }];
    let error = record_financial_facts(
        &state,
        RecordFinancialFactsInput {
            company_id: &company_id,
            report_document_id: &document_id,
            fiscal_year: 2026,
            period_type: "H1",
            period_end: None,
            data_quality: None,
            facts: &facts,
        },
    )
    .expect_err("garbage attribution must be refused");
    assert_eq!(error.code, CommandErrorCode::InvalidInput);
    assert!(company_facts(&state, &company_id).is_empty());
}

#[test]
fn unknown_company_id_is_a_typed_not_found_refusal() {
    let (state, _company_id, document_id) = state_with_company_and_document();
    let facts = [fact("revenue", "1200000", "p.3")];
    let error = record_financial_facts(
        &state,
        RecordFinancialFactsInput {
            company_id: "company_does_not_exist",
            report_document_id: &document_id,
            fiscal_year: 2026,
            period_type: "H1",
            period_end: None,
            data_quality: None,
            facts: &facts,
        },
    )
    .expect_err("unknown company must be refused");
    assert_eq!(error.code, CommandErrorCode::NotFound);
}

#[test]
fn unknown_report_document_id_is_a_typed_not_found_refusal() {
    let (state, company_id, _document_id) = state_with_company_and_document();
    let facts = [fact("revenue", "1200000", "p.3")];
    let error = record_financial_facts(
        &state,
        RecordFinancialFactsInput {
            company_id: &company_id,
            report_document_id: "document_does_not_exist",
            fiscal_year: 2026,
            period_type: "H1",
            period_end: None,
            data_quality: None,
            facts: &facts,
        },
    )
    .expect_err("unknown report document must be refused");
    assert_eq!(error.code, CommandErrorCode::NotFound);
    // The period must NOT have been ensured — the document check runs first.
    assert!(company_facts(&state, &company_id).is_empty());
}

// ---------------------------------------------------------------------------
// Preliminary lifecycle (ADR 0093 dec. 2/6)
// ---------------------------------------------------------------------------

#[test]
fn preliminary_batch_writes_data_quality_and_final_batch_supersedes_it() {
    let (state, company_id, document_id) = state_with_company_and_document();
    let prelim_facts = [fact("net_profit", "492200000", "XTB RB 18/2026 p.2")];
    record_financial_facts(
        &state,
        RecordFinancialFactsInput {
            company_id: &company_id,
            report_document_id: &document_id,
            fiscal_year: 2026,
            period_type: "H1",
            period_end: Some("2026-06-30"),
            data_quality: Some("preliminary"),
            facts: &prelim_facts,
        },
    )
    .expect("preliminary batch");

    let after_prelim = company_facts(&state, &company_id);
    assert_eq!(after_prelim.len(), 1);
    assert_eq!(after_prelim[0].data_quality, "preliminary");
    assert!(after_prelim[0].supersedes_id.is_none());

    let final_facts = [fact("net_profit", "495000000", "audited report p.12")];
    record_financial_facts(
        &state,
        RecordFinancialFactsInput {
            company_id: &company_id,
            report_document_id: &document_id,
            fiscal_year: 2026,
            period_type: "H1",
            period_end: Some("2026-06-30"),
            data_quality: None, // defaults to "final"
            facts: &final_facts,
        },
    )
    .expect("final batch");

    let after_final = company_facts(&state, &company_id);
    // Both the preliminary and the final row coexist (the slot dimension
    // includes data_quality).
    assert_eq!(after_final.len(), 2);
    let final_row = after_final
        .iter()
        .find(|f| f.data_quality == "final")
        .expect("final row present");
    let prelim_row = after_final
        .iter()
        .find(|f| f.data_quality == "preliminary")
        .expect("preliminary row still present");
    assert_eq!(final_row.value_numeric, "495000000");
    assert_eq!(
        final_row.supersedes_id.as_deref(),
        Some(prelim_row.id.as_str())
    );
}
