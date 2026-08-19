//! KPI staging domain store (ADR 0098 decisions 3, 4, 5; epic #352, card
//! #359). `kpi_staged_observations` holds run-owned LLM proposals — NEVER
//! visible to any fact reader (dec. 3); a proposal becomes a canonical fact
//! only through the commit transaction (`KpiIngestCommitStore::commit_manifest`, #362). `kpi_ingest_commit_receipts`
//! is the immutable per-run commit outcome, append-only (the `valuation_runs`
//! idiom — zero UPDATE path here). Reach the observation surface via
//! `AppState::kpi_ingest_staging()`; `record_commit_receipt` is a
//! connection-level free fn `KpiIngestCommitStore::commit_manifest` (#362)
//! calls under its own `&Connection` (the `record_structured_fact` pattern,
//! `kpi_extraction.rs:359`).

use std::collections::{HashMap, HashSet};

use serde::{Deserialize, Serialize};

use crate::fundamentals::kpi_manifest::{Outcome as ManifestOutcome, SealedManifest};

use super::database::Database;
use super::kpi_ingest_runs::{
    mark_ready_to_commit_on_connection, mark_validation_failed_on_connection, KpiIngestRunState,
};
use super::*;

mod identity;
mod receipts;

use identity::{
    content_matches_projection, generate_attempt_id, generate_observation_id, map_observation_row,
    normalize_currency, validate_vocab,
};
pub(super) use receipts::{get_commit_receipt_on_connection, record_commit_receipt};
pub use receipts::{CommitReceipt, NewCommitReceipt};

/// `kpi_staged_observations.unit_scale` vocabulary — the live `UnitScale`
/// enum (`fundamentals::extraction::text_numbers`), NOT the dead
/// `as_reported_scale` column (M4/M3 sol review).
const UNIT_SCALE_VALUES: &[&str] = &["ones", "thousands", "millions"];
/// ADR 0098 dec. 3 period dimensions — deliberately stricter than
/// `financial_facts`, which does not `CHECK` these (staging is where the
/// hardness belongs).
const MEASURE_WINDOW_VALUES: &[&str] = &[
    "flow",
    "point_in_time",
    "trailing",
    "cumulative",
    "duration",
];
const ATTRIBUTION_VALUES: &[&str] = &["total", "owners_of_parent", "nci"];
const SCOPE_VALUES: &[&str] = &["standalone", "consolidated"];
const MAPPING_STATUS_VALUES: &[&str] = &["unmapped", "mapped", "no_definition", "excluded"];
/// Which run states `stage_observations` may act on (B1 sol review round 2):
/// `extracting` is the first snapshot, `validation_failed` is a repair
/// restage. Every other state — including `staged`/`ready_to_commit`/
/// `committing` — refuses, so staging can never invalidate a manifest a
/// commit is already holding, nor leave `ready_to_commit` without one.
pub(super) const STAGEABLE_STATUSES: &[KpiIngestRunState] = &[
    KpiIngestRunState::Extracting,
    KpiIngestRunState::ValidationFailed,
];

/// One proposed observation to stage. The store assigns `id`, `revision` and
/// `ordinal` — never caller-supplied. `validation_state`/
/// `validation_codes_json` are ALWAYS `none`/`NULL` at staging time; only
/// [`KpiIngestStagingStore::apply_validation_outcome`] ever sets them.
/// `Serialize`/`Deserialize` (epic #399 S6) are for the INTERNAL
/// `kpi_ingest_draft_chunks.payload_json` round-trip only — plain
/// `snake_case`, never a wire contract (the MCP boundary has its own
/// `ObservationInput` with camelCase/`deny_unknown_fields`/byte caps).
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct NewStagedObservation {
    /// Exactly as the document states it — never normalized.
    pub raw_label: String,
    pub raw_value: String,
    pub raw_currency: Option<String>,
    pub raw_unit_scale: Option<String>,
    /// Decimal-exact TEXT (the `valuation_runs` convention).
    pub normalized_value: Option<String>,
    pub currency: Option<String>,
    pub unit_scale: Option<String>,
    pub measure_window: Option<String>,
    pub attribution: Option<String>,
    pub scope: Option<String>,
    pub metric_key_candidate: Option<String>,
    /// `None` normalizes to `unmapped` at the write boundary.
    pub mapping_status: Option<String>,
    /// Required exactly when `mapping_status == Some("excluded")` (ADR 0102
    /// dec. 1); refused otherwise. `None`/blank normalizes to `NULL`.
    pub exclusion_reason: Option<String>,
    pub citation_page: Option<i64>,
    pub citation_table: Option<String>,
    pub citation_row: Option<String>,
    pub citation_quote: Option<String>,
}

