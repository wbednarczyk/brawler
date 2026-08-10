//! KPI ingest run domain store (ADR 0098 decisions 2, 6, 8; epic #352, card
//! #358). `kpi_ingest_runs` is the external agent's durable worklist and
//! lease/heartbeat holder (dec. 8, model A): one row per (report document,
//! company, extraction profile). The closed 11-state lifecycle (dec. 6) is
//! enforced by the DB `CHECK` and the [`KpiIngestRunState`] enum; the typed
//! transition table (repair, cancellation, crash recovery) lands in #360 — this
//! module ships zero production status-mutation beyond create/lease/capture.
//! Reach it via `AppState::kpi_ingest_runs()`.

use super::database::Database;
use super::*;

/// Which lifecycle states an external agent may claim (dec. 8 / B2 sol review):
/// `staged`/`ready_to_commit` wait on deterministic validation/commit, not the
/// agent worklist; `committing` and every terminal state never re-enter it.
const CLAIMABLE_STATUSES_SQL: &str =
    "('discovered','source_captured','extracting','validation_failed')";

/// Terminal states exempt from the active-triple uniqueness gate (a content
/// change under the same URL may start a fresh run for the same triple).
const TERMINAL_STATUSES_SQL: &str = "('complete','partial','failed','cancelled')";

const RUN_COLUMNS: &str = "id, report_document_id, company_id, period_id, source_content_hash, \
     scope, data_quality, profile_version, instruction_version, status, manifest_hash, \
     manifest_revision, attempt_count, lease_holder, lease_expires_at, last_heartbeat_at, \
     expected_kpis_json, missing_reasons_json, progress_json, cost_json, last_error, \
     created_at, updated_at, period_fiscal_year, period_type";

/// `financial_periods.period_type` vocabulary (data-model.md § financial
///_periods, `storage/financials.rs:525`): FY, H1, H2, Q1-Q4, 9M, M01-M12.
/// The run's period descriptor (migration 0138) validates against the SAME
/// list — it is a natural key mirroring `financial_periods` and must stay in
/// lockstep with a period row when both are present.
const PERIOD_TYPE_VALUES: &[&str] = &[
    "FY", "H1", "H2", "Q1", "Q2", "Q3", "Q4", "9M", "M01", "M02", "M03", "M04", "M05", "M06",
    "M07", "M08", "M09", "M10", "M11", "M12",
];

/// The closed run lifecycle (ADR 0098 dec. 6). `as_str`/`parse` idiom from
/// `SourceTier`, except `parse` returns a typed [`StorageError`] on an unknown
/// stored token rather than silently degrading — a run's status is trust-load-
/// bearing (claimability, terminality), never a best-effort read.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum KpiIngestRunState {
    Discovered,
    SourceCaptured,
    Extracting,
    Staged,
    ValidationFailed,
    ReadyToCommit,
    Committing,
    Complete,
    Partial,
    Failed,
    Cancelled,
}

impl KpiIngestRunState {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Discovered => "discovered",
            Self::SourceCaptured => "source_captured",
            Self::Extracting => "extracting",
            Self::Staged => "staged",
            Self::ValidationFailed => "validation_failed",
            Self::ReadyToCommit => "ready_to_commit",
            Self::Committing => "committing",
            Self::Complete => "complete",
            Self::Partial => "partial",
            Self::Failed => "failed",
            Self::Cancelled => "cancelled",
        }
    }

    /// `Err(UnknownKpiIngestRunState)` on a token this closed lifecycle does not
    /// recognize — never a silent `None`/default.
    pub fn parse(value: &str) -> StorageResult<Self> {
        match value {
            "discovered" => Ok(Self::Discovered),
            "source_captured" => Ok(Self::SourceCaptured),
            "extracting" => Ok(Self::Extracting),
            "staged" => Ok(Self::Staged),
            "validation_failed" => Ok(Self::ValidationFailed),
            "ready_to_commit" => Ok(Self::ReadyToCommit),
            "committing" => Ok(Self::Committing),
            "complete" => Ok(Self::Complete),
            "partial" => Ok(Self::Partial),
            "failed" => Ok(Self::Failed),
            "cancelled" => Ok(Self::Cancelled),
            other => Err(StorageError::UnknownKpiIngestRunState {
                value: other.to_owned(),
            }),
        }
    }

    /// Whether this state is one of the four run-ending states.
    pub fn is_terminal(self) -> bool {
        matches!(
            self,
            Self::Complete | Self::Partial | Self::Failed | Self::Cancelled
        )
    }

    /// Whether an external agent may `claim_next` a run in this state (B2 sol).
    pub fn is_agent_claimable(self) -> bool {
        matches!(
            self,
            Self::Discovered | Self::SourceCaptured | Self::Extracting | Self::ValidationFailed
        )
    }
}

/// The identity + discovery-known fields for a new run. `instruction_version`
/// is deliberately absent: nullable at creation, filled before `extracting`
/// (#360 invariant).
///
/// `period_fiscal_year`/`period_type` are the run's durable period
/// descriptor (ADR 0098 dec. 3, B2 sol review round 2, migration 0138): a
/// `financial_periods` row legally does not exist until the commit
/// transaction (#363) creates one, so staging needs a natural-key descriptor
/// to stage against before that. All-or-none (`create_run_if_absent` refuses
/// a partial descriptor); when both `period_id` and the descriptor are
/// present, the descriptor must match the referenced period row.
#[derive(Debug, Clone)]
pub struct NewKpiIngestRun {
    pub report_document_id: String,
    pub company_id: String,
    pub period_id: Option<String>,
    pub profile_version: String,
    pub scope: Option<String>,
    pub data_quality: Option<String>,
    pub period_fiscal_year: Option<i64>,
    pub period_type: Option<String>,
}

/// A stored run (the full read model). No `ts_rs` — this is a headless
/// domain-store card; MCP/UI surfaces are out of #358's scope.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct KpiIngestRun {
    pub id: String,
    pub report_document_id: String,
    pub company_id: String,
    pub period_id: Option<String>,
    pub source_content_hash: Option<String>,
    pub scope: Option<String>,
    pub data_quality: Option<String>,
    pub profile_version: String,
    pub instruction_version: Option<String>,
    pub status: KpiIngestRunState,
    pub manifest_hash: Option<String>,
    pub manifest_revision: i64,
    pub attempt_count: i64,
    pub lease_holder: Option<String>,
    pub lease_expires_at: Option<String>,
    pub last_heartbeat_at: Option<String>,
    pub expected_kpis_json: Option<String>,
    pub missing_reasons_json: Option<String>,
    pub progress_json: Option<String>,
    pub cost_json: Option<String>,
    pub last_error: Option<String>,
    pub created_at: String,
    pub updated_at: String,
    /// The run's period descriptor (migration 0138, ADR 0098 dec. 3) — see
    /// [`NewKpiIngestRun`] doc.
    pub period_fiscal_year: Option<i64>,
    pub period_type: Option<String>,
}

