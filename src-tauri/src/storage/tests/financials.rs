use super::*;
use rust_decimal::Decimal;

// ---------------------------------------------------------------------------
// stored_fact_set (ADR 0061 dec. 4b): the comparative cross-check's
// "already known" prior period, read out of storage and bridged from
// definition_id to metric_key.
// ---------------------------------------------------------------------------

fn seed_fact(state: &AppState, company_id: &str, period_id: &str, metric_key: &str, value: &str) {
    let definitions = state
        .list_kpi_definitions(ListKpiDefinitionsInput {
            scope: Some("canonical".to_owned()),
            sector: None,
            company_id: None,
        })
        .expect("canonical definitions should list");
    let definition = definitions
        .iter()
        .find(|d| d.metric_key == metric_key)
        .unwrap_or_else(|| panic!("{metric_key} should exist in the canonical catalog"));
    state
        .create_financial_fact(NewFinancialFact {
            company_id: company_id.to_owned(),
            period_id: period_id.to_owned(),
            definition_id: definition.id.clone(),
            value_numeric: value.to_owned(),
            currency: Some("PLN".to_owned()),
            statement_basis: None,
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
        })
        .expect("financial fact should create");
}

#[test]
fn stored_fact_set_bridges_definition_id_to_metric_key() {
    let connection = open_in_memory_database().expect("database should initialize");
    let state = AppState::new(connection);
    let company = tracked_company(&state);

    let period = state
        .create_financial_period(NewFinancialPeriod {
            company_id: company.id.clone(),
            fiscal_year: 2025,
            period_type: "FY".to_owned(),
            period_end_date: Some("2025-12-31".to_owned()),
            report_evidence_ref: None,
        })
        .expect("financial period should create");
    seed_fact(&state, &company.id, &period.id, "total_assets", "40000000");
    seed_fact(&state, &company.id, &period.id, "net_profit", "1100000");

    let set = state
        .financials()
        .stored_fact_set(&company.id, 2025, "FY")
        .expect("stored_fact_set should query")
        .expect("a period with facts should yield Some");

    assert_eq!(set.get("total_assets"), Some(&Decimal::new(40_000_000, 0)));
    assert_eq!(set.get("net_profit"), Some(&Decimal::new(1_100_000, 0)));
    assert_eq!(set.len(), 2);
}

/// Guardrail (card f64cea2): the out-of-spec 'annual' fiscal label is folded to
/// the canonical FY at the create-period boundary, so a legacy label cannot be
/// reintroduced after migration 0066.
#[test]
fn create_financial_period_folds_annual_label_to_fy() {
    let connection = open_in_memory_database().expect("database should initialize");
    let state = AppState::new(connection);
    let company = tracked_company(&state);

    let period = state
        .create_financial_period(NewFinancialPeriod {
            company_id: company.id.clone(),
            fiscal_year: 2025,
            period_type: "annual".to_owned(),
            period_end_date: None,
            report_evidence_ref: None,
        })
        .expect("financial period should create");

    assert_eq!(period.period_type, "FY", "annual must be stored as FY");
}

#[test]
fn stored_fact_set_period_type_matches_case_insensitively() {
    let connection = open_in_memory_database().expect("database should initialize");
    let state = AppState::new(connection);
    let company = tracked_company(&state);

    let period = state
        .create_financial_period(NewFinancialPeriod {
            company_id: company.id.clone(),
            fiscal_year: 2025,
            period_type: "FY".to_owned(),
            period_end_date: None,
            report_evidence_ref: None,
        })
        .expect("financial period should create");
    seed_fact(&state, &company.id, &period.id, "total_assets", "1");

    let set = state
        .financials()
        .stored_fact_set(&company.id, 2025, "fy")
        .expect("stored_fact_set should query");
    assert!(set.is_some(), "period_type match must be case-insensitive");
}

#[test]
fn stored_fact_set_none_when_no_matching_period() {
    let connection = open_in_memory_database().expect("database should initialize");
    let state = AppState::new(connection);
    let company = tracked_company(&state);

    let set = state
        .financials()
        .stored_fact_set(&company.id, 2025, "FY")
        .expect("stored_fact_set should query");
    assert!(set.is_none());
}

