use super::*;
use crate::storage::financials::HistorySlotKey;
use rust_decimal::Decimal;
use std::collections::BTreeSet;

use crate::fundamentals::extraction::SourceTier;
use crate::fundamentals::validation::completeness;

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
            annotation: None,
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
            origin: None,
            statement_group: None,
            period_nature: None,
        })
        .expect("custom KPI definition should create");

    assert_eq!(custom_kpi.scope, "company");
    assert_eq!(custom_kpi.company_id, Some(company.id));
    assert_eq!(custom_kpi.metric_key, "custom_metric");
    assert_eq!(
        custom_kpi.origin, "user",
        "absent origin defaults to user (the UI create path)"
    );
}

// ---------------------------------------------------------------------------
// `kpi_definitions.origin` enum guard (ADR 0093 decision 4, epic #285 T9):
// `seed | user | agent` — `seed` is migration-backfill-only, never settable
// by a live `create_kpi_definition` writer.
// ---------------------------------------------------------------------------

fn new_kpi_definition_with_origin(
    company_id: &str,
    metric_key: &str,
    origin: Option<&str>,
) -> NewKpiDefinition {
    NewKpiDefinition {
        scope: "company".to_owned(),
        company_id: Some(company_id.to_owned()),
        sector: None,
        metric_key: metric_key.to_owned(),
        label: metric_key.to_owned(),
        value_kind: "count".to_owned(),
        unit: None,
        computation: "reported".to_owned(),
        formula: None,
        display_format: None,
        origin: origin.map(str::to_owned),
        statement_group: None,
        period_nature: None,
    }
}

#[test]
fn create_kpi_definition_accepts_the_agent_origin_token() {
    let connection = open_in_memory_database().expect("database should initialize");
    let state = AppState::new(connection);
    let company = tracked_company(&state);

    let definition = state
        .create_kpi_definition(new_kpi_definition_with_origin(
            &company.id,
            "broker_client_count",
            Some("agent"),
        ))
        .expect("agent origin is a valid token");
    assert_eq!(definition.origin, "agent");
}

#[test]
fn create_kpi_definition_rejects_the_seed_origin_token() {
    // `seed` is migration-backfill-only (ADR 0093 decision 4) — no live
    // writer may mint a fake seed row.
    let connection = open_in_memory_database().expect("database should initialize");
    let state = AppState::new(connection);
    let company = tracked_company(&state);

    let error = state
        .create_kpi_definition(new_kpi_definition_with_origin(
            &company.id,
            "fake_seed_metric",
            Some("seed"),
        ))
        .expect_err("seed must never be settable by a live writer");
    assert!(
        matches!(
            error,
            StorageError::InvalidFinancialsValue {
                key: "origin",
                ref value
            } if value == "seed"
        ),
        "expected a typed invalid-origin error, got {error:?}"
    );
}

#[test]
fn create_kpi_definition_rejects_an_unknown_origin_token() {
    let connection = open_in_memory_database().expect("database should initialize");
    let state = AppState::new(connection);
    let company = tracked_company(&state);

    let error = state
        .create_kpi_definition(new_kpi_definition_with_origin(
            &company.id,
            "garbage_origin_metric",
            Some("garbage"),
        ))
        .expect_err("an unknown origin token must never be silently stored");
    assert!(matches!(
        error,
        StorageError::InvalidFinancialsValue { key: "origin", .. }
    ));
}

// ---------------------------------------------------------------------------
// `kpi_definitions.statement_group` vocabulary guard (card #307): `income |
// balance | cash_flow | per_share | other` — validated the same way as
// `origin`, absent/empty normalizing to the `other` default.
// ---------------------------------------------------------------------------

fn new_kpi_definition_with_statement_group(
    company_id: &str,
    metric_key: &str,
    statement_group: Option<&str>,
) -> NewKpiDefinition {
    NewKpiDefinition {
        scope: "company".to_owned(),
        company_id: Some(company_id.to_owned()),
        sector: None,
        metric_key: metric_key.to_owned(),
        label: metric_key.to_owned(),
        value_kind: "count".to_owned(),
        unit: None,
        computation: "reported".to_owned(),
        formula: None,
        display_format: None,
        origin: None,
        statement_group: statement_group.map(str::to_owned),
        period_nature: None,
    }
}

#[test]
fn create_kpi_definition_accepts_every_statement_group_token() {
    let connection = open_in_memory_database().expect("database should initialize");
    let state = AppState::new(connection);
    let company = tracked_company(&state);

    for (index, token) in ["income", "balance", "cash_flow", "per_share", "other"]
        .into_iter()
        .enumerate()
    {
        let definition = state
            .create_kpi_definition(new_kpi_definition_with_statement_group(
                &company.id,
                &format!("statement_group_metric_{index}"),
                Some(token),
            ))
            .expect("a vocabulary token is a valid statement_group");
        assert_eq!(definition.statement_group, token);
    }
}

#[test]
fn create_kpi_definition_defaults_absent_statement_group_to_other() {
    let connection = open_in_memory_database().expect("database should initialize");
    let state = AppState::new(connection);
    let company = tracked_company(&state);

    let definition = state
        .create_kpi_definition(new_kpi_definition_with_statement_group(
            &company.id,
            "no_group_metric",
            None,
        ))
        .expect("absent statement_group defaults");
    assert_eq!(definition.statement_group, "other");
}

#[test]
fn create_kpi_definition_rejects_an_unknown_statement_group_token() {
    let connection = open_in_memory_database().expect("database should initialize");
    let state = AppState::new(connection);
    let company = tracked_company(&state);

    let error = state
        .create_kpi_definition(new_kpi_definition_with_statement_group(
            &company.id,
            "garbage_group_metric",
            Some("garbage"),
        ))
        .expect_err("an unknown statement_group token must never be silently stored");
    assert!(matches!(
        error,
        StorageError::InvalidFinancialsValue {
            key: "statement_group",
            ..
        }
    ));
}

// ---------------------------------------------------------------------------
// `kpi_definitions.period_nature` vocabulary guard (ADR 0100 decision 6,
// epic #398): `instant | duration` — validated the same way as
// `statement_group`, absent/empty normalizing to the `duration` default (the
// pre-existing `is_ttm_eligible`/`measure_window_for` no-definition
// fallback, `fundamentals::metrics`).
// ---------------------------------------------------------------------------

fn new_kpi_definition_with_period_nature(
    company_id: &str,
    metric_key: &str,
    period_nature: Option<&str>,
) -> NewKpiDefinition {
    NewKpiDefinition {
        scope: "company".to_owned(),
        company_id: Some(company_id.to_owned()),
        sector: None,
        metric_key: metric_key.to_owned(),
        label: metric_key.to_owned(),
        value_kind: "count".to_owned(),
        unit: None,
        computation: "reported".to_owned(),
        formula: None,
        display_format: None,
        origin: None,
        statement_group: None,
        period_nature: period_nature.map(str::to_owned),
    }
}

#[test]
fn create_kpi_definition_accepts_every_period_nature_token() {
    let connection = open_in_memory_database().expect("database should initialize");
    let state = AppState::new(connection);
    let company = tracked_company(&state);

    for (index, token) in ["instant", "duration"].into_iter().enumerate() {
        let definition = state
            .create_kpi_definition(new_kpi_definition_with_period_nature(
                &company.id,
                &format!("period_nature_metric_{index}"),
                Some(token),
            ))
            .expect("a vocabulary token is a valid period_nature");
        assert_eq!(definition.period_nature, token);
    }
}

#[test]
fn create_kpi_definition_defaults_absent_period_nature_to_duration() {
    let connection = open_in_memory_database().expect("database should initialize");
    let state = AppState::new(connection);
    let company = tracked_company(&state);

    let definition = state
        .create_kpi_definition(new_kpi_definition_with_period_nature(
            &company.id,
            "no_nature_metric",
            None,
        ))
        .expect("absent period_nature defaults");
    assert_eq!(definition.period_nature, "duration");
}

#[test]
fn create_kpi_definition_rejects_an_unknown_period_nature_token() {
    let connection = open_in_memory_database().expect("database should initialize");
    let state = AppState::new(connection);
    let company = tracked_company(&state);

    let error = state
        .create_kpi_definition(new_kpi_definition_with_period_nature(
            &company.id,
            "garbage_nature_metric",
            Some("garbage"),
        ))
        .expect_err("an unknown period_nature token must never be silently stored");
    assert!(matches!(
        error,
        StorageError::InvalidFinancialsValue {
            key: "period_nature",
            ..
        }
    ));
}

// ---------------------------------------------------------------------------
// kpi_definitions id scoping (issue #149, T7 slice 2). The unique INDEX is
// (metric_key, scope, company_id, sector) — a metric key legitimately exists
// once per scope bucket. The PRIMARY KEY must therefore carry the same
// discriminator, otherwise a company-scoped definition whose reported measure
// happens to share a generic catalog key collides with the canonical row.
// Canonical ids stay bare (`kpidef_<key>`) — everything references them.
// ---------------------------------------------------------------------------

#[test]
fn company_scoped_definition_coexists_with_canonical_same_metric_key() {
    let connection = open_in_memory_database().expect("database should initialize");
    let state = AppState::new(connection);
    let company = tracked_company(&state);

    let canonical = state
        .list_kpi_definitions(ListKpiDefinitionsInput {
            scope: Some("canonical".to_owned()),
            sector: None,
            company_id: None,
        })
        .expect("canonical definitions should list")
        .into_iter()
        .find(|d| d.metric_key == "revenue")
        .expect("canonical revenue should be seeded");
    assert_eq!(canonical.id, "kpidef_revenue");

    // The company reports a measure it CALLS revenue but which is not the
    // generic concept (ADR 0077 d.8 no-repaint rule): tracked company-scoped.
    let scoped = state
        .create_kpi_definition(NewKpiDefinition {
            scope: "company".to_owned(),
            company_id: Some(company.id.clone()),
            sector: None,
            metric_key: "revenue".to_owned(),
            label: "Revenue (as the issuer defines it)".to_owned(),
            value_kind: "monetary".to_owned(),
            unit: None,
            computation: "reported".to_owned(),
            formula: None,
            display_format: None,
            origin: None,
            statement_group: None,
            period_nature: None,
        })
        .expect("a company-scoped definition must not collide with the canonical row");

    assert_ne!(scoped.id, canonical.id);
    assert_eq!(scoped.scope, "company");
    assert_eq!(scoped.company_id, Some(company.id.clone()));
    assert_eq!(scoped.metric_key, "revenue");

    // The canonical row is untouched — nothing was upserted onto it.
    let canonical_after = state
        .list_kpi_definitions(ListKpiDefinitionsInput {
            scope: Some("canonical".to_owned()),
            sector: None,
            company_id: None,
        })
        .expect("canonical definitions should list")
        .into_iter()
        .find(|d| d.id == canonical.id)
        .expect("canonical definition should still resolve");
    assert_eq!(canonical_after.scope, "canonical");
    assert_eq!(canonical_after.company_id, None);
    assert_eq!(canonical_after.label, canonical.label);

    // Both are visible to the company's catalog view (CustomKpiManager flow).
    let visible = state
        .list_kpi_definitions(ListKpiDefinitionsInput {
            scope: None,
            sector: None,
            company_id: Some(company.id.clone()),
        })
        .expect("company catalog should list");
    let revenue_rows: Vec<_> = visible
        .iter()
        .filter(|d| d.metric_key == "revenue")
        .collect();
    assert_eq!(
        revenue_rows.len(),
        2,
        "canonical + company-scoped revenue must coexist, got {revenue_rows:?}"
    );
}

