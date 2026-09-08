use super::*;
use std::io::Write;
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
const PRE_RESTORE_PREFIX: &str = "pre-restore-";
const RESTORE_STAGING_FILE: &str = "restore-pending.sqlite3";
/// A staged/restore candidate that failed verification, kept for inspection
/// (#319) — never read by the app itself, overwritten by the next rejection.
const RESTORE_REJECTED_FILE: &str = "restore-rejected.sqlite3";
/// Marks an in-progress swap (`apply_staged_restore`'s sidecar-removal +
/// rename step) so a crash between those two file operations is resumable on
/// the next start (#319) — see that fn's doc for the full protocol.
const RESTORE_JOURNAL_FILE: &str = "restore-journal.json";
const DATABASE_FILE: &str = "brawler.sqlite3";

/// The crash-resumable journal `apply_staged_restore` writes right before the
/// two file operations (sidecar removal + rename) a mid-swap crash could
/// leave half-done. `pre_restore` is the absolute path to the ordinary backup
/// the original database was copied to just before the swap.
#[derive(Debug, Serialize, Deserialize)]
struct RestoreJournal {
    pre_restore: String,
    started_at: String,
}

#[derive(Debug, Serialize)]
#[cfg_attr(feature = "ts-export", derive(ts_rs::TS))]
#[cfg_attr(
    feature = "ts-export",
    ts(export, export_to = "../../src/api/generated/")
)]
#[serde(rename_all = "camelCase")]
pub struct BackupEntry {
    pub file_name: String,
    pub created_at: Option<String>,
    #[cfg_attr(feature = "ts-export", ts(type = "\"rotating\" | \"snapshot\""))]
    pub kind: String,
    pub size_bytes: u64,
}

#[derive(Debug, Serialize)]
#[cfg_attr(feature = "ts-export", derive(ts_rs::TS))]
#[cfg_attr(
    feature = "ts-export",
    ts(export, export_to = "../../src/api/generated/")
)]
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

/// Verify a restore candidate is a real, intact Brawler database this build
/// recognizes, before it is ever allowed to become the live database (#319).
/// Read-only end to end ([`migrations::open_database_readonly`]): verification
/// never mutates the candidate. Used both when a restore is staged
/// ([`request_restore`]) and again right before it is applied
/// ([`apply_staged_restore`]) — the staged file sat in a user-visible folder
/// in between and could have been swapped out from under the app.
pub(super) fn verify_backup_file(path: &Path) -> StorageResult<()> {
    let reject = |reason: String| StorageError::RestoreRejected(reason);

    let connection = migrations::open_database_readonly(path)
        .map_err(|error| reject(format!("could not open the file as a database: {error}")))?;

    let integrity: String = connection
        .query_row("PRAGMA integrity_check", [], |row| row.get(0))
        .map_err(|error| reject(format!("integrity_check could not run: {error}")))?;
    if integrity != "ok" {
        return Err(reject(format!(
            "database failed integrity_check: {integrity}"
        )));
    }

    // A missing `schema_migrations` table reads as 0 applied migrations — the
    // same signal as a bare non-Brawler SQLite file, and refused the same way
    // (a marker table with zero rows would be indistinguishable otherwise).
    let applied = migrations::count_applied_migrations(&connection)
        .map_err(|error| reject(format!("could not read schema_migrations: {error}")))?;
    if applied == 0 {
        return Err(reject(
            "no applied migrations found — not a Brawler database".to_owned(),
        ));
    }

    // Every listed version must be one this build actually ships (set
    // membership, not a row count): migrations are numbered 1..=migration_count()
    // with no gaps or reuse (append-only, ADR-enforced), so a version outside
    // that range is unrecognized — most likely a newer app version's database.
    let known_versions = migrations::migration_count();
    let mut statement = connection
        .prepare("SELECT version FROM schema_migrations")
        .map_err(|error| reject(format!("could not read schema_migrations: {error}")))?;
    let versions = statement
        .query_map([], |row| row.get::<_, i64>(0))
        .map_err(|error| reject(format!("could not read schema_migrations: {error}")))?;
    for version in versions {
        let version = version
            .map_err(|error| reject(format!("could not read schema_migrations: {error}")))?;
        if !(1..=known_versions).contains(&version) {
            return Err(reject(format!(
                "schema_migrations lists version {version}, which this build does not \
                 recognize (likely a newer app version's database)"
            )));
        }
    }

    Ok(())
}

