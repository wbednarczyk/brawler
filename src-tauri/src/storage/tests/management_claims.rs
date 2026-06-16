use super::*;

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

#[test]
fn creates_lists_and_defaults_claim() {
    let connection = open_in_memory_database().expect("database should initialize");
    let state = AppState::new(connection);
    let company = sample_company(&state);

    let claim = state
        .create_management_claim(NewManagementClaim {
            company_id: company.id.clone(),
            statement: "Management expects the next release within two quarters.".to_owned(),
            due_fiscal_year: Some(2026),
            due_period_type: Some("q4".to_owned()),
            source_evidence_type: Some("transcript_segment".to_owned()),
            source_evidence_id: Some("seg_42".to_owned()),
            ..Default::default()
        })
        .expect("claim should be created");

    assert_eq!(claim.status, "pending");
    assert_eq!(claim.body_format, "markdown");
    assert_eq!(claim.due_fiscal_year, Some(2026));
    assert_eq!(claim.due_period_type.as_deref(), Some("Q4"));
    assert_eq!(claim.source_evidence_type, "transcript_segment");

    let claims = state
        .list_management_claims(&company.id)
        .expect("claims should list");
    assert_eq!(claims.len(), 1);
    assert_eq!(claims[0].id, claim.id);
}

#[test]
fn updates_claim_fields_and_preserves_company() {
    let connection = open_in_memory_database().expect("database should initialize");
    let state = AppState::new(connection);
    let company = sample_company(&state);

    let claim = state
        .create_management_claim(NewManagementClaim {
            company_id: company.id.clone(),
            statement: "Original statement".to_owned(),
            ..Default::default()
        })
        .expect("claim should be created");

    let updated = state
        .update_management_claim(ManagementClaimUpdate {
            id: claim.id.clone(),
            statement: Some("Revised statement".to_owned()),
            due_fiscal_year: Some(2027),
            due_period_type: Some("H1".to_owned()),
            target_metric_key: Some("net_revenue".to_owned()),
            target_comparator: Some("gte".to_owned()),
            target_value_numeric: Some("1000000".to_owned()),
            ..Default::default()
        })
        .expect("claim should update");

    assert_eq!(updated.statement, "Revised statement");
    assert_eq!(updated.company_id, company.id);
    assert_eq!(updated.due_fiscal_year, Some(2027));
    assert_eq!(updated.due_period_type.as_deref(), Some("H1"));
    assert_eq!(updated.target_metric_key.as_deref(), Some("net_revenue"));
    assert_eq!(updated.target_comparator.as_deref(), Some("gte"));
}

#[test]
fn sets_verdict_and_rejects_invalid_status() {
    let connection = open_in_memory_database().expect("database should initialize");
    let state = AppState::new(connection);
    let company = sample_company(&state);

    let claim = state
        .create_management_claim(NewManagementClaim {
            company_id: company.id.clone(),
            statement: "Claim to resolve".to_owned(),
            ..Default::default()
        })
        .expect("claim should be created");

    let resolved = state
        .set_claim_verdict(SetClaimVerdictInput {
            claim_id: claim.id.clone(),
            status: "delivered".to_owned(),
            verifying_fact_id: None,
            verifying_relation: None,
            revises_claim_id: None,
        })
        .expect("verdict should set");
    assert_eq!(resolved.status, "delivered");

    let invalid = state.set_claim_verdict(SetClaimVerdictInput {
        claim_id: claim.id.clone(),
        status: "shipped".to_owned(),
        verifying_fact_id: None,
        verifying_relation: None,
        revises_claim_id: None,
    });
    assert!(matches!(
        invalid,
        Err(StorageError::InvalidClaimValue { key: "status", .. })
    ));
}

#[test]
fn rejects_claim_for_unknown_company() {
    let connection = open_in_memory_database().expect("database should initialize");
    let state = AppState::new(connection);

    let result = state.create_management_claim(NewManagementClaim {
        company_id: "company_missing".to_owned(),
        statement: "Orphan claim".to_owned(),
        ..Default::default()
    });
    assert!(matches!(
        result,
        Err(StorageError::MissingClaimReference { .. })
    ));
}

#[test]
fn deletes_claim() {
    let connection = open_in_memory_database().expect("database should initialize");
    let state = AppState::new(connection);
    let company = sample_company(&state);

    let claim = state
        .create_management_claim(NewManagementClaim {
            company_id: company.id.clone(),
            statement: "Disposable claim".to_owned(),
            ..Default::default()
        })
        .expect("claim should be created");

    state
        .delete_management_claim(&claim.id)
        .expect("claim should delete");
    assert!(state
        .list_management_claims(&company.id)
        .expect("claims should list")
        .is_empty());
    assert!(matches!(
        state.delete_management_claim(&claim.id),
        Err(StorageError::MissingClaimReference { .. })
    ));
}