#[test]
fn sector_scoped_definition_coexists_with_canonical_same_metric_key() {
    let connection = open_in_memory_database().expect("database should initialize");
    let state = AppState::new(connection);

    let scoped = state
        .create_kpi_definition(NewKpiDefinition {
            scope: "sector".to_owned(),
            company_id: None,
            sector: Some("banking".to_owned()),
            metric_key: "revenue".to_owned(),
            label: "Revenue (banking presentation)".to_owned(),
            value_kind: "monetary".to_owned(),
            unit: None,
            computation: "reported".to_owned(),
            formula: None,
            display_format: None,
            origin: None,
            statement_group: None,
            period_nature: None,
        })
        .expect("a sector-scoped definition must not collide with the canonical row");

    assert_ne!(scoped.id, "kpidef_revenue");
    assert_eq!(scoped.sector, Some("banking".to_owned()));
}

#[test]
fn two_companies_may_each_scope_the_same_metric_key() {
    let connection = open_in_memory_database().expect("database should initialize");
    let state = AppState::new(connection);
    let first = tracked_company(&state);
    let second = state
        .create_company(NewCompany {
            exchange: "GPW".to_owned(),
            ticker: "SCND".to_owned(),
            display_name: "Second Co.".to_owned(),
            isin: None,
            cik: None,
            lei: None,
        })
        .expect("second company should create");

    let make = |company_id: &str| NewKpiDefinition {
        scope: "company".to_owned(),
        company_id: Some(company_id.to_owned()),
        sector: None,
        metric_key: "backlog".to_owned(),
        label: "Backlog".to_owned(),
        value_kind: "monetary".to_owned(),
        unit: None,
        computation: "reported".to_owned(),
        formula: None,
        display_format: None,
        origin: None,
        statement_group: None,
        period_nature: None,
    };

    let a = state
        .create_kpi_definition(make(&first.id))
        .expect("first company-scoped definition should create");
    let b = state
        .create_kpi_definition(make(&second.id))
        .expect("second company-scoped definition must not collide with the first");

    assert_ne!(a.id, b.id);
}

#[test]
fn company_scoped_facts_do_not_leak_into_canonical_metric_history() {
    let connection = open_in_memory_database().expect("database should initialize");
    let state = AppState::new(connection);
    let company = tracked_company(&state);

    let scoped = state
        .create_kpi_definition(NewKpiDefinition {
            scope: "company".to_owned(),
            company_id: Some(company.id.clone()),
            sector: None,
            metric_key: "revenue".to_owned(),
            label: "Revenue (issuer definition)".to_owned(),
            value_kind: "monetary".to_owned(),
            unit: None,
            computation: "reported".to_owned(),
            formula: None,
            display_format: None,
            origin: None,
            statement_group: None,
            period_nature: None,
        })
        .expect("company-scoped definition should create");

    let period = state
        .create_financial_period(NewFinancialPeriod {
            company_id: company.id.clone(),
            fiscal_year: 2024,
            period_type: "FY".to_owned(),
            period_end_date: Some("2024-12-31".to_owned()),
            report_evidence_ref: None,
        })
        .expect("period should create");

    state
        .create_financial_fact(NewFinancialFact {
            company_id: company.id.clone(),
            period_id: period.id.clone(),
            definition_id: scoped.id.clone(),
            value_numeric: "42".to_owned(),
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
            annotation: None,
        })
        .expect("company-scoped fact should create");

    // The canonical `revenue` history is keyed on the canonical definition id,
    // so the issuer's differently-defined measure never contaminates it.
    let history = state
        .financials()
        .metric_history(&company.id, "revenue", 2999, "FY")
        .expect("metric history should read");
    assert!(
        history.is_empty(),
        "a company-scoped measure must not be folded into the canonical key, got {history:?}"
    );

    // But the fact still bridges to its own metric_key for the fact matrix.
    let facts = state
        .list_financial_facts(ListFinancialFactsInput {
            company_id: Some(company.id.clone()),
            period_id: None,
            definition_id: Some(scoped.id.clone()),
        })
        .expect("facts should list");
    assert_eq!(facts.len(), 1);
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
fn curating_a_core_seeded_metric_restates_the_profile_instead_of_failing() {
    // Company creation seeds the core set (issue #203), so the five core
    // metrics are already relevant on every company. Curating one of them —
    // the `create_kpi_relevance` command / MCP act — must restate that row, not
    // hard-fail on the UNIQUE(company_id, definition_id) constraint.
    let connection = open_in_memory_database().expect("database should initialize");
    let state = AppState::new(connection);
    let company = tracked_company(&state);

    let seeded = state
        .list_kpi_relevance(&company.id)
        .expect("relevance should list")
        .into_iter()
        .find(|r| r.definition_id == "kpidef_revenue")
        .expect("revenue is core-seeded at creation");
    assert_eq!(seeded.source, "core");

    let curated = state
        .create_kpi_relevance(NewKpiRelevance {
            company_id: company.id.clone(),
            definition_id: "kpidef_revenue".to_owned(),
            source: "user".to_owned(),
            rank: Some("primary".to_owned()),
            first_seen_period: Some("2026-Q1".to_owned()),
            last_seen_period: None,
        })
        .expect("curating a core-seeded metric must not fail");

    assert_eq!(curated.source, "user", "curation overwrites the app seed");
    assert_eq!(curated.first_seen_period, Some("2026-Q1".to_owned()));
    assert_eq!(
        state
            .list_kpi_relevance(&company.id)
            .expect("relevance should list")
            .len(),
        5,
        "curation restates a row, it never duplicates one"
    );
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
            annotation: None,
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
            annotation: None,
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
            annotation: None,
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

    // An agent reading its own writes back must not have to reverse-engineer
    // the metric from the definition id — the list carries metricKey directly
    // (epic #285 surface bug).
    let net_profit_fact = facts
        .iter()
        .find(|f| f.definition_id == net_profit_def.id)
        .expect("net_profit fact should be present");
    assert_eq!(net_profit_fact.metric_key, net_profit_def.metric_key);
    let revenue_fact = facts
        .iter()
        .find(|f| f.definition_id == revenue_def.id)
        .expect("revenue fact should be present");
    assert_eq!(revenue_fact.metric_key, revenue_def.metric_key);
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
            // ADR 0093 decision 2 canonical vocabulary (`estimate` synonym
            // migrated to `estimated`).
            data_quality: Some("estimated".to_owned()),
            as_reported_value: None,
            as_reported_scale: None,
            reporting_standard: None,
            extraction_method: None,
            confidence: None,
            confirmation_state: None,
            supersedes_id: None,
            source_document_ref: None,
            annotation: None,
        })
        .expect("fact should create");

    let updated = state
        .update_financial_fact(UpdateFinancialFact {
            id: fact.id.clone(),
            value_numeric: Some("550000".to_owned()),
            currency: Some("EUR".to_owned()),
            // `data_quality` is a slot dimension the update path never edits in
            // place (§4, ADR 0093) — resending the fact's own quality is a
            // no-op, exercised together with the rest of the field update.
            data_quality: Some("estimated".to_owned()),
            confirmation_state: Some("provisional".to_owned()),
            supersedes_id: None,
            source_document_ref: Some("revised_report.pdf".to_owned()),
            annotation: None,
        })
        .expect("fact should update");

    assert_eq!(updated.value_numeric, "550000");
    assert_eq!(updated.data_quality, "estimated");
    assert_eq!(updated.confirmation_state, "provisional");
    assert_eq!(
        updated.source_document_ref,
        Some("revised_report.pdf".to_owned())
    );
}

// ---------------------------------------------------------------------------
// Currency integrity (#93): a stored currency is NULL or a 3-letter ISO-4217
// code, normalized to uppercase at the write boundary. The ESEF divide-unit bug
// wrote "shares" onto every EPS fact; the guard makes that class unstorable.
// ---------------------------------------------------------------------------

fn canonical_definition_id(state: &AppState, metric_key: &str) -> String {
    state
        .list_kpi_definitions(ListKpiDefinitionsInput {
            scope: Some("canonical".to_owned()),
            sector: None,
            company_id: None,
        })
        .expect("canonical definitions should list")
        .iter()
        .find(|d| d.metric_key == metric_key)
        .unwrap_or_else(|| panic!("{metric_key} should exist in the canonical catalog"))
        .id
        .clone()
}

/// Seeds a company + FY period and returns `(company_id, period_id, cash
/// definition id)` for the currency-guard tests.
fn currency_guard_slot(state: &AppState) -> (String, String, String) {
    let company = tracked_company(state);
    let period = state
        .create_financial_period(NewFinancialPeriod {
            company_id: company.id.clone(),
            fiscal_year: 2026,
            period_type: "FY".to_owned(),
            period_end_date: None,
            report_evidence_ref: None,
        })
        .expect("financial period should create");
    let definition_id = canonical_definition_id(state, "cash");
    (company.id, period.id, definition_id)
}

fn new_fact_with_currency(
    company_id: &str,
    period_id: &str,
    definition_id: &str,
    currency: Option<&str>,
) -> NewFinancialFact {
    NewFinancialFact {
        company_id: company_id.to_owned(),
        period_id: period_id.to_owned(),
        definition_id: definition_id.to_owned(),
        value_numeric: "500000".to_owned(),
        currency: currency.map(str::to_owned),
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
        annotation: None,
    }
}

#[test]
fn create_financial_fact_rejects_a_non_iso_currency() {
    let connection = open_in_memory_database().expect("database should initialize");
    let state = AppState::new(connection);
    let (company_id, period_id, definition_id) = currency_guard_slot(&state);

    let error = state
        .create_financial_fact(new_fact_with_currency(
            &company_id,
            &period_id,
            &definition_id,
            Some("shares"),
        ))
        .expect_err("a unit that is not a currency must never be stored");

    assert!(
        matches!(
            error,
            StorageError::InvalidFinancialsValue {
                key: "currency",
                ref value
            } if value == "shares"
        ),
        "expected a typed invalid-currency error, got {error:?}"
    );
}