/// A stored staged observation (the full read model).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct StagedObservation {
    pub id: String,
    pub run_id: String,
    pub revision: i64,
    pub ordinal: i64,
    pub raw_label: String,
    pub raw_value: String,
    pub raw_currency: Option<String>,
    pub raw_unit_scale: Option<String>,
    pub normalized_value: Option<String>,
    pub currency: Option<String>,
    pub unit_scale: Option<String>,
    pub measure_window: Option<String>,
    pub attribution: Option<String>,
    pub scope: Option<String>,
    pub metric_key_candidate: Option<String>,
    pub mapping_status: String,
    pub exclusion_reason: Option<String>,
    pub citation_page: Option<i64>,
    pub citation_table: Option<String>,
    pub citation_row: Option<String>,
    pub citation_quote: Option<String>,
    pub validation_state: String,
    pub validation_codes_json: Option<String>,
    pub created_at: String,
    pub updated_at: String,
}

const OBSERVATION_COLUMNS: &str = "id, run_id, revision, ordinal, raw_label, raw_value, \
     raw_currency, raw_unit_scale, normalized_value, currency, unit_scale, measure_window, \
     attribution, scope, metric_key_candidate, mapping_status, citation_page, citation_table, \
     citation_row, citation_quote, validation_state, validation_codes_json, created_at, \
     updated_at, exclusion_reason";

#[derive(Clone)]
pub struct KpiIngestStagingStore {
    db: Database,
}

impl KpiIngestStagingStore {
    pub(super) fn new(db: Database) -> Self {
        Self { db }
    }

    /// Atomically bump `kpi_ingest_runs.manifest_revision`, zero its
    /// `manifest_hash` (any new staging invalidates every prior manifest),
    /// flip `status` to `staged`, and insert the COMPLETE snapshot under the
    /// new revision — one `Immediate` transaction, so two racing stagers
    /// serialize on the run row (B1 sol review) and never interleave two
    /// revisions. Refuses an empty snapshot, an unknown run, a run outside
    /// `{extracting, validation_failed}` (`InvalidRunStateForStaging`), a run
    /// whose lease `holder` does not currently hold LIVE (#360 back-fit —
    /// `stage_observations` is one of the three agent-facing intents and
    /// never shipped a lease-free staging path, F9 r2), and a run with
    /// neither `period_id` nor a complete period descriptor. All checks run
    /// on the SAME `Immediate` transaction as the mutating UPDATE below, so
    /// there is no TOCTOU window between the SELECT and the write despite the
    /// pre-check shape (`Immediate` already holds the write lock).
    pub fn stage_observations(
        &self,
        run_id: &str,
        holder: &str,
        observations: Vec<NewStagedObservation>,
        missing_reasons: &std::collections::BTreeMap<String, String>,
        execution: Option<&serde_json::Value>,
    ) -> StorageResult<(i64, Vec<StagedObservation>)> {
        if observations.is_empty() {
            return Err(StorageError::InvalidKpiIngestRunValue {
                key: "observations",
                value: "empty".to_owned(),
            });
        }

        let mut connection = self.db.checkout()?;
        let tx = connection.transaction_with_behavior(rusqlite::TransactionBehavior::Immediate)?;

        // A single-call snapshot while a draft is open is a typed refusal
        // (ADR 0102 dec. 11) — an explicit abort is required, never a silent
        // orphan. A STALE draft (lease epoch behind the run's current
        // `attempt_count` — a takeover happened) is lazily superseded here
        // instead of blocking, the same lazy-invalidation this check shares
        // with `KpiIngestDraftsStore::open_draft`.
        let attempt_count: Option<i64> = tx
            .query_row(
                "SELECT attempt_count FROM kpi_ingest_runs WHERE id = ?1",
                [run_id],
                |row| row.get(0),
            )
            .optional()?;
        // An unknown run falls through to `install_observations_on_connection`'s
        // own (unchanged) `KpiIngestRunNotFound` — this pre-check only needs to
        // run the draft-conflict guard for a run that actually exists.
        if let Some(attempt_count) = attempt_count {
            super::kpi_ingest_drafts::resolve_active_draft_conflict_on_connection(
                &tx,
                run_id,
                attempt_count,
            )?;
        }

        let result = install_observations_on_connection(
            &tx,
            run_id,
            holder,
            observations,
            missing_reasons,
            execution,
        )?;
        tx.commit()?;
        Ok(result)
    }

