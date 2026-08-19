//! Chunked staging drafts (ADR 0102 decisions 6-11; epic #399 S6, migration
//! 0151). `kpi_ingest_drafts` + `kpi_ingest_draft_chunks` are a sub-resource
//! of a run — never a new run state; `STAGEABLE_STATUSES` and the ADR 0098
//! dec. 6 lifecycle are untouched. A draft is bound to
//! `kpi_ingest_runs.attempt_count` at open time (its "lease epoch" — that
//! column already only increments on a genuine claim after an absent/expired
//! lease, never on a same-holder renewal, `kpi_ingest_runs::claim_run_on_connection`)
//! — a lease takeover (any subsequent claim, same or foreign holder) bumps
//! it, which the FIRST operation to notice (open/append/finalize/a status
//! read) treats as invalidation. Finalize shares
//! [`super::kpi_ingest_staging::install_observations_on_connection`] with the
//! single-call path (ADR 0102 dec. 9) — it does not call the public
//! `stage_observations` as a black box. Reach via `AppState::kpi_ingest_drafts()`.

use std::collections::BTreeMap;

use sha2::{Digest, Sha256};

use super::database::Database;
use super::kpi_ingest_runs::lease_refusal_on_connection;
use super::kpi_ingest_staging::{
    install_observations_on_connection, NewStagedObservation, StagedObservation, STAGEABLE_STATUSES,
};
use super::*;

/// Aggregate observation ceiling at finalize (ADR 0102 dec. 10): headroom
/// over #398's measured ~426 tagged facts per package.
const AGGREGATE_OBSERVATIONS_MAX: i64 = 1000;
/// Frozen aggregate byte cap (ADR 0102 dec. 10, contracts.md tool 5): the
/// per-chunk transport bound (100 observations, 1 MiB) is unraised by
/// chunking — `AGGREGATE_OBSERVATIONS_MAX` / 100-per-chunk = at most 10
/// chunks, times the documented per-call escaping bound (703 KiB =
/// 719,872 B): 10 * 719,872 = 7,198,720 B (≈ 6.87 MiB). Deliberately
/// conservative — it reuses the ALREADY-PROVEN per-call arithmetic rather
/// than deriving a tighter, un-proven number. Checked against the SUM of
/// stored `payload_json` byte lengths at finalize.
const AGGREGATE_BYTES_MAX: i64 = 10 * 719_872;

/// The read summary [`get_open_draft`]/`open_draft`/`append_chunk` share —
/// `get_kpi_ingest_status`'s `openDraft` field (ADR 0102 dec. 11).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct OpenDraftSummary {
    pub draft_id: String,
    pub expected_observations: i64,
    pub chunks_received: i64,
}

/// One chunk append's result.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct AppendChunkResult {
    pub draft_id: String,
    pub chunk_index: i64,
    pub chunks_received: i64,
}

/// Collision-safe, non-deterministic id (the `generate_observation_id` idiom,
/// `kpi_ingest_staging.rs`): `kpidft_` + 32 hex chars of sha256 over the
/// identity plus a nanosecond time component.
fn generate_draft_id(run_id: &str) -> String {
    let now_nanos = time::OffsetDateTime::now_utc().unix_timestamp_nanos();
    let key = format!("kpidft:{run_id}\u{1f}{now_nanos}");
    let digest = Sha256::digest(key.as_bytes());
    let mut hex = String::with_capacity(32);
    for byte in &digest[..16] {
        hex.push_str(&format!("{byte:02x}"));
    }
    format!("kpidft_{hex}")
}

/// The server-computed chunk identity hash (ADR 0102 dec. 8) — full 32-byte
/// sha256 hex over the canonical `payload_json` bytes; never trusted from the
/// client.
fn chunk_hash(payload_json: &str) -> String {
    let digest = Sha256::digest(payload_json.as_bytes());
    let mut hex = String::with_capacity(64);
    for byte in &digest[..] {
        hex.push_str(&format!("{byte:02x}"));
    }
    hex
}

/// Shared entry guard for open/append/finalize (mirrors
/// `install_observations_on_connection`'s own run pre-check): the run must be
/// in `{extracting, validation_failed}` and `holder` must hold the LIVE
/// lease. Returns the run's current `attempt_count` — the epoch a fresh open
/// binds to and every later call re-verifies against.
fn check_stageable_and_lease(tx: &Connection, run_id: &str, holder: &str) -> StorageResult<i64> {
    let row: Option<(String, Option<String>, Option<String>, i64)> = tx
        .query_row(
            "SELECT status, lease_holder, lease_expires_at, attempt_count \
             FROM kpi_ingest_runs WHERE id = ?1",
            [run_id],
            |row| Ok((row.get(0)?, row.get(1)?, row.get(2)?, row.get(3)?)),
        )
        .optional()?;
    let Some((status, lease_holder, lease_expires_at, attempt_count)) = row else {
        return Err(StorageError::KpiIngestRunNotFound {
            id: run_id.to_owned(),
        });
    };
    let parsed_status = KpiIngestRunState::parse(&status)?;
    if !STAGEABLE_STATUSES.contains(&parsed_status) {
        return Err(StorageError::InvalidRunStateForStaging {
            id: run_id.to_owned(),
            status: parsed_status.as_str().to_owned(),
        });
    }
    let now: String = tx.query_row("SELECT strftime('%Y-%m-%dT%H:%M:%fZ', 'now')", [], |row| {
        row.get(0)
    })?;
    let lease_live = lease_holder.as_deref() == Some(holder)
        && lease_expires_at
            .as_deref()
            .is_some_and(|expires| expires > now.as_str());
    if !lease_live {
        return Err(lease_refusal_on_connection(tx, run_id, holder)?);
    }
    Ok(attempt_count)
}

