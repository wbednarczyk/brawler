use std::path::Path;

use rusqlite::Connection;
use thiserror::Error;

#[derive(Debug, Error)]
pub enum StorageError {
    #[error("sqlite error: {0}")]
    Sqlite(#[from] rusqlite::Error),
}

pub type StorageResult<T> = Result<T, StorageError>;

#[derive(Debug, Clone, Copy)]
struct Migration {
    version: i64,
    name: &'static str,
    sql: &'static str,
}

const MIGRATIONS: &[Migration] = &[Migration {
    version: 1,
    name: "initial_schema",
    sql: include_str!("../migrations/0001_initial.sql"),
}];

pub fn open_database(path: impl AsRef<Path>) -> StorageResult<Connection> {
    let mut connection = Connection::open(path)?;
    apply_migrations(&mut connection)?;
    Ok(connection)
}

pub fn open_in_memory_database() -> StorageResult<Connection> {
    let mut connection = Connection::open_in_memory()?;
    apply_migrations(&mut connection)?;
    Ok(connection)
}

fn apply_migrations(connection: &mut Connection) -> StorageResult<()> {
    connection.pragma_update(None, "foreign_keys", "ON")?;
    connection.execute_batch(
        "
        CREATE TABLE IF NOT EXISTS schema_migrations (
            version INTEGER PRIMARY KEY,
            name TEXT NOT NULL,
            applied_at TEXT NOT NULL DEFAULT (strftime('%Y-%m-%dT%H:%M:%fZ', 'now'))
        );
        ",
    )?;

    let transaction = connection.transaction()?;

    for migration in MIGRATIONS {
        let already_applied: bool = transaction.query_row(
            "SELECT EXISTS(SELECT 1 FROM schema_migrations WHERE version = ?1)",
            [migration.version],
            |row| row.get(0),
        )?;

        if already_applied {
            continue;
        }

        transaction.execute_batch(migration.sql)?;
        transaction.execute(
            "INSERT INTO schema_migrations (version, name) VALUES (?1, ?2)",
            (migration.version, migration.name),
        )?;
    }

    transaction.commit()?;
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn creates_clean_database_with_initial_schema() {
        let connection = open_in_memory_database().expect("database should initialize");

        let migration_count: i64 = connection
            .query_row("SELECT COUNT(*) FROM schema_migrations", [], |row| {
                row.get(0)
            })
            .expect("schema_migrations should exist");

        assert_eq!(migration_count, 1);

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
    }

    #[test]
    fn seeds_default_settings_and_gpw_adapter() {
        let connection = open_in_memory_database().expect("database should initialize");

        let theme: String = connection
            .query_row(
                "SELECT value FROM settings WHERE key = 'theme'",
                [],
                |row| row.get(0),
            )
            .expect("theme setting should be seeded");

        let adapter_name: String = connection
            .query_row(
                "SELECT display_name FROM source_adapters WHERE id = 'gpw-espi-ebi'",
                [],
                |row| row.get(0),
            )
            .expect("GPW adapter should be seeded");

        assert_eq!(theme, "dark");
        assert_eq!(adapter_name, "GPW ESPI/EBI");
    }

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
}