    /// `revision = None` resolves to the highest stored revision for the run;
    /// an unknown run or revision returns an empty list, never an error (the
    /// "list" read-model convention — `get_run`'s `Option`, not a typed
    /// refusal).
    pub fn list_staged_observations(
        &self,
        run_id: &str,
        revision: Option<i64>,
    ) -> StorageResult<Vec<StagedObservation>> {
        let connection = self.db.checkout()?;
        let rows: Vec<StagedObservation> = match revision {
            Some(revision) => {
                let mut statement = connection.prepare(&format!(
                    "SELECT {OBSERVATION_COLUMNS} FROM kpi_staged_observations
                     WHERE run_id = ?1 AND revision = ?2 ORDER BY ordinal"
                ))?;
                let rows = statement
                    .query_map(params![run_id, revision], map_observation_row)?
                    .collect::<Result<_, _>>()?;
                rows
            }
            None => {
                let mut statement = connection.prepare(&format!(
                    "SELECT {OBSERVATION_COLUMNS} FROM kpi_staged_observations
                     WHERE run_id = ?1
                       AND revision = (SELECT MAX(revision) FROM kpi_staged_observations WHERE run_id = ?1)
                     ORDER BY ordinal"
                ))?;
                let rows = statement
                    .query_map([run_id], map_observation_row)?
                    .collect::<Result<_, _>>()?;
                rows
            }
        };
        Ok(rows)
    }

    /// The highest staged revision for a run, or `None` if it was never
    /// staged.
    pub fn latest_staging_revision(&self, run_id: &str) -> StorageResult<Option<i64>> {
        let connection = self.db.checkout()?;
        connection
            .query_row(
                "SELECT MAX(revision) FROM kpi_staged_observations WHERE run_id = ?1",
                [run_id],
                |row| row.get(0),
            )
            .map_err(StorageError::from)
    }

