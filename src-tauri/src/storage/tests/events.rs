use super::*;

#[test]
fn creates_and_lists_company_events() {
    let connection = open_in_memory_database().expect("database should initialize");
    let state = AppState::new(connection);
    let company = state
        .create_company(NewCompany {
            exchange: "GPW".to_owned(),
            ticker: "CDR".to_owned(),
            display_name: "CD PROJEKT S.A.".to_owned(),
            isin: Some("PLOPTTC00011".to_owned()),
            cik: None,
            lei: None,
        })
        .expect("company should be created");

    let created = state
        .create_company_event(NewCompanyEvent {
            company_id: company.id.clone(),
            event_type: "periodic_report".to_owned(),
            title: "Quarterly report publication".to_owned(),
            event_date: "2099-08-29".to_owned(),
            event_time: None,
            status: Some("scheduled".to_owned()),
            source_type: Some("manual".to_owned()),
            source_adapter_id: None,
            source_event_key: None,
            source_url: None,
            attribution: None,
            fetched_at: None,
        })
        .expect("event should be created");

    let events = state
        .list_company_events(CompanyEventListInput {
            mode: Some("upcoming".to_owned()),
            company_id: Some(company.id.clone()),
            watchlist_id: None,
            event_type: Some("periodic_report".to_owned()),
            status: None,
            date_from: None,
            date_to: None,
        })
        .expect("events should list");

    assert_eq!(events.len(), 1);
    assert_eq!(created.company, "GPW:CDR");
    assert_eq!(events[0].title, "Quarterly report publication");
    assert!(events[0].manual);
}

#[test]
fn creates_and_lists_manual_events_for_future_exchange_company() {
    let connection = open_in_memory_database().expect("database should initialize");
    let state = AppState::new(connection);
    let company = state
        .create_company(NewCompany {
            exchange: "XETRA".to_owned(),
            ticker: "SAP".to_owned(),
            display_name: "SAP SE".to_owned(),
            isin: Some("DE0007164600".to_owned()),
            cik: None,
            lei: None,
        })
        .expect("future exchange company should be created");

    let created = state
        .create_company_event(NewCompanyEvent {
            company_id: company.id.clone(),
            event_type: "periodic_report".to_owned(),
            title: "Quarterly report publication".to_owned(),
            event_date: "2099-08-29".to_owned(),
            event_time: None,
            status: Some("scheduled".to_owned()),
            source_type: Some("manual".to_owned()),
            source_adapter_id: None,
            source_event_key: None,
            source_url: None,
            attribution: None,
            fetched_at: None,
        })
        .expect("event should be created");

    let events = state
        .list_company_events(CompanyEventListInput {
            mode: Some("upcoming".to_owned()),
            company_id: Some(company.id),
            watchlist_id: None,
            event_type: Some("periodic_report".to_owned()),
            status: None,
            date_from: None,
            date_to: None,
        })
        .expect("events should list");

    assert_eq!(created.company, "XETRA:SAP");
    assert_eq!(events.len(), 1);
    assert_eq!(events[0].company, "XETRA:SAP");
}

#[test]
fn updates_sourced_company_events_by_source_key() {
    let connection = open_in_memory_database().expect("database should initialize");
    let state = AppState::new(connection);
    let company = state
        .create_company(NewCompany {
            exchange: "GPW".to_owned(),
            ticker: "CDR".to_owned(),
            display_name: "CD PROJEKT S.A.".to_owned(),
            isin: Some("PLOPTTC00011".to_owned()),
            cik: None,
            lei: None,
        })
        .expect("company should be created");

    let input = NewCompanyEvent {
        company_id: company.id,
        event_type: "dividend".to_owned(),
        title: "Dividend day".to_owned(),
        event_date: "2099-06-12".to_owned(),
        event_time: None,
        status: Some("confirmed".to_owned()),
        source_type: Some("official_calendar".to_owned()),
        source_adapter_id: Some("bankier-company-komunikaty".to_owned()),
        source_event_key: Some("bankier-company-komunikaty:GPW:CDR:dividend:2099-06-12".to_owned()),
        source_url: Some("https://www.bankier.pl/".to_owned()),
        attribution: Some("Bankier.pl".to_owned()),
        fetched_at: Some("2026-06-01T10:00:00Z".to_owned()),
    };

    let first = state
        .create_company_event(input)
        .expect("source event should be created");
    let second = state
        .create_company_event(NewCompanyEvent {
            company_id: first.company_id.clone(),
            event_type: "dividend".to_owned(),
            title: "Updated dividend day".to_owned(),
            event_date: "2099-06-14".to_owned(),
            event_time: None,
            status: Some("changed".to_owned()),
            source_type: Some("official_calendar".to_owned()),
            source_adapter_id: Some("bankier-company-komunikaty".to_owned()),
            source_event_key: Some(
                "bankier-company-komunikaty:GPW:CDR:dividend:2099-06-12".to_owned(),
            ),
            source_url: Some("https://www.bankier.pl/duplicate".to_owned()),
            attribution: Some("Bankier.pl".to_owned()),
            fetched_at: Some("2026-06-01T10:05:00Z".to_owned()),
        })
        .expect("updated source event should return existing record");
    let events = state
        .list_company_events(CompanyEventListInput {
            mode: Some("all".to_owned()),
            company_id: Some(first.company_id.clone()),
            watchlist_id: None,
            event_type: None,
            status: None,
            date_from: None,
            date_to: None,
        })
        .expect("events should list");

    assert_eq!(first.id, second.id);
    assert_eq!(events.len(), 1);
    assert_eq!(events[0].title, "Updated dividend day");
    assert_eq!(events[0].event_date, "2099-06-14");
    assert_eq!(events[0].status, "changed");
    assert!(!events[0].manual);
}

