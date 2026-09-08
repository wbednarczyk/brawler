//! Fetch-status transition writers for [`super::ReportDocumentStore`] —
//! internal to the `report_documents` module tree (`pub(super)` only).
//! Split out of `report_documents.rs` for file size (file-size ratchet);
//! behavior unchanged.

use super::*;

/// Report-bytes protection contract (data-model.md § Report Document Model
/// retention; ADR 0098 dec. 3, card #359 — the first implementation of this
/// doc-only contract). FIVE semantic legs, SIX physical `EXISTS` (b splits
/// into notebook evidence and management-claim evidence, columns confirmed
/// against their migrations):
///
/// (a) a CONFIRMED financial fact citing the document (`financial_facts`,
///     migration 0034); (b1) research evidence (`notebook_entry_origins`,
///     migration 0001/0003 rename); (b2) a management-claim's evidence
///     (`management_claims`, migration 0045); (c) a CONFIRMED signal derived
///     from the document, joined via `report_documents.origin_ref =
///     company_signals.feed_item_id` (migrations 0035/0041); (d) any durable
///     `kpi_ingest_runs` row referencing the document, in ANY state
///     (migration 0137, ADR 0098 dec. 3 — a run's citations must stay
///     verifiable even if the run failed/was cancelled); (e) a
///     `report_tagged_fact_extractions` row for the document, in ANY state
///     (migration 0142, ADR 0100 dec. 8 — Layer 1 is derived data rebuilt
///     from the document's bytes, so a pruned document can never be
///     rebuilt; mirrors leg (d)'s "any state" rule for the same reason).
pub(super) const PROTECTION_EXISTS_CLAUSES: &str = "
    EXISTS (SELECT 1 FROM financial_facts
            WHERE source_document_ref = ?1 AND confirmation_state = 'confirmed')
    OR EXISTS (SELECT 1 FROM notebook_entry_origins
               WHERE source_type = 'report_document' AND source_id = ?1)
    OR EXISTS (SELECT 1 FROM management_claims
               WHERE source_evidence_type = 'report_document' AND source_evidence_id = ?1)
    OR EXISTS (SELECT 1 FROM company_signals cs
               JOIN report_documents rd ON rd.origin_ref = cs.feed_item_id
               WHERE rd.id = ?1 AND cs.status = 'confirmed')
    OR EXISTS (SELECT 1 FROM kpi_ingest_runs WHERE report_document_id = ?1)
    OR EXISTS (SELECT 1 FROM report_tagged_fact_extractions WHERE report_document_id = ?1)
";

/// Downgrade a document to `metadata_only`: keep the URL/title/attribution row for
/// citation and the source ladder, but record that no file is stored locally. Used by
/// retention pruning and for non-periodic ESPI/EBI attachments (ADR 0036).
///
/// **Atomic guard** (#359, B3 sol review): a single `UPDATE … WHERE id = ?1
/// AND NOT (<protection>)` — never a read-then-write, so no window exists
/// where a fact/claim/signal/run could attach between the check and the
/// write. Zero rows updated re-reads to classify: missing document (typed
/// not-found, existing behavior) vs protected document
/// (`ReportDocumentBytesProtected`).
pub(super) fn mark_metadata_only(
    connection: &Connection,
    id: &str,
) -> StorageResult<ReportDocument> {
    let updated = connection.execute(
        &format!(
            "
        UPDATE report_documents
        SET fetch_status = 'metadata_only',
            local_path = NULL,
            fetch_error = NULL,
            updated_at = strftime('%Y-%m-%dT%H:%M:%fZ', 'now')
        WHERE id = ?1
          AND NOT ({PROTECTION_EXISTS_CLAUSES})
        "
        ),
        params![id],
    )?;
    if updated == 1 {
        return get_report_document(connection, id);
    }

    // Zero rows: re-read to classify missing (typed not-found, unchanged
    // behavior) vs protected (the document exists but the guard refused).
    let _doc = get_report_document(connection, id)?;
    Err(StorageError::ReportDocumentBytesProtected { id: id.to_owned() })
}