/// Row shape as read straight from SQLite, before `status` is parsed into the
/// domain enum (kept separate so a bad stored token surfaces as a typed error
/// at the exact call that read it, not a panic inside a `query_map` closure).
struct RawRun {
    id: String,
    report_document_id: String,
    company_id: String,
    period_id: Option<String>,
    source_content_hash: Option<String>,
    scope: Option<String>,
    data_quality: Option<String>,
    profile_version: String,
    instruction_version: Option<String>,
    status: String,
    manifest_hash: Option<String>,
    manifest_revision: i64,
    attempt_count: i64,
    lease_holder: Option<String>,
    lease_expires_at: Option<String>,
    last_heartbeat_at: Option<String>,
    expected_kpis_json: Option<String>,
    missing_reasons_json: Option<String>,
    progress_json: Option<String>,
    cost_json: Option<String>,
    last_error: Option<String>,
    created_at: String,
    updated_at: String,
    period_fiscal_year: Option<i64>,
    period_type: Option<String>,
}

fn map_raw_row(row: &rusqlite::Row<'_>) -> rusqlite::Result<RawRun> {
    Ok(RawRun {
        id: row.get(0)?,
        report_document_id: row.get(1)?,
        company_id: row.get(2)?,
        period_id: row.get(3)?,
        source_content_hash: row.get(4)?,
        scope: row.get(5)?,
        data_quality: row.get(6)?,
        profile_version: row.get(7)?,
        instruction_version: row.get(8)?,
        status: row.get(9)?,
        manifest_hash: row.get(10)?,
        manifest_revision: row.get(11)?,
        attempt_count: row.get(12)?,
        lease_holder: row.get(13)?,
        lease_expires_at: row.get(14)?,
        last_heartbeat_at: row.get(15)?,
        expected_kpis_json: row.get(16)?,
        missing_reasons_json: row.get(17)?,
        progress_json: row.get(18)?,
        cost_json: row.get(19)?,
        last_error: row.get(20)?,
        created_at: row.get(21)?,
        updated_at: row.get(22)?,
        period_fiscal_year: row.get(23)?,
        period_type: row.get(24)?,
    })
}

fn raw_to_domain(raw: RawRun) -> StorageResult<KpiIngestRun> {
    Ok(KpiIngestRun {
        status: KpiIngestRunState::parse(&raw.status)?,
        id: raw.id,
        report_document_id: raw.report_document_id,
        company_id: raw.company_id,
        period_id: raw.period_id,
        source_content_hash: raw.source_content_hash,
        scope: raw.scope,
        data_quality: raw.data_quality,
        profile_version: raw.profile_version,
        instruction_version: raw.instruction_version,
        manifest_hash: raw.manifest_hash,
        manifest_revision: raw.manifest_revision,
        attempt_count: raw.attempt_count,
        lease_holder: raw.lease_holder,
        lease_expires_at: raw.lease_expires_at,
        last_heartbeat_at: raw.last_heartbeat_at,
        expected_kpis_json: raw.expected_kpis_json,
        missing_reasons_json: raw.missing_reasons_json,
        progress_json: raw.progress_json,
        cost_json: raw.cost_json,
        last_error: raw.last_error,
        created_at: raw.created_at,
        updated_at: raw.updated_at,
        period_fiscal_year: raw.period_fiscal_year,
        period_type: raw.period_type,
    })
}

/// Collision-safe, non-deterministic (by design — B1 sol) id: `kpiing_` + 32
/// hex chars of sha256 over the identity triple plus a nanosecond time
/// component, mirroring `valuation_runs::run_id`'s idiom but WITH the time
/// component baked into the hash input (append-history, not a signature key).
fn generate_run_id(report_document_id: &str, company_id: &str, profile_version: &str) -> String {
    use sha2::{Digest, Sha256};
    let now_nanos = time::OffsetDateTime::now_utc().unix_timestamp_nanos();
    let key = format!(
        "kpiing:{report_document_id}\u{1f}{company_id}\u{1f}{profile_version}\u{1f}{now_nanos}"
    );
    let digest = Sha256::digest(key.as_bytes());
    let mut hex = String::with_capacity(32);
    for byte in &digest[..16] {
        hex.push_str(&format!("{byte:02x}"));
    }
    format!("kpiing_{hex}")
}

#[derive(Clone)]
pub struct KpiIngestRunsStore {
    db: Database,
}

impl KpiIngestRunsStore {
    pub(super) fn new(db: Database) -> Self {
        Self { db }
    }