/// Refuses a second open / a single-call snapshot while an active draft is
/// live (ADR 0102 dec. 11): a STALE active draft (lease epoch behind
/// `current_epoch` — a takeover happened since it opened) is lazily
/// superseded instead of blocking; a LIVE-epoch active draft is a typed
/// conflict — `stage_observations` and [`open_draft`] share this exact check.
pub(super) fn resolve_active_draft_conflict_on_connection(
    tx: &Connection,
    run_id: &str,
    current_epoch: i64,
) -> StorageResult<()> {
    let active: Option<(String, i64)> = tx
        .query_row(
            "SELECT draft_id, lease_epoch FROM kpi_ingest_drafts \
             WHERE run_id = ?1 AND status = 'active'",
            [run_id],
            |row| Ok((row.get(0)?, row.get(1)?)),
        )
        .optional()?;
    let Some((draft_id, lease_epoch)) = active else {
        return Ok(());
    };
    if lease_epoch == current_epoch {
        return Err(StorageError::KpiIngestActiveDraftExists {
            run_id: run_id.to_owned(),
            draft_id,
        });
    }
    supersede(tx, &draft_id)?;
    Ok(())
}

/// Lazy takeover detection writes this from two branches — one statement,
/// one place.
fn supersede(tx: &Connection, draft_id: &str) -> StorageResult<()> {
    tx.execute(
        "UPDATE kpi_ingest_drafts SET status = 'superseded' WHERE draft_id = ?1",
        [draft_id],
    )?;
    Ok(())
}

/// Loads a NAMED draft for append/finalize: must belong to `run_id`, be
/// `active`, and sit at `current_epoch` — otherwise lazily superseded (a
/// takeover) or reported not-found (a foreign/unknown draft id, never
/// leaking cross-run existence). Returns the draft's declared
/// `expected_observations`.
fn load_live_draft_or_supersede(
    tx: &Connection,
    run_id: &str,
    draft_id: &str,
    current_epoch: i64,
) -> StorageResult<i64> {
    let row: Option<(String, i64, String, i64)> = tx
        .query_row(
            "SELECT run_id, lease_epoch, status, expected_observations \
             FROM kpi_ingest_drafts WHERE draft_id = ?1",
            [draft_id],
            |row| Ok((row.get(0)?, row.get(1)?, row.get(2)?, row.get(3)?)),
        )
        .optional()?;
    let Some((draft_run_id, lease_epoch, status, expected_observations)) = row else {
        return Err(StorageError::KpiIngestDraftNotFound {
            draft_id: draft_id.to_owned(),
        });
    };
    if draft_run_id != run_id {
        return Err(StorageError::KpiIngestDraftNotFound {
            draft_id: draft_id.to_owned(),
        });
    }
    if status != "active" {
        return Err(StorageError::KpiIngestDraftSuperseded {
            draft_id: draft_id.to_owned(),
        });
    }
    if lease_epoch != current_epoch {
        supersede(tx, draft_id)?;
        return Err(StorageError::KpiIngestDraftSuperseded {
            draft_id: draft_id.to_owned(),
        });
    }
    Ok(expected_observations)
}

#[derive(Clone)]
pub struct KpiIngestDraftsStore {
    db: Database,
}

impl KpiIngestDraftsStore {
    pub(super) fn new(db: Database) -> Self {
        Self { db }
    }

    /// `draft:{open:true, expectedObservations}` (ADR 0102 dec. 6): mints a
    /// server-issued `draftId` bound to the run's current lease epoch. Exactly
    /// one active draft per run (the partial-unique index, and
    /// [`resolve_active_draft_conflict_on_connection`] ahead of the INSERT for
    /// a typed refusal instead of a raw constraint failure).
    pub fn open_draft(
        &self,
        run_id: &str,
        holder: &str,
        expected_observations: i64,
    ) -> StorageResult<OpenDraftSummary> {
        if expected_observations < 1 {
            return Err(StorageError::InvalidKpiIngestRunValue {
                key: "expectedObservations",
                value: expected_observations.to_string(),
            });
        }
        let mut connection = self.db.checkout()?;
        let tx = connection.transaction_with_behavior(rusqlite::TransactionBehavior::Immediate)?;
        let epoch = check_stageable_and_lease(&tx, run_id, holder)?;
        resolve_active_draft_conflict_on_connection(&tx, run_id, epoch)?;
        let draft_id = generate_draft_id(run_id);
        tx.execute(
            "INSERT INTO kpi_ingest_drafts \
                (draft_id, run_id, lease_epoch, expected_observations, status) \
             VALUES (?1, ?2, ?3, ?4, 'active')",
            params![draft_id, run_id, epoch, expected_observations],
        )?;
        tx.commit()?;
        Ok(OpenDraftSummary {
            draft_id,
            expected_observations,
            chunks_received: 0,
        })
    }