#[test]
fn stored_fact_set_none_when_period_has_no_facts() {
    let connection = open_in_memory_database().expect("database should initialize");
    let state = AppState::new(connection);
    let company = tracked_company(&state);

    state
        .create_financial_period(NewFinancialPeriod {
            company_id: company.id.clone(),
            fiscal_year: 2025,
            period_type: "FY".to_owned(),
            period_end_date: None,
            report_evidence_ref: None,
        })
        .expect("financial period should create");

    let set = state
        .financials()
        .stored_fact_set(&company.id, 2025, "FY")
        .expect("stored_fact_set should query");
    assert!(set.is_none(), "a period with zero facts is not Some(empty)");
}

#[test]
fn stored_fact_set_skips_unparsable_values_but_keeps_the_rest() {
    let connection = open_in_memory_database().expect("database should initialize");
    let state = AppState::new(connection);
    let company = tracked_company(&state);

    let period = state
        .create_financial_period(NewFinancialPeriod {
            company_id: company.id.clone(),
            fiscal_year: 2025,
            period_type: "FY".to_owned(),
            period_end_date: None,
            report_evidence_ref: None,
        })
        .expect("financial period should create");
    seed_fact(&state, &company.id, &period.id, "total_assets", "40000000");
    seed_fact(
        &state,
        &company.id,
        &period.id,
        "net_profit",
        "not-a-number",
    );

    let set = state
        .financials()
        .stored_fact_set(&company.id, 2025, "FY")
        .expect("stored_fact_set should query")
        .expect("total_assets alone still yields Some");
    assert_eq!(set.get("total_assets"), Some(&Decimal::new(40_000_000, 0)));
    assert!(!set.contains_key("net_profit"));
}

#[test]
fn lists_canonical_kpi_definitions() {
    let connection = open_in_memory_database().expect("database should initialize");
    let state = AppState::new(connection);

    let definitions = state
        .list_kpi_definitions(ListKpiDefinitionsInput {
            scope: Some("canonical".to_owned()),
            sector: None,
            company_id: None,
        })
        .expect("canonical KPI definitions should list");

    assert!(!definitions.is_empty());
    let net_profit = definitions
        .iter()
        .find(|d| d.metric_key == "net_profit")
        .expect("net_profit should be in canonical definitions");
    assert_eq!(net_profit.scope, "canonical");
    assert_eq!(net_profit.label, "Net profit");
    assert_eq!(net_profit.value_kind, "monetary");
    assert_eq!(net_profit.computation, "reported");
}

#[test]
fn lists_sector_kpi_definitions() {
    let connection = open_in_memory_database().expect("database should initialize");
    let state = AppState::new(connection);

    let banking_defs = state
        .list_kpi_definitions(ListKpiDefinitionsInput {
            scope: Some("sector".to_owned()),
            sector: Some("banking".to_owned()),
            company_id: None,
        })
        .expect("banking KPI definitions should list");

    assert!(!banking_defs.is_empty());
    assert!(banking_defs
        .iter()
        .any(|d| d.metric_key == "net_interest_income"));
    assert!(banking_defs.iter().all(|d| d.scope == "sector"));
    assert!(banking_defs
        .iter()
        .all(|d| d.sector == Some("banking".to_owned())));
}

#[test]
fn creates_company_custom_kpi_definition() {
    let connection = open_in_memory_database().expect("database should initialize");
    let state = AppState::new(connection);
    let company = tracked_company(&state);

    let custom_kpi = state
        .create_kpi_definition(NewKpiDefinition {
            scope: "company".to_owned(),
            company_id: Some(company.id.clone()),
            sector: None,
            metric_key: "custom_metric".to_owned(),
            label: "Custom Metric".to_owned(),
            value_kind: "percentage".to_owned(),
            unit: None,
            computation: "derived".to_owned(),
            formula: Some("metric_a / metric_b".to_owned()),
            display_format: None,
        })
        .expect("custom KPI definition should create");

    assert_eq!(custom_kpi.scope, "company");
    assert_eq!(custom_kpi.company_id, Some(company.id));
    assert_eq!(custom_kpi.metric_key, "custom_metric");
}