/// Stage a chosen backup for restore on the next launch. The file name must be a
/// plain backup file in the backups directory (no path traversal). The live
/// database is not touched; the swap happens at startup (ADR 0032).
///
/// The candidate is copied to a private temp name and verified there — never
/// the source backup file itself — so a failed verification leaves the
/// backups directory untouched and publishes no new staging file (#319). A
/// later successful request replaces an earlier pending one (the temp file is
/// renamed onto the well-known staging name, an atomic publish).
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

    let staging_temp = data_dir.join(format!("{RESTORE_STAGING_FILE}.verify-{}", unix_nanos()));
    std::fs::copy(&source, &staging_temp)?;

    if let Err(error) = verify_backup_file(&staging_temp) {
        let _ = std::fs::remove_file(&staging_temp);
        return Err(error);
    }

    std::fs::rename(&staging_temp, data_dir.join(RESTORE_STAGING_FILE))?;
    Ok(())
}

/// Remove the current database's WAL/SHM sidecars — the swap below only ever
/// runs once they are checkpointed (empty) or about to be replaced outright.
fn clear_database_sidecars(database_path: &Path) {
    for sidecar in ["-wal", "-shm"] {
        let path = database_path.with_file_name(format!("{DATABASE_FILE}{sidecar}"));
        let _ = std::fs::remove_file(path);
    }
}

fn database_passes_integrity_check(database_path: &Path) -> bool {
    migrations::open_database_readonly(database_path)
        .ok()
        .and_then(|connection| {
            connection
                .query_row("PRAGMA integrity_check", [], |row| row.get::<_, String>(0))
                .ok()
        })
        .map(|integrity| integrity == "ok")
        .unwrap_or(false)
}

/// Recover from a crash mid-swap on the previous run (step 8 of
/// [`apply_staged_restore`]'s protocol): a journal on disk means the
/// sidecar-removal + rename step below was interrupted. If the live database
/// still opens and passes `integrity_check`, the swap actually completed —
/// just the journal cleanup didn't — so the journal is simply deleted.
/// Otherwise the pre-restore copy the journal names is copied back over the
/// live path. A failure copying it back is unrecoverable (`StorageError::Io`,
/// propagated so `open_pool` aborts setup as it always has for a corrupt
/// database) — there is no safe database to open at that point.
///
/// The journal itself is published atomically ([`write_journal_atomically`]),
/// but a crash could still predate this fix, or a filesystem could lie about
/// a completed write — so a journal that fails to parse is handled
/// explicitly rather than aborting startup forever (#319 P1): if the live
/// database is intact, the malformed journal is simply dropped (the swap
/// completed; only the cleanup didn't). Otherwise the newest
/// `pre-restore-*.sqlite3` in `backups/` is used as the recovery source
/// instead of the journal's own (unreadable) pointer — fatal only when no
/// such backup exists either.
fn recover_from_crash_journal(
    database_path: &Path,
    data_dir: &Path,
    journal_path: &Path,
) -> StorageResult<()> {
    if !journal_path.is_file() {
        return Ok(());
    }

    let parsed: Option<RestoreJournal> = std::fs::read_to_string(journal_path)
        .ok()
        .and_then(|contents| serde_json::from_str(&contents).ok());

    if parsed.is_none() {
        log::warn!(
            "restore journal at {} is malformed (likely a truncated write mid-crash) — \
             recovering without it",
            journal_path.display()
        );
    }

    if !database_passes_integrity_check(database_path) {
        let pre_restore_source = match &parsed {
            Some(journal) => Some(PathBuf::from(&journal.pre_restore)),
            None => newest_pre_restore_backup(data_dir),
        };

        match pre_restore_source {
            Some(source) => {
                std::fs::copy(&source, database_path)?;
                clear_database_sidecars(database_path);
            }
            None => {
                return Err(StorageError::Io(std::io::Error::other(
                    "crash recovery: restore journal is unreadable and no pre-restore backup \
                     exists to recover the database from",
                )));
            }
        }
    }

    std::fs::remove_file(journal_path)?;
    Ok(())
}