/// ADR 0095 (sol review, single-chokepoint completion): the plain create
/// path passes caller `extraction_method` through verbatim — including from
/// the MCP act — so the retired positional marker must be refused HERE, not
/// only in the structured pipeline.
#[test]
fn create_financial_fact_refuses_the_retired_positional_method() {
    let connection = open_in_memory_database().expect("database should initialize");
    let state = AppState::new(connection);
    let (company_id, period_id, definition_id) = currency_guard_slot(&state);

    let mut input = new_fact_with_currency(&company_id, &period_id, &definition_id, Some("PLN"));
    input.extraction_method = Some("html_positional".to_owned());
    let error = state
        .create_financial_fact(input)
        .expect_err("the retired html_positional method must never be stored on a new fact");

    assert!(
        matches!(error, StorageError::RetiredExtractionMethod),
        "expected the typed retired-method error, got {error:?}"
    );
}

#[test]
fn create_financial_fact_normalizes_currency_case() {
    let connection = open_in_memory_database().expect("database should initialize");
    let state = AppState::new(connection);
    let (company_id, period_id, definition_id) = currency_guard_slot(&state);

    let fact = state
        .create_financial_fact(new_fact_with_currency(
            &company_id,
            &period_id,
            &definition_id,
            Some(" pln "),
        ))
        .expect("a lowercase ISO code is normalized, not rejected");

    assert_eq!(fact.currency.as_deref(), Some("PLN"));
}

#[test]
fn create_financial_fact_allows_absent_currency() {
    let connection = open_in_memory_database().expect("database should initialize");
    let state = AppState::new(connection);
    let (company_id, period_id, definition_id) = currency_guard_slot(&state);

    let none = state
        .create_financial_fact(new_fact_with_currency(
            &company_id,
            &period_id,
            &definition_id,
            None,
        ))
        .expect("a unit-less ratio fact keeps a NULL currency");
    assert_eq!(none.currency, None);

    // An empty string is the same absence, not a validation failure.
    let revenue_id = canonical_definition_id(&state, "revenue");
    let empty = state
        .create_financial_fact(new_fact_with_currency(
            &company_id,
            &period_id,
            &revenue_id,
            Some("   "),
        ))
        .expect("an empty currency is absence");
    assert_eq!(empty.currency, None);
}

#[test]
fn update_financial_fact_rejects_a_non_iso_currency_and_normalizes_case() {
    let connection = open_in_memory_database().expect("database should initialize");
    let state = AppState::new(connection);
    let (company_id, period_id, definition_id) = currency_guard_slot(&state);
    let fact = state
        .create_financial_fact(new_fact_with_currency(
            &company_id,
            &period_id,
            &definition_id,
            Some("PLN"),
        ))
        .expect("fact should create");

    let error = state
        .update_financial_fact(UpdateFinancialFact {
            id: fact.id.clone(),
            value_numeric: None,
            currency: Some("shares".to_owned()),
            data_quality: None,
            confirmation_state: None,
            supersedes_id: None,
            source_document_ref: None,
            annotation: None,
        })
        .expect_err("an update must not smuggle in a non-currency unit");
    assert!(
        matches!(
            error,
            StorageError::InvalidFinancialsValue {
                key: "currency",
                ref value
            } if value == "shares"
        ),
        "expected a typed invalid-currency error, got {error:?}"
    );

    let updated = state
        .update_financial_fact(UpdateFinancialFact {
            id: fact.id.clone(),
            value_numeric: None,
            currency: Some("eur".to_owned()),
            data_quality: None,
            confirmation_state: None,
            supersedes_id: None,
            source_document_ref: None,
            annotation: None,
        })
        .expect("a lowercase ISO code is normalized");
    assert_eq!(updated.currency.as_deref(), Some("EUR"));
}

// ---------------------------------------------------------------------------
// Data quality (ADR 0093 decision 2): `final | preliminary | estimated`
// canonical vocabulary, normalized at the write boundary
// (`normalize_data_quality`, the `normalize_currency` pattern); preliminary
// and final coexist in the slot (`data_quality` is a uniqueness-slot
// dimension); a `final` fact created into a slot whose sibling is
// `preliminary`/`estimated` stamps `supersedes_id` at it.
// ---------------------------------------------------------------------------

fn new_fact_with_quality(
    company_id: &str,
    period_id: &str,
    definition_id: &str,
    value_numeric: &str,
    data_quality: Option<&str>,
) -> NewFinancialFact {
    NewFinancialFact {
        company_id: company_id.to_owned(),
        period_id: period_id.to_owned(),
        definition_id: definition_id.to_owned(),
        value_numeric: value_numeric.to_owned(),
        currency: Some("PLN".to_owned()),
        statement_basis: None,
        attribution: None,
        variant: None,
        measure_window: None,
        data_quality: data_quality.map(str::to_owned),
        as_reported_value: None,
        as_reported_scale: None,
        reporting_standard: None,
        extraction_method: None,
        confidence: None,
        confirmation_state: None,
        supersedes_id: None,
        source_document_ref: None,
        annotation: None,
    }
}

#[test]
fn create_financial_fact_defaults_absent_data_quality_to_final() {
    let connection = open_in_memory_database().expect("database should initialize");
    let state = AppState::new(connection);
    let (company_id, period_id, definition_id) = currency_guard_slot(&state);

    let fact = state
        .create_financial_fact(new_fact_with_quality(
            &company_id,
            &period_id,
            &definition_id,
            "500000",
            None,
        ))
        .expect("fact should create");
    assert_eq!(fact.data_quality, "final");
}

#[test]
fn create_financial_fact_normalizes_the_estimate_synonym() {
    let connection = open_in_memory_database().expect("database should initialize");
    let state = AppState::new(connection);
    let (company_id, period_id, definition_id) = currency_guard_slot(&state);

    let fact = state
        .create_financial_fact(new_fact_with_quality(
            &company_id,
            &period_id,
            &definition_id,
            "500000",
            Some("estimate"),
        ))
        .expect("the 'estimate' synonym should normalize, not reject");
    assert_eq!(fact.data_quality, "estimated");
}

#[test]
fn create_financial_fact_rejects_an_unknown_data_quality_token() {
    let connection = open_in_memory_database().expect("database should initialize");
    let state = AppState::new(connection);
    let (company_id, period_id, definition_id) = currency_guard_slot(&state);

    let error = state
        .create_financial_fact(new_fact_with_quality(
            &company_id,
            &period_id,
            &definition_id,
            "500000",
            Some("garbage"),
        ))
        .expect_err("an unknown data_quality token must never be silently slotted");
    assert!(
        matches!(
            error,
            StorageError::InvalidFinancialsValue {
                key: "data_quality",
                ref value
            } if value == "garbage"
        ),
        "expected a typed invalid-data_quality error, got {error:?}"
    );
}

// ---------------------------------------------------------------------------
// `attribution` slot-dimension enum guard (ADR 0093 epic #285 T9): the
// FactCitation gate defect closure means `attribution` is validated the same
// way `data_quality`/`currency` are — never a free-text citation carrier.
// ---------------------------------------------------------------------------

fn new_fact_with_attribution(
    company_id: &str,
    period_id: &str,
    definition_id: &str,
    value_numeric: &str,
    attribution: Option<&str>,
) -> NewFinancialFact {
    NewFinancialFact {
        company_id: company_id.to_owned(),
        period_id: period_id.to_owned(),
        definition_id: definition_id.to_owned(),
        value_numeric: value_numeric.to_owned(),
        currency: Some("PLN".to_owned()),
        statement_basis: None,
        attribution: attribution.map(str::to_owned),
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
        annotation: None,
    }
}

#[test]
fn create_financial_fact_defaults_absent_attribution_to_total() {
    let connection = open_in_memory_database().expect("database should initialize");
    let state = AppState::new(connection);
    let (company_id, period_id, definition_id) = currency_guard_slot(&state);

    let fact = state
        .create_financial_fact(new_fact_with_attribution(
            &company_id,
            &period_id,
            &definition_id,
            "500000",
            None,
        ))
        .expect("fact should create");
    assert_eq!(fact.attribution, "total");
}

#[test]
fn create_financial_fact_accepts_the_owners_of_parent_and_nci_slot_tokens() {
    let connection = open_in_memory_database().expect("database should initialize");
    let state = AppState::new(connection);
    let (company_id, period_id, definition_id) = currency_guard_slot(&state);

    let owners = state
        .create_financial_fact(new_fact_with_attribution(
            &company_id,
            &period_id,
            &definition_id,
            "500000",
            Some("owners_of_parent"),
        ))
        .expect("owners_of_parent is a valid slot dimension");
    assert_eq!(owners.attribution, "owners_of_parent");

    let nci = state
        .create_financial_fact(new_fact_with_attribution(
            &company_id,
            &period_id,
            &definition_id,
            "500000",
            Some("nci"),
        ))
        .expect("nci is a valid slot dimension");
    assert_eq!(nci.attribution, "nci");
}

#[test]
fn create_financial_fact_rejects_citation_prose_masquerading_as_attribution() {
    // The exact defect ADR 0093 epic #285 T9 closes: an agent (or any caller)
    // putting a citation string in `attribution` must never silently mint a
    // phantom uniqueness slot — it is a typed refusal, like `data_quality`'s
    // unknown-token guard.
    let connection = open_in_memory_database().expect("database should initialize");
    let state = AppState::new(connection);
    let (company_id, period_id, definition_id) = currency_guard_slot(&state);

    let error = state
        .create_financial_fact(new_fact_with_attribution(
            &company_id,
            &period_id,
            &definition_id,
            "500000",
            Some("FY2024 report, p.42"),
        ))
        .expect_err("citation prose must never be silently slotted as attribution");
    assert!(
        matches!(
            error,
            StorageError::InvalidFinancialsValue {
                key: "attribution",
                ref value
            } if value == "fy2024 report, p.42"
        ),
        "expected a typed invalid-attribution error, got {error:?}"
    );
}

#[test]
fn preliminary_and_final_coexist_in_the_same_slot_via_plain_create() {
    let connection = open_in_memory_database().expect("database should initialize");
    let state = AppState::new(connection);
    let (company_id, period_id, definition_id) = currency_guard_slot(&state);

    let preliminary = state
        .create_financial_fact(new_fact_with_quality(
            &company_id,
            &period_id,
            &definition_id,
            "492200000",
            Some("preliminary"),
        ))
        .expect("preliminary fact should create");
    let final_fact = state
        .create_financial_fact(new_fact_with_quality(
            &company_id,
            &period_id,
            &definition_id,
            "495000000",
            Some("final"),
        ))
        .expect("final fact should create");

    assert_ne!(preliminary.id, final_fact.id, "distinct rows, same slot");
    let facts = state
        .list_financial_facts(ListFinancialFactsInput {
            company_id: Some(company_id.clone()),
            period_id: Some(period_id.clone()),
            definition_id: Some(definition_id.clone()),
        })
        .expect("facts should list");
    assert_eq!(facts.len(), 2, "both quality variants persist in the slot");
}

