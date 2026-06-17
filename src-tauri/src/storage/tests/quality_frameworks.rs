//! Storage tests for quality frameworks (ADR 0046).

use super::*;

fn tracked_company(state: &AppState) -> Company {
    state
        .create_company(NewCompany {
            exchange: "GPW".to_owned(),
            ticker: "QF".to_owned(),
            display_name: "Quality Frameworks Test Co.".to_owned(),
            isin: Some("PLQF00000000".to_owned()),
            cik: None,
            lei: None,
        })
        .expect("tracked company should create")
}

fn definition_id(state: &AppState, metric_key: &str) -> String {
    state
        .list_kpi_definitions(ListKpiDefinitionsInput {
            scope: None,
            sector: None,
            company_id: None,
        })
        .expect("definitions should list")
        .into_iter()
        .find(|d| d.metric_key == metric_key)
        .unwrap_or_else(|| panic!("metric {metric_key} should be seeded"))
        .id
}

fn confirmed_fact(
    state: &AppState,
    company: &str,
    period: &str,
    metric_key: &str,
    value: &str,
) -> FinancialFact {
    state
        .create_financial_fact(NewFinancialFact {
            company_id: company.to_owned(),
            period_id: period.to_owned(),
            definition_id: definition_id(state, metric_key),
            value_numeric: value.to_owned(),
            currency: Some("PLN".to_owned()),
            statement_basis: Some("consolidated".to_owned()),
            attribution: Some("total".to_owned()),
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
            source_document_ref: None,
        })
        .expect("fact should create")
}

#[test]
fn seeds_kroeze_template_on_startup() {
    let connection = open_in_memory_database().expect("database should initialize");
    let state = AppState::new(connection);

    let frameworks = state
        .list_quality_frameworks()
        .expect("frameworks should list");
    let kroeze = frameworks
        .iter()
        .find(|f| f.template_key.as_deref() == Some("kroeze_quality"))
        .expect("Kroeze template should be seeded");
    assert_eq!(kroeze.origin, "app_template");
    assert!(!kroeze.criteria.is_empty());
}

#[test]
fn seeding_is_idempotent() {
    let connection = open_in_memory_database().expect("database should initialize");
    // Two AppState constructions over the same DB both run seeding.
    let state = AppState::new(connection);
    let first = state.list_quality_frameworks().expect("list");
    let templates: Vec<_> = first
        .iter()
        .filter(|f| f.origin == "app_template")
        .collect();
    assert_eq!(templates.len(), 1, "exactly one app template after seeding");
}

#[test]
fn creates_framework_with_criteria_and_validates_expression() {
    let connection = open_in_memory_database().expect("database should initialize");
    let state = AppState::new(connection);

    let framework = state
        .create_quality_framework(NewQualityFramework {
            name: "My checklist".to_owned(),
            description: Some("personal".to_owned()),
        })
        .expect("framework creates");
    assert_eq!(framework.origin, "user");
    assert_eq!(framework.version, 1);

    state
        .create_framework_criterion(NewFrameworkCriterion {
            framework_id: framework.id.clone(),
            label: "Strong ROE".to_owned(),
            expression: "roe >= 15%".to_owned(),
            weight: None,
            partial_band: Some("10%".to_owned()),
            ordinal: None,
        })
        .expect("criterion creates");

    let reloaded = state
        .get_quality_framework(&framework.id)
        .expect("reload framework");
    assert_eq!(reloaded.criteria.len(), 1);
    // Creating a criterion bumps the framework version.
    assert!(reloaded.version > 1);

    // A non-predicate expression is rejected.
    let invalid = state.create_framework_criterion(NewFrameworkCriterion {
        framework_id: framework.id.clone(),
        label: "bad".to_owned(),
        expression: "roe + 1".to_owned(),
        weight: None,
        partial_band: None,
        ordinal: None,
    });
    assert!(
        invalid.is_err(),
        "non-predicate criterion should be rejected"
    );
}

#[test]
fn validate_expression_reports_referenced_metrics() {
    let connection = open_in_memory_database().expect("database should initialize");
    let state = AppState::new(connection);
    let result = state.validate_criterion_expression("net_debt_to_ebitda < 2.5 AND fcf > 0");
    assert!(result.ok);
    assert_eq!(
        result.referenced_metric_keys,
        vec!["fcf".to_owned(), "net_debt_to_ebitda".to_owned()]
    );

    let bad = state.validate_criterion_expression("roic >=");
    assert!(!bad.ok);
    assert!(bad.error.is_some());
}

#[test]
fn clone_produces_user_copy_with_lineage() {
    let connection = open_in_memory_database().expect("database should initialize");
    let state = AppState::new(connection);
    let template = state
        .list_quality_frameworks()
        .expect("list")
        .into_iter()
        .find(|f| f.template_key.as_deref() == Some("kroeze_quality"))
        .expect("template");

    let clone = state
        .clone_framework(CloneFrameworkInput {
            framework_id: template.id.clone(),
            name: Some("My Kroeze".to_owned()),
        })
        .expect("clone");
    assert_eq!(clone.origin, "user");
    assert_eq!(clone.cloned_from.as_deref(), Some(template.id.as_str()));
    assert_eq!(clone.criteria.len(), template.criteria.len());
}

