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

// ---------------------------------------------------------------------------
// Create-time core `kpi_relevance` seeding (issue #203 residual, T7 slice 1).
// Migration 0106 seeded the five-key IFRS core set for the companies that
// existed WHEN IT APPLIED; every company created afterwards had an empty
// denominator, so the completeness check silently never fired for it. Creation
// now seeds the same set, with the same INSERT OR IGNORE / NOT EXISTS
// semantics — a curated row is never overwritten.
// ---------------------------------------------------------------------------

const CORE_KPI_KEYS: [&str; 5] = [
    "net_profit",
    "operating_profit",
    "revenue",
    "total_assets",
    "total_equity",
];

fn core_relevance_keys(state: &AppState, company_id: &str) -> Vec<(String, String, String)> {
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
fn creating_a_company_seeds_the_core_kpi_relevance_set() {
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

    let rows = core_relevance_keys(&state, &created.id);
    let expected: Vec<(String, String, String)> = CORE_KPI_KEYS
        .iter()
        .map(|k| ((*k).to_owned(), "core".to_owned(), "primary".to_owned()))
        .collect();
    assert_eq!(rows, expected, "a new company must get the 0106 core set");

    for row in state
        .list_kpi_relevance(&created.id)
        .expect("relevance should list")
    {
        assert_eq!(row.status, "active");
        assert!(
            row.id.starts_with(&format!("kpirel_core_{}_", created.id)),
            "seeded ids must mirror the 0106 scheme, got {}",
            row.id
        );
    }

    // The seeded denominator is what the completeness check reads.
    let expected_keys = state
        .financials()
        .expected_primary_metric_keys(&created.id)
        .expect("expected keys should read");
    assert_eq!(
        expected_keys.map(|k| k.into_iter().collect::<Vec<_>>()),
        Some(CORE_KPI_KEYS.iter().map(|k| (*k).to_owned()).collect())
    );
}

#[test]
fn core_kpi_relevance_seeding_never_overwrites_a_curated_row() {
    let connection = open_in_memory_database().expect("database should initialize");
    let state = AppState::new(connection);

    let created = state
        .create_company(NewCompany {
            exchange: "GPW".to_owned(),
            ticker: "CDR".to_owned(),
            display_name: "CD PROJEKT S.A.".to_owned(),
            isin: None,
            cik: None,
            lei: None,
        })
        .expect("company should be created");

    // The owner demotes one core metric for this company.
    let revenue_row = state
        .list_kpi_relevance(&created.id)
        .expect("relevance should list")
        .into_iter()
        .find(|r| r.definition_id == "kpidef_revenue")
        .expect("revenue should be seeded");
    state
        .update_kpi_relevance(UpdateKpiRelevance {
            id: revenue_row.id.clone(),
            status: Some("retired".to_owned()),
            rank: Some("secondary".to_owned()),
            first_seen_period: None,
            last_seen_period: None,
        })
        .expect("curated update should apply");

    // Re-seeding (idempotence: the healing migration, a retried creation) must
    // converge, not resurrect the row the owner curated.
    let connection = state.checkout().expect("connection should check out");
    crate::storage::financials::seed_core_kpi_relevance(&connection, &created.id)
        .expect("re-seed should be idempotent");
    drop(connection);

    let rows = core_relevance_keys(&state, &created.id);
    assert_eq!(rows.len(), 5, "re-seeding must not duplicate rows");
    let curated = state
        .list_kpi_relevance(&created.id)
        .expect("relevance should list")
        .into_iter()
        .find(|r| r.id == revenue_row.id)
        .expect("curated row should survive");
    assert_eq!(curated.status, "retired");
    assert_eq!(curated.rank, Some("secondary".to_owned()));
}

// ---------------------------------------------------------------------------
// Create-time statement-pack seeding — ADR 0092 layer 2 (issue #273).
//
// The `scope='sector'` KPI packs have been seeded since ADR 0027 but nothing
// ever read them at runtime. Layer 2 turns `companies.statement_type` into a
// conservative additive selection over them. Two truths this pins:
//   * the INDUSTRIAL pack is `scope='canonical'` (migration 0034 seeds it with
//     `scope='canonical', sector=NULL`), so an industrial company gets the core
//     five and nothing more — there is no industrial sector pack to add;
//   * `statement_type` is written by the registry-sector bridge, so a bank has
//     to be RECOGNISED at creation for its pack to land.
// ---------------------------------------------------------------------------

fn banking_registry_entry(ticker: &str, display_name: &str) -> GpwCompanyRegistryEntry {
    GpwCompanyRegistryEntry {
        exchange: "GPW".to_owned(),
        ticker: ticker.to_owned(),
        qualified_ticker: format!("GPW:{ticker}"),
        display_name: display_name.to_owned(),
        isin: format!("PL{ticker}00000000"),
        source_url: "https://www.gpw.pl/spolka".to_owned(),
        sector: Some("banki komercyjne".to_owned()),
    }
}

#[test]
fn creating_a_recognised_bank_seeds_core_plus_the_banking_pack() {
    let connection = open_in_memory_database().expect("database should initialize");
    let state = AppState::new(connection);
    state
        .refresh_gpw_company_registry(
            &[banking_registry_entry("PKO", "PKO BP S.A.")],
            "2026-07-31T12:00:00Z",
        )
        .expect("registry should refresh");

    let created = state
        .create_company(NewCompany {
            exchange: "GPW".to_owned(),
            ticker: "PKO".to_owned(),
            display_name: "PKO BP S.A.".to_owned(),
            isin: None,
            cik: None,
            lei: None,
        })
        .expect("company should be created");

    assert_eq!(
        state
            .companies()
            .get_statement_type(&created.id)
            .expect("statement type should read"),
        "banking",
        "the registry-sector bridge has to run at creation or the pack cannot key off anything"
    );

    let rows = core_relevance_keys(&state, &created.id);
    let keys: Vec<&str> = rows.iter().map(|(k, _, _)| k.as_str()).collect();
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
        ],
        "core five + the conservative banking pack"
    );
    for (key, source, rank) in &rows {
        let expected_source = if CORE_KPI_KEYS.contains(&key.as_str()) {
            "core"
        } else {
            "sector"
        };
        assert_eq!(source, expected_source, "{key} source");
        assert_eq!(rank, "primary", "{key} rank");
    }
}

