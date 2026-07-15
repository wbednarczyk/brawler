use super::common::*;
use super::*;

#[test]
fn enforces_exchange_qualified_ticker_uniqueness() {
    let connection = open_in_memory_database().expect("database should initialize");

    connection
        .execute(
            "
                INSERT INTO companies (
                    id,
                    exchange,
                    ticker,
                    qualified_ticker,
                    display_name
                ) VALUES (?1, ?2, ?3, ?4, ?5)
                ",
            (
                "company_gpw_cdr",
                "GPW",
                "CDR",
                "GPW:CDR",
                "CD PROJEKT S.A.",
            ),
        )
        .expect("first company insert should pass");

    let duplicate = connection.execute(
        "
            INSERT INTO companies (
                id,
                exchange,
                ticker,
                qualified_ticker,
                display_name
            ) VALUES (?1, ?2, ?3, ?4, ?5)
            ",
        (
            "company_gpw_cdr_duplicate",
            "GPW",
            "CDR",
            "GPW:CDR",
            "Duplicate",
        ),
    );

    assert!(duplicate.is_err());
}

#[test]
fn creates_and_lists_company_through_storage_api() {
    let connection = open_in_memory_database().expect("database should initialize");
    let state = AppState::new(connection);

    let created = state
        .create_company(NewCompany {
            exchange: "gpw".to_owned(),
            ticker: "cdr".to_owned(),
            display_name: "CD PROJEKT S.A.".to_owned(),
            isin: Some("PLOPTTC00011".to_owned()),
            cik: None,
            lei: None,
        })
        .expect("company should be created");

    let companies = state.list_companies().expect("companies should be listed");

    assert_eq!(created.id, "company_gpw_cdr");
    assert_eq!(created.qualified_ticker, "GPW:CDR");
    assert_eq!(companies.len(), 1);
    assert_eq!(companies[0].display_name, "CD PROJEKT S.A.");
}

#[test]
fn looks_up_registry_company_by_ticker() {
    let connection = open_in_memory_database().expect("database should initialize");
    let state = AppState::new(connection);
    state
        .refresh_gpw_company_registry(
            &[registry_entry("CDR", "CD PROJEKT S.A.", "PLOPTTC00011")],
            "2026-05-31T12:00:00Z",
        )
        .expect("test registry should refresh");

    let result = state
        .lookup_company(CompanyLookupInput {
            exchange: "gpw".to_owned(),
            ticker: Some("cdr".to_owned()),
            display_name: None,
            isin: None,
        })
        .expect("lookup should succeed")
        .expect("registry should match");

    assert_eq!(result.qualified_ticker, "GPW:CDR");
    assert_eq!(result.display_name, "CD PROJEKT S.A.");
    assert_eq!(result.isin, "PLOPTTC00011");
    assert_eq!(result.source, "company_directory");
}

#[test]
fn looks_up_registry_company_by_isin() {
    let connection = open_in_memory_database().expect("database should initialize");
    let state = AppState::new(connection);
    state
        .refresh_gpw_company_registry(
            &[registry_entry("PZU", "PZU S.A.", "PLPZU0000011")],
            "2026-05-31T12:00:00Z",
        )
        .expect("test registry should refresh");

    let result = state
        .lookup_company(CompanyLookupInput {
            exchange: "GPW".to_owned(),
            ticker: None,
            display_name: None,
            isin: Some("plpzu0000011".to_owned()),
        })
        .expect("lookup should succeed")
        .expect("registry should match");

    assert_eq!(result.ticker, "PZU");
    assert_eq!(result.display_name, "PZU S.A.");
    assert_eq!(result.source, "company_directory");
}

#[test]
fn refreshes_gpw_company_registry_cache() {
    let connection = open_in_memory_database().expect("database should initialize");
    let state = AppState::new(connection);
    let entries = vec![GpwCompanyRegistryEntry {
        exchange: "GPW".to_owned(),
        ticker: "TST".to_owned(),
        qualified_ticker: "GPW:TST".to_owned(),
        display_name: "TEST COMPANY S.A.".to_owned(),
        isin: "PLTEST000001".to_owned(),
        source_url: "https://www.gpw.pl/spolka?isin=PLTEST000001".to_owned(),
        sector: None,
    }];

    let result = state
        .refresh_gpw_company_registry(&entries, "2026-05-31T12:00:00Z")
        .expect("registry refresh should succeed");

    assert_eq!(result.adapter_id, GPW_REGISTRY_ADAPTER_ID);
    assert_eq!(result.entries_fetched, 1);
    assert_eq!(result.entries_upserted, 1);

    let lookup = state
        .lookup_company(CompanyLookupInput {
            exchange: "GPW".to_owned(),
            ticker: Some("tst".to_owned()),
            display_name: None,
            isin: None,
        })
        .expect("lookup should succeed")
        .expect("refreshed registry entry should match");

    assert_eq!(lookup.qualified_ticker, "GPW:TST");
    assert_eq!(lookup.source, "company_directory");
}

