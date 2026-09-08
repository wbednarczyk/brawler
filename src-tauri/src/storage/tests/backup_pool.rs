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

/// The staged file was corrupted BEFORE `apply_staged_restore` ever ran
/// (#319 review): verification rejects it up front, so this exercises only
/// the pre-verification reject path, never the post-verify rollback branch —
/// see `apply_rolls_back_when_the_staged_file_is_corrupted_between_verification_and_swap`
/// for that one.
#[test]
fn apply_rejects_a_corrupt_staged_file_before_touching_the_database() {
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
/// afterward (disk tampering, partial write) BEFORE `apply_staged_restore`
/// ever runs — its own second verification pass catches it and rejects
/// before any swap starts, same branch as the test above, different setup
/// path. Real post-swap rollback (integrity_check failing AFTER the rename)
/// is exercised by
/// `apply_rolls_back_when_the_staged_file_is_corrupted_between_verification_and_swap`
/// below, via the test-only seam — corrupting the file here, before
/// `apply_staged_restore` starts, cannot reach that branch.
#[test]
fn apply_rejects_a_staged_file_corrupted_after_staging_but_before_apply_runs() {
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

/// The staged file passes BOTH verification passes and only becomes corrupt
/// in the window between the second (re-)verification and the swap — the
/// only way to reach the real post-rename rollback branch, exercised here
/// via the `apply_staged_restore_with` test-only seam (#319 review point 3a).
#[test]
fn apply_rolls_back_when_the_staged_file_is_corrupted_between_verification_and_swap() {
    let dir = temp_dir("apply-post-verify-corrupt");
    let db_path = dir.join("brawler.sqlite3");
    migrated_database_file(&db_path);
    let before_hash = sha256_file(&db_path);

    let backups = dir.join("backups");
    std::fs::create_dir_all(&backups).expect("backups dir");
    let backup_file = backups.join("backup-good.sqlite3");
    migrated_database_file(&backup_file);
    backup::request_restore(&dir, "backup-good.sqlite3").expect("stage restore");

    let notice = backup::apply_staged_restore_with(&db_path, &dir, |staged| {
        std::fs::write(staged, b"corrupted after verification, before swap")
            .expect("corrupt staged file between verify and swap");
    })
    .expect("apply must not error");
    assert!(
        notice.is_some(),
        "a candidate corrupted between verification and swap must be recoverably rejected"
    );

    assert_eq!(
        sha256_file(&db_path),
        before_hash,
        "the original database must be byte-identical — rolled back after the swap failed"
    );
    assert!(
        dir.join("restore-rejected.sqlite3").exists(),
        "the bad candidate should be kept for inspection"
    );
    assert!(!dir.join("restore-journal.json").exists());

    let _ = std::fs::remove_dir_all(&dir);
}

/// A failure before the sidecar-clear + rename (here: `backups/` cannot be
/// created because it already exists as a plain file) must be a recoverable
/// rejection, not fatal — nothing has touched the live database yet (#319 P2).
#[test]
fn apply_recovers_when_the_backups_directory_cannot_be_created_pre_swap() {
    let dir = temp_dir("backups-dir-blocked");
    let db_path = dir.join("brawler.sqlite3");
    migrated_database_file(&db_path);
    let before_hash = sha256_file(&db_path);

    // Pre-create `backups` as a FILE (not a directory) so `create_dir_all`
    // fails downstream.
    std::fs::write(dir.join("backups"), b"not a directory").expect("block backups dir");

    // A verified, staged candidate — built directly since staging via
    // `request_restore` doesn't touch `backups/` at all.
    migrated_database_file(&dir.join("restore-pending.sqlite3"));

    let notice = backup::apply_staged_restore(&db_path, &dir).expect("apply must not error");
    assert!(
        notice.is_some(),
        "a pre-swap failure must be a recoverable rejection, not an error"
    );

    assert_eq!(
        sha256_file(&db_path),
        before_hash,
        "the live database must be untouched by a pre-swap failure"
    );
    assert!(
        !dir.join("restore-journal.json").exists(),
        "no journal should be written for a pre-swap failure"
    );
    assert!(
        dir.join("restore-rejected.sqlite3").exists(),
        "the staged candidate should be kept for inspection"
    );

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

/// A crash mid-write of the journal itself (pre-fix `apply_staged_restore`
/// wrote it directly with `fs::write`, not atomically) leaves truncated,
/// unparseable JSON on disk. If the live database is intact, that must never
/// block startup forever (#319 P1) — the malformed journal is simply dropped.
#[test]
fn startup_tolerates_a_truncated_journal_when_the_database_is_intact() {
    let dir = temp_dir("journal-truncated-intact");
    let db_path = dir.join("brawler.sqlite3");
    migrated_database_file(&db_path);
    let expected_hash = sha256_file(&db_path);

    let journal_path = dir.join("restore-journal.json");
    std::fs::write(&journal_path, b"{\"pre_restore\":\"/tmp/truncated-mid-w")
        .expect("write truncated journal");

    let notice =
        backup::apply_staged_restore(&db_path, &dir).expect("apply/recover must not error");
    assert!(
        notice.is_none(),
        "an unreadable journal alongside an intact database is not a rejection notice"
    );

    assert_eq!(
        sha256_file(&db_path),
        expected_hash,
        "an intact database must be left untouched when the journal is unreadable"
    );
    assert!(
        !journal_path.exists(),
        "the malformed journal must be cleaned up"
    );

    let _ = std::fs::remove_dir_all(&dir);
}

/// Same crash-mid-journal-write scenario, but the live database is ALSO bad
/// (the swap really was interrupted) — with no readable journal to name the
/// pre-restore source, recovery must fall back to the newest
/// `pre-restore-*.sqlite3` in `backups/` rather than failing startup forever
/// (#319 P1).
#[test]
fn startup_falls_back_to_the_newest_pre_restore_copy_when_the_journal_is_unreadable() {
    let dir = temp_dir("journal-truncated-fallback");
    let db_path = dir.join("brawler.sqlite3");
    std::fs::write(&db_path, b"not a database").expect("write bad live db");

    let backups = dir.join("backups");
    std::fs::create_dir_all(&backups).expect("backups dir");

    // An older pre-restore copy that must NOT be picked.
    let older = backups.join("pre-restore-100.sqlite3");
    migrated_database_file(&older);
    {
        let connection = Connection::open(&older).expect("open older pre-restore copy");
        connection
            .execute(
                "INSERT INTO companies (id, exchange, ticker, qualified_ticker, display_name)
                 VALUES ('company_gpw_old', 'GPW', 'OLD', 'GPW:OLD', 'Old S.A.')",
                [],
            )
            .expect("seed older copy");
    }
    // Ensure the newer file's mtime actually sorts after the older one —
    // some filesystems have coarse mtime resolution.
    std::thread::sleep(std::time::Duration::from_millis(20));

    let newer = backups.join("pre-restore-200.sqlite3");
    migrated_database_file(&newer);
    let expected_hash = sha256_file(&newer);

    let journal_path = dir.join("restore-journal.json");
    std::fs::write(&journal_path, b"{not valid json at all").expect("write malformed journal");

    let notice =
        backup::apply_staged_restore(&db_path, &dir).expect("apply/recover must not error");
    assert!(notice.is_none());

    assert_eq!(
        sha256_file(&db_path),
        expected_hash,
        "the newest pre-restore copy must be recovered onto the live path"
    );
    assert!(
        !journal_path.exists(),
        "the malformed journal must be cleaned up"
    );

    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn committed_rows_in_the_original_wal_survive_a_refused_restore() {
    let dir = temp_dir("wal-survive");
    let db_path = dir.join("brawler.sqlite3");
    migrated_database_file(&db_path);

    // Commit a row under WAL WITHOUT checkpointing — it lives only in the
    // `-wal` sidecar until something checkpoints it. The connection stays
    // OPEN across the `apply_staged_restore` call below: closing the only
    // connection to a WAL database auto-checkpoints it, which would defeat
    // this test's setup and hide the very bug it exists to catch (#319
    // review point 3b — the previous version closed the connection before
    // calling apply, so it could never have detected lost WAL-only rows).
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
    let wal_sidecar = std::path::PathBuf::from(format!("{}-wal", db_path.display()));
    assert!(
        std::fs::metadata(&wal_sidecar)
            .map(|metadata| metadata.len() > 0)
            .unwrap_or(false),
        "the WAL sidecar should be non-empty right before calling apply"
    );

    // A refused restore must never touch the original database or its WAL.
    std::fs::write(dir.join("restore-pending.sqlite3"), b"not a database")
        .expect("write bad staged file");
    let notice = backup::apply_staged_restore(&db_path, &dir).expect("apply must not error");
    assert!(notice.is_some());

    drop(connection);
    let reopened = Connection::open(&db_path).expect("reopen db");
    let ticker: String = reopened
        .query_row(
            "SELECT ticker FROM companies WHERE id = 'company_gpw_wal'",
            [],
            |row| row.get(0),
        )
        .expect("the uncheckpointed row must survive a refused restore");
    assert_eq!(ticker, "WAL");

    let _ = std::fs::remove_dir_all(&dir);
}

/// The other half of point 3b: when the restore actually SUCCEEDS, the
/// pre-restore copy `apply_staged_restore` writes must contain the WAL-only
/// row — proving the checkpoint (`PRAGMA wal_checkpoint(TRUNCATE)`) runs
/// BEFORE the original is copied aside, not after.
#[test]
fn pre_restore_copy_captures_wal_only_rows_when_a_restore_succeeds() {
    let dir = temp_dir("wal-checkpoint-on-restore");
    let db_path = dir.join("brawler.sqlite3");
    migrated_database_file(&db_path);

    let connection = Connection::open(&db_path).expect("open db");
    connection
        .pragma_update(None, "journal_mode", "WAL")
        .expect("wal mode");
    connection
        .execute(
            "INSERT INTO companies (id, exchange, ticker, qualified_ticker, display_name)
             VALUES ('company_gpw_wal2', 'GPW', 'WA2', 'GPW:WA2', 'Wal Two S.A.')",
            [],
        )
        .expect("insert row");
    let wal_sidecar = std::path::PathBuf::from(format!("{}-wal", db_path.display()));
    assert!(
        std::fs::metadata(&wal_sidecar)
            .map(|metadata| metadata.len() > 0)
            .unwrap_or(false),
        "the WAL sidecar should be non-empty right before calling apply"
    );

    let backups = dir.join("backups");
    std::fs::create_dir_all(&backups).expect("backups dir");
    let backup_file = backups.join("backup-good.sqlite3");
    migrated_database_file(&backup_file);
    backup::request_restore(&dir, "backup-good.sqlite3").expect("stage restore");

    let notice = backup::apply_staged_restore(&db_path, &dir).expect("apply must not error");
    assert!(notice.is_none(), "a verified restore should succeed");

    drop(connection);

    let pre_restore_file = std::fs::read_dir(&backups)
        .expect("read backups dir")
        .filter_map(|entry| entry.ok())
        .map(|entry| entry.path())
        .find(|path| {
            path.file_name()
                .and_then(|name| name.to_str())
                .map(|name| name.starts_with("pre-restore-"))
                .unwrap_or(false)
        })
        .expect("a pre-restore copy should have been written");

    let pre_restore_connection =
        Connection::open(&pre_restore_file).expect("open pre-restore copy");
    let ticker: String = pre_restore_connection
        .query_row(
            "SELECT ticker FROM companies WHERE id = 'company_gpw_wal2'",
            [],
            |row| row.get(0),
        )
        .expect("the WAL-only row must be present in the pre-restore copy");
    assert_eq!(ticker, "WA2");

    let _ = std::fs::remove_dir_all(&dir);
}
