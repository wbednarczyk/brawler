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

fn report_event(state: &AppState, company: &Company, date: &str, key: &str, fetched_at: &str) {
    state
        .create_company_event(NewCompanyEvent {
            company_id: company.id.clone(),
            event_type: "periodic_report".to_owned(),
            title: format!("Raport okresowy {date}"),
            event_date: date.to_owned(),
            event_time: None,
            status: Some("scheduled".to_owned()),
            source_type: Some("official_calendar".to_owned()),
            source_adapter_id: Some("bankier-kalendarium-html".to_owned()),
            source_event_key: Some(key.to_owned()),
            source_url: None,
            attribution: None,
            fetched_at: Some(fetched_at.to_owned()),
        })
        .expect("report event should be created");
}

#[test]
fn calendar_splits_upcoming_and_past_and_overlays_preparation_status() {
    let connection = open_in_memory_database().expect("database should initialize");
    let state = AppState::new(connection);
    let company = sample_company(&state);

    report_event(
        &state,
        &company,
        "2099-08-29",
        "evt-upcoming",
        "2099-08-01T08:00:00Z",
    );
    report_event(
        &state,
        &company,
        "2020-03-15",
        "evt-past",
        "2020-03-01T08:00:00Z",
    );

    let season = state
        .list_report_season(ReportSeasonInput::default())
        .expect("report season should list");

    assert_eq!(season.upcoming.len(), 1);
    assert_eq!(season.past.len(), 1);
    assert_eq!(season.upcoming[0].event_key, "evt-upcoming");
    assert_eq!(season.upcoming[0].qualified_ticker, "GPW:CDR");
    assert_eq!(season.upcoming[0].preparation_status, "upcoming");
    assert_eq!(season.past[0].event_key, "evt-past");

    // Freshest fetched_at wins for the freshness readout.
    assert_eq!(
        season.calendar_freshness.last_fetched_at.as_deref(),
        Some("2099-08-01T08:00:00Z")
    );
    assert!(!season.calendar_freshness.stale);

    state
        .mark_report_prepared(MarkReportPreparedInput {
            company_id: company.id.clone(),
            event_key: "evt-upcoming".to_owned(),
        })
        .expect("prepare should persist");

    let season = state
        .list_report_season(ReportSeasonInput::default())
        .expect("report season should list");
    assert_eq!(season.upcoming[0].preparation_status, "prepared");
}

#[test]
fn calendar_is_watchlist_scoped_and_rejects_unknown_watchlist() {
    let connection = open_in_memory_database().expect("database should initialize");
    let state = AppState::new(connection);
    let company = sample_company(&state);
    report_event(
        &state,
        &company,
        "2099-08-29",
        "evt-upcoming",
        "2099-08-01T08:00:00Z",
    );

    let watchlist = state
        .create_watchlist(NewWatchlist {
            name: "Main GPW".to_owned(),
            description: None,
        })
        .expect("watchlist should be created");
    state
        .add_company_to_watchlist(WatchlistCompanyInput {
            watchlist_id: watchlist.id.clone(),
            company_id: company.id.clone(),
        })
        .expect("company should be assigned");

    let scoped = state
        .list_report_season(ReportSeasonInput {
            watchlist_id: Some(watchlist.id.clone()),
        })
        .expect("scoped season should list");
    assert_eq!(scoped.upcoming.len(), 1);

    let empty = state
        .create_watchlist(NewWatchlist {
            name: "Empty".to_owned(),
            description: None,
        })
        .expect("watchlist should be created");
    let empty_scoped = state
        .list_report_season(ReportSeasonInput {
            watchlist_id: Some(empty.id),
        })
        .expect("empty scoped season should list");
    assert!(empty_scoped.upcoming.is_empty());

    let missing = state.list_report_season(ReportSeasonInput {
        watchlist_id: Some("watchlist_does_not_exist".to_owned()),
    });
    assert!(missing.is_err());
}

