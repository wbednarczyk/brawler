//! KPI staging domain store (ADR 0098 decisions 3, 4, 5; epic #352, card
//! #359). `kpi_staged_observations` holds run-owned LLM proposals — NEVER
//! visible to any fact reader (dec. 3); a proposal becomes a canonical fact
//! only through the commit transaction (#363). `kpi_ingest_commit_receipts`
//! is the immutable per-run commit outcome, append-only (the `valuation_runs`
//! idiom — zero UPDATE path here). Reach the observation surface via
//! `AppState::kpi_ingest_staging()`; `record_commit_receipt` is a
//! connection-level free fn for #363's commit transaction to call under its
//! own `&Connection` (the `record_structured_fact` pattern,
//! `kpi_extraction.rs:359`).

use std::collections::HashSet;

use super::database::Database;
use super::kpi_ingest_runs::KpiIngestRunState;
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
const MAPPING_STATUS_VALUES: &[&str] = &["unmapped", "mapped", "no_definition"];
/// Reused from `financial_fact_provenance.validation_status` (migration
/// 0057).
const VALIDATION_STATE_VALUES: &[&str] = &["none", "passed", "unreviewed", "flagged"];
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
/// [`KpiIngestStagingStore::apply_validation_results`] ever sets them.
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
     citation_row, citation_quote, validation_state, validation_codes_json, created_at, updated_at";

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
    })
}

