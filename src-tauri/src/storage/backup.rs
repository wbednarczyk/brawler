use super::*;
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicU64, Ordering};
use std::time::{SystemTime, UNIX_EPOCH};
use time::format_description::well_known::Rfc3339;
use time::OffsetDateTime;

/// Per-process sequence so rapid backups never collide on a filename.
static BACKUP_SEQUENCE: AtomicU64 = AtomicU64::new(0);

/// Number of rotating automatic backups to keep. Pre-migration snapshots are
/// retained separately and not pruned by rotation.
const ROTATING_BACKUP_RETENTION: usize = 5;
const ROTATING_PREFIX: &str = "backup-";
const SNAPSHOT_PREFIX: &str = "pre-migration-";
const RESTORE_STAGING_FILE: &str = "restore-pending.sqlite3";
const DATABASE_FILE: &str = "brawler.sqlite3";

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct BackupEntry {
    pub file_name: String,
    pub created_at: Option<String>,
    pub kind: String,
    pub size_bytes: u64,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct BackupStatus {
    pub last_backup_at: Option<String>,
    pub backup_count: usize,
    pub backups: Vec<BackupEntry>,
}

/// Write a consistent, compacted copy of the database to `dest` using
/// `VACUUM INTO`. Safe to run on a live connection (ADR 0032).
pub(super) fn vacuum_into(connection: &Connection, dest: &Path) -> StorageResult<()> {
    connection.execute("VACUUM INTO ?1", [dest.to_string_lossy().as_ref()])?;
    Ok(())
}

fn backups_dir(data_dir: &Path) -> PathBuf {
    data_dir.join("backups")
}

fn unix_nanos() -> u128 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|elapsed| elapsed.as_nanos())
        .unwrap_or(0)
}

fn format_modified(path: &Path) -> Option<String> {
    let modified = std::fs::metadata(path).ok()?.modified().ok()?;
    OffsetDateTime::from(modified).format(&Rfc3339).ok()
}

/// Create a rotating automatic backup and prune to the retention limit.
pub(super) fn create_rotating_backup(
    connection: &Connection,
    data_dir: &Path,
) -> StorageResult<BackupStatus> {
    let dir = backups_dir(data_dir);
    std::fs::create_dir_all(&dir)?;
    let sequence = BACKUP_SEQUENCE.fetch_add(1, Ordering::Relaxed);
    let dest = dir.join(format!(
        "{ROTATING_PREFIX}{}-{sequence}.sqlite3",
        unix_nanos()
    ));
    vacuum_into(connection, &dest)?;
    prune_rotating_backups(&dir)?;
    collect_status(data_dir)
}

fn prune_rotating_backups(dir: &Path) -> StorageResult<()> {
    let mut rotating: Vec<(PathBuf, SystemTime)> = std::fs::read_dir(dir)?
        .filter_map(|entry| entry.ok())
        .map(|entry| entry.path())
        .filter(|path| {
            path.file_name()
                .and_then(|name| name.to_str())
                .map(|name| name.starts_with(ROTATING_PREFIX))
                .unwrap_or(false)
        })
        .filter_map(|path| {
            let modified = std::fs::metadata(&path).ok()?.modified().ok()?;
            Some((path, modified))
        })
        .collect();

    // Newest first; remove everything past the retention limit.
    rotating.sort_by_key(|(_, modified)| std::cmp::Reverse(*modified));
    for (path, _) in rotating.into_iter().skip(ROTATING_BACKUP_RETENTION) {
        let _ = std::fs::remove_file(path);
    }

    Ok(())
}

/// List all backups (rotating + pre-migration snapshots) with status metadata.
pub(super) fn collect_status(data_dir: &Path) -> StorageResult<BackupStatus> {
    let dir = backups_dir(data_dir);
    let mut entries: Vec<(BackupEntry, SystemTime)> = match std::fs::read_dir(&dir) {
        Ok(read_dir) => read_dir
            .filter_map(|entry| entry.ok())
            .filter_map(|entry| {
                let path = entry.path();
                let name = path.file_name()?.to_str()?.to_owned();
                let kind = if name.starts_with(ROTATING_PREFIX) {
                    "rotating"
                } else if name.starts_with(SNAPSHOT_PREFIX) {
                    "snapshot"
                } else {
                    return None;
                };
                let metadata = std::fs::metadata(&path).ok()?;
                let modified = metadata.modified().ok()?;
                Some((
                    BackupEntry {
                        file_name: name,
                        created_at: format_modified(&path),
                        kind: kind.to_owned(),
                        size_bytes: metadata.len(),
                    },
                    modified,
                ))
            })
            .collect(),
        Err(_) => Vec::new(),
    };

    entries.sort_by_key(|(_, modified)| std::cmp::Reverse(*modified));
    let last_backup_at = entries
        .first()
        .and_then(|(entry, _)| entry.created_at.clone());
    let backups: Vec<BackupEntry> = entries.into_iter().map(|(entry, _)| entry).collect();

    Ok(BackupStatus {
        backup_count: backups.len(),
        last_backup_at,
        backups,
    })
}

/// Stage a chosen backup for restore on the next launch. The file name must be a
/// plain backup file in the backups directory (no path traversal). The live
/// database is not touched; the swap happens at startup (ADR 0032).
pub(super) fn request_restore(data_dir: &Path, file_name: &str) -> StorageResult<()> {
    if file_name.contains('/') || file_name.contains('\\') || file_name.contains("..") {
        return Err(StorageError::Io(std::io::Error::new(
            std::io::ErrorKind::InvalidInput,
            "invalid backup file name",
        )));
    }

    let source = backups_dir(data_dir).join(file_name);
    if !source.is_file() {
        return Err(StorageError::Io(std::io::Error::new(
            std::io::ErrorKind::NotFound,
            "backup file not found",
        )));
    }

    std::fs::copy(&source, data_dir.join(RESTORE_STAGING_FILE))?;
    Ok(())
}

/// Apply a staged restore, if any, before the database is opened. Replaces the
/// database file and clears stale WAL sidecars so the restored copy is used.
pub(super) fn apply_staged_restore(database_path: &Path, data_dir: &Path) -> StorageResult<()> {
    let staged = data_dir.join(RESTORE_STAGING_FILE);
    if !staged.is_file() {
        return Ok(());
    }

    // Remove WAL/SHM sidecars of the current database so the restored file is authoritative.
    for sidecar in ["-wal", "-shm"] {
        let path = database_path.with_file_name(format!("{DATABASE_FILE}{sidecar}"));
        let _ = std::fs::remove_file(path);
    }

    std::fs::rename(&staged, database_path)?;
    Ok(())
}

/// Take a pre-migration snapshot when an existing database has pending
/// migrations. A brand-new database (no applied migrations) has nothing to
/// restore, so it is skipped. Returns the snapshot path when one was written.
///
/// A failure here propagates so the caller can block migration with a clear
/// error rather than upgrading the schema without a restorable copy.
pub(super) fn snapshot_before_migrations(
    connection: &Connection,
    data_dir: &Path,
) -> StorageResult<Option<PathBuf>> {
    let applied = migrations::count_applied_migrations(connection)?;
    let expected = migrations::migration_count();

    if applied == 0 || applied >= expected {
        return Ok(None);
    }

    let backups_dir = data_dir.join("backups");
    std::fs::create_dir_all(&backups_dir)?;

    let timestamp = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|elapsed| elapsed.as_secs())
        .unwrap_or(0);
    let dest = backups_dir.join(format!("pre-migration-v{applied}-{timestamp}.sqlite3"));

    vacuum_into(connection, &dest)?;
    Ok(Some(dest))
}