    /// `draft:{draftId, chunkIndex}` (ADR 0102 dec. 7/8): re-checks the live
    /// lease on EVERY call (not just open/finalize), never bumps the run's
    /// manifest revision, and is structurally invisible to validation (its
    /// own table, merged only at finalize). Replaying the same
    /// `(draftId, chunkIndex)` with matching server-computed content is an
    /// idempotent ack; different content at the same index is a typed
    /// `KpiIngestDraftChunkConflict`.
    pub fn append_chunk(
        &self,
        run_id: &str,
        holder: &str,
        draft_id: &str,
        chunk_index: i64,
        observations: Vec<NewStagedObservation>,
    ) -> StorageResult<AppendChunkResult> {
        if observations.is_empty() {
            return Err(StorageError::InvalidKpiIngestRunValue {
                key: "observations",
                value: "empty".to_owned(),
            });
        }
        if chunk_index < 0 {
            return Err(StorageError::InvalidKpiIngestRunValue {
                key: "chunkIndex",
                value: chunk_index.to_string(),
            });
        }
        let mut connection = self.db.checkout()?;
        let tx = connection.transaction_with_behavior(rusqlite::TransactionBehavior::Immediate)?;
        let epoch = check_stageable_and_lease(&tx, run_id, holder)?;
        load_live_draft_or_supersede(&tx, run_id, draft_id, epoch)?;

        let payload_json = serde_json::to_string(&observations)?;
        let hash = chunk_hash(&payload_json);
        let observation_count = observations.len() as i64;

        let existing_hash: Option<String> = tx
            .query_row(
                "SELECT chunk_hash FROM kpi_ingest_draft_chunks \
                 WHERE draft_id = ?1 AND chunk_index = ?2",
                params![draft_id, chunk_index],
                |row| row.get(0),
            )
            .optional()?;
        match existing_hash {
            Some(existing) if existing == hash => {
                // Idempotent replay ack (ADR 0102 dec. 8) — no duplicate insert.
            }
            Some(_) => {
                return Err(StorageError::KpiIngestDraftChunkConflict {
                    draft_id: draft_id.to_owned(),
                    chunk_index,
                });
            }
            None => {
                tx.execute(
                    "INSERT INTO kpi_ingest_draft_chunks \
                        (draft_id, chunk_index, chunk_hash, payload_json, observation_count) \
                     VALUES (?1, ?2, ?3, ?4, ?5)",
                    params![draft_id, chunk_index, hash, payload_json, observation_count],
                )?;
            }
        }
        let chunks_received: i64 = tx.query_row(
            "SELECT COUNT(*) FROM kpi_ingest_draft_chunks WHERE draft_id = ?1",
            [draft_id],
            |row| row.get(0),
        )?;
        tx.commit()?;
        Ok(AppendChunkResult {
            draft_id: draft_id.to_owned(),
            chunk_index,
            chunks_received,
        })
    }

    /// `draft:{draftId, final:true}` + `missingReasons` (ADR 0102 dec. 9):
    /// checks chunk contiguity (a gap or zero chunks → `KpiIngestDraftIncomplete`),
    /// the aggregate caps (dec. 10 → `KpiIngestDraftAggregateBudgetExceeded`),
    /// re-checks the live lease, assembles the flattened observation list
    /// ordered `(chunkIndex, position-within-chunk)`, then shares
    /// [`install_observations_on_connection`] with the single-call path —
    /// bump-revision + install + flip-state + delete-drafts in ONE
    /// transaction. Global ordinals are store-assigned by that shared helper,
    /// identically to today's single-call behavior.
    pub fn finalize_draft(
        &self,
        run_id: &str,
        holder: &str,
        draft_id: &str,
        missing_reasons: &BTreeMap<String, String>,
        execution: Option<&serde_json::Value>,
    ) -> StorageResult<(i64, Vec<StagedObservation>)> {
        let mut connection = self.db.checkout()?;
        let tx = connection.transaction_with_behavior(rusqlite::TransactionBehavior::Immediate)?;
        let epoch = check_stageable_and_lease(&tx, run_id, holder)?;
        let expected_observations = load_live_draft_or_supersede(&tx, run_id, draft_id, epoch)?;

        let mut statement = tx.prepare(
            "SELECT chunk_index, payload_json, observation_count \
             FROM kpi_ingest_draft_chunks WHERE draft_id = ?1 ORDER BY chunk_index",
        )?;
        let chunks: Vec<(i64, String, i64)> = statement
            .query_map([draft_id], |row| {
                Ok((row.get(0)?, row.get(1)?, row.get(2)?))
            })?
            .collect::<Result<_, _>>()?;
        drop(statement);

        if chunks.is_empty() {
            return Err(StorageError::KpiIngestDraftIncomplete {
                draft_id: draft_id.to_owned(),
                reason: "no chunks were appended".to_owned(),
            });
        }
        for (position, (chunk_index, _, _)) in chunks.iter().enumerate() {
            if *chunk_index != position as i64 {
                return Err(StorageError::KpiIngestDraftIncomplete {
                    draft_id: draft_id.to_owned(),
                    reason: format!(
                        "chunk indices are not contiguous from 0 (expected {position}, found {chunk_index})"
                    ),
                });
            }
        }

        let total_observations: i64 = chunks.iter().map(|(_, _, count)| count).sum();
        let total_bytes: i64 = chunks
            .iter()
            .map(|(_, payload, _)| payload.len() as i64)
            .sum();
        if total_observations > AGGREGATE_OBSERVATIONS_MAX {
            return Err(StorageError::KpiIngestDraftAggregateBudgetExceeded {
                draft_id: draft_id.to_owned(),
                reason: format!(
                    "{total_observations} assembled observations exceed the aggregate cap of {AGGREGATE_OBSERVATIONS_MAX}"
                ),
            });
        }
        if total_bytes > AGGREGATE_BYTES_MAX {
            return Err(StorageError::KpiIngestDraftAggregateBudgetExceeded {
                draft_id: draft_id.to_owned(),
                reason: format!(
                    "{total_bytes} assembled bytes exceed the aggregate cap of {AGGREGATE_BYTES_MAX}"
                ),
            });
        }
        if total_observations != expected_observations {
            return Err(StorageError::KpiIngestDraftIncomplete {
                draft_id: draft_id.to_owned(),
                reason: format!(
                    "assembled {total_observations} observations, draft declared {expected_observations}"
                ),
            });
        }

        let mut flattened: Vec<NewStagedObservation> =
            Vec::with_capacity(total_observations as usize);
        for (_, payload_json, _) in &chunks {
            let mut chunk_observations: Vec<NewStagedObservation> =
                serde_json::from_str(payload_json)?;
            flattened.append(&mut chunk_observations);
        }

        let result = install_observations_on_connection(
            &tx,
            run_id,
            holder,
            flattened,
            missing_reasons,
            execution,
        )?;
        tx.commit()?;
        Ok(result)
    }