/// The most recently modified `pre-restore-*.sqlite3` in `backups/` — the
/// fallback recovery source when the crash journal itself is unreadable
/// (#319 P1), since the journal's own `pre_restore` pointer can't be trusted
/// if the journal didn't parse.
fn newest_pre_restore_backup(data_dir: &Path) -> Option<PathBuf> {
    let dir = backups_dir(data_dir);
    std::fs::read_dir(dir)
        .ok()?
        .filter_map(|entry| entry.ok())
        .map(|entry| entry.path())
        .filter(|path| {
            path.file_name()
                .and_then(|name| name.to_str())
                .map(|name| name.starts_with(PRE_RESTORE_PREFIX))
                .unwrap_or(false)
        })
        .filter_map(|path| {
            let modified = std::fs::metadata(&path).ok()?.modified().ok()?;
            Some((path, modified))
        })
        .max_by_key(|(_, modified)| *modified)
        .map(|(path, _)| path)
}

/// Publish the crash-recovery journal atomically: write to a private,
/// uniquely-named temp file, `sync_all` it, then rename onto the well-known
/// journal path. A crash mid-write leaves the temp file half-written, never
/// the journal itself, so [`recover_from_crash_journal`] never has to parse
/// truncated JSON (#319 P1).
fn write_journal_atomically(journal_path: &Path, journal: &RestoreJournal) -> StorageResult<()> {
    let temp_path =
        journal_path.with_file_name(format!("{RESTORE_JOURNAL_FILE}.tmp-{}", unix_nanos()));
    let mut file = std::fs::File::create(&temp_path)?;
    file.write_all(serde_json::to_string(journal)?.as_bytes())?;
    file.sync_all()?;
    drop(file);
    std::fs::rename(&temp_path, journal_path)?;
    Ok(())
}

/// Reject a staged restore candidate recoverably: keep it as
/// `restore-rejected.sqlite3` for inspection and report `reason`. Used both
/// for a failed re-verification and for any pre-swap failure (#319 P2) —
/// nothing has touched the live database yet in either case.
fn reject_staged_candidate(staged: &Path, data_dir: &Path, reason: String) -> Option<String> {
    let _ = std::fs::rename(staged, data_dir.join(RESTORE_REJECTED_FILE));
    Some(reason)
}

/// The human-readable rejection reason from a `StorageError` produced during
/// restore verification or the swap itself.
fn reject_reason(error: StorageError) -> String {
    match error {
        StorageError::RestoreRejected(reason) => reason,
        other => other.to_string(),
    }
}

/// Apply a staged restore, if any, before the database is opened
/// ([`open_pool`](super::pool::open_pool) calls this first thing). The
/// original database is never renamed away — it is checkpointed and copied
/// aside as an ordinary backup, and a journal makes a crash between the
/// sidecar cleanup and the rename resumable on the next start (#319):
///
/// 1. Recover from a previous crash first ([`recover_from_crash_journal`]).
/// 2. No staged file and no journal → nothing to do.
/// 3. Verify the staged file again (it sat in a user-visible folder since
///    [`request_restore`] accepted it) → reject on failure, original
///    untouched, the bad candidate kept as `restore-rejected.sqlite3`.
/// 4. Checkpoint the original (`PRAGMA wal_checkpoint(TRUNCATE)`) so its
///    `.sqlite3` file alone is a complete, consistent copy.
/// 5. Copy the checkpointed original to `backups/pre-restore-<ts>.sqlite3` —
///    an ordinary rotating-adjacent backup, restorable like any other.
/// 6. Write the journal, remove the (now empty) sidecars, rename the staged
///    file onto the live path, open it and re-run `integrity_check`.
/// 7. Success → delete the journal. Failure → copy the pre-restore backup
///    back over the live path, clear sidecars, delete the journal, keep the
///    bad candidate as `restore-rejected.sqlite3`.
///
/// Everything through the journal write (checkpoint, `backups/` creation,
/// pre-restore copy) touches nothing the live database depends on — a
/// failure at any of those steps is a recoverable rejection (`Ok(Some(_))`,
/// #319 P2), not fatal. Only the sidecar-clear + rename after it is
/// irreversible.
///
/// Returns `Ok(Some(reason))` when a staged restore was recoverably refused —
/// the live database is untouched and still authoritative; never fatal,
/// `open_pool` continues normally and surfaces `reason` to the user via a
/// Today attention event. Returns `Ok(None)` when there was nothing to do or
/// the swap succeeded. Returns `Err` only for the unrecoverable case: crash
/// recovery itself could not restore a safe database (setup aborts, as
/// before this fix).
pub(super) fn apply_staged_restore(
    database_path: &Path,
    data_dir: &Path,
) -> StorageResult<Option<String>> {
    apply_staged_restore_with(database_path, data_dir, |_staged| {})
}