    /// Idempotent create: if a NON-TERMINAL run already exists for the
    /// (document, company, profile) triple, return it unchanged; otherwise
    /// insert a new `discovered` run with an opaque id. Validates the document
    /// belongs to the given company first (`RunDocumentCompanyMismatch`).
    pub fn create_run_if_absent(&self, new_run: &NewKpiIngestRun) -> StorageResult<KpiIngestRun> {
        let mut connection = self.db.checkout()?;
        let tx = connection.transaction_with_behavior(rusqlite::TransactionBehavior::Immediate)?;

        if let Some(scope) = new_run.scope.as_deref() {
            if !matches!(scope, "standalone" | "consolidated") {
                return Err(StorageError::InvalidKpiIngestRunValue {
                    key: "scope",
                    value: scope.to_owned(),
                });
            }
        }
        if let Some(quality) = new_run.data_quality.as_deref() {
            if !matches!(quality, "final" | "preliminary" | "estimated") {
                return Err(StorageError::InvalidKpiIngestRunValue {
                    key: "data_quality",
                    value: quality.to_owned(),
                });
            }
        }

        // Period descriptor (ADR 0098 dec. 3, B2 sol review round 2):
        // all-or-none, then vocabulary. Cross-checked against an explicit
        // period_id below once that row is resolved.
        match (new_run.period_fiscal_year, new_run.period_type.as_deref()) {
            (None, None) => {}
            (Some(_), Some(period_type)) => {
                if !PERIOD_TYPE_VALUES.contains(&period_type) {
                    return Err(StorageError::InvalidKpiIngestRunValue {
                        key: "period_type",
                        value: period_type.to_owned(),
                    });
                }
            }
            _ => {
                return Err(StorageError::InvalidKpiIngestRunValue {
                    key: "period_descriptor",
                    value: format!(
                        "partial descriptor: period_fiscal_year={:?} period_type={:?}",
                        new_run.period_fiscal_year, new_run.period_type
                    ),
                });
            }
        }

        let doc_company: Option<String> = tx
            .query_row(
                "SELECT company_id FROM report_documents WHERE id = ?1",
                [&new_run.report_document_id],
                |row| row.get(0),
            )
            .optional()?;
        match doc_company {
            None => {
                return Err(StorageError::MissingIngestReference {
                    table: "report_documents".to_owned(),
                    id: new_run.report_document_id.clone(),
                });
            }
            Some(owner) if owner != new_run.company_id => {
                return Err(StorageError::RunDocumentCompanyMismatch {
                    run_document: new_run.report_document_id.clone(),
                    company: new_run.company_id.clone(),
                });
            }
            Some(_) => {}
        }
        if let Some(period_id) = new_run.period_id.as_deref() {
            let period_row: Option<(String, i64, String)> = tx
                .query_row(
                    "SELECT company_id, fiscal_year, period_type FROM financial_periods WHERE id = ?1",
                    [period_id],
                    |row| Ok((row.get(0)?, row.get(1)?, row.get(2)?)),
                )
                .optional()?;
            match period_row {
                None => {
                    return Err(StorageError::MissingIngestReference {
                        table: "financial_periods".to_owned(),
                        id: period_id.to_owned(),
                    });
                }
                Some((owner, ..)) if owner != new_run.company_id => {
                    return Err(StorageError::RunPeriodCompanyMismatch {
                        period: period_id.to_owned(),
                        company: new_run.company_id.clone(),
                    });
                }
                Some((_, period_fiscal_year, period_type)) => {
                    // Descriptor<->period_id consistency (B2 sol review round
                    // 2): when BOTH are supplied, they must name the same
                    // period — a caller-shaped mismatch, not a genuine
                    // cross-run conflict (that is RunPeriodConflict below).
                    if let (Some(descriptor_year), Some(descriptor_type)) =
                        (new_run.period_fiscal_year, new_run.period_type.as_deref())
                    {
                        if descriptor_year != period_fiscal_year || descriptor_type != period_type {
                            return Err(StorageError::InvalidKpiIngestRunValue {
                                key: "period_descriptor",
                                value: format!(
                                    "descriptor ({descriptor_year}, {descriptor_type}) does not \
                                     match period {period_id} ({period_fiscal_year}, {period_type})"
                                ),
                            });
                        }
                    }
                }
            }
        }

        let existing: Option<RawRun> = tx
            .query_row(
                &format!(
                    "SELECT {RUN_COLUMNS} FROM kpi_ingest_runs
                     WHERE report_document_id = ?1 AND company_id = ?2 AND profile_version = ?3
                       AND status NOT IN {TERMINAL_STATUSES_SQL}"
                ),
                params![
                    new_run.report_document_id,
                    new_run.company_id,
                    new_run.profile_version
                ],
                map_raw_row,
            )
            .optional()?;

        if let Some(raw) = existing {
            // An active run for the same triple with a DIFFERENT period is a
            // genuine conflict (B2 sol review round 3), never a silent
            // idempotent return: comparing on whichever identity both sides
            // supply (period_id, else the descriptor pair) so a request that
            // merely fills in a previously-absent value is not a conflict.
            let requested_id = new_run.period_id.as_deref();
            if let (Some(existing_id), Some(requested_id)) =
                (raw.period_id.as_deref(), requested_id)
            {
                if existing_id != requested_id {
                    return Err(StorageError::RunPeriodConflict { id: raw.id });
                }
            }
            if let (
                Some(existing_year),
                Some(existing_type),
                Some(requested_year),
                Some(requested_type),
            ) = (
                raw.period_fiscal_year,
                raw.period_type.as_deref(),
                new_run.period_fiscal_year,
                new_run.period_type.as_deref(),
            ) {
                if existing_year != requested_year || existing_type != requested_type {
                    return Err(StorageError::RunPeriodConflict { id: raw.id });
                }
            }
            tx.commit()?;
            return raw_to_domain(raw);
        }

        let id = generate_run_id(
            &new_run.report_document_id,
            &new_run.company_id,
            &new_run.profile_version,
        );
        tx.execute(
            "INSERT INTO kpi_ingest_runs
                (id, report_document_id, company_id, period_id, profile_version, scope, data_quality,
                 period_fiscal_year, period_type)
             VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9)",
            params![
                id,
                new_run.report_document_id,
                new_run.company_id,
                new_run.period_id,
                new_run.profile_version,
                new_run.scope,
                new_run.data_quality,
                new_run.period_fiscal_year,
                new_run.period_type,
            ],
        )?;

