use super::*;

#[test]
fn creates_clean_database_with_initial_schema() {
    let connection = open_in_memory_database().expect("database should initialize");

    let migration_count: i64 = connection
        .query_row("SELECT COUNT(*) FROM schema_migrations", [], |row| {
            row.get(0)
        })
        .expect("schema_migrations should exist");

    assert_eq!(migration_count, 29);

    let company_table_exists: bool = connection
        .query_row(
            "SELECT EXISTS(
                    SELECT 1
                    FROM sqlite_master
                    WHERE type = 'table' AND name = 'companies'
                )",
            [],
            |row| row.get(0),
        )
        .expect("companies table lookup should work");

    assert!(company_table_exists);

    let diagnostic_events_table_exists: bool = connection
        .query_row(
            "SELECT EXISTS(
                    SELECT 1
                    FROM sqlite_master
                    WHERE type = 'table' AND name = 'diagnostic_events'
                )",
            [],
            |row| row.get(0),
        )
        .expect("diagnostic events table lookup should work");

    assert!(diagnostic_events_table_exists);

    let license_metadata_table_exists: bool = connection
        .query_row(
            "SELECT EXISTS(
                    SELECT 1
                    FROM sqlite_master
                    WHERE type = 'table' AND name = 'license_metadata'
                )",
            [],
            |row| row.get(0),
        )
        .expect("license metadata table lookup should work");

    assert!(license_metadata_table_exists);
}

#[test]
fn seeds_default_settings_and_source_adapters() {
    let connection = open_in_memory_database().expect("database should initialize");

    let theme: String = connection
        .query_row(
            "SELECT value FROM settings WHERE key = 'theme'",
            [],
            |row| row.get(0),
        )
        .expect("theme setting should be seeded");
    let locale: String = connection
        .query_row(
            "SELECT value FROM settings WHERE key = 'locale'",
            [],
            |row| row.get(0),
        )
        .expect("locale setting should be seeded");
    let general_analysis_model: String = connection
        .query_row(
            "SELECT value FROM settings WHERE key = 'general_analysis_model'",
            [],
            |row| row.get(0),
        )
        .expect("general analysis model setting should be seeded");
    let developer_mode: String = connection
        .query_row(
            "SELECT value FROM settings WHERE key = 'developer_mode'",
            [],
            |row| row.get(0),
        )
        .expect("developer mode setting should be seeded");
    let log_level: String = connection
        .query_row(
            "SELECT value FROM settings WHERE key = 'log_level'",
            [],
            |row| row.get(0),
        )
        .expect("log level setting should be seeded");

    let gpw_adapter: (String, bool) = connection
        .query_row(
            "SELECT display_name, enabled FROM source_adapters WHERE id = 'gpw-espi-ebi'",
            [],
            |row| Ok((row.get(0)?, row.get(1)?)),
        )
        .expect("GPW adapter should be seeded");
    let registry_adapter_name: String = connection
        .query_row(
            "SELECT display_name FROM source_adapters WHERE id = 'gpw-company-registry'",
            [],
            |row| row.get(0),
        )
        .expect("GPW registry adapter should be seeded");
    let newconnect_directory_adapter: (String, bool) = connection
        .query_row(
            "SELECT display_name, enabled FROM source_adapters WHERE id = 'newconnect-company-directory'",
            [],
            |row| Ok((row.get(0)?, row.get(1)?)),
        )
        .expect("NewConnect directory adapter should be seeded");
    let bankier_adapter_name: String = connection
        .query_row(
            "SELECT display_name FROM source_adapters WHERE id = 'bankier-market-rss'",
            [],
            |row| row.get(0),
        )
        .expect("Bankier adapter should be seeded");
    let bankier_company_adapter_name: String = connection
        .query_row(
            "SELECT display_name FROM source_adapters WHERE id = 'bankier-company-komunikaty'",
            [],
            |row| row.get(0),
        )
        .expect("Bankier company adapter should be seeded");
    let portal_analiz_adapter: (String, bool) = connection
        .query_row(
            "SELECT display_name, enabled FROM source_adapters WHERE id = 'portal-analiz'",
            [],
            |row| Ok((row.get(0)?, row.get(1)?)),
        )
        .expect("Portal Analiz placeholder should be seeded");
    let gpw_events_adapter: (String, bool) = connection
        .query_row(
            "SELECT display_name, enabled FROM source_adapters WHERE id = 'gpw-market-events-rss'",
            [],
            |row| Ok((row.get(0)?, row.get(1)?)),
        )
        .expect("GPW market events adapter should be seeded");
    let bankier_calendar_adapter: (String, bool) = connection
            .query_row(
                "SELECT display_name, enabled FROM source_adapters WHERE id = 'bankier-kalendarium-html'",
                [],
                |row| Ok((row.get(0)?, row.get(1)?)),
            )
            .expect("Bankier calendar adapter should be seeded");

    assert_eq!(theme, "dark");
    assert_eq!(locale, "en");
    assert_eq!(general_analysis_model, "gemini-2.5-flash");
    assert_eq!(developer_mode, "false");
    assert_eq!(log_level, "info");
    assert_eq!(gpw_adapter, ("GPW ESPI/EBI".to_owned(), false));
    assert_eq!(registry_adapter_name, "GPW Company Registry");
    assert_eq!(
        newconnect_directory_adapter,
        ("NewConnect Company Directory".to_owned(), true)
    );
    assert_eq!(bankier_adapter_name, "Bankier Giełda RSS");
    assert_eq!(bankier_company_adapter_name, "Bankier Company Komunikaty");
    assert_eq!(portal_analiz_adapter, ("Portal Analiz".to_owned(), false));
    assert_eq!(
        gpw_events_adapter,
        ("GPW Market Events RSS".to_owned(), true)
    );
    assert_eq!(
        bankier_calendar_adapter,
        ("Bankier Kalendarium".to_owned(), true)
    );
}

#[test]
fn reports_database_status() {
    let connection = open_in_memory_database().expect("database should initialize");
    let status = database_status(&connection).expect("status should be available");

    assert_eq!(status.applied_migrations, 29);
    assert_eq!(status.companies, 0);
    assert_eq!(status.source_adapters, 12);
    assert_eq!(status.settings, 17);
}