#[test]
fn creates_financial_period() {
    let connection = open_in_memory_database().expect("database should initialize");
    let state = AppState::new(connection);
    let company = tracked_company(&state);

    let period = state
        .create_financial_period(NewFinancialPeriod {
            company_id: company.id.clone(),
            fiscal_year: 2026,
            period_type: "FY".to_owned(),
            period_end_date: Some("2026-12-31".to_owned()),
            report_evidence_ref: Some("annual_report_2026".to_owned()),
        })
        .expect("financial period should create");

    assert_eq!(period.company_id, company.id);
    assert_eq!(period.fiscal_year, 2026);
    assert_eq!(period.period_type, "FY");
    assert_eq!(period.period_end_date, Some("2026-12-31".to_owned()));
    assert_eq!(
        period.report_evidence_ref,
        Some("annual_report_2026".to_owned())
    );
}

#[test]
fn updates_financial_period() {
    let connection = open_in_memory_database().expect("database should initialize");
    let state = AppState::new(connection);
    let company = tracked_company(&state);

    let period = state
        .create_financial_period(NewFinancialPeriod {
            company_id: company.id.clone(),
            fiscal_year: 2026,
            period_type: "H1".to_owned(),
            period_end_date: Some("2026-06-30".to_owned()),
            report_evidence_ref: None,
        })
        .expect("financial period should create");

    let updated = state
        .update_financial_period(UpdateFinancialPeriod {
            id: period.id.clone(),
            period_end_date: Some("2026-07-15".to_owned()),
            report_evidence_ref: Some("h1_report_2026".to_owned()),
        })
        .expect("financial period should update");

    assert_eq!(updated.period_end_date, Some("2026-07-15".to_owned()));
    assert_eq!(
        updated.report_evidence_ref,
        Some("h1_report_2026".to_owned())
    );
}

#[test]
fn lists_financial_periods_by_company() {
    let connection = open_in_memory_database().expect("database should initialize");
    let state = AppState::new(connection);
    let company = tracked_company(&state);

    state
        .create_financial_period(NewFinancialPeriod {
            company_id: company.id.clone(),
            fiscal_year: 2026,
            period_type: "FY".to_owned(),
            period_end_date: None,
            report_evidence_ref: None,
        })
        .expect("first period should create");

    state
        .create_financial_period(NewFinancialPeriod {
            company_id: company.id.clone(),
            fiscal_year: 2026,
            period_type: "H1".to_owned(),
            period_end_date: None,
            report_evidence_ref: None,
        })
        .expect("second period should create");

    let periods = state
        .list_financial_periods(ListFinancialPeriodsInput {
            company_id: company.id.clone(),
            fiscal_year: None,
        })
        .expect("financial periods should list");

    assert_eq!(periods.len(), 2);
    assert!(periods.iter().all(|p| p.company_id == company.id));
    assert!(periods.iter().any(|p| p.period_type == "FY"));
    assert!(periods.iter().any(|p| p.period_type == "H1"));
}

#[test]
fn creates_and_lists_kpi_relevance() {
    let connection = open_in_memory_database().expect("database should initialize");
    let state = AppState::new(connection);
    let company = tracked_company(&state);

    let definitions = state
        .list_kpi_definitions(ListKpiDefinitionsInput {
            scope: Some("canonical".to_owned()),
            sector: None,
            company_id: None,
        })
        .expect("canonical definitions should list");
    let net_profit_def = definitions
        .iter()
        .find(|d| d.metric_key == "net_profit")
        .expect("net_profit should exist");

    let relevance = state
        .create_kpi_relevance(NewKpiRelevance {
            company_id: company.id.clone(),
            definition_id: net_profit_def.id.clone(),
            source: "financial_report".to_owned(),
            rank: Some("1".to_owned()),
            first_seen_period: Some("2026-Q1".to_owned()),
            last_seen_period: None,
        })
        .expect("kpi relevance should create");

    assert_eq!(relevance.company_id, company.id);
    assert_eq!(relevance.definition_id, net_profit_def.id);
    assert_eq!(relevance.status, "active");
    assert_eq!(relevance.source, "financial_report");
    assert_eq!(relevance.rank, Some("1".to_owned()));

    let relevances = state
        .list_kpi_relevance(&company.id)
        .expect("kpi relevance should list");

    assert!(!relevances.is_empty());
    assert!(relevances
        .iter()
        .any(|r| r.definition_id == net_profit_def.id));
}