    /// `get_kpi_ingest_status`'s `openDraft` (ADR 0102 dec. 11): `None` for no
    /// run, no active draft, or a STALE active draft (a takeover happened —
    /// not resumable, so not reported as resumable either; a pure read never
    /// mutates the lazily-superseded row itself).
    pub fn get_open_draft(&self, run_id: &str) -> StorageResult<Option<OpenDraftSummary>> {
        let connection = self.db.checkout()?;
        let attempt_count: Option<i64> = connection
            .query_row(
                "SELECT attempt_count FROM kpi_ingest_runs WHERE id = ?1",
                [run_id],
                |row| row.get(0),
            )
            .optional()?;
        let Some(attempt_count) = attempt_count else {
            return Ok(None);
        };
        let row: Option<(String, i64, i64)> = connection
            .query_row(
                "SELECT draft_id, expected_observations, lease_epoch \
                 FROM kpi_ingest_drafts WHERE run_id = ?1 AND status = 'active'",
                [run_id],
                |row| Ok((row.get(0)?, row.get(1)?, row.get(2)?)),
            )
            .optional()?;
        let Some((draft_id, expected_observations, lease_epoch)) = row else {
            return Ok(None);
        };
        if lease_epoch != attempt_count {
            return Ok(None);
        }
        let chunks_received: i64 = connection.query_row(
            "SELECT COUNT(*) FROM kpi_ingest_draft_chunks WHERE draft_id = ?1",
            [&draft_id],
            |row| row.get(0),
        )?;
        Ok(Some(OpenDraftSummary {
            draft_id,
            expected_observations,
            chunks_received,
        }))
    }
}

/// Connection-level unconditional draft cleanup (the
/// `install_observations_on_connection` idiom): cancel/failure/startup-reclaim
/// (`kpi_ingest_runs.rs`) call this under their OWN transaction so a run
/// leaving `{extracting, validation_failed}` never leaves an orphaned draft
/// behind (ADR 0102 dec. 11) — `ON DELETE CASCADE` clears the chunks.
pub(super) fn clear_drafts_on_connection(tx: &Connection, run_id: &str) -> StorageResult<()> {
    tx.execute("DELETE FROM kpi_ingest_drafts WHERE run_id = ?1", [run_id])?;
    Ok(())
}

