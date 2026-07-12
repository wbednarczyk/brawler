use super::*;
use proptest::prelude::*;

fn sample_company(state: &AppState) -> Company {
    state
        .create_company(NewCompany {
            exchange: "GPW".to_owned(),
            ticker: "CDR".to_owned(),
            display_name: "CD PROJEKT S.A.".to_owned(),
            isin: Some("PLOPTTC00011".to_owned()),
            cik: None,
            lei: None,
        })
        .expect("company should be created")
}

fn expectation(company_id: &str, event_key: &str) -> NewReportExpectation {
    NewReportExpectation {
        company_id: company_id.to_owned(),
        event_key: event_key.to_owned(),
        fiscal_year: 2026,
        period_type: "H1".to_owned(),
        stance_md: "Expecting **margin recovery** on the game launch.".to_owned(),
        metrics: Vec::new(),
    }
}

/// Seed a financial period + one fact for `(company, 2026, H1)` through the raw
/// connection — the moment the occurrence's facts exist, expectations freeze.
fn seed_confirmed_facts(state: &AppState, company_id: &str) {
    let raw = state.checkout_for_tests().expect("raw connection");
    raw.execute(
        "INSERT INTO financial_periods (id, company_id, fiscal_year, period_type)
         VALUES ('p_h1', ?1, 2026, 'H1')",
        [company_id],
    )
    .expect("seed period");
    raw.execute(
        "INSERT INTO financial_facts (id, company_id, period_id, definition_id, value_numeric)
         VALUES ('f_np', ?1, 'p_h1', 'kpidef_net_profit', '120')",
        [company_id],
    )
    .expect("seed fact");
}

#[test]
fn create_update_round_trip() {
    let connection = open_in_memory_database().expect("database should initialize");
    let state = AppState::new(connection);
    let company = sample_company(&state);
    let store = state.report_expectations();

    let created = store
        .create_report_expectation(expectation(&company.id, "evt-h1-2026"))
        .expect("expectation should be created");
    assert_eq!(created.company_id, company.id);
    assert_eq!(created.event_key, "evt-h1-2026");
    assert_eq!(created.fiscal_year, 2026);
    assert_eq!(created.period_type, "H1");
    assert!(created.frozen_at.is_none());
    assert!(created.resolved_at.is_none());
    assert!(created.resolution_note_md.is_none());

    // Editable while no facts exist for the occurrence's period.
    let updated = store
        .update_report_expectation(UpdateReportExpectation {
            company_id: company.id.clone(),
            event_key: "evt-h1-2026".to_owned(),
            stance_md: Some("Revised: **flat revenue**, better cash.".to_owned()),
            metrics: None,
        })
        .expect("update should succeed before facts arrive");
    assert_eq!(updated.stance_md, "Revised: **flat revenue**, better cash.");
    assert_eq!(updated.id, created.id);

    let listed = store
        .list_report_expectations(ListReportExpectationsInput {
            company_id: Some(company.id.clone()),
        })
        .expect("list");
    assert_eq!(listed.len(), 1);
    assert_eq!(
        listed[0].stance_md,
        "Revised: **flat revenue**, better cash."
    );
}

#[test]
fn metrics_child_rows_round_trip() {
    let connection = open_in_memory_database().expect("database should initialize");
    let state = AppState::new(connection);
    let company = sample_company(&state);
    let store = state.report_expectations();

    let mut input = expectation(&company.id, "evt-h1-2026");
    input.metrics = vec![
        NewExpectationMetric {
            metric_key: "revenue".to_owned(),
            comparator: "gte".to_owned(),
            expected_value: "250000000".to_owned(),
            unit: Some("PLN".to_owned()),
        },
        NewExpectationMetric {
            metric_key: "net_profit".to_owned(),
            comparator: "gt".to_owned(),
            expected_value: "40000000".to_owned(),
            unit: None,
        },
    ];
    let created = store
        .create_report_expectation(input)
        .expect("expectation with metrics should be created");
    assert_eq!(created.metrics.len(), 2);
    assert_eq!(created.metrics[0].metric_key, "revenue");
    assert_eq!(created.metrics[0].comparator, "gte");
    assert_eq!(created.metrics[0].expected_value, "250000000");
    assert_eq!(created.metrics[0].unit.as_deref(), Some("PLN"));

    // An update with metric rows replaces the child set wholesale.
    let updated = store
        .update_report_expectation(UpdateReportExpectation {
            company_id: company.id.clone(),
            event_key: "evt-h1-2026".to_owned(),
            stance_md: None,
            metrics: Some(vec![NewExpectationMetric {
                metric_key: "revenue".to_owned(),
                comparator: "lt".to_owned(),
                expected_value: "200000000".to_owned(),
                unit: Some("PLN".to_owned()),
            }]),
        })
        .expect("metric replacement should succeed");
    assert_eq!(updated.metrics.len(), 1);
    assert_eq!(updated.metrics[0].comparator, "lt");
    // Stance untouched when not supplied.
    assert_eq!(
        updated.stance_md,
        "Expecting **margin recovery** on the game launch."
    );
}