#[test]
fn ingests_gpw_market_events_for_tracked_companies_only() {
    let connection = open_in_memory_database().expect("database should initialize");
    let state = AppState::new(connection);
    let company = state
        .create_company(NewCompany {
            exchange: "GPW".to_owned(),
            ticker: "DIAG".to_owned(),
            display_name: "DIAGNOSTYKA S.A.".to_owned(),
            isin: Some("PLDIAG000019".to_owned()),
            cik: None,
            lei: None,
        })
        .expect("tracked company should create");
    let items = vec![
            GpwMarketEventItem {
                market: "Main Market".to_owned(),
                event_label: "Corporate actions".to_owned(),
                instrument_type: "Equity".to_owned(),
                ticker: "DIAG".to_owned(),
                event_type: "corporate_action".to_owned(),
                title: "Main Market - Corporate actions - Equity - DIAG".to_owned(),
                link: "https://www.gpw.pl/market-events-calendar?market_section=RGL&market_category=64&date=2026-06-01".to_owned(),
                event_date: "2099-06-01".to_owned(),
                fetched_at: "2026-06-01T08:00:00Z".to_owned(),
                source_event_key:
                    "gpw-market-events-rss:2099-06-01:corporate-actions:equity:diag".to_owned(),
            },
            GpwMarketEventItem {
                market: "Main Market".to_owned(),
                event_label: "Corporate actions".to_owned(),
                instrument_type: "Equity".to_owned(),
                ticker: "SNIEZKA".to_owned(),
                event_type: "corporate_action".to_owned(),
                title: "Main Market - Corporate actions - Equity - SNIEZKA".to_owned(),
                link: "https://www.gpw.pl/market-events-calendar?market_section=RGL&market_category=64&date=2026-06-01".to_owned(),
                event_date: "2099-06-01".to_owned(),
                fetched_at: "2026-06-01T08:00:00Z".to_owned(),
                source_event_key:
                    "gpw-market-events-rss:2099-06-01:corporate-actions:equity:sniezka"
                        .to_owned(),
            },
        ];

    let first_result = state
        .ingest_gpw_market_event_items(&items)
        .expect("events should ingest");
    let mut updated_items = items.clone();
    updated_items[0].title = "Main Market - Updated corporate actions - Equity - DIAG".to_owned();
    updated_items[0].event_date = "2099-06-02".to_owned();
    updated_items[0].link =
            "https://www.gpw.pl/market-events-calendar?market_section=RGL&market_category=64&date=2026-06-02"
                .to_owned();
    let second_result = state
        .ingest_gpw_market_event_items(&updated_items)
        .expect("updated source events should ingest harmlessly");
    let events = state
        .list_company_events(CompanyEventListInput {
            mode: Some("all".to_owned()),
            company_id: Some(company.id),
            watchlist_id: None,
            event_type: None,
            status: None,
            date_from: None,
            date_to: None,
        })
        .expect("events should list");

    assert_eq!(first_result.items_fetched, 2);
    assert_eq!(first_result.items_created, 1);
    assert_eq!(first_result.items_matched, 1);
    assert_eq!(first_result.items_unmatched, 1);
    assert_eq!(second_result.items_created, 0);
    assert_eq!(events.len(), 1);
    assert_eq!(events[0].company, "GPW:DIAG");
    assert_eq!(events[0].event_type, "corporate_action");
    assert_eq!(
        events[0].title,
        "Main Market - Updated corporate actions - Equity - DIAG"
    );
    assert_eq!(events[0].event_date, "2099-06-02");
    assert_eq!(
        events[0].source_adapter_id.as_deref(),
        Some("gpw-market-events-rss")
    );
}

