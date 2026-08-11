//! KPI ingest run domain store (ADR 0098 decisions 2, 6, 8; epic #352, cards
//! #358/#360). `kpi_ingest_runs` is the external agent's durable worklist and
//! lease/heartbeat holder (dec. 8, model A): one row per (report document,
//! company, extraction profile). The closed 11-state lifecycle (dec. 6) is
//! enforced by the DB `CHECK`, the [`KpiIngestRunState`] enum, and the
//! exhaustive [`KpiIngestRunState::can_transition`] match (#360) — every
//! production write to `status` goes through a guarded `UPDATE … WHERE
//! id=? AND status IN (…from)` plus a deterministic 0-row classifier (never a
//! pre-check outside the transaction — TOCTOU). Reach it via
//! `AppState::kpi_ingest_runs()`.

use super::database::Database;
use super::kpi_ingest_staging::get_commit_receipt_on_connection;
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

    /// The closed transition table (ADR 0098 dec. 6, #360 — normative,
    /// mirrored verbatim in `docs/data-model.md` § KPI Ingest Runs). ONE
    /// exhaustive `match` over `self` with no wildcard arm: the compiler
    /// forces every one of the 11 states to be covered, so a 12th state
    /// added later fails to compile here until this table is updated.
    /// Terminal recovery mid-`committing` (crash reclaim) is legal
    /// (`Committing -> ReadyToCommit`) even though it is driven by
    /// `reclaim_ingest_runs_on_startup`, not a caller-facing method.
    pub fn can_transition(self, to: Self) -> bool {
        use KpiIngestRunState as S;
        match self {
            S::Discovered => matches!(to, S::SourceCaptured | S::Cancelled | S::Failed),
            S::SourceCaptured => matches!(to, S::Extracting | S::Cancelled | S::Failed),
            S::Extracting => matches!(to, S::Staged | S::Cancelled | S::Failed),
            S::Staged => matches!(
                to,
                S::ValidationFailed | S::ReadyToCommit | S::Cancelled | S::Failed
            ),
            S::ValidationFailed => matches!(to, S::Staged | S::Cancelled | S::Failed),
            S::ReadyToCommit => matches!(to, S::Staged | S::Committing | S::Cancelled | S::Failed),
            S::Committing => matches!(to, S::Complete | S::Partial | S::ReadyToCommit),
            S::Complete | S::Partial | S::Failed | S::Cancelled => false,
        }
    }
}

/// Pre-commit, non-terminal states legal as the `from` side of `cancel_run`
/// and `mark_failed` (ADR 0098 dec. 6 transition table): every state up to
/// and including `ready_to_commit`, excluding `committing` itself — a
/// `committing` transaction resolves ONLY by rollback (retryable) or
/// completion, never by cancellation or exhaustion (dec. 6).
const PRE_COMMIT_STATES: &[KpiIngestRunState] = &[
    KpiIngestRunState::Discovered,
    KpiIngestRunState::SourceCaptured,
    KpiIngestRunState::Extracting,
    KpiIngestRunState::Staged,
    KpiIngestRunState::ValidationFailed,
    KpiIngestRunState::ReadyToCommit,
];

/// Shared core for every one-row status transition (#360 B1 sol): a guarded
/// `UPDATE … SET status = ?to, {extra_set} … WHERE id = ?id AND status IN
/// (…from){extra_guard}`. Zero rows updated does NOT by itself mean the
/// transition is illegal — the caller re-reads the row (via [`read_raw_run`])
/// and classifies the reason (wrong status vs. a specific missing
/// prerequisite) itself; `apply_transition` never classifies, only executes.
/// `extra_set` must end with `", "` when non-empty; `extra_guard` must start
/// with `" AND "` when non-empty. Anonymous `?` placeholders bind
/// POSITIONALLY in strict left-to-right textual order, so `extra_set_params`
/// and `extra_guard_params` are kept as SEPARATE slices — `extra_set`'s
/// placeholders sit in the SQL text before `id`'s, `extra_guard`'s after —
/// rather than one combined slice a caller could accidentally mis-order.
#[allow(clippy::too_many_arguments)]
fn apply_transition(
    conn: &Connection,
    id: &str,
    from: &[KpiIngestRunState],
    to: KpiIngestRunState,
    extra_set: &str,
    extra_set_params: &[&dyn rusqlite::types::ToSql],
    extra_guard: &str,
    extra_guard_params: &[&dyn rusqlite::types::ToSql],
) -> StorageResult<usize> {
    let from_list = from
        .iter()
        .map(|state| format!("'{}'", state.as_str()))
        .collect::<Vec<_>>()
        .join(", ");
    let sql = format!(
        "UPDATE kpi_ingest_runs SET status = ?, {extra_set}updated_at = strftime('%Y-%m-%dT%H:%M:%fZ', 'now') \
         WHERE id = ? AND status IN ({from_list}){extra_guard}"
    );
    let to_str = to.as_str();
    let mut all_params: Vec<&dyn rusqlite::types::ToSql> =
        Vec::with_capacity(extra_set_params.len() + extra_guard_params.len() + 2);
    all_params.push(&to_str);
    all_params.extend_from_slice(extra_set_params);
    all_params.push(&id);
    all_params.extend_from_slice(extra_guard_params);
    Ok(conn.execute(&sql, all_params.as_slice())?)
}

/// Re-read every column classification needs, on the SAME connection/
/// transaction the failed guarded UPDATE just ran on (no TOCTOU window).
fn read_raw_run(conn: &Connection, id: &str) -> StorageResult<Option<RawRun>> {
    conn.query_row(
        &format!("SELECT {RUN_COLUMNS} FROM kpi_ingest_runs WHERE id = ?1"),
        [id],
        map_raw_row,
    )
    .optional()
    .map_err(StorageError::from)
}

/// Whether `holder` currently owns a LIVE lease on `id` — computed in SQL
/// (never in Rust) so the liveness comparison against `now` shares the exact
/// same clock/format `claim_next`/`heartbeat` use.
fn lease_is_live_on_connection(conn: &Connection, id: &str, holder: &str) -> StorageResult<bool> {
    conn.query_row(
        "SELECT lease_holder = ?2 AND lease_expires_at > strftime('%Y-%m-%dT%H:%M:%fZ', 'now') \
         FROM kpi_ingest_runs WHERE id = ?1",
        params![id, holder],
        |row| row.get(0),
    )
    .map_err(StorageError::from)
}

/// The three lease-failure scenarios (wrong holder / expired / absent) all
/// collapse to the SAME typed refusal — `RunLeaseNotHeld` — across every
/// agent-facing intent (#360 F9 r2 shared corpus).
fn lease_not_held(id: &str, holder: &str) -> StorageError {
    StorageError::RunLeaseNotHeld {
        id: id.to_owned(),
        holder: holder.to_owned(),
    }
}

fn wrong_state(id: &str, actual: KpiIngestRunState, to: KpiIngestRunState) -> StorageError {
    StorageError::InvalidRunTransition {
        id: id.to_owned(),
        from: actual.as_str().to_owned(),
        to: to.as_str().to_owned(),
    }
}

/// Every `committing` row currently holding a non-null lease — the
/// structural invariant the transition INTO `committing` must uphold
/// (`begin_committing` clears it atomically as part of the same UPDATE that
/// sets the status). Connection-level (#360 B3 sol): callable under an
/// externally-owned transaction so `reclaim_ingest_runs_on_startup` can check
/// this FIRST, on the SAME handle, before repairing anything else.
fn committing_lease_violations_on_connection(
    conn: &Connection,
) -> StorageResult<Vec<(String, String)>> {
    let mut statement = conn.prepare(
        "SELECT id, status FROM kpi_ingest_runs WHERE status = 'committing' AND lease_holder IS NOT NULL",
    )?;
    let rows = statement
        .query_map([], |row| Ok((row.get(0)?, row.get(1)?)))?
        .collect::<Result<_, _>>()?;
    Ok(rows)
}

/// Clear every expired lease on a claimable-status row — the bare UPDATE,
/// with NO invariant guard (the caller is responsible for that: the public
/// [`KpiIngestRunsStore::reclaim_expired_leases`] checks
/// [`committing_lease_violations_on_connection`] itself first and refuses on
/// a hit; `reclaim_ingest_runs_on_startup` performs that same check as its
/// own step 1 and must not re-trip it here). Connection-level (#360 B3 sol).
fn clear_expired_leases_on_connection(conn: &Connection) -> StorageResult<usize> {
    let sql = format!(
        "UPDATE kpi_ingest_runs
         SET lease_holder = NULL, lease_expires_at = NULL, last_heartbeat_at = NULL,
             updated_at = strftime('%Y-%m-%dT%H:%M:%fZ', 'now')
         WHERE status IN {CLAIMABLE_STATUSES_SQL}
           AND lease_expires_at IS NOT NULL
           AND lease_expires_at <= strftime('%Y-%m-%dT%H:%M:%fZ', 'now')"
    );
    Ok(conn.execute(&sql, [])?)
}