#[test]
fn updates_kpi_relevance_status() {
    let connection = open_in_memory_database().expect("database should initialize");
    let state = AppState::new(connection);
    let company = tracked_company(&state);

    let definitions = state
        .list_kpi_definitions(ListKpiDefinitionsInput {
            scope: Some("canonical".to_owned()),
            sector: None,
            company_id: None,
        })
        .expect("canonical definitions should list");
    let revenue_def = definitions
        .iter()
        .find(|d| d.metric_key == "revenue")
        .expect("revenue should exist");

    let relevance = state
        .create_kpi_relevance(NewKpiRelevance {
            company_id: company.id.clone(),
            definition_id: revenue_def.id.clone(),
            source: "earnings_call".to_owned(),
            rank: None,
            first_seen_period: None,
            last_seen_period: None,
        })
        .expect("kpi relevance should create");

    let updated = state
        .update_kpi_relevance(UpdateKpiRelevance {
            id: relevance.id.clone(),
            status: Some("inactive".to_owned()),
            rank: Some("2".to_owned()),
            first_seen_period: Some("2025-Q4".to_owned()),
            last_seen_period: Some("2026-Q2".to_owned()),
        })
        .expect("kpi relevance should update");

    assert_eq!(updated.status, "inactive");
    assert_eq!(updated.rank, Some("2".to_owned()));
    assert_eq!(updated.first_seen_period, Some("2025-Q4".to_owned()));
}

#[test]
fn creates_financial_fact() {
    let connection = open_in_memory_database().expect("database should initialize");
    let state = AppState::new(connection);
    let company = tracked_company(&state);

    let period = state
        .create_financial_period(NewFinancialPeriod {
            company_id: company.id.clone(),
            fiscal_year: 2026,
            period_type: "FY".to_owned(),
            period_end_date: Some("2026-12-31".to_owned()),
            report_evidence_ref: None,
        })
        .expect("financial period should create");

    let definitions = state
        .list_kpi_definitions(ListKpiDefinitionsInput {
            scope: Some("canonical".to_owned()),
            sector: None,
            company_id: None,
        })
        .expect("canonical definitions should list");
    let net_profit_def = definitions
        .iter()
        .find(|d| d.metric_key == "net_profit")
        .expect("net_profit should exist");

    let fact = state
        .create_financial_fact(NewFinancialFact {
            company_id: company.id.clone(),
            period_id: period.id.clone(),
            definition_id: net_profit_def.id.clone(),
            value_numeric: "1234567.89".to_owned(),
            currency: Some("PLN".to_owned()),
            statement_basis: Some("consolidated".to_owned()),
            attribution: None,
            variant: Some("reported".to_owned()),
            measure_window: Some("flow".to_owned()),
            data_quality: Some("final".to_owned()),
            as_reported_value: None,
            as_reported_scale: None,
            reporting_standard: Some("IFRS".to_owned()),
            extraction_method: None,
            confidence: None,
            confirmation_state: Some("confirmed".to_owned()),
            supersedes_id: None,
            source_document_ref: Some("annual_report_2026.pdf".to_owned()),
        })
        .expect("financial fact should create");

    assert_eq!(fact.company_id, company.id);
    assert_eq!(fact.period_id, period.id);
    assert_eq!(fact.definition_id, net_profit_def.id);
    assert_eq!(fact.value_numeric, "1234567.89");
    assert_eq!(fact.currency, Some("PLN".to_owned()));
    assert_eq!(fact.statement_basis, "consolidated");
    assert_eq!(fact.data_quality, "final");
}

