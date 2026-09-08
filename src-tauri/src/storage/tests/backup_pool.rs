use super::*;
use crate::storage::{backup, migrations, pool};
use rusqlite::Connection;
use sha2::{Digest, Sha256};
use std::time::{SystemTime, UNIX_EPOCH};

fn temp_dir(tag: &str) -> std::path::PathBuf {
    let nanos = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .expect("clock")
        .as_nanos();
    let dir = std::env::temp_dir().join(format!("brawler-test-{tag}-{nanos}"));
    std::fs::create_dir_all(&dir).expect("temp dir");
    dir
}

/// A real, fully migrated Brawler database file — the shape
/// `verify_backup_file` (#319) accepts, as opposed to a bare marker table.
fn migrated_database_file(path: &std::path::Path) {
    let mut connection = Connection::open(path).expect("open db file");
    migrations::apply_migrations(&mut connection).expect("apply migrations");
}

fn sha256_file(path: &std::path::Path) -> String {
    let bytes = std::fs::read(path).expect("read file for hashing");
    let digest = Sha256::digest(&bytes);
    let mut hex = String::with_capacity(digest.len() * 2);
    for byte in digest {
        hex.push_str(&format!("{byte:02x}"));
    }
    hex
}

#[test]
fn snapshot_written_when_existing_database_has_pending_migrations() {
    let dir = temp_dir("snapshot");
    let db_path = dir.join("brawler.sqlite3");
    let connection = Connection::open(&db_path).expect("open file database");

    // Simulate an existing database that is one migration behind.
    connection
        .execute_batch(
            "CREATE TABLE schema_migrations (version INTEGER PRIMARY KEY, name TEXT NOT NULL, applied_at TEXT NOT NULL DEFAULT '');",
        )
        .expect("create migrations table");
    let expected = migrations::migration_count();
    for version in 1..expected {
        connection
            .execute(
                "INSERT INTO schema_migrations (version, name) VALUES (?1, 'seed')",
                [version],
            )
            .expect("seed migration row");
    }

    let snapshot = backup::snapshot_before_migrations(&connection, &dir).expect("snapshot");
    let snapshot_path = snapshot.expect("a snapshot should be written");
    assert!(snapshot_path.exists(), "snapshot file should exist on disk");

    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn snapshot_skipped_for_fresh_database() {
    let dir = temp_dir("fresh");
    let db_path = dir.join("brawler.sqlite3");
    let connection = Connection::open(&db_path).expect("open file database");

    // No schema_migrations table yet: a brand-new database has nothing to restore.
    let snapshot = backup::snapshot_before_migrations(&connection, &dir).expect("snapshot");
    assert!(snapshot.is_none());

    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn pool_config_uses_defaults_and_clamps_out_of_range_values() {
    let connection = open_in_memory_database().expect("database should initialize");

    let defaults = pool::read_pool_config(&connection);
    assert_eq!(defaults.max_connections, 4);
    assert_eq!(defaults.busy_timeout_ms, 5_000);
    assert_eq!(defaults.acquire_timeout_ms, 10_000);

    connection
        .execute(
            "INSERT OR REPLACE INTO settings (key, value) VALUES ('db_max_connections', '999')",
            [],
        )
        .expect("insert oversized pool size");
    connection
        .execute(
            "INSERT OR REPLACE INTO settings (key, value) VALUES ('db_acquire_timeout_ms', '0')",
            [],
        )
        .expect("insert tiny acquire timeout");

    let clamped = pool::read_pool_config(&connection);
    assert_eq!(
        clamped.max_connections, 16,
        "max connections clamps to ceiling"
    );
    assert_eq!(
        clamped.acquire_timeout_ms, 1_000,
        "acquire timeout clamps to floor"
    );
}

#[test]
fn rotating_backups_prune_to_retention() {
    let dir = temp_dir("rotate");
    let state = AppState::with_data_dir(
        open_in_memory_database().expect("database should initialize"),
        dir.clone(),
    );

    for _ in 0..8 {
        state.create_backup().expect("backup should be created");
    }

    let status = state.backup_status().expect("status should be available");
    let rotating = status
        .backups
        .iter()
        .filter(|backup| backup.kind == "rotating")
        .count();
    assert!(
        rotating <= 5,
        "rotating backups should be pruned to retention, found {rotating}"
    );
    assert!(status.last_backup_at.is_some());

    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn restore_stages_backup_and_apply_swaps_database() {
    let dir = temp_dir("restore");
    let db_path = dir.join("brawler.sqlite3");
    let backups = dir.join("backups");
    std::fs::create_dir_all(&backups).expect("backups dir");

    let backup_file = backups.join("backup-good.sqlite3");
    migrated_database_file(&backup_file);
    {
        let connection = Connection::open(&backup_file).expect("open backup db");
        connection
            .execute(
                "INSERT INTO companies (id, exchange, ticker, qualified_ticker, display_name)
                 VALUES ('company_gpw_test', 'GPW', 'TST', 'GPW:TST', 'Test S.A.')",
                [],
            )
            .expect("seed company row");
    }

    backup::request_restore(&dir, "backup-good.sqlite3").expect("stage restore");
    assert!(dir.join("restore-pending.sqlite3").exists());

    let notice = backup::apply_staged_restore(&db_path, &dir).expect("apply restore");
    assert!(
        notice.is_none(),
        "a verified restore is not a rejection notice"
    );
    assert!(
        !dir.join("restore-pending.sqlite3").exists(),
        "staging file should be consumed"
    );

    let restored = Connection::open(&db_path).expect("open restored db");
    let ticker: String = restored
        .query_row(
            "SELECT ticker FROM companies WHERE id = 'company_gpw_test'",
            [],
            |row| row.get(0),
        )
        .expect("company row should be present in restored database");
    assert_eq!(ticker, "TST");

    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn restore_rejects_path_traversal() {
    let dir = temp_dir("traversal");
    let result = backup::request_restore(&dir, "../evil.sqlite3");
    assert!(result.is_err(), "path traversal must be rejected");
    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn request_restore_reports_a_missing_file() {
    let dir = temp_dir("missing");
    let result = backup::request_restore(&dir, "does-not-exist.sqlite3");
    assert!(result.is_err(), "a missing backup file must be reported");
    let _ = std::fs::remove_dir_all(&dir);
}

/// #319: a bare-table SQLite file (the shape the pre-fix `apply_staged_restore`
/// happily swapped in) is not a Brawler database and must be refused, staging
/// nothing.
#[test]
fn request_restore_refuses_an_unrelated_sqlite_file() {
    let dir = temp_dir("unrelated");
    let backups = dir.join("backups");
    std::fs::create_dir_all(&backups).expect("backups dir");

    let backup_file = backups.join("backup-marker.sqlite3");
    {
        let connection = Connection::open(&backup_file).expect("open backup db");
        connection
            .execute_batch("CREATE TABLE marker (id INTEGER); INSERT INTO marker VALUES (42);")
            .expect("seed marker");
    }

    let result = backup::request_restore(&dir, "backup-marker.sqlite3");
    assert!(result.is_err(), "a non-Brawler sqlite file must be refused");
    assert!(
        !dir.join("restore-pending.sqlite3").exists(),
        "a refused candidate must not be staged"
    );

    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn request_restore_refuses_a_corrupt_file_and_stages_nothing() {
    let dir = temp_dir("corrupt");
    let backups = dir.join("backups");
    std::fs::create_dir_all(&backups).expect("backups dir");

    let backup_file = backups.join("backup-corrupt.sqlite3");
    std::fs::write(&backup_file, b"not a database").expect("write corrupt file");

    let result = backup::request_restore(&dir, "backup-corrupt.sqlite3");
    assert!(result.is_err(), "a corrupt file must be refused");
    assert!(!dir.join("restore-pending.sqlite3").exists());

    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn request_restore_refuses_a_ledger_with_an_unknown_migration_version() {
    let dir = temp_dir("future-version");
    let backups = dir.join("backups");
    std::fs::create_dir_all(&backups).expect("backups dir");

    let backup_file = backups.join("backup-future.sqlite3");
    migrated_database_file(&backup_file);
    {
        let connection = Connection::open(&backup_file).expect("open backup db");
        connection
            .execute(
                "INSERT INTO schema_migrations (version, name) VALUES (999999, 'from_the_future')",
                [],
            )
            .expect("seed future migration row");
    }

    let result = backup::request_restore(&dir, "backup-future.sqlite3");
    assert!(
        result.is_err(),
        "a schema_migrations row this build does not recognize must be refused (a newer app version's database)"
    );
    assert!(!dir.join("restore-pending.sqlite3").exists());

    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn a_later_request_replaces_an_earlier_pending_one() {
    let dir = temp_dir("replace-pending");
    let backups = dir.join("backups");
    std::fs::create_dir_all(&backups).expect("backups dir");

    let first = backups.join("backup-first.sqlite3");
    migrated_database_file(&first);
    let second = backups.join("backup-second.sqlite3");
    migrated_database_file(&second);
    {
        let connection = Connection::open(&second).expect("open second backup");
        connection
            .execute(
                "INSERT INTO companies (id, exchange, ticker, qualified_ticker, display_name)
                 VALUES ('company_gpw_second', 'GPW', 'SEC', 'GPW:SEC', 'Second S.A.')",
                [],
            )
            .expect("seed company row");
    }

    backup::request_restore(&dir, "backup-first.sqlite3").expect("stage first");
    backup::request_restore(&dir, "backup-second.sqlite3").expect("stage second");

    let staged = Connection::open(dir.join("restore-pending.sqlite3")).expect("open staged file");
    let count: i64 = staged
        .query_row(
            "SELECT COUNT(*) FROM companies WHERE id = 'company_gpw_second'",
            [],
            |row| row.get(0),
        )
        .expect("query staged content");
    assert_eq!(
        count, 1,
        "the later request must replace the earlier pending file"
    );

    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn apply_keeps_the_original_when_the_staged_file_fails_verification() {
    let dir = temp_dir("apply-verify-fail");
    let db_path = dir.join("brawler.sqlite3");
    migrated_database_file(&db_path);
    let before_hash = sha256_file(&db_path);

    // Stage a bad candidate directly, bypassing `request_restore`'s own
    // verification, to exercise `apply_staged_restore`'s second pass.
    std::fs::write(dir.join("restore-pending.sqlite3"), b"not a database")
        .expect("write bad staged file");

    let notice = backup::apply_staged_restore(&db_path, &dir).expect("apply must not error");
    assert!(
        notice.is_some(),
        "a verification failure must surface as a recoverable notice, not an error"
    );

    assert_eq!(
        sha256_file(&db_path),
        before_hash,
        "the original database must be byte-identical — never touched"
    );
    assert!(
        dir.join("restore-rejected.sqlite3").exists(),
        "the bad candidate should be kept for inspection"
    );
    assert!(!dir.join("restore-journal.json").exists());
    assert!(!dir.join("restore-pending.sqlite3").exists());

    let _ = std::fs::remove_dir_all(&dir);
}

/// The staged file passed `request_restore`'s verification but was corrupted
/// afterward (disk tampering, partial write) — `apply_staged_restore`'s own
/// second verification pass is what this test exists to prove exists.
#[test]
fn apply_rolls_back_when_the_restored_file_does_not_open() {
    let dir = temp_dir("apply-rollback");
    let db_path = dir.join("brawler.sqlite3");
    migrated_database_file(&db_path);
    let before_hash = sha256_file(&db_path);

    let backups = dir.join("backups");
    std::fs::create_dir_all(&backups).expect("backups dir");
    let backup_file = backups.join("backup-good.sqlite3");
    migrated_database_file(&backup_file);

    backup::request_restore(&dir, "backup-good.sqlite3").expect("stage restore");

    // Corrupt the ALREADY-STAGED file after staging accepted it.
    std::fs::write(
        dir.join("restore-pending.sqlite3"),
        b"truncated after staging",
    )
    .expect("truncate staged file");

    let notice = backup::apply_staged_restore(&db_path, &dir).expect("apply must not error");
    assert!(notice.is_some());

    assert_eq!(
        sha256_file(&db_path),
        before_hash,
        "the original must be restored byte-for-byte"
    );
    assert!(dir.join("restore-rejected.sqlite3").exists());
    assert!(!dir.join("restore-journal.json").exists());

    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn startup_recovers_from_a_journal_left_by_a_crash_mid_apply() {
    let dir = temp_dir("crash-recover");
    let db_path = dir.join("brawler.sqlite3");

    // The state right after a crash mid-swap: a bad database at the live
    // path (the swap that never finished cleanly), a good pre-restore copy
    // sitting where the journal points, and the journal itself.
    std::fs::write(&db_path, b"not a database").expect("write bad live db");

    let backups = dir.join("backups");
    std::fs::create_dir_all(&backups).expect("backups dir");
    let pre_restore = backups.join("pre-restore-123.sqlite3");
    migrated_database_file(&pre_restore);
    let expected_hash = sha256_file(&pre_restore);

    let journal_path = dir.join("restore-journal.json");
    std::fs::write(
        &journal_path,
        format!(
            r#"{{"pre_restore":"{}","started_at":"2026-09-08T00:00:00Z"}}"#,
            pre_restore.to_string_lossy().replace('\\', "\\\\")
        ),
    )
    .expect("write journal");

    let notice =
        backup::apply_staged_restore(&db_path, &dir).expect("apply/recover must not error");
    assert!(
        notice.is_none(),
        "crash recovery on its own is not a rejection notice"
    );

    assert_eq!(
        sha256_file(&db_path),
        expected_hash,
        "the pre-restore copy must be recovered onto the live path"
    );
    assert!(!journal_path.exists(), "the journal must be cleaned up");

    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn startup_recovers_from_a_journal_when_the_restore_had_actually_completed() {
    let dir = temp_dir("crash-completed");
    let db_path = dir.join("brawler.sqlite3");
    migrated_database_file(&db_path);
    let expected_hash = sha256_file(&db_path);

    let backups = dir.join("backups");
    std::fs::create_dir_all(&backups).expect("backups dir");
    let pre_restore = backups.join("pre-restore-456.sqlite3");
    migrated_database_file(&pre_restore); // different content — must never be read

    let journal_path = dir.join("restore-journal.json");
    std::fs::write(
        &journal_path,
        format!(
            r#"{{"pre_restore":"{}","started_at":"2026-09-08T00:00:00Z"}}"#,
            pre_restore.to_string_lossy().replace('\\', "\\\\")
        ),
    )
    .expect("write journal");

    let notice =
        backup::apply_staged_restore(&db_path, &dir).expect("apply/recover must not error");
    assert!(notice.is_none());

    assert_eq!(
        sha256_file(&db_path),
        expected_hash,
        "a genuinely completed swap must be left untouched — the journal's cleanup step just didn't run"
    );
    assert!(!journal_path.exists(), "the journal must be cleaned up");

    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn committed_rows_in_the_original_wal_survive_a_refused_restore() {
    let dir = temp_dir("wal-survive");
    let db_path = dir.join("brawler.sqlite3");
    migrated_database_file(&db_path);

    // Commit a row under WAL WITHOUT checkpointing — it lives only in the
    // `-wal` sidecar until something checkpoints it. The sidecar check must
    // happen BEFORE the connection drops: closing the only connection to a
    // WAL database auto-checkpoints it, which would defeat the setup.
    {
        let connection = Connection::open(&db_path).expect("open db");
        connection
            .pragma_update(None, "journal_mode", "WAL")
            .expect("wal mode");
        connection
            .execute(
                "INSERT INTO companies (id, exchange, ticker, qualified_ticker, display_name)
                 VALUES ('company_gpw_wal', 'GPW', 'WAL', 'GPW:WAL', 'Wal S.A.')",
                [],
            )
            .expect("insert row");
        assert!(
            std::path::Path::new(&format!("{}-wal", db_path.display())).exists(),
            "the WAL sidecar should exist after an uncheckpointed write"
        );
    }

    // A refused restore must never touch the original database or its WAL.
    std::fs::write(dir.join("restore-pending.sqlite3"), b"not a database")
        .expect("write bad staged file");
    let notice = backup::apply_staged_restore(&db_path, &dir).expect("apply must not error");
    assert!(notice.is_some());

    let connection = Connection::open(&db_path).expect("reopen db");
    let ticker: String = connection
        .query_row(
            "SELECT ticker FROM companies WHERE id = 'company_gpw_wal'",
            [],
            |row| row.get(0),
        )
        .expect("the uncheckpointed row must survive a refused restore");
    assert_eq!(ticker, "WAL");

    let _ = std::fs::remove_dir_all(&dir);
}