        let raw: RawRun = tx.query_row(
            &format!("SELECT {RUN_COLUMNS} FROM kpi_ingest_runs WHERE id = ?1"),
            [&id],
            map_raw_row,
        )?;
        tx.commit()?;
        raw_to_domain(raw)
    }

    /// Set-once `source_content_hash`. Same hash again is a no-op; a different
    /// hash is refused (`RunSourceHashAlreadyRecorded`); an unknown run id is
    /// `KpiIngestRunNotFound` — never a silent no-op. The write is a guarded
    /// `UPDATE … WHERE source_content_hash IS NULL`, so two racing writers can
    /// never overwrite each other (the loser re-reads and gets classified).
    pub fn record_source_capture(&self, id: &str, source_content_hash: &str) -> StorageResult<()> {
        let connection = self.db.checkout()?;
        let updated = connection.execute(
            "UPDATE kpi_ingest_runs
             SET source_content_hash = ?2,
                 updated_at = strftime('%Y-%m-%dT%H:%M:%fZ', 'now')
             WHERE id = ?1 AND source_content_hash IS NULL",
            params![id, source_content_hash],
        )?;
        if updated == 1 {
            return Ok(());
        }
        let current: Option<Option<String>> = connection
            .query_row(
                "SELECT source_content_hash FROM kpi_ingest_runs WHERE id = ?1",
                [id],
                |row| row.get(0),
            )
            .optional()?;
        match current {
            None => Err(StorageError::KpiIngestRunNotFound { id: id.to_owned() }),
            Some(Some(existing)) if existing == source_content_hash => Ok(()),
            _ => Err(StorageError::RunSourceHashAlreadyRecorded { id: id.to_owned() }),
        }
    }

    pub fn get_run(&self, id: &str) -> StorageResult<Option<KpiIngestRun>> {
        let connection = self.db.checkout()?;
        let raw: Option<RawRun> = connection
            .query_row(
                &format!("SELECT {RUN_COLUMNS} FROM kpi_ingest_runs WHERE id = ?1"),
                [id],
                map_raw_row,
            )
            .optional()?;
        raw.map(raw_to_domain).transpose()
    }

    /// Filtered listing, newest-first by `created_at` with `id` as a stable
    /// tie-break (never a domain ordering signal — this table has no as-of date).
    pub fn list_runs(
        &self,
        status: Option<KpiIngestRunState>,
        company_id: Option<&str>,
    ) -> StorageResult<Vec<KpiIngestRun>> {
        let connection = self.db.checkout()?;
        let base = format!("SELECT {RUN_COLUMNS} FROM kpi_ingest_runs");
        const ORDER: &str = " ORDER BY created_at DESC, id ASC";
        let raws: Vec<RawRun> = match (status, company_id) {
            (Some(status), Some(company_id)) => {
                let sql = format!("{base} WHERE status = ?1 AND company_id = ?2{ORDER}");
                let mut stmt = connection.prepare(&sql)?;
                let rows = stmt
                    .query_map(params![status.as_str(), company_id], map_raw_row)?
                    .collect::<Result<_, _>>()?;
                rows
            }
            (Some(status), None) => {
                let sql = format!("{base} WHERE status = ?1{ORDER}");
                let mut stmt = connection.prepare(&sql)?;
                let rows = stmt
                    .query_map(params![status.as_str()], map_raw_row)?
                    .collect::<Result<_, _>>()?;
                rows
            }
            (None, Some(company_id)) => {
                let sql = format!("{base} WHERE company_id = ?1{ORDER}");
                let mut stmt = connection.prepare(&sql)?;
                let rows = stmt
                    .query_map(params![company_id], map_raw_row)?
                    .collect::<Result<_, _>>()?;
                rows
            }
            (None, None) => {
                let sql = format!("{base}{ORDER}");
                let mut stmt = connection.prepare(&sql)?;
                let rows = stmt.query_map([], map_raw_row)?.collect::<Result<_, _>>()?;
                rows
            }
        };
        raws.into_iter().map(raw_to_domain).collect()
    }

    /// Atomically claim the oldest runnable row: claimable status AND
    /// (unleased OR lease expired). One `UPDATE … WHERE id = (SELECT …)
    /// RETURNING` statement (jobs.rs::claim_next idiom) so two workers can
    /// never win the same row, and a lease takeover counts as a new attempt.
    /// Rejects `lease_seconds <= 0` (`InvalidRunLeaseDuration`).
    pub fn claim_next(
        &self,
        holder: &str,
        lease_seconds: i64,
    ) -> StorageResult<Option<KpiIngestRun>> {
        if lease_seconds <= 0 {
            return Err(StorageError::InvalidRunLeaseDuration {
                seconds: lease_seconds,
            });
        }
        let connection = self.db.checkout()?;
        let sql = format!(
            "UPDATE kpi_ingest_runs
             SET lease_holder = ?1,
                 lease_expires_at = strftime('%Y-%m-%dT%H:%M:%fZ', 'now', ?2),
                 last_heartbeat_at = strftime('%Y-%m-%dT%H:%M:%fZ', 'now'),
                 attempt_count = attempt_count + 1,
                 updated_at = strftime('%Y-%m-%dT%H:%M:%fZ', 'now')
             WHERE id = (
                 SELECT candidate.id
                 FROM kpi_ingest_runs candidate
                 WHERE candidate.status IN {CLAIMABLE_STATUSES_SQL}
                   AND (candidate.lease_expires_at IS NULL
                        OR candidate.lease_expires_at <= strftime('%Y-%m-%dT%H:%M:%fZ', 'now'))
                 ORDER BY candidate.created_at, candidate.id
                 LIMIT 1
             )
             RETURNING {RUN_COLUMNS}"
        );
        let raw: Option<RawRun> = connection
            .query_row(
                &sql,
                params![holder, format!("+{lease_seconds} seconds")],
                map_raw_row,
            )
            .optional()?;
        raw.map(raw_to_domain).transpose()
    }

    /// Extend a held, still-live lease. Wrong holder or an already-expired
    /// lease both refuse with `RunLeaseNotHeld` (an expired lease cannot be
    /// resurrected by its old holder — only a fresh `claim_next` may reclaim
    /// it). Rejects `lease_seconds <= 0`. Never increments `attempt_count`.
    pub fn heartbeat(&self, id: &str, holder: &str, lease_seconds: i64) -> StorageResult<()> {
        if lease_seconds <= 0 {
            return Err(StorageError::InvalidRunLeaseDuration {
                seconds: lease_seconds,
            });
        }
        let connection = self.db.checkout()?;
        let changed = connection.execute(
            "UPDATE kpi_ingest_runs
             SET lease_expires_at = strftime('%Y-%m-%dT%H:%M:%fZ', 'now', ?3),
                 last_heartbeat_at = strftime('%Y-%m-%dT%H:%M:%fZ', 'now'),
                 updated_at = strftime('%Y-%m-%dT%H:%M:%fZ', 'now')
             WHERE id = ?1 AND lease_holder = ?2
               AND lease_expires_at > strftime('%Y-%m-%dT%H:%M:%fZ', 'now')",
            params![id, holder, format!("+{lease_seconds} seconds")],
        )?;
        if changed == 0 {
            return Err(StorageError::RunLeaseNotHeld {
                id: id.to_owned(),
                holder: holder.to_owned(),
            });
        }
        Ok(())
    }

    /// Clear the lease for its owner. Wrong holder refuses with
    /// `RunLeaseNotHeld`. Never increments `attempt_count`.
    pub fn release_lease(&self, id: &str, holder: &str) -> StorageResult<()> {
        let connection = self.db.checkout()?;
        let changed = connection.execute(
            "UPDATE kpi_ingest_runs
             SET lease_holder = NULL, lease_expires_at = NULL, last_heartbeat_at = NULL,
                 updated_at = strftime('%Y-%m-%dT%H:%M:%fZ', 'now')
             WHERE id = ?1 AND lease_holder = ?2",
            params![id, holder],
        )?;
        if changed == 0 {
            return Err(StorageError::RunLeaseNotHeld {
                id: id.to_owned(),
                holder: holder.to_owned(),
            });
        }
        Ok(())
    }

    /// Clear every expired lease on a claimable-status row. Refuses
    /// (`RunLeaseInvariantViolation`) instead of silently repairing if any
    /// `committing` row holds a non-null lease — that combination must never
    /// occur (the transition to `committing` clears the lease, #360) and is a
    /// bug to surface, not paper over. Returns the number of rows cleared.
    pub fn reclaim_expired_leases(&self) -> StorageResult<usize> {
        let connection = self.db.checkout()?;
        // Invariant guard first: a `committing` row must never hold a lease
        // (the transition to `committing` clears it — #360). Refuse instead of
        // silently repairing a state that should never occur.
        if let Some((id, status)) = connection
            .query_row(
                "SELECT id, status FROM kpi_ingest_runs
                 WHERE status = 'committing' AND lease_holder IS NOT NULL LIMIT 1",
                [],
                |row| Ok((row.get::<_, String>(0)?, row.get::<_, String>(1)?)),
            )
            .optional()?
        {
            return Err(StorageError::RunLeaseInvariantViolation { id, status });
        }

        let sql = format!(
            "UPDATE kpi_ingest_runs
             SET lease_holder = NULL, lease_expires_at = NULL, last_heartbeat_at = NULL,
                 updated_at = strftime('%Y-%m-%dT%H:%M:%fZ', 'now')
             WHERE status IN {CLAIMABLE_STATUSES_SQL}
               AND lease_expires_at IS NOT NULL
               AND lease_expires_at <= strftime('%Y-%m-%dT%H:%M:%fZ', 'now')"
        );
        let changed = connection.execute(&sql, [])?;
        Ok(changed)
    }
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

    fn seed_period(connection: &Connection, id: &str, company_id: &str) {
        connection
            .execute(
                "INSERT INTO financial_periods (id, company_id, fiscal_year, period_type)
                 VALUES (?1, ?2, 2025, 'annual')",
                params![id, company_id],
            )
            .expect("seed period");
    }

    fn new_run(doc: &str, company: &str, profile: &str) -> NewKpiIngestRun {
        NewKpiIngestRun {
            report_document_id: doc.to_owned(),
            company_id: company.to_owned(),
            period_id: None,
            profile_version: profile.to_owned(),
            scope: None,
            data_quality: None,
            period_fiscal_year: None,
            period_type: None,
        }
    }

    /// Raw insert bypassing the store — the only way to seed a specific status
    /// / lease combination, since production ships zero status-mutation seam
    /// beyond create/lease/capture (B3 sol; typed transitions land in #360).
    #[allow(clippy::too_many_arguments)]
    fn seed_run_raw(
        connection: &Connection,
        id: &str,
        doc: &str,
        company: &str,
        profile: &str,
        status: &str,
        lease_holder: Option<&str>,
        lease_expires_at: Option<&str>,
        last_heartbeat_at: Option<&str>,
    ) {
        connection
            .execute(
                "INSERT INTO kpi_ingest_runs
                    (id, report_document_id, company_id, profile_version, status,
                     lease_holder, lease_expires_at, last_heartbeat_at)
                 VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8)",
                params![
                    id,
                    doc,
                    company,
                    profile,
                    status,
                    lease_holder,
                    lease_expires_at,
                    last_heartbeat_at
                ],
            )
            .expect("seed run");
    }

    fn setup_one_company_doc() -> (AppState, &'static str, &'static str) {
        let connection = open_in_memory_database().expect("db");
        seed_company(&connection, "c1");
        seed_document(&connection, "doc1", "c1");
        (AppState::new(connection), "doc1", "c1")
    }

    #[test]
    fn create_run_if_absent_is_idempotent_for_a_nonterminal_triple() {
        let (state, doc, company) = setup_one_company_doc();
        let store = state.kpi_ingest_runs();
        let first = store
            .create_run_if_absent(&new_run(doc, company, "p1"))
            .expect("create");
        let second = store
            .create_run_if_absent(&new_run(doc, company, "p1"))
            .expect("create again");
        assert_eq!(
            first.id, second.id,
            "a non-terminal run for the same triple is reused"
        );
        assert_eq!(store.list_runs(None, None).expect("list").len(), 1);
    }

    #[test]
    fn create_run_if_absent_starts_a_new_run_after_a_terminal_one() {
        let (state, doc, company) = setup_one_company_doc();
        let store = state.kpi_ingest_runs();
        let first = store
            .create_run_if_absent(&new_run(doc, company, "p1"))
            .expect("create");
        // Seed the first run as terminal directly (no production status seam).
        let connection = state.checkout_for_tests().expect("raw connection");
        connection
            .execute(
                "UPDATE kpi_ingest_runs SET status = 'complete' WHERE id = ?1",
                [&first.id],
            )
            .expect("terminate");
        drop(connection);

        let second = store
            .create_run_if_absent(&new_run(doc, company, "p1"))
            .expect("create after terminal");
        assert_ne!(
            first.id, second.id,
            "a content change under the same triple starts a NEW run once the old one is terminal"
        );
        assert_eq!(store.list_runs(None, None).expect("list").len(), 2);
    }

    #[test]
    fn create_run_if_absent_separates_different_profile_versions() {
        let (state, doc, company) = setup_one_company_doc();
        let store = state.kpi_ingest_runs();
        let p1 = store
            .create_run_if_absent(&new_run(doc, company, "p1"))
            .expect("create p1");
        let p2 = store
            .create_run_if_absent(&new_run(doc, company, "p2"))
            .expect("create p2");
        assert_ne!(p1.id, p2.id);
    }

    #[test]
    fn create_run_if_absent_rejects_a_document_from_another_company() {
        let connection = open_in_memory_database().expect("db");
        seed_company(&connection, "c1");
        seed_company(&connection, "c2");
        seed_document(&connection, "doc1", "c1");
        let state = AppState::new(connection);
        let store = state.kpi_ingest_runs();

        let error = store
            .create_run_if_absent(&new_run("doc1", "c2", "p1"))
            .expect_err("document belongs to c1, not c2");
        assert!(matches!(
            error,
            StorageError::RunDocumentCompanyMismatch { .. }
        ));
    }

    #[test]
    fn claim_next_matrix_of_claimable_and_non_claimable_states() {
        let claimable = [
            "discovered",
            "source_captured",
            "extracting",
            "validation_failed",
        ];
        let non_claimable = [
            "staged",
            "ready_to_commit",
            "committing",
            "complete",
            "partial",
            "failed",
            "cancelled",
        ];

        for status in claimable {
            let connection = open_in_memory_database().expect("db");
            seed_company(&connection, "c1");
            seed_document(&connection, "doc1", "c1");
            seed_run_raw(
                &connection,
                "run1",
                "doc1",
                "c1",
                "p1",
                status,
                None,
                None,
                None,
            );
            let state = AppState::new(connection);
            let store = state.kpi_ingest_runs();
            let claimed = store.claim_next("worker-a", 60).expect("claim");
            assert!(claimed.is_some(), "status '{status}' must be claimable");
            let claimed = claimed.unwrap();
            assert_eq!(claimed.attempt_count, 1);
            assert_eq!(claimed.lease_holder.as_deref(), Some("worker-a"));
        }

        for status in non_claimable {
            let connection = open_in_memory_database().expect("db");
            seed_company(&connection, "c1");
            seed_document(&connection, "doc1", "c1");
            seed_run_raw(
                &connection,
                "run1",
                "doc1",
                "c1",
                "p1",
                status,
                None,
                None,
                None,
            );
            let state = AppState::new(connection);
            let store = state.kpi_ingest_runs();
            let claimed = store.claim_next("worker-a", 60).expect("claim");
            assert!(claimed.is_none(), "status '{status}' must NOT be claimable");
        }
    }

    #[test]
    fn claim_next_respects_a_live_lease_and_takes_over_an_expired_one() {
        let connection = open_in_memory_database().expect("db");
        seed_company(&connection, "c1");
        seed_document(&connection, "doc1", "c1");
        seed_run_raw(
            &connection,
            "run1",
            "doc1",
            "c1",
            "p1",
            "extracting",
            Some("worker-a"),
            Some("2999-01-01T00:00:00.000Z"),
            Some("2026-01-01T00:00:00.000Z"),
        );
        let state = AppState::new(connection);
        let store = state.kpi_ingest_runs();

        assert!(
            store.claim_next("worker-b", 60).expect("claim").is_none(),
            "a live lease must not be claimable by another worker"
        );

        // Expire it directly (no production seam), then a takeover is a new attempt.
        let connection = state.checkout_for_tests().expect("raw connection");
        connection
            .execute(
                "UPDATE kpi_ingest_runs SET lease_expires_at = '2000-01-01T00:00:00.000Z' WHERE id = 'run1'",
                [],
            )
            .expect("expire lease");
        drop(connection);

        let claimed = store
            .claim_next("worker-b", 60)
            .expect("claim after expiry")
            .expect("some");
        assert_eq!(claimed.lease_holder.as_deref(), Some("worker-b"));
        assert_eq!(
            claimed.attempt_count, 1,
            "expired takeover is a NEW attempt, counted once"
        );
    }

    #[test]
    fn claim_next_and_heartbeat_reject_non_positive_lease_seconds() {
        let (state, doc, company) = setup_one_company_doc();
        let store = state.kpi_ingest_runs();
        store
            .create_run_if_absent(&new_run(doc, company, "p1"))
            .expect("create");

        let error = store.claim_next("worker-a", 0).expect_err("must reject");
        assert!(matches!(
            error,
            StorageError::InvalidRunLeaseDuration { seconds: 0 }
        ));
        let error = store.claim_next("worker-a", -5).expect_err("must reject");
        assert!(matches!(
            error,
            StorageError::InvalidRunLeaseDuration { seconds: -5 }
        ));

        let error = store
            .heartbeat("whatever", "worker-a", 0)
            .expect_err("must reject");
        assert!(matches!(
            error,
            StorageError::InvalidRunLeaseDuration { seconds: 0 }
        ));
    }

    #[test]
    fn heartbeat_extends_for_the_live_owner_and_rejects_others_and_expired() {
        let (state, doc, company) = setup_one_company_doc();
        let store = state.kpi_ingest_runs();
        let run = store
            .create_run_if_absent(&new_run(doc, company, "p1"))
            .expect("create");
        let claimed = store
            .claim_next("worker-a", 60)
            .expect("claim")
            .expect("some");
        assert_eq!(claimed.id, run.id);

        store
            .heartbeat(&run.id, "worker-a", 120)
            .expect("live owner heartbeat succeeds");
        let after = store.get_run(&run.id).expect("get").expect("some");
        assert_eq!(
            after.attempt_count, 1,
            "heartbeat never increments attempt_count"
        );

        let error = store
            .heartbeat(&run.id, "worker-b", 60)
            .expect_err("wrong holder must fail");
        assert!(matches!(error, StorageError::RunLeaseNotHeld { .. }));

        // Expire, then even the original holder cannot resurrect it.
        let connection = state.checkout_for_tests().expect("raw connection");
        connection
            .execute(
                &format!(
                    "UPDATE kpi_ingest_runs SET lease_expires_at = '2000-01-01T00:00:00.000Z' WHERE id = '{}'",
                    run.id
                ),
                [],
            )
            .expect("expire");
        drop(connection);
        let error = store
            .heartbeat(&run.id, "worker-a", 60)
            .expect_err("expired lease cannot be heartbeat by the old holder");
        assert!(matches!(error, StorageError::RunLeaseNotHeld { .. }));
    }

    #[test]
    fn release_lease_clears_for_owner_and_rejects_others() {
        let (state, doc, company) = setup_one_company_doc();
        let store = state.kpi_ingest_runs();
        let run = store
            .create_run_if_absent(&new_run(doc, company, "p1"))
            .expect("create");
        store.claim_next("worker-a", 60).expect("claim");

        let error = store
            .release_lease(&run.id, "worker-b")
            .expect_err("wrong holder must fail");
        assert!(matches!(error, StorageError::RunLeaseNotHeld { .. }));

        store
            .release_lease(&run.id, "worker-a")
            .expect("owner releases");
        let after = store.get_run(&run.id).expect("get").expect("some");
        assert!(after.lease_holder.is_none());
        assert!(after.lease_expires_at.is_none());
        assert!(after.last_heartbeat_at.is_none());
        assert_eq!(
            after.attempt_count, 1,
            "release never increments attempt_count"
        );
    }

    #[test]
    fn reclaim_expired_leases_clears_only_claimable_states_and_guards_committing() {
        let connection = open_in_memory_database().expect("db");
        seed_company(&connection, "c1");
        seed_document(&connection, "doc1", "c1");
        seed_document(&connection, "doc2", "c1");
        seed_run_raw(
            &connection,
            "run1",
            "doc1",
            "c1",
            "p1",
            "extracting",
            Some("worker-a"),
            Some("2000-01-01T00:00:00.000Z"),
            Some("2000-01-01T00:00:00.000Z"),
        );
        let state = AppState::new(connection);
        let store = state.kpi_ingest_runs();

        let cleared = store.reclaim_expired_leases().expect("reclaim");
        assert_eq!(cleared, 1);
        let after = store.get_run("run1").expect("get").expect("some");
        assert!(after.lease_holder.is_none());
        assert_eq!(
            after.status,
            KpiIngestRunState::Extracting,
            "reclaim never changes status"
        );

        // Now seed a `committing` row holding a lease — a structural invariant
        // violation (the transition to `committing` must clear the lease, #360).
        let connection = state.checkout_for_tests().expect("raw connection");
        seed_run_raw(
            &connection,
            "run2",
            "doc2",
            "c1",
            "p1",
            "committing",
            Some("worker-a"),
            Some("2000-01-01T00:00:00.000Z"),
            Some("2000-01-01T00:00:00.000Z"),
        );
        drop(connection);

        let error = store
            .reclaim_expired_leases()
            .expect_err("committing+lease must refuse, never silently repair");
        assert!(matches!(
            error,
            StorageError::RunLeaseInvariantViolation { .. }
        ));
    }

    #[test]
    fn record_source_capture_is_set_once() {
        let (state, doc, company) = setup_one_company_doc();
        let store = state.kpi_ingest_runs();
        let run = store
            .create_run_if_absent(&new_run(doc, company, "p1"))
            .expect("create");

        store
            .record_source_capture(&run.id, "hash1")
            .expect("first capture");
        let after = store.get_run(&run.id).expect("get").expect("some");
        assert_eq!(after.source_content_hash.as_deref(), Some("hash1"));

        store
            .record_source_capture(&run.id, "hash1")
            .expect("same hash again is a no-op");

        let error = store
            .record_source_capture(&run.id, "hash2")
            .expect_err("a different hash must be refused");
        assert!(matches!(
            error,
            StorageError::RunSourceHashAlreadyRecorded { .. }
        ));

        let error = store
            .record_source_capture("kpiing_missing", "hash1")
            .expect_err("an unknown run id must be refused, never a silent no-op");
        assert!(matches!(error, StorageError::KpiIngestRunNotFound { .. }));
    }

    #[test]
    fn check_constraints_reject_bad_rows() {
        let connection = open_in_memory_database().expect("db");
        seed_company(&connection, "c1");
        seed_document(&connection, "doc1", "c1");

        let base = |extra_cols: &str, extra_vals: &str| {
            format!(
                "INSERT INTO kpi_ingest_runs (id, report_document_id, company_id, profile_version{extra_cols})
                 VALUES ('bad', 'doc1', 'c1', 'p1'{extra_vals})"
            )
        };

        assert!(
            connection
                .execute(&base(", status", ", 'not_a_status'"), [])
                .is_err(),
            "unknown status must be rejected"
        );
        assert!(
            connection
                .execute(&base(", scope", ", 'not_a_scope'"), [])
                .is_err(),
            "bad scope must be rejected"
        );
        assert!(
            connection
                .execute(&base(", data_quality", ", 'not_a_quality'"), [])
                .is_err(),
            "bad data_quality must be rejected"
        );
        assert!(
            connection
                .execute(&base(", attempt_count", ", -1"), [])
                .is_err(),
            "negative attempt_count must be rejected"
        );
        assert!(
            connection
                .execute(
                    &base(", lease_holder, lease_expires_at", ", 'w1', NULL"),
                    []
                )
                .is_err(),
            "a half-filled lease must be rejected"
        );
    }

    #[test]
    fn get_run_and_list_runs_filter_and_order_stably() {
        let connection = open_in_memory_database().expect("db");
        seed_company(&connection, "c1");
        seed_company(&connection, "c2");
        seed_document(&connection, "doc1", "c1");
        seed_document(&connection, "doc2", "c2");
        let state = AppState::new(connection);
        let store = state.kpi_ingest_runs();

        let r1 = store
            .create_run_if_absent(&new_run("doc1", "c1", "p1"))
            .expect("r1");
        let r2 = store
            .create_run_if_absent(&new_run("doc2", "c2", "p1"))
            .expect("r2");

        assert!(store.get_run("does-not-exist").expect("get").is_none());
        assert_eq!(store.get_run(&r1.id).expect("get").expect("some").id, r1.id);

        let for_c1 = store.list_runs(None, Some("c1")).expect("list c1");
        assert_eq!(for_c1.len(), 1);
        assert_eq!(for_c1[0].id, r1.id);

        let discovered = store
            .list_runs(Some(KpiIngestRunState::Discovered), None)
            .expect("list discovered");
        assert_eq!(discovered.len(), 2);

        let _ = r2;
    }

    #[test]
    fn unknown_stored_status_token_is_a_typed_parse_error() {
        let error = KpiIngestRunState::parse("not_a_real_state").expect_err("unknown token");
        assert!(matches!(
            error,
            StorageError::UnknownKpiIngestRunState { .. }
        ));
    }

    /// Two-thread race on a SINGLE qualifying run: exactly one winner. Needs a
    /// FILE-backed pool (not `open_in_memory_database`) — the in-memory test
    /// path wraps one shared connection behind a single `Mutex`
    /// (`Database::from_connection`), which serializes every checkout and can
    /// never exercise a genuine SQLite-level race between two connections.
    #[test]
    fn claim_next_two_threads_exactly_one_winner() {
        use r2d2_sqlite::SqliteConnectionManager;
        use std::sync::Arc;

        let db_path = std::env::temp_dir().join(format!(
            "brawler-kpi-ingest-race-{}-{}.sqlite3",
            std::process::id(),
            time::OffsetDateTime::now_utc().unix_timestamp_nanos()
        ));

        {
            let mut connection = Connection::open(&db_path).expect("open file db");
            crate::storage::migrations::apply_migrations(&mut connection).expect("migrate");
            seed_company(&connection, "c1");
            seed_document(&connection, "doc1", "c1");
            seed_run_raw(
                &connection,
                "run1",
                "doc1",
                "c1",
                "p1",
                "discovered",
                None,
                None,
                None,
            );
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
        let store = KpiIngestRunsStore::new(Database::from_pool(pool));
        let store = Arc::new(store);

        let store_a = store.clone();
        let store_b = store.clone();
        let thread_a = std::thread::spawn(move || store_a.claim_next("worker-a", 60));
        let thread_b = std::thread::spawn(move || store_b.claim_next("worker-b", 60));

        let result_a = thread_a.join().expect("thread a").expect("claim a");
        let result_b = thread_b.join().expect("thread b").expect("claim b");

        let winners = [result_a, result_b]
            .into_iter()
            .flatten()
            .collect::<Vec<_>>();
        assert_eq!(
            winners.len(),
            1,
            "exactly one thread must win the single claimable run"
        );

        let _ = std::fs::remove_file(&db_path);
        let _ = std::fs::remove_file(db_path.with_extension("sqlite3-wal"));
        let _ = std::fs::remove_file(db_path.with_extension("sqlite3-shm"));
    }

    /// Two writers racing the set-once capture: the guarded UPDATE guarantees
    /// exactly one hash survives and the loser gets the typed refusal — the
    /// TOCTOU class a SELECT-then-UPDATE would reintroduce.
    #[test]
    fn record_source_capture_two_threads_one_hash_survives() {
        use r2d2_sqlite::SqliteConnectionManager;
        use std::sync::Arc;

        let db_path = std::env::temp_dir().join(format!(
            "brawler-kpi-ingest-capture-race-{}-{}.sqlite3",
            std::process::id(),
            time::OffsetDateTime::now_utc().unix_timestamp_nanos()
        ));
        {
            let mut connection = Connection::open(&db_path).expect("open file db");
            crate::storage::migrations::apply_migrations(&mut connection).expect("migrate");
            seed_company(&connection, "c1");
            seed_document(&connection, "doc1", "c1");
            seed_run_raw(
                &connection,
                "run1",
                "doc1",
                "c1",
                "p1",
                "discovered",
                None,
                None,
                None,
            );
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
        let store = Arc::new(KpiIngestRunsStore::new(Database::from_pool(pool)));

        let store_a = store.clone();
        let store_b = store.clone();
        let a = std::thread::spawn(move || store_a.record_source_capture("run1", "hash-a"));
        let b = std::thread::spawn(move || store_b.record_source_capture("run1", "hash-b"));
        let result_a = a.join().expect("thread a");
        let result_b = b.join().expect("thread b");

        let ok_count = [&result_a, &result_b]
            .iter()
            .filter(|result| result.is_ok())
            .count();
        assert_eq!(ok_count, 1, "exactly one writer records the hash");
        let loser = if result_a.is_err() {
            result_a
        } else {
            result_b
        };
        assert!(matches!(
            loser.expect_err("loser"),
            StorageError::RunSourceHashAlreadyRecorded { .. }
        ));
        let run = store.get_run("run1").expect("get").expect("some");
        assert!(matches!(
            run.source_content_hash.as_deref(),
            Some("hash-a") | Some("hash-b")
        ));

        let _ = std::fs::remove_file(&db_path);
        let _ = std::fs::remove_file(db_path.with_extension("sqlite3-wal"));
        let _ = std::fs::remove_file(db_path.with_extension("sqlite3-shm"));
    }

    #[test]
    fn create_run_validates_period_ownership_and_references() {
        let connection = open_in_memory_database().expect("db");
        seed_company(&connection, "c1");
        seed_company(&connection, "c-other");
        seed_document(&connection, "doc1", "c1");
        seed_period(&connection, "finper_other", "c-other");
        let state = AppState::new(connection);
        let store = state.kpi_ingest_runs();
        let (doc, company) = ("doc1", "c1");

        // Unknown document is NotFound, never a company mismatch.
        let missing_doc = new_run("doc-missing", company, "p1");
        let error = store
            .create_run_if_absent(&missing_doc)
            .expect_err("unknown document");
        assert!(matches!(error, StorageError::MissingIngestReference { .. }));

        // Unknown period.
        let mut unknown_period = new_run(doc, company, "p1");
        unknown_period.period_id = Some("finper_missing".to_owned());
        let error = store
            .create_run_if_absent(&unknown_period)
            .expect_err("unknown period");
        assert!(matches!(error, StorageError::MissingIngestReference { .. }));

        // Period owned by another company.
        let mut foreign_period = new_run(doc, company, "p1");
        foreign_period.period_id = Some("finper_other".to_owned());
        let error = store
            .create_run_if_absent(&foreign_period)
            .expect_err("foreign period");
        assert!(matches!(
            error,
            StorageError::RunPeriodCompanyMismatch { .. }
        ));

        // Invalid vocabulary values are typed refusals, not raw CHECK conflicts.
        let mut bad_scope = new_run(doc, company, "p1");
        bad_scope.scope = Some("group".to_owned());
        assert!(matches!(
            store.create_run_if_absent(&bad_scope).expect_err("scope"),
            StorageError::InvalidKpiIngestRunValue { key: "scope", .. }
        ));
        let mut bad_quality = new_run(doc, company, "p1");
        bad_quality.data_quality = Some("draft".to_owned());
        assert!(matches!(
            store
                .create_run_if_absent(&bad_quality)
                .expect_err("quality"),
            StorageError::InvalidKpiIngestRunValue {
                key: "data_quality",
                ..
            }
        ));
    }

    /// Test 10 (#359 plan): the run's period descriptor — all-or-none,
    /// consistency with an explicit `period_id`, and the cross-request
    /// conflict on an active triple.
    #[test]
    fn create_run_if_absent_enforces_the_period_descriptor_contract() {
        let connection = open_in_memory_database().expect("db");
        seed_company(&connection, "c1");
        seed_document(&connection, "doc1", "c1");
        // A period whose stored period_type is the canonical 'FY' token (not
        // the legacy 'annual' alias `seed_period` uses elsewhere), so it is a
        // valid descriptor consistency target.
        connection
            .execute(
                "INSERT INTO financial_periods (id, company_id, fiscal_year, period_type)
                 VALUES ('finper1', 'c1', 2025, 'FY')",
                [],
            )
            .expect("seed period");
        let state = AppState::new(connection);
        let store = state.kpi_ingest_runs();

        // A descriptor with no period_id succeeds.
        let mut with_descriptor = new_run("doc1", "c1", "p1");
        with_descriptor.period_fiscal_year = Some(2025);
        with_descriptor.period_type = Some("FY".to_owned());
        let created = store
            .create_run_if_absent(&with_descriptor)
            .expect("descriptor-only run must be creatable");
        assert_eq!(created.period_fiscal_year, Some(2025));
        assert_eq!(created.period_type.as_deref(), Some("FY"));
        assert!(created.period_id.is_none());

        // A partial descriptor is refused.
        let mut partial = new_run("doc1", "c1", "p2");
        partial.period_fiscal_year = Some(2025);
        let error = store
            .create_run_if_absent(&partial)
            .expect_err("partial descriptor must be refused");
        assert!(matches!(
            error,
            StorageError::InvalidKpiIngestRunValue {
                key: "period_descriptor",
                ..
            }
        ));

        // A descriptor that contradicts an explicit period_id is refused.
        let mut contradictory = new_run("doc1", "c1", "p3");
        contradictory.period_id = Some("finper1".to_owned());
        contradictory.period_fiscal_year = Some(2024);
        contradictory.period_type = Some("FY".to_owned());
        let error = store
            .create_run_if_absent(&contradictory)
            .expect_err("descriptor contradicting period_id must be refused");
        assert!(matches!(
            error,
            StorageError::InvalidKpiIngestRunValue {
                key: "period_descriptor",
                ..
            }
        ));

        // A second create on the SAME active triple ("p1") with a DIFFERENT
        // descriptor conflicts with the already-running one.
        let mut conflicting = new_run("doc1", "c1", "p1");
        conflicting.period_fiscal_year = Some(2024);
        conflicting.period_type = Some("FY".to_owned());
        let error = store
            .create_run_if_absent(&conflicting)
            .expect_err("a different descriptor on the same active triple must conflict");
        assert!(matches!(
            error,
            StorageError::RunPeriodConflict { id } if id == created.id
        ));

        // The SAME descriptor on the same active triple stays idempotent.
        let same_again = store
            .create_run_if_absent(&with_descriptor)
            .expect("the identical descriptor is not a conflict");
        assert_eq!(same_again.id, created.id);
    }
}