#[test]
fn refreshes_newconnect_company_directory_cache() {
    let connection = open_in_memory_database().expect("database should initialize");
    let state = AppState::new(connection);
    let entries = vec![GpwCompanyRegistryEntry {
        exchange: "NC".to_owned(),
        ticker: "4MB".to_owned(),
        qualified_ticker: "NC:4MB".to_owned(),
        display_name: "4MOBILITY SPÓŁKA AKCYJNA".to_owned(),
        isin: "PLESLTN00010".to_owned(),
        source_url: "https://newconnect.pl/spolka?isin=PLESLTN00010".to_owned(),
        sector: None,
    }];

    let result = state
        .refresh_newconnect_company_directory(&entries, "2026-05-31T12:00:00Z")
        .expect("NewConnect directory refresh should succeed");

    assert_eq!(result.adapter_id, NEWCONNECT_DIRECTORY_ADAPTER_ID);
    assert_eq!(result.entries_fetched, 1);
    assert_eq!(result.entries_upserted, 1);

    let lookup = state
        .lookup_company(CompanyLookupInput {
            exchange: "NC".to_owned(),
            ticker: Some("4mb".to_owned()),
            display_name: None,
            isin: None,
        })
        .expect("lookup should succeed")
        .expect("refreshed NewConnect entry should match");

    assert_eq!(lookup.qualified_ticker, "NC:4MB");
    assert_eq!(lookup.source, "company_directory");
}

#[test]
fn creates_newconnect_company_from_directory_result() {
    let connection = open_in_memory_database().expect("database should initialize");
    let state = AppState::new(connection);

    let company = state
        .create_company(NewCompany {
            exchange: "NC".to_owned(),
            ticker: "4MB".to_owned(),
            display_name: "4MOBILITY SPÓŁKA AKCYJNA".to_owned(),
            isin: Some("PLESLTN00010".to_owned()),
            cik: None,
            lei: None,
        })
        .expect("NewConnect company should create");
    let companies = state.list_companies().expect("companies should list");

    assert_eq!(company.id, "company_nc_4mb");
    assert_eq!(company.qualified_ticker, "NC:4MB");
    assert!(companies
        .iter()
        .any(|listed| listed.qualified_ticker == "NC:4MB"));
}

#[test]
fn lookup_searches_all_company_directories_and_returns_newconnect_from_default_exchange() {
    let connection = open_in_memory_database().expect("database should initialize");
    let state = AppState::new(connection);
    let entries = vec![GpwCompanyRegistryEntry {
        exchange: "NC".to_owned(),
        ticker: "4MB".to_owned(),
        qualified_ticker: "NC:4MB".to_owned(),
        display_name: "4MOBILITY SPÓŁKA AKCYJNA".to_owned(),
        isin: "PLESLTN00010".to_owned(),
        source_url: "https://newconnect.pl/spolka?isin=PLESLTN00010".to_owned(),
        sector: None,
    }];

    state
        .refresh_newconnect_company_directory(&entries, "2026-05-31T12:00:00Z")
        .expect("NewConnect directory refresh should succeed");

    let lookup = state
        .lookup_company(CompanyLookupInput {
            exchange: "GPW".to_owned(),
            ticker: Some("4mb".to_owned()),
            display_name: None,
            isin: None,
        })
        .expect("lookup should succeed")
        .expect("lookup should search all company directories");

    assert_eq!(lookup.exchange, "NC");
    assert_eq!(lookup.qualified_ticker, "NC:4MB");
}

#[test]
fn lookup_prefers_selected_exchange_when_directory_tickers_collide() {
    let connection = open_in_memory_database().expect("database should initialize");
    let state = AppState::new(connection);

    state
        .refresh_gpw_company_registry(
            &[GpwCompanyRegistryEntry {
                exchange: "GPW".to_owned(),
                ticker: "ABC".to_owned(),
                qualified_ticker: "GPW:ABC".to_owned(),
                display_name: "ABC MAIN MARKET S.A.".to_owned(),
                isin: "PLABC000001".to_owned(),
                source_url: "https://www.gpw.pl/spolka?isin=PLABC000001".to_owned(),
                sector: None,
            }],
            "2026-05-31T12:00:00Z",
        )
        .expect("GPW directory refresh should succeed");
    state
        .refresh_newconnect_company_directory(
            &[GpwCompanyRegistryEntry {
                exchange: "NC".to_owned(),
                ticker: "ABC".to_owned(),
                qualified_ticker: "NC:ABC".to_owned(),
                display_name: "ABC NEWCONNECT S.A.".to_owned(),
                isin: "PLABC000002".to_owned(),
                source_url: "https://newconnect.pl/spolka?isin=PLABC000002".to_owned(),
                sector: None,
            }],
            "2026-05-31T12:00:00Z",
        )
        .expect("NewConnect directory refresh should succeed");

    let lookup = state
        .lookup_company(CompanyLookupInput {
            exchange: "NC".to_owned(),
            ticker: Some("abc".to_owned()),
            display_name: None,
            isin: None,
        })
        .expect("lookup should succeed")
        .expect("selected exchange should be preferred");

    assert_eq!(lookup.qualified_ticker, "NC:ABC");
}