/// Startup self-heal companion (`reclaim_ingest_runs_on_startup`): a draft
/// requires a LIVE lease to be usable at all, so any draft left on a
/// currently leaseless run is definitionally orphaned — broader than "only
/// the leases THIS call just cleared" but strictly correct, and structurally
/// simpler than threading the affected run ids back out of
/// `clear_expired_leases_on_connection`.
pub(super) fn clear_orphaned_drafts_on_connection(tx: &Connection) -> StorageResult<usize> {
    Ok(tx.execute(
        "DELETE FROM kpi_ingest_drafts WHERE run_id IN \
            (SELECT id FROM kpi_ingest_runs WHERE lease_holder IS NULL)",
        [],
    )?)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::storage::migrations::open_in_memory_database;
    use rusqlite::Connection;

    const TEST_HOLDER: &str = "agent-1";

    fn seed_company(connection: &Connection, id: &str) {
        connection
            .execute(
                &format!(
                    "INSERT INTO companies (id, exchange, ticker, qualified_ticker, display_name)
                     VALUES ('{id}', 'gpw', '{id}', 'GPW:{id}', '{id} SA')"
                ),
                [],
            )
            .expect("company");
    }

    fn seed_document(connection: &Connection, id: &str, company_id: &str) {
        connection
            .execute(
                "INSERT INTO report_documents (id, company_id, source_type, url, fetch_status)
                 VALUES (?1, ?2, 'espi_attachment', ?3, 'fetched')",
                params![id, company_id, format!("https://x/{id}.pdf")],
            )
            .expect("document");
    }

    /// Mirrors `kpi_ingest_staging::tests::seed_run` (same file-local
    /// duplication idiom already established there) — a complete period
    /// descriptor, `extracting` by default.
    fn seed_run(connection: &Connection, id: &str, doc: &str, company: &str, status: &str) {
        connection
            .execute(
                "INSERT INTO kpi_ingest_runs
                    (id, report_document_id, company_id, profile_version, status,
                     period_fiscal_year, period_type, scope, data_quality)
                 VALUES (?1, ?2, ?3, 'p1', ?4, 2025, 'FY', 'consolidated', 'final')",
                params![id, doc, company, status],
            )
            .expect("seed run");
    }

    fn observation(label: &str) -> NewStagedObservation {
        NewStagedObservation {
            raw_label: label.to_owned(),
            raw_value: "1000".to_owned(),
            normalized_value: Some("1000".to_owned()),
            currency: Some("PLN".to_owned()),
            metric_key_candidate: Some("revenue".to_owned()),
            mapping_status: Some("mapped".to_owned()),
            citation_page: Some(3),
            ..Default::default()
        }
    }

    fn setup() -> (AppState, &'static str) {
        let connection = open_in_memory_database().expect("db");
        seed_company(&connection, "c1");
        seed_document(&connection, "doc1", "c1");
        seed_run(&connection, "run1", "doc1", "c1", "extracting");
        let state = AppState::new(connection);
        state
            .kpi_ingest_runs()
            .claim_next(TEST_HOLDER, 3600)
            .expect("claim")
            .expect("run1 must be claimable");
        (state, "run1")
    }

    fn attempt_count(state: &AppState, run_id: &str) -> i64 {
        state
            .checkout_for_tests()
            .expect("raw")
            .query_row(
                "SELECT attempt_count FROM kpi_ingest_runs WHERE id = ?1",
                [run_id],
                |row| row.get(0),
            )
            .expect("attempt_count")
    }

    // --- open_draft ----------------------------------------------------------

    #[test]
    fn open_draft_issues_server_draft_id_and_second_open_refused() {
        let (state, run_id) = setup();
        let store = state.kpi_ingest_drafts();

        let opened = store
            .open_draft(run_id, TEST_HOLDER, 5)
            .expect("first open");
        assert!(opened.draft_id.starts_with("kpidft_"));
        assert_eq!(opened.expected_observations, 5);
        assert_eq!(opened.chunks_received, 0);

        // A second open on the SAME live epoch is a typed refusal — an
        // explicit abort is required, never a silent orphan (ADR 0102 dec.
        // 6/11).
        let error = store
            .open_draft(run_id, TEST_HOLDER, 3)
            .expect_err("second open must refuse");
        assert!(matches!(
            error,
            StorageError::KpiIngestActiveDraftExists { ref draft_id, .. }
                if *draft_id == opened.draft_id
        ));
    }

    // --- append_chunk ----------------------------------------------------------

    #[test]
    fn chunk_replay_same_content_idempotent_ack() {
        let (state, run_id) = setup();
        let store = state.kpi_ingest_drafts();
        let draft = store.open_draft(run_id, TEST_HOLDER, 2).expect("open");

        let first = store
            .append_chunk(
                run_id,
                TEST_HOLDER,
                &draft.draft_id,
                0,
                vec![observation("a"), observation("b")],
            )
            .expect("first append");
        assert_eq!(first.chunks_received, 1);

        // Replaying the EXACT same (draftId, chunkIndex, content) is an
        // idempotent ack — safe retry over an unreliable transport (ADR 0102
        // dec. 8), never a duplicate row.
        let replay = store
            .append_chunk(
                run_id,
                TEST_HOLDER,
                &draft.draft_id,
                0,
                vec![observation("a"), observation("b")],
            )
            .expect("replay is idempotent");
        assert_eq!(replay.chunks_received, 1, "no duplicate chunk row");
    }

    #[test]
    fn chunk_same_index_different_content_typed_conflict() {
        let (state, run_id) = setup();
        let store = state.kpi_ingest_drafts();
        let draft = store.open_draft(run_id, TEST_HOLDER, 2).expect("open");
        store
            .append_chunk(
                run_id,
                TEST_HOLDER,
                &draft.draft_id,
                0,
                vec![observation("a")],
            )
            .expect("first append");

        let error = store
            .append_chunk(
                run_id,
                TEST_HOLDER,
                &draft.draft_id,
                0,
                vec![observation("DIFFERENT")],
            )
            .expect_err("same index, different content must conflict");
        assert!(matches!(
            error,
            StorageError::KpiIngestDraftChunkConflict { chunk_index: 0, .. }
        ));
    }

    #[test]
    fn append_requires_live_lease() {
        let (state, run_id) = setup();
        let store = state.kpi_ingest_drafts();
        let draft = store.open_draft(run_id, TEST_HOLDER, 1).expect("open");

        // Expire the lease directly (the wall-clock TTL is not a correctness
        // mechanism — `lease_refusal_on_connection`'s three-way classifier is
        // what every agent-facing intent routes through, staging included).
        state
            .checkout_for_tests()
            .expect("raw")
            .execute(
                "UPDATE kpi_ingest_runs SET lease_expires_at = '2000-01-01T00:00:00.000Z' \
                 WHERE id = ?1",
                [run_id],
            )
            .expect("expire lease");

        let error = store
            .append_chunk(
                run_id,
                TEST_HOLDER,
                &draft.draft_id,
                0,
                vec![observation("a")],
            )
            .expect_err("append against an expired lease must refuse");
        assert!(matches!(error, StorageError::RunLeaseExpired { .. }));
    }

    #[test]
    fn append_never_bumps_revision_invisible_to_validation() {
        let (state, run_id) = setup();
        let store = state.kpi_ingest_drafts();
        let draft = store.open_draft(run_id, TEST_HOLDER, 3).expect("open");
        store
            .append_chunk(
                run_id,
                TEST_HOLDER,
                &draft.draft_id,
                0,
                vec![observation("a"), observation("b"), observation("c")],
            )
            .expect("append");

        let run = state
            .kpi_ingest_runs()
            .get_run(run_id)
            .expect("get")
            .expect("some");
        assert_eq!(
            run.manifest_revision, 0,
            "append must never bump manifest_revision"
        );
        assert_eq!(
            run.status,
            KpiIngestRunState::Extracting,
            "append must never flip run status"
        );
        assert!(
            state
                .kpi_ingest_staging()
                .list_staged_observations(run_id, None)
                .expect("list")
                .is_empty(),
            "an appended-but-unfinalized chunk is structurally invisible to \
             the staged-observations table validation reads"
        );
    }

    // --- finalize_draft ----------------------------------------------------------

    #[test]
    fn finalize_gap_refused() {
        let (state, run_id) = setup();
        let store = state.kpi_ingest_drafts();
        let draft = store.open_draft(run_id, TEST_HOLDER, 2).expect("open");
        store
            .append_chunk(
                run_id,
                TEST_HOLDER,
                &draft.draft_id,
                0,
                vec![observation("a")],
            )
            .expect("chunk 0");
        // Skip chunk 1 — append chunk 2 directly (a gap).
        store
            .append_chunk(
                run_id,
                TEST_HOLDER,
                &draft.draft_id,
                2,
                vec![observation("b")],
            )
            .expect("chunk 2");

        let error = store
            .finalize_draft(run_id, TEST_HOLDER, &draft.draft_id, &BTreeMap::new(), None)
            .expect_err("a gap in chunk indices must refuse finalize");
        assert!(matches!(
            error,
            StorageError::KpiIngestDraftIncomplete { .. }
        ));
    }

    #[test]
    fn finalize_zero_chunks_refused() {
        let (state, run_id) = setup();
        let store = state.kpi_ingest_drafts();
        let draft = store.open_draft(run_id, TEST_HOLDER, 1).expect("open");
        let error = store
            .finalize_draft(run_id, TEST_HOLDER, &draft.draft_id, &BTreeMap::new(), None)
            .expect_err("zero chunks must refuse finalize");
        assert!(matches!(
            error,
            StorageError::KpiIngestDraftIncomplete { .. }
        ));
    }

    #[test]
    fn finalize_aggregate_caps_refused() {
        // Count: a single chunk carrying MORE than AGGREGATE_OBSERVATIONS_MAX
        // (1000) observations — storage itself does not cap a chunk's size
        // (that per-call 100-observation cap is enforced at the MCP tool
        // boundary, uniformly for every call carrying `observations`); this
        // test proves the AGGREGATE ceiling at finalize independently of that
        // per-call cap.
        {
            let (state, run_id) = setup();
            let store = state.kpi_ingest_drafts();
            let draft = store.open_draft(run_id, TEST_HOLDER, 1001).expect("open");
            let observations: Vec<NewStagedObservation> =
                (0..1001).map(|i| observation(&format!("o{i}"))).collect();
            store
                .append_chunk(run_id, TEST_HOLDER, &draft.draft_id, 0, observations)
                .expect("oversized chunk append");
            let error = store
                .finalize_draft(run_id, TEST_HOLDER, &draft.draft_id, &BTreeMap::new(), None)
                .expect_err("1001 observations must exceed AGGREGATE_OBSERVATIONS_MAX");
            assert!(matches!(
                error,
                StorageError::KpiIngestDraftAggregateBudgetExceeded { .. }
            ));
        }

        // Bytes: two chunks whose payload_json alone exceeds AGGREGATE_BYTES_MAX
        // (7,198,720 B) via one enormous field each — proves the byte cap
        // independently of the observation-count cap above.
        {
            let (state, run_id) = setup();
            let store = state.kpi_ingest_drafts();
            let draft = store.open_draft(run_id, TEST_HOLDER, 2).expect("open");
            let huge = "x".repeat(5_000_000);
            let mut big_a = observation("a");
            big_a.citation_quote = Some(huge.clone());
            let mut big_b = observation("b");
            big_b.citation_quote = Some(huge);
            store
                .append_chunk(run_id, TEST_HOLDER, &draft.draft_id, 0, vec![big_a])
                .expect("chunk 0");
            store
                .append_chunk(run_id, TEST_HOLDER, &draft.draft_id, 1, vec![big_b])
                .expect("chunk 1");

            let error = store
                .finalize_draft(run_id, TEST_HOLDER, &draft.draft_id, &BTreeMap::new(), None)
                .expect_err("10 MB of payload must exceed AGGREGATE_BYTES_MAX");
            assert!(matches!(
                error,
                StorageError::KpiIngestDraftAggregateBudgetExceeded { .. }
            ));
        }
    }

    #[test]
    fn finalize_installs_via_the_shared_helper_and_deletes_the_draft() {
        let (state, run_id) = setup();
        let store = state.kpi_ingest_drafts();
        let draft = store.open_draft(run_id, TEST_HOLDER, 3).expect("open");
        store
            .append_chunk(
                run_id,
                TEST_HOLDER,
                &draft.draft_id,
                0,
                vec![observation("a"), observation("b")],
            )
            .expect("chunk 0");
        store
            .append_chunk(
                run_id,
                TEST_HOLDER,
                &draft.draft_id,
                1,
                vec![observation("c")],
            )
            .expect("chunk 1");

        let (revision, inserted) = store
            .finalize_draft(run_id, TEST_HOLDER, &draft.draft_id, &BTreeMap::new(), None)
            .expect("finalize");
        assert_eq!(revision, 1);
        assert_eq!(inserted.len(), 3);
        // Global ordinals ordered (chunkIndex, position-within-chunk).
        assert_eq!(inserted[0].raw_label, "a");
        assert_eq!(inserted[1].raw_label, "b");
        assert_eq!(inserted[2].raw_label, "c");
        assert_eq!(inserted[0].ordinal, 0);
        assert_eq!(inserted[2].ordinal, 2);

        let run = state
            .kpi_ingest_runs()
            .get_run(run_id)
            .expect("get")
            .expect("some");
        assert_eq!(run.status, KpiIngestRunState::Staged);
        assert_eq!(run.manifest_revision, 1);

        assert_eq!(
            store.get_open_draft(run_id).expect("read"),
            None,
            "finalize deletes the draft row (cascades to its chunks)"
        );
    }

    // --- lease takeover ----------------------------------------------------------

    #[test]
    fn takeover_invalidates_draft() {
        let (state, run_id) = setup();
        let store = state.kpi_ingest_drafts();
        let draft = store.open_draft(run_id, TEST_HOLDER, 1).expect("open");
        let epoch_at_open = attempt_count(&state, run_id);

        // Simulate a takeover: expire the lease, then a DIFFERENT holder
        // claims the run — `attempt_count` bumps (the epoch this draft is
        // bound to), exactly like a real lease takeover would.
        state
            .checkout_for_tests()
            .expect("raw")
            .execute(
                "UPDATE kpi_ingest_runs SET lease_expires_at = '2000-01-01T00:00:00.000Z' \
                 WHERE id = ?1",
                [run_id],
            )
            .expect("expire lease");
        state
            .kpi_ingest_runs()
            .claim_next("agent-2", 3600)
            .expect("takeover claim")
            .expect("run1 must be claimable by the new holder");
        assert!(
            attempt_count(&state, run_id) > epoch_at_open,
            "attempt_count must have bumped on takeover"
        );

        let error = store
            .append_chunk(
                run_id,
                "agent-2",
                &draft.draft_id,
                0,
                vec![observation("a")],
            )
            .expect_err("a stale-epoch draft is superseded, never resumable");
        assert!(matches!(
            error,
            StorageError::KpiIngestDraftSuperseded { ref draft_id } if *draft_id == draft.draft_id
        ));

        // The new holder can open a FRESH draft — the stale one no longer
        // occupies the one-active-draft-per-run slot (lazily superseded).
        let fresh = store
            .open_draft(run_id, "agent-2", 1)
            .expect("fresh open after takeover");
        assert_ne!(fresh.draft_id, draft.draft_id);
    }

    // --- single-call interaction ----------------------------------------------------------

    #[test]
    fn single_call_with_open_draft_refused() {
        let (state, run_id) = setup();
        let store = state.kpi_ingest_drafts();
        let draft = store.open_draft(run_id, TEST_HOLDER, 1).expect("open");

        let error = state
            .kpi_ingest_staging()
            .stage_observations(
                run_id,
                TEST_HOLDER,
                vec![observation("a")],
                &BTreeMap::new(),
                None,
            )
            .expect_err("single-call while a draft is open must refuse");
        assert!(matches!(
            error,
            StorageError::KpiIngestActiveDraftExists { ref draft_id, .. }
                if *draft_id == draft.draft_id
        ));
    }

    // --- lifecycle cleanup ----------------------------------------------------------

    #[test]
    fn cancel_and_failure_clear_draft() {
        let terminalizers: [fn(&AppState, &str); 2] = [
            |state, id| {
                state.kpi_ingest_runs().cancel_run(id).expect("cancel");
            },
            |state, id| {
                state
                    .kpi_ingest_runs()
                    .mark_failed(id, "boom")
                    .expect("mark_failed");
            },
        ];
        for terminalize in terminalizers {
            let (state, run_id) = setup();
            let store = state.kpi_ingest_drafts();
            let draft = store.open_draft(run_id, TEST_HOLDER, 1).expect("open");
            store
                .append_chunk(
                    run_id,
                    TEST_HOLDER,
                    &draft.draft_id,
                    0,
                    vec![observation("a")],
                )
                .expect("append");

            terminalize(&state, run_id);

            assert_eq!(
                store.get_open_draft(run_id).expect("read"),
                None,
                "the draft row must be gone"
            );
            let chunk_count: i64 = state
                .checkout_for_tests()
                .expect("raw")
                .query_row(
                    "SELECT COUNT(*) FROM kpi_ingest_draft_chunks WHERE draft_id = ?1",
                    [&draft.draft_id],
                    |row| row.get(0),
                )
                .expect("count");
            assert_eq!(
                chunk_count, 0,
                "ON DELETE CASCADE must clear its chunks too"
            );
        }
    }

    // --- two racing finalizers ----------------------------------------------------------

    /// The chunked-draft analog of `kpi_ingest_staging::tests::
    /// stage_observations_two_threads_exactly_one_winner` (which proves
    /// exactly one of two racing SINGLE-CALL `stage_observations` calls wins
    /// the same `extracting` run): two threads racing `finalize_draft` on the
    /// SAME already-appended draft. The mechanism is UNCHANGED — finalize
    /// shares `install_observations_on_connection`'s guarded UPDATE with the
    /// single-call path (ADR 0102 dec. 9), so the winner still flips the run
    /// `extracting -> staged` under the SAME `Immediate`-transaction
    /// serialization. What's DIFFERENT from the old test: the winner's
    /// transaction also DELETEs the draft row (dec. 9's unconditional
    /// cleanup) before the loser's transaction ever starts, so the loser's
    /// OWN `load_live_draft_or_supersede` lookup (which runs BEFORE the
    /// shared guarded UPDATE) now finds no draft row at all — the loser's
    /// typed error is `KpiIngestDraftNotFound`, not the old test's
    /// `InvalidRunStateForStaging` (that status-guard trip is exactly what
    /// the FIRST loser's OWN concurrent single-call/finalize would still hit
    /// if a draft row happened to survive — here it structurally cannot,
    /// because deleting the draft is part of the very same winning
    /// transaction).
    #[test]
    fn finalize_draft_two_threads_exactly_one_winner_typed_conflict() {
        use r2d2_sqlite::SqliteConnectionManager;
        use std::sync::Arc;

        let db_path = std::env::temp_dir().join(format!(
            "brawler-kpi-draft-finalize-race-{}-{}.sqlite3",
            std::process::id(),
            time::OffsetDateTime::now_utc().unix_timestamp_nanos()
        ));
        let draft_id = {
            let mut connection = Connection::open(&db_path).expect("open file db");
            crate::storage::migrations::apply_migrations(&mut connection).expect("migrate");
            seed_company(&connection, "c1");
            seed_document(&connection, "doc1", "c1");
            seed_run(&connection, "run1", "doc1", "c1", "extracting");
            let state = AppState::new(connection);
            state
                .kpi_ingest_runs()
                .claim_next(TEST_HOLDER, 3600)
                .expect("claim")
                .expect("run1 must be claimable");
            let draft = state
                .kpi_ingest_drafts()
                .open_draft("run1", TEST_HOLDER, 1)
                .expect("open");
            state
                .kpi_ingest_drafts()
                .append_chunk(
                    "run1",
                    TEST_HOLDER,
                    &draft.draft_id,
                    0,
                    vec![observation("a")],
                )
                .expect("append");
            draft.draft_id
        };

        let manager = SqliteConnectionManager::file(&db_path).with_init(|connection| {
            connection.execute_batch("PRAGMA journal_mode = WAL; PRAGMA foreign_keys = ON;")?;
            connection.pragma_update(None, "busy_timeout", 5000i64)?;
            Ok(())
        });
        let pool = r2d2::Pool::builder()
            .max_size(4)
            .build(manager)
            .expect("build pool");
        let store = Arc::new(KpiIngestDraftsStore::new(Database::from_pool(pool)));

        let barrier = Arc::new(std::sync::Barrier::new(2));
        let store_a = store.clone();
        let store_b = store.clone();
        let barrier_a = barrier.clone();
        let barrier_b = barrier;
        let draft_id_a = draft_id.clone();
        let draft_id_b = draft_id;
        let a = std::thread::spawn(move || {
            barrier_a.wait();
            store_a.finalize_draft("run1", TEST_HOLDER, &draft_id_a, &BTreeMap::new(), None)
        });
        let b = std::thread::spawn(move || {
            barrier_b.wait();
            store_b.finalize_draft("run1", TEST_HOLDER, &draft_id_b, &BTreeMap::new(), None)
        });
        let result_a = a.join().expect("thread a");
        let result_b = b.join().expect("thread b");

        let winners = [&result_a, &result_b]
            .iter()
            .filter(|result| result.is_ok())
            .count();
        assert_eq!(winners, 1, "exactly one finalize must win");
        for result in [&result_a, &result_b] {
            if let Err(error) = result {
                assert!(
                    matches!(
                        error,
                        StorageError::KpiIngestDraftNotFound { .. }
                            | StorageError::InvalidRunStateForStaging { .. }
                    ),
                    "the loser's error must be a typed conflict, not {error:?}"
                );
            }
        }

        let _ = std::fs::remove_file(&db_path);
        let _ = std::fs::remove_file(db_path.with_extension("sqlite3-wal"));
        let _ = std::fs::remove_file(db_path.with_extension("sqlite3-shm"));
    }
}