#[test]
fn update_blocked_once_facts_confirmed_for_period() {
    let connection = open_in_memory_database().expect("database should initialize");
    let state = AppState::new(connection);
    let company = sample_company(&state);
    let store = state.report_expectations();

    store
        .create_report_expectation(expectation(&company.id, "evt-h1-2026"))
        .expect("create");

    // The report lands: facts for (2026, H1) are recorded.
    seed_confirmed_facts(&state, &company.id);

    let result = store.update_report_expectation(UpdateReportExpectation {
        company_id: company.id.clone(),
        event_key: "evt-h1-2026".to_owned(),
        stance_md: Some("hindsight rewrite".to_owned()),
        metrics: None,
    });
    assert!(
        matches!(result, Err(StorageError::ReportExpectationFrozen { .. })),
        "an update after facts exist must be a typed frozen refusal, got: {result:?}"
    );

    // The refusal stamped frozen_at and left the content untouched.
    let listed = store
        .list_report_expectations(ListReportExpectationsInput {
            company_id: Some(company.id.clone()),
        })
        .expect("list");
    assert!(
        listed[0].frozen_at.is_some(),
        "frozen_at must be stamped on first observation"
    );
    assert_eq!(
        listed[0].stance_md, "Expecting **margin recovery** on the game launch.",
        "the refused update must not change the stance"
    );
}

#[test]
fn freeze_stamp_is_idempotent() {
    let connection = open_in_memory_database().expect("database should initialize");
    let state = AppState::new(connection);
    let company = sample_company(&state);
    let store = state.report_expectations();

    store
        .create_report_expectation(expectation(&company.id, "evt-h1-2026"))
        .expect("create");
    seed_confirmed_facts(&state, &company.id);

    // First read observes the confirmed facts and stamps frozen_at.
    let first = store
        .list_report_expectations(ListReportExpectationsInput {
            company_id: Some(company.id.clone()),
        })
        .expect("first list");
    let stamped = first[0].frozen_at.clone().expect("frozen_at stamped");

    // A later read must keep the ORIGINAL stamp, not re-stamp.
    let second = store
        .list_report_expectations(ListReportExpectationsInput {
            company_id: Some(company.id.clone()),
        })
        .expect("second list");
    assert_eq!(
        second[0].frozen_at.as_deref(),
        Some(stamped.as_str()),
        "frozen_at must be stamped exactly once"
    );
}

#[test]
fn resolution_note_recorded() {
    let connection = open_in_memory_database().expect("database should initialize");
    let state = AppState::new(connection);
    let company = sample_company(&state);
    let store = state.report_expectations();

    store
        .create_report_expectation(expectation(&company.id, "evt-h1-2026"))
        .expect("create");
    seed_confirmed_facts(&state, &company.id);

    // The user's own verdict at review time — allowed after the freeze.
    let resolved = store
        .record_expectation_resolution(RecordExpectationResolutionInput {
            company_id: company.id.clone(),
            event_key: "evt-h1-2026".to_owned(),
            resolution_note_md: "Margin recovered, revenue missed my bar.".to_owned(),
        })
        .expect("resolution should be recordable after the freeze");
    assert_eq!(
        resolved.resolution_note_md.as_deref(),
        Some("Margin recovered, revenue missed my bar.")
    );
    assert!(resolved.resolved_at.is_some());
}

#[test]
fn unique_occurrence_conflict() {
    let connection = open_in_memory_database().expect("database should initialize");
    let state = AppState::new(connection);
    let company = sample_company(&state);
    let store = state.report_expectations();

    store
        .create_report_expectation(expectation(&company.id, "evt-h1-2026"))
        .expect("first create");
    let duplicate = store.create_report_expectation(expectation(&company.id, "evt-h1-2026"));
    assert!(
        duplicate.is_err(),
        "a second expectation for the same (company, event_key) occurrence must be rejected"
    );
}