#[test]
fn future_company_directory_entries_work_without_registry_specific_code() {
    let connection = open_in_memory_database().expect("database should initialize");
    connection
        .execute(
            "
                INSERT INTO source_adapters (
                    id,
                    display_name,
                    source_type,
                    fetch_mode,
                    enabled,
                    default_poll_interval_seconds
                ) VALUES (?1, ?2, ?3, ?4, ?5, ?6)
                ",
            (
                "future-company-directory",
                "Future Company Directory",
                "company_registry",
                "public_page",
                1,
                86_400,
            ),
        )
        .expect("future directory source adapter should insert");
    connection
        .execute(
            "
                INSERT INTO source_adapter_markets (source_adapter_id, market)
                VALUES (?1, ?2)
                ",
            ("future-company-directory", "XETRA"),
        )
        .expect("future directory market should insert");
    connection
        .execute(
            "
                INSERT INTO company_registry_entries (
                    id,
                    exchange,
                    ticker,
                    qualified_ticker,
                    display_name,
                    isin,
                    source_adapter_id,
                    source_url,
                    fetched_at
                ) VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9)
                ",
            (
                "registry_future_xetra_sap",
                "XETRA",
                "SAP",
                "XETRA:SAP",
                "SAP SE",
                "DE0007164600",
                "future-company-directory",
                "https://example.test/xetra/sap",
                "2026-05-31T12:00:00Z",
            ),
        )
        .expect("future directory entry should insert");
    let state = AppState::new(connection);

    let lookup = state
        .lookup_company(CompanyLookupInput {
            exchange: "GPW".to_owned(),
            ticker: Some("sap".to_owned()),
            display_name: None,
            isin: None,
        })
        .expect("lookup should succeed")
        .expect("lookup should search all company directories");

    assert_eq!(lookup.exchange, "XETRA");
    assert_eq!(lookup.qualified_ticker, "XETRA:SAP");

    let created = state
        .create_company(NewCompany {
            exchange: lookup.exchange,
            ticker: lookup.ticker,
            display_name: lookup.display_name,
            isin: Some(lookup.isin),
            cik: None,
            lei: None,
        })
        .expect("future directory company should create");

    assert_eq!(created.qualified_ticker, "XETRA:SAP");
}

#[test]
fn detects_stale_company_directory_cache() {
    let connection = open_in_memory_database().expect("database should initialize");

    assert!(company_directories_are_stale(&connection, 86_400)
        .expect("registry should report stale when never refreshed"));

    connection
        .execute(
            "
                UPDATE source_adapters
                SET last_success_at = strftime('%Y-%m-%dT%H:%M:%fZ', 'now')
                WHERE id IN (?1, ?2)
                ",
            params![GPW_REGISTRY_ADAPTER_ID, NEWCONNECT_DIRECTORY_ADAPTER_ID],
        )
        .expect("registry adapter timestamp should update");

    assert!(!company_directories_are_stale(&connection, 86_400)
        .expect("fresh registry should not be stale"));

    connection
        .execute(
            "
                UPDATE source_adapters
                SET last_success_at = strftime('%Y-%m-%dT%H:%M:%fZ', 'now', '-2 days')
                WHERE id = ?1
                ",
            [GPW_REGISTRY_ADAPTER_ID],
        )
        .expect("registry adapter timestamp should update");

    assert!(
        company_directories_are_stale(&connection, 86_400).expect("old registry should be stale")
    );
}

#[test]
fn future_enabled_company_directory_participates_in_staleness_checks() {
    let connection = open_in_memory_database().expect("database should initialize");
    connection
        .execute(
            "
                INSERT INTO source_adapters (
                    id,
                    display_name,
                    source_type,
                    fetch_mode,
                    enabled,
                    default_poll_interval_seconds,
                    last_success_at
                ) VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7)
                ",
            (
                "future-company-directory",
                "Future Company Directory",
                "company_registry",
                "public_page",
                1,
                86_400,
                "2026-05-31T12:00:00Z",
            ),
        )
        .expect("future directory source adapter should insert");

    assert!(company_directories_are_stale(&connection, 86_400)
        .expect("old future registry should be stale"));
}

#[test]
fn deletes_company_through_storage_api() {
    let connection = open_in_memory_database().expect("database should initialize");
    let state = AppState::new(connection);

    let created = state
        .create_company(NewCompany {
            exchange: "GPW".to_owned(),
            ticker: "CDR".to_owned(),
            display_name: "CD PROJEKT S.A.".to_owned(),
            isin: Some("PLOPTTC00011".to_owned()),
            cik: None,
            lei: None,
        })
        .expect("company should be created");

    state
        .delete_company(&created.id)
        .expect("company should be deleted");

    let companies = state.list_companies().expect("companies should be listed");

    assert!(companies.is_empty());
}
