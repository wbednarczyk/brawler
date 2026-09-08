//! `AppState` facade for backups and the staged-restore notice (#319) — split
//! out of `storage/mod.rs` to keep that file under its size pin (ADR 0103).

use super::{backup, AppState, BackupStatus, StorageResult};

impl AppState {
    /// Record a restore that was recoverably refused during `open_pool`
    /// startup (#319) — the live database is untouched. Reached solely from
    /// `storage::open_pool`, right before it returns the freshly-built state
    /// to `lib.rs`, which reads it back once via
    /// [`Self::pending_restore_notice`] to raise the Today attention event.
    pub(super) fn with_pending_restore_notice(mut self, reason: Option<String>) -> Self {
        self.pending_restore_notice = reason;
        self
    }

    /// The reason a staged restore was refused at THIS startup, if any
    /// (#319). `None` in the overwhelming common case.
    pub fn pending_restore_notice(&self) -> Option<&str> {
        self.pending_restore_notice.as_deref()
    }

    pub fn backup_status(&self) -> StorageResult<BackupStatus> {
        backup::collect_status(&self.data_dir)
    }

    pub fn create_backup(&self) -> StorageResult<BackupStatus> {
        let connection = self.checkout()?;

        backup::create_rotating_backup(&connection, &self.data_dir)
    }

    pub fn request_restore(&self, file_name: &str) -> StorageResult<()> {
        backup::request_restore(&self.data_dir, file_name)
    }
}
