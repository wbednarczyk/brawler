use super::*;
use crate::storage::migrations::{apply_migrations_up_to, count_applied_migrations, count_rows};

#[test]
fn rerunning_migrations_is_idempotent_and_preserves_data() {
    let mut connection = open_in_memory_database().expect("database should initialize");
    let expected = expected_migration_count();

    connection
        .execute(
            "INSERT INTO companies (id, exchange, ticker, qualified_ticker, display_name)
             VALUES ('c1', 'GPW', 'CDR', 'GPW:CDR', 'CD PROJEKT S.A.')",
            [],
        )
        .expect("seed a company");

    // Re-running the runner must be a safe no-op: no error, no duplicated
    // migration rows, and existing tables + data survive. This is the v0.40.0
    // "no such table" / silent-skip class encoded as a guard.
    apply_migrations(&mut connection).expect("re-running migrations should be safe");

    assert_eq!(
        count_applied_migrations(&connection).expect("count applied"),
        expected,
        "re-run must not add or drop migration rows",
    );
    assert_eq!(
        count_rows(&connection, "companies").expect("count companies"),
        1,
        "re-running migrations must not wipe existing data",
    );
}

#[test]
fn older_database_upgrades_to_latest_without_losing_data() {
    // Corpus-style upgrade path: a historical database stuck at the initial
    // schema, with data, must upgrade cleanly to the latest schema with the
    // pre-existing data intact — without needing a binary .sqlite snapshot.
    let mut connection = rusqlite::Connection::open_in_memory().expect("open in-memory database");
    apply_migrations_up_to(&mut connection, 1).expect("apply initial migration");
    assert_eq!(
        count_applied_migrations(&connection).expect("count applied"),
        1,
        "database should be pinned to the initial schema",
    );

    connection
        .execute(
            "INSERT INTO companies (id, exchange, ticker, qualified_ticker, display_name)
             VALUES ('legacy', 'GPW', 'PKN', 'GPW:PKN', 'ORLEN S.A.')",
            [],
        )
        .expect("seed legacy data on the old schema");

    apply_migrations(&mut connection).expect("upgrade to the latest schema");

    assert_eq!(
        count_applied_migrations(&connection).expect("count applied"),
        expected_migration_count(),
        "upgrade should reach the latest migration",
    );
    let survived: i64 = connection
        .query_row(
            "SELECT COUNT(*) FROM companies WHERE id = 'legacy'",
            [],
            |row| row.get(0),
        )
        .expect("query legacy company");
    assert_eq!(survived, 1, "legacy data must survive the upgrade");
}