#[test]
fn pre_report_card_composes_owning_domains() {
    let connection = open_in_memory_database().expect("database should initialize");
    let state = AppState::new(connection);
    let company = sample_company(&state);
    report_event(
        &state,
        &company,
        "2099-08-29",
        "evt-upcoming",
        "2099-08-01T08:00:00Z",
    );

    state
        .create_research_question(NewResearchQuestion {
            scope_type: "company".to_owned(),
            scope_id: company.id.clone(),
            title: "Czy marża brutto się utrzyma?".to_owned(),
            body: None,
        })
        .expect("question should be created");

    state
        .create_management_claim(NewManagementClaim {
            company_id: company.id.clone(),
            statement: "Zarząd oczekuje wzrostu przychodów.".to_owned(),
            due_fiscal_year: Some(2099),
            due_period_type: Some("Q4".to_owned()),
            ..Default::default()
        })
        .expect("claim should be created");

    let kpi = state
        .create_kpi_definition(NewKpiDefinition {
            scope: "company".to_owned(),
            company_id: Some(company.id.clone()),
            sector: None,
            metric_key: "net_revenue".to_owned(),
            label: "Net Revenue".to_owned(),
            value_kind: "currency".to_owned(),
            unit: Some("PLN".to_owned()),
            computation: "reported".to_owned(),
            formula: None,
            display_format: None,
            origin: None,
            statement_group: None,
            period_nature: None,
        })
        .expect("kpi definition should be created");

    let older = state
        .create_financial_period(NewFinancialPeriod {
            company_id: company.id.clone(),
            fiscal_year: 2024,
            period_type: "FY".to_owned(),
            period_end_date: Some("2024-12-31".to_owned()),
            report_evidence_ref: None,
        })
        .expect("older period should be created");
    let latest = state
        .create_financial_period(NewFinancialPeriod {
            company_id: company.id.clone(),
            fiscal_year: 2025,
            period_type: "FY".to_owned(),
            period_end_date: Some("2025-12-31".to_owned()),
            report_evidence_ref: None,
        })
        .expect("latest period should be created");

    // A confirmed fact in the latest period, plus an older-period fact that must
    // not leak into "last-period KPIs".
    state
        .create_financial_fact(NewFinancialFact {
            company_id: company.id.clone(),
            period_id: latest.id.clone(),
            definition_id: kpi.id.clone(),
            value_numeric: "950000000".to_owned(),
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
            annotation: None,
        })
        .expect("confirmed fact should be created");
    state
        .create_financial_fact(NewFinancialFact {
            company_id: company.id.clone(),
            period_id: older.id.clone(),
            definition_id: kpi.id.clone(),
            value_numeric: "800000000".to_owned(),
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
            annotation: None,
        })
        .expect("older fact should be created");

    let card = state
        .get_pre_report_card(PreReportCardInput {
            company_id: company.id.clone(),
            event_key: "evt-upcoming".to_owned(),
        })
        .expect("card should be assembled");

    assert_eq!(card.event_date.as_deref(), Some("2099-08-29"));
    assert_eq!(card.preparation_status, "upcoming");
    assert_eq!(card.open_questions.len(), 1);
    assert_eq!(card.unresolved_claims.upcoming.len(), 1);
    assert_eq!(
        card.last_period_kpis.len(),
        1,
        "only the latest period's KPIs"
    );
    assert_eq!(card.last_period_kpis[0].period_id, latest.id);
    assert_eq!(card.last_period_kpis[0].metric_key, "net_revenue");
    assert_eq!(card.last_period_kpis[0].value_numeric, "950000000");
}

#[test]
fn prepare_then_process_persists_workflow_and_defaults_to_upcoming() {
    let connection = open_in_memory_database().expect("database should initialize");
    let state = AppState::new(connection);
    let company = sample_company(&state);
    report_event(
        &state,
        &company,
        "2099-08-29",
        "evt-upcoming",
        "2099-08-01T08:00:00Z",
    );

    // Default before any action.
    let card = state
        .get_pre_report_card(PreReportCardInput {
            company_id: company.id.clone(),
            event_key: "evt-upcoming".to_owned(),
        })
        .expect("card should be assembled");
    assert_eq!(card.preparation_status, "upcoming");

    let prepared = state
        .mark_report_prepared(MarkReportPreparedInput {
            company_id: company.id.clone(),
            event_key: "evt-upcoming".to_owned(),
        })
        .expect("prepare should persist");
    assert_eq!(prepared.status, "prepared");
    assert!(prepared.prepared_at.is_some());
    assert!(prepared.processed_at.is_none());

    let processed = state
        .mark_report_processed(MarkReportProcessedInput {
            company_id: company.id.clone(),
            event_key: "evt-upcoming".to_owned(),
            linked_report_document_id: None,
        })
        .expect("process should persist");
    assert_eq!(processed.status, "processed");
    assert!(processed.processed_at.is_some());
    // prepared_at is preserved across the transition.
    assert_eq!(processed.prepared_at, prepared.prepared_at);

    let card = state
        .get_pre_report_card(PreReportCardInput {
            company_id: company.id.clone(),
            event_key: "evt-upcoming".to_owned(),
        })
        .expect("card should be assembled");
    assert_eq!(card.preparation_status, "processed");
}

#[test]
fn workflow_actions_reject_unknown_company() {
    let connection = open_in_memory_database().expect("database should initialize");
    let state = AppState::new(connection);

    let result = state.mark_report_prepared(MarkReportPreparedInput {
        company_id: "company_gpw_unknown".to_owned(),
        event_key: "evt-upcoming".to_owned(),
    });
    assert!(result.is_err());
}
