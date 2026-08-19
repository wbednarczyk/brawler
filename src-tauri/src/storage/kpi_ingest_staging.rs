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
use std::str::FromStr;

use rust_decimal::Decimal;

use crate::fundamentals::kpi_manifest::{Outcome as ManifestOutcome, SealedManifest};

use super::database::Database;
use super::kpi_ingest_runs::{
    mark_ready_to_commit_on_connection, mark_validation_failed_on_connection, KpiIngestRunState,
};
use super::*;

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
const STAGEABLE_STATUSES: &[KpiIngestRunState] = &[
    KpiIngestRunState::Extracting,
    KpiIngestRunState::ValidationFailed,
];

/// One proposed observation to stage. The store assigns `id`, `revision` and
/// `ordinal` — never caller-supplied. `validation_state`/
/// `validation_codes_json` are ALWAYS `none`/`NULL` at staging time; only
/// [`KpiIngestStagingStore::apply_validation_outcome`] ever sets them.
#[derive(Debug, Clone, Default)]
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

fn map_observation_row(row: &rusqlite::Row<'_>) -> rusqlite::Result<StagedObservation> {
    Ok(StagedObservation {
        id: row.get(0)?,
        run_id: row.get(1)?,
        revision: row.get(2)?,
        ordinal: row.get(3)?,
        raw_label: row.get(4)?,
        raw_value: row.get(5)?,
        raw_currency: row.get(6)?,
        raw_unit_scale: row.get(7)?,
        normalized_value: row.get(8)?,
        currency: row.get(9)?,
        unit_scale: row.get(10)?,
        measure_window: row.get(11)?,
        attribution: row.get(12)?,
        scope: row.get(13)?,
        metric_key_candidate: row.get(14)?,
        mapping_status: row.get(15)?,
        citation_page: row.get(16)?,
        citation_table: row.get(17)?,
        citation_row: row.get(18)?,
        citation_quote: row.get(19)?,
        validation_state: row.get(20)?,
        validation_codes_json: row.get(21)?,
        created_at: row.get(22)?,
        updated_at: row.get(23)?,
        exclusion_reason: row.get(24)?,
    })
}

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

fn map_receipt_row(row: &rusqlite::Row<'_>) -> rusqlite::Result<CommitReceipt> {
    Ok(CommitReceipt {
        id: row.get(0)?,
        run_id: row.get(1)?,
        manifest_hash: row.get(2)?,
        manifest_revision: row.get(3)?,
        terminal_status: row.get(4)?,
        period_id: row.get(5)?,
        accepted_count: row.get(6)?,
        outcomes_schema_version: row.get(7)?,
        outcomes_json: row.get(8)?,
        committed_at: row.get(9)?,
    })
}

/// Mirrors `storage::financials::normalize_currency`, which is private to
/// that module (checked — not re-exported): trimmed, empty → absent,
/// otherwise exactly three ASCII letters upper-cased into the ISO-4217 shape.
/// Duplicated rather than made `pub(super)` in `financials.rs` because that
/// module treats it as an internal write-boundary helper for
/// `financial_facts`, a different table with its own error variant; keep
/// both in sync if the ISO-4217 shape rule ever changes.
fn normalize_currency(currency: Option<String>) -> StorageResult<Option<String>> {
    let Some(currency) = empty_string_to_none(currency.map(|s| s.trim().to_owned())) else {
        return Ok(None);
    };
    if currency.len() != 3 || !currency.chars().all(|c| c.is_ascii_alphabetic()) {
        return Err(StorageError::InvalidKpiIngestRunValue {
            key: "currency",
            value: currency,
        });
    }
    Ok(Some(currency.to_ascii_uppercase()))
}

fn validate_vocab(
    key: &'static str,
    value: &Option<String>,
    allowed: &[&str],
) -> StorageResult<()> {
    if let Some(value) = value.as_deref() {
        if !allowed.contains(&value) {
            return Err(StorageError::InvalidKpiIngestRunValue {
                key,
                value: value.to_owned(),
            });
        }
    }
    Ok(())
}

/// Collision-safe, non-deterministic id (the `generate_run_id` idiom,
/// `kpi_ingest_runs.rs`): `kpiobs_` + 32 hex chars of sha256 over the
/// identity plus a nanosecond time component.
fn generate_observation_id(run_id: &str, revision: i64, ordinal: i64) -> String {
    use sha2::{Digest, Sha256};
    let now_nanos = time::OffsetDateTime::now_utc().unix_timestamp_nanos();
    let key = format!("kpiobs:{run_id}\u{1f}{revision}\u{1f}{ordinal}\u{1f}{now_nanos}");
    let digest = Sha256::digest(key.as_bytes());
    let mut hex = String::with_capacity(32);
    for byte in &digest[..16] {
        hex.push_str(&format!("{byte:02x}"));
    }
    format!("kpiobs_{hex}")
}

/// Collision-safe id for one `kpi_ingest_validation_attempts` row (the
/// `generate_observation_id` idiom): `kpiatt_` + 32 hex chars of sha256 over
/// the identity plus a nanosecond time component. `attempt` is part of the
/// hashed key (not just `run_id`/`revision`) purely for extra collision
/// distance across the rare case of two attempts landing at the same
/// nanosecond — the `UNIQUE(run_id, revision, attempt)` constraint is the
/// real invariant guard, not this id's uniqueness.
fn generate_attempt_id(run_id: &str, revision: i64, attempt: i64) -> String {
    use sha2::{Digest, Sha256};
    let now_nanos = time::OffsetDateTime::now_utc().unix_timestamp_nanos();
    let key = format!("kpiatt:{run_id}\u{1f}{revision}\u{1f}{attempt}\u{1f}{now_nanos}");
    let digest = Sha256::digest(key.as_bytes());
    let mut hex = String::with_capacity(32);
    for byte in &digest[..16] {
        hex.push_str(&format!("{byte:02x}"));
    }
    format!("kpiatt_{hex}")
}

/// `normalized_value` equality for the content-tamper guard (finding 2):
/// the manifest side is Decimal-canonical (`fundamentals::kpi_manifest`'s
/// `canonical_normalized_value`) while the live row keeps the raw staged
/// string, so a byte comparison would false-positive on a coherent value
/// carrying trailing zeros (e.g. row `"1234.500"` vs projection `"1234.5"`).
/// Parse both sides and compare numerically; fall back to verbatim string
/// equality when either side does not parse (mirrors `value.unparseable`'s
/// own carry-verbatim rule).
fn normalized_values_match(row_value: &Option<String>, projection_value: &Option<String>) -> bool {
    match (row_value, projection_value) {
        (None, None) => true,
        (Some(a), Some(b)) => match (Decimal::from_str(a.trim()), Decimal::from_str(b.trim())) {
            (Ok(da), Ok(db)) => da == db,
            _ => a == b,
        },
        _ => false,
    }
}

/// The sol r5-2 content-tamper guard: whether a [`SealedManifest`]'s
/// canonical staged-content projection for one observation matches the LIVE
/// `kpi_staged_observations` row, dimension for dimension (effective scale/
/// attribution defaulted the SAME way `unit.scale_incoherent`'s `scale_eff`
/// and the `attribution_eff` slot key are, everything else compared raw).
fn content_matches_projection(
    row: &StagedObservation,
    projection: &crate::fundamentals::kpi_manifest::ObservationContentProjection,
) -> bool {
    row.ordinal == projection.ordinal
        && row.raw_label == projection.raw_label
        && (row.mapping_status == "excluded") == projection.excluded
        && row.exclusion_reason == projection.exclusion_reason
        && row.metric_key_candidate == projection.metric_key_candidate
        && normalized_values_match(&row.normalized_value, &projection.normalized_value)
        && row.currency == projection.currency
        && row.unit_scale.as_deref().unwrap_or("ones") == projection.unit_scale_eff
        && row.attribution.as_deref().unwrap_or("total") == projection.attribution_eff
        && row.measure_window == projection.measure_window_eff
        && row.scope == projection.scope
        && row.citation_page == projection.citation_page
        && row.citation_table == projection.citation_table
        && row.citation_row == projection.citation_row
        && row.citation_quote == projection.citation_quote
}