#[test]
fn lists_financial_facts_by_company() {
    let connection = open_in_memory_database().expect("database should initialize");
    let state = AppState::new(connection);
    let company = tracked_company(&state);

    let period = state
        .create_financial_period(NewFinancialPeriod {
            company_id: company.id.clone(),
            fiscal_year: 2026,
            period_type: "FY".to_owned(),
            period_end_date: None,
            report_evidence_ref: None,
        })
        .expect("financial period should create");

    let definitions = state
        .list_kpi_definitions(ListKpiDefinitionsInput {
            scope: Some("canonical".to_owned()),
            sector: None,
            company_id: None,
        })
        .expect("canonical definitions should list");

    let net_profit_def = definitions
        .iter()
        .find(|d| d.metric_key == "net_profit")
        .expect("net_profit should exist");
    let revenue_def = definitions
        .iter()
        .find(|d| d.metric_key == "revenue")
        .expect("revenue should exist");

    state
        .create_financial_fact(NewFinancialFact {
            company_id: company.id.clone(),
            period_id: period.id.clone(),
            definition_id: net_profit_def.id.clone(),
            value_numeric: "100".to_owned(),
            currency: None,
            statement_basis: None,
            attribution: None,
            variant: None,
            measure_window: None,
            data_quality: None,
            as_reported_value: None,
            as_reported_scale: None,
            reporting_standard: None,
            extraction_method: None,
            confidence: None,
            confirmation_state: None,
            supersedes_id: None,
            source_document_ref: None,
        })
        .expect("first fact should create");

    state
        .create_financial_fact(NewFinancialFact {
            company_id: company.id.clone(),
            period_id: period.id.clone(),
            definition_id: revenue_def.id.clone(),
            value_numeric: "1000".to_owned(),
            currency: None,
            statement_basis: None,
            attribution: None,
            variant: None,
            measure_window: None,
            data_quality: None,
            as_reported_value: None,
            as_reported_scale: None,
            reporting_standard: None,
            extraction_method: None,
            confidence: None,
            confirmation_state: None,
            supersedes_id: None,
            source_document_ref: None,
        })
        .expect("second fact should create");

    let facts = state
        .list_financial_facts(ListFinancialFactsInput {
            company_id: Some(company.id.clone()),
            period_id: None,
            definition_id: None,
        })
        .expect("facts should list");

    assert_eq!(facts.len(), 2);
    assert!(facts.iter().all(|f| f.company_id == company.id));
}

#[test]
fn updates_financial_fact() {
    let connection = open_in_memory_database().expect("database should initialize");
    let state = AppState::new(connection);
    let company = tracked_company(&state);

    let period = state
        .create_financial_period(NewFinancialPeriod {
            company_id: company.id.clone(),
            fiscal_year: 2026,
            period_type: "FY".to_owned(),
            period_end_date: None,
            report_evidence_ref: None,
        })
        .expect("financial period should create");

    let definitions = state
        .list_kpi_definitions(ListKpiDefinitionsInput {
            scope: Some("canonical".to_owned()),
            sector: None,
            company_id: None,
        })
        .expect("canonical definitions should list");

    let cash_def = definitions
        .iter()
        .find(|d| d.metric_key == "cash")
        .expect("cash should exist");

    let fact = state
        .create_financial_fact(NewFinancialFact {
            company_id: company.id.clone(),
            period_id: period.id.clone(),
            definition_id: cash_def.id.clone(),
            value_numeric: "500000".to_owned(),
            currency: Some("EUR".to_owned()),
            statement_basis: None,
            attribution: None,
            variant: None,
            measure_window: None,
            data_quality: Some("estimate".to_owned()),
            as_reported_value: None,
            as_reported_scale: None,
            reporting_standard: None,
            extraction_method: None,
            confidence: None,
            confirmation_state: None,
            supersedes_id: None,
            source_document_ref: None,
        })
        .expect("fact should create");

    let updated = state
        .update_financial_fact(UpdateFinancialFact {
            id: fact.id.clone(),
            value_numeric: Some("550000".to_owned()),
            currency: Some("EUR".to_owned()),
            data_quality: Some("final".to_owned()),
            confirmation_state: Some("provisional".to_owned()),
            supersedes_id: None,
            source_document_ref: Some("revised_report.pdf".to_owned()),
        })
        .expect("fact should update");

    assert_eq!(updated.value_numeric, "550000");
    assert_eq!(updated.data_quality, "final");
    assert_eq!(updated.confirmation_state, "provisional");
    assert_eq!(
        updated.source_document_ref,
        Some("revised_report.pdf".to_owned())
    );
}