#[test]
fn create_financial_fact_stamps_supersedes_id_when_a_final_lands_next_to_a_preliminary() {
    let connection = open_in_memory_database().expect("database should initialize");
    let state = AppState::new(connection);
    let (company_id, period_id, definition_id) = currency_guard_slot(&state);

    let preliminary = state
        .create_financial_fact(new_fact_with_quality(
            &company_id,
            &period_id,
            &definition_id,
            "492200000",
            Some("preliminary"),
        ))
        .expect("preliminary fact should create");
    let final_fact = state
        .create_financial_fact(new_fact_with_quality(
            &company_id,
            &period_id,
            &definition_id,
            "495000000",
            Some("final"),
        ))
        .expect("final fact should create");

    assert_eq!(final_fact.supersedes_id, Some(preliminary.id));
}

#[test]
fn create_financial_fact_never_stamps_supersedes_id_when_no_sibling_exists() {
    let connection = open_in_memory_database().expect("database should initialize");
    let state = AppState::new(connection);
    let (company_id, period_id, definition_id) = currency_guard_slot(&state);

    let final_fact = state
        .create_financial_fact(new_fact_with_quality(
            &company_id,
            &period_id,
            &definition_id,
            "495000000",
            Some("final"),
        ))
        .expect("final fact should create");

    assert_eq!(final_fact.supersedes_id, None);
}

#[test]
fn update_financial_fact_rejects_a_data_quality_change() {
    let connection = open_in_memory_database().expect("database should initialize");
    let state = AppState::new(connection);
    let (company_id, period_id, definition_id) = currency_guard_slot(&state);

    let fact = state
        .create_financial_fact(new_fact_with_quality(
            &company_id,
            &period_id,
            &definition_id,
            "492200000",
            Some("preliminary"),
        ))
        .expect("preliminary fact should create");

    let error = state
        .update_financial_fact(UpdateFinancialFact {
            id: fact.id.clone(),
            value_numeric: None,
            currency: None,
            data_quality: Some("final".to_owned()),
            confirmation_state: None,
            supersedes_id: None,
            source_document_ref: None,
            annotation: None,
        })
        .expect_err(
            "data_quality is a slot dimension: update must never silently re-slot or raise a raw UNIQUE",
        );
    assert!(
        matches!(
            error,
            StorageError::FinancialFactDataQualityLocked {
                ref id,
                ref current,
                ref requested,
            } if *id == fact.id && current == "preliminary" && requested == "final"
        ),
        "expected a typed data_quality-locked error, got {error:?}"
    );

    // The fact is untouched by the rejected update.
    let unchanged = state
        .list_financial_facts(ListFinancialFactsInput {
            company_id: Some(company_id.clone()),
            period_id: Some(period_id.clone()),
            definition_id: Some(definition_id.clone()),
        })
        .expect("facts should list")
        .into_iter()
        .find(|f| f.id == fact.id)
        .expect("fact should still exist");
    assert_eq!(unchanged.data_quality, "preliminary");
}

#[test]
fn update_financial_fact_rejects_a_data_quality_change_even_without_a_colliding_sibling() {
    // No sibling exists at all — but the ADR models the lifecycle as a NEW
    // final fact superseding a preliminary one, never an in-place edit, so the
    // rejection is unconditional, not just a collision guard.
    let connection = open_in_memory_database().expect("database should initialize");
    let state = AppState::new(connection);
    let (company_id, period_id, definition_id) = currency_guard_slot(&state);

    let fact = state
        .create_financial_fact(new_fact_with_quality(
            &company_id,
            &period_id,
            &definition_id,
            "492200000",
            Some("estimated"),
        ))
        .expect("estimated fact should create");

    let error = state
        .update_financial_fact(UpdateFinancialFact {
            id: fact.id.clone(),
            value_numeric: None,
            currency: None,
            data_quality: Some("preliminary".to_owned()),
            confirmation_state: None,
            supersedes_id: None,
            source_document_ref: None,
            annotation: None,
        })
        .expect_err("no colliding sibling still rejects the quality flip");
    assert!(matches!(
        error,
        StorageError::FinancialFactDataQualityLocked { .. }
    ));
}

