//! Commit-receipt persistence for [`super::KpiIngestStagingStore`].
//! `NewCommitReceipt`/`CommitReceipt` and the two connection-level
//! primitives are re-exported from the module root — external callers use
//! `storage::kpi_ingest_staging::…` paths, never this submodule directly.

use super::identity::{generate_receipt_id, map_receipt_row};
use super::*;

/// A new immutable commit receipt (ADR 0098 dec. 5).
#[derive(Debug, Clone)]
pub struct NewCommitReceipt {
    pub run_id: String,
    pub manifest_hash: String,
    pub manifest_revision: i64,
    /// `complete` | `partial`.
    pub terminal_status: String,
    pub period_id: Option<String>,
    pub accepted_count: i64,
    pub outcomes_schema_version: i64,
    /// Versioned array of `{observationId, revision, ordinal, metricKey,
    /// factId, outcome, detail?}` — NOT a reuse of
    /// `jobs::record_financial_facts::FactOutcome` (M5 sol review: that type
    /// carries no `factId`).
    pub outcomes_json: String,
}

/// A stored commit receipt (the full read model).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CommitReceipt {
    pub id: String,
    pub run_id: String,
    pub manifest_hash: String,
    pub manifest_revision: i64,
    pub terminal_status: String,
    pub period_id: Option<String>,
    pub accepted_count: i64,
    pub outcomes_schema_version: i64,
    pub outcomes_json: String,
    pub committed_at: String,
}

const RECEIPT_COLUMNS: &str = "id, run_id, manifest_hash, manifest_revision, terminal_status, \
     period_id, accepted_count, outcomes_schema_version, outcomes_json, committed_at";

/// Connection-level variant of [`KpiIngestStagingStore::get_commit_receipt`]
/// (#360 B3 sol): `finalize_committing` and `reclaim_ingest_runs_on_startup`
/// (`storage/kpi_ingest_runs.rs`) call this under their OWN externally-owned
/// transaction — the public method above checks out its own connection and
/// would deadlock under an outer `Immediate` write lock.
pub(in crate::storage) fn get_commit_receipt_on_connection(
    connection: &Connection,
    run_id: &str,
) -> StorageResult<Option<CommitReceipt>> {
    connection
        .query_row(
            &format!("SELECT {RECEIPT_COLUMNS} FROM kpi_ingest_commit_receipts WHERE run_id = ?1"),
            [run_id],
            map_receipt_row,
        )
        .optional()
        .map_err(StorageError::from)
}

/// Connection-level free fn (the `record_structured_fact` pattern,
/// `kpi_extraction.rs:359`): `KpiIngestCommitStore::commit_manifest` (#362)
/// calls this under its own externally-owned `&Connection`/transaction — this
/// fn never opens one itself. A second insert for the same `run_id` maps the
/// `UNIQUE(run_id)` violation to the typed `CommitReceiptAlreadyRecorded`
/// (ADR 0098 dec. 5: idempotent replay must return the stored receipt, never
/// re-execute the commit primitives — that is #363's job once it sees this
/// error/the existing row; #362 itself never replays — a normal re-call
/// against a terminal run dies on `InvalidRunTransition` inside
/// `begin_committing`, before this insert is ever reached). INTEGRATION GATE
/// (luna review, closed by #362): this primitive inserts caller-supplied
/// values without reading the run — `commit_manifest` verifies, inside the
/// SAME outer transaction (`begin_committing` transitions the row to
/// `committing` first), that manifest_hash/manifest_revision match the run
/// row before this call is ever reached.
pub(in crate::storage) fn record_commit_receipt(
    connection: &Connection,
    receipt: NewCommitReceipt,
) -> StorageResult<CommitReceipt> {
    let id = generate_receipt_id(&receipt.run_id);
    let result = connection.execute(
        "INSERT INTO kpi_ingest_commit_receipts
            (id, run_id, manifest_hash, manifest_revision, terminal_status, period_id,
             accepted_count, outcomes_schema_version, outcomes_json)
         VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9)",
        params![
            id,
            receipt.run_id,
            receipt.manifest_hash,
            receipt.manifest_revision,
            receipt.terminal_status,
            receipt.period_id,
            receipt.accepted_count,
            receipt.outcomes_schema_version,
            receipt.outcomes_json,
        ],
    );
    match result {
        Ok(_) => {}
        // ONLY the run_id uniqueness gate is "already recorded"; every other
        // constraint (CHECK vocab, FK, NOT NULL) is a distinct storage error —
        // mapping them all to the idempotency conflict would misreport a
        // malformed receipt as a replay (luna review P1).
        Err(rusqlite::Error::SqliteFailure(err, Some(message)))
            if err.code == rusqlite::ErrorCode::ConstraintViolation
                && message.contains("kpi_ingest_commit_receipts.run_id") =>
        {
            return Err(StorageError::CommitReceiptAlreadyRecorded {
                run: receipt.run_id,
            });
        }
        Err(other) => return Err(other.into()),
    }
    connection
        .query_row(
            &format!("SELECT {RECEIPT_COLUMNS} FROM kpi_ingest_commit_receipts WHERE id = ?1"),
            [&id],
            map_receipt_row,
        )
        .map_err(StorageError::from)
}