#[test]
fn expectation_review_reflects_confirmed_facts() {
    use MetricExpectationOutcome::{Met, Missed, Unknown};
    let connection = open_in_memory_database().expect("database should initialize");
    let state = AppState::new(connection);
    let company = sample_company(&state);
    let store = state.report_expectations();

    // Three metric rows against the H1 2026 occurrence: one the actual meets, one
    // it misses, one whose metric has no confirmed fact (→ unknown).
    let mut input = expectation(&company.id, "evt-h1-2026");
    input.metrics = vec![
        NewExpectationMetric {
            metric_key: "net_profit".to_owned(),
            comparator: "gte".to_owned(),
            expected_value: "100".to_owned(),
            unit: None,
        },
        NewExpectationMetric {
            metric_key: "net_profit".to_owned(),
            comparator: "lt".to_owned(),
            expected_value: "100".to_owned(),
            unit: None,
        },
        NewExpectationMetric {
            metric_key: "roe".to_owned(),
            comparator: "gte".to_owned(),
            expected_value: "10".to_owned(),
            unit: None,
        },
    ];
    store.create_report_expectation(input).expect("create");

    // The report lands: net_profit = 120 confirmed for (2026, H1); no roe fact.
    seed_confirmed_facts(&state, &company.id);

    let review = store
        .expectation_review(&company.id, "evt-h1-2026")
        .expect("review composes");

    assert_eq!(review.company_id, company.id);
    assert_eq!(review.event_key, "evt-h1-2026");
    assert!(review.facts_available, "facts exist for the period");
    assert!(
        review.frozen_at.is_some(),
        "reading the review after facts land freezes the expectation"
    );
    assert_eq!(review.metrics.len(), 3);
    // Row order preserved from insertion (list_metrics ORDER BY id).
    assert_eq!(review.metrics[0].outcome, Met);
    assert_eq!(review.metrics[0].actual_value.as_deref(), Some("120"));
    assert_eq!(review.metrics[1].outcome, Missed);
    assert_eq!(review.metrics[1].actual_value.as_deref(), Some("120"));
    assert_eq!(
        review.metrics[2].outcome, Unknown,
        "no confirmed fact for roe → mirror unknown, never guess"
    );
    assert!(review.metrics[2].actual_value.is_none());
}

#[test]
fn expectation_review_before_facts_is_unknown() {
    let connection = open_in_memory_database().expect("database should initialize");
    let state = AppState::new(connection);
    let company = sample_company(&state);
    let store = state.report_expectations();

    let mut input = expectation(&company.id, "evt-h1-2026");
    input.metrics = vec![NewExpectationMetric {
        metric_key: "net_profit".to_owned(),
        comparator: "gte".to_owned(),
        expected_value: "100".to_owned(),
        unit: None,
    }];
    store.create_report_expectation(input).expect("create");

    let review = store
        .expectation_review(&company.id, "evt-h1-2026")
        .expect("review composes even before facts land");
    assert!(!review.facts_available);
    assert!(review.frozen_at.is_none(), "no facts → not frozen");
    assert_eq!(review.metrics.len(), 1);
    assert_eq!(review.metrics[0].outcome, MetricExpectationOutcome::Unknown);
    assert!(review.metrics[0].actual_value.is_none());
}

#[test]
fn comparator_evaluator_known_cases() {
    use MetricExpectationOutcome::{Met, Missed, Unknown};

    assert_eq!(evaluate_metric_expectation("gte", "100", "120"), Met);
    assert_eq!(evaluate_metric_expectation("gte", "100", "100"), Met);
    assert_eq!(evaluate_metric_expectation("gte", "100", "99.5"), Missed);
    assert_eq!(evaluate_metric_expectation("lt", "0", "-0.01"), Met);
    assert_eq!(evaluate_metric_expectation("lte", "5", "5"), Met);
    assert_eq!(evaluate_metric_expectation("eq", "5", "5.0"), Met);
    assert_eq!(evaluate_metric_expectation("gt", "5", "5"), Missed);
    // Unparseable / unknown inputs never panic and never guess.
    assert_eq!(evaluate_metric_expectation("gte", "NaN", "100"), Unknown);
    assert_eq!(evaluate_metric_expectation("gte", "100", "inf"), Unknown);
    assert_eq!(evaluate_metric_expectation("gte", "", "100"), Unknown);
    assert_eq!(evaluate_metric_expectation("between", "1", "2"), Unknown);
}

proptest! {
    /// The evaluator is TOTAL: any comparator string and any f64-shaped values
    /// (including NaN/inf/huge magnitudes that overflow Decimal) return an
    /// outcome instead of panicking.
    #[test]
    fn comparator_evaluator_is_total(
        expected in proptest::num::f64::ANY,
        actual in proptest::num::f64::ANY,
        comparator in "\\PC*",
    ) {
        let _ = evaluate_metric_expectation(
            &comparator,
            &expected.to_string(),
            &actual.to_string(),
        );
    }

    /// lt/gte duality: over the same inputs, `lt` and `gte` are exact
    /// complements when the values parse, and agree on Unknown when they don't.
    #[test]
    fn comparator_lt_gte_duality(
        expected in proptest::num::f64::ANY,
        actual in proptest::num::f64::ANY,
    ) {
        use MetricExpectationOutcome::{Met, Missed, Unknown};
        let e = expected.to_string();
        let a = actual.to_string();
        let lt = evaluate_metric_expectation("lt", &e, &a);
        let gte = evaluate_metric_expectation("gte", &e, &a);
        match lt {
            Met => prop_assert_eq!(gte, Missed),
            Missed => prop_assert_eq!(gte, Met),
            Unknown => prop_assert_eq!(gte, Unknown),
        }
    }
}