#[test]
fn deletes_financial_period_and_cascades_to_facts() {
    let connection = open_in_memory_database().expect("database should initialize");
    let state = AppState::new(connection);
    let company = tracked_company(&state);

    let period = state
        .create_financial_period(NewFinancialPeriod {
            company_id: company.id.clone(),
            fiscal_year: 2026,
            period_type: "FY".to_owned(),
            period_end_date: None,
            report_evidence_ref: None,
        })
        .expect("financial period should create");

    let definitions = state
        .list_kpi_definitions(ListKpiDefinitionsInput {
            scope: Some("canonical".to_owned()),
            sector: None,
            company_id: None,
        })
        .expect("canonical definitions should list");

    let total_assets_def = definitions
        .iter()
        .find(|d| d.metric_key == "total_assets")
        .expect("total_assets should exist");

    state
        .create_financial_fact(NewFinancialFact {
            company_id: company.id.clone(),
            period_id: period.id.clone(),
            definition_id: total_assets_def.id.clone(),
            value_numeric: "5000000".to_owned(),
            currency: None,
            statement_basis: None,
            attribution: None,
            variant: None,
            measure_window: None,
            data_quality: None,
            as_reported_value: None,
            as_reported_scale: None,
            reporting_standard: None,
            extraction_method: None,
            confidence: None,
            confirmation_state: None,
            supersedes_id: None,
            source_document_ref: None,
        })
        .expect("fact should create");

    state
        .delete_financial_period(&period.id)
        .expect("period should delete");

    let facts = state
        .list_financial_facts(ListFinancialFactsInput {
            company_id: Some(company.id),
            period_id: None,
            definition_id: None,
        })
        .expect("facts should list");

    assert_eq!(facts.len(), 0);
}

fn tracked_company(state: &AppState) -> Company {
    state
        .create_company(NewCompany {
            exchange: "GPW".to_owned(),
            ticker: "FIN".to_owned(),
            display_name: "Financials Test Co.".to_owned(),
            isin: Some("PLFINANCIALS".to_owned()),
            cik: None,
            lei: None,
        })
        .expect("tracked company should create")
}

// ---------------------------------------------------------------------------
// Migration 0106 — core KPI relevance seed (owner decision 2026-07-21).
//
// `kpi_relevance` had zero rows in production, so ADR 0061 decision 2(d)'s
// completeness check never fired: `expected_primary_metric_keys` returned
// `None` and recall had no denominator. The seed supplies a common IFRS core
// set as a starting denominator, WITHOUT ever touching a curated row.
// ---------------------------------------------------------------------------

const CORE_KPI_RELEVANCE_SEED: &str =
    include_str!("../../../migrations/0106_seed_core_kpi_relevance.sql");

const CORE_KPI_METRIC_KEYS: [&str; 5] = [
    "net_profit",
    "operating_profit",
    "revenue",
    "total_assets",
    "total_equity",
];

fn company_with_ticker(state: &AppState, ticker: &str) -> Company {
    state
        .create_company(NewCompany {
            exchange: "GPW".to_owned(),
            ticker: ticker.to_owned(),
            display_name: format!("{ticker} S.A."),
            isin: None,
            cik: None,
            lei: None,
        })
        .expect("company should create")
}

fn definition_id_for(state: &AppState, metric_key: &str) -> String {
    state
        .list_kpi_definitions(ListKpiDefinitionsInput {
            scope: Some("canonical".to_owned()),
            sector: None,
            company_id: None,
        })
        .expect("canonical definitions should list")
        .into_iter()
        .find(|d| d.metric_key == metric_key)
        .unwrap_or_else(|| panic!("{metric_key} should exist in the canonical catalog"))
        .id
}

