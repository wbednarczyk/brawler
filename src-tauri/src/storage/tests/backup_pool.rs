use super::*;
use crate::storage::{backup, migrations, pool};
use rusqlite::Connection;
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

    let backup_file = backups.join("backup-marker.sqlite3");
    {
        let connection = Connection::open(&backup_file).expect("open backup db");
        connection
            .execute_batch("CREATE TABLE marker (id INTEGER); INSERT INTO marker VALUES (42);")
            .expect("seed marker");
    }

    backup::request_restore(&dir, "backup-marker.sqlite3").expect("stage restore");
    assert!(dir.join("restore-pending.sqlite3").exists());

    backup::apply_staged_restore(&db_path, &dir).expect("apply restore");
    assert!(
        !dir.join("restore-pending.sqlite3").exists(),
        "staging file should be consumed"
    );

    let restored = Connection::open(&db_path).expect("open restored db");
    let value: i64 = restored
        .query_row("SELECT id FROM marker", [], |row| row.get(0))
        .expect("marker should be present in restored database");
    assert_eq!(value, 42);

    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn restore_rejects_path_traversal() {
    let dir = temp_dir("traversal");
    let result = backup::request_restore(&dir, "../evil.sqlite3");
    assert!(result.is_err(), "path traversal must be rejected");
    let _ = std::fs::remove_dir_all(&dir);
}