#[test]
fn update_financial_fact_allows_a_data_quality_resend_that_normalizes_to_the_current_value() {
    let connection = open_in_memory_database().expect("database should initialize");
    let state = AppState::new(connection);
    let (company_id, period_id, definition_id) = currency_guard_slot(&state);

    let fact = state
        .create_financial_fact(new_fact_with_quality(
            &company_id,
            &period_id,
            &definition_id,
            "492200000",
            Some("estimate"),
        ))
        .expect("fact should create with the estimate synonym normalized");
    assert_eq!(fact.data_quality, "estimated");

    let updated = state
        .update_financial_fact(UpdateFinancialFact {
            id: fact.id.clone(),
            value_numeric: Some("493000000".to_owned()),
            currency: None,
            data_quality: Some("estimate".to_owned()),
            confirmation_state: None,
            supersedes_id: None,
            source_document_ref: None,
            annotation: None,
        })
        .expect(
            "resending the synonym for the fact's own normalized quality is a no-op, not a rejection",
        );
    assert_eq!(updated.data_quality, "estimated");
    assert_eq!(updated.value_numeric, "493000000");
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
            annotation: None,
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
    // Since T7 the same core set is also seeded at creation time (issue #203
    // residual), so both companies start WITH it — which makes re-applying
    // 0106's statement the exact convergence case this test exists to pin.
    let fresh = company_with_ticker(&state, "SEED");
    let curated = company_with_ticker(&state, "CUR");
    assert_eq!(
        state
            .list_kpi_relevance(&fresh.id)
            .expect("relevance should list")
            .len(),
        CORE_KPI_METRIC_KEYS.len()
    );

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

    // The whole core set, exactly once, however many times the seed runs.
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
                origin: None,
                statement_group: None,
                period_nature: None,
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

/// #156: the one-off annotation is stored on create (whitespace normalizes to
/// NULL), survives an unrelated update, is replaced by new text, and an empty
/// string on update clears it. Reddens if the keep/clear/replace contract or
/// the create normalization drifts.
#[test]
fn fact_annotation_set_kept_replaced_and_cleared() {
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
    let definitions = state
        .list_kpi_definitions(ListKpiDefinitionsInput {
            scope: Some("canonical".to_owned()),
            sector: None,
            company_id: None,
        })
        .expect("canonical definitions should list");
    let new_fact = |metric_key: &str, annotation: Option<&str>| NewFinancialFact {
        company_id: company.id.clone(),
        period_id: period.id.clone(),
        definition_id: definitions
            .iter()
            .find(|d| d.metric_key == metric_key)
            .expect("canonical metric")
            .id
            .clone(),
        value_numeric: "1000".to_owned(),
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
        annotation: annotation.map(str::to_owned),
    };
    let update = |id: &str, value: Option<&str>, annotation: Option<&str>| UpdateFinancialFact {
        id: id.to_owned(),
        value_numeric: value.map(str::to_owned),
        currency: None,
        data_quality: None,
        confirmation_state: None,
        supersedes_id: None,
        source_document_ref: None,
        annotation: annotation.map(str::to_owned),
    };

    let fact = state
        .create_financial_fact(new_fact("net_profit", Some("includes a one-off gain")))
        .expect("fact should create");
    assert_eq!(fact.annotation.as_deref(), Some("includes a one-off gain"));

    // Whitespace-only on create normalizes to NULL.
    let blank = state
        .create_financial_fact(new_fact("total_assets", Some("   ")))
        .expect("fact should create");
    assert_eq!(blank.annotation, None);

    // An unrelated update keeps the stored annotation.
    let kept = state
        .update_financial_fact(update(&fact.id, Some("2000"), None))
        .expect("update should apply");
    assert_eq!(kept.annotation.as_deref(), Some("includes a one-off gain"));

    // New text replaces it.
    let replaced = state
        .update_financial_fact(update(&fact.id, None, Some("revised note")))
        .expect("update should apply");
    assert_eq!(replaced.annotation.as_deref(), Some("revised note"));

    // An empty string clears it.
    let cleared = state
        .update_financial_fact(update(&fact.id, None, Some("")))
        .expect("update should apply");
    assert_eq!(cleared.annotation, None);
}

// ---------------------------------------------------------------------------
// ADR 0092 layers 2 and 3 (issues #273 / #274).
//
// Layer 2 (`source='sector'`) is a real gate contributor: it must reach
// `expected_primary_metric_keys` alongside the core floor.
// Layer 3 (`source='derived'`) must NEVER reach it — the completeness gate
// compares extraction output against expectations, so deriving the expectations
// FROM extraction output would let a systematic extraction hole silently erase
// the very expectation that would have caught it.
// ---------------------------------------------------------------------------

/// Give `company_id` an issuer-tier fact for `metric_key` in `period_id`.
fn seed_issuer_fact(
    state: &AppState,
    company_id: &str,
    period_id: &str,
    metric_key: &str,
    value: &str,
    tier: &str,
) {
    let fact = state
        .create_financial_fact(NewFinancialFact {
            company_id: company_id.to_owned(),
            period_id: period_id.to_owned(),
            definition_id: definition_id_for(state, metric_key),
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
            annotation: None,
        })
        .expect("fact should create");
    state
        .fundamentals_provenance()
        .set_fact_provenance(NewFactProvenance {
            fact_id: &fact.id,
            source_tier: tier,
            validation_status: "passed",
            drift_json: None,
            citation: None,
        })
        .expect("provenance should record");
}

fn relevance_rows(state: &AppState, company_id: &str) -> Vec<(String, String, String)> {
    let definitions = state
        .list_kpi_definitions(ListKpiDefinitionsInput {
            scope: None,
            sector: None,
            company_id: None,
        })
        .expect("definitions should list");
    let mut rows: Vec<(String, String, String)> = state
        .list_kpi_relevance(company_id)
        .expect("relevance should list")
        .into_iter()
        .map(|r| {
            let key = definitions
                .iter()
                .find(|d| d.id == r.definition_id)
                .map(|d| d.metric_key.clone())
                .unwrap_or_else(|| panic!("dangling definition_id {}", r.definition_id));
            (key, r.source, r.rank.unwrap_or_default())
        })
        .collect();
    rows.sort();
    rows
}

#[test]
fn expected_primary_metric_keys_includes_statement_pack_additions() {
    let connection = open_in_memory_database().expect("database should initialize");
    let state = AppState::new(connection);
    let company = company_with_ticker(&state, "PKO");

    {
        let connection = state
            .checkout_for_tests()
            .expect("connection should check out");
        connection
            .execute(
                "UPDATE companies SET statement_type = 'banking' WHERE id = ?1",
                [&company.id],
            )
            .expect("reclassify");
        crate::storage::financials::seed_statement_pack_kpi_relevance(&connection, &company.id)
            .expect("statement pack should seed");
    }

    // The gate reads any active+primary row regardless of `source`, so layer 2
    // widens the denominator for financial issuers.
    let expected = state
        .financials()
        .expected_primary_metric_keys(&company.id)
        .expect("expected keys should read")
        .expect("a seeded company has a denominator");
    let keys: Vec<&str> = expected.iter().map(String::as_str).collect();
    assert_eq!(
        keys,
        vec![
            "net_fee_commission_income",
            "net_interest_income",
            "net_profit",
            "operating_profit",
            "revenue",
            "total_assets",
            "total_deposits",
            "total_equity",
            "total_loans",
        ]
    );
}

#[test]
fn derived_pass_marks_keys_reported_in_at_least_three_of_the_last_four_periods() {
    let connection = open_in_memory_database().expect("database should initialize");
    let state = AppState::new(connection);
    let company = company_with_ticker(&state, "DRV");

    let periods: Vec<String> = (2022..=2025)
        .map(|year| seed_fy_period(&state, &company.id, year))
        .collect();

    // `ebitda`: 3 of the last 4 → marked.
    for period in periods.iter().take(3) {
        seed_issuer_fact(&state, &company.id, period, "ebitda", "100", "esef");
    }
    // `gross_profit`: only 2 of the last 4 → not marked.
    for period in periods.iter().take(2) {
        seed_issuer_fact(&state, &company.id, period, "gross_profit", "50", "esef");
    }
    // `capex`: 4 of the last 4 but ONLY from the aggregator — not issuer-tier,
    // so it never becomes a derived observation.
    for period in &periods {
        seed_issuer_fact(
            &state,
            &company.id,
            period,
            "capex",
            "10",
            "html_aggregator",
        );
    }

    {
        let connection = state
            .checkout_for_tests()
            .expect("connection should check out");
        crate::storage::financials::refresh_derived_kpi_relevance(&connection, &company.id)
            .expect("derived pass should run");
    }

    let derived: Vec<String> = relevance_rows(&state, &company.id)
        .into_iter()
        .filter(|(_, source, _)| source == "derived")
        .map(|(key, _, rank)| {
            assert_eq!(rank, "secondary", "derived rows are never ranked primary");
            key
        })
        .collect();
    assert_eq!(derived, vec!["ebitda".to_owned()]);
}

/// ADR 0093 decision 1: the agent tier is NOT an issuer tier — an agent's
/// figure is never evidence "the issuer reports this key" for the derived
/// completeness observation, exactly like `html_aggregator`.
#[test]
fn derived_pass_never_marks_a_key_reported_only_by_the_agent_tier() {
    let connection = open_in_memory_database().expect("database should initialize");
    let state = AppState::new(connection);
    let company = company_with_ticker(&state, "AGT");

    let periods: Vec<String> = (2022..=2025)
        .map(|year| seed_fy_period(&state, &company.id, year))
        .collect();
    // `net_deposits`: 4 of the last 4, but ONLY from the agent tier — not
    // issuer-tier, so it never becomes a derived observation.
    for period in &periods {
        seed_issuer_fact(&state, &company.id, period, "capex", "10", "agent");
    }

    {
        let connection = state
            .checkout_for_tests()
            .expect("connection should check out");
        crate::storage::financials::refresh_derived_kpi_relevance(&connection, &company.id)
            .expect("derived pass should run");
    }

    let derived: Vec<String> = relevance_rows(&state, &company.id)
        .into_iter()
        .filter(|(_, source, _)| source == "derived")
        .map(|(key, _, _)| key)
        .collect();
    assert!(
        derived.is_empty(),
        "an agent-only reported key must not enter the derived layer: {derived:?}"
    );
}

#[test]
fn derived_pass_never_touches_core_sector_or_user_rows() {
    let connection = open_in_memory_database().expect("database should initialize");
    let state = AppState::new(connection);
    let company = company_with_ticker(&state, "KEEP");

    let curated = state
        .create_kpi_relevance(NewKpiRelevance {
            company_id: company.id.clone(),
            definition_id: definition_id_for(&state, "revenue"),
            source: "user".to_owned(),
            rank: Some("primary".to_owned()),
            first_seen_period: None,
            last_seen_period: None,
        })
        .expect("curated row should create");

    // `revenue` (user) and `net_profit` (core) are both consistently reported,
    // so the pass would want to mark them — and must not.
    let periods: Vec<String> = (2022..=2025)
        .map(|year| seed_fy_period(&state, &company.id, year))
        .collect();
    for period in &periods {
        seed_issuer_fact(&state, &company.id, period, "revenue", "100", "esef");
        seed_issuer_fact(&state, &company.id, period, "net_profit", "10", "esef");
    }

    let before = relevance_rows(&state, &company.id);
    {
        let connection = state
            .checkout_for_tests()
            .expect("connection should check out");
        crate::storage::financials::refresh_derived_kpi_relevance(&connection, &company.id)
            .expect("derived pass should run");
        // Idempotence: the pass converges instead of accumulating.
        crate::storage::financials::refresh_derived_kpi_relevance(&connection, &company.id)
            .expect("derived pass should be idempotent");
    }
    assert_eq!(
        relevance_rows(&state, &company.id),
        before,
        "an occupied (company, definition) slot is never re-sourced or duplicated"
    );

    let survivor = state
        .list_kpi_relevance(&company.id)
        .expect("relevance should list")
        .into_iter()
        .find(|r| r.id == curated.id)
        .expect("the user row must survive");
    assert_eq!(survivor.source, "user");
    assert_eq!(survivor.rank, Some("primary".to_owned()));
}

#[test]
fn derived_rows_never_enter_the_completeness_denominator() {
    // ADR 0092's no-self-referential-gate rule. `rank='secondary'` already keeps
    // derived rows out incidentally; this pins the STRUCTURAL exclusion by
    // hand-upgrading one to `primary` — it must STILL not count.
    let connection = open_in_memory_database().expect("database should initialize");
    let state = AppState::new(connection);
    let company = company_with_ticker(&state, "GATE");

    let periods: Vec<String> = (2022..=2025)
        .map(|year| seed_fy_period(&state, &company.id, year))
        .collect();
    for period in &periods {
        seed_issuer_fact(&state, &company.id, period, "ebitda", "100", "esef");
    }
    {
        let connection = state
            .checkout_for_tests()
            .expect("connection should check out");
        crate::storage::financials::refresh_derived_kpi_relevance(&connection, &company.id)
            .expect("derived pass should run");
    }

    let derived_row = state
        .list_kpi_relevance(&company.id)
        .expect("relevance should list")
        .into_iter()
        .find(|r| r.source == "derived")
        .expect("ebitda should be a derived observation");
    state
        .update_kpi_relevance(UpdateKpiRelevance {
            id: derived_row.id.clone(),
            status: Some("active".to_owned()),
            rank: Some("primary".to_owned()),
            first_seen_period: None,
            last_seen_period: None,
        })
        .expect("hand-upgrade should apply");
    assert_eq!(
        state
            .list_kpi_relevance(&company.id)
            .expect("relevance should list")
            .into_iter()
            .find(|r| r.id == derived_row.id)
            .expect("row should still exist")
            .rank,
        Some("primary".to_owned()),
        "the upgrade must really be stored, or the guard proves nothing"
    );

    let expected = state
        .financials()
        .expected_primary_metric_keys(&company.id)
        .expect("expected keys should read")
        .expect("the core floor still supplies a denominator");
    assert!(
        !expected.contains("ebitda"),
        "a derived observation must never gate, whatever its rank — got {expected:?}"
    );
    let keys: Vec<&str> = expected.iter().map(String::as_str).collect();
    assert_eq!(
        keys,
        vec![
            "net_profit",
            "operating_profit",
            "revenue",
            "total_assets",
            "total_equity"
        ]
    );
}

// ---------------------------------------------------------------------------
// ADR 0093 T4 (epic #285): making the data_quality-blind readers
// preliminary-aware. One shared fixture, driven through every reader that
// must respect the `preliminary`/`final` coexistence ADR 0093 dec. 2
// introduced.
// ---------------------------------------------------------------------------

/// One company, one FY2025 period, three metrics covering every quality shape
/// the readers must handle:
/// - `total_assets` (A): a `preliminary` (agent tier, unreviewed) fact
///   followed by its `final` (esef tier, passed) sibling — the exact ADR 0093
///   dec. 2 coexistence shape, with a DIFFERENT value so a reader that picks
///   the wrong one is caught.
/// - `net_profit` (B): preliminary-only (agent tier) — never confirmed by an
///   issuer filing.
/// - `revenue` (C): final-only (esef tier) — the untouched control.
///
/// Every fact goes through `record_structured_fact` (not the bare
/// `create_financial_fact` `seed_fact` helper above) so each one carries a
/// real `financial_fact_provenance` row — the veto filter and the coverage
/// validation buckets both key off it.
struct QualityFixture {
    company_id: String,
}

fn seed_quality_fixture(state: &AppState) -> QualityFixture {
    let company = tracked_company(state);

    // A: preliminary first (chronologically realistic — the agent write
    // precedes the audited filing), then its final sibling with a DIFFERENT
    // value.
    state
        .kpi_extraction()
        .record_structured_fact(StructuredFactInput {
            company_id: &company.id,
            fiscal_year: 2025,
            period_type: "FY",
            period_end: Some("2025-12-31"),
            report_document_id: "doc-agent",
            metric_key: "total_assets",
            value_numeric: "39000000",
            currency: Some("PLN"),
            confirmation_state: "confirmed",
            source_tier: "agent",
            extraction_method: "mcp_agent",
            validation_status: "unreviewed",
            drift_json: None,
            citation: Some("XTB RB 18/2026"),
            attribution: None,
            measure_window: None,
            data_quality: Some("preliminary"),
        })
        .expect("preliminary total_assets should record");
    state
        .kpi_extraction()
        .record_structured_fact(StructuredFactInput {
            company_id: &company.id,
            fiscal_year: 2025,
            period_type: "FY",
            period_end: Some("2025-12-31"),
            report_document_id: "doc-esef",
            metric_key: "total_assets",
            value_numeric: "40000000",
            currency: Some("PLN"),
            confirmation_state: "confirmed",
            source_tier: "esef",
            extraction_method: "api",
            validation_status: "passed",
            drift_json: None,
            citation: Some("ifrs-full:Assets"),
            attribution: None,
            measure_window: None,
            data_quality: None, // normalizes to "final"
        })
        .expect("final total_assets should record");

    // B: preliminary-only.
    state
        .kpi_extraction()
        .record_structured_fact(StructuredFactInput {
            company_id: &company.id,
            fiscal_year: 2025,
            period_type: "FY",
            period_end: Some("2025-12-31"),
            report_document_id: "doc-agent",
            metric_key: "net_profit",
            value_numeric: "1200000",
            currency: Some("PLN"),
            confirmation_state: "confirmed",
            source_tier: "agent",
            extraction_method: "mcp_agent",
            validation_status: "unreviewed",
            drift_json: None,
            citation: Some("XTB RB 18/2026"),
            attribution: None,
            measure_window: None,
            data_quality: Some("preliminary"),
        })
        .expect("preliminary-only net_profit should record");

    // C: final-only.
    state
        .kpi_extraction()
        .record_structured_fact(StructuredFactInput {
            company_id: &company.id,
            fiscal_year: 2025,
            period_type: "FY",
            period_end: Some("2025-12-31"),
            report_document_id: "doc-esef",
            metric_key: "revenue",
            value_numeric: "9000000",
            currency: Some("PLN"),
            confirmation_state: "confirmed",
            source_tier: "esef",
            extraction_method: "api",
            validation_status: "passed",
            drift_json: None,
            citation: Some("ifrs-full:Revenue"),
            attribution: None,
            measure_window: None,
            data_quality: None,
        })
        .expect("final-only revenue should record");

    QualityFixture {
        company_id: company.id,
    }
}

/// #1: `facts_coverage_by_period` must count each SLOT once (dims minus
/// `data_quality`), not each ROW — a preliminary+final pair for `total_assets`
/// is ONE slot, not two, and the counted row's provenance bucket must be the
/// final-preferred one.
#[test]
fn facts_coverage_by_period_counts_each_slot_once_final_preferred() {
    let connection = open_in_memory_database().expect("database should initialize");
    let state = AppState::new(connection);
    let fixture = seed_quality_fixture(&state);

    let coverage = state
        .financials()
        .facts_coverage_by_period(&fixture.company_id)
        .expect("coverage should read");
    assert_eq!(coverage.len(), 1, "one period bucket, got {coverage:?}");
    let cell = &coverage[0];
    assert_eq!(cell.fiscal_year, 2025);
    assert_eq!(cell.period_type, "FY");
    assert_eq!(
        cell.total, 3,
        "3 SLOTS (A, B, C) counted once each, not the 4 underlying rows — got {cell:?}"
    );
    assert_eq!(
        cell.validated, 2,
        "A's FINAL (esef/passed) + C (esef/passed) — A's preliminary sibling must not \
         contribute its own count: {cell:?}"
    );
    assert_eq!(
        cell.unvalidated, 1,
        "only B (agent/unreviewed, preliminary-only) is unvalidated: {cell:?}"
    );
    assert_eq!(cell.flagged, 0, "{cell:?}");
}

/// #2 — THE REAL HAZARD: `stored_fact_set_for_cross_check` last-wins map
/// insertion let a preliminary row silently become the cross-check prior. An
/// incoming tier that does NOT outrank either of A's siblings (here:
/// `html_aggregator`, ranked below both `esef` and `agent`) lets BOTH the
/// preliminary and the final fact survive the tier veto filter — exactly the
/// scenario where final-preference inside the merge itself (not just the tier
/// filter) is load-bearing.
#[test]
fn stored_fact_set_for_cross_check_final_preferred_slot_once() {
    let connection = open_in_memory_database().expect("database should initialize");
    let state = AppState::new(connection);
    let fixture = seed_quality_fixture(&state);

    let prior = state
        .financials()
        .stored_fact_set_for_cross_check(
            &fixture.company_id,
            2025,
            "FY",
            SourceTier::HtmlAggregator,
        )
        .expect("cross-check prior should query")
        .expect("A, B, C all survive a veto filter with no incoming outranking");

    assert_eq!(
        prior.len(),
        3,
        "A, B, C — one value per metric slot, got {prior:?}"
    );
    assert_eq!(
        prior.get("total_assets"),
        Some(&Decimal::new(40_000_000, 0)),
        "A's FINAL value must win over its preliminary sibling in the cross-check prior — a \
         preliminary must never silently shadow a final value here"
    );
    assert_eq!(
        prior.get("net_profit"),
        Some(&Decimal::new(1_200_000, 0)),
        "B (preliminary-only) stays visible in the prior, counted once"
    );
    assert_eq!(
        prior.get("revenue"),
        Some(&Decimal::new(9_000_000, 0)),
        "C unchanged"
    );
}

/// The unfiltered `stored_fact_set` variant (only test harnesses read it
/// today, per its doc comment) exercises the same final-preferred merge with
/// no tier veto filter in play at all.
#[test]
fn stored_fact_set_unfiltered_final_preferred_slot_once() {
    let connection = open_in_memory_database().expect("database should initialize");
    let state = AppState::new(connection);
    let fixture = seed_quality_fixture(&state);

    let set = state
        .financials()
        .stored_fact_set(&fixture.company_id, 2025, "FY")
        .expect("stored_fact_set should query")
        .expect("a period with facts should yield Some");

    assert_eq!(set.len(), 3, "A, B, C — got {set:?}");
    assert_eq!(
        set.get("total_assets"),
        Some(&Decimal::new(40_000_000, 0)),
        "final must win"
    );
}

/// §2 safety property (load-bearing, ADR 0093 dec. 1): an incoming ESEF
/// (issuer) extraction outranks the agent tier, so EVERY agent-tier fact — A's
/// preliminary sibling AND B (preliminary-only) — must be excluded from the
/// veto-capable prior entirely. A tier the incoming outranks cannot veto: it
/// never even enters the comparison, so a wrong agent preliminary can never
/// block or contradict a correct issuer filing.
#[test]
fn stored_fact_set_for_cross_check_esef_never_vetoed_by_agent_preliminary() {
    let connection = open_in_memory_database().expect("database should initialize");
    let state = AppState::new(connection);
    let fixture = seed_quality_fixture(&state);

    let prior = state
        .financials()
        .stored_fact_set_for_cross_check(&fixture.company_id, 2025, "FY", SourceTier::Esef)
        .expect("cross-check prior should query")
        .expect("A and C's esef-tier facts still yield a prior");

    assert_eq!(
        prior.get("total_assets"),
        Some(&Decimal::new(40_000_000, 0)),
        "only A's esef-tier FINAL value is veto-capable against an incoming esef set"
    );
    assert_eq!(
        prior.get("revenue"),
        Some(&Decimal::new(9_000_000, 0)),
        "C is unaffected"
    );
    assert!(
        !prior.contains_key("net_profit"),
        "B is agent-tier-only: an incoming ESEF extraction outranks the agent tier (T2), so \
         the agent tier is not veto-capable here and B is excluded from the prior entirely — \
         it can never veto the incoming issuer set. Got {prior:?}"
    );
    assert_eq!(
        prior.len(),
        2,
        "only A and C survive the tier veto filter: {prior:?}"
    );
}

/// #3: `metric_history` must weight each PERIOD once, final-preferred — a
/// preliminary+final pair for the same period must not double-count into the
/// plausibility median.
#[test]
fn metric_history_final_preferred_one_value_per_period() {
    let connection = open_in_memory_database().expect("database should initialize");
    let state = AppState::new(connection);
    let fixture = seed_quality_fixture(&state);

    // A nonexistent fiscal year excludes nothing (the same trick
    // `company_scoped_facts_do_not_leak_into_canonical_metric_history` uses
    // above with `2999`), so FY2025's facts become "the history".
    let history = state
        .financials()
        .metric_history(&fixture.company_id, "total_assets", 2099, "FY")
        .expect("metric history should read");
    assert_eq!(
        history,
        vec![Decimal::new(40_000_000, 0)],
        "one value for the period (final wins), not two — got {history:?}"
    );

    let b_history = state
        .financials()
        .metric_history(&fixture.company_id, "net_profit", 2099, "FY")
        .expect("metric history should read");
    assert_eq!(
        b_history,
        vec![Decimal::new(1_200_000, 0)],
        "preliminary-only still counts once"
    );

    let c_history = state
        .financials()
        .metric_history(&fixture.company_id, "revenue", 2099, "FY")
        .expect("metric history should read");
    assert_eq!(c_history, vec![Decimal::new(9_000_000, 0)], "unchanged");
}

/// `metric_history_batch`'s (`metric_histories`) equivalence contract: same
/// values, same slot-once/final-preferred collapse as the per-metric read.
#[test]
fn metric_histories_batch_matches_single_read_final_preferred() {
    let connection = open_in_memory_database().expect("database should initialize");
    let state = AppState::new(connection);
    let fixture = seed_quality_fixture(&state);

    let keys: BTreeSet<String> = ["total_assets", "net_profit", "revenue"]
        .into_iter()
        .map(str::to_owned)
        .collect();
    let batch = state
        .financials()
        .metric_histories(&fixture.company_id, &keys, 2099, "FY")
        .expect("batched metric histories should read");

    for key in &keys {
        let single = state
            .financials()
            .metric_history(&fixture.company_id, key, 2099, "FY")
            .expect("single metric history should read");
        assert_eq!(
            batch.get(key),
            Some(&single),
            "batch must match the single read exactly for {key}"
        );
    }
    assert_eq!(
        batch.get("total_assets"),
        Some(&vec![Decimal::new(40_000_000, 0)]),
        "final wins, one value per period"
    );
}

// ---------------------------------------------------------------------------
// slot_metric_histories (#361): the slot-aware batched history the manifest
// builder's plausibility gate reads — never a plain metric_key collapse.
// ---------------------------------------------------------------------------

#[allow(clippy::too_many_arguments)]
fn seed_slot_fact(
    state: &AppState,
    company_id: &str,
    period_id: &str,
    definition_id: &str,
    statement_basis: &str,
    attribution: &str,
    measure_window: &str,
    data_quality: &str,
    value: &str,
) {
    state
        .create_financial_fact(NewFinancialFact {
            company_id: company_id.to_owned(),
            period_id: period_id.to_owned(),
            definition_id: definition_id.to_owned(),
            value_numeric: value.to_owned(),
            currency: Some("PLN".to_owned()),
            statement_basis: Some(statement_basis.to_owned()),
            attribution: Some(attribution.to_owned()),
            variant: None,
            measure_window: Some(measure_window.to_owned()),
            data_quality: Some(data_quality.to_owned()),
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
        .expect("financial fact should create");
}

#[test]
fn slot_metric_histories_never_mixes_attribution() {
    let connection = open_in_memory_database().expect("database should initialize");
    let state = AppState::new(connection);
    let company = tracked_company(&state);
    let definition_id = canonical_definition_id(&state, "total_equity");
    let p2023 = seed_fy_period(&state, &company.id, 2023);
    let p2024 = seed_fy_period(&state, &company.id, 2024);

    seed_slot_fact(
        &state,
        &company.id,
        &p2023,
        &definition_id,
        "consolidated",
        "total",
        "point_in_time",
        "final",
        "1000",
    );
    seed_slot_fact(
        &state,
        &company.id,
        &p2024,
        &definition_id,
        "consolidated",
        "owners_of_parent",
        "point_in_time",
        "final",
        "9999",
    );

    let total_key = HistorySlotKey {
        definition_id: definition_id.clone(),
        statement_basis: "consolidated".to_owned(),
        attribution_eff: "total".to_owned(),
        measure_window_eff: Some("point_in_time".to_owned()),
    };
    let parent_key = HistorySlotKey {
        definition_id,
        statement_basis: "consolidated".to_owned(),
        attribution_eff: "owners_of_parent".to_owned(),
        measure_window_eff: Some("point_in_time".to_owned()),
    };
    let slots: std::collections::BTreeSet<_> = [total_key.clone(), parent_key.clone()].into();
    let histories = state
        .financials()
        .slot_metric_histories(&company.id, &slots, 2099, "FY")
        .expect("slot histories should read");

    assert_eq!(
        histories.get(&total_key),
        Some(&vec![Decimal::new(1000, 0)])
    );
    assert_eq!(
        histories.get(&parent_key),
        Some(&vec![Decimal::new(9999, 0)])
    );
}

#[test]
fn slot_metric_histories_never_mixes_measure_window() {
    let connection = open_in_memory_database().expect("database should initialize");
    let state = AppState::new(connection);
    let company = tracked_company(&state);
    let definition_id = canonical_definition_id(&state, "revenue");
    let p2023 = seed_fy_period(&state, &company.id, 2023);
    let p2024 = seed_fy_period(&state, &company.id, 2024);

    seed_slot_fact(
        &state,
        &company.id,
        &p2023,
        &definition_id,
        "consolidated",
        "total",
        "flow",
        "final",
        "500",
    );
    seed_slot_fact(
        &state,
        &company.id,
        &p2024,
        &definition_id,
        "consolidated",
        "total",
        "trailing",
        "final",
        "7777",
    );

    let flow_key = HistorySlotKey {
        definition_id: definition_id.clone(),
        statement_basis: "consolidated".to_owned(),
        attribution_eff: "total".to_owned(),
        measure_window_eff: Some("flow".to_owned()),
    };
    let trailing_key = HistorySlotKey {
        definition_id,
        statement_basis: "consolidated".to_owned(),
        attribution_eff: "total".to_owned(),
        measure_window_eff: Some("trailing".to_owned()),
    };
    let slots: std::collections::BTreeSet<_> = [flow_key.clone(), trailing_key.clone()].into();
    let histories = state
        .financials()
        .slot_metric_histories(&company.id, &slots, 2099, "FY")
        .expect("slot histories should read");

    assert_eq!(histories.get(&flow_key), Some(&vec![Decimal::new(500, 0)]));
    assert_eq!(
        histories.get(&trailing_key),
        Some(&vec![Decimal::new(7777, 0)])
    );
}

#[test]
fn slot_metric_histories_company_scoped_definition_has_its_own_history() {
    let connection = open_in_memory_database().expect("database should initialize");
    let state = AppState::new(connection);
    let company = tracked_company(&state);
    let canonical_id = canonical_definition_id(&state, "revenue");
    let company_definition = state
        .financials()
        .create_kpi_definition(NewKpiDefinition {
            scope: "company".to_owned(),
            company_id: Some(company.id.clone()),
            sector: None,
            metric_key: "revenue".to_owned(),
            label: "Przychody segmentu".to_owned(),
            value_kind: "monetary".to_owned(),
            unit: None,
            computation: "reported".to_owned(),
            formula: None,
            display_format: None,
            origin: None,
            statement_group: None,
            period_nature: None,
        })
        .expect("company-scoped definition should create");
    let p2023 = seed_fy_period(&state, &company.id, 2023);

    // Only the company-scoped definition has history; the canonical one has none.
    seed_slot_fact(
        &state,
        &company.id,
        &p2023,
        &company_definition.id,
        "consolidated",
        "total",
        "flow",
        "final",
        "4242",
    );

    let company_key = HistorySlotKey {
        definition_id: company_definition.id,
        statement_basis: "consolidated".to_owned(),
        attribution_eff: "total".to_owned(),
        measure_window_eff: Some("flow".to_owned()),
    };
    let canonical_key = HistorySlotKey {
        definition_id: canonical_id,
        statement_basis: "consolidated".to_owned(),
        attribution_eff: "total".to_owned(),
        measure_window_eff: Some("flow".to_owned()),
    };
    let slots: std::collections::BTreeSet<_> = [company_key.clone(), canonical_key.clone()].into();
    let histories = state
        .financials()
        .slot_metric_histories(&company.id, &slots, 2099, "FY")
        .expect("slot histories should read");

    assert_eq!(
        histories.get(&company_key),
        Some(&vec![Decimal::new(4242, 0)]),
        "company-scoped definition gets its own history, not the canonical one's"
    );
    assert_eq!(
        histories.get(&canonical_key),
        Some(&Vec::new()),
        "the canonical definition has no facts of its own -- empty, not borrowed"
    );
}

#[test]
fn slot_metric_histories_final_over_preliminary_one_value_per_period() {
    let connection = open_in_memory_database().expect("database should initialize");
    let state = AppState::new(connection);
    let company = tracked_company(&state);
    let definition_id = canonical_definition_id(&state, "total_assets");
    let p2023 = seed_fy_period(&state, &company.id, 2023);

    seed_slot_fact(
        &state,
        &company.id,
        &p2023,
        &definition_id,
        "consolidated",
        "total",
        "point_in_time",
        "preliminary",
        "39000000",
    );
    seed_slot_fact(
        &state,
        &company.id,
        &p2023,
        &definition_id,
        "consolidated",
        "total",
        "point_in_time",
        "final",
        "40000000",
    );

    let key = HistorySlotKey {
        definition_id,
        statement_basis: "consolidated".to_owned(),
        attribution_eff: "total".to_owned(),
        measure_window_eff: Some("point_in_time".to_owned()),
    };
    let slots: std::collections::BTreeSet<_> = [key.clone()].into();
    let histories = state
        .financials()
        .slot_metric_histories(&company.id, &slots, 2099, "FY")
        .expect("slot histories should read");

    assert_eq!(
        histories.get(&key),
        Some(&vec![Decimal::new(40_000_000, 0)]),
        "final wins, one value for the period"
    );
}

/// #4: completeness/recall over a stored `FactSet`. The FactSet type carries
/// no quality axis (`metric_key -> Decimal`) — the smallest honest fix is at
/// the loader (final-preferred, slot-once, fixed above), never at
/// `completeness` itself. A preliminary-only metric (B) still counts as
/// covered — it IS issuer/agent-observed data — and the final-preferred merge
/// means recall is never inflated by counting A's pair twice.
#[test]
fn completeness_over_stored_fact_set_counts_each_slot_once() {
    let connection = open_in_memory_database().expect("database should initialize");
    let state = AppState::new(connection);
    let fixture = seed_quality_fixture(&state);

    let set = state
        .financials()
        .stored_fact_set(&fixture.company_id, 2025, "FY")
        .expect("stored_fact_set should query")
        .expect("a period with facts should yield Some");

    let expected: BTreeSet<String> = ["total_assets", "net_profit", "revenue", "total_equity"]
        .into_iter()
        .map(str::to_owned)
        .collect();
    let result = completeness(&set, &expected);

    assert_eq!(result.expected, 4);
    assert_eq!(
        result.present, 3,
        "A, B, C each count once — a preliminary-only metric (B) counts as covered, but A's \
         pair must not inflate recall past 3: {result:?}"
    );
    assert_eq!(result.missing, vec!["total_equity".to_owned()]);
}

// ---------------------------------------------------------------------------
// slot_history_points (#385): the slot-DISCOVERING dated twin the MCP context
// read model consumes; `slot_metric_histories` is its projection, so these
// tests are also the wrapper's zero-drift guardrail.
// ---------------------------------------------------------------------------

fn seed_period(
    state: &AppState,
    company_id: &str,
    fiscal_year: i64,
    period_type: &str,
    period_end: Option<&str>,
) -> String {
    state
        .create_financial_period(NewFinancialPeriod {
            company_id: company_id.to_owned(),
            fiscal_year,
            period_type: period_type.to_owned(),
            period_end_date: period_end.map(str::to_owned),
            report_evidence_ref: None,
        })
        .expect("financial period should create")
        .id
}

/// Multi-period characterization of the values twin, written BEFORE the #385
/// refactor and kept green after it: final-over-preliminary collapses IN
/// PLACE (the period keeps its original position in the vector), and order is
/// fact-iteration order — NOT chronological.
#[test]
fn slot_metric_histories_multi_period_collapse_keeps_scan_order() {
    let connection = open_in_memory_database().expect("database should initialize");
    let state = AppState::new(connection);
    let company = tracked_company(&state);
    let definition_id = canonical_definition_id(&state, "revenue");
    let p2022 = seed_fy_period(&state, &company.id, 2022);
    let p2023 = seed_fy_period(&state, &company.id, 2023);
    let p2024 = seed_fy_period(&state, &company.id, 2024);

    // Seed 2024 preliminary FIRST, then older periods, then the 2024 final —
    // the final overwrites the preliminary in place (index 0), so the vector
    // order proves in-place collapse + scan order survive the refactor.
    for (period, quality, value) in [
        (&p2024, "preliminary", "999"),
        (&p2022, "final", "100"),
        (&p2023, "final", "200"),
        (&p2024, "final", "300"),
    ] {
        seed_slot_fact(
            &state,
            &company.id,
            period,
            &definition_id,
            "consolidated",
            "total",
            "flow",
            quality,
            value,
        );
    }

    let key = HistorySlotKey {
        definition_id: definition_id.clone(),
        statement_basis: "consolidated".to_owned(),
        attribution_eff: "total".to_owned(),
        measure_window_eff: Some("flow".to_owned()),
    };
    let slots: std::collections::BTreeSet<_> = [key.clone()].into();
    let histories = state
        .financials()
        .slot_metric_histories(&company.id, &slots, 2099, "FY")
        .expect("slot histories should read");

    let values = histories.get(&key).expect("slot present");
    assert_eq!(values.len(), 3, "one value per period after collapse");
    assert!(
        values.contains(&Decimal::new(300, 0)),
        "the 2024 final overwrote the preliminary: {values:?}"
    );
    assert!(
        !values.contains(&Decimal::new(999, 0)),
        "no preliminary survives a final sibling: {values:?}"
    );
}

#[test]
fn slot_history_points_matches_the_values_twin_and_carries_period_identity() {
    let connection = open_in_memory_database().expect("database should initialize");
    let state = AppState::new(connection);
    let company = tracked_company(&state);
    let definition_id = canonical_definition_id(&state, "revenue");
    let p2023 = seed_fy_period(&state, &company.id, 2023);
    let ph1 = seed_period(&state, &company.id, 2024, "H1", None);

    seed_slot_fact(
        &state,
        &company.id,
        &p2023,
        &definition_id,
        "consolidated",
        "total",
        "flow",
        "final",
        "100",
    );
    seed_slot_fact(
        &state,
        &company.id,
        &ph1,
        &definition_id,
        "consolidated",
        "total",
        "flow",
        "final",
        "60",
    );

    let key = HistorySlotKey {
        definition_id: definition_id.clone(),
        statement_basis: "consolidated".to_owned(),
        attribution_eff: "total".to_owned(),
        measure_window_eff: Some("flow".to_owned()),
    };
    let ids: std::collections::BTreeSet<String> = [definition_id.clone()].into();
    let points = state
        .financials()
        .slot_history_points(&company.id, &ids, 2099, "FY")
        .expect("points should read");
    let slots: std::collections::BTreeSet<_> = [key.clone()].into();
    let histories = state
        .financials()
        .slot_metric_histories(&company.id, &slots, 2099, "FY")
        .expect("values should read");

    let slot_points = points.get(&key).expect("discovered slot");
    let projected: Vec<Decimal> = slot_points.iter().map(|p| p.value).collect();
    assert_eq!(
        Some(&projected),
        histories.get(&key),
        "the values twin is exactly the points projection"
    );
    let h1 = slot_points
        .iter()
        .find(|p| p.period_type == "H1")
        .expect("H1 point");
    assert_eq!(h1.fiscal_year, 2024);
    assert_eq!(h1.period_end, None, "stored period_end travels verbatim");
    let fy = slot_points
        .iter()
        .find(|p| p.period_type == "FY")
        .expect("FY point");
    assert_eq!(fy.period_end.as_deref(), Some("2023-12-31"));
}

#[test]
fn slot_history_points_scopes_to_requested_definitions_and_excludes_the_run_period() {
    let connection = open_in_memory_database().expect("database should initialize");
    let state = AppState::new(connection);
    let company = tracked_company(&state);
    let revenue = canonical_definition_id(&state, "revenue");
    let equity = canonical_definition_id(&state, "total_equity");
    let p2023 = seed_fy_period(&state, &company.id, 2023);
    let p2024 = seed_fy_period(&state, &company.id, 2024);

    for (definition, period, value, measure_window) in [
        (&revenue, &p2023, "100", "flow"),
        (&revenue, &p2024, "150", "flow"),
        // total_equity is `instant`-natured (ADR 0100 decision 6) — its own
        // slot uses `point_in_time`, never `flow`.
        (&equity, &p2023, "900", "point_in_time"),
    ] {
        seed_slot_fact(
            &state,
            &company.id,
            period,
            definition,
            "consolidated",
            "total",
            measure_window,
            "final",
            value,
        );
    }

    let ids: std::collections::BTreeSet<String> = [revenue.clone()].into();
    // Excluding the 2024 FY run period drops that point; the unrequested
    // equity definition is not discovered at all.
    let points = state
        .financials()
        .slot_history_points(&company.id, &ids, 2024, "FY")
        .expect("points should read");
    assert_eq!(
        points.len(),
        1,
        "only the requested definition's slot: {points:?}"
    );
    let (_, slot_points) = points.iter().next().expect("one slot");
    assert_eq!(slot_points.len(), 1);
    assert_eq!(slot_points[0].fiscal_year, 2023);
}

// ---------------------------------------------------------------------------
// KPI definition identity bound (#385): metric_key (and imported definition
// ids) are ≤256 bytes, control-character-free, at EVERY producer.
// ---------------------------------------------------------------------------

#[test]
fn create_kpi_definition_bounds_the_metric_key() {
    let connection = open_in_memory_database().expect("database should initialize");
    let state = AppState::new(connection);
    let attempt = |metric_key: String| {
        state.create_kpi_definition(NewKpiDefinition {
            scope: "user".to_owned(),
            company_id: None,
            sector: None,
            metric_key,
            label: "L".to_owned(),
            value_kind: "currency".to_owned(),
            unit: None,
            computation: "reported".to_owned(),
            formula: None,
            display_format: None,
            origin: None,
            statement_group: None,
            period_nature: None,
        })
    };

    assert!(
        matches!(
            attempt("x".repeat(257)),
            Err(StorageError::InvalidFinancialsValue {
                key: "metric_key",
                ..
            })
        ),
        "a 257-byte key must refuse"
    );
    assert!(
        matches!(
            attempt("bad\u{0007}key".to_owned()),
            Err(StorageError::InvalidFinancialsValue {
                key: "metric_key",
                ..
            })
        ),
        "a control character must refuse"
    );
    attempt("x".repeat(256)).expect("a 256-byte key is the inclusive bound");
}

// ---------------------------------------------------------------------------
// Derived-period cache provenance (#385, migration 0140).
// ---------------------------------------------------------------------------

#[test]
fn derived_period_cache_round_trips_the_content_hash() {
    let connection = open_in_memory_database().expect("database should initialize");
    let state = AppState::new(connection);
    let company = tracked_company(&state);
    let document = state
        .create_or_find_pending_report_document(CaptureReportDocumentInput {
            company_id: company.id.clone(),
            source_type: "espi_attachment".to_owned(),
            url: "https://example.com/r.pdf".to_owned(),
            period_id: None,
            origin_ref: None,
            title: None,
            attribution: None,
        })
        .expect("document");

    state
        .financials()
        .store_derived_period(
            &document.id,
            Some((2024, "FY", "2024-12-31")),
            2,
            Some("a1"),
        )
        .expect("store with hash");
    let cached = state
        .financials()
        .cached_derived_period(&document.id)
        .expect("read")
        .expect("row");
    assert_eq!(cached.content_hash.as_deref(), Some("a1"));

    // Overwrite without a hash (the pre-0140 legacy shape) → None survives the
    // upsert, which is exactly what the provenance predicate treats as a miss.
    state
        .financials()
        .store_derived_period(&document.id, Some((2024, "FY", "2024-12-31")), 2, None)
        .expect("store without hash");
    let cached = state
        .financials()
        .cached_derived_period(&document.id)
        .expect("read")
        .expect("row");
    assert_eq!(cached.content_hash, None);
}

// ---------------------------------------------------------------------------
// Curated catalog aliases (ADR 0100 decision 12, epic #398)
// ---------------------------------------------------------------------------

/// A write under a dead catalog key lands in the live key's slot. `inventory`
/// and `inventories` are both canonical rows; only the second one has ever
/// held facts, so a writer that reaches the first would file into a series
/// nothing reads. The redirect happens in the resolver, so no caller changes.
#[test]
fn a_write_under_an_alias_source_lands_on_the_live_definition() {
    let connection = open_in_memory_database().expect("database should initialize");
    let state = AppState::new(connection);
    let company = tracked_company(&state);

    state
        .kpi_extraction()
        .record_structured_fact(crate::storage::StructuredFactInput {
            company_id: &company.id,
            fiscal_year: 2025,
            period_type: "FY",
            period_end: Some("2025-12-31"),
            report_document_id: "repdoc_alias",
            metric_key: "inventory",
            value_numeric: "4200",
            currency: Some("PLN"),
            confirmation_state: "confirmed",
            source_tier: "esef",
            extraction_method: "api",
            validation_status: "passed",
            drift_json: None,
            citation: Some("alias test"),
            attribution: None,
            measure_window: None,
            data_quality: None,
        })
        .expect("the aliased write must be accepted");

    let definitions = state
        .list_kpi_definitions(ListKpiDefinitionsInput {
            scope: None,
            sector: None,
            company_id: None,
        })
        .expect("definitions");
    let live = definitions
        .iter()
        .find(|def| def.metric_key == "inventories")
        .expect("inventories is seeded");
    let dead = definitions
        .iter()
        .find(|def| def.metric_key == "inventory")
        .expect("inventory is still seeded — an alias never deletes a row");

    let facts = state
        .list_financial_facts(ListFinancialFactsInput {
            company_id: Some(company.id.clone()),
            period_id: None,
            definition_id: None,
        })
        .expect("facts list");
    assert_eq!(facts.len(), 1, "exactly one fact was written");
    assert_eq!(
        facts[0].definition_id, live.id,
        "the fact must land on `inventories`, the key that carries the series"
    );
    assert_ne!(
        facts[0].definition_id, dead.id,
        "nothing may file into the dead key"
    );
}

/// The alias's one-sidedness is a RUNTIME guard, not a curation promise (sol
/// review finding 9): on a database where the dead key already holds a fact
/// (an import, an older schema, a manual entry), the redirect must never
/// fire — redirecting there would split one series across two keys.
#[test]
fn an_alias_source_that_already_holds_facts_is_never_redirected() {
    let connection = open_in_memory_database().expect("database should initialize");
    let state = AppState::new(connection);
    let company = tracked_company(&state);

    // Simulate the pre-alias world: a fact already sits under `inventory`.
    // Written via the raw creation path against the dead definition directly,
    // the way an old database or an import would have left it.
    let period = state
        .create_financial_period(NewFinancialPeriod {
            company_id: company.id.clone(),
            fiscal_year: 2024,
            period_type: "FY".to_owned(),
            period_end_date: Some("2024-12-31".to_owned()),
            report_evidence_ref: None,
        })
        .expect("period");
    state
        .create_financial_fact(NewFinancialFact {
            company_id: company.id.clone(),
            period_id: period.id.clone(),
            definition_id: "kpidef_inventory".to_owned(),
            value_numeric: "1000".to_owned(),
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
            annotation: None,
        })
        .expect("pre-existing inventory fact");

    // A structured write under the dead key must now land on `inventory`
    // itself — the series already exists there, so no redirect.
    state
        .kpi_extraction()
        .record_structured_fact(crate::storage::StructuredFactInput {
            company_id: &company.id,
            fiscal_year: 2025,
            period_type: "FY",
            period_end: Some("2025-12-31"),
            report_document_id: "repdoc_alias_guard",
            metric_key: "inventory",
            value_numeric: "2000",
            currency: Some("PLN"),
            confirmation_state: "confirmed",
            source_tier: "esef",
            extraction_method: "api",
            validation_status: "passed",
            drift_json: None,
            citation: Some("alias one-sidedness"),
            attribution: None,
            measure_window: None,
            data_quality: None,
        })
        .expect("write accepted");

    let facts = state
        .list_financial_facts(ListFinancialFactsInput {
            company_id: Some(company.id.clone()),
            period_id: None,
            definition_id: None,
        })
        .expect("facts");
    assert_eq!(facts.len(), 2);
    assert!(
        facts
            .iter()
            .all(|fact| fact.definition_id == "kpidef_inventory"),
        "both facts stay in the `inventory` series — a populated alias source is never redirected"
    );

    // Per-company one-sidedness (sol round 2): company A's legacy series
    // must not flip routing for company B — B has no `inventory` facts, so
    // B's write redirects to `inventories` exactly as on a clean database.
    let other = state
        .create_company(NewCompany {
            exchange: "GPW".to_owned(),
            ticker: "OTH".to_owned(),
            display_name: "Other S.A.".to_owned(),
            isin: None,
            cik: None,
            lei: None,
        })
        .expect("second company");
    state
        .kpi_extraction()
        .record_structured_fact(crate::storage::StructuredFactInput {
            company_id: &other.id,
            fiscal_year: 2025,
            period_type: "FY",
            period_end: Some("2025-12-31"),
            report_document_id: "repdoc_alias_guard_b",
            metric_key: "inventory",
            value_numeric: "700",
            currency: Some("PLN"),
            confirmation_state: "confirmed",
            source_tier: "esef",
            extraction_method: "api",
            validation_status: "passed",
            drift_json: None,
            citation: Some("alias per-company"),
            attribution: None,
            measure_window: None,
            data_quality: None,
        })
        .expect("company B write accepted");
    let b_facts = state
        .list_financial_facts(ListFinancialFactsInput {
            company_id: Some(other.id.clone()),
            period_id: None,
            definition_id: None,
        })
        .expect("company B facts");
    assert_eq!(b_facts.len(), 1);
    assert_eq!(
        b_facts[0].definition_id, "kpidef_inventories",
        "a clean company still redirects — the guard is per company, never a global switch"
    );
}
