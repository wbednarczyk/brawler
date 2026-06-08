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
fn detects_stale_gpw_company_registry_cache() {
    let connection = open_in_memory_database().expect("database should initialize");

    assert!(gpw_company_registry_is_stale(&connection, 86_400)
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

    assert!(!gpw_company_registry_is_stale(&connection, 86_400)
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
        gpw_company_registry_is_stale(&connection, 86_400).expect("old registry should be stale")
    );
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