fn generate_receipt_id(run_id: &str) -> String {
    use sha2::{Digest, Sha256};
    let now_nanos = time::OffsetDateTime::now_utc().unix_timestamp_nanos();
    let key = format!("kpircpt:{run_id}\u{1f}{now_nanos}");
    let digest = Sha256::digest(key.as_bytes());
    let mut hex = String::with_capacity(32);
    for byte in &digest[..16] {
        hex.push_str(&format!("{byte:02x}"));
    }
    format!("kpircpt_{hex}")
}

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
        let Some((
            status,
            period_id,
            period_fiscal_year,
            period_type,
            lease_holder,
            lease_expires_at,
        )) = run
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
        let now: String =
            tx.query_row("SELECT strftime('%Y-%m-%dT%H:%M:%fZ', 'now')", [], |row| {
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
                &tx, run_id, holder,
            )?);
        }
        if period_id.is_none() && (period_fiscal_year.is_none() || period_type.is_none()) {
            return Err(StorageError::InvalidKpiIngestRunValue {
                key: "period",
                value: "run has neither period_id nor a complete period descriptor".to_owned(),
            });
        }

        // Validate + normalize every observation BEFORE touching the run row
        // or inserting anything (typed refusal ahead of any raw CHECK).
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
            // non-blank reason to seal, and a reason on any other
            // disposition is nonsense the caller must have mis-threaded —
            // both are typed refusals, not silently corrected.
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
        // the batch above can be long, and the lease is WALL-CLOCK state — it
        // can expire mid-transaction even though no other writer can touch the
        // row under this Immediate tx. An expired holder must not stage.
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
                &tx, run_id, holder,
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
            super::kpi_ingest_runs::merge_cost_json_on_connection(&tx, run_id, execution)?;
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

        tx.commit()?;
        Ok((new_revision, inserted))
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