    /// Applies a [`SealedManifest`] to ONE staging revision — the ONLY path
    /// to `staged -> {validation_failed, ready_to_commit}` (#361 sol r4):
    /// verdicts, codes, the immutable attempt row, and the run transition are
    /// ALL derived from `sealed`, in ONE `Immediate` transaction, so a
    /// manifest↔stored-rows disagreement is structurally impossible.
    ///
    /// Guards, in order (any failure rolls back EVERYTHING, including the
    /// attempt insert):
    /// 1. run exists, `status = 'staged'`, `manifest_revision = revision`,
    ///    `manifest_hash IS NULL` (`InvalidStagingRevision`, the same class
    ///    the retired `apply_validation_results` used).
    /// 2. `sealed.run_id() == run_id` and `sealed.revision() == revision`
    ///    (`SealedManifestRejected`) — a manifest built for a different run/
    ///    revision must never bind here even if the caller mis-threads it.
    /// 3. RUN-CONTEXT BINDING (finding 1b): `sealed`'s companyId,
    ///    reportDocumentId, sourceContentHash, scope, dataQuality, period
    ///    (periodId + fiscalYear + periodType), and expectedKpis (byte-compared
    ///    against `expected_kpis_json`, both-NULL ok) must equal the LIVE run
    ///    row's values (`SealedManifestRejected`) — a manifest built against a
    ///    stale/wrong run context must never bind even when the run/revision
    ///    id itself is right.
    /// 4. COUNT: `sealed`'s observation count equals the live row count —
    ///    defense in depth alongside the coverage/content checks below, since
    ///    `SealedManifest::seal` already refuses a manifest with a duplicate
    ///    `observationId` (`SealedManifestRejected`).
    /// 5. COVERAGE: the observation ids in `sealed` are EXACTLY the staged
    ///    observations of this revision — no fewer, no more
    ///    (`SealedManifestRejected`).
    /// 6. CONTENT (sol r5-2 tamper guard): `sealed`'s canonical staged-content
    ///    projection per observation is re-compared against the LIVE row —
    ///    right ids, wrong value/citation/dims is refused, zero writes
    ///    (`SealedManifestRejected`).
    ///
    /// Then: batch-write every verdict (`validation_state` +
    /// `validation_codes_json`, both taken from `sealed` — never a separate
    /// caller-supplied `results` list), INSERT the immutable attempt row
    /// (`attempt = COALESCE(MAX(attempt), 0) + 1` for `(run_id, revision)`,
    /// computed in this same transaction), and transition the run via
    /// [`mark_ready_to_commit_on_connection`] or
    /// [`mark_validation_failed_on_connection`] according to
    /// `sealed.outcome()`.
    pub fn apply_validation_outcome(
        &self,
        run_id: &str,
        revision: i64,
        sealed: SealedManifest,
    ) -> StorageResult<()> {
        struct RunBindingRow {
            status: String,
            manifest_revision: i64,
            manifest_hash: Option<String>,
            company_id: String,
            report_document_id: String,
            source_content_hash: Option<String>,
            scope: Option<String>,
            data_quality: Option<String>,
            period_id: Option<String>,
            period_fiscal_year: Option<i64>,
            period_type: Option<String>,
            expected_kpis_json: Option<String>,
        }

        let mut connection = self.db.checkout()?;
        let tx = connection.transaction_with_behavior(rusqlite::TransactionBehavior::Immediate)?;

        let run: Option<RunBindingRow> = tx
            .query_row(
                "SELECT status, manifest_revision, manifest_hash, company_id, \
                     report_document_id, source_content_hash, scope, data_quality, \
                     period_id, period_fiscal_year, period_type, expected_kpis_json \
                 FROM kpi_ingest_runs WHERE id = ?1",
                [run_id],
                |row| {
                    Ok(RunBindingRow {
                        status: row.get(0)?,
                        manifest_revision: row.get(1)?,
                        manifest_hash: row.get(2)?,
                        company_id: row.get(3)?,
                        report_document_id: row.get(4)?,
                        source_content_hash: row.get(5)?,
                        scope: row.get(6)?,
                        data_quality: row.get(7)?,
                        period_id: row.get(8)?,
                        period_fiscal_year: row.get(9)?,
                        period_type: row.get(10)?,
                        expected_kpis_json: row.get(11)?,
                    })
                },
            )
            .optional()?;
        let Some(run) = run else {
            return Err(StorageError::KpiIngestRunNotFound {
                id: run_id.to_owned(),
            });
        };
        let RunBindingRow {
            status,
            manifest_revision,
            manifest_hash,
            company_id,
            report_document_id,
            source_content_hash,
            scope,
            data_quality,
            period_id,
            period_fiscal_year,
            period_type,
            expected_kpis_json,
        } = run;
        if KpiIngestRunState::parse(&status)? != KpiIngestRunState::Staged {
            return Err(StorageError::InvalidStagingRevision {
                run_id: run_id.to_owned(),
                revision,
                reason: "run is not in status 'staged'",
            });
        }
        if manifest_hash.is_some() {
            return Err(StorageError::InvalidStagingRevision {
                run_id: run_id.to_owned(),
                revision,
                reason: "revision is frozen: the run already has an issued manifest",
            });
        }
        if manifest_revision != revision {
            return Err(StorageError::InvalidStagingRevision {
                run_id: run_id.to_owned(),
                revision,
                reason: "revision is not the run's current staging revision",
            });
        }

        if sealed.run_id() != run_id {
            return Err(StorageError::SealedManifestRejected {
                run_id: run_id.to_owned(),
                revision,
                reason: "sealed manifest's runId does not match the target run",
            });
        }
        if sealed.revision() != revision {
            return Err(StorageError::SealedManifestRejected {
                run_id: run_id.to_owned(),
                revision,
                reason: "sealed manifest's revision does not match the target revision",
            });
        }

        // Run-context binding (#361 finding 1b): a manifest built for a
        // different company/document/hash/scope/quality/period/expected-
        // snapshot must never bind here even if it names the right run id
        // and revision.
        if sealed.company_id() != company_id {
            return Err(StorageError::SealedManifestRejected {
                run_id: run_id.to_owned(),
                revision,
                reason: "sealed manifest's companyId does not match the run",
            });
        }
        if sealed.report_document_id() != report_document_id {
            return Err(StorageError::SealedManifestRejected {
                run_id: run_id.to_owned(),
                revision,
                reason: "sealed manifest's reportDocumentId does not match the run",
            });
        }
        if sealed.source_content_hash() != source_content_hash.as_deref() {
            return Err(StorageError::SealedManifestRejected {
                run_id: run_id.to_owned(),
                revision,
                reason: "sealed manifest's sourceContentHash does not match the run",
            });
        }
        if Some(sealed.scope()) != scope.as_deref() {
            return Err(StorageError::SealedManifestRejected {
                run_id: run_id.to_owned(),
                revision,
                reason: "sealed manifest's scope does not match the run",
            });
        }
        if Some(sealed.data_quality()) != data_quality.as_deref() {
            return Err(StorageError::SealedManifestRejected {
                run_id: run_id.to_owned(),
                revision,
                reason: "sealed manifest's dataQuality does not match the run",
            });
        }
        if sealed.period_id() != period_id.as_deref()
            || Some(sealed.fiscal_year()) != period_fiscal_year
            || Some(sealed.period_type()) != period_type.as_deref()
        {
            return Err(StorageError::SealedManifestRejected {
                run_id: run_id.to_owned(),
                revision,
                reason: "sealed manifest's period does not match the run",
            });
        }
        if sealed.expected_kpis_json() != expected_kpis_json.as_deref() {
            return Err(StorageError::SealedManifestRejected {
                run_id: run_id.to_owned(),
                revision,
                reason: "sealed manifest's expectedKpis does not match the run's stamped snapshot",
            });
        }

        let mut statement = tx.prepare(&format!(
            "SELECT {OBSERVATION_COLUMNS} FROM kpi_staged_observations
             WHERE run_id = ?1 AND revision = ?2 ORDER BY ordinal"
        ))?;
        let staged: Vec<StagedObservation> = statement
            .query_map(params![run_id, revision], map_observation_row)?
            .collect::<Result<_, _>>()?;
        drop(statement);

        // Defense in depth (finding 1b): the set-equality coverage check
        // below collapses duplicates, so also require the sealed manifest's
        // observation COUNT to equal the live row count -- duplicates are
        // refused even if `SealedManifest::seal` somehow missed them.
        if sealed.observation_verdicts().len() != staged.len() {
            return Err(StorageError::SealedManifestRejected {
                run_id: run_id.to_owned(),
                revision,
                reason: "sealed manifest's observation count does not match this revision's staged observation count",
            });
        }

        let staged_ids: HashSet<&str> = staged.iter().map(|o| o.id.as_str()).collect();
        let sealed_ids: HashSet<&str> = sealed
            .observation_verdicts()
            .iter()
            .map(|v| v.observation_id.as_str())
            .collect();
        if staged_ids != sealed_ids {
            return Err(StorageError::SealedManifestRejected {
                run_id: run_id.to_owned(),
                revision,
                reason: "sealed manifest's observations do not exactly match this revision's staged observations",
            });
        }

        let staged_by_id: HashMap<&str, &StagedObservation> =
            staged.iter().map(|o| (o.id.as_str(), o)).collect();
        for projection in sealed.observation_content() {
            let row = staged_by_id
                .get(projection.observation_id.as_str())
                .expect("coverage check above guarantees this id exists");
            if !content_matches_projection(row, projection) {
                return Err(StorageError::SealedManifestRejected {
                    run_id: run_id.to_owned(),
                    revision,
                    reason: "sealed manifest's staged-content projection does not match the live observation row",
                });
            }
        }

        for verdict in sealed.observation_verdicts() {
            let changed = tx.execute(
                "UPDATE kpi_staged_observations
                 SET validation_state = ?1,
                     validation_codes_json = ?2,
                     updated_at = strftime('%Y-%m-%dT%H:%M:%fZ', 'now')
                 WHERE id = ?3 AND run_id = ?4 AND revision = ?5",
                params![
                    verdict.validation_state,
                    verdict.validation_codes_json,
                    verdict.observation_id,
                    run_id,
                    revision
                ],
            )?;
            debug_assert_eq!(
                changed, 1,
                "coverage check above guarantees this row exists"
            );
        }

        let attempt: i64 = tx.query_row(
            "SELECT COALESCE(MAX(attempt), 0) + 1 FROM kpi_ingest_validation_attempts
             WHERE run_id = ?1 AND revision = ?2",
            params![run_id, revision],
            |row| row.get(0),
        )?;
        let attempt_id = generate_attempt_id(run_id, revision, attempt);
        let outcome_str = match sealed.outcome() {
            ManifestOutcome::Ready => "ready",
            ManifestOutcome::Failed => "failed",
        };
        tx.execute(
            "INSERT INTO kpi_ingest_validation_attempts
                (id, run_id, revision, attempt, outcome, manifest_hash, manifest_json)
             VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7)",
            params![
                attempt_id,
                run_id,
                revision,
                attempt,
                outcome_str,
                sealed.manifest_hash(),
                sealed.manifest_json(),
            ],
        )?;

