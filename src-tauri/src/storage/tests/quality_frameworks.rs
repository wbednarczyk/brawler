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
            annotation: None,
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
fn kroeze_template_seeds_qualitative_criteria_with_guidance() {
    // §T6 extends the Kroeze template with agent-assessed qualitative criteria
    // (ADR 0075). Each carries owner guidance and no DSL expression.
    let connection = open_in_memory_database().expect("database should initialize");
    let state = AppState::new(connection);

    let frameworks = state
        .list_quality_frameworks()
        .expect("frameworks should list");
    let kroeze = frameworks
        .iter()
        .find(|f| f.template_key.as_deref() == Some("kroeze_quality"))
        .expect("Kroeze template should be seeded");

    let qualitative: Vec<_> = kroeze
        .criteria
        .iter()
        .filter(|c| c.kind == "qualitative")
        .collect();
    assert_eq!(
        qualitative.len(),
        6,
        "Kroeze template should seed 6 qualitative criteria"
    );
    for criterion in &qualitative {
        assert!(
            !criterion
                .assessment_guidance
                .as_deref()
                .unwrap_or("")
                .trim()
                .is_empty(),
            "qualitative criterion '{}' must carry guidance",
            criterion.label
        );
        assert!(
            criterion.expression.is_empty(),
            "qualitative criterion '{}' must have no DSL expression",
            criterion.label
        );
    }
    assert!(
        qualitative
            .iter()
            .any(|c| c.label.to_lowercase().contains("moat")),
        "the moat criterion should be present"
    );
    // The quantitative criteria are unchanged.
    assert!(
        kroeze.criteria.iter().any(|c| c.kind == "quantitative"),
        "quantitative criteria should remain"
    );
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
            kind: None,
            assessment_guidance: None,
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
        kind: None,
        assessment_guidance: None,
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
            kind: None,
            assessment_guidance: None,
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
            kind: None,
            assessment_guidance: None,
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
fn quant_evaluation_skips_qualitative_criteria() {
    // A quantitative run must ignore qualitative criteria entirely (ADR 0075,
    // composition boundary): a qualitative criterion has an empty expression, so
    // the quant engine would otherwise write a phantom `unavailable` result row.
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
    confirmed_fact(&state, &company.id, &period.id, "revenue", "1000");
    confirmed_fact(&state, &company.id, &period.id, "gross_profit", "350");

    let framework = state
        .create_quality_framework(NewQualityFramework {
            name: "Mixed checklist".to_owned(),
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
            kind: None,
            assessment_guidance: None,
        })
        .expect("quant criterion");
    state
        .create_framework_criterion(NewFrameworkCriterion {
            framework_id: framework.id.clone(),
            label: "Durable moat".to_owned(),
            expression: String::new(),
            weight: None,
            partial_band: None,
            ordinal: None,
            kind: Some("qualitative".to_owned()),
            assessment_guidance: Some("Assess the durability of the moat.".to_owned()),
        })
        .expect("qualitative criterion");

    let evaluation = state
        .evaluate_framework(EvaluateFrameworkInput {
            framework_id: framework.id.clone(),
            company_id: company.id.clone(),
        })
        .expect("evaluation");

    // Only the quantitative criterion is scored; the qualitative one is absent.
    assert_eq!(evaluation.pass_count, 1);
    assert_eq!(evaluation.unavailable_count, 0);
    assert_eq!(evaluation.results.len(), 1);
    assert!(
        evaluation.results.iter().all(|r| r.label != "Durable moat"),
        "qualitative criterion must not produce an engine result row"
    );
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
            kind: None,
            assessment_guidance: None,
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
            annotation: None,
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
            kind: None,
            assessment_guidance: None,
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

// ---- Qualitative, agent-assessed criteria (ADR 0075, v0.50.0) --------------

#[test]
fn creates_qualitative_criterion_persists_kind_and_guidance() {
    let state = AppState::new(open_in_memory_database().expect("database should initialize"));
    let framework = state
        .create_quality_framework(NewQualityFramework {
            name: "Qual".to_owned(),
            description: None,
        })
        .expect("framework");

    // A qualitative criterion carries owner guidance and no DSL expression.
    state
        .create_framework_criterion(NewFrameworkCriterion {
            framework_id: framework.id.clone(),
            label: "Wide moat".to_owned(),
            expression: String::new(),
            weight: None,
            partial_band: None,
            ordinal: None,
            kind: Some("qualitative".to_owned()),
            assessment_guidance: Some("Assess durable competitive advantage.".to_owned()),
        })
        .expect("qualitative criterion creates");

    let reloaded = state
        .get_quality_framework(&framework.id)
        .expect("reload framework");
    let criterion = reloaded
        .criteria
        .iter()
        .find(|c| c.label == "Wide moat")
        .expect("qualitative criterion present");
    assert_eq!(criterion.kind, "qualitative");
    assert_eq!(
        criterion.assessment_guidance.as_deref(),
        Some("Assess durable competitive advantage.")
    );
    assert_eq!(
        criterion.expression, "",
        "a qualitative criterion stores no DSL expression"
    );
}

#[test]
fn quantitative_criterion_defaults_kind_and_has_no_guidance() {
    let state = AppState::new(open_in_memory_database().expect("database should initialize"));
    let framework = state
        .create_quality_framework(NewQualityFramework {
            name: "Quant".to_owned(),
            description: None,
        })
        .expect("framework");
    // Absent kind ⇒ quantitative (safe default), predicate still validated.
    state
        .create_framework_criterion(NewFrameworkCriterion {
            framework_id: framework.id.clone(),
            label: "Strong ROE".to_owned(),
            expression: "roe >= 15%".to_owned(),
            weight: None,
            partial_band: None,
            ordinal: None,
            kind: None,
            assessment_guidance: None,
        })
        .expect("quantitative criterion creates");

    let reloaded = state.get_quality_framework(&framework.id).expect("reload");
    let criterion = &reloaded.criteria[0];
    assert_eq!(criterion.kind, "quantitative");
    assert_eq!(criterion.assessment_guidance, None);
}

#[test]
fn qualitative_criterion_requires_guidance() {
    let state = AppState::new(open_in_memory_database().expect("database should initialize"));
    let framework = state
        .create_quality_framework(NewQualityFramework {
            name: "Q".to_owned(),
            description: None,
        })
        .expect("framework");
    let result = state.create_framework_criterion(NewFrameworkCriterion {
        framework_id: framework.id.clone(),
        label: "No guidance".to_owned(),
        expression: String::new(),
        weight: None,
        partial_band: None,
        ordinal: None,
        kind: Some("qualitative".to_owned()),
        assessment_guidance: None,
    });
    assert!(
        result.is_err(),
        "a qualitative criterion without guidance must be rejected"
    );
}

#[test]
fn updating_assessment_guidance_persists_and_preserves_kind() {
    let state = AppState::new(open_in_memory_database().expect("database should initialize"));
    let framework = state
        .create_quality_framework(NewQualityFramework {
            name: "Q".to_owned(),
            description: None,
        })
        .expect("framework");
    let criterion = state
        .create_framework_criterion(NewFrameworkCriterion {
            framework_id: framework.id.clone(),
            label: "Pricing power".to_owned(),
            expression: String::new(),
            weight: None,
            partial_band: None,
            ordinal: None,
            kind: Some("qualitative".to_owned()),
            assessment_guidance: Some("v1 guidance".to_owned()),
        })
        .expect("criterion");

    state
        .update_framework_criterion(UpdateFrameworkCriterion {
            id: criterion.id.clone(),
            label: None,
            expression: None,
            weight: None,
            partial_band: None,
            ordinal: None,
            kind: None,
            assessment_guidance: Some("v2 refined guidance".to_owned()),
        })
        .expect("update guidance");

    let reloaded = state.get_quality_framework(&framework.id).expect("reload");
    let updated = reloaded
        .criteria
        .iter()
        .find(|c| c.id == criterion.id)
        .expect("criterion present");
    assert_eq!(
        updated.assessment_guidance.as_deref(),
        Some("v2 refined guidance")
    );
    assert_eq!(updated.kind, "qualitative", "kind preserved across update");
}

/// Helper: create a qualitative criterion with guidance.
fn qualitative_criterion(state: &AppState, framework_id: &str, label: &str) -> FrameworkCriterion {
    state
        .create_framework_criterion(NewFrameworkCriterion {
            framework_id: framework_id.to_owned(),
            label: label.to_owned(),
            expression: String::new(),
            weight: None,
            partial_band: None,
            ordinal: None,
            kind: Some("qualitative".to_owned()),
            assessment_guidance: Some(format!("Assess {label}.")),
        })
        .expect("qualitative criterion creates")
}

fn qual_result(
    criterion_id: &str,
    ordinal: i64,
    label: &str,
    verdict: &str,
    reasoning: &str,
) -> QualitativeCriterionResult {
    QualitativeCriterionResult {
        criterion_id: criterion_id.to_owned(),
        ordinal,
        label: label.to_owned(),
        verdict: verdict.to_owned(),
        reasoning: reasoning.to_owned(),
        citations_json: "[]".to_owned(),
        confidence: "medium".to_owned(),
        prompt_version: "qualitative_assessment_v1".to_owned(),
    }
}

/// ADR 0075 Decision 5 "two read surfaces": the current-state read returns, per
/// qualitative criterion, the most-recent agent-assessed row across snapshots —
/// so a later single-criterion re-run never blanks the other criteria, and a
/// never-assessed criterion is simply absent (empty state).
#[test]
fn get_qualitative_assessment_returns_latest_agent_row_per_criterion() {
    let state = AppState::new(open_in_memory_database().expect("database should initialize"));
    let company = tracked_company(&state);
    let framework = state
        .create_quality_framework(NewQualityFramework {
            name: "Q".to_owned(),
            description: None,
        })
        .expect("framework");
    let moat = qualitative_criterion(&state, &framework.id, "Wide moat");
    let pricing = qualitative_criterion(&state, &framework.id, "Pricing power");
    let recurring = qualitative_criterion(&state, &framework.id, "Recurring revenue");

    // Run 1: moat + pricing assessed together.
    state
        .persist_qualitative_assessment(PersistQualitativeAssessmentInput {
            framework_id: framework.id.clone(),
            company_id: company.id.clone(),
            results: vec![
                qual_result(&moat.id, 0, "Wide moat", "pass", "Durable advantage."),
                qual_result(
                    &pricing.id,
                    1,
                    "Pricing power",
                    "partial",
                    "Some pricing power.",
                ),
            ],
        })
        .expect("run 1 persists");

    // Run 2 (later snapshot): only moat re-assessed (single-criterion re-run),
    // verdict changes fail. This must NOT blank pricing's earlier assessment.
    state
        .persist_qualitative_assessment(PersistQualitativeAssessmentInput {
            framework_id: framework.id.clone(),
            company_id: company.id.clone(),
            results: vec![qual_result(
                &moat.id,
                0,
                "Wide moat",
                "fail",
                "Moat eroding.",
            )],
        })
        .expect("run 2 persists");

    let current = state
        .get_qualitative_assessment(&framework.id, &company.id)
        .expect("current-state read");

    assert_eq!(current.len(), 2, "only assessed criteria appear");
    let moat_row = current
        .iter()
        .find(|r| r.criterion_id.as_deref() == Some(moat.id.as_str()))
        .expect("moat present");
    assert_eq!(moat_row.verdict, "fail", "latest moat verdict wins");
    assert_eq!(moat_row.reasoning.as_deref(), Some("Moat eroding."));
    assert_eq!(moat_row.source, "agent");
    let pricing_row = current
        .iter()
        .find(|r| r.criterion_id.as_deref() == Some(pricing.id.as_str()))
        .expect("pricing present");
    assert_eq!(
        pricing_row.verdict, "partial",
        "a later single-criterion re-run must not blank pricing"
    );
    assert!(
        current
            .iter()
            .all(|r| r.criterion_id.as_deref() != Some(recurring.id.as_str())),
        "a never-assessed criterion is absent (empty state)"
    );
}

/// §T6 change detection (ADR 0075 Decision 5): per qualitative criterion,
/// compare the two most-recent agent-assessed verdicts and report a transition
/// when they differ. A criterion assessed only once (no previous) is not a
/// change; an unchanged verdict is not a change.
#[test]
fn qualitative_verdict_changes_reports_per_criterion_transitions() {
    let state = AppState::new(open_in_memory_database().expect("database should initialize"));
    let company = tracked_company(&state);
    let framework = state
        .create_quality_framework(NewQualityFramework {
            name: "Q".to_owned(),
            description: None,
        })
        .expect("framework");
    let moat = qualitative_criterion(&state, &framework.id, "Wide moat");
    let pricing = qualitative_criterion(&state, &framework.id, "Pricing power");
    let recurring = qualitative_criterion(&state, &framework.id, "Recurring revenue");

    // Run 1: all three assessed.
    state
        .persist_qualitative_assessment(PersistQualitativeAssessmentInput {
            framework_id: framework.id.clone(),
            company_id: company.id.clone(),
            results: vec![
                qual_result(&moat.id, 0, "Wide moat", "pass", "Durable advantage."),
                qual_result(&pricing.id, 1, "Pricing power", "partial", "Some pricing."),
                qual_result(&recurring.id, 2, "Recurring revenue", "pass", "Recurring."),
            ],
        })
        .expect("run 1 persists");

    // Run 2: moat changes (pass→fail), pricing unchanged (partial), recurring not
    // re-run (only one assessment ever).
    state
        .persist_qualitative_assessment(PersistQualitativeAssessmentInput {
            framework_id: framework.id.clone(),
            company_id: company.id.clone(),
            results: vec![
                qual_result(&moat.id, 0, "Wide moat", "fail", "Moat eroding."),
                qual_result(&pricing.id, 1, "Pricing power", "partial", "Still some."),
            ],
        })
        .expect("run 2 persists");

    let changes = state
        .qualitative_verdict_changes(&framework.id, &company.id)
        .expect("verdict changes");

    assert_eq!(changes.len(), 1, "only the moat verdict changed");
    let change = &changes[0];
    assert_eq!(change.criterion_id, moat.id);
    assert_eq!(change.label, "Wide moat");
    assert_eq!(change.previous_verdict, "pass");
    assert_eq!(change.current_verdict, "fail");
}

#[test]
fn updating_quantitative_criterion_revalidates_and_preserves_kind() {
    let state = AppState::new(open_in_memory_database().expect("database should initialize"));
    let framework = state
        .create_quality_framework(NewQualityFramework {
            name: "Quant".to_owned(),
            description: None,
        })
        .expect("framework");
    let criterion = state
        .create_framework_criterion(NewFrameworkCriterion {
            framework_id: framework.id.clone(),
            label: "ROE".to_owned(),
            expression: "roe >= 15%".to_owned(),
            weight: None,
            partial_band: None,
            ordinal: None,
            kind: None,
            assessment_guidance: None,
        })
        .expect("criterion");

    // A valid new expression is accepted and the kind stays quantitative.
    state
        .update_framework_criterion(UpdateFrameworkCriterion {
            id: criterion.id.clone(),
            label: None,
            expression: Some("roe >= 20%".to_owned()),
            weight: None,
            partial_band: None,
            ordinal: None,
            kind: None,
            assessment_guidance: None,
        })
        .expect("update expression");
    let reloaded = state.get_quality_framework(&framework.id).expect("reload");
    let updated = &reloaded.criteria[0];
    assert_eq!(updated.expression, "roe >= 20%");
    assert_eq!(updated.kind, "quantitative");

    // A non-predicate expression is still rejected on update.
    let invalid = state.update_framework_criterion(UpdateFrameworkCriterion {
        id: criterion.id.clone(),
        label: None,
        expression: Some("roe + 1".to_owned()),
        weight: None,
        partial_band: None,
        ordinal: None,
        kind: None,
        assessment_guidance: None,
    });
    assert!(invalid.is_err(), "a non-predicate update must be rejected");
}

#[test]
fn switching_qualitative_to_quantitative_requires_valid_expression() {
    let state = AppState::new(open_in_memory_database().expect("database should initialize"));
    let framework = state
        .create_quality_framework(NewQualityFramework {
            name: "Q".to_owned(),
            description: None,
        })
        .expect("framework");
    let criterion = state
        .create_framework_criterion(NewFrameworkCriterion {
            framework_id: framework.id.clone(),
            label: "Moat".to_owned(),
            expression: String::new(),
            weight: None,
            partial_band: None,
            ordinal: None,
            kind: Some("qualitative".to_owned()),
            assessment_guidance: Some("Assess the moat.".to_owned()),
        })
        .expect("qualitative criterion");

    // Switching to quantitative without a valid expression must be rejected — never
    // silently stored as an empty-expression quantitative criterion (which would
    // evaluate to a bogus verdict).
    let result = state.update_framework_criterion(UpdateFrameworkCriterion {
        id: criterion.id.clone(),
        label: None,
        expression: None,
        weight: None,
        partial_band: None,
        ordinal: None,
        kind: Some("quantitative".to_owned()),
        assessment_guidance: None,
    });
    assert!(
        result.is_err(),
        "a quantitative criterion must not persist an empty/invalid expression"
    );
}

#[test]
fn criterion_result_agent_fields_round_trip() {
    // The read model surfaces the agent-assessed columns (ADR 0075). The typed
    // write is T4's job; here we seed a persisted agent result directly and prove
    // the storage read maps every field back.
    let connection = open_in_memory_database().expect("database should initialize");
    connection
        .execute(
            "INSERT INTO companies (id, exchange, ticker, qualified_ticker, display_name)
             VALUES ('co1','GPW','QF','GPW:QF','QF Co')",
            [],
        )
        .expect("seed company");
    connection
        .execute(
            "INSERT INTO quality_frameworks (id, name, origin, version) VALUES ('fw1','F','user',1)",
            [],
        )
        .expect("seed framework");
    connection
        .execute(
            "INSERT INTO framework_evaluations
                (id, framework_id, framework_version, company_id, period_id,
                 pass_count, partial_count, fail_count, unavailable_count, engine_version)
             VALUES ('ev1','fw1',1,'co1',NULL,0,0,0,1,'test')",
            [],
        )
        .expect("seed evaluation");
    connection
        .execute(
            r#"INSERT INTO criterion_results
                (id, evaluation_id, criterion_id, ordinal, label, expression, verdict,
                 reasoning, citations, confidence, prompt_version, source)
             VALUES ('cr1','ev1',NULL,0,'Wide moat','','insufficient_evidence',
                     'Not enough disclosure to judge the moat.',
                     '[{"evidenceType":"claim","evidenceId":"cl1","label":"mgmt claim","snippet":"..."}]',
                     'medium','qual_assessment_v1','agent')"#,
            [],
        )
        .expect("seed agent result");

    let evaluation =
        crate::storage::quality_frameworks::get_framework_evaluation(&connection, "ev1")
            .expect("evaluation loads");
    let result = &evaluation.results[0];
    assert_eq!(result.verdict, "insufficient_evidence");
    assert_eq!(result.source, "agent");
    assert_eq!(
        result.reasoning.as_deref(),
        Some("Not enough disclosure to judge the moat.")
    );
    assert_eq!(result.confidence.as_deref(), Some("medium"));
    assert_eq!(result.prompt_version.as_deref(), Some("qual_assessment_v1"));
    assert!(
        result
            .citations
            .as_deref()
            .expect("citations present")
            .contains("evidenceType"),
        "citations JSON round-trips"
    );
}

#[test]
fn upgrade_backfills_quantitative_kind_and_engine_source() {
    // A database at v58 (pre-qualitative) with a quantitative framework, criterion,
    // evaluation, and result. Upgrading to the latest schema backfills the new
    // discriminators with safe defaults (kind='quantitative', source='engine') and
    // leaves the pre-existing verdict untouched.
    let mut connection = rusqlite::Connection::open_in_memory().expect("open in-memory database");
    crate::storage::migrations::apply_migrations_up_to(&mut connection, 58).expect("apply v58");
    connection
        .execute(
            "INSERT INTO companies (id, exchange, ticker, qualified_ticker, display_name)
             VALUES ('co','GPW','X','GPW:X','X Co')",
            [],
        )
        .expect("seed company at v58");
    connection
        .execute(
            "INSERT INTO quality_frameworks (id, name, origin, version) VALUES ('fw','F','user',1)",
            [],
        )
        .expect("seed framework at v58");
    connection
        .execute(
            "INSERT INTO framework_criteria (id, framework_id, ordinal, label, expression)
             VALUES ('cri','fw',0,'ROE','roe >= 15%')",
            [],
        )
        .expect("seed criterion at v58");
    connection
        .execute(
            "INSERT INTO framework_evaluations
                (id, framework_id, framework_version, company_id, period_id,
                 pass_count, partial_count, fail_count, unavailable_count, engine_version)
             VALUES ('ev','fw',1,'co',NULL,1,0,0,0,'v0')",
            [],
        )
        .expect("seed evaluation at v58");
    connection
        .execute(
            "INSERT INTO criterion_results
                (id, evaluation_id, criterion_id, ordinal, label, expression, verdict)
             VALUES ('cr','ev','cri',0,'ROE','roe >= 15%','pass')",
            [],
        )
        .expect("seed result at v58");

    crate::storage::migrations::apply_migrations(&mut connection).expect("upgrade to latest");

    let kind: String = connection
        .query_row(
            "SELECT kind FROM framework_criteria WHERE id='cri'",
            [],
            |r| r.get(0),
        )
        .expect("read kind");
    assert_eq!(
        kind, "quantitative",
        "existing criterion backfills to quantitative"
    );
    let source: String = connection
        .query_row(
            "SELECT source FROM criterion_results WHERE id='cr'",
            [],
            |r| r.get(0),
        )
        .expect("read source");
    assert_eq!(
        source, "engine",
        "existing result backfills to engine source"
    );
    let verdict: String = connection
        .query_row(
            "SELECT verdict FROM criterion_results WHERE id='cr'",
            [],
            |r| r.get(0),
        )
        .expect("read verdict");
    assert_eq!(
        verdict, "pass",
        "pre-existing quantitative verdict untouched"
    );
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

#[test]
fn every_canonical_derived_metric_is_computable_from_a_representative_fact_set() {
    // Regression guard for issue 674cb5a: ROIC/ROCE silently returned Unavailable
    // because their formulas referenced synthetic intermediates (nopat,
    // invested_capital, capital_employed) that were never seeded or resolved. The
    // grammar-drift gate above only proves a formula *parses* — it does not prove
    // its inputs *resolve*. This gate asserts that, given a full set of reported
    // canonical inputs, every canonical derived metric actually computes a value.
    use crate::fundamentals::expr::MetricResolver as _;
    use crate::fundamentals::metrics::{
        parse_formula, Computation, MetricDef, MetricsContext, PeriodFacts,
    };
    use rust_decimal::Decimal;
    use std::collections::HashMap;

    let connection = open_in_memory_database().expect("database should initialize");
    let state = AppState::new(connection);
    let definitions = state
        .list_kpi_definitions(ListKpiDefinitionsInput {
            scope: None,
            sector: None,
            company_id: None,
        })
        .expect("definitions");

    // One annual period in which every reported input carries a distinct nonzero
    // value, so no denominator is zero. Reported inputs of every scope are included
    // so a canonical formula never lacks a dependency.
    let mut defs: HashMap<String, MetricDef> = HashMap::new();
    let mut reported: HashMap<String, Decimal> = HashMap::new();
    let mut next: i64 = 100;
    for def in &definitions {
        let computation = if def.computation == "derived" {
            Computation::Derived
        } else {
            Computation::Reported
        };
        let formula = def
            .formula
            .clone()
            .filter(|f| !f.trim().is_empty())
            .and_then(|f| parse_formula(&f));
        defs.entry(def.metric_key.clone()).or_insert(MetricDef {
            computation,
            formula,
            value_kind: def.value_kind.clone(),
            unit: def.unit.clone(),
        });
        if def.computation == "reported" {
            next += 7;
            reported
                .entry(def.metric_key.clone())
                .or_insert(Decimal::from(next));
        }
    }

    let period = PeriodFacts {
        period_id: "period-representative".to_owned(),
        fiscal_year: 2024,
        period_type: "FY".to_owned(),
        reported,
    };
    let context = MetricsContext::new(defs, vec![period]);
    let resolver = context.resolver();

    for def in &definitions {
        if def.scope == "canonical" && def.computation == "derived" {
            assert!(
                resolver.value(&def.metric_key).is_some(),
                "canonical derived metric `{}` (formula {:?}) is not computable from a full canonical fact set",
                def.metric_key,
                def.formula
            );
        }
    }
}

#[test]
fn health_score_liquidity_metrics_derive_from_current_items() {
    // ADR 0083 Decision 5 (v0.57): the four reported health-score inputs are
    // seeded as canonical reported definitions, and the two liquidity derivations
    // (`working_capital`, `current_ratio`) compute from `current_assets` and
    // `current_liabilities` alone — nothing else. Reddens until migration 0089
    // seeds the rows (compare the ROIC/ROCE regression this guard-class caught).
    use crate::fundamentals::expr::MetricResolver as _;
    use crate::fundamentals::metrics::{
        parse_formula, Computation, MetricDef, MetricsContext, PeriodFacts,
    };
    use rust_decimal::Decimal;
    use std::collections::HashMap;

    let connection = open_in_memory_database().expect("database should initialize");
    let state = AppState::new(connection);
    let definitions = state
        .list_kpi_definitions(ListKpiDefinitionsInput {
            scope: None,
            sector: None,
            company_id: None,
        })
        .expect("definitions");

    // The four new reported inputs exist as canonical reported metrics.
    for key in [
        "current_assets",
        "current_liabilities",
        "retained_earnings",
        "long_term_debt",
    ] {
        let def = definitions
            .iter()
            .find(|d| d.metric_key == key)
            .unwrap_or_else(|| panic!("reported metric `{key}` should be seeded"));
        assert_eq!(def.scope, "canonical");
        assert_eq!(def.computation, "reported");
    }

    // Build a context with only the two current items reported, then prove both
    // liquidity derivations resolve from them (working_capital = 18000 - 9000,
    // current_ratio = 18000 / 9000 = 2).
    let mut defs: HashMap<String, MetricDef> = HashMap::new();
    for def in &definitions {
        let computation = if def.computation == "derived" {
            Computation::Derived
        } else {
            Computation::Reported
        };
        let formula = def
            .formula
            .clone()
            .filter(|f| !f.trim().is_empty())
            .and_then(|f| parse_formula(&f));
        defs.entry(def.metric_key.clone()).or_insert(MetricDef {
            computation,
            formula,
            value_kind: def.value_kind.clone(),
            unit: def.unit.clone(),
        });
    }
    let mut reported: HashMap<String, Decimal> = HashMap::new();
    reported.insert("current_assets".to_owned(), Decimal::from(18_000));
    reported.insert("current_liabilities".to_owned(), Decimal::from(9_000));
    let period = PeriodFacts {
        period_id: "period-liquidity".to_owned(),
        fiscal_year: 2025,
        period_type: "FY".to_owned(),
        reported,
    };
    let context = MetricsContext::new(defs, vec![period]);
    let resolver = context.resolver();

    assert_eq!(
        resolver.value("working_capital"),
        Some(Decimal::from(9_000)),
        "working_capital = current_assets - current_liabilities"
    );
    assert_eq!(
        resolver.value("current_ratio"),
        Some(Decimal::from(2)),
        "current_ratio = current_assets / current_liabilities"
    );
}

// ============================================================================
// U8a — bilingual template seeds + non-destructive top-up (ADR 0076 Decision 8)
// ============================================================================

const POLISH_DIACRITICS: &str = "ąćęłńóśźżĄĆĘŁŃÓŚŹŻ";
const KROEZE_FRAMEWORK_ID: &str = "qframework_kroeze_quality";

fn kroeze_at_locale(locale: &str) -> QualityFramework {
    let connection = open_in_memory_database().expect("database should initialize");
    connection
        .execute(
            "UPDATE settings SET value = ?1 WHERE key = 'locale'",
            [locale],
        )
        .expect("locale should update");
    let state = AppState::new(connection);
    state
        .list_quality_frameworks()
        .expect("frameworks should list")
        .into_iter()
        .find(|f| f.template_key.as_deref() == Some("kroeze_quality"))
        .expect("Kroeze template should be seeded")
}

fn count_framework_criteria(connection: &rusqlite::Connection, framework_id: &str) -> i64 {
    connection
        .query_row(
            "SELECT COUNT(*) FROM framework_criteria WHERE framework_id = ?1",
            [framework_id],
            |row| row.get(0),
        )
        .expect("criteria count")
}

#[test]
fn seeds_bilingual_template_by_locale() {
    let pl = kroeze_at_locale("pl");
    let en = kroeze_at_locale("en");

    // The localized framework name differs by locale.
    assert_ne!(pl.name, en.name, "the template name should be localized");

    // Keys/kinds/expressions stay locale-independent; only the human-facing
    // label + guidance are localized.
    assert_eq!(pl.criteria.len(), en.criteria.len());
    for (p, e) in pl.criteria.iter().zip(en.criteria.iter()) {
        assert_eq!(p.ordinal, e.ordinal, "ordinal is locale-independent");
        assert_eq!(p.kind, e.kind, "kind is locale-independent");
        assert_eq!(
            p.expression, e.expression,
            "expression (the criterion key) is locale-independent"
        );
    }

    // The Polish seed actually carries Polish text.
    let has_polish = pl.name.chars().any(|c| POLISH_DIACRITICS.contains(c))
        || pl
            .criteria
            .iter()
            .any(|c| c.label.chars().any(|c| POLISH_DIACRITICS.contains(c)));
    assert!(has_polish, "the pl seed should contain Polish characters");

    // At least one label actually differs between locales.
    assert!(
        pl.criteria
            .iter()
            .zip(en.criteria.iter())
            .any(|(p, e)| p.label != e.label),
        "labels should be localized"
    );
}

#[test]
fn tops_up_missing_template_criteria_idempotently() {
    let connection = open_in_memory_database().expect("database should initialize");
    crate::storage::quality_frameworks::seed_templates(&connection).expect("initial seed");
    let total = count_framework_criteria(&connection, KROEZE_FRAMEWORK_ID);
    assert!(total > 0, "the template should seed criteria");

    // Simulate a pre-upgrade install whose qualitative criteria did not exist
    // yet. A raw delete does not bump the version (still an untouched template).
    connection
        .execute(
            "DELETE FROM framework_criteria WHERE framework_id = ?1 AND kind = 'qualitative'",
            [KROEZE_FRAMEWORK_ID],
        )
        .expect("remove qualitative criteria");
    let reduced = count_framework_criteria(&connection, KROEZE_FRAMEWORK_ID);
    assert!(reduced < total, "the qualitative criteria should be gone");

    // Startup top-up restores exactly the missing criteria.
    crate::storage::quality_frameworks::seed_templates(&connection).expect("top-up seed");
    assert_eq!(
        count_framework_criteria(&connection, KROEZE_FRAMEWORK_ID),
        total,
        "top-up should add the missing template criteria"
    );

    // Idempotent: a second top-up adds nothing.
    crate::storage::quality_frameworks::seed_templates(&connection).expect("idempotent top-up");
    assert_eq!(
        count_framework_criteria(&connection, KROEZE_FRAMEWORK_ID),
        total
    );

    // Top-up is not a user edit — the version stays 1 so future template growth
    // also tops up.
    let version: i64 = connection
        .query_row(
            "SELECT version FROM quality_frameworks WHERE id = ?1",
            [KROEZE_FRAMEWORK_ID],
            |row| row.get(0),
        )
        .expect("framework version");
    assert_eq!(version, 1, "top-up must not bump the framework version");
}

#[test]
fn top_up_skips_edited_and_user_frameworks() {
    let connection = open_in_memory_database().expect("database should initialize");
    crate::storage::quality_frameworks::seed_templates(&connection).expect("initial seed");

    // A user *edited* the template: version bumped past 1 and some criteria gone.
    connection
        .execute(
            "DELETE FROM framework_criteria WHERE framework_id = ?1 AND kind = 'qualitative'",
            [KROEZE_FRAMEWORK_ID],
        )
        .expect("remove criteria");
    connection
        .execute(
            "UPDATE quality_frameworks SET version = 2 WHERE id = ?1",
            [KROEZE_FRAMEWORK_ID],
        )
        .expect("bump version");
    let edited_count = count_framework_criteria(&connection, KROEZE_FRAMEWORK_ID);

    // A user-created framework (origin user, no template_key).
    connection
        .execute(
            "INSERT INTO quality_frameworks (id, name, description, origin, version)
             VALUES ('user_fw', 'Mine', NULL, 'user', 1)",
            [],
        )
        .expect("create user framework");
    connection
        .execute(
            "INSERT INTO framework_criteria (id, framework_id, ordinal, label, expression, kind)
             VALUES ('user_c0', 'user_fw', 0, 'x', 'roe > 0', 'quantitative')",
            [],
        )
        .expect("create user criterion");

    // Re-run startup seeding.
    crate::storage::quality_frameworks::seed_templates(&connection).expect("re-seed");

    assert_eq!(
        count_framework_criteria(&connection, KROEZE_FRAMEWORK_ID),
        edited_count,
        "an edited (version > 1) template framework must not be topped up"
    );
    assert_eq!(
        count_framework_criteria(&connection, "user_fw"),
        1,
        "a user-created framework must not be topped up"
    );
}

#[test]
fn reset_reseeds_criteria_in_current_locale() {
    let connection = open_in_memory_database().expect("database should initialize");
    // Default locale is en (seeded by migration 0022).
    let state = AppState::new(connection);
    let kroeze = state
        .list_quality_frameworks()
        .expect("list")
        .into_iter()
        .find(|f| f.template_key.as_deref() == Some("kroeze_quality"))
        .expect("kroeze seeded");
    let en_label = kroeze.criteria[0].label.clone();

    // Switch the app locale to Polish, then reset the framework.
    state
        .update_settings(SettingsUpdate {
            locale: Some("pl".to_owned()),
            ..Default::default()
        })
        .expect("switch locale to pl");
    let reset = state
        .reset_framework_to_template(&kroeze.id)
        .expect("reset");

    assert_ne!(
        reset.criteria[0].label, en_label,
        "reset should re-seed criteria in the now-current (pl) locale"
    );
    assert!(
        reset
            .criteria
            .iter()
            .any(|c| c.label.chars().any(|ch| POLISH_DIACRITICS.contains(ch))),
        "reset criteria should carry Polish text under the pl locale"
    );
}

// ---------------------------------------------------------------------------
// baba638 — untouched template frameworks auto-relocalize on startup
// ---------------------------------------------------------------------------

fn framework_string(connection: &rusqlite::Connection, sql: &str) -> String {
    connection
        .query_row(sql, [KROEZE_FRAMEWORK_ID], |row| row.get(0))
        .expect("query framework/criterion string")
}

const NAME_SQL: &str = "SELECT name FROM quality_frameworks WHERE id = ?1";
const LABEL0_SQL: &str =
    "SELECT label FROM framework_criteria WHERE framework_id = ?1 AND ordinal = 0";
const GUIDANCE8_SQL: &str =
    "SELECT assessment_guidance FROM framework_criteria WHERE framework_id = ?1 AND ordinal = 8";

#[test]
fn relocalizes_untouched_template_on_locale_switch() {
    // A framework seeded before the bilingual pass keeps its seed-time locale's
    // English strings. Switching the app locale must auto-relocalize the untouched
    // (app_template, version == 1) template's name/description/label/guidance.
    let connection = open_in_memory_database().expect("database should initialize");
    // Seed under the default (en) locale — reproduces the pre-bilingual state.
    crate::storage::quality_frameworks::seed_templates(&connection).expect("initial en seed");
    let en_name = framework_string(&connection, NAME_SQL);
    let en_label = framework_string(&connection, LABEL0_SQL);
    let en_guidance = framework_string(&connection, GUIDANCE8_SQL);

    // The user switches the app locale to Polish.
    connection
        .execute("UPDATE settings SET value = 'pl' WHERE key = 'locale'", [])
        .expect("switch locale to pl");

    // Startup re-runs seeding: the untouched template auto-relocalizes.
    crate::storage::quality_frameworks::seed_templates(&connection).expect("relocalize seed");

    let pl_name = framework_string(&connection, NAME_SQL);
    let pl_label = framework_string(&connection, LABEL0_SQL);
    let pl_guidance = framework_string(&connection, GUIDANCE8_SQL);

    assert_ne!(
        pl_name, en_name,
        "the framework name should relocalize to pl"
    );
    assert_ne!(
        pl_label, en_label,
        "the criterion label should relocalize to pl"
    );
    assert_ne!(
        pl_guidance, en_guidance,
        "the criterion guidance should relocalize to pl"
    );
    assert!(
        pl_name.chars().any(|c| POLISH_DIACRITICS.contains(c))
            || pl_label.chars().any(|c| POLISH_DIACRITICS.contains(c))
            || pl_guidance.chars().any(|c| POLISH_DIACRITICS.contains(c)),
        "relocalized text should carry Polish characters"
    );

    // Relocalization is a self-heal, not a user edit: the version stays 1.
    let version: i64 = connection
        .query_row(
            "SELECT version FROM quality_frameworks WHERE id = ?1",
            [KROEZE_FRAMEWORK_ID],
            |row| row.get(0),
        )
        .expect("framework version");
    assert_eq!(
        version, 1,
        "relocalization must not bump the framework version"
    );

    // Idempotent: a second startup under the same locale rewrites the same text.
    crate::storage::quality_frameworks::seed_templates(&connection).expect("idempotent relocalize");
    assert_eq!(framework_string(&connection, NAME_SQL), pl_name);
    assert_eq!(framework_string(&connection, LABEL0_SQL), pl_label);
}

#[test]
fn relocalize_skips_edited_framework() {
    // A user-edited template (version > 1) must never be relocalized — that would
    // destroy the user's own strings.
    let connection = open_in_memory_database().expect("database should initialize");
    crate::storage::quality_frameworks::seed_templates(&connection).expect("initial en seed");
    let en_label = framework_string(&connection, LABEL0_SQL);

    connection
        .execute(
            "UPDATE quality_frameworks SET version = 2 WHERE id = ?1",
            [KROEZE_FRAMEWORK_ID],
        )
        .expect("mark framework edited");
    connection
        .execute("UPDATE settings SET value = 'pl' WHERE key = 'locale'", [])
        .expect("switch locale to pl");

    crate::storage::quality_frameworks::seed_templates(&connection).expect("re-seed");

    assert_eq!(
        framework_string(&connection, LABEL0_SQL),
        en_label,
        "an edited (version > 1) framework must not be relocalized"
    );
}

#[test]
fn relocalize_preserves_non_template_field_values() {
    // Field-level guard: even on an untouched (version == 1) template, a field
    // whose value is not a shipped template string in any locale is left alone —
    // relocalization only rewrites recognisably-template text.
    let connection = open_in_memory_database().expect("database should initialize");
    crate::storage::quality_frameworks::seed_templates(&connection).expect("initial en seed");

    connection
        .execute(
            "UPDATE framework_criteria SET label = 'My custom label'
             WHERE framework_id = ?1 AND ordinal = 0",
            [KROEZE_FRAMEWORK_ID],
        )
        .expect("customize a criterion label");
    connection
        .execute("UPDATE settings SET value = 'pl' WHERE key = 'locale'", [])
        .expect("switch locale to pl");

    crate::storage::quality_frameworks::seed_templates(&connection).expect("relocalize seed");

    assert_eq!(
        framework_string(&connection, LABEL0_SQL),
        "My custom label",
        "a non-template field value must not be relocalized"
    );
}