/// Connection-level variant of [`KpiIngestStagingStore::get_commit_receipt`]
/// (#360 B3 sol): `finalize_committing` and `reclaim_ingest_runs_on_startup`
/// (`storage/kpi_ingest_runs.rs`) call this under their OWN externally-owned
/// transaction — the public method above checks out its own connection and
/// would deadlock under an outer `Immediate` write lock.
pub(super) fn get_commit_receipt_on_connection(
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
pub(super) fn record_commit_receipt(
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

#[cfg(test)]
mod tests {
    use super::*;
    use crate::storage::migrations::open_in_memory_database;
    use rusqlite::Connection;

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

    /// Seed a run directly at a given status with a complete period
    /// descriptor (staging's minimum period requirement) — bypasses
    /// `create_run_if_absent` so tests can start from any status. `scope`/
    /// `data_quality` are fixed at `consolidated`/`final` — the SAME values
    /// [`sealed_manifest_for`]/[`failed_sealed_manifest_for`] hardcode into
    /// their manifests' run context, so the #361 finding-1b run-context
    /// binding check (`apply_validation_outcome`) sees a consistent world by
    /// default.
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

    /// #360 back-fit: `stage_observations` now requires a live lease. Every
    /// test that stages against `setup()`'s run authenticates as this holder.
    const TEST_HOLDER: &str = "agent-1";

    fn one_observation() -> NewStagedObservation {
        NewStagedObservation {
            raw_label: "Przychody ze sprzedaży".to_owned(),
            raw_value: "1 234,5".to_owned(),
            currency: Some("pln".to_owned()),
            normalized_value: Some("1234.5".to_owned()),
            metric_key_candidate: Some("revenue".to_owned()),
            citation_page: Some(3),
            ..Default::default()
        }
    }

    thread_local! {
        /// Test-only hook: a delay injected between `stage_observations`'
        /// entry checks and its final guarded flip, so a test can cross the
        /// wall-clock lease expiry EXACTLY mid-batch (luna review B1 —
        /// sleeping before the call only exercises the entry check).
        static MID_BATCH_DELAY: std::cell::Cell<Option<std::time::Duration>> =
            const { std::cell::Cell::new(None) };
    }

    pub(super) fn mid_batch_test_delay() {
        if let Some(delay) = MID_BATCH_DELAY.with(|cell| cell.take()) {
            std::thread::sleep(delay);
        }
    }

    fn setup() -> (AppState, &'static str) {
        let connection = open_in_memory_database().expect("db");
        seed_company(&connection, "c1");
        seed_document(&connection, "doc1", "c1");
        seed_run(&connection, "run1", "doc1", "c1", "extracting");
        let state = AppState::new(connection);
        // The real claim seam (#360: "claim before staging") — establishes
        // the live lease `stage_observations` now requires.
        state
            .kpi_ingest_runs()
            .claim_next(TEST_HOLDER, 3600)
            .expect("claim")
            .expect("run1 must be claimable");
        (state, "run1")
    }

    // --- Test 1: stage ---------------------------------------------------

    #[test]
    fn stage_observations_first_snapshot_and_restage_after_validation_failure() {
        let (state, run_id) = setup();
        let store = state.kpi_ingest_staging();

        let (revision, observations) = store
            .stage_observations(
                run_id,
                TEST_HOLDER,
                vec![one_observation(), one_observation()],
                &std::collections::BTreeMap::new(),
                None,
            )
            .expect("first stage");
        assert_eq!(revision, 1);
        assert_eq!(observations.len(), 2);
        assert_eq!(observations[0].ordinal, 0);
        assert_eq!(observations[1].ordinal, 1);
        assert_eq!(observations[0].currency.as_deref(), Some("PLN"));

        let run = state
            .kpi_ingest_runs()
            .get_run(run_id)
            .expect("get")
            .expect("some");
        assert_eq!(run.status, KpiIngestRunState::Staged);
        assert!(run.manifest_hash.is_none());
        assert_eq!(run.manifest_revision, 1);

        // Restage requires validation_failed, not staged directly — #360's
        // real seam, replacing the raw status flip this test used before it
        // existed. The lease claimed in `setup()` is untouched by
        // `mark_validation_failed_on_connection` (no lease requirement on
        // that edge), so it stays live for the restage below.
        mark_validation_failed_on_connection(
            &state.checkout_for_tests().expect("checkout"),
            run_id,
            revision,
        )
        .expect("flip to validation_failed");

        let (revision2, observations2) = store
            .stage_observations(
                run_id,
                TEST_HOLDER,
                vec![one_observation()],
                &std::collections::BTreeMap::new(),
                None,
            )
            .expect("restage");
        assert_eq!(revision2, 2);
        assert_eq!(observations2.len(), 1);

        // Revision 1 is untouched (audit trail).
        let rev1 = store
            .list_staged_observations(run_id, Some(1))
            .expect("list rev1");
        assert_eq!(rev1.len(), 2);
    }

    #[test]
    fn stage_observations_rejects_non_stageable_statuses() {
        for status in [
            "discovered",
            "source_captured",
            "staged",
            "ready_to_commit",
            "committing",
            "complete",
            "partial",
            "failed",
            "cancelled",
        ] {
            let connection = open_in_memory_database().expect("db");
            seed_company(&connection, "c1");
            seed_document(&connection, "doc1", "c1");
            seed_run(&connection, "run1", "doc1", "c1", status);
            let state = AppState::new(connection);
            let store = state.kpi_ingest_staging();

            let error = store
                .stage_observations(
                    "run1",
                    TEST_HOLDER,
                    vec![one_observation()],
                    &std::collections::BTreeMap::new(),
                    None,
                )
                .expect_err(&format!("status '{status}' must be refused"));
            assert!(
                matches!(error, StorageError::InvalidRunStateForStaging { .. }),
                "status '{status}' produced {error:?}"
            );
        }
    }

    #[test]
    fn stage_observations_requires_a_period_identity() {
        let connection = open_in_memory_database().expect("db");
        seed_company(&connection, "c1");
        seed_document(&connection, "doc1", "c1");
        connection
            .execute(
                "INSERT INTO kpi_ingest_runs (id, report_document_id, company_id, profile_version, status)
                 VALUES ('run1', 'doc1', 'c1', 'p1', 'extracting')",
                [],
            )
            .expect("seed run without period");
        let state = AppState::new(connection);
        state
            .kpi_ingest_runs()
            .claim_next(TEST_HOLDER, 3600)
            .expect("claim")
            .expect("run1 must be claimable");
        let store = state.kpi_ingest_staging();

        let error = store
            .stage_observations(
                "run1",
                TEST_HOLDER,
                vec![one_observation()],
                &std::collections::BTreeMap::new(),
                None,
            )
            .expect_err("no period_id, no descriptor");
        assert!(matches!(
            error,
            StorageError::InvalidKpiIngestRunValue { key: "period", .. }
        ));
    }

    #[test]
    fn stage_observations_rejects_unknown_run_and_bad_vocab() {
        let (state, run_id) = setup();
        let store = state.kpi_ingest_staging();

        let error = store
            .stage_observations(
                "kpiing_missing",
                TEST_HOLDER,
                vec![one_observation()],
                &std::collections::BTreeMap::new(),
                None,
            )
            .expect_err("unknown run");
        assert!(matches!(error, StorageError::KpiIngestRunNotFound { .. }));

        let mut bad_currency = one_observation();
        bad_currency.currency = Some("dollars".to_owned());
        let error = store
            .stage_observations(
                run_id,
                TEST_HOLDER,
                vec![bad_currency],
                &std::collections::BTreeMap::new(),
                None,
            )
            .expect_err("bad currency");
        assert!(matches!(
            error,
            StorageError::InvalidKpiIngestRunValue {
                key: "currency",
                ..
            }
        ));

        // Nothing was inserted for the rejected batch.
        assert!(store
            .list_staged_observations(run_id, None)
            .expect("list")
            .is_empty());
    }

    #[test]
    fn excluded_requires_reason() {
        // ADR 0102 dec. 1: mappingStatus="excluded" is legal ONLY with a
        // non-blank exclusionReason; a reason on any other disposition is a
        // typed refusal too (never silently dropped).
        let (state, run_id) = setup();
        let store = state.kpi_ingest_staging();

        let mut missing_reason = one_observation();
        missing_reason.mapping_status = Some("excluded".to_owned());
        let error = store
            .stage_observations(
                run_id,
                TEST_HOLDER,
                vec![missing_reason],
                &std::collections::BTreeMap::new(),
                None,
            )
            .expect_err("excluded without a reason must refuse");
        assert!(matches!(
            error,
            StorageError::InvalidKpiIngestRunValue {
                key: "exclusion_reason",
                ..
            }
        ));

        let mut blank_reason = one_observation();
        blank_reason.mapping_status = Some("excluded".to_owned());
        blank_reason.exclusion_reason = Some("   ".to_owned());
        let error = store
            .stage_observations(
                run_id,
                TEST_HOLDER,
                vec![blank_reason],
                &std::collections::BTreeMap::new(),
                None,
            )
            .expect_err("blank exclusion reason must refuse");
        assert!(matches!(
            error,
            StorageError::InvalidKpiIngestRunValue {
                key: "exclusion_reason",
                ..
            }
        ));

        let mut reason_on_mapped = one_observation();
        reason_on_mapped.mapping_status = Some("mapped".to_owned());
        reason_on_mapped.exclusion_reason = Some("not applicable".to_owned());
        let error = store
            .stage_observations(
                run_id,
                TEST_HOLDER,
                vec![reason_on_mapped],
                &std::collections::BTreeMap::new(),
                None,
            )
            .expect_err("a reason on a non-excluded observation must refuse");
        assert!(matches!(
            error,
            StorageError::InvalidKpiIngestRunValue {
                key: "exclusion_reason",
                ..
            }
        ));

        assert!(store
            .list_staged_observations(run_id, None)
            .expect("list")
            .is_empty());

        // The legal shape: excluded with a real reason stages cleanly.
        let mut excluded = one_observation();
        excluded.mapping_status = Some("excluded".to_owned());
        excluded.exclusion_reason = Some("footnote disclosure, not a KPI".to_owned());
        let (_, rows) = store
            .stage_observations(
                run_id,
                TEST_HOLDER,
                vec![excluded],
                &std::collections::BTreeMap::new(),
                None,
            )
            .expect("excluded with a reason stages");
        assert_eq!(rows[0].mapping_status, "excluded");
        assert_eq!(
            rows[0].exclusion_reason.as_deref(),
            Some("footnote disclosure, not a KPI")
        );
    }

    #[test]
    fn stage_observations_numbers_ordinals_zero_based_and_contiguous() {
        let (state, run_id) = setup();
        let store = state.kpi_ingest_staging();
        let (_, observations) = store
            .stage_observations(
                run_id,
                TEST_HOLDER,
                vec![one_observation(), one_observation(), one_observation()],
                &std::collections::BTreeMap::new(),
                None,
            )
            .expect("stage three");
        let ordinals: Vec<i64> = observations.iter().map(|o| o.ordinal).collect();
        assert_eq!(ordinals, vec![0, 1, 2]);
    }

    // --- Test 2: list / latest_revision -----------------------------------

    #[test]
    fn list_and_latest_revision_default_to_the_newest() {
        let (state, run_id) = setup();
        let store = state.kpi_ingest_staging();
        assert_eq!(
            store.latest_staging_revision(run_id).expect("none yet"),
            None
        );

        let (revision1, _) = store
            .stage_observations(
                run_id,
                TEST_HOLDER,
                vec![one_observation()],
                &std::collections::BTreeMap::new(),
                None,
            )
            .expect("rev1");
        mark_validation_failed_on_connection(
            &state.checkout_for_tests().expect("checkout"),
            run_id,
            revision1,
        )
        .expect("flip");
        store
            .stage_observations(
                run_id,
                TEST_HOLDER,
                vec![one_observation(), one_observation()],
                &std::collections::BTreeMap::new(),
                None,
            )
            .expect("rev2");

        assert_eq!(
            store.latest_staging_revision(run_id).expect("latest"),
            Some(2)
        );
        assert_eq!(
            store
                .list_staged_observations(run_id, None)
                .expect("list latest")
                .len(),
            2
        );
        assert_eq!(
            store
                .list_staged_observations(run_id, Some(1))
                .expect("list rev1")
                .len(),
            1
        );
    }

    // --- Test 3: apply_validation_outcome (#361) ----------------------------

    /// `one_observation()` leaves `measure_window` unset (`period.window_missing`
    /// would always flag it) — these atom tests need a "clean" staged row
    /// that seals to a `passed`/`unreviewed` (never `flagged`) observation by
    /// default, so a happy-path "ready" scenario is actually reachable.
    fn clean_observation() -> NewStagedObservation {
        NewStagedObservation {
            measure_window: Some("flow".to_owned()),
            ..one_observation()
        }
    }

    /// Builds a [`SealedManifest`] whose observations are EXACTLY the given
    /// staged rows (same ids/ordinal/content — the atom's coverage/content
    /// guards require this), each carrying a resolved `revenue` definition
    /// with no history (an honest `plausibility.abstained` -> `unreviewed`,
    /// which never flips `outcome` — the atom tests below care about the
    /// COVERAGE/CONTENT/BINDING guards, not the rule engine already covered
    /// by `fundamentals::kpi_manifest::tests`).
    fn sealed_manifest_for(
        run_id: &str,
        revision: i64,
        rows: &[StagedObservation],
    ) -> SealedManifest {
        sealed_manifest_with_run(run_id, revision, rows, |_run| {})
    }

    /// [`sealed_manifest_for`], but `customize` may mutate the manifest's own
    /// run context (companyId, sourceContentHash, expectedKpis, ...) BEFORE
    /// it is sealed — the #361 finding-1b run-context binding tests need a
    /// manifest that disagrees with the live run row on exactly one field.
    fn sealed_manifest_with_run(
        run_id: &str,
        revision: i64,
        rows: &[StagedObservation],
        customize: impl FnOnce(&mut crate::fundamentals::kpi_manifest::ManifestRunInput),
    ) -> SealedManifest {
        use crate::fundamentals::kpi_manifest::{
            build_manifest, ManifestObservationInput, ManifestRunInput, MissingReasons,
            ResolvedDefinitionInput, SlotHistoryInput,
        };
        let mut run = ManifestRunInput {
            run_id: run_id.to_owned(),
            revision,
            company_id: "c1".to_owned(),
            report_document_id: "doc1".to_owned(),
            source_content_hash: None,
            scope: "consolidated".to_owned(),
            data_quality: "final".to_owned(),
            period_id: None,
            fiscal_year: 2025,
            period_type: "FY".to_owned(),
            missing_reasons: MissingReasons::None,
            expected: None,
        };
        customize(&mut run);
        let observations: Vec<ManifestObservationInput> = rows
            .iter()
            .map(|row| ManifestObservationInput {
                observation_id: row.id.clone(),
                ordinal: row.ordinal,
                raw_label: row.raw_label.clone(),
                raw_value: row.raw_value.clone(),
                metric_key_candidate: row.metric_key_candidate.clone(),
                mapping_status: "mapped".to_owned(),
                exclusion_reason: None,
                normalized_value: row.normalized_value.clone(),
                currency: row.currency.clone(),
                unit_scale: row.unit_scale.clone(),
                measure_window: row.measure_window.clone(),
                attribution: row.attribution.clone(),
                scope: row.scope.clone(),
                citation_page: row.citation_page,
                citation_table: row.citation_table.clone(),
                citation_row: row.citation_row.clone(),
                citation_quote: row.citation_quote.clone(),
                // `definition_id` is distinct per ordinal -- two staged rows
                // built from the same `clean_observation()` fixture would
                // otherwise share one slot (definition_id, attribution_eff,
                // measure_window_eff) and trip `duplicate.repeat`, which
                // these tests don't want to reason about. `metric_key`
                // mirrors the row's own candidate (the real resolver always
                // echoes back the exact key it matched on) -- a synthetic
                // per-ordinal metric_key here would fake a mapping the
                // resolver could never produce and desync `present`'s
                // completeness accounting (finding 1a) from the candidate
                // this same content projection binds to the live row.
                definition: Some(ResolvedDefinitionInput {
                    definition_id: format!("kpidef_test_metric_{}", row.ordinal),
                    metric_key: row.metric_key_candidate.clone().unwrap_or_default(),
                    value_kind: "monetary".to_owned(),
                    period_nature: "duration".to_owned(),
                    history: SlotHistoryInput::default(),
                }),
            })
            .collect();
        SealedManifest::seal(build_manifest(&run, &observations))
            .expect("consistent manifest seals")
    }

    /// Same as [`sealed_manifest_for`], but the given `row.id`s are flagged
    /// (no resolved definition -> `mapping.unresolved`) so the sealed
    /// manifest's derived `outcome` is `failed`.
    fn failed_sealed_manifest_for(
        run_id: &str,
        revision: i64,
        rows: &[StagedObservation],
    ) -> SealedManifest {
        use crate::fundamentals::kpi_manifest::{
            build_manifest, ManifestObservationInput, ManifestRunInput, MissingReasons,
        };
        let run = ManifestRunInput {
            run_id: run_id.to_owned(),
            revision,
            company_id: "c1".to_owned(),
            report_document_id: "doc1".to_owned(),
            source_content_hash: None,
            scope: "consolidated".to_owned(),
            data_quality: "final".to_owned(),
            period_id: None,
            fiscal_year: 2025,
            period_type: "FY".to_owned(),
            missing_reasons: MissingReasons::None,
            expected: None,
        };
        let observations: Vec<ManifestObservationInput> = rows
            .iter()
            .map(|row| ManifestObservationInput {
                observation_id: row.id.clone(),
                ordinal: row.ordinal,
                raw_label: row.raw_label.clone(),
                raw_value: row.raw_value.clone(),
                metric_key_candidate: row.metric_key_candidate.clone(),
                mapping_status: "unmapped".to_owned(),
                exclusion_reason: None,
                normalized_value: row.normalized_value.clone(),
                currency: row.currency.clone(),
                unit_scale: row.unit_scale.clone(),
                measure_window: row.measure_window.clone(),
                attribution: row.attribution.clone(),
                scope: row.scope.clone(),
                citation_page: row.citation_page,
                citation_table: row.citation_table.clone(),
                citation_row: row.citation_row.clone(),
                citation_quote: row.citation_quote.clone(),
                definition: None,
            })
            .collect();
        SealedManifest::seal(build_manifest(&run, &observations))
            .expect("consistent manifest seals")
    }

    /// [`sealed_manifest_for`], but every row is sealed `excluded` with
    /// `reason` — used by the ADR 0102 tamper tests, which build a manifest
    /// claiming a disposition/reason the LIVE staged row does not actually
    /// carry.
    fn excluded_sealed_manifest_for(
        run_id: &str,
        revision: i64,
        rows: &[StagedObservation],
        reason: &str,
    ) -> SealedManifest {
        use crate::fundamentals::kpi_manifest::{
            build_manifest, ManifestObservationInput, ManifestRunInput, MissingReasons,
        };
        let run = ManifestRunInput {
            run_id: run_id.to_owned(),
            revision,
            company_id: "c1".to_owned(),
            report_document_id: "doc1".to_owned(),
            source_content_hash: None,
            scope: "consolidated".to_owned(),
            data_quality: "final".to_owned(),
            period_id: None,
            fiscal_year: 2025,
            period_type: "FY".to_owned(),
            missing_reasons: MissingReasons::None,
            expected: None,
        };
        let observations: Vec<ManifestObservationInput> = rows
            .iter()
            .map(|row| ManifestObservationInput {
                observation_id: row.id.clone(),
                ordinal: row.ordinal,
                raw_label: row.raw_label.clone(),
                raw_value: row.raw_value.clone(),
                metric_key_candidate: row.metric_key_candidate.clone(),
                mapping_status: "excluded".to_owned(),
                exclusion_reason: Some(reason.to_owned()),
                normalized_value: row.normalized_value.clone(),
                currency: row.currency.clone(),
                unit_scale: row.unit_scale.clone(),
                measure_window: row.measure_window.clone(),
                attribution: row.attribution.clone(),
                scope: row.scope.clone(),
                citation_page: row.citation_page,
                citation_table: row.citation_table.clone(),
                citation_row: row.citation_row.clone(),
                citation_quote: row.citation_quote.clone(),
                definition: None,
            })
            .collect();
        SealedManifest::seal(build_manifest(&run, &observations))
            .expect("consistent manifest seals")
    }

    #[test]
    fn apply_validation_outcome_happy_path_ready_in_one_transaction() {
        let (state, run_id) = setup();
        let store = state.kpi_ingest_staging();
        let (revision, rows) = store
            .stage_observations(
                run_id,
                TEST_HOLDER,
                vec![clean_observation(), clean_observation()],
                &std::collections::BTreeMap::new(),
                None,
            )
            .expect("stage");
        let sealed = sealed_manifest_for(run_id, revision, &rows);
        let manifest_hash = sealed.manifest_hash().to_owned();

        store
            .apply_validation_outcome(run_id, revision, sealed)
            .expect("apply");

        let after = store
            .list_staged_observations(run_id, Some(revision))
            .expect("list");
        assert!(after.iter().all(|o| o.validation_state == "unreviewed"));
        assert!(after.iter().all(|o| o.validation_codes_json.is_some()));

        let run = state
            .kpi_ingest_runs()
            .get_run(run_id)
            .expect("get")
            .expect("some");
        assert_eq!(run.status, KpiIngestRunState::ReadyToCommit);
        assert_eq!(run.manifest_hash.as_deref(), Some(manifest_hash.as_str()));

        let attempts = state
            .kpi_ingest_runs()
            .list_validation_attempts(run_id)
            .expect("attempts");
        assert_eq!(attempts.len(), 1);
        assert_eq!(attempts[0].outcome, "ready");
        assert_eq!(attempts[0].attempt, 1);
        assert_eq!(attempts[0].manifest_hash, manifest_hash);
    }

    #[test]
    fn apply_validation_outcome_failed_in_one_transaction_leaves_run_hash_null() {
        let (state, run_id) = setup();
        let store = state.kpi_ingest_staging();
        let (revision, rows) = store
            .stage_observations(
                run_id,
                TEST_HOLDER,
                vec![clean_observation()],
                &std::collections::BTreeMap::new(),
                None,
            )
            .expect("stage");
        let sealed = failed_sealed_manifest_for(run_id, revision, &rows);
        let manifest_hash = sealed.manifest_hash().to_owned();

        store
            .apply_validation_outcome(run_id, revision, sealed)
            .expect("apply");

        let after = store
            .list_staged_observations(run_id, Some(revision))
            .expect("list");
        assert_eq!(after[0].validation_state, "flagged");

        let run = state
            .kpi_ingest_runs()
            .get_run(run_id)
            .expect("get")
            .expect("some");
        assert_eq!(run.status, KpiIngestRunState::ValidationFailed);
        assert!(
            run.manifest_hash.is_none(),
            "failed never sets run.manifest_hash"
        );

        let attempts = state
            .kpi_ingest_runs()
            .list_validation_attempts(run_id)
            .expect("attempts");
        assert_eq!(attempts.len(), 1);
        assert_eq!(attempts[0].outcome, "failed");
        assert_eq!(attempts[0].manifest_hash, manifest_hash);
        assert!(attempts[0].manifest_json.contains("mapping.unresolved"));
    }

    #[test]
    fn apply_validation_outcome_refuses_a_non_staged_run() {
        let (state, run_id) = setup();
        let store = state.kpi_ingest_staging();
        let (revision, rows) = store
            .stage_observations(
                run_id,
                TEST_HOLDER,
                vec![clean_observation()],
                &std::collections::BTreeMap::new(),
                None,
            )
            .expect("stage");
        state
            .kpi_ingest_runs()
            .cancel_run(run_id)
            .expect("cancel from staged is legal");
        let sealed = sealed_manifest_for(run_id, revision, &rows);
        let error = store
            .apply_validation_outcome(run_id, revision, sealed)
            .expect_err("validating a cancelled run must refuse (status != staged)");
        assert!(matches!(error, StorageError::InvalidStagingRevision { .. }));
    }

    /// The lease is wall-clock state: it can expire DURING a long staging
    /// batch even though no other writer can touch the row under the Immediate
    /// transaction. The final flip re-guards the live lease (luna review B1).
    #[test]
    fn stage_observations_refuses_when_the_lease_expires_mid_batch() {
        let connection = open_in_memory_database().expect("db");
        seed_company(&connection, "c1");
        seed_document(&connection, "doc1", "c1");
        seed_run(&connection, "run1", "doc1", "c1", "extracting");
        let state = AppState::new(connection);
        state
            .kpi_ingest_runs()
            .claim_next(TEST_HOLDER, 1)
            .expect("claim")
            .expect("claimed");
        // The lease is LIVE at the entry check; the injected delay crosses
        // the 1-second expiry between that check and the final guarded flip —
        // the genuine mid-batch scenario (luna review B1: the pre-call sleep
        // variant only exercised the entry check, and the originally broken
        // implementation would have passed it).
        MID_BATCH_DELAY.with(|cell| cell.set(Some(std::time::Duration::from_millis(1200))));
        let error = state
            .kpi_ingest_staging()
            .stage_observations(
                "run1",
                TEST_HOLDER,
                vec![one_observation()],
                &std::collections::BTreeMap::new(),
                None,
            )
            .expect_err("an expired lease must not stage");
        // #386: the mid-batch refusal classifies through the shared three-way
        // vocabulary — the holder's OWN lease expired, so the typed remedy is
        // `run_lease_expired` (re-claim via start), not the residual shape.
        assert!(matches!(error, StorageError::RunLeaseExpired { .. }));
        let run = state
            .kpi_ingest_runs()
            .get_run("run1")
            .expect("get")
            .expect("some");
        assert_eq!(run.status, KpiIngestRunState::Extracting);
        assert_eq!(run.manifest_revision, 0, "no revision bump on refusal");
    }

    #[test]
    fn apply_validation_outcome_refuses_stale_or_frozen_revision() {
        let (state, run_id) = setup();
        let store = state.kpi_ingest_staging();
        let (revision, rows) = store
            .stage_observations(
                run_id,
                TEST_HOLDER,
                vec![clean_observation()],
                &std::collections::BTreeMap::new(),
                None,
            )
            .expect("stage");

        let sealed = sealed_manifest_for(run_id, revision + 1, &rows);
        let error = store
            .apply_validation_outcome(run_id, revision + 1, sealed)
            .expect_err("stale revision");
        assert!(matches!(error, StorageError::InvalidStagingRevision { .. }));

        // Freeze the revision (a manifest was issued). Structurally
        // unreachable through any production seam while `status` stays
        // `staged` (the atom's own transition to `ready_to_commit` moves
        // status off `staged` in the SAME update that sets the hash) —
        // raw-seeded defensively to probe this method's OWN freeze guard.
        state
            .checkout_for_tests()
            .expect("raw")
            .execute(
                "UPDATE kpi_ingest_runs SET manifest_hash = 'deadbeef' WHERE id = ?1",
                [run_id],
            )
            .expect("freeze");
        let sealed = sealed_manifest_for(run_id, revision, &rows);
        let error = store
            .apply_validation_outcome(run_id, revision, sealed)
            .expect_err("frozen revision");
        assert!(matches!(error, StorageError::InvalidStagingRevision { .. }));
    }

    #[test]
    fn apply_validation_outcome_refuses_wrong_run_id_and_revision_binding() {
        let (state, run_id) = setup();
        let store = state.kpi_ingest_staging();
        let (revision, rows) = store
            .stage_observations(
                run_id,
                TEST_HOLDER,
                vec![clean_observation()],
                &std::collections::BTreeMap::new(),
                None,
            )
            .expect("stage");

        // Sealed for a DIFFERENT run id -- binding mismatch, distinct from
        // the coverage/revision guards.
        let foreign_run_sealed = sealed_manifest_for("kpiing_other", revision, &rows);
        let error = store
            .apply_validation_outcome(run_id, revision, foreign_run_sealed)
            .expect_err("foreign run id must be refused");
        assert!(matches!(error, StorageError::SealedManifestRejected { .. }));

        // Sealed for the RIGHT run but a revision the manifest itself claims
        // is different from what the caller passed.
        let wrong_revision_sealed = sealed_manifest_for(run_id, revision + 41, &rows);
        let error = store
            .apply_validation_outcome(run_id, revision, wrong_revision_sealed)
            .expect_err("mismatched revision binding must be refused");
        assert!(matches!(error, StorageError::SealedManifestRejected { .. }));

        // Zero writes from either refusal.
        let after = store
            .list_staged_observations(run_id, Some(revision))
            .expect("list");
        assert_eq!(after[0].validation_state, "none");
        assert!(state
            .kpi_ingest_runs()
            .list_validation_attempts(run_id)
            .expect("attempts")
            .is_empty());
    }

    #[test]
    fn apply_validation_outcome_refuses_run_context_mismatches_with_zero_writes() {
        let (state, run_id) = setup();
        let store = state.kpi_ingest_staging();
        let (revision, rows) = store
            .stage_observations(
                run_id,
                TEST_HOLDER,
                vec![clean_observation()],
                &std::collections::BTreeMap::new(),
                None,
            )
            .expect("stage");

        let assert_refused = |sealed: SealedManifest| {
            let error = store
                .apply_validation_outcome(run_id, revision, sealed)
                .expect_err("run-context mismatch must be refused");
            assert!(matches!(error, StorageError::SealedManifestRejected { .. }));
        };

        // wrong companyId
        assert_refused(sealed_manifest_with_run(run_id, revision, &rows, |run| {
            run.company_id = "someone_else".to_owned();
        }));
        // wrong sourceContentHash (live run's is NULL; manifest claims one)
        assert_refused(sealed_manifest_with_run(run_id, revision, &rows, |run| {
            run.source_content_hash = Some("different-hash".to_owned());
        }));
        // wrong expectedKpis (live run's expected_kpis_json is NULL; manifest
        // claims a stamped snapshot)
        assert_refused(sealed_manifest_with_run(run_id, revision, &rows, |run| {
            run.expected = Some(crate::fundamentals::kpi_manifest::ExpectedSnapshot {
                schema_version: 1,
                source: "kpi_relevance".to_owned(),
                pack_version: None,
                keys: ["revenue".to_owned()].into_iter().collect(),
            });
        }));

        let after = store
            .list_staged_observations(run_id, Some(revision))
            .expect("list");
        assert_eq!(
            after[0].validation_state, "none",
            "every context-mismatch refusal above must write nothing"
        );
        assert!(state
            .kpi_ingest_runs()
            .list_validation_attempts(run_id)
            .expect("attempts")
            .is_empty());
    }

    #[test]
    fn apply_validation_outcome_refuses_missing_or_extra_observation_coverage() {
        let (state, run_id) = setup();
        let store = state.kpi_ingest_staging();
        let (revision, rows) = store
            .stage_observations(
                run_id,
                TEST_HOLDER,
                vec![clean_observation(), clean_observation()],
                &std::collections::BTreeMap::new(),
                None,
            )
            .expect("stage");

        // Missing one of the two staged observations.
        let partial = sealed_manifest_for(run_id, revision, &rows[..1]);
        let error = store
            .apply_validation_outcome(run_id, revision, partial)
            .expect_err("a manifest missing a staged observation must be refused");
        assert!(matches!(error, StorageError::SealedManifestRejected { .. }));

        let after = store
            .list_staged_observations(run_id, Some(revision))
            .expect("list");
        assert!(
            after.iter().all(|o| o.validation_state == "none"),
            "a coverage refusal must write nothing"
        );
        assert!(state
            .kpi_ingest_runs()
            .list_validation_attempts(run_id)
            .expect("attempts")
            .is_empty());
    }

    #[test]
    fn apply_validation_outcome_refuses_content_tamper_with_zero_writes() {
        let (state, run_id) = setup();
        let store = state.kpi_ingest_staging();
        let (revision, rows) = store
            .stage_observations(
                run_id,
                TEST_HOLDER,
                vec![clean_observation()],
                &std::collections::BTreeMap::new(),
                None,
            )
            .expect("stage");

        // Same observation id, but the manifest's own staged-content
        // projection was built from a TAMPERED value that no longer matches
        // the live row.
        let mut tampered_row = rows[0].clone();
        tampered_row.normalized_value = Some("999999".to_owned());
        let sealed = sealed_manifest_for(run_id, revision, std::slice::from_ref(&tampered_row));

        let error = store
            .apply_validation_outcome(run_id, revision, sealed)
            .expect_err(
                "a manifest whose content projection disagrees with the live row must refuse",
            );
        assert!(matches!(error, StorageError::SealedManifestRejected { .. }));

        let after = store
            .list_staged_observations(run_id, Some(revision))
            .expect("list");
        assert_eq!(
            after[0].validation_state, "none",
            "tamper refusal writes nothing"
        );
        assert_eq!(
            after[0].normalized_value.as_deref(),
            Some("1234.5"),
            "the LIVE row's value must be untouched"
        );
        assert!(state
            .kpi_ingest_runs()
            .list_validation_attempts(run_id)
            .expect("attempts")
            .is_empty());
    }

    #[test]
    fn status_flipped_to_excluded_after_validation_is_refused() {
        // ADR 0102 dec. 1/3: a manifest claiming a row is `excluded` when the
        // LIVE staged row was never actually staged that way must be refused
        // by the content-projection compare — zero writes, exactly like any
        // other content tamper.
        let (state, run_id) = setup();
        let store = state.kpi_ingest_staging();
        let (revision, rows) = store
            .stage_observations(
                run_id,
                TEST_HOLDER,
                vec![clean_observation()],
                &std::collections::BTreeMap::new(),
                None,
            )
            .expect("stage");
        // The live row's mapping_status stays whatever `clean_observation()`
        // defaults to (never "excluded") -- the manifest claims otherwise.
        let sealed = excluded_sealed_manifest_for(run_id, revision, &rows, "not applicable");

        let error = store
            .apply_validation_outcome(run_id, revision, sealed)
            .expect_err("a manifest claiming excluded over a non-excluded live row must refuse");
        assert!(matches!(error, StorageError::SealedManifestRejected { .. }));

        let after = store
            .list_staged_observations(run_id, Some(revision))
            .expect("list");
        assert_eq!(after[0].validation_state, "none", "tamper writes nothing");
        assert_ne!(after[0].mapping_status, "excluded");
        assert!(state
            .kpi_ingest_runs()
            .list_validation_attempts(run_id)
            .expect("attempts")
            .is_empty());
    }

    #[test]
    fn reason_changed_after_validation_is_refused() {
        // ADR 0102 dec. 1/3: the live row IS excluded, but with a DIFFERENT
        // reason than the manifest claims -- content-projection still
        // refuses (the reason is part of the sealed disposition, dec. 1).
        let (state, run_id) = setup();
        let store = state.kpi_ingest_staging();
        let mut excluded_obs = clean_observation();
        excluded_obs.mapping_status = Some("excluded".to_owned());
        excluded_obs.exclusion_reason = Some("original reason".to_owned());
        let (revision, rows) = store
            .stage_observations(
                run_id,
                TEST_HOLDER,
                vec![excluded_obs],
                &std::collections::BTreeMap::new(),
                None,
            )
            .expect("stage");
        assert_eq!(rows[0].exclusion_reason.as_deref(), Some("original reason"));
        let sealed = excluded_sealed_manifest_for(run_id, revision, &rows, "a different reason");

        let error = store
            .apply_validation_outcome(run_id, revision, sealed)
            .expect_err("a manifest with a changed exclusion reason must refuse");
        assert!(matches!(error, StorageError::SealedManifestRejected { .. }));

        let after = store
            .list_staged_observations(run_id, Some(revision))
            .expect("list");
        assert_eq!(after[0].validation_state, "none", "tamper writes nothing");
        assert_eq!(
            after[0].exclusion_reason.as_deref(),
            Some("original reason"),
            "the LIVE row's reason must be untouched"
        );
        assert!(state
            .kpi_ingest_runs()
            .list_validation_attempts(run_id)
            .expect("attempts")
            .is_empty());
    }

    #[test]
    fn apply_validation_outcome_codes_on_rows_equal_codes_in_manifest() {
        let (state, run_id) = setup();
        let store = state.kpi_ingest_staging();
        let (revision, rows) = store
            .stage_observations(
                run_id,
                TEST_HOLDER,
                vec![clean_observation()],
                &std::collections::BTreeMap::new(),
                None,
            )
            .expect("stage");
        let sealed = sealed_manifest_for(run_id, revision, &rows);
        let expected_codes_json = sealed.observation_verdicts()[0]
            .validation_codes_json
            .clone();

        store
            .apply_validation_outcome(run_id, revision, sealed)
            .expect("apply");

        let after = store
            .list_staged_observations(run_id, Some(revision))
            .expect("list");
        assert_eq!(
            after[0].validation_codes_json.as_deref(),
            Some(expected_codes_json.as_str()),
            "the row's stored codes must be byte-identical to the sealed manifest's"
        );
    }

    #[test]
    fn apply_validation_outcome_attempt_survives_restage_and_invalidate_bumps_attempt() {
        let (state, run_id) = setup();
        let store = state.kpi_ingest_staging();
        let (revision, rows) = store
            .stage_observations(
                run_id,
                TEST_HOLDER,
                vec![clean_observation()],
                &std::collections::BTreeMap::new(),
                None,
            )
            .expect("stage");

        // First attempt: fails.
        let failed = failed_sealed_manifest_for(run_id, revision, &rows);
        store
            .apply_validation_outcome(run_id, revision, failed)
            .expect("apply failed");
        let attempts = state
            .kpi_ingest_runs()
            .list_validation_attempts(run_id)
            .expect("attempts");
        assert_eq!(attempts.len(), 1);
        assert_eq!(attempts[0].attempt, 1);
        assert_eq!(attempts[0].outcome, "failed");

        // Restage the SAME revision's repair (validation_failed -> staged is
        // legal, #360) and validate again -- attempt 2 for revision 2.
        let (revision2, rows2) = store
            .stage_observations(
                run_id,
                TEST_HOLDER,
                vec![clean_observation()],
                &std::collections::BTreeMap::new(),
                None,
            )
            .expect("restage");
        let sealed2 = sealed_manifest_for(run_id, revision2, &rows2);
        store
            .apply_validation_outcome(run_id, revision2, sealed2)
            .expect("apply ready");

        // Invalidate the manifest and re-validate the SAME revision -- the
        // UNIQUE(run_id, revision, attempt) constraint means this must land
        // as attempt 2 for revision2, not collide with attempt 1.
        state
            .kpi_ingest_runs()
            .invalidate_manifest(run_id)
            .expect("invalidate");
        let rows2_current = store
            .list_staged_observations(run_id, Some(revision2))
            .expect("list");
        let sealed2_again = sealed_manifest_for(run_id, revision2, &rows2_current);
        store
            .apply_validation_outcome(run_id, revision2, sealed2_again)
            .expect("re-apply after invalidate");

        let attempts = state
            .kpi_ingest_runs()
            .list_validation_attempts(run_id)
            .expect("attempts");
        assert_eq!(
            attempts.len(),
            3,
            "revision1 attempt 1 + revision2 attempts 1 and 2"
        );
        let rev2_attempts: Vec<_> = attempts
            .iter()
            .filter(|a| a.revision == revision2)
            .collect();
        assert_eq!(rev2_attempts.len(), 2);
        assert_eq!(rev2_attempts[0].attempt, 1);
        assert_eq!(rev2_attempts[1].attempt, 2);

        // The revision-1 failed attempt's diagnostics are still readable
        // (audit survives the re-stage/invalidate cycle).
        let rev1_attempt = attempts
            .iter()
            .find(|a| a.revision == revision)
            .expect("rev1 attempt");
        assert_eq!(rev1_attempt.outcome, "failed");
        assert!(rev1_attempt.manifest_json.contains("mapping.unresolved"));
    }

    // --- Test 4: record_commit_receipt ------------------------------------

    #[test]
    fn record_commit_receipt_in_an_external_transaction_and_replay() {
        let (state, run_id) = setup();
        let new_receipt = || NewCommitReceipt {
            run_id: run_id.to_owned(),
            manifest_hash: "hash1".to_owned(),
            manifest_revision: 1,
            terminal_status: "complete".to_owned(),
            period_id: None,
            accepted_count: 3,
            outcomes_schema_version: 1,
            outcomes_json: "[{\"observationId\":\"kpiobs_1\",\"outcome\":\"created\"}]".to_owned(),
        };

        let receipt = {
            let mut connection = state.checkout_for_tests().expect("raw connection");
            let tx = connection
                .transaction_with_behavior(rusqlite::TransactionBehavior::Immediate)
                .expect("tx");
            let receipt = record_commit_receipt(&tx, new_receipt()).expect("record");
            tx.commit().expect("commit");
            receipt
        };
        assert_eq!(receipt.run_id, run_id);
        assert_eq!(receipt.accepted_count, 3);
        assert_eq!(receipt.outcomes_schema_version, 1);

        let error = {
            let mut connection = state.checkout_for_tests().expect("raw connection");
            let tx = connection
                .transaction_with_behavior(rusqlite::TransactionBehavior::Immediate)
                .expect("tx");
            let error = record_commit_receipt(&tx, new_receipt()).expect_err("second insert");
            tx.commit().expect("commit");
            error
        };
        assert!(matches!(
            error,
            StorageError::CommitReceiptAlreadyRecorded { .. }
        ));

        let fetched = state
            .kpi_ingest_staging()
            .get_commit_receipt(run_id)
            .expect("get")
            .expect("some");
        assert_eq!(fetched.id, receipt.id);
        assert_eq!(fetched.outcomes_json, receipt.outcomes_json);
    }

    /// Only the `UNIQUE(run_id)` violation is a replay; every other
    /// constraint (bad status vocab, missing run FK) must surface as its own
    /// storage error, never as `CommitReceiptAlreadyRecorded` (luna review P1).
    #[test]
    fn record_commit_receipt_maps_only_run_uniqueness_to_already_recorded() {
        let (state, run_id) = setup();
        let base = |run: &str| NewCommitReceipt {
            run_id: run.to_owned(),
            manifest_hash: "hash1".to_owned(),
            manifest_revision: 1,
            terminal_status: "complete".to_owned(),
            period_id: None,
            accepted_count: 1,
            outcomes_schema_version: 1,
            outcomes_json: "[]".to_owned(),
        };

        // Bad terminal_status trips the CHECK — a constraint violation that is
        // NOT a replay.
        let mut connection = state.checkout_for_tests().expect("raw connection");
        let tx = connection
            .transaction_with_behavior(rusqlite::TransactionBehavior::Immediate)
            .expect("tx");
        let mut bad_status = base(run_id);
        bad_status.terminal_status = "half-done".to_owned();
        let error = record_commit_receipt(&tx, bad_status).expect_err("bad status");
        assert!(
            !matches!(error, StorageError::CommitReceiptAlreadyRecorded { .. }),
            "a CHECK violation must not masquerade as a replay: {error:?}"
        );

        // A receipt for a nonexistent run trips the FK — also not a replay.
        let error = record_commit_receipt(&tx, base("kpiing_missing")).expect_err("missing run");
        assert!(
            !matches!(error, StorageError::CommitReceiptAlreadyRecorded { .. }),
            "an FK violation must not masquerade as a replay: {error:?}"
        );
        drop(tx);
    }

    #[test]
    fn record_commit_receipt_rolls_back_with_its_external_transaction() {
        let (state, run_id) = setup();
        {
            let mut connection = state.checkout_for_tests().expect("raw connection");
            let tx = connection
                .transaction_with_behavior(rusqlite::TransactionBehavior::Immediate)
                .expect("tx");
            record_commit_receipt(
                &tx,
                NewCommitReceipt {
                    run_id: run_id.to_owned(),
                    manifest_hash: "hash1".to_owned(),
                    manifest_revision: 1,
                    terminal_status: "complete".to_owned(),
                    period_id: None,
                    accepted_count: 1,
                    outcomes_schema_version: 1,
                    outcomes_json: "[]".to_owned(),
                },
            )
            .expect("record");
            // Deliberately dropped without commit -> rollback.
        }

        assert!(state
            .kpi_ingest_staging()
            .get_commit_receipt(run_id)
            .expect("get")
            .is_none());
    }

    // --- Test 6: CHECK constraints ------------------------------------------

    #[test]
    fn check_constraints_reject_bad_rows() {
        let (state, run_id) = setup();
        let connection = state.checkout_for_tests().expect("raw connection");

        assert!(
            connection
                .execute(
                    "INSERT INTO kpi_staged_observations (id, run_id, revision, ordinal, raw_label, raw_value)
                     VALUES ('bad1', ?1, 0, 0, 'l', 'v')",
                    [run_id],
                )
                .is_err(),
            "revision 0 must be rejected"
        );
        assert!(
            connection
                .execute(
                    "INSERT INTO kpi_staged_observations (id, run_id, revision, ordinal, raw_label, raw_value)
                     VALUES ('bad2', ?1, 1, -1, 'l', 'v')",
                    [run_id],
                )
                .is_err(),
            "negative ordinal must be rejected"
        );
        connection
            .execute(
                "INSERT INTO kpi_staged_observations (id, run_id, revision, ordinal, raw_label, raw_value)
                 VALUES ('ok1', ?1, 1, 0, 'l', 'v')",
                [run_id],
            )
            .expect("first row at (run, 1, 0) must succeed");
        assert!(
            connection
                .execute(
                    "INSERT INTO kpi_staged_observations (id, run_id, revision, ordinal, raw_label, raw_value)
                     VALUES ('dup', ?1, 1, 0, 'l', 'v')",
                    [run_id],
                )
                .is_err(),
            "UNIQUE(run_id, revision, ordinal) must reject a duplicate"
        );

        // A second commit receipt for the same run is rejected by UNIQUE(run_id).
        connection
            .execute(
                "INSERT INTO kpi_ingest_commit_receipts
                    (id, run_id, manifest_hash, manifest_revision, terminal_status, accepted_count, outcomes_json)
                 VALUES ('r1', ?1, 'h1', 1, 'complete', 0, '[]')",
                [run_id],
            )
            .expect("first receipt must succeed");
        assert!(
            connection
                .execute(
                    "INSERT INTO kpi_ingest_commit_receipts
                        (id, run_id, manifest_hash, manifest_revision, terminal_status, accepted_count, outcomes_json)
                     VALUES ('r2', ?1, 'h2', 1, 'complete', 0, '[]')",
                    [run_id],
                )
                .is_err(),
            "a second receipt for the same run must be rejected"
        );
    }

    /// Migration 0139's CHECK constraints, exercised directly against raw
    /// SQL (#361 test group 10: "0139 w inwentarzu, tabela utworzona, CHECKi
    /// działają" -- the inventory guard is `migrations::tests::
    /// every_migration_file_is_registered_and_vice_versa`).
    #[test]
    fn validation_attempts_check_constraints_reject_bad_rows() {
        let (state, run_id) = setup();
        let connection = state.checkout_for_tests().expect("raw connection");
        let insert = |id: &str, revision: i64, attempt: i64, outcome: &str| {
            connection.execute(
                "INSERT INTO kpi_ingest_validation_attempts
                    (id, run_id, revision, attempt, outcome, manifest_hash, manifest_json)
                 VALUES (?1, ?2, ?3, ?4, ?5, 'h', '{}')",
                params![id, run_id, revision, attempt, outcome],
            )
        };

        assert!(
            insert("bad1", 0, 1, "ready").is_err(),
            "revision 0 must be rejected"
        );
        assert!(
            insert("bad2", 1, 0, "ready").is_err(),
            "attempt 0 must be rejected"
        );
        assert!(
            insert("bad3", 1, 1, "half-done").is_err(),
            "an outcome outside ('ready', 'failed') must be rejected"
        );
        insert("ok1", 1, 1, "ready").expect("(run, 1, 1) must succeed");
        assert!(
            insert("dup", 1, 1, "failed").is_err(),
            "UNIQUE(run_id, revision, attempt) must reject a duplicate"
        );
        insert("ok2", 1, 2, "failed").expect("a second attempt for the same revision must succeed");
    }

    // --- Test 7: revision consistency ---------------------------------------

    #[test]
    fn stage_observations_bumps_revision_and_zeroes_manifest_hash() {
        let (state, run_id) = setup();
        let store = state.kpi_ingest_staging();
        let (revision, _) = store
            .stage_observations(
                run_id,
                TEST_HOLDER,
                vec![one_observation()],
                &std::collections::BTreeMap::new(),
                None,
            )
            .expect("stage");

        // Simulate an issued manifest while still `staged` — structurally
        // unreachable through any production seam (same carve-out as
        // `apply_validation_outcome_refuses_stale_or_frozen_revision`), so
        // raw-seeded; the SUBSEQUENT status flip to `validation_failed`,
        // however, has a real #360/#361 seam now and no longer needs raw SQL.
        state
            .checkout_for_tests()
            .expect("raw")
            .execute(
                "UPDATE kpi_ingest_runs SET manifest_hash = 'deadbeef' WHERE id = ?1",
                [run_id],
            )
            .expect("simulate an issued manifest");
        mark_validation_failed_on_connection(
            &state.checkout_for_tests().expect("checkout"),
            run_id,
            revision,
        )
        .expect("flip");

        store
            .stage_observations(
                run_id,
                TEST_HOLDER,
                vec![one_observation()],
                &std::collections::BTreeMap::new(),
                None,
            )
            .expect("restage");
        let run = state
            .kpi_ingest_runs()
            .get_run(run_id)
            .expect("get")
            .expect("some");
        assert!(
            run.manifest_hash.is_none(),
            "a new staging snapshot must zero out the prior manifest_hash"
        );
        assert_eq!(run.manifest_revision, 2);
    }

    // --- Test 8: two racing stagers -----------------------------------------

    /// Two threads racing `stage_observations` on the SAME run: the new
    /// status guard means exactly ONE winner (the run leaves `extracting`
    /// for `staged`), the other gets a typed `InvalidRunStateForStaging` —
    /// never two revisions racing in. Needs a FILE-backed pool (the
    /// `claim_next_two_threads_exactly_one_winner` idiom,
    /// `kpi_ingest_runs.rs`) — the in-memory single-connection path can never
    /// exercise a genuine SQLite-level race.
    #[test]
    fn stage_observations_two_threads_exactly_one_winner() {
        use r2d2_sqlite::SqliteConnectionManager;
        use std::sync::Arc;

        let db_path = std::env::temp_dir().join(format!(
            "brawler-kpi-staging-race-{}-{}.sqlite3",
            std::process::id(),
            time::OffsetDateTime::now_utc().unix_timestamp_nanos()
        ));
        {
            let mut connection = Connection::open(&db_path).expect("open file db");
            crate::storage::migrations::apply_migrations(&mut connection).expect("migrate");
            seed_company(&connection, "c1");
            seed_document(&connection, "doc1", "c1");
            seed_run(&connection, "run1", "doc1", "c1", "extracting");
        }
        let manager = SqliteConnectionManager::file(&db_path).with_init(|connection| {
            connection.execute_batch("PRAGMA journal_mode = WAL; PRAGMA foreign_keys = ON;")?;
            connection.pragma_update(None, "busy_timeout", 5000i64)?;
            Ok(())
        });
        let pool = r2d2::Pool::builder()
            .max_size(4)
            .build(manager)
            .expect("build pool");
        let runs_store = KpiIngestRunsStore::new(Database::from_pool(pool.clone()));
        runs_store
            .claim_next(TEST_HOLDER, 3600)
            .expect("claim")
            .expect("run1 must be claimable");
        let store = Arc::new(KpiIngestStagingStore::new(Database::from_pool(pool)));

        // Barrier so both contenders genuinely start together (luna review:
        // without it the threads may serialize before either call begins).
        let barrier = Arc::new(std::sync::Barrier::new(2));
        let store_a = store.clone();
        let store_b = store.clone();
        let barrier_a = barrier.clone();
        let barrier_b = barrier;
        let a = std::thread::spawn(move || {
            barrier_a.wait();
            store_a.stage_observations(
                "run1",
                TEST_HOLDER,
                vec![one_observation()],
                &std::collections::BTreeMap::new(),
                None,
            )
        });
        let b = std::thread::spawn(move || {
            barrier_b.wait();
            store_b.stage_observations(
                "run1",
                TEST_HOLDER,
                vec![one_observation()],
                &std::collections::BTreeMap::new(),
                None,
            )
        });
        let result_a = a.join().expect("thread a");
        let result_b = b.join().expect("thread b");

        let winners = [&result_a, &result_b]
            .iter()
            .filter(|result| result.is_ok())
            .count();
        assert_eq!(
            winners, 1,
            "exactly one thread must win the single stageable run"
        );
        let losers = [&result_a, &result_b]
            .iter()
            .filter(|result| result.is_err())
            .count();
        assert_eq!(losers, 1);
        for result in [&result_a, &result_b] {
            if let Err(error) = result {
                assert!(matches!(
                    error,
                    StorageError::InvalidRunStateForStaging { .. }
                ));
            }
        }

        let _ = std::fs::remove_file(&db_path);
        let _ = std::fs::remove_file(db_path.with_extension("sqlite3-wal"));
        let _ = std::fs::remove_file(db_path.with_extension("sqlite3-shm"));
    }

    // --- Test 9: empty snapshot, raw round-trip -----------------------------

    #[test]
    fn stage_observations_rejects_an_empty_snapshot() {
        let (state, run_id) = setup();
        let store = state.kpi_ingest_staging();
        let error = store
            .stage_observations(
                run_id,
                TEST_HOLDER,
                vec![],
                &std::collections::BTreeMap::new(),
                None,
            )
            .expect_err("empty batch");
        assert!(matches!(
            error,
            StorageError::InvalidKpiIngestRunValue {
                key: "observations",
                ..
            }
        ));
    }

    #[test]
    fn stage_observations_round_trips_raw_currency_and_unit_scale() {
        let (state, run_id) = setup();
        let store = state.kpi_ingest_staging();
        let mut observation = one_observation();
        observation.raw_currency = Some("PLN".to_owned());
        observation.raw_unit_scale = Some("tys. zł".to_owned());
        let (_, observations) = store
            .stage_observations(
                run_id,
                TEST_HOLDER,
                vec![observation],
                &std::collections::BTreeMap::new(),
                None,
            )
            .expect("stage");
        assert_eq!(observations[0].raw_currency.as_deref(), Some("PLN"));
        assert_eq!(observations[0].raw_unit_scale.as_deref(), Some("tys. zł"));
    }

    // --- #386: missingReasons + execution travel in the staging tx --------

    #[test]
    fn staging_writes_missing_reasons_with_replace_semantics() {
        let (state, run_id) = setup();
        let reasons: std::collections::BTreeMap<String, String> =
            [("net_profit".to_owned(), "not disclosed".to_owned())].into();
        state
            .kpi_ingest_staging()
            .stage_observations(run_id, TEST_HOLDER, vec![one_observation()], &reasons, None)
            .expect("stage");
        let run = state
            .kpi_ingest_runs()
            .get_run(run_id)
            .expect("get")
            .expect("run");
        assert_eq!(
            run.missing_reasons_json.as_deref(),
            Some(r#"{"net_profit":"not disclosed"}"#),
            "the declaration lands in the SAME staging transaction"
        );

        // A repair revision with `{}` is the explicit clear — replace, never
        // a merge and never a destructive default.
        state.kpi_ingest_runs().invalidate_manifest(run_id).ok();
        let connection = state.checkout_for_tests().expect("raw");
        connection
            .execute(
                "UPDATE kpi_ingest_runs SET status = 'validation_failed' WHERE id = ?1",
                [run_id],
            )
            .expect("force repairable state");
        drop(connection);
        state
            .kpi_ingest_staging()
            .stage_observations(
                run_id,
                TEST_HOLDER,
                vec![one_observation()],
                &std::collections::BTreeMap::new(),
                None,
            )
            .expect("re-stage");
        let run = state
            .kpi_ingest_runs()
            .get_run(run_id)
            .expect("get")
            .expect("run");
        assert_eq!(
            run.missing_reasons_json.as_deref(),
            Some("{}"),
            "an empty map clears the previous declaration"
        );
    }

    #[test]
    fn staging_merges_execution_into_cost_json_and_corrupt_stored_json_is_replaced() {
        let (state, run_id) = setup();
        // Corrupt pre-existing cost_json: diagnostic, so the merge replaces
        // it fresh instead of failing the stage.
        {
            let connection = state.checkout_for_tests().expect("raw");
            connection
                .execute(
                    "UPDATE kpi_ingest_runs SET cost_json = 'not json' WHERE id = ?1",
                    [run_id],
                )
                .expect("corrupt");
        }
        let execution = serde_json::json!({ "client": "test-agent", "tokensIn": 5 });
        state
            .kpi_ingest_staging()
            .stage_observations(
                run_id,
                TEST_HOLDER,
                vec![one_observation()],
                &std::collections::BTreeMap::new(),
                Some(&execution),
            )
            .expect("stage");
        let run = state
            .kpi_ingest_runs()
            .get_run(run_id)
            .expect("get")
            .expect("run");
        let cost: serde_json::Value =
            serde_json::from_str(run.cost_json.as_deref().expect("cost_json written"))
                .expect("valid json");
        assert_eq!(cost["schemaVersion"], 1);
        assert_eq!(cost["client"], "test-agent");
        assert_eq!(cost["tokensIn"], 5);
    }
}