#[test]
fn claim_resurfaces_when_due_period_report_arrives_and_resolves_with_evidence() {
    // The milestone exit criterion (ADR 0040): a claim with a due period resurfaces in
    // the review queue when the due-period report arrives, and can be resolved with a
    // verdict linked to the verifying financial fact.
    let connection = open_in_memory_database().expect("database should initialize");
    let state = AppState::new(connection);
    let company = sample_company(&state);

    let claim = state
        .create_management_claim(NewManagementClaim {
            company_id: company.id.clone(),
            statement: "Net revenue will reach at least 1,000,000 by FY2026 Q4.".to_owned(),
            due_fiscal_year: Some(2026),
            due_period_type: Some("Q4".to_owned()),
            target_metric_key: Some("net_revenue".to_owned()),
            target_comparator: Some("gte".to_owned()),
            target_value_numeric: Some("1000000".to_owned()),
            ..Default::default()
        })
        .expect("claim should be created");

    // Before the report arrives, the claim is upcoming, not yet verifiable.
    let before = state
        .list_claims_to_verify(&company.id)
        .expect("queue should compute");
    assert_eq!(before.upcoming.len(), 1);
    assert!(before.due.is_empty());
    assert!(before.overdue.is_empty());

    // The due-period report arrives: a financial period for 2026 Q4 is created.
    let period = state
        .create_financial_period(NewFinancialPeriod {
            company_id: company.id.clone(),
            fiscal_year: 2026,
            period_type: "Q4".to_owned(),
            period_end_date: Some("2026-12-31".to_owned()),
            report_evidence_ref: Some("report_2026_q4".to_owned()),
        })
        .expect("financial period should create");

    // The claim now resurfaces in the `due` bucket with the arrived period.
    let after_arrival = state
        .list_claims_to_verify(&company.id)
        .expect("queue should compute");
    assert_eq!(
        after_arrival.due.len(),
        1,
        "claim resurfaces when report arrives"
    );
    assert!(after_arrival.upcoming.is_empty());
    let due_entry = &after_arrival.due[0];
    assert_eq!(due_entry.claim.id, claim.id);
    assert_eq!(
        due_entry.arrived_period_id.as_deref(),
        Some(period.id.as_str())
    );
    // No confirmed fact yet, so no candidate.
    assert!(due_entry.verifying_fact_candidate.is_none());

    // A confirmed fact for the metric/period becomes the verification candidate.
    let definition = state
        .create_kpi_definition(NewKpiDefinition {
            scope: "company".to_owned(),
            company_id: Some(company.id.clone()),
            sector: None,
            metric_key: "net_revenue".to_owned(),
            label: "Net revenue".to_owned(),
            value_kind: "monetary".to_owned(),
            unit: Some("PLN".to_owned()),
            computation: "reported".to_owned(),
            formula: None,
            display_format: None,
        })
        .expect("definition should create");
    let fact = state
        .create_financial_fact(NewFinancialFact {
            company_id: company.id.clone(),
            period_id: period.id.clone(),
            definition_id: definition.id.clone(),
            value_numeric: "1250000".to_owned(),
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
            source_document_ref: None,
        })
        .expect("fact should create");

    let with_candidate = state
        .list_claims_to_verify(&company.id)
        .expect("queue should compute");
    let candidate = with_candidate.due[0]
        .verifying_fact_candidate
        .as_ref()
        .expect("confirmed fact becomes the verification candidate");
    assert_eq!(candidate.fact_id, fact.id);
    assert_eq!(candidate.value_numeric, "1250000");

    // The user resolves the claim with a verdict linked to the fact.
    let resolved = state
        .set_claim_verdict(SetClaimVerdictInput {
            claim_id: claim.id.clone(),
            status: "delivered".to_owned(),
            verifying_fact_id: Some(fact.id.clone()),
            verifying_relation: Some("supports".to_owned()),
            revises_claim_id: None,
        })
        .expect("verdict should set");
    assert_eq!(resolved.status, "delivered");
    assert_eq!(
        resolved.verifying_fact_id.as_deref(),
        Some(fact.id.as_str())
    );

    // A resolved claim leaves the review queue.
    let after_resolution = state
        .list_claims_to_verify(&company.id)
        .expect("queue should compute");
    assert!(after_resolution.due.is_empty());
    assert!(after_resolution.upcoming.is_empty());
    assert!(after_resolution.overdue.is_empty());
}

#[test]
fn migration_0045_converges_legacy_claim_notes_idempotently() {
    let connection = open_in_memory_database().expect("database should initialize");
    let state = AppState::new(connection);
    let company = sample_company(&state);

    // A legacy claim note created the old way is not yet in management_claims
    // (migration 0045 already ran at init, before this row existed).
    let note = state
        .create_notebook_entry(NewNotebookEntry {
            company_id: company.id.clone(),
            title: "Legacy management claim".to_owned(),
            body: "Board said the dividend should be raised next year.".to_owned(),
            body_format: None,
            tags: vec![],
            kind: "claim".to_owned(),
            claim_status: Some("delivered".to_owned()),
            event_date: Some("2026-03-01".to_owned()),
            follow_up_after: Some("2026-Q4".to_owned()),
            follow_up_date: None,
            origins: vec![],
        })
        .expect("legacy claim note should be created");
    assert!(state
        .list_management_claims(&company.id)
        .expect("claims should list")
        .is_empty());

    // Re-run the forward migration twice: it must converge the legacy note and be idempotent.
    {
        let connection = state.checkout().expect("connection should check out");
        let sql = include_str!("../../../migrations/0045_management_claims.sql");
        connection
            .execute_batch(sql)
            .expect("migration should apply");
        connection
            .execute_batch(sql)
            .expect("migration should be idempotent");
    }

    let claims = state
        .list_management_claims(&company.id)
        .expect("claims should list");
    assert_eq!(claims.len(), 1, "legacy note converges exactly once");
    let migrated = &claims[0];
    assert_eq!(migrated.id, note.id, "claim keeps the originating note id");
    assert_eq!(migrated.statement, "Legacy management claim");
    assert_eq!(migrated.status, "delivered");
    assert_eq!(migrated.due_fiscal_year, Some(2026));
    assert_eq!(migrated.due_period_type.as_deref(), Some("Q4"));
}