        match sealed.outcome() {
            ManifestOutcome::Ready => {
                mark_ready_to_commit_on_connection(&tx, run_id, revision, sealed.manifest_hash())?;
            }
            ManifestOutcome::Failed => {
                mark_validation_failed_on_connection(&tx, run_id, revision)?;
            }
        }

        // Diagnostic progress snapshot (#364) — same transaction as the verdict
        // batch and the run transition, so it can never lag or lead the state
        // it describes.
        let flagged = sealed
            .observation_verdicts()
            .iter()
            .filter(|v| v.validation_state == "flagged")
            .count();
        let step = match sealed.outcome() {
            ManifestOutcome::Ready => "validation_ready",
            ManifestOutcome::Failed => "validation_failed",
        };
        crate::storage::kpi_ingest_runs::write_progress_snapshot_on_connection(
            &tx,
            run_id,
            step,
            revision,
            Some(sealed.manifest_hash()),
            serde_json::json!({
                "observations": sealed.observation_verdicts().len(),
                "flagged": flagged,
            }),
        )?;

        tx.commit()?;
        Ok(())
    }

    pub fn get_commit_receipt(&self, run_id: &str) -> StorageResult<Option<CommitReceipt>> {
        let connection = self.db.checkout()?;
        get_commit_receipt_on_connection(&connection, run_id)
    }
}

