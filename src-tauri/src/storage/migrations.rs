use super::*;
use std::path::Path;

#[path = "migrations_list.rs"]
mod migrations_list;
use migrations_list::MIGRATIONS;

pub fn open_database(path: impl AsRef<Path>) -> StorageResult<Connection> {
    let mut connection = Connection::open(path)?;
    apply_migrations(&mut connection)?;
    Ok(connection)
}

/// Opens WITHOUT applying migrations, enforced by SQLite's own read-only
/// flag — for read-only diagnostics pointed at a real database (sol review
/// finding 5: `open_database` migrates on open, so a "read-only" harvest
/// against the owner's copy silently rewrote it through 0141+). A schema too
/// old for the caller's query fails loudly at the query, never by mutating
/// the file. `no_mutex` matches the pool's threading assumption elsewhere;
/// diagnostics are single-threaded anyway.
pub fn open_database_readonly(path: impl AsRef<Path>) -> StorageResult<Connection> {
    use rusqlite::OpenFlags;
    Ok(Connection::open_with_flags(
        path,
        OpenFlags::SQLITE_OPEN_READ_ONLY | OpenFlags::SQLITE_OPEN_NO_MUTEX,
    )?)
}

pub fn open_in_memory_database() -> StorageResult<Connection> {
    let mut connection = Connection::open_in_memory()?;
    apply_migrations(&mut connection)?;
    Ok(connection)
}

pub(super) fn database_status(connection: &Connection) -> StorageResult<DatabaseStatus> {
    Ok(DatabaseStatus {
        applied_migrations: count_rows(connection, "schema_migrations")?,
        companies: count_rows(connection, "companies")?,
        source_adapters: count_rows(connection, "source_adapters")?,
        settings: count_rows(connection, "settings")?,
    })
}

#[cfg(test)]
pub(super) fn expected_migration_count() -> i64 {
    MIGRATIONS.len() as i64
}

pub(super) fn migration_count() -> i64 {
    MIGRATIONS.len() as i64
}

/// Number of applied migrations, or 0 when the database has no migration table
/// yet (a brand-new database).
pub(super) fn count_applied_migrations(connection: &Connection) -> StorageResult<i64> {
    let table_exists: bool = connection.query_row(
        "SELECT EXISTS(SELECT 1 FROM sqlite_master WHERE type = 'table' AND name = 'schema_migrations')",
        [],
        |row| row.get(0),
    )?;

    if !table_exists {
        return Ok(0);
    }

    count_rows(connection, "schema_migrations")
}