/// [`apply_staged_restore`] with a test-only seam: `after_verify` runs right
/// after the staged file passes its second (re-)verification and before the
/// swap begins — the only way to exercise the post-verify rollback path,
/// since verification alone would otherwise always catch a bad candidate
/// first. Production always passes a no-op.
pub(super) fn apply_staged_restore_with(
    database_path: &Path,
    data_dir: &Path,
    after_verify: impl FnOnce(&Path),
) -> StorageResult<Option<String>> {
    let journal_path = data_dir.join(RESTORE_JOURNAL_FILE);
    recover_from_crash_journal(database_path, data_dir, &journal_path)?;

    let staged = data_dir.join(RESTORE_STAGING_FILE);
    if !staged.is_file() {
        return Ok(None);
    }

    if let Err(error) = verify_backup_file(&staged) {
        return Ok(reject_staged_candidate(
            &staged,
            data_dir,
            reject_reason(error),
        ));
    }

    after_verify(&staged);

    let checkpoint_result: StorageResult<()> = (|| {
        // Scoped so the connection (and its own pooled resources) closes
        // before the file is copied below. Function-call PRAGMA syntax
        // (unlike `PRAGMA x = value`, which `pragma_update` builds) is what
        // `wal_checkpoint` actually expects a mode argument through.
        let connection = Connection::open(database_path)?;
        connection.execute_batch("PRAGMA wal_checkpoint(TRUNCATE);")?;
        Ok(())
    })();
    if let Err(error) = checkpoint_result {
        return Ok(reject_staged_candidate(
            &staged,
            data_dir,
            error.to_string(),
        ));
    }

    let backups_directory = backups_dir(data_dir);
    if let Err(error) = std::fs::create_dir_all(&backups_directory) {
        return Ok(reject_staged_candidate(
            &staged,
            data_dir,
            error.to_string(),
        ));
    }

    let pre_restore_path =
        backups_directory.join(format!("{PRE_RESTORE_PREFIX}{}.sqlite3", unix_nanos()));
    if let Err(error) = std::fs::copy(database_path, &pre_restore_path) {
        let _ = std::fs::remove_file(&pre_restore_path);
        return Ok(reject_staged_candidate(
            &staged,
            data_dir,
            error.to_string(),
        ));
    }

    let journal = RestoreJournal {
        pre_restore: pre_restore_path.to_string_lossy().into_owned(),
        started_at: OffsetDateTime::now_utc()
            .format(&Rfc3339)
            .unwrap_or_default(),
    };
    if let Err(error) = write_journal_atomically(&journal_path, &journal) {
        let _ = std::fs::remove_file(&pre_restore_path);
        return Ok(reject_staged_candidate(
            &staged,
            data_dir,
            error.to_string(),
        ));
    }

    clear_database_sidecars(database_path);

    // Tracked separately from `swap_result` so the rollback below knows WHERE
    // the bad candidate ended up: the rename either moved `staged` onto
    // `database_path` (then integrity_check may still have failed) or never
    // ran at all, leaving `staged` exactly where it was.
    let rename_result = std::fs::rename(&staged, database_path);
    let rename_succeeded = rename_result.is_ok();
    let swap_result: StorageResult<()> = match rename_result {
        Err(io_error) => Err(StorageError::RestoreRejected(format!(
            "could not move the staged file onto the database path: {io_error}"
        ))),
        Ok(()) if database_passes_integrity_check(database_path) => Ok(()),
        Ok(()) => Err(StorageError::RestoreRejected(
            "restored database failed integrity_check after the swap".to_owned(),
        )),
    };

    match swap_result {
        Ok(()) => {
            std::fs::remove_file(&journal_path)?;
            Ok(None)
        }
        Err(error) => {
            // Preserve whatever bad candidate exists for inspection before
            // restoring the original.
            if rename_succeeded {
                let _ = std::fs::copy(database_path, data_dir.join(RESTORE_REJECTED_FILE));
            } else if staged.is_file() {
                let _ = std::fs::rename(&staged, data_dir.join(RESTORE_REJECTED_FILE));
            }

            std::fs::copy(&pre_restore_path, database_path)?;
            clear_database_sidecars(database_path);
            std::fs::remove_file(&journal_path)?;

            Ok(Some(reject_reason(error)))
        }
    }
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