/// One observation's validation verdict for
/// [`KpiIngestStagingStore::apply_validation_results`].
#[derive(Debug, Clone)]
pub struct ObservationValidation {
    pub observation_id: String,
    pub validation_state: String,
    pub validation_codes_json: Option<String>,
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

// ponytail: unwired until #363's commit transaction calls `record_commit_receipt`
// under its own externally-owned transaction (ADR 0098 dec. 5); #359 ships
// the primitive plus its round-trip/idempotency/rollback tests per the
// approved plan, so it is exercised only from #[cfg(test)] until then.
#[allow(dead_code)]
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
    /// `{extracting, validation_failed}` (`InvalidRunStateForStaging`), and a
    /// run with neither `period_id` nor a complete period descriptor.
    pub fn stage_observations(
        &self,
        run_id: &str,
        observations: Vec<NewStagedObservation>,
    ) -> StorageResult<(i64, Vec<StagedObservation>)> {
        if observations.is_empty() {
            return Err(StorageError::InvalidKpiIngestRunValue {
                key: "observations",
                value: "empty".to_owned(),
            });
        }

        let mut connection = self.db.checkout()?;
        let tx = connection.transaction_with_behavior(rusqlite::TransactionBehavior::Immediate)?;

        type RunPeriodRow = (String, Option<String>, Option<i64>, Option<String>);
        let run: Option<RunPeriodRow> = tx
            .query_row(
                "SELECT status, period_id, period_fiscal_year, period_type
                 FROM kpi_ingest_runs WHERE id = ?1",
                [run_id],
                |row| Ok((row.get(0)?, row.get(1)?, row.get(2)?, row.get(3)?)),
            )
            .optional()?;
        let Some((status, period_id, period_fiscal_year, period_type)) = run else {
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
            if let Some(page) = observation.citation_page {
                if page < 1 {
                    return Err(StorageError::InvalidKpiIngestRunValue {
                        key: "citation_page",
                        value: page.to_string(),
                    });
                }
            }
            normalized.push((observation, currency, mapping_status));
        }

        let new_revision: i64 = tx.query_row(
            "UPDATE kpi_ingest_runs
             SET manifest_revision = manifest_revision + 1,
                 manifest_hash = NULL,
                 status = 'staged',
                 updated_at = strftime('%Y-%m-%dT%H:%M:%fZ', 'now')
             WHERE id = ?1
             RETURNING manifest_revision",
            [run_id],
            |row| row.get(0),
        )?;

        for (ordinal, (observation, currency, mapping_status)) in normalized.into_iter().enumerate()
        {
            let ordinal = ordinal as i64;
            let id = generate_observation_id(run_id, new_revision, ordinal);
            tx.execute(
                &format!(
                    "INSERT INTO kpi_staged_observations
                        ({OBSERVATION_COLUMNS})
                     VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11, ?12, ?13, ?14, ?15, ?16, \
                        ?17, ?18, ?19, ?20, 'none', NULL, \
                        strftime('%Y-%m-%dT%H:%M:%fZ', 'now'), strftime('%Y-%m-%dT%H:%M:%fZ', 'now'))"
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

    /// Apply a batch of validation verdicts to ONE staging revision in a
    /// SINGLE transaction (M6 sol review — never a partially-validated
    /// revision). Refuses when `revision` is not the run's current
    /// `manifest_revision` or the run's `manifest_hash` is already set (the
    /// revision is frozen once a manifest is issued) — both are
    /// `InvalidStagingRevision`. An unknown or duplicated `observation_id` in
    /// the batch is a typed refusal; nothing in the batch is applied on any
    /// failure (the transaction rolls back).
    pub fn apply_validation_results(
        &self,
        run_id: &str,
        revision: i64,
        results: Vec<ObservationValidation>,
    ) -> StorageResult<()> {
        let mut seen = HashSet::with_capacity(results.len());
        for result in &results {
            validate_vocab(
                "validation_state",
                &Some(result.validation_state.clone()),
                VALIDATION_STATE_VALUES,
            )?;
            if !seen.insert(result.observation_id.clone()) {
                return Err(StorageError::InvalidKpiIngestRunValue {
                    key: "observation_id",
                    value: format!("duplicate in batch: {}", result.observation_id),
                });
            }
        }

        let mut connection = self.db.checkout()?;
        let tx = connection.transaction_with_behavior(rusqlite::TransactionBehavior::Immediate)?;

        let run: Option<(i64, Option<String>)> = tx
            .query_row(
                "SELECT manifest_revision, manifest_hash FROM kpi_ingest_runs WHERE id = ?1",
                [run_id],
                |row| Ok((row.get(0)?, row.get(1)?)),
            )
            .optional()?;
        let Some((manifest_revision, manifest_hash)) = run else {
            return Err(StorageError::KpiIngestRunNotFound {
                id: run_id.to_owned(),
            });
        };
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

        for result in results {
            let changed = tx.execute(
                "UPDATE kpi_staged_observations
                 SET validation_state = ?1,
                     validation_codes_json = ?2,
                     updated_at = strftime('%Y-%m-%dT%H:%M:%fZ', 'now')
                 WHERE id = ?3 AND run_id = ?4 AND revision = ?5",
                params![
                    result.validation_state,
                    result.validation_codes_json,
                    result.observation_id,
                    run_id,
                    revision
                ],
            )?;
            if changed == 0 {
                return Err(StorageError::StagedObservationNotFound {
                    id: result.observation_id,
                });
            }
        }

        tx.commit()?;
        Ok(())
    }

    pub fn get_commit_receipt(&self, run_id: &str) -> StorageResult<Option<CommitReceipt>> {
        let connection = self.db.checkout()?;
        connection
            .query_row(
                &format!(
                    "SELECT {RECEIPT_COLUMNS} FROM kpi_ingest_commit_receipts WHERE run_id = ?1"
                ),
                [run_id],
                map_receipt_row,
            )
            .optional()
            .map_err(StorageError::from)
    }
}

/// Connection-level free fn (the `record_structured_fact` pattern,
/// `kpi_extraction.rs:359`): #363's commit transaction calls this under its
/// own externally-owned `&Connection`/transaction — this fn never opens one
/// itself. A second insert for the same `run_id` maps the `UNIQUE(run_id)`
/// violation to the typed `CommitReceiptAlreadyRecorded` (ADR 0098 dec. 5:
/// idempotent replay must return the stored receipt, never re-execute the
/// commit primitives — that is #363's job once it sees this error/the
/// existing row).
// ponytail: same #363-unwired situation as `generate_receipt_id` above.
#[allow(dead_code)]
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
        Err(rusqlite::Error::SqliteFailure(err, _))
            if err.code == rusqlite::ErrorCode::ConstraintViolation =>
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
    /// `create_run_if_absent` so tests can start from any status.
    fn seed_run(connection: &Connection, id: &str, doc: &str, company: &str, status: &str) {
        connection
            .execute(
                "INSERT INTO kpi_ingest_runs
                    (id, report_document_id, company_id, profile_version, status,
                     period_fiscal_year, period_type)
                 VALUES (?1, ?2, ?3, 'p1', ?4, 2025, 'FY')",
                params![id, doc, company, status],
            )
            .expect("seed run");
    }

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

    fn setup() -> (AppState, &'static str) {
        let connection = open_in_memory_database().expect("db");
        seed_company(&connection, "c1");
        seed_document(&connection, "doc1", "c1");
        seed_run(&connection, "run1", "doc1", "c1", "extracting");
        (AppState::new(connection), "run1")
    }

    // --- Test 1: stage ---------------------------------------------------

    #[test]
    fn stage_observations_first_snapshot_and_restage_after_validation_failure() {
        let (state, run_id) = setup();
        let store = state.kpi_ingest_staging();

        let (revision, observations) = store
            .stage_observations(run_id, vec![one_observation(), one_observation()])
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

        // Restage requires validation_failed, not staged directly — flip it
        // the way #360's transition table eventually will (raw update here,
        // no production seam exists yet).
        state
            .checkout_for_tests()
            .expect("raw connection")
            .execute(
                "UPDATE kpi_ingest_runs SET status = 'validation_failed' WHERE id = ?1",
                [run_id],
            )
            .expect("flip to validation_failed");

        let (revision2, observations2) = store
            .stage_observations(run_id, vec![one_observation()])
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
                .stage_observations("run1", vec![one_observation()])
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
        let store = state.kpi_ingest_staging();

        let error = store
            .stage_observations("run1", vec![one_observation()])
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
            .stage_observations("kpiing_missing", vec![one_observation()])
            .expect_err("unknown run");
        assert!(matches!(error, StorageError::KpiIngestRunNotFound { .. }));

        let mut bad_currency = one_observation();
        bad_currency.currency = Some("dollars".to_owned());
        let error = store
            .stage_observations(run_id, vec![bad_currency])
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
    fn stage_observations_numbers_ordinals_zero_based_and_contiguous() {
        let (state, run_id) = setup();
        let store = state.kpi_ingest_staging();
        let (_, observations) = store
            .stage_observations(
                run_id,
                vec![one_observation(), one_observation(), one_observation()],
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

        store
            .stage_observations(run_id, vec![one_observation()])
            .expect("rev1");
        state
            .checkout_for_tests()
            .expect("raw")
            .execute(
                "UPDATE kpi_ingest_runs SET status = 'validation_failed' WHERE id = ?1",
                [run_id],
            )
            .expect("flip");
        store
            .stage_observations(run_id, vec![one_observation(), one_observation()])
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

    // --- Test 3: apply_validation_results ---------------------------------

    #[test]
    fn apply_validation_results_happy_path_in_one_transaction() {
        let (state, run_id) = setup();
        let store = state.kpi_ingest_staging();
        let (revision, observations) = store
            .stage_observations(run_id, vec![one_observation(), one_observation()])
            .expect("stage");

        store
            .apply_validation_results(
                run_id,
                revision,
                vec![
                    ObservationValidation {
                        observation_id: observations[0].id.clone(),
                        validation_state: "passed".to_owned(),
                        validation_codes_json: None,
                    },
                    ObservationValidation {
                        observation_id: observations[1].id.clone(),
                        validation_state: "flagged".to_owned(),
                        validation_codes_json: Some("[\"code_1\"]".to_owned()),
                    },
                ],
            )
            .expect("apply");

        let after = store
            .list_staged_observations(run_id, Some(revision))
            .expect("list");
        assert_eq!(after[0].validation_state, "passed");
        assert_eq!(after[1].validation_state, "flagged");
        assert_eq!(
            after[1].validation_codes_json.as_deref(),
            Some("[\"code_1\"]")
        );
    }

    #[test]
    fn apply_validation_results_refuses_stale_or_frozen_revision() {
        let (state, run_id) = setup();
        let store = state.kpi_ingest_staging();
        let (revision, observations) = store
            .stage_observations(run_id, vec![one_observation()])
            .expect("stage");

        let error = store
            .apply_validation_results(
                run_id,
                revision + 1,
                vec![ObservationValidation {
                    observation_id: observations[0].id.clone(),
                    validation_state: "passed".to_owned(),
                    validation_codes_json: None,
                }],
            )
            .expect_err("stale revision");
        assert!(matches!(error, StorageError::InvalidStagingRevision { .. }));

        // Freeze the revision (a manifest was issued).
        state
            .checkout_for_tests()
            .expect("raw")
            .execute(
                "UPDATE kpi_ingest_runs SET manifest_hash = 'deadbeef' WHERE id = ?1",
                [run_id],
            )
            .expect("freeze");
        let error = store
            .apply_validation_results(
                run_id,
                revision,
                vec![ObservationValidation {
                    observation_id: observations[0].id.clone(),
                    validation_state: "passed".to_owned(),
                    validation_codes_json: None,
                }],
            )
            .expect_err("frozen revision");
        assert!(matches!(error, StorageError::InvalidStagingRevision { .. }));
    }

    #[test]
    fn apply_validation_results_rejects_unknown_id_bad_vocab_and_batch_duplicate() {
        let (state, run_id) = setup();
        let store = state.kpi_ingest_staging();
        let (revision, observations) = store
            .stage_observations(run_id, vec![one_observation()])
            .expect("stage");
        let obs_id = observations[0].id.clone();

        let error = store
            .apply_validation_results(
                run_id,
                revision,
                vec![ObservationValidation {
                    observation_id: "kpiobs_missing".to_owned(),
                    validation_state: "passed".to_owned(),
                    validation_codes_json: None,
                }],
            )
            .expect_err("unknown id");
        assert!(matches!(
            error,
            StorageError::StagedObservationNotFound { .. }
        ));

        let error = store
            .apply_validation_results(
                run_id,
                revision,
                vec![ObservationValidation {
                    observation_id: obs_id.clone(),
                    validation_state: "not_a_state".to_owned(),
                    validation_codes_json: None,
                }],
            )
            .expect_err("bad vocab");
        assert!(matches!(
            error,
            StorageError::InvalidKpiIngestRunValue {
                key: "validation_state",
                ..
            }
        ));

        let error = store
            .apply_validation_results(
                run_id,
                revision,
                vec![
                    ObservationValidation {
                        observation_id: obs_id.clone(),
                        validation_state: "passed".to_owned(),
                        validation_codes_json: None,
                    },
                    ObservationValidation {
                        observation_id: obs_id.clone(),
                        validation_state: "flagged".to_owned(),
                        validation_codes_json: None,
                    },
                ],
            )
            .expect_err("duplicate observationId in batch");
        assert!(matches!(
            error,
            StorageError::InvalidKpiIngestRunValue {
                key: "observation_id",
                ..
            }
        ));

        // Nothing from the rejected batch landed.
        let after = store
            .list_staged_observations(run_id, Some(revision))
            .expect("list");
        assert_eq!(after[0].validation_state, "none");
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

    // --- Test 7: revision consistency ---------------------------------------

    #[test]
    fn stage_observations_bumps_revision_and_zeroes_manifest_hash() {
        let (state, run_id) = setup();
        let store = state.kpi_ingest_staging();
        store
            .stage_observations(run_id, vec![one_observation()])
            .expect("stage");

        state
            .checkout_for_tests()
            .expect("raw")
            .execute(
                "UPDATE kpi_ingest_runs SET manifest_hash = 'deadbeef' WHERE id = ?1",
                [run_id],
            )
            .expect("simulate an issued manifest");
        state
            .checkout_for_tests()
            .expect("raw")
            .execute(
                "UPDATE kpi_ingest_runs SET status = 'validation_failed' WHERE id = ?1",
                [run_id],
            )
            .expect("flip");

        store
            .stage_observations(run_id, vec![one_observation()])
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
        let store = Arc::new(KpiIngestStagingStore::new(Database::from_pool(pool)));

        let store_a = store.clone();
        let store_b = store.clone();
        let a =
            std::thread::spawn(move || store_a.stage_observations("run1", vec![one_observation()]));
        let b =
            std::thread::spawn(move || store_b.stage_observations("run1", vec![one_observation()]));
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
            .stage_observations(run_id, vec![])
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
            .stage_observations(run_id, vec![observation])
            .expect("stage");
        assert_eq!(observations[0].raw_currency.as_deref(), Some("PLN"));
        assert_eq!(observations[0].raw_unit_scale.as_deref(), Some("tys. zł"));
    }
}