/// Startup-reclaim outcome counters (#360). `is_noop` lets `lib.rs` log at
/// `info!` only when there was something to report — a cold/steady-state
/// start stays silent.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct ReclaimSummary {
    /// `committing` rows finalized to `complete`/`partial` from a matching receipt.
    pub finalized: usize,
    /// `committing` rows without a receipt, reverted to `ready_to_commit`.
    pub reverted: usize,
    /// Expired leases cleared on claimable-status rows.
    pub lease_cleared: usize,
    /// `committing` rows found holding a non-null lease — a structural
    /// invariant violation, reported but left UNTOUCHED (never auto-repaired).
    pub violations: usize,
}

impl ReclaimSummary {
    pub fn is_noop(&self) -> bool {
        self.finalized == 0 && self.reverted == 0 && self.lease_cleared == 0 && self.violations == 0
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
            // genuine conflict (B2 sol round 3), never a silent idempotent
            // return. Both representations — a period_id and a descriptor pair
            // — resolve to the SAME natural key (fiscal_year, period_type)
            // before comparing, so a mixed-representation mismatch (existing
            // period_id vs requested descriptor, or the reverse) is caught too
            // (luna review P1). A side that supplies neither stays None and
            // never conflicts: filling in a previously-absent period is legal.
            let resolve_natural_key = |period_id: Option<&str>,
                                       year: Option<i64>,
                                       period_type: Option<&str>|
             -> StorageResult<Option<(i64, String)>> {
                if let Some(pid) = period_id {
                    let row: Option<(i64, String)> = tx
                        .query_row(
                            "SELECT fiscal_year, period_type FROM financial_periods WHERE id = ?1",
                            [pid],
                            |row| Ok((row.get(0)?, row.get(1)?)),
                        )
                        .optional()?;
                    return Ok(row);
                }
                Ok(match (year, period_type) {
                    (Some(year), Some(kind)) => Some((year, kind.to_owned())),
                    _ => None,
                })
            };
            let existing_key = resolve_natural_key(
                raw.period_id.as_deref(),
                raw.period_fiscal_year,
                raw.period_type.as_deref(),
            )?;
            let requested_key = resolve_natural_key(
                new_run.period_id.as_deref(),
                new_run.period_fiscal_year,
                new_run.period_type.as_deref(),
            )?;
            if let (Some(existing_key), Some(requested_key)) = (existing_key, requested_key) {
                if existing_key != requested_key {
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

    /// `discovered -> source_captured` (#360; replaces #359's lease-free
    /// `record_source_capture` — F6 sol: no production caller ever needed a
    /// lease-free write). Hash and status flip ATOMICALLY in ONE guarded
    /// UPDATE requiring a live lease for `holder`. Set-once semantics survive
    /// past the edge: the SAME hash again (run already `source_captured` or
    /// later) is a no-op; a DIFFERENT hash is `RunSourceHashAlreadyRecorded`.
    pub fn mark_source_captured(
        &self,
        id: &str,
        holder: &str,
        source_content_hash: &str,
    ) -> StorageResult<()> {
        let mut connection = self.db.checkout()?;
        let tx = connection.transaction_with_behavior(rusqlite::TransactionBehavior::Immediate)?;

        let changed = apply_transition(
            &tx,
            id,
            &[KpiIngestRunState::Discovered],
            KpiIngestRunState::SourceCaptured,
            "source_content_hash = ?, ",
            &[&source_content_hash],
            " AND lease_holder = ? AND lease_expires_at > strftime('%Y-%m-%dT%H:%M:%fZ', 'now')",
            &[&holder],
        )?;
        if changed == 1 {
            tx.commit()?;
            return Ok(());
        }

        let Some(raw) = read_raw_run(&tx, id)? else {
            return Err(StorageError::KpiIngestRunNotFound { id: id.to_owned() });
        };
        let status = KpiIngestRunState::parse(&raw.status)?;
        let result = if status != KpiIngestRunState::Discovered {
            // Already past this edge — set-once idempotency, independent of
            // lease/status from here on (mirrors #359's `record_source_capture`).
            match raw.source_content_hash.as_deref() {
                Some(existing) if existing == source_content_hash => Ok(()),
                Some(_) => Err(StorageError::RunSourceHashAlreadyRecorded { id: id.to_owned() }),
                None => Err(wrong_state(id, status, KpiIngestRunState::SourceCaptured)),
            }
        } else {
            Err(lease_not_held(id, holder))
        };
        tx.commit()?;
        result
    }

    /// `source_captured -> extracting` (#360). Requires a live lease for
    /// `holder`; writes `instruction_version` atomically with the status
    /// flip. Prerequisites — `scope`, `data_quality`, and a period identity
    /// (`period_id` or a complete descriptor) — are enforced IN the guarded
    /// UPDATE's WHERE clause (never a pre-check outside the transaction), so
    /// a 0-row result needs re-read classification to tell "wrong state" from
    /// "which specific prerequisite is missing".
    pub fn begin_extracting(
        &self,
        id: &str,
        holder: &str,
        instruction_version: &str,
    ) -> StorageResult<()> {
        let mut connection = self.db.checkout()?;
        let tx = connection.transaction_with_behavior(rusqlite::TransactionBehavior::Immediate)?;

        let changed = apply_transition(
            &tx,
            id,
            &[KpiIngestRunState::SourceCaptured],
            KpiIngestRunState::Extracting,
            "instruction_version = ?, ",
            &[&instruction_version],
            " AND lease_holder = ? AND lease_expires_at > strftime('%Y-%m-%dT%H:%M:%fZ', 'now') \
              AND scope IS NOT NULL AND data_quality IS NOT NULL \
              AND (period_id IS NOT NULL OR (period_fiscal_year IS NOT NULL AND period_type IS NOT NULL))",
            &[&holder],
        )?;
        if changed == 1 {
            tx.commit()?;
            return Ok(());
        }

        let Some(raw) = read_raw_run(&tx, id)? else {
            return Err(StorageError::KpiIngestRunNotFound { id: id.to_owned() });
        };
        let status = KpiIngestRunState::parse(&raw.status)?;
        let result = if status != KpiIngestRunState::SourceCaptured {
            Err(wrong_state(id, status, KpiIngestRunState::Extracting))
        } else if raw.lease_holder.as_deref() != Some(holder)
            || !lease_is_live_on_connection(&tx, id, holder)?
        {
            Err(lease_not_held(id, holder))
        } else if raw.scope.is_none() {
            Err(StorageError::RunTransitionPrerequisiteMissing {
                id: id.to_owned(),
                requirement: "scope",
            })
        } else if raw.data_quality.is_none() {
            Err(StorageError::RunTransitionPrerequisiteMissing {
                id: id.to_owned(),
                requirement: "data_quality",
            })
        } else if raw.period_id.is_none()
            && (raw.period_fiscal_year.is_none() || raw.period_type.is_none())
        {
            Err(StorageError::RunTransitionPrerequisiteMissing {
                id: id.to_owned(),
                requirement: "period",
            })
        } else {
            // Every guard condition re-reads as satisfied — a genuine
            // concurrent modification raced the classification window itself
            // (extremely unlikely under `Immediate`). Surface as a lease
            // refusal rather than silently retrying.
            Err(lease_not_held(id, holder))
        };
        tx.commit()?;
        result
    }

    /// `staged -> validation_failed` (#360): the deterministic validator
    /// (#361) rejects the current staging revision. No lease requirement —
    /// the caller is the in-process validator step, never the agent. Targets
    /// the run's CURRENT `manifest_revision` (guards against a race with a
    /// concurrent re-stage bumping it).
    pub fn mark_validation_failed(&self, id: &str, revision: i64) -> StorageResult<()> {
        let mut connection = self.db.checkout()?;
        let tx = connection.transaction_with_behavior(rusqlite::TransactionBehavior::Immediate)?;

        let changed = apply_transition(
            &tx,
            id,
            &[KpiIngestRunState::Staged],
            KpiIngestRunState::ValidationFailed,
            "",
            &[],
            " AND manifest_revision = ?",
            &[&revision],
        )?;
        if changed == 1 {
            tx.commit()?;
            return Ok(());
        }

        let Some(raw) = read_raw_run(&tx, id)? else {
            return Err(StorageError::KpiIngestRunNotFound { id: id.to_owned() });
        };
        let status = KpiIngestRunState::parse(&raw.status)?;
        let result = if status != KpiIngestRunState::Staged {
            Err(wrong_state(id, status, KpiIngestRunState::ValidationFailed))
        } else {
            Err(StorageError::InvalidStagingRevision {
                run_id: id.to_owned(),
                revision,
                reason: "revision is not the run's current staging revision",
            })
        };
        tx.commit()?;
        result
    }

    /// `staged -> ready_to_commit` (#360): the deterministic validator (#361)
    /// accepts the current staging revision and freezes it as a manifest. No
    /// lease requirement. Guard: `manifest_revision = ?revision AND
    /// manifest_hash IS NULL` (the revision must be both current AND never
    /// previously frozen). SETs `manifest_hash` and clears all three lease
    /// columns ATOMICALLY (ADR 0098 dec. 6 — a `ready_to_commit` row never
    /// holds a lease; `begin_committing`'s invariant guard depends on this).
    pub fn mark_ready_to_commit(
        &self,
        id: &str,
        revision: i64,
        manifest_hash: &str,
    ) -> StorageResult<()> {
        let mut connection = self.db.checkout()?;
        let tx = connection.transaction_with_behavior(rusqlite::TransactionBehavior::Immediate)?;

        let changed = apply_transition(
            &tx,
            id,
            &[KpiIngestRunState::Staged],
            KpiIngestRunState::ReadyToCommit,
            "manifest_hash = ?, lease_holder = NULL, lease_expires_at = NULL, last_heartbeat_at = NULL, ",
            &[&manifest_hash],
            " AND manifest_revision = ? AND manifest_hash IS NULL",
            &[&revision],
        )?;
        if changed == 1 {
            tx.commit()?;
            return Ok(());
        }

        let Some(raw) = read_raw_run(&tx, id)? else {
            return Err(StorageError::KpiIngestRunNotFound { id: id.to_owned() });
        };
        let status = KpiIngestRunState::parse(&raw.status)?;
        let result = if status != KpiIngestRunState::Staged {
            Err(wrong_state(id, status, KpiIngestRunState::ReadyToCommit))
        } else if raw.manifest_hash.is_some() {
            Err(StorageError::InvalidStagingRevision {
                run_id: id.to_owned(),
                revision,
                reason: "revision is frozen: a manifest is already issued",
            })
        } else {
            Err(StorageError::InvalidStagingRevision {
                run_id: id.to_owned(),
                revision,
                reason: "revision is not the run's current staging revision",
            })
        };
        tx.commit()?;
        result
    }

    /// `ready_to_commit -> staged` (#360): manifest invalidation. Clears
    /// `manifest_hash`; the staging revision is unchanged (the SAME
    /// observations are still there — only the frozen verdict is discarded).
    /// No lease requirement.
    pub fn invalidate_manifest(&self, id: &str) -> StorageResult<()> {
        let mut connection = self.db.checkout()?;
        let tx = connection.transaction_with_behavior(rusqlite::TransactionBehavior::Immediate)?;

        let changed = apply_transition(
            &tx,
            id,
            &[KpiIngestRunState::ReadyToCommit],
            KpiIngestRunState::Staged,
            "manifest_hash = NULL, ",
            &[],
            "",
            &[],
        )?;
        if changed == 1 {
            tx.commit()?;
            return Ok(());
        }

        let Some(raw) = read_raw_run(&tx, id)? else {
            return Err(StorageError::KpiIngestRunNotFound { id: id.to_owned() });
        };
        let status = KpiIngestRunState::parse(&raw.status)?;
        tx.commit()?;
        Err(wrong_state(id, status, KpiIngestRunState::Staged))
    }

    /// `{discovered, source_captured, extracting, staged, validation_failed,
    /// ready_to_commit} -> cancelled` (#360): system/user-initiated
    /// cancellation from ANY pre-commit state. Releases the lease
    /// unconditionally (a no-op SET when none is held). Never legal from
    /// `committing` or a terminal state — a `committing` transaction resolves
    /// only by rollback or completion (ADR 0098 dec. 6).
    pub fn cancel_run(&self, id: &str) -> StorageResult<()> {
        let mut connection = self.db.checkout()?;
        let tx = connection.transaction_with_behavior(rusqlite::TransactionBehavior::Immediate)?;

        let changed = apply_transition(
            &tx,
            id,
            PRE_COMMIT_STATES,
            KpiIngestRunState::Cancelled,
            "lease_holder = NULL, lease_expires_at = NULL, last_heartbeat_at = NULL, ",
            &[],
            "",
            &[],
        )?;
        if changed == 1 {
            tx.commit()?;
            return Ok(());
        }

        let Some(raw) = read_raw_run(&tx, id)? else {
            return Err(StorageError::KpiIngestRunNotFound { id: id.to_owned() });
        };
        let status = KpiIngestRunState::parse(&raw.status)?;
        tx.commit()?;
        Err(wrong_state(id, status, KpiIngestRunState::Cancelled))
    }

    /// `{discovered, source_captured, extracting, staged, validation_failed,
    /// ready_to_commit} -> failed` (#360): the system records exhausted
    /// retries (the agent's for the pre-staging states, the validator's for
    /// `staged`, the commit job's for `ready_to_commit` — ADR 0098 dec. 6).
    /// Releases the lease unconditionally. Never legal from `committing`.
    pub fn mark_failed(&self, id: &str, last_error: &str) -> StorageResult<()> {
        let mut connection = self.db.checkout()?;
        let tx = connection.transaction_with_behavior(rusqlite::TransactionBehavior::Immediate)?;

        let changed = apply_transition(
            &tx,
            id,
            PRE_COMMIT_STATES,
            KpiIngestRunState::Failed,
            "last_error = ?, lease_holder = NULL, lease_expires_at = NULL, last_heartbeat_at = NULL, ",
            &[&last_error],
            "",
            &[],
        )?;
        if changed == 1 {
            tx.commit()?;
            return Ok(());
        }

        let Some(raw) = read_raw_run(&tx, id)? else {
            return Err(StorageError::KpiIngestRunNotFound { id: id.to_owned() });
        };
        let status = KpiIngestRunState::parse(&raw.status)?;
        tx.commit()?;
        Err(wrong_state(id, status, KpiIngestRunState::Failed))
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
        if let Some((id, status)) = committing_lease_violations_on_connection(&connection)?
            .into_iter()
            .next()
        {
            return Err(StorageError::RunLeaseInvariantViolation { id, status });
        }
        clear_expired_leases_on_connection(&connection)
    }

    /// Startup self-heal (#360): resolve every `committing` row a crash may
    /// have left behind, then clear ordinary expired leases — ONE `Immediate`
    /// transaction on ONE connection handle (B3 sol), using ONLY
    /// connection-level helpers (`get_commit_receipt_on_connection`,
    /// `clear_expired_leases_on_connection`): the public,
    /// own-connection-checking-out `KpiIngestStagingStore::get_commit_receipt`
    /// / `Self::reclaim_expired_leases` would each try to check out their own
    /// connection and deadlock under this method's `Immediate` write lock.
    ///
    /// Order (normative): (1) the lease invariant FIRST — a `committing` row
    /// holding a non-null lease is reported and left completely UNTOUCHED,
    /// never auto-repaired here (`reclaim_expired_leases` would refuse
    /// outright; this method instead counts it and keeps going for every
    /// OTHER row); (2) every remaining `committing` row (lease already NULL)
    /// is resolved by receipt presence: a receipt matching hash+revision
    /// finalizes to `receipt.terminal_status`; a receipt that does NOT match
    /// leaves the row untouched (uncounted — needs manual intervention, not a
    /// crash-reclaim case); no receipt at all reverts to `ready_to_commit`
    /// with `manifest_hash`/`manifest_revision` preserved (the commit
    /// transaction rolled back — retryable); (3) ordinary expired-lease
    /// clearing on claimable rows (never touches `committing` — the WHERE
    /// clause structurally excludes it, so step (1)'s findings are never
    /// re-tripped here).
    pub fn reclaim_ingest_runs_on_startup(&self) -> StorageResult<ReclaimSummary> {
        let mut connection = self.db.checkout()?;
        let tx = connection.transaction_with_behavior(rusqlite::TransactionBehavior::Immediate)?;
        let mut summary = ReclaimSummary {
            violations: committing_lease_violations_on_connection(&tx)?.len(),
            ..Default::default()
        };

        let mut statement = tx.prepare(
            "SELECT id, manifest_hash, manifest_revision FROM kpi_ingest_runs
             WHERE status = 'committing' AND lease_holder IS NULL",
        )?;
        let committing_rows: Vec<(String, Option<String>, i64)> = statement
            .query_map([], |row| Ok((row.get(0)?, row.get(1)?, row.get(2)?)))?
            .collect::<Result<_, _>>()?;
        drop(statement);

        for (id, manifest_hash, manifest_revision) in committing_rows {
            match get_commit_receipt_on_connection(&tx, &id)? {
                Some(receipt)
                    if Some(receipt.manifest_hash.as_str()) == manifest_hash.as_deref()
                        && receipt.manifest_revision == manifest_revision =>
                {
                    let terminal = match receipt.terminal_status.as_str() {
                        "complete" => KpiIngestRunState::Complete,
                        "partial" => KpiIngestRunState::Partial,
                        other => {
                            return Err(StorageError::UnknownKpiIngestRunState {
                                value: other.to_owned(),
                            })
                        }
                    };
                    tx.execute(
                        "UPDATE kpi_ingest_runs SET status = ?1, \
                         updated_at = strftime('%Y-%m-%dT%H:%M:%fZ', 'now') WHERE id = ?2",
                        params![terminal.as_str(), id],
                    )?;
                    summary.finalized += 1;
                }
                // A receipt exists but disagrees with the run's manifest — left
                // untouched, uncounted: this needs manual investigation, it is
                // not the ordinary crash-recovery case.
                Some(_) => {}
                None => {
                    tx.execute(
                        "UPDATE kpi_ingest_runs SET status = 'ready_to_commit', \
                         updated_at = strftime('%Y-%m-%dT%H:%M:%fZ', 'now') WHERE id = ?1",
                        [&id],
                    )?;
                    summary.reverted += 1;
                }
            }
        }

        summary.lease_cleared = clear_expired_leases_on_connection(&tx)?;
        tx.commit()?;
        Ok(summary)
    }
}

/// `ready_to_commit -> committing` (#360, ADR 0098 dec. 6/5): connection-level
/// free fn for #363's commit transaction to call under its own
/// externally-owned `&Connection` (the `record_structured_fact` pattern) —
/// composing the PUBLIC, connection-checking-out store methods inside an
/// outer transaction is prohibited (ADR 0098 dec. 5). Guard: status =
/// `ready_to_commit`, `manifest_hash`/`manifest_revision` match the caller's
/// (mismatch -> `StaleManifestForCommit`), lease is NULL (ADR 0098 dec. 6
/// invariant: `mark_ready_to_commit` clears it; a non-null lease here is a
/// structural bug, `RunLeaseInvariantViolation`, checked FIRST).
// ponytail: unwired until #363's commit transaction calls this under its own
// externally-owned transaction (ADR 0098 dec. 5) — #360 ships the primitive
// plus its composition/rollback tests per the approved plan, so it is
// exercised only from #[cfg(test)] until then (same situation as
// `kpi_ingest_staging::record_commit_receipt`, #359).
#[allow(dead_code)]
pub(super) fn begin_committing(
    conn: &Connection,
    id: &str,
    manifest_hash: &str,
    revision: i64,
) -> StorageResult<()> {
    let changed = apply_transition(
        conn,
        id,
        &[KpiIngestRunState::ReadyToCommit],
        KpiIngestRunState::Committing,
        "",
        &[],
        " AND manifest_hash = ? AND manifest_revision = ? AND lease_holder IS NULL",
        &[&manifest_hash, &revision],
    )?;
    if changed == 1 {
        return Ok(());
    }

    let Some(raw) = read_raw_run(conn, id)? else {
        return Err(StorageError::KpiIngestRunNotFound { id: id.to_owned() });
    };
    let status = KpiIngestRunState::parse(&raw.status)?;
    if status == KpiIngestRunState::ReadyToCommit && raw.lease_holder.is_some() {
        return Err(StorageError::RunLeaseInvariantViolation {
            id: id.to_owned(),
            status: raw.status,
        });
    }
    if status != KpiIngestRunState::ReadyToCommit {
        return Err(wrong_state(id, status, KpiIngestRunState::Committing));
    }
    // status is ready_to_commit, lease is NULL: the guard's remaining
    // condition — hash/revision — must be what failed.
    Err(StorageError::StaleManifestForCommit { id: id.to_owned() })
}

/// `committing -> complete | partial` (#360, ADR 0098 dec. 5/6): connection-
/// level free fn for #363's commit transaction, called on the SAME handle
/// AFTER it writes the commit receipt (`kpi_ingest_staging::
/// record_commit_receipt`) in the same outer transaction. Deliberately takes
/// NO terminal-status parameter (B2 sol — a structural gate, not a
/// doc-comment promise): it reads and verifies the receipt ITSELF —
/// `manifest_hash`/`manifest_revision` must match the run row — and derives
/// `complete`/`partial` from `receipt.terminal_status`. No receipt yet ->
/// `RunTransitionPrerequisiteMissing`; a receipt that disagrees with the run
/// -> `StaleManifestForCommit`. Returns the terminal state it set.
// ponytail: same #363-unwired situation as `begin_committing` above.
#[allow(dead_code)]
pub(super) fn finalize_committing(conn: &Connection, id: &str) -> StorageResult<KpiIngestRunState> {
    let Some(raw) = read_raw_run(conn, id)? else {
        return Err(StorageError::KpiIngestRunNotFound { id: id.to_owned() });
    };
    let status = KpiIngestRunState::parse(&raw.status)?;
    if status != KpiIngestRunState::Committing {
        return Err(wrong_state(id, status, KpiIngestRunState::Complete));
    }

    let Some(receipt) = get_commit_receipt_on_connection(conn, id)? else {
        return Err(StorageError::RunTransitionPrerequisiteMissing {
            id: id.to_owned(),
            requirement: "commit_receipt",
        });
    };
    if Some(receipt.manifest_hash.as_str()) != raw.manifest_hash.as_deref()
        || receipt.manifest_revision != raw.manifest_revision
    {
        return Err(StorageError::StaleManifestForCommit { id: id.to_owned() });
    }
    let terminal = match receipt.terminal_status.as_str() {
        "complete" => KpiIngestRunState::Complete,
        "partial" => KpiIngestRunState::Partial,
        other => {
            return Err(StorageError::UnknownKpiIngestRunState {
                value: other.to_owned(),
            })
        }
    };

    let changed = conn.execute(
        "UPDATE kpi_ingest_runs SET status = ?1, \
         updated_at = strftime('%Y-%m-%dT%H:%M:%fZ', 'now') WHERE id = ?2 AND status = 'committing'",
        params![terminal.as_str(), id],
    )?;
    if changed == 0 {
        // Raced between the read above and this UPDATE within what must be
        // the caller's own transaction — surface rather than silently retry.
        return Err(wrong_state(id, status, terminal));
    }
    Ok(terminal)
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

    /// #360 F6 sol: `mark_source_captured` replaces the lease-free
    /// `record_source_capture` (#359) — hash and status write ATOMICALLY in
    /// one guarded UPDATE, requiring a live lease for `holder`. Set-once
    /// semantics survive: the same hash again (now past the `discovered`
    /// edge) is a no-op; a different hash is refused.
    #[test]
    fn mark_source_captured_is_set_once_and_atomic_with_the_transition() {
        let (state, doc, company) = setup_one_company_doc();
        let store = state.kpi_ingest_runs();
        let run = store
            .create_run_if_absent(&new_run(doc, company, "p1"))
            .expect("create");
        store.claim_next("worker-a", 3600).expect("claim");

        store
            .mark_source_captured(&run.id, "worker-a", "hash1")
            .expect("first capture");
        let after = store.get_run(&run.id).expect("get").expect("some");
        assert_eq!(after.source_content_hash.as_deref(), Some("hash1"));
        assert_eq!(after.status, KpiIngestRunState::SourceCaptured);

        store
            .mark_source_captured(&run.id, "worker-a", "hash1")
            .expect("same hash again is a no-op, even past the discovered edge");

        let error = store
            .mark_source_captured(&run.id, "worker-a", "hash2")
            .expect_err("a different hash must be refused");
        assert!(matches!(
            error,
            StorageError::RunSourceHashAlreadyRecorded { .. }
        ));

        let error = store
            .mark_source_captured("kpiing_missing", "worker-a", "hash1")
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
    fn mark_source_captured_two_threads_one_hash_survives() {
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
        // Both racers share the SAME live lease (F6 sol: mark_source_captured
        // now requires one) — the race is on the guarded status transition,
        // not on lease ownership.
        store.claim_next("worker-a", 3600).expect("claim");

        let store_a = store.clone();
        let store_b = store.clone();
        let a =
            std::thread::spawn(move || store_a.mark_source_captured("run1", "worker-a", "hash-a"));
        let b =
            std::thread::spawn(move || store_b.mark_source_captured("run1", "worker-a", "hash-b"));
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

    /// Mixed representations must resolve to the natural key before comparing
    /// (luna review P1): an existing run holding a period_id conflicts with a
    /// requested DESCRIPTOR naming a different period, and vice versa.
    #[test]
    fn create_run_period_conflict_across_mixed_representations() {
        let connection = open_in_memory_database().expect("db");
        seed_company(&connection, "c1");
        seed_document(&connection, "doc1", "c1");
        seed_document(&connection, "doc2", "c1");
        connection
            .execute(
                "INSERT INTO financial_periods (id, company_id, fiscal_year, period_type)
                 VALUES ('finper_fy2025', 'c1', 2025, 'FY')",
                [],
            )
            .expect("seed period");
        let state = AppState::new(connection);
        let store = state.kpi_ingest_runs();

        // Existing run holds a period_id (FY2025); request carries only a
        // DESCRIPTOR for FY2024 — must conflict, not silently reuse.
        let mut with_id = new_run("doc1", "c1", "p1");
        with_id.period_id = Some("finper_fy2025".to_owned());
        let created = store.create_run_if_absent(&with_id).expect("create");
        let mut descriptor_mismatch = new_run("doc1", "c1", "p1");
        descriptor_mismatch.period_fiscal_year = Some(2024);
        descriptor_mismatch.period_type = Some("FY".to_owned());
        let error = store
            .create_run_if_absent(&descriptor_mismatch)
            .expect_err("descriptor naming another period must conflict with the stored period_id");
        assert!(matches!(
            error,
            StorageError::RunPeriodConflict { id } if id == created.id
        ));

        // The matching descriptor for the SAME period is idempotent.
        let mut descriptor_match = new_run("doc1", "c1", "p1");
        descriptor_match.period_fiscal_year = Some(2025);
        descriptor_match.period_type = Some("FY".to_owned());
        let same = store
            .create_run_if_absent(&descriptor_match)
            .expect("the matching descriptor is not a conflict");
        assert_eq!(same.id, created.id);

        // Reverse direction: existing run holds a descriptor; request carries
        // a period_id resolving to a different natural key — must conflict.
        let mut with_descriptor = new_run("doc2", "c1", "p1");
        with_descriptor.period_fiscal_year = Some(2024);
        with_descriptor.period_type = Some("FY".to_owned());
        let created2 = store
            .create_run_if_absent(&with_descriptor)
            .expect("create");
        let mut id_mismatch = new_run("doc2", "c1", "p1");
        id_mismatch.period_id = Some("finper_fy2025".to_owned());
        let error = store
            .create_run_if_absent(&id_mismatch)
            .expect_err("a period_id resolving elsewhere must conflict with the stored descriptor");
        assert!(matches!(
            error,
            StorageError::RunPeriodConflict { id } if id == created2.id
        ));
    }

    // ==== #360: closed state machine ========================================

    fn ready_new_run(doc: &str, company: &str, profile: &str) -> NewKpiIngestRun {
        let mut run = new_run(doc, company, profile);
        run.scope = Some("standalone".to_owned());
        run.data_quality = Some("final".to_owned());
        run.period_fiscal_year = Some(2025);
        run.period_type = Some("FY".to_owned());
        run
    }

    fn advance_to_extracting(
        state: &AppState,
        doc: &str,
        company: &str,
        holder: &str,
    ) -> KpiIngestRun {
        let store = state.kpi_ingest_runs();
        let run = store
            .create_run_if_absent(&ready_new_run(doc, company, "p1"))
            .expect("create");
        store.claim_next(holder, 3600).expect("claim");
        store
            .mark_source_captured(&run.id, holder, "hash1")
            .expect("capture");
        store
            .begin_extracting(&run.id, holder, "instr-1")
            .expect("begin extracting");
        store.get_run(&run.id).expect("get").expect("some")
    }

    fn one_test_observation() -> NewStagedObservation {
        NewStagedObservation {
            raw_label: "l".to_owned(),
            raw_value: "v".to_owned(),
            ..Default::default()
        }
    }

    fn stage_once(state: &AppState, run_id: &str, holder: &str) -> i64 {
        let (revision, _) = state
            .kpi_ingest_staging()
            .stage_observations(run_id, holder, vec![one_test_observation()])
            .expect("stage");
        revision
    }

    fn advance_to_ready_to_commit(
        state: &AppState,
        doc: &str,
        company: &str,
        holder: &str,
        manifest_hash: &str,
    ) -> KpiIngestRun {
        let run = advance_to_extracting(state, doc, company, holder);
        let revision = stage_once(state, &run.id, holder);
        let store = state.kpi_ingest_runs();
        store
            .mark_ready_to_commit(&run.id, revision, manifest_hash)
            .expect("ready");
        store.get_run(&run.id).expect("get").expect("some")
    }

    /// Test 1 (#360 plan): 11x11 transition matrix against an INDEPENDENT
    /// hard-coded oracle — deliberately NOT derived from `can_transition`
    /// itself, mirroring `docs/data-model.md` § KPI Ingest Runs exactly. If
    /// the two drift, this test (not `can_transition`'s own logic) must catch
    /// it.
    #[test]
    fn can_transition_matches_the_independent_legal_pairs_oracle() {
        use KpiIngestRunState::{
            Cancelled, Committing, Complete, Discovered, Extracting, Failed, Partial,
            ReadyToCommit, SourceCaptured, Staged, ValidationFailed,
        };
        const LEGAL_PAIRS: &[(KpiIngestRunState, KpiIngestRunState)] = &[
            (Discovered, SourceCaptured),
            (Discovered, Cancelled),
            (Discovered, Failed),
            (SourceCaptured, Extracting),
            (SourceCaptured, Cancelled),
            (SourceCaptured, Failed),
            (Extracting, Staged),
            (Extracting, Cancelled),
            (Extracting, Failed),
            (Staged, ValidationFailed),
            (Staged, ReadyToCommit),
            (Staged, Cancelled),
            (Staged, Failed),
            (ValidationFailed, Staged),
            (ValidationFailed, Cancelled),
            (ValidationFailed, Failed),
            (ReadyToCommit, Staged),
            (ReadyToCommit, Committing),
            (ReadyToCommit, Cancelled),
            (ReadyToCommit, Failed),
            (Committing, Complete),
            (Committing, Partial),
            (Committing, ReadyToCommit),
        ];
        const ALL_STATES: &[KpiIngestRunState] = &[
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
        ];
        for &from in ALL_STATES {
            for &to in ALL_STATES {
                let expected = LEGAL_PAIRS.contains(&(from, to));
                assert_eq!(
                    from.can_transition(to),
                    expected,
                    "can_transition({from:?}, {to:?}) should be {expected}"
                );
            }
        }
    }

    // --- Test 2: prerequisites ----------------------------------------------

    #[test]
    fn begin_extracting_happy_path_writes_instruction_version_atomically() {
        let (state, doc, company) = setup_one_company_doc();
        let store = state.kpi_ingest_runs();
        let run = store
            .create_run_if_absent(&ready_new_run(doc, company, "p1"))
            .expect("create");
        store.claim_next("worker-a", 3600).expect("claim");
        store
            .mark_source_captured(&run.id, "worker-a", "hash1")
            .expect("capture");

        store
            .begin_extracting(&run.id, "worker-a", "instr-1")
            .expect("begin extracting");
        let after = store.get_run(&run.id).expect("get").expect("some");
        assert_eq!(after.status, KpiIngestRunState::Extracting);
        assert_eq!(after.instruction_version.as_deref(), Some("instr-1"));
    }

    #[test]
    fn begin_extracting_refuses_each_missing_prerequisite_independently() {
        type PrereqCase = (&'static str, fn(&mut NewKpiIngestRun));
        let cases: &[PrereqCase] = &[
            ("scope", |run| {
                run.data_quality = Some("final".to_owned());
                run.period_fiscal_year = Some(2025);
                run.period_type = Some("FY".to_owned());
            }),
            ("data_quality", |run| {
                run.scope = Some("standalone".to_owned());
                run.period_fiscal_year = Some(2025);
                run.period_type = Some("FY".to_owned());
            }),
            ("period", |run| {
                run.scope = Some("standalone".to_owned());
                run.data_quality = Some("final".to_owned());
            }),
        ];
        for (requirement, build) in cases {
            let (state, doc, company) = setup_one_company_doc();
            let store = state.kpi_ingest_runs();
            let mut run_def = new_run(doc, company, "p1");
            build(&mut run_def);
            let run = store.create_run_if_absent(&run_def).expect("create");
            store.claim_next("worker-a", 3600).expect("claim");
            store
                .mark_source_captured(&run.id, "worker-a", "hash1")
                .expect("capture");
            let error = store
                .begin_extracting(&run.id, "worker-a", "instr-1")
                .expect_err(&format!("{requirement} missing must refuse"));
            assert!(
                matches!(
                    &error,
                    StorageError::RunTransitionPrerequisiteMissing { requirement: r, .. } if r == requirement
                ),
                "expected missing '{requirement}', got {error:?}"
            );
        }
    }

    #[test]
    fn begin_extracting_refuses_wrong_state_with_unchanged_snapshot() {
        let (state, doc, company) = setup_one_company_doc();
        let store = state.kpi_ingest_runs();
        let run = store
            .create_run_if_absent(&ready_new_run(doc, company, "p1"))
            .expect("create");
        let before = store.get_run(&run.id).expect("get").expect("some");
        let error = store
            .begin_extracting(&run.id, "worker-a", "instr-1")
            .expect_err("still discovered");
        assert!(matches!(
            error,
            StorageError::InvalidRunTransition { ref from, .. } if from == "discovered"
        ));
        assert_eq!(
            before,
            store.get_run(&run.id).expect("get").expect("some"),
            "a refused transition must not mutate the row"
        );
    }

    #[test]
    fn mark_validation_failed_happy_path_targets_the_current_revision() {
        let (state, doc, company) = setup_one_company_doc();
        let run = advance_to_extracting(&state, doc, company, "worker-a");
        let revision = stage_once(&state, &run.id, "worker-a");

        let store = state.kpi_ingest_runs();
        store
            .mark_validation_failed(&run.id, revision)
            .expect("mark failed");
        let after = store.get_run(&run.id).expect("get").expect("some");
        assert_eq!(after.status, KpiIngestRunState::ValidationFailed);
    }

    #[test]
    fn mark_validation_failed_refuses_a_stale_revision() {
        let (state, doc, company) = setup_one_company_doc();
        let run = advance_to_extracting(&state, doc, company, "worker-a");
        let revision = stage_once(&state, &run.id, "worker-a");

        let error = state
            .kpi_ingest_runs()
            .mark_validation_failed(&run.id, revision + 1)
            .expect_err("stale revision");
        assert!(matches!(error, StorageError::InvalidStagingRevision { .. }));
    }

    #[test]
    fn mark_validation_failed_refuses_wrong_state_with_unchanged_snapshot() {
        let (state, doc, company) = setup_one_company_doc();
        let store = state.kpi_ingest_runs();
        let run = store
            .create_run_if_absent(&ready_new_run(doc, company, "p1"))
            .expect("create");
        let before = store.get_run(&run.id).expect("get").expect("some");
        let error = store
            .mark_validation_failed(&run.id, 0)
            .expect_err("still discovered");
        assert!(matches!(error, StorageError::InvalidRunTransition { .. }));
        assert_eq!(before, store.get_run(&run.id).expect("get").expect("some"));
    }

    #[test]
    fn mark_ready_to_commit_happy_path_freezes_manifest_and_clears_lease() {
        let (state, doc, company) = setup_one_company_doc();
        let run = advance_to_extracting(&state, doc, company, "worker-a");
        let revision = stage_once(&state, &run.id, "worker-a");

        let store = state.kpi_ingest_runs();
        store
            .mark_ready_to_commit(&run.id, revision, "hash-abc")
            .expect("ready");
        let after = store.get_run(&run.id).expect("get").expect("some");
        assert_eq!(after.status, KpiIngestRunState::ReadyToCommit);
        assert_eq!(after.manifest_hash.as_deref(), Some("hash-abc"));
        assert!(after.lease_holder.is_none());
        assert!(after.lease_expires_at.is_none());
        assert!(after.last_heartbeat_at.is_none());
    }

    #[test]
    fn mark_ready_to_commit_refuses_a_stale_revision() {
        let (state, doc, company) = setup_one_company_doc();
        let run = advance_to_extracting(&state, doc, company, "worker-a");
        let revision = stage_once(&state, &run.id, "worker-a");

        let error = state
            .kpi_ingest_runs()
            .mark_ready_to_commit(&run.id, revision + 1, "hash-abc")
            .expect_err("stale revision");
        assert!(matches!(error, StorageError::InvalidStagingRevision { .. }));
    }

    /// Structurally unreachable via any production seam (`mark_ready_to_commit`'s
    /// own guard requires `manifest_hash IS NULL` and moves status off `staged`
    /// on success) — raw-seeded defensively, the same carve-out
    /// `kpi_ingest_staging`'s frozen-revision coverage uses.
    #[test]
    fn mark_ready_to_commit_refuses_when_a_manifest_is_already_issued() {
        let connection = open_in_memory_database().expect("db");
        seed_company(&connection, "c1");
        seed_document(&connection, "doc1", "c1");
        seed_run_raw(
            &connection,
            "run1",
            "doc1",
            "c1",
            "p1",
            "staged",
            None,
            None,
            None,
        );
        connection
            .execute(
                "UPDATE kpi_ingest_runs SET manifest_revision = 1, manifest_hash = 'already' WHERE id = 'run1'",
                [],
            )
            .expect("freeze");
        let state = AppState::new(connection);
        let error = state
            .kpi_ingest_runs()
            .mark_ready_to_commit("run1", 1, "new-hash")
            .expect_err("already issued");
        assert!(matches!(error, StorageError::InvalidStagingRevision { .. }));
    }

    #[test]
    fn invalidate_manifest_clears_hash_and_returns_to_staged() {
        let (state, doc, company) = setup_one_company_doc();
        let run = advance_to_extracting(&state, doc, company, "worker-a");
        let revision = stage_once(&state, &run.id, "worker-a");
        let store = state.kpi_ingest_runs();
        store
            .mark_ready_to_commit(&run.id, revision, "hash-abc")
            .expect("ready");

        store.invalidate_manifest(&run.id).expect("invalidate");
        let after = store.get_run(&run.id).expect("get").expect("some");
        assert_eq!(after.status, KpiIngestRunState::Staged);
        assert!(after.manifest_hash.is_none());
        assert_eq!(
            after.manifest_revision, revision,
            "invalidation never changes the revision"
        );
    }

    #[test]
    fn invalidate_manifest_refuses_wrong_state_with_unchanged_snapshot() {
        let (state, doc, company) = setup_one_company_doc();
        let store = state.kpi_ingest_runs();
        let run = store
            .create_run_if_absent(&ready_new_run(doc, company, "p1"))
            .expect("create");
        let before = store.get_run(&run.id).expect("get").expect("some");
        let error = store
            .invalidate_manifest(&run.id)
            .expect_err("still discovered");
        assert!(matches!(error, StorageError::InvalidRunTransition { .. }));
        assert_eq!(before, store.get_run(&run.id).expect("get").expect("some"));
    }

    // --- Test 3: caller/lease — shared table-driven corpus for the three
    // agent-facing intents (F9 r2) ---------------------------------------

    #[test]
    fn agent_intents_share_the_same_three_lease_failure_scenarios() {
        #[derive(Clone, Copy)]
        enum Scenario {
            WrongHolder,
            Expired,
            NoLease,
        }
        fn apply(state: &AppState, run_id: &str, scenario: Scenario, correct_holder: &str) {
            let connection = state.checkout_for_tests().expect("raw connection");
            match scenario {
                Scenario::WrongHolder => connection
                    .execute(
                        "UPDATE kpi_ingest_runs SET lease_holder = 'someone-else', \
                         lease_expires_at = '2999-01-01T00:00:00.000Z', \
                         last_heartbeat_at = '2026-01-01T00:00:00.000Z' WHERE id = ?1",
                        [run_id],
                    )
                    .expect("wrong holder"),
                Scenario::Expired => connection
                    .execute(
                        &format!(
                            "UPDATE kpi_ingest_runs SET lease_holder = '{correct_holder}', \
                             lease_expires_at = '2000-01-01T00:00:00.000Z', \
                             last_heartbeat_at = '2000-01-01T00:00:00.000Z' WHERE id = ?1"
                        ),
                        [run_id],
                    )
                    .expect("expired"),
                Scenario::NoLease => connection
                    .execute(
                        "UPDATE kpi_ingest_runs SET lease_holder = NULL, lease_expires_at = NULL, \
                         last_heartbeat_at = NULL WHERE id = ?1",
                        [run_id],
                    )
                    .expect("no lease"),
            };
        }

        for scenario in [Scenario::WrongHolder, Scenario::Expired, Scenario::NoLease] {
            // mark_source_captured.
            {
                let (state, doc, company) = setup_one_company_doc();
                let store = state.kpi_ingest_runs();
                let run = store
                    .create_run_if_absent(&new_run(doc, company, "p1"))
                    .expect("create");
                apply(&state, &run.id, scenario, "worker-a");
                let error = store
                    .mark_source_captured(&run.id, "worker-a", "hash1")
                    .expect_err("lease scenario must refuse");
                assert!(matches!(error, StorageError::RunLeaseNotHeld { .. }));
            }
            // begin_extracting.
            {
                let (state, doc, company) = setup_one_company_doc();
                let store = state.kpi_ingest_runs();
                let run = store
                    .create_run_if_absent(&ready_new_run(doc, company, "p1"))
                    .expect("create");
                store.claim_next("worker-a", 3600).expect("claim");
                store
                    .mark_source_captured(&run.id, "worker-a", "hash1")
                    .expect("capture");
                apply(&state, &run.id, scenario, "worker-a");
                let error = store
                    .begin_extracting(&run.id, "worker-a", "instr-1")
                    .expect_err("lease scenario must refuse");
                assert!(matches!(error, StorageError::RunLeaseNotHeld { .. }));
            }
            // stage_observations (back-fit, #360).
            {
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
                    None,
                    None,
                    None,
                );
                connection
                    .execute(
                        "UPDATE kpi_ingest_runs SET period_fiscal_year = 2025, period_type = 'FY' \
                         WHERE id = 'run1'",
                        [],
                    )
                    .expect("period");
                let state = AppState::new(connection);
                apply(&state, "run1", scenario, "worker-a");
                let error = state
                    .kpi_ingest_staging()
                    .stage_observations("run1", "worker-a", vec![one_test_observation()])
                    .expect_err("lease scenario must refuse");
                assert!(matches!(error, StorageError::RunLeaseNotHeld { .. }));
            }
        }
    }

    // --- Test 4: freeze/stale ------------------------------------------------
    // (covered above: mark_ready_to_commit_refuses_a_stale_revision,
    // mark_ready_to_commit_refuses_when_a_manifest_is_already_issued,
    // invalidate_manifest_clears_hash_and_returns_to_staged; begin_committing
    // staleness below.)

    #[test]
    fn begin_committing_refuses_a_stale_manifest_hash_or_revision() {
        let (state, doc, company) = setup_one_company_doc();
        let ready = advance_to_ready_to_commit(&state, doc, company, "worker-a", "hash-abc");
        let mut connection = state.checkout_for_tests().expect("raw connection");
        let tx = connection
            .transaction_with_behavior(rusqlite::TransactionBehavior::Immediate)
            .expect("tx");
        let error = begin_committing(&tx, &ready.id, "wrong-hash", ready.manifest_revision)
            .expect_err("stale hash");
        assert!(matches!(error, StorageError::StaleManifestForCommit { .. }));
        let error = begin_committing(&tx, &ready.id, "hash-abc", ready.manifest_revision + 1)
            .expect_err("stale revision");
        assert!(matches!(error, StorageError::StaleManifestForCommit { .. }));
    }

    #[test]
    fn begin_committing_refuses_a_lingering_lease_as_an_invariant_violation() {
        let connection = open_in_memory_database().expect("db");
        seed_company(&connection, "c1");
        seed_document(&connection, "doc1", "c1");
        seed_run_raw(
            &connection,
            "run1",
            "doc1",
            "c1",
            "p1",
            "ready_to_commit",
            Some("worker-a"),
            Some("2999-01-01T00:00:00.000Z"),
            Some("2026-01-01T00:00:00.000Z"),
        );
        connection
            .execute(
                "UPDATE kpi_ingest_runs SET manifest_hash = 'hash-abc', manifest_revision = 1 WHERE id = 'run1'",
                [],
            )
            .expect("manifest");
        let state = AppState::new(connection);
        let mut conn2 = state.checkout_for_tests().expect("raw connection");
        let tx = conn2
            .transaction_with_behavior(rusqlite::TransactionBehavior::Immediate)
            .expect("tx");
        let error =
            begin_committing(&tx, "run1", "hash-abc", 1).expect_err("lease invariant violated");
        assert!(matches!(
            error,
            StorageError::RunLeaseInvariantViolation { .. }
        ));
    }

    // --- Test 6: connection-level composition --------------------------------

    #[test]
    fn begin_committing_and_finalize_compose_in_one_external_transaction() {
        let (state, doc, company) = setup_one_company_doc();
        let ready = advance_to_ready_to_commit(&state, doc, company, "worker-a", "hash-abc");

        let terminal = {
            // Scoped so the in-memory DB's single-connection guard is
            // released before the `get_run` call below checks out its own
            // (the in-memory test path serializes every checkout behind ONE
            // `Mutex` — holding this guard across another checkout deadlocks).
            let mut connection = state.checkout_for_tests().expect("raw connection");
            let tx = connection
                .transaction_with_behavior(rusqlite::TransactionBehavior::Immediate)
                .expect("tx");
            begin_committing(&tx, &ready.id, "hash-abc", ready.manifest_revision).expect("begin");
            kpi_ingest_staging::record_commit_receipt(
                &tx,
                NewCommitReceipt {
                    run_id: ready.id.clone(),
                    manifest_hash: "hash-abc".to_owned(),
                    manifest_revision: ready.manifest_revision,
                    terminal_status: "complete".to_owned(),
                    period_id: None,
                    accepted_count: 1,
                    outcomes_schema_version: 1,
                    outcomes_json: "[]".to_owned(),
                },
            )
            .expect("receipt");
            let terminal = finalize_committing(&tx, &ready.id).expect("finalize");
            tx.commit().expect("commit");
            terminal
        };
        assert_eq!(terminal, KpiIngestRunState::Complete);

        let after = state
            .kpi_ingest_runs()
            .get_run(&ready.id)
            .expect("get")
            .expect("some");
        assert_eq!(after.status, KpiIngestRunState::Complete);
    }

    #[test]
    fn double_begin_committing_and_double_finalize_both_refuse() {
        let (state, doc, company) = setup_one_company_doc();
        let ready = advance_to_ready_to_commit(&state, doc, company, "worker-a", "hash-abc");
        let mut connection = state.checkout_for_tests().expect("raw connection");
        let tx = connection
            .transaction_with_behavior(rusqlite::TransactionBehavior::Immediate)
            .expect("tx");
        begin_committing(&tx, &ready.id, "hash-abc", ready.manifest_revision).expect("first begin");
        let error = begin_committing(&tx, &ready.id, "hash-abc", ready.manifest_revision)
            .expect_err("double begin");
        assert!(matches!(error, StorageError::InvalidRunTransition { .. }));

        kpi_ingest_staging::record_commit_receipt(
            &tx,
            NewCommitReceipt {
                run_id: ready.id.clone(),
                manifest_hash: "hash-abc".to_owned(),
                manifest_revision: ready.manifest_revision,
                terminal_status: "complete".to_owned(),
                period_id: None,
                accepted_count: 0,
                outcomes_schema_version: 1,
                outcomes_json: "[]".to_owned(),
            },
        )
        .expect("receipt");
        finalize_committing(&tx, &ready.id).expect("first finalize");
        let error = finalize_committing(&tx, &ready.id).expect_err("double finalize");
        assert!(matches!(error, StorageError::InvalidRunTransition { .. }));
        tx.commit().expect("commit");
    }

    #[test]
    fn finalize_committing_refuses_without_a_receipt_and_on_receipt_mismatch() {
        let (state, doc, company) = setup_one_company_doc();
        let ready = advance_to_ready_to_commit(&state, doc, company, "worker-a", "hash-abc");
        let mut connection = state.checkout_for_tests().expect("raw connection");
        let tx = connection
            .transaction_with_behavior(rusqlite::TransactionBehavior::Immediate)
            .expect("tx");
        begin_committing(&tx, &ready.id, "hash-abc", ready.manifest_revision).expect("begin");

        let error = finalize_committing(&tx, &ready.id).expect_err("no receipt");
        assert!(matches!(
            error,
            StorageError::RunTransitionPrerequisiteMissing { .. }
        ));

        kpi_ingest_staging::record_commit_receipt(
            &tx,
            NewCommitReceipt {
                run_id: ready.id.clone(),
                manifest_hash: "OTHER-HASH".to_owned(),
                manifest_revision: ready.manifest_revision,
                terminal_status: "complete".to_owned(),
                period_id: None,
                accepted_count: 0,
                outcomes_schema_version: 1,
                outcomes_json: "[]".to_owned(),
            },
        )
        .expect("mismatched receipt");
        let error = finalize_committing(&tx, &ready.id).expect_err("mismatch");
        assert!(matches!(error, StorageError::StaleManifestForCommit { .. }));
    }

    #[test]
    fn begin_committing_finalize_rollback_undoes_everything() {
        let (state, doc, company) = setup_one_company_doc();
        let ready = advance_to_ready_to_commit(&state, doc, company, "worker-a", "hash-abc");
        {
            let mut connection = state.checkout_for_tests().expect("raw connection");
            let tx = connection
                .transaction_with_behavior(rusqlite::TransactionBehavior::Immediate)
                .expect("tx");
            begin_committing(&tx, &ready.id, "hash-abc", ready.manifest_revision).expect("begin");
            kpi_ingest_staging::record_commit_receipt(
                &tx,
                NewCommitReceipt {
                    run_id: ready.id.clone(),
                    manifest_hash: "hash-abc".to_owned(),
                    manifest_revision: ready.manifest_revision,
                    terminal_status: "complete".to_owned(),
                    period_id: None,
                    accepted_count: 0,
                    outcomes_schema_version: 1,
                    outcomes_json: "[]".to_owned(),
                },
            )
            .expect("receipt");
            finalize_committing(&tx, &ready.id).expect("finalize");
            // Deliberately dropped without commit -> rollback.
        }
        let after = state
            .kpi_ingest_runs()
            .get_run(&ready.id)
            .expect("get")
            .expect("some");
        assert_eq!(
            after.status,
            KpiIngestRunState::ReadyToCommit,
            "rollback restores the pre-commit state"
        );
        assert!(state
            .kpi_ingest_staging()
            .get_commit_receipt(&ready.id)
            .expect("get")
            .is_none());
    }

    // --- Test 5: reclaim ------------------------------------------------------

    #[test]
    fn reclaim_finalizes_committing_runs_with_a_matching_receipt_and_is_idempotent() {
        let (state, doc, company) = setup_one_company_doc();
        let ready = advance_to_ready_to_commit(&state, doc, company, "worker-a", "hash-abc");
        {
            let mut connection = state.checkout_for_tests().expect("raw connection");
            let tx = connection
                .transaction_with_behavior(rusqlite::TransactionBehavior::Immediate)
                .expect("tx");
            begin_committing(&tx, &ready.id, "hash-abc", ready.manifest_revision).expect("begin");
            kpi_ingest_staging::record_commit_receipt(
                &tx,
                NewCommitReceipt {
                    run_id: ready.id.clone(),
                    manifest_hash: "hash-abc".to_owned(),
                    manifest_revision: ready.manifest_revision,
                    terminal_status: "partial".to_owned(),
                    period_id: None,
                    accepted_count: 2,
                    outcomes_schema_version: 1,
                    outcomes_json: "[]".to_owned(),
                },
            )
            .expect("receipt");
            // Commit WITHOUT finalizing — simulates a crash between the receipt
            // write and finalize_committing.
            tx.commit().expect("commit");
        }
        let store = state.kpi_ingest_runs();
        let summary = store.reclaim_ingest_runs_on_startup().expect("reclaim");
        assert_eq!(summary.finalized, 1);
        assert_eq!(summary.reverted, 0);
        assert_eq!(summary.violations, 0);
        let after = store.get_run(&ready.id).expect("get").expect("some");
        assert_eq!(after.status, KpiIngestRunState::Partial);

        let second = store
            .reclaim_ingest_runs_on_startup()
            .expect("second reclaim");
        assert!(second.is_noop(), "a second reclaim must be a pure no-op");
    }

    #[test]
    fn reclaim_reverts_committing_without_a_receipt_to_ready_to_commit() {
        let connection = open_in_memory_database().expect("db");
        seed_company(&connection, "c1");
        seed_document(&connection, "doc1", "c1");
        seed_run_raw(
            &connection,
            "run1",
            "doc1",
            "c1",
            "p1",
            "committing",
            None,
            None,
            None,
        );
        connection
            .execute(
                "UPDATE kpi_ingest_runs SET manifest_hash = 'hash-abc', manifest_revision = 3 WHERE id = 'run1'",
                [],
            )
            .expect("manifest");
        let state = AppState::new(connection);
        let store = state.kpi_ingest_runs();
        let summary = store.reclaim_ingest_runs_on_startup().expect("reclaim");
        assert_eq!(summary.reverted, 1);
        assert_eq!(summary.finalized, 0);
        let after = store.get_run("run1").expect("get").expect("some");
        assert_eq!(after.status, KpiIngestRunState::ReadyToCommit);
        assert_eq!(after.manifest_hash.as_deref(), Some("hash-abc"));
        assert_eq!(after.manifest_revision, 3);
    }

    #[test]
    fn reclaim_reports_a_committing_lease_violation_and_leaves_the_row_untouched() {
        let connection = open_in_memory_database().expect("db");
        seed_company(&connection, "c1");
        seed_document(&connection, "doc1", "c1");
        seed_run_raw(
            &connection,
            "run1",
            "doc1",
            "c1",
            "p1",
            "committing",
            Some("worker-a"),
            Some("2000-01-01T00:00:00.000Z"),
            Some("2000-01-01T00:00:00.000Z"),
        );
        let state = AppState::new(connection);
        let store = state.kpi_ingest_runs();
        let before = store.get_run("run1").expect("get").expect("some");
        let summary = store.reclaim_ingest_runs_on_startup().expect("reclaim");
        assert_eq!(summary.violations, 1);
        assert_eq!(summary.finalized, 0);
        assert_eq!(summary.reverted, 0);
        let after = store.get_run("run1").expect("get").expect("some");
        assert_eq!(before, after, "a violating row must be left exactly as-is");
    }

    #[test]
    fn reclaim_leaves_a_committing_row_untouched_when_the_receipt_mismatches() {
        let connection = open_in_memory_database().expect("db");
        seed_company(&connection, "c1");
        seed_document(&connection, "doc1", "c1");
        seed_run_raw(
            &connection,
            "run1",
            "doc1",
            "c1",
            "p1",
            "committing",
            None,
            None,
            None,
        );
        connection
            .execute(
                "UPDATE kpi_ingest_runs SET manifest_hash = 'hash-abc', manifest_revision = 1 WHERE id = 'run1'",
                [],
            )
            .expect("manifest");
        connection
            .execute(
                "INSERT INTO kpi_ingest_commit_receipts
                    (id, run_id, manifest_hash, manifest_revision, terminal_status, accepted_count, outcomes_json)
                 VALUES ('r1', 'run1', 'OTHER-HASH', 1, 'complete', 0, '[]')",
                [],
            )
            .expect("mismatched receipt");
        let state = AppState::new(connection);
        let store = state.kpi_ingest_runs();
        let before = store.get_run("run1").expect("get").expect("some");
        let summary = store.reclaim_ingest_runs_on_startup().expect("reclaim");
        assert_eq!(summary.finalized, 0);
        assert_eq!(summary.reverted, 0);
        let after = store.get_run("run1").expect("get").expect("some");
        assert_eq!(
            before, after,
            "a mismatched receipt must leave the run untouched"
        );
    }

    #[test]
    fn reclaim_also_clears_expired_leases_on_claimable_rows() {
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
            Some("2000-01-01T00:00:00.000Z"),
            Some("2000-01-01T00:00:00.000Z"),
        );
        let state = AppState::new(connection);
        let store = state.kpi_ingest_runs();
        let summary = store.reclaim_ingest_runs_on_startup().expect("reclaim");
        assert_eq!(summary.lease_cleared, 1);
        let after = store.get_run("run1").expect("get").expect("some");
        assert!(after.lease_holder.is_none());
        assert_eq!(
            after.status,
            KpiIngestRunState::Extracting,
            "reclaim never changes the status of a claimable row"
        );
    }

    // --- Test 7: cancel ---------------------------------------------------

    #[test]
    fn cancel_run_succeeds_from_every_pre_commit_state_and_releases_the_lease() {
        for status in [
            "discovered",
            "source_captured",
            "extracting",
            "staged",
            "validation_failed",
            "ready_to_commit",
        ] {
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
                Some("worker-a"),
                Some("2999-01-01T00:00:00.000Z"),
                Some("2026-01-01T00:00:00.000Z"),
            );
            let state = AppState::new(connection);
            let store = state.kpi_ingest_runs();
            store
                .cancel_run("run1")
                .unwrap_or_else(|error| panic!("status '{status}' must cancel: {error:?}"));
            let after = store.get_run("run1").expect("get").expect("some");
            assert_eq!(after.status, KpiIngestRunState::Cancelled);
            assert!(after.lease_holder.is_none());
        }
    }

    #[test]
    fn cancel_run_refuses_from_committing_and_every_terminal_state() {
        for status in ["committing", "complete", "partial", "failed", "cancelled"] {
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
            let before = store.get_run("run1").expect("get").expect("some");
            let error = store
                .cancel_run("run1")
                .expect_err(&format!("status '{status}' must refuse cancel"));
            assert!(matches!(error, StorageError::InvalidRunTransition { .. }));
            assert_eq!(before, store.get_run("run1").expect("get").expect("some"));
        }
    }

    // --- Test 2 (mark_failed): prerequisites/exhaustion ----------------------

    #[test]
    fn mark_failed_succeeds_from_every_pre_commit_state_including_ready_to_commit() {
        for status in [
            "discovered",
            "source_captured",
            "extracting",
            "staged",
            "validation_failed",
            "ready_to_commit",
        ] {
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
                Some("worker-a"),
                Some("2999-01-01T00:00:00.000Z"),
                Some("2026-01-01T00:00:00.000Z"),
            );
            let state = AppState::new(connection);
            let store = state.kpi_ingest_runs();
            store
                .mark_failed("run1", "boom")
                .unwrap_or_else(|error| panic!("status '{status}' must fail: {error:?}"));
            let after = store.get_run("run1").expect("get").expect("some");
            assert_eq!(after.status, KpiIngestRunState::Failed);
            assert_eq!(after.last_error.as_deref(), Some("boom"));
            assert!(after.lease_holder.is_none());
        }
    }

    #[test]
    fn mark_failed_refuses_from_committing_with_unchanged_snapshot() {
        let connection = open_in_memory_database().expect("db");
        seed_company(&connection, "c1");
        seed_document(&connection, "doc1", "c1");
        seed_run_raw(
            &connection,
            "run1",
            "doc1",
            "c1",
            "p1",
            "committing",
            None,
            None,
            None,
        );
        let state = AppState::new(connection);
        let store = state.kpi_ingest_runs();
        let before = store.get_run("run1").expect("get").expect("some");
        let error = store
            .mark_failed("run1", "boom")
            .expect_err("committing must refuse mark_failed");
        assert!(matches!(error, StorageError::InvalidRunTransition { .. }));
        assert_eq!(before, store.get_run("run1").expect("get").expect("some"));
    }
}