#[test]
fn migration_0106_seeds_the_core_kpi_set_idempotently_without_touching_curation() {
    let connection = open_in_memory_database().expect("database should initialize");
    let state = AppState::new(connection);
    // The seed runs at migration time, before either company exists, so both
    // start with an empty relevance profile — the production starting point.
    let fresh = company_with_ticker(&state, "SEED");
    let curated = company_with_ticker(&state, "CUR");
    assert!(state
        .list_kpi_relevance(&fresh.id)
        .expect("relevance should list")
        .is_empty());

    // A user-curated row for one of the core metrics, deliberately ranked
    // `secondary`: the seed must leave it exactly as the owner left it.
    let revenue_definition = definition_id_for(&state, "revenue");
    let curated_row = state
        .create_kpi_relevance(NewKpiRelevance {
            company_id: curated.id.clone(),
            definition_id: revenue_definition.clone(),
            source: "user".to_owned(),
            rank: Some("secondary".to_owned()),
            first_seen_period: None,
            last_seen_period: None,
        })
        .expect("curated relevance should create");

    {
        let raw = state.checkout().expect("connection should check out");
        raw.execute_batch(CORE_KPI_RELEVANCE_SEED)
            .expect("seed should apply");
        raw.execute_batch(CORE_KPI_RELEVANCE_SEED)
            .expect("seed should be idempotent");
    }

    // Seeds the whole core set on an empty profile, exactly once.
    let seeded = state
        .list_kpi_relevance(&fresh.id)
        .expect("relevance should list");
    assert_eq!(seeded.len(), CORE_KPI_METRIC_KEYS.len());
    assert!(seeded
        .iter()
        .all(|r| r.status == "active" && r.source == "core"));
    assert!(seeded.iter().all(|r| r.rank.as_deref() == Some("primary")));

    // The curated row is untouched, and is not duplicated by the seed.
    let curated_rows = state
        .list_kpi_relevance(&curated.id)
        .expect("relevance should list");
    assert_eq!(curated_rows.len(), CORE_KPI_METRIC_KEYS.len());
    let revenue_rows: Vec<_> = curated_rows
        .iter()
        .filter(|r| r.definition_id == revenue_definition)
        .collect();
    assert_eq!(
        revenue_rows.len(),
        1,
        "no duplicate row for a curated metric"
    );
    assert_eq!(revenue_rows[0].id, curated_row.id);
    assert_eq!(revenue_rows[0].source, "user");
    assert_eq!(revenue_rows[0].rank.as_deref(), Some("secondary"));

    // The completeness check now has a denominator (ADR 0061 dec. 2(d)) — it was
    // `None` for every company before the seed.
    let expected = state
        .financials()
        .expected_primary_metric_keys(&fresh.id)
        .expect("expected keys should read")
        .expect("a seeded company has a denominator");
    let keys: Vec<&str> = expected.iter().map(String::as_str).collect();
    assert_eq!(keys, CORE_KPI_METRIC_KEYS);
}

// ---------------------------------------------------------------------------
// metric_histories (E1): the batched history read the per-fact plausibility
// gate uses. Its equivalence to N single `metric_history` calls is the oracle
// the batch refactor must preserve — same values, same order, every key present.
// ---------------------------------------------------------------------------

fn seed_fy_period(state: &AppState, company_id: &str, fiscal_year: i64) -> String {
    state
        .create_financial_period(NewFinancialPeriod {
            company_id: company_id.to_owned(),
            fiscal_year,
            period_type: "FY".to_owned(),
            period_end_date: Some(format!("{fiscal_year}-12-31")),
            report_evidence_ref: None,
        })
        .expect("financial period should create")
        .id
}