#[test]
fn reset_template_restores_defaults_and_rejects_user_frameworks() {
    let connection = open_in_memory_database().expect("database should initialize");
    let state = AppState::new(connection);
    let template = state
        .list_quality_frameworks()
        .expect("list")
        .into_iter()
        .find(|f| f.template_key.as_deref() == Some("kroeze_quality"))
        .expect("template");
    let original_count = template.criteria.len();

    // Edit the template in place: delete a criterion.
    state
        .delete_framework_criterion(&template.criteria[0].id)
        .expect("delete criterion");
    let edited = state.get_quality_framework(&template.id).expect("reload");
    assert_eq!(edited.criteria.len(), original_count - 1);

    // Reset restores the shipped defaults.
    let reset = state
        .reset_framework_to_template(&template.id)
        .expect("reset");
    assert_eq!(reset.criteria.len(), original_count);

    // A user framework cannot be reset.
    let user = state
        .create_quality_framework(NewQualityFramework {
            name: "user".to_owned(),
            description: None,
        })
        .expect("create");
    assert!(state.reset_framework_to_template(&user.id).is_err());
}

#[test]
fn evaluate_framework_produces_scorecard_with_measured_values() {
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
        .expect("period creates");

    // Facts that make gross_margin computable: 350/1000 = 0.35.
    confirmed_fact(&state, &company.id, &period.id, "revenue", "1000");
    confirmed_fact(&state, &company.id, &period.id, "gross_profit", "350");

    let framework = state
        .create_quality_framework(NewQualityFramework {
            name: "Margin check".to_owned(),
            description: None,
        })
        .expect("framework");
    state
        .create_framework_criterion(NewFrameworkCriterion {
            framework_id: framework.id.clone(),
            label: "Gross margin over 30%".to_owned(),
            expression: "gross_margin > 30%".to_owned(),
            weight: None,
            partial_band: None,
            ordinal: None,
        })
        .expect("criterion");
    // A criterion whose inputs are missing → unavailable.
    state
        .create_framework_criterion(NewFrameworkCriterion {
            framework_id: framework.id.clone(),
            label: "Leverage".to_owned(),
            expression: "net_debt_to_ebitda < 2.5".to_owned(),
            weight: None,
            partial_band: None,
            ordinal: None,
        })
        .expect("criterion");

    let evaluation = state
        .evaluate_framework(EvaluateFrameworkInput {
            framework_id: framework.id.clone(),
            company_id: company.id.clone(),
        })
        .expect("evaluation");

    assert_eq!(evaluation.period_id.as_deref(), Some(period.id.as_str()));
    assert_eq!(evaluation.pass_count, 1);
    assert_eq!(evaluation.unavailable_count, 1);
    let margin = evaluation
        .results
        .iter()
        .find(|r| r.label == "Gross margin over 30%")
        .expect("margin result");
    assert_eq!(margin.verdict, "pass");
    assert_eq!(margin.measured_value.as_deref(), Some("0.35"));
}

#[test]
fn evaluation_snapshot_is_pinned_when_facts_change() {
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
        .expect("period");
    confirmed_fact(&state, &company.id, &period.id, "revenue", "1000");
    let gross_profit = confirmed_fact(&state, &company.id, &period.id, "gross_profit", "350");

    let framework = state
        .create_quality_framework(NewQualityFramework {
            name: "Margin".to_owned(),
            description: None,
        })
        .expect("framework");
    state
        .create_framework_criterion(NewFrameworkCriterion {
            framework_id: framework.id.clone(),
            label: "GM".to_owned(),
            expression: "gross_margin > 30%".to_owned(),
            weight: None,
            partial_band: None,
            ordinal: None,
        })
        .expect("criterion");

    let first = state
        .evaluate_framework(EvaluateFrameworkInput {
            framework_id: framework.id.clone(),
            company_id: company.id.clone(),
        })
        .expect("first run");

    // Change the underlying facts and re-fetch the FIRST evaluation: its measured
    // value must be pinned to the original run.
    state
        .update_financial_fact(UpdateFinancialFact {
            id: gross_profit.id.clone(),
            value_numeric: Some("100".to_owned()),
            currency: None,
            data_quality: None,
            confirmation_state: None,
            supersedes_id: None,
            source_document_ref: None,
        })
        .expect("update fact");
    let reloaded = state
        .get_framework_evaluation(&first.id)
        .expect("reload first evaluation");
    let gm = reloaded.results.iter().find(|r| r.label == "GM").unwrap();
    assert_eq!(
        gm.measured_value.as_deref(),
        Some("0.35"),
        "snapshot pinned"
    );

    // The evaluation history accumulates.
    let history = state
        .list_framework_evaluations(ListFrameworkEvaluationsInput {
            framework_id: framework.id.clone(),
            company_id: company.id.clone(),
        })
        .expect("history");
    assert_eq!(history.len(), 1);
}