/// Clears `fetch_error` too (#455): a stale error from a prior failed
/// attempt must not linger once the document is genuinely fetched — this is
/// also how the capture layer heals a `failed` row whose file still matches
/// its identity, by calling this with the row's own already-stored values.
pub(super) fn mark_fetched(
    connection: &Connection,
    id: &str,
    local_path: Option<&str>,
    content_type: Option<&str>,
    content_hash: Option<&str>,
    byte_size: Option<i64>,
) -> StorageResult<ReportDocument> {
    let local_path = empty_string_to_none(local_path.map(|s| s.to_owned()));
    let content_type = empty_string_to_none(content_type.map(|s| s.to_owned()));
    let content_hash = empty_string_to_none(content_hash.map(|s| s.to_owned()));

    let _doc = get_report_document(connection, id)?;

    connection.execute(
        "
        UPDATE report_documents
        SET local_path = ?2,
            content_type = ?3,
            content_hash = ?4,
            byte_size = ?5,
            fetch_status = ?6,
            fetch_error = NULL,
            fetched_at = strftime('%Y-%m-%dT%H:%M:%fZ', 'now'),
            updated_at = strftime('%Y-%m-%dT%H:%M:%fZ', 'now')
        WHERE id = ?1
        ",
        params![
            id,
            local_path,
            content_type,
            content_hash,
            byte_size,
            "fetched"
        ],
    )?;

    get_report_document(connection, id)
}

/// **Atomic guard** (#487 P1, adversarial review on the atomicity-wave PR):
/// publishes a fetched identity only when doing so would not overwrite a
/// DIFFERENT already-published identity — two concurrent captures of the
/// same document must never leave the row claiming bytes that are not the
/// ones actually on disk. A `fetched` row whose `content_hash` differs from
/// the one this call is about to write declines (0 rows updated); every
/// other state (`pending` / `metadata_only` / `failed`, or already `fetched`
/// with the SAME hash — an idempotent same-bytes republish) accepts. The
/// caller distinguishes "applied" from "declined" by comparing the returned
/// row's `content_hash` to the one it tried to publish, then re-reads to
/// learn the identity that won.
pub(super) fn mark_fetched_if_unchanged(
    connection: &Connection,
    id: &str,
    local_path: Option<&str>,
    content_type: Option<&str>,
    content_hash: &str,
    byte_size: i64,
) -> StorageResult<ReportDocument> {
    let local_path = empty_string_to_none(local_path.map(|s| s.to_owned()));
    let content_type = empty_string_to_none(content_type.map(|s| s.to_owned()));

    let _doc = get_report_document(connection, id)?;

    connection.execute(
        "
        UPDATE report_documents
        SET local_path = ?2,
            content_type = ?3,
            content_hash = ?4,
            byte_size = ?5,
            fetch_status = 'fetched',
            fetch_error = NULL,
            fetched_at = strftime('%Y-%m-%dT%H:%M:%fZ', 'now'),
            updated_at = strftime('%Y-%m-%dT%H:%M:%fZ', 'now')
        WHERE id = ?1
          AND (fetch_status <> 'fetched' OR content_hash IS NULL OR content_hash = ?4)
        ",
        params![id, local_path, content_type, content_hash, byte_size],
    )?;

    get_report_document(connection, id)
}