#[test]
fn ingests_bankier_calendar_events_for_tracked_companies_only() {
    let connection = open_in_memory_database().expect("database should initialize");
    let state = AppState::new(connection);
    let company = state
        .create_company(NewCompany {
            exchange: "GPW".to_owned(),
            ticker: "DIAG".to_owned(),
            display_name: "DIAGNOSTYKA S.A.".to_owned(),
            isin: Some("PLDIAG000019".to_owned()),
            cik: None,
            lei: None,
        })
        .expect("tracked company should create");
    let items = vec![
        BankierCalendarEventItem {
            ticker: "DIAG".to_owned(),
            event_type: "dividend".to_owned(),
            title: "DIAG: Dzień ustalenia prawa do dywidendy 4,40 zł na akcję.".to_owned(),
            description: "Dzień ustalenia prawa do dywidendy 4,40 zł na akcję.".to_owned(),
            category: "Dywidendy".to_owned(),
            link: "https://www.bankier.pl/gielda/notowania/akcje/DIAG/kalendarium".to_owned(),
            event_date: "2099-06-01".to_owned(),
            fetched_at: "2026-06-01T08:00:00Z".to_owned(),
            source_event_key: "bankier-kalendarium-html:diag:dywidendy:dywidenda".to_owned(),
        },
        BankierCalendarEventItem {
            ticker: "SNIEZKA".to_owned(),
            event_type: "periodic_report".to_owned(),
            title: "SNIEZKA: Raport kwartalny.".to_owned(),
            description: "Raport kwartalny.".to_owned(),
            category: "Wyniki spółek".to_owned(),
            link: "https://www.bankier.pl/gielda/notowania/akcje/SNIEZKA/kalendarium".to_owned(),
            event_date: "2099-06-02".to_owned(),
            fetched_at: "2026-06-01T08:00:00Z".to_owned(),
            source_event_key: "bankier-kalendarium-html:sniezka:wyniki-spolek:raport-kwartalny"
                .to_owned(),
        },
    ];

    let first_result = state
        .ingest_bankier_calendar_event_items(&items)
        .expect("Bankier calendar events should ingest");
    let mut updated_items = items.clone();
    updated_items[0].title = "DIAG: Zaktualizowany dzień ustalenia prawa do dywidendy.".to_owned();
    updated_items[0].event_date = "2099-06-03".to_owned();
    let second_result = state
        .ingest_bankier_calendar_event_items(&updated_items)
        .expect("updated Bankier calendar events should ingest harmlessly");
    let events = state
        .list_company_events(CompanyEventListInput {
            mode: Some("all".to_owned()),
            company_id: Some(company.id),
            watchlist_id: None,
            event_type: None,
            status: None,
            date_from: None,
            date_to: None,
        })
        .expect("events should list");

    assert_eq!(first_result.items_fetched, 2);
    assert_eq!(first_result.items_created, 1);
    assert_eq!(first_result.items_matched, 1);
    assert_eq!(first_result.items_unmatched, 1);
    assert_eq!(second_result.items_created, 0);
    assert_eq!(events.len(), 1);
    assert_eq!(events[0].company, "GPW:DIAG");
    assert_eq!(events[0].event_type, "dividend");
    assert_eq!(
        events[0].title,
        "DIAG: Zaktualizowany dzień ustalenia prawa do dywidendy."
    );
    assert_eq!(events[0].event_date, "2099-06-03");
    assert_eq!(events[0].source_type, "public_calendar");
    assert_eq!(
        events[0].source_adapter_id.as_deref(),
        Some("bankier-kalendarium-html")
    );
}

#[test]
fn ingests_bankier_calendar_events_by_cached_bankier_slug_when_symbol_is_not_ticker() {
    let connection = open_in_memory_database().expect("database should initialize");
    let state = AppState::new(connection);
    let company = state
        .create_company(NewCompany {
            exchange: "GPW".to_owned(),
            ticker: "BDX".to_owned(),
            display_name: "BUDIMEX S.A.".to_owned(),
            isin: Some("PLBUDMX00013".to_owned()),
            cik: None,
            lei: None,
        })
        .expect("tracked company should create");
    state
        .upsert_bankier_company_identifiers(
            &company.id,
            &BankierCompanyIdentifiers {
                slug: "BUDIMEX".to_owned(),
                tag_id: "123".to_owned(),
            },
        )
        .expect("Bankier identifiers should cache");
    let items = vec![BankierCalendarEventItem {
        ticker: "BUDIMEX".to_owned(),
        event_type: "dividend".to_owned(),
        title: "BUDIMEX: Dzień ustalenia prawa do dywidendy.".to_owned(),
        description: "Dzień ustalenia prawa do dywidendy.".to_owned(),
        category: "Dywidendy".to_owned(),
        link: "https://www.bankier.pl/gielda/notowania/akcje/BUDIMEX/kalendarium".to_owned(),
        event_date: "2099-06-03".to_owned(),
        fetched_at: "2026-06-01T08:00:00Z".to_owned(),
        source_event_key: "bankier-kalendarium-html:budimex:dywidendy:dywidenda".to_owned(),
    }];

    let result = state
        .ingest_bankier_calendar_event_items(&items)
        .expect("Bankier calendar event should match cached slug");
    let events = state
        .list_company_events(CompanyEventListInput {
            mode: Some("all".to_owned()),
            company_id: None,
            watchlist_id: None,
            event_type: None,
            status: None,
            date_from: None,
            date_to: None,
        })
        .expect("events should list");

    assert_eq!(result.items_matched, 1);
    assert_eq!(result.items_unmatched, 0);
    assert_eq!(events.len(), 1);
    assert_eq!(events[0].company, "GPW:BDX");
    assert_eq!(
        events[0].title,
        "BUDIMEX: Dzień ustalenia prawa do dywidendy."
    );
}
