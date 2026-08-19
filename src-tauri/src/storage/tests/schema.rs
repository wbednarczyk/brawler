use super::*;

#[test]
fn creates_clean_database_with_initial_schema() {
    let connection = open_in_memory_database().expect("database should initialize");

    let migration_count: i64 = connection
        .query_row("SELECT COUNT(*) FROM schema_migrations", [], |row| {
            row.get(0)
        })
        .expect("schema_migrations should exist");

    assert_eq!(migration_count, super::expected_migration_count());

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

    let research_review_checkpoints_table_exists: bool = connection
        .query_row(
            "SELECT EXISTS(
                    SELECT 1
                    FROM sqlite_master
                    WHERE type = 'table' AND name = 'research_review_checkpoints'
                )",
            [],
            |row| row.get(0),
        )
        .expect("research review checkpoint table lookup should work");
    let evidence_links_table_exists: bool = connection
        .query_row(
            "SELECT EXISTS(
                    SELECT 1
                    FROM sqlite_master
                    WHERE type = 'table' AND name = 'evidence_links'
                )",
            [],
            |row| row.get(0),
        )
        .expect("evidence links table lookup should work");

    assert!(research_review_checkpoints_table_exists);
    assert!(evidence_links_table_exists);
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
    assert_eq!(developer_mode, "false");
    assert_eq!(log_level, "info");
    // v0.55 T3 (ADR 0069 D2): migration 0081 re-enabled gpw-espi-ebi as the
    // reconciliation witness (enabled = 1), reversing the 0011 disable.
    assert_eq!(gpw_adapter, ("GPW ESPI/EBI".to_owned(), true));
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

    assert_eq!(status.applied_migrations, super::expected_migration_count());
    assert_eq!(status.companies, 0);
    // DB-seeded adapter rows must match the code registry, derived — never a
    // hand-counted constant (guardrail 2026-07-19; content parity is guarded by
    // `registry_matches_seeded_catalog`).
    assert_eq!(
        status.source_adapters,
        crate::source_adapters::registry::REGISTRY.len() as i64
    );
    // Seeded settings after the ADR 0084 clean cut: migration 0102 deletes the
    // seven retired AI rows (analysis mode, ai_workers, ai_provider_concurrency,
    // capability_providers, general_analysis_*, espi_ai_fallback_enabled,
    // history_sweep_ai_call_limit) plus the general-analysis model/timeout rows.
    assert_eq!(status.settings, 19);
}

/// Guard (issue #404 H6): every write transaction must open `IMMEDIATE` via
/// `rusqlite::Transaction::new_unchecked(&conn, TransactionBehavior::Immediate)`.
/// The `unchecked_transaction` connection method and the bare, argument-less
/// `transaction` call (rusqlite's DEFERRED default) both open DEFERRED, so a
/// read->write upgrade under WAL returns `SQLITE_BUSY_SNAPSHOT` immediately,
/// bypassing `busy_timeout` entirely (harvest 2026-08-19). Empty allowlist by
/// design. The banned needles are built via `format!` so this scan never
/// flags its own source line.
#[test]
fn no_write_transaction_is_deferred() {
    let unchecked_needle = format!("{}{}", "unchecked_transaction", "(");
    let bare_needle = format!("{}{}", ".transaction", "()");

    let src_dir = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("src");
    let mut violations = Vec::new();
    let mut stack = vec![src_dir];
    while let Some(dir) = stack.pop() {
        for entry in std::fs::read_dir(&dir).expect("readable source dir") {
            let path = entry.expect("readable dir entry").path();
            if path.is_dir() {
                stack.push(path);
            } else if path.extension().is_some_and(|ext| ext == "rs") {
                let content = std::fs::read_to_string(&path).expect("readable source file");
                for (line_no, line) in content.lines().enumerate() {
                    if line.contains(&unchecked_needle) || line.contains(&bare_needle) {
                        violations.push(format!(
                            "{}:{}: {}",
                            path.display(),
                            line_no + 1,
                            line.trim()
                        ));
                    }
                }
            }
        }
    }

    assert!(
        violations.is_empty(),
        "DEFERRED write transaction(s) found (#404 H6); use \
         rusqlite::Transaction::new_unchecked(&conn, rusqlite::TransactionBehavior::Immediate) \
         instead:\n{}",
        violations.join("\n")
    );
}