/// **Atomic guard** (#487 P1): heals a `failed` row back to `fetched` only
/// when it is STILL `failed` with the SAME `content_hash` this call
/// inspected (`report_documents_capture::resolve_before_fetch`'s heal
/// branch, verified via `local_file_matches_row` before this is called) —
/// otherwise the row moved between the inspection and this write (a
/// concurrent capture already healed or republished it over the same
/// document), and applying the stale heal anyway would erase that
/// concurrent outcome. Declines (0 rows) leave the row untouched; the
/// caller re-reads to see whichever state won.
pub(super) fn heal_failed_if_unchanged(
    connection: &Connection,
    id: &str,
    local_path: Option<&str>,
    content_type: Option<&str>,
    content_hash: Option<&str>,
    byte_size: Option<i64>,
) -> StorageResult<ReportDocument> {
    let local_path = empty_string_to_none(local_path.map(|s| s.to_owned()));
    let content_type = empty_string_to_none(content_type.map(|s| s.to_owned()));
    let content_hash = empty_string_to_none(content_hash.map(|s| s.to_owned()));

    connection.execute(
        "
        UPDATE report_documents
        SET local_path = ?2,
            content_type = ?3,
            content_hash = ?4,
            byte_size = ?5,
            fetch_status = 'fetched',
            fetch_error = NULL,
            fetched_at = strftime('%Y-%m-%dT%H:%M:%fZ', 'now'),
            updated_at = strftime('%Y-%m-%dT%H:%M:%fZ', 'now')
        WHERE id = ?1
          AND fetch_status = 'failed'
          AND content_hash IS ?4
        ",
        params![id, local_path, content_type, content_hash, byte_size],
    )?;

    get_report_document(connection, id)
}

/// **Atomic guard** (#455, mirroring [`mark_metadata_only`]'s idiom): a
/// `fetched` document with a stored file is never downgraded to `failed` —
/// a race where a concurrent capture stored the file between this caller's
/// fetch attempt and its failure write must not strand the row `failed`
/// while bytes sit on disk. Zero-row (guard declined, or unknown id) always
/// re-reads and returns the current row rather than erroring: the caller
/// distinguishes "declined" from "applied" by checking the returned
/// `fetch_status` (still `fetched` vs now `failed`).
pub(super) fn mark_failed(
    connection: &Connection,
    id: &str,
    error: &str,
) -> StorageResult<ReportDocument> {
    let error = error.trim().to_owned();
    let _doc = get_report_document(connection, id)?;

    connection.execute(
        "
        UPDATE report_documents
        SET fetch_status = ?2,
            fetch_error = ?3,
            updated_at = strftime('%Y-%m-%dT%H:%M:%fZ', 'now')
        WHERE id = ?1
          AND NOT (fetch_status = 'fetched' AND local_path IS NOT NULL AND local_path <> '')
        ",
        params![id, "failed", error],
    )?;

    get_report_document(connection, id)
}

/// Reset a `fetched` document back to `pending` when the identity check
/// (#455, `report_documents_capture::local_file_matches_row`) finds the file
/// missing or no longer matching its stored hash/size — the row must not
/// keep claiming bytes that are not really there. `local_path` is cleared so
/// the row cannot be misread as fetched again before the refetch lands.
///
/// **Compare-and-set on the inspected identity** (#487 P1): the caller
/// passes the EXACT `fetch_status` / `local_path` / `content_hash` it
/// inspected before deciding to reset. If the row moved since — a
/// concurrent capture published a fresh, valid identity for this document
/// between the inspection and this write — the reset is refused rather than
/// wiping out that concurrent success. Returns `(row, applied)`: `applied`
/// is false when the guard declined, in which case `row` is whatever is
/// current (possibly the concurrent winner) and the caller should re-inspect
/// it rather than treat the reset as having happened.
pub(super) fn mark_pending_for_refetch(
    connection: &Connection,
    id: &str,
    expected_status: &str,
    expected_local_path: Option<&str>,
    expected_content_hash: Option<&str>,
) -> StorageResult<(ReportDocument, bool)> {
    let applied = connection.execute(
        "
        UPDATE report_documents
        SET fetch_status = 'pending',
            local_path = NULL,
            updated_at = strftime('%Y-%m-%dT%H:%M:%fZ', 'now')
        WHERE id = ?1
          AND fetch_status = ?2
          AND local_path IS ?3
          AND content_hash IS ?4
        ",
        params![
            id,
            expected_status,
            expected_local_path,
            expected_content_hash
        ],
    )? == 1;

    Ok((get_report_document(connection, id)?, applied))
}