pub(super) fn apply_migrations(connection: &mut Connection) -> StorageResult<()> {
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

    let transaction =
        connection.transaction_with_behavior(rusqlite::TransactionBehavior::Immediate)?;

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

pub(super) fn count_rows(connection: &Connection, table_name: &str) -> StorageResult<i64> {
    let sql = format!("SELECT COUNT(*) FROM {table_name}");

    connection
        .query_row(&sql, [], |row| row.get(0))
        .map_err(StorageError::from)
}

/// Test-only: apply migrations up to (and including) `max_version`, leaving the
/// database at a historical schema so an upgrade path can be exercised without
/// shipping binary `.sqlite` snapshots (ADR 0048 migration-safety coverage).
#[cfg(test)]
pub(super) fn apply_migrations_up_to(
    connection: &mut Connection,
    max_version: i64,
) -> StorageResult<()> {
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

    let transaction =
        connection.transaction_with_behavior(rusqlite::TransactionBehavior::Immediate)?;

    for migration in MIGRATIONS {
        if migration.version > max_version {
            break;
        }

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
mod migration_invariants {
    use super::MIGRATIONS;

    #[test]
    fn versions_are_contiguous_unique_and_ordered() {
        // Migrations are append-only and immutable once shipped (CLAUDE.md): a
        // reused, out-of-order, or gapped version number is the mistake this
        // guards. They must be exactly 1..=N in declaration order.
        for (index, migration) in MIGRATIONS.iter().enumerate() {
            let expected = index as i64 + 1;
            assert_eq!(
                migration.version, expected,
                "migration #{index} ('{}') has version {} but must be {} (contiguous, ordered, unique)",
                migration.name, migration.version, expected,
            );
        }
    }

    /// Guardrail harvest, epic #229 T3 (#140/#171): **no migration may delete
    /// report documents on URL evidence.**
    ///
    /// `0107_repair_misassociation_and_note_ref_facts.sql` did exactly that —
    /// `company_id = 'company_gpw_cbf' AND url LIKE '%energa%'` — on the belief
    /// that a `Grupy-Energa` filename meant an Energa filing. The bytes say
    /// otherwise: all four deleted files are cyber_Folks' own Q3-2024 filing
    /// (PDF `/Author: cyber_Folks`, body "Raport kwartalny Grupy cyber_Folks_"),
    /// and the attachment host simply reuses one issuer's filename across
    /// unrelated same-day filings. Four legitimate periodic documents were lost.
    ///
    /// Only document **content** can settle an association, and SQL cannot read
    /// bytes — so a migration is structurally the wrong instrument for this
    /// class. 0107 is grandfathered (immutable once applied); every later
    /// migration must fail here instead of repeating it.
    #[test]
    fn no_migration_deletes_report_documents_by_url_pattern() {
        const GRANDFATHERED: i64 = 107;
        let mut offenders = Vec::new();
        for migration in MIGRATIONS {
            if migration.version == GRANDFATHERED {
                continue;
            }
            let sql = migration.sql.to_lowercase();
            for statement in sql.split(';') {
                let Some(start) = statement.find("delete from report_documents") else {
                    continue;
                };
                let predicate = &statement[start..];
                if predicate.contains("url") && predicate.contains("like") {
                    offenders.push(format!(
                        "  {:04}_{} deletes report_documents on a URL pattern",
                        migration.version, migration.name
                    ));
                }
            }
        }
        assert!(
            offenders.is_empty(),
            "a report document's owner cannot be decided from its URL — the host reuses one \
             issuer's filename across unrelated filings, and migration 0107 destroyed four \
             legitimate cyber_Folks statements that way. Verify content, not slugs:\n{}",
            offenders.join("\n")
        );
    }

    #[test]
    fn names_are_unique() {
        let mut names: Vec<&str> = MIGRATIONS.iter().map(|migration| migration.name).collect();
        let total = names.len();
        names.sort_unstable();
        names.dedup();
        assert_eq!(names.len(), total, "migration names must be unique");
    }

    /// Guardrail harvest (#358, sol F7): the contiguity/count checks above only
    /// ever see what IS registered in `MIGRATIONS`, so a `.sql` file dropped into
    /// `migrations/` and never wired into this const passes every existing gate
    /// silently — it never applies, and no test catches the drift. This walks
    /// the actual `migrations/*.sql` directory and asserts the file inventory and
    /// `MIGRATIONS` agree exactly, in both directions.
    #[test]
    fn every_migration_file_is_registered_and_vice_versa() {
        let migrations_dir = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("migrations");
        let mut file_versions: Vec<i64> =
            std::fs::read_dir(&migrations_dir)
                .expect("read migrations dir")
                .filter_map(|entry| {
                    let entry = entry.expect("dir entry");
                    let name = entry.file_name();
                    let name = name.to_str().expect("utf8 filename").to_owned();
                    if !name.ends_with(".sql") {
                        return None;
                    }
                    let prefix = &name[..4];
                    Some(prefix.parse::<i64>().unwrap_or_else(|_| {
                        panic!("migration file '{name}' has no numeric prefix")
                    }))
                })
                .collect();
        file_versions.sort_unstable();

        let mut registered_versions: Vec<i64> = MIGRATIONS
            .iter()
            .map(|migration| migration.version)
            .collect();
        registered_versions.sort_unstable();

        assert_eq!(
            file_versions, registered_versions,
            "every migrations/*.sql file must have exactly one MIGRATIONS entry, and vice versa \
             (a file present but unregistered silently never applies)"
        );
    }
}

#[cfg(test)]
mod job_runs_migration {
    use super::*;

    /// Migration 0153 (`job_runs`, ADR 0109) applies cleanly to a COPY of the
    /// owner's real database, and a second apply on the same connection is a
    /// no-op (idempotent — `apply_migrations` skips already-applied versions).
    /// Skipped when the read-only snapshot is not present in this sandbox
    /// (CI / a fresh checkout never has it).
    #[test]
    fn applies_to_real_snapshot_and_reapply_is_idempotent() {
        let snapshot = std::path::Path::new(
            "/tmp/claude-1000/-home-wojtas-projects-brawler/d9ef921f-6b1b-4904-8b65-b5f67e25e394/scratchpad/realdb/snap.sqlite3",
        );
        if !snapshot.exists() {
            eprintln!("real snapshot not present in this sandbox, skipping");
            return;
        }
        let dir =
            std::env::temp_dir().join(format!("brawler-job-runs-migration-{}", std::process::id()));
        std::fs::create_dir_all(&dir).expect("scratch dir");
        let copy_path = dir.join("snap-copy.sqlite3");
        std::fs::copy(snapshot, &copy_path).expect("copy real snapshot");

        let mut connection = Connection::open(&copy_path).expect("open copy");
        apply_migrations(&mut connection).expect("apply migrations to real snapshot copy");

        let table_exists: bool = connection
            .query_row(
                "SELECT EXISTS(SELECT 1 FROM sqlite_master WHERE type = 'table' AND name = 'job_runs')",
                [],
                |row| row.get(0),
            )
            .expect("check job_runs exists");
        assert!(table_exists, "job_runs table must exist after migration");

        // Re-apply is a no-op: same migration count, no error.
        let before = count_applied_migrations(&connection).expect("count before");
        apply_migrations(&mut connection).expect("re-apply is idempotent");
        let after = count_applied_migrations(&connection).expect("count after");
        assert_eq!(
            before, after,
            "re-applying migrations must not change the count"
        );

        let _ = std::fs::remove_dir_all(&dir);
    }
}