/// The shared connection-level install primitive (ADR 0102 dec. 9, epic #399
/// S6): bump-revision + install-observations + flip-state + delete-drafts, in
/// ONE transaction owned by the CALLER. Extracted verbatim from
/// [`KpiIngestStagingStore::stage_observations`]'s body (only the connection
/// checkout/commit moved to the caller) so both routes — the single-call
/// path above and `KpiIngestDraftsStore::finalize_draft`'s assembled,
/// chunk-flattened observation list — share EXACTLY the same
/// validate/normalize/guarded-UPDATE/insert logic; finalize does not call the
/// public `stage_observations` as a black box (that path opens its own
/// transaction, ADR 0102 dec. 9). Draft cleanup at the end is UNCONDITIONAL:
/// `stage_observations` already refuses upfront when a live-epoch draft is
/// open (dec. 11), so the `DELETE` is a guaranteed no-op on that path and a
/// real cleanup on the draft-finalize path — one line, no extra parameter.
pub(super) fn install_observations_on_connection(
    tx: &Connection,
    run_id: &str,
    holder: &str,
    observations: Vec<NewStagedObservation>,
    missing_reasons: &std::collections::BTreeMap<String, String>,
    execution: Option<&serde_json::Value>,
) -> StorageResult<(i64, Vec<StagedObservation>)> {
    type RunPeriodRow = (
        String,
        Option<String>,
        Option<i64>,
        Option<String>,
        Option<String>,
        Option<String>,
    );
    let run: Option<RunPeriodRow> = tx
        .query_row(
            "SELECT status, period_id, period_fiscal_year, period_type, \
                    lease_holder, lease_expires_at
             FROM kpi_ingest_runs WHERE id = ?1",
            [run_id],
            |row| {
                Ok((
                    row.get(0)?,
                    row.get(1)?,
                    row.get(2)?,
                    row.get(3)?,
                    row.get(4)?,
                    row.get(5)?,
                ))
            },
        )
        .optional()?;
    let Some((status, period_id, period_fiscal_year, period_type, lease_holder, lease_expires_at)) =
        run
    else {
        return Err(StorageError::KpiIngestRunNotFound {
            id: run_id.to_owned(),
        });
    };
    let status = KpiIngestRunState::parse(&status)?;
    if !STAGEABLE_STATUSES.contains(&status) {
        return Err(StorageError::InvalidRunStateForStaging {
            id: run_id.to_owned(),
            status: status.as_str().to_owned(),
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
        // Three-way classification (#386, promised at #384): own-expired →
        // RunLeaseExpired, live-foreign → RunTakenOver, residual →
        // RunLeaseNotHeld.
        return Err(super::kpi_ingest_runs::lease_refusal_on_connection(
            tx, run_id, holder,
        )?);
    }
    if period_id.is_none() && (period_fiscal_year.is_none() || period_type.is_none()) {
        return Err(StorageError::InvalidKpiIngestRunValue {
            key: "period",
            value: "run has neither period_id nor a complete period descriptor".to_owned(),
        });
    }

    // Validate + normalize every observation BEFORE touching the run row or
    // inserting anything (typed refusal ahead of any raw CHECK).
    let mut normalized = Vec::with_capacity(observations.len());
    for observation in observations {
        if observation.raw_label.trim().is_empty() {
            return Err(StorageError::InvalidKpiIngestRunValue {
                key: "raw_label",
                value: "empty".to_owned(),
            });
        }
        if observation.raw_value.trim().is_empty() {
            return Err(StorageError::InvalidKpiIngestRunValue {
                key: "raw_value",
                value: "empty".to_owned(),
            });
        }
        let currency = normalize_currency(observation.currency.clone())?;
        validate_vocab("unit_scale", &observation.unit_scale, UNIT_SCALE_VALUES)?;
        validate_vocab(
            "measure_window",
            &observation.measure_window,
            MEASURE_WINDOW_VALUES,
        )?;
        validate_vocab("attribution", &observation.attribution, ATTRIBUTION_VALUES)?;
        validate_vocab("scope", &observation.scope, SCOPE_VALUES)?;
        let mapping_status = observation
            .mapping_status
            .clone()
            .unwrap_or_else(|| "unmapped".to_owned());
        if !MAPPING_STATUS_VALUES.contains(&mapping_status.as_str()) {
            return Err(StorageError::InvalidKpiIngestRunValue {
                key: "mapping_status",
                value: mapping_status,
            });
        }
        // Excluded is a sealed disposition (ADR 0102 dec. 1): it needs a
        // non-blank reason to seal, and a reason on any other disposition is
        // nonsense the caller must have mis-threaded — both are typed
        // refusals, not silently corrected.
        let exclusion_reason = empty_string_to_none(observation.exclusion_reason.clone());
        if mapping_status == "excluded" {
            if exclusion_reason
                .as_deref()
                .map(str::trim)
                .unwrap_or("")
                .is_empty()
            {
                return Err(StorageError::InvalidKpiIngestRunValue {
                    key: "exclusion_reason",
                    value: "required when mapping_status is excluded".to_owned(),
                });
            }
        } else if exclusion_reason.is_some() {
            return Err(StorageError::InvalidKpiIngestRunValue {
                key: "exclusion_reason",
                value: "must be absent unless mapping_status is excluded".to_owned(),
            });
        }
        if let Some(page) = observation.citation_page {
            if page < 1 {
                return Err(StorageError::InvalidKpiIngestRunValue {
                    key: "citation_page",
                    value: page.to_string(),
                });
            }
        }
        normalized.push((observation, currency, mapping_status, exclusion_reason));
    }

    // The final flip re-guards status AND the live lease (luna review B1):
    // the batch above can be long, and the lease is WALL-CLOCK state — it can
    // expire mid-transaction even though no other writer can touch the row
    // under this Immediate tx. An expired holder must not stage.
    #[cfg(test)]
    tests::mid_batch_test_delay();
    let new_revision: Option<i64> = tx
        .query_row(
            "UPDATE kpi_ingest_runs
             SET manifest_revision = manifest_revision + 1,
                 manifest_hash = NULL,
                 status = 'staged',
                 updated_at = strftime('%Y-%m-%dT%H:%M:%fZ', 'now')
             WHERE id = ?1
               AND status IN ('extracting', 'validation_failed')
               AND lease_holder = ?2
               AND lease_expires_at > strftime('%Y-%m-%dT%H:%M:%fZ', 'now')
             RETURNING manifest_revision",
            params![run_id, holder],
            |row| row.get(0),
        )
        .optional()?;
    let Some(new_revision) = new_revision else {
        // Mid-batch expiry or takeover: classify three-way too (#386).
        return Err(super::kpi_ingest_runs::lease_refusal_on_connection(
            tx, run_id, holder,
        )?);
    };

    // missingReasons travel in the SAME staging transaction (contracts.md
    // tool 5): the map is REQUIRED and `{}` is the explicit clear — every
    // revision replaces the whole declaration, never a destructive default.
    let missing_reasons_json = serde_json::to_string(missing_reasons)?;
    tx.execute(
        "UPDATE kpi_ingest_runs SET missing_reasons_json = ?1 WHERE id = ?2",
        params![missing_reasons_json, run_id],
    )?;
    if let Some(execution) = execution {
        super::kpi_ingest_runs::merge_cost_json_on_connection(tx, run_id, execution)?;
    }

    for (ordinal, (observation, currency, mapping_status, exclusion_reason)) in
        normalized.into_iter().enumerate()
    {
        let ordinal = ordinal as i64;
        let id = generate_observation_id(run_id, new_revision, ordinal);
        tx.execute(
            &format!(
                "INSERT INTO kpi_staged_observations
                    ({OBSERVATION_COLUMNS})
                 VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11, ?12, ?13, ?14, ?15, ?16, \
                    ?17, ?18, ?19, ?20, 'none', NULL, \
                    strftime('%Y-%m-%dT%H:%M:%fZ', 'now'), strftime('%Y-%m-%dT%H:%M:%fZ', 'now'), \
                    ?21)"
            ),
            params![
                id,
                run_id,
                new_revision,
                ordinal,
                observation.raw_label,
                observation.raw_value,
                observation.raw_currency,
                observation.raw_unit_scale,
                observation.normalized_value,
                currency,
                observation.unit_scale,
                observation.measure_window,
                observation.attribution,
                observation.scope,
                observation.metric_key_candidate,
                mapping_status,
                observation.citation_page,
                observation.citation_table,
                observation.citation_row,
                observation.citation_quote,
                exclusion_reason,
            ],
        )?;
    }

    let mut statement = tx.prepare(&format!(
        "SELECT {OBSERVATION_COLUMNS} FROM kpi_staged_observations
         WHERE run_id = ?1 AND revision = ?2 ORDER BY ordinal"
    ))?;
    let inserted: Vec<StagedObservation> = statement
        .query_map(params![run_id, new_revision], map_observation_row)?
        .collect::<Result<_, _>>()?;
    drop(statement);

    // Unconditional (see doc comment above): a real cleanup on the
    // draft-finalize route, a guaranteed no-op on the single-call route.
    tx.execute("DELETE FROM kpi_ingest_drafts WHERE run_id = ?1", [run_id])?;

    Ok((new_revision, inserted))
}

#[cfg(test)]
mod tests;