#[test]
fn metric_histories_equals_n_single_metric_history_reads() {
    let connection = open_in_memory_database().expect("database should initialize");
    let state = AppState::new(connection);
    let company = tracked_company(&state);

    // Three years of two metrics, plus the period being extracted (2025) which
    // the gate excludes, plus a metric with no facts at all.
    let p2022 = seed_fy_period(&state, &company.id, 2022);
    let p2023 = seed_fy_period(&state, &company.id, 2023);
    let p2024 = seed_fy_period(&state, &company.id, 2024);
    let p2025 = seed_fy_period(&state, &company.id, 2025);
    for (period, assets, profit) in [
        (&p2022, "10000000", "500000"),
        (&p2023, "20000000", "700000"),
        (&p2024, "30000000", "900000"),
        (&p2025, "40000000", "1100000"), // excluded period
    ] {
        seed_fact(&state, &company.id, period, "total_assets", assets);
        seed_fact(&state, &company.id, period, "net_profit", profit);
    }

    let keys: std::collections::BTreeSet<String> = ["total_assets", "net_profit", "revenue"]
        .into_iter()
        .map(str::to_owned)
        .collect();

    let batched = state
        .financials()
        .metric_histories(&company.id, &keys, 2025, "FY")
        .expect("batched read should query");

    // Every key present; the batched vector is byte-identical (values AND order)
    // to the single read for that key.
    for key in &keys {
        let single = state
            .financials()
            .metric_history(&company.id, key, 2025, "FY")
            .expect("single read should query");
        assert_eq!(
            batched.get(key),
            Some(&single),
            "batched history for {key} must equal the single read"
        );
    }

    // Sanity: the excluded 2025 value is absent — history is the three prior years.
    assert_eq!(batched.get("total_assets").expect("present").len(), 3);
    // A metric with no stored facts maps to an empty vector, not a missing key.
    assert_eq!(batched.get("revenue"), Some(&Vec::new()));
}

/// Owner-dogfooding catch (2026-07-22): the cockpit loads definitions with
/// `{ companyId }` and no scope — that call MUST return the canonical catalog
/// PLUS the company's own definitions, never company-scoped rows only. The old
/// `company_id = ?` filter excluded every canonical row (company_id NULL), so
/// the fact matrix synthesized placeholder definitions: title-cased English
/// labels ("Wdf Equity Parent") and lost `per_share` formatting for EPS.
#[test]
fn company_scoped_definition_list_includes_the_canonical_catalog() {
    let state = AppState::new(open_in_memory_database().expect("db"));
    let company = state
        .create_company(NewCompany {
            exchange: "GPW".to_owned(),
            ticker: "DEF".to_owned(),
            display_name: "Defs S.A.".to_owned(),
            isin: None,
            cik: None,
            lei: None,
        })
        .expect("company");
    let other = state
        .create_company(NewCompany {
            exchange: "GPW".to_owned(),
            ticker: "OTH".to_owned(),
            display_name: "Other S.A.".to_owned(),
            isin: None,
            cik: None,
            lei: None,
        })
        .expect("other company");
    for (owner_id, key) in [(&company.id, "custom_mine"), (&other.id, "custom_theirs")] {
        state
            .create_kpi_definition(NewKpiDefinition {
                scope: "company".to_owned(),
                company_id: Some(owner_id.clone()),
                sector: None,
                metric_key: key.to_owned(),
                label: key.to_owned(),
                value_kind: "monetary".to_owned(),
                unit: None,
                computation: "reported".to_owned(),
                formula: None,
                display_format: None,
            })
            .expect("custom definition");
    }

    let listed = state
        .list_kpi_definitions(ListKpiDefinitionsInput {
            scope: None,
            sector: None,
            company_id: Some(company.id.clone()),
        })
        .expect("list");

    let keys: std::collections::BTreeSet<&str> =
        listed.iter().map(|d| d.metric_key.as_str()).collect();
    assert!(
        keys.contains("eps_basic"),
        "canonical catalog must be included"
    );
    assert!(
        keys.contains("wdf_equity_parent"),
        "0111 seeds must be included"
    );
    assert!(
        keys.contains("custom_mine"),
        "the company's own definitions stay"
    );
    assert!(
        !keys.contains("custom_theirs"),
        "another company's custom definitions must NOT leak"
    );
    let eps = listed
        .iter()
        .find(|d| d.metric_key == "eps_basic")
        .expect("eps");
    assert_eq!(eps.unit.as_deref(), Some("per_share"));
}