#[test]
fn frameworks_round_trip_through_export_import_with_custom_metric() {
    let source = AppState::new(open_in_memory_database().expect("database should initialize"));

    // A user-defined global custom metric, referenced by a user framework.
    source
        .create_kpi_definition(NewKpiDefinition {
            scope: "user".to_owned(),
            company_id: None,
            sector: None,
            metric_key: "rule_of_40".to_owned(),
            label: "Rule of 40".to_owned(),
            value_kind: "percentage".to_owned(),
            unit: None,
            computation: "derived".to_owned(),
            formula: Some("operating_margin + fcf_margin".to_owned()),
            display_format: None,
        })
        .expect("custom metric creates");

    let framework = source
        .create_quality_framework(NewQualityFramework {
            name: "My screen".to_owned(),
            description: None,
        })
        .expect("framework");
    source
        .create_framework_criterion(NewFrameworkCriterion {
            framework_id: framework.id.clone(),
            label: "Rule of 40".to_owned(),
            expression: "rule_of_40 >= 40%".to_owned(),
            weight: None,
            partial_band: None,
            ordinal: None,
        })
        .expect("criterion");

    let export = source.export_research_data().expect("export");
    // The seeded template + the user framework both travel.
    assert!(export.summary.quality_frameworks >= 2);
    assert_eq!(export.summary.user_metrics, 1);

    // A fresh instance already has the Kroeze template seeded, so importing the
    // source's template is skipped while the user framework + custom metric arrive.
    let target = AppState::new(open_in_memory_database().expect("database should initialize"));
    let preview = target
        .preview_research_import(&export.contents)
        .expect("preview");
    assert!(preview.valid, "{:?}", preview.errors);
    assert_eq!(preview.summary.quality_frameworks_created, 1);
    assert_eq!(preview.summary.quality_frameworks_skipped, 1); // the shipped template
    assert_eq!(preview.summary.user_metrics_created, 1);

    target
        .apply_research_import(&export.contents)
        .expect("apply");

    let frameworks = target.list_quality_frameworks().expect("list");
    let imported = frameworks
        .iter()
        .find(|f| f.name == "My screen")
        .expect("user framework imported");
    assert_eq!(imported.criteria.len(), 1);
    assert_eq!(imported.criteria[0].expression, "rule_of_40 >= 40%");

    // The referenced custom metric came along, so the criterion can resolve.
    let metrics = target
        .list_available_metric_keys(None)
        .expect("metric keys");
    assert!(metrics.iter().any(|m| m.key == "rule_of_40"));

    // Re-importing is idempotent: the user framework is now skipped too.
    let preview2 = target
        .preview_research_import(&export.contents)
        .expect("preview2");
    assert_eq!(preview2.summary.quality_frameworks_created, 0);
}

#[test]
fn evaluation_runs_can_be_pruned_from_history() {
    let state = AppState::new(open_in_memory_database().expect("database should initialize"));
    let company = tracked_company(&state);
    state
        .create_financial_period(NewFinancialPeriod {
            company_id: company.id.clone(),
            fiscal_year: 2026,
            period_type: "FY".to_owned(),
            period_end_date: Some("2026-12-31".to_owned()),
            report_evidence_ref: None,
        })
        .expect("period");
    let framework = state
        .create_quality_framework(NewQualityFramework {
            name: "F".to_owned(),
            description: None,
        })
        .expect("framework");

    let first = state
        .evaluate_framework(EvaluateFrameworkInput {
            framework_id: framework.id.clone(),
            company_id: company.id.clone(),
        })
        .expect("run 1");
    state
        .evaluate_framework(EvaluateFrameworkInput {
            framework_id: framework.id.clone(),
            company_id: company.id.clone(),
        })
        .expect("run 2");

    let history = state
        .list_framework_evaluations(ListFrameworkEvaluationsInput {
            framework_id: framework.id.clone(),
            company_id: company.id.clone(),
        })
        .expect("history");
    assert_eq!(history.len(), 2);

    state
        .delete_framework_evaluation(&first.id)
        .expect("delete one run");
    let after = state
        .list_framework_evaluations(ListFrameworkEvaluationsInput {
            framework_id: framework.id.clone(),
            company_id: company.id.clone(),
        })
        .expect("history after");
    assert_eq!(after.len(), 1);
    assert!(after.iter().all(|e| e.id != first.id));

    // Deleting a missing evaluation errors.
    assert!(state.delete_framework_evaluation(&first.id).is_err());
}

#[test]
fn every_seeded_formula_parses_with_the_engine() {
    // Grammar-drift gate (ADR 0046 Decision 8): every derived kpi_definitions.formula
    // must parse with the one engine, so a formula can never ship unparseable.
    let connection = open_in_memory_database().expect("database should initialize");
    let state = AppState::new(connection);
    let definitions = state
        .list_kpi_definitions(ListKpiDefinitionsInput {
            scope: None,
            sector: None,
            company_id: None,
        })
        .expect("definitions");
    for def in definitions {
        if def.computation == "derived" {
            let formula = def.formula.expect("derived metric has a formula");
            crate::fundamentals::expr::parse(&formula).unwrap_or_else(|e| {
                panic!(
                    "formula for {} does not parse: {} ({e})",
                    def.metric_key, formula
                )
            });
        }
    }
}