#[test]
fn creating_an_industrial_company_seeds_only_the_core_set() {
    let connection = open_in_memory_database().expect("database should initialize");
    let state = AppState::new(connection);

    let created = state
        .create_company(NewCompany {
            exchange: "GPW".to_owned(),
            ticker: "CDR".to_owned(),
            display_name: "CD PROJEKT S.A.".to_owned(),
            isin: None,
            cik: None,
            lei: None,
        })
        .expect("company should be created");

    assert_eq!(
        state
            .companies()
            .get_statement_type(&created.id)
            .expect("statement type should read"),
        "industrial"
    );
    let keys: Vec<String> = core_relevance_keys(&state, &created.id)
        .into_iter()
        .map(|(k, _, _)| k)
        .collect();
    assert_eq!(
        keys,
        CORE_KPI_KEYS
            .iter()
            .map(|k| (*k).to_owned())
            .collect::<Vec<_>>(),
        "the industrial pack is scope='canonical', so there is nothing to add"
    );
}

#[test]
fn statement_type_change_adds_the_pack_and_removes_nothing() {
    let connection = open_in_memory_database().expect("database should initialize");
    let state = AppState::new(connection);

    let created = state
        .create_company(NewCompany {
            exchange: "GPW".to_owned(),
            ticker: "PEO".to_owned(),
            display_name: "Bank Pekao S.A.".to_owned(),
            isin: None,
            cik: None,
            lei: None,
        })
        .expect("company should be created");

    // A user-curated row that automation must treat as untouchable (ADR 0092
    // layer 4), plus the core five already seeded.
    let curated = state
        .create_kpi_relevance(NewKpiRelevance {
            company_id: created.id.clone(),
            definition_id: "kpidef_revenue".to_owned(),
            source: "user".to_owned(),
            rank: Some("secondary".to_owned()),
            first_seen_period: None,
            last_seen_period: None,
        })
        .expect("curated row should create");

    // The bridge (or a future setter) reclassifies the company.
    {
        let connection = state
            .checkout_for_tests()
            .expect("connection should check out");
        connection
            .execute(
                "UPDATE companies SET statement_type = 'banking' WHERE id = ?1",
                [&created.id],
            )
            .expect("reclassify");
        crate::storage::financials::seed_statement_pack_kpi_relevance(&connection, &created.id)
            .expect("additive re-seed should apply");
    }

    let rows = core_relevance_keys(&state, &created.id);
    let keys: Vec<&str> = rows.iter().map(|(k, _, _)| k.as_str()).collect();
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
        ],
        "the change ADDS the pack; ADR 0092 forbids deleting on reclassification"
    );

    let survivor = state
        .list_kpi_relevance(&created.id)
        .expect("relevance should list")
        .into_iter()
        .find(|r| r.id == curated.id)
        .expect("the user row must survive an automatic re-seed");
    assert_eq!(survivor.source, "user");
    assert_eq!(survivor.rank, Some("secondary".to_owned()));
}
