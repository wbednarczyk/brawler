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
use super::financials;
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

/// The residual lease-failure shape — an absent lease, or a foreign lease
/// that is no longer live (#360 F9 r2 shared corpus). The two agent-facing
/// specializations split out in #384: [`lease_refusal`] classifies a caller's
/// own expired lease as `RunLeaseExpired` and a LIVE foreign lease as
/// `RunTakenOver` (the frozen ADR 0099 remedies).
fn lease_not_held(id: &str, holder: &str) -> StorageError {
    StorageError::RunLeaseNotHeld {
        id: id.to_owned(),
        holder: holder.to_owned(),
    }
}

/// Three-way lease-refusal classification (ADR 0099, #384): the caller's OWN
/// lease expired → `RunLeaseExpired` (retryable via `start_kpi_ingest(runId)`);
/// a LIVE foreign lease → `RunTakenOver` (abandon); anything else (absent, or
/// foreign-and-expired) → `RunLeaseNotHeld`. The TTL is not a correctness
/// mechanism — a lease may lapse between claim and any later guarded step.
/// Classify a lease refusal for `holder` from a fresh read on the SAME
/// connection/transaction — the three-way vocabulary (#360/#384) shared by
/// every agent-facing intent; #386 routes staging through it too.
pub(super) fn lease_refusal_on_connection(
    conn: &Connection,
    id: &str,
    holder: &str,
) -> StorageResult<StorageError> {
    let Some(raw) = read_raw_run(conn, id)? else {
        return Ok(StorageError::KpiIngestRunNotFound { id: id.to_owned() });
    };
    lease_refusal(conn, &raw, id, holder)
}

fn lease_refusal(
    conn: &Connection,
    raw: &RawRun,
    id: &str,
    holder: &str,
) -> StorageResult<StorageError> {
    Ok(match raw.lease_holder.as_deref() {
        Some(current) if current == holder => {
            if lease_is_live_on_connection(conn, id, current)? {
                // A live same-holder lease never reaches a refusal branch on
                // its own; a racing writer is surfaced as the residual shape.
                lease_not_held(id, holder)
            } else {
                StorageError::RunLeaseExpired {
                    id: id.to_owned(),
                    holder: holder.to_owned(),
                }
            }
        }
        Some(current) => {
            if lease_is_live_on_connection(conn, id, current)? {
                StorageError::RunTakenOver {
                    id: id.to_owned(),
                    holder: current.to_owned(),
                }
            } else {
                lease_not_held(id, holder)
            }
        }
        None => lease_not_held(id, holder),
    })
}

/// The first set-once context field whose stored value differs from the
/// requested one ([`KpiIngestRunsStore::attach_run_context`]'s 0-row
/// classification) — field order scope → data_quality → period.
fn classify_context_conflict(
    raw: &RawRun,
    ctx: &RunContextAttach<'_>,
    id: &str,
) -> Option<StorageError> {
    if let (Some(requested), Some(existing)) = (ctx.scope, raw.scope.as_deref()) {
        if requested != existing {
            return Some(StorageError::RunContextValueConflict {
                id: id.to_owned(),
                key: "scope",
                existing: existing.to_owned(),
                requested: requested.to_owned(),
            });
        }
    }
    if let (Some(requested), Some(existing)) = (ctx.data_quality, raw.data_quality.as_deref()) {
        if requested != existing {
            return Some(StorageError::RunContextValueConflict {
                id: id.to_owned(),
                key: "data_quality",
                existing: existing.to_owned(),
                requested: requested.to_owned(),
            });
        }
    }
    if let Some((fy, pt)) = ctx.period {
        if let (Some(existing_fy), Some(existing_pt)) =
            (raw.period_fiscal_year, raw.period_type.as_deref())
        {
            if existing_fy != fy || existing_pt != pt {
                return Some(StorageError::RunContextValueConflict {
                    id: id.to_owned(),
                    key: "period",
                    existing: format!("{existing_fy}/{existing_pt}"),
                    requested: format!("{fy}/{pt}"),
                });
            }
        }
    }
    None
}

/// [`KpiIngestRunsStore::claim_run`]'s body, parameterized on the transaction
/// clock — production passes a freshly-read `strftime` timestamp, tests a
/// fixed one (the deterministic seam for the `lease_expires_at == now`
/// boundary: `>` renews, `<=` claims).
fn claim_run_on_connection(
    tx: &Connection,
    id: &str,
    holder: &str,
    lease_seconds: i64,
    now: &str,
) -> StorageResult<KpiIngestRun> {
    let modifier = format!("+{lease_seconds} seconds");

    // (a) Renewal: same holder, live lease — the keepalive. No attempt++.
    let renewal_sql = format!(
        "UPDATE kpi_ingest_runs
         SET lease_expires_at = strftime('%Y-%m-%dT%H:%M:%fZ', ?4, ?3),
             last_heartbeat_at = ?4,
             updated_at = ?4
         WHERE id = ?1 AND status IN {CLAIMABLE_STATUSES_SQL}
           AND lease_holder = ?2 AND lease_expires_at > ?4
         RETURNING {RUN_COLUMNS}"
    );
    let renewed: Option<RawRun> = tx
        .query_row(
            &renewal_sql,
            params![id, holder, modifier, now],
            map_raw_row,
        )
        .optional()?;
    if let Some(raw) = renewed {
        return raw_to_domain(raw);
    }

    // (b) Claim: absent or expired lease (any holder). attempt++.
    let claim_sql = format!(
        "UPDATE kpi_ingest_runs
         SET lease_holder = ?2,
             lease_expires_at = strftime('%Y-%m-%dT%H:%M:%fZ', ?4, ?3),
             last_heartbeat_at = ?4,
             attempt_count = attempt_count + 1,
             updated_at = ?4
         WHERE id = ?1 AND status IN {CLAIMABLE_STATUSES_SQL}
           AND (lease_expires_at IS NULL OR lease_expires_at <= ?4)
         RETURNING {RUN_COLUMNS}"
    );
    let claimed: Option<RawRun> = tx
        .query_row(&claim_sql, params![id, holder, modifier, now], map_raw_row)
        .optional()?;
    if let Some(raw) = claimed {
        return raw_to_domain(raw);
    }

    // 0 rows on both branches — classify on the same transaction.
    let Some(raw) = read_raw_run(tx, id)? else {
        return Err(StorageError::KpiIngestRunNotFound { id: id.to_owned() });
    };
    let status = KpiIngestRunState::parse(&raw.status)?;
    if !status.is_agent_claimable() {
        return Err(StorageError::RunNotClaimable {
            id: id.to_owned(),
            status: status.as_str().to_owned(),
        });
    }
    // Claimable + neither branch matched ⇒ a lease is live under another
    // holder as of THIS transaction's clock (a live same-holder lease would
    // have renewed). Compare against the same bound `now` for consistency.
    match (raw.lease_holder.as_deref(), raw.lease_expires_at.as_deref()) {
        (Some(current), Some(expires)) if current != holder && expires > now => {
            Err(StorageError::RunTakenOver {
                id: id.to_owned(),
                holder: current.to_owned(),
            })
        }
        _ => Err(lease_not_held(id, holder)),
    }
}

/// Vocabulary validation shared by [`KpiIngestRunsStore::create_run_if_absent`]
/// and [`KpiIngestRunsStore::attach_run_context`] (#384) — one place, both
/// writers.
fn validate_scope_value(scope: &str) -> StorageResult<()> {
    if matches!(scope, "standalone" | "consolidated") {
        Ok(())
    } else {
        Err(StorageError::InvalidKpiIngestRunValue {
            key: "scope",
            value: scope.to_owned(),
        })
    }
}

fn validate_data_quality_value(quality: &str) -> StorageResult<()> {
    if matches!(quality, "final" | "preliminary" | "estimated") {
        Ok(())
    } else {
        Err(StorageError::InvalidKpiIngestRunValue {
            key: "data_quality",
            value: quality.to_owned(),
        })
    }
}

fn validate_period_type_value(period_type: &str) -> StorageResult<()> {
    if PERIOD_TYPE_VALUES.contains(&period_type) {
        Ok(())
    } else {
        Err(StorageError::InvalidKpiIngestRunValue {
            key: "period_type",
            value: period_type.to_owned(),
        })
    }
}

/// One exact queue generation of a run (#386, E1): the tuple a terminalization
/// caller SELECTED before deciding to fail the run. Typed so the
/// ready-to-commit variant cannot omit its manifest hash — a
/// `(status, Option<hash>)` pair would admit an unsafe `ReadyToCommit + None`.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum IngestGeneration {
    Staged {
        revision: i64,
    },
    ReadyToCommit {
        revision: i64,
        manifest_hash: String,
    },
}

/// Bound `last_error` at its single write boundary (#385): ≤2048 UTF-8 bytes,
/// truncated on a char boundary with an `…` marker. The MCP context read model
/// embeds `RunStatus` (and its stored `lastError`) verbatim — its 256 KiB
/// response budget is provable only with every variable-length column bounded
/// at the producer.
fn bound_last_error(message: &str) -> String {
    const MAX: usize = 2048;
    const MARKER: &str = "…";
    if message.len() <= MAX {
        return message.to_owned();
    }
    let mut end = MAX - MARKER.len();
    while !message.is_char_boundary(end) {
        end -= 1;
    }
    format!("{}{MARKER}", &message[..end])
}

fn wrong_state(id: &str, actual: KpiIngestRunState, to: KpiIngestRunState) -> StorageError {
    StorageError::InvalidRunTransition {
        id: id.to_owned(),
        from: actual.as_str().to_owned(),
        to: to.as_str().to_owned(),
    }
}

/// Connection-level `staged -> validation_failed` primitive (#361 sol r4):
/// the ONLY callers are [`super::kpi_ingest_staging::apply_validation_outcome`]'s
/// atom (under its own transaction, after inserting the immutable attempt
/// row) and this module's own tests. No public `pub fn` wraps this anymore —
/// a hash-bearing `ready_to_commit` row or a `validation_failed` row with no
/// attempt evidence is structurally uncreatable after migration 0139 (every
/// path to either transition now inserts an attempt row first). Targets the
/// run's CURRENT `manifest_revision`, guarding a race with a concurrent
/// re-stage bumping it.
pub(super) fn mark_validation_failed_on_connection(
    conn: &Connection,
    id: &str,
    revision: i64,
) -> StorageResult<()> {
    let changed = apply_transition(
        conn,
        id,
        &[KpiIngestRunState::Staged],
        KpiIngestRunState::ValidationFailed,
        "",
        &[],
        " AND manifest_revision = ?",
        &[&revision],
    )?;
    if changed == 1 {
        return Ok(());
    }

    let Some(raw) = read_raw_run(conn, id)? else {
        return Err(StorageError::KpiIngestRunNotFound { id: id.to_owned() });
    };
    let status = KpiIngestRunState::parse(&raw.status)?;
    if status != KpiIngestRunState::Staged {
        Err(wrong_state(id, status, KpiIngestRunState::ValidationFailed))
    } else {
        Err(StorageError::InvalidStagingRevision {
            run_id: id.to_owned(),
            revision,
            reason: "revision is not the run's current staging revision",
        })
    }
}

/// Connection-level `staged -> ready_to_commit` primitive (#361 sol r4) — see
/// [`mark_validation_failed_on_connection`]'s doc for why no `pub fn` wraps
/// this. SETs `manifest_hash` and clears all three lease columns ATOMICALLY
/// (ADR 0098 dec. 6 — a `ready_to_commit` row never holds a lease). Guard:
/// `manifest_revision = ?revision AND manifest_hash IS NULL` (current AND
/// never previously frozen).
pub(super) fn mark_ready_to_commit_on_connection(
    conn: &Connection,
    id: &str,
    revision: i64,
    manifest_hash: &str,
) -> StorageResult<()> {
    let changed = apply_transition(
        conn,
        id,
        &[KpiIngestRunState::Staged],
        KpiIngestRunState::ReadyToCommit,
        "manifest_hash = ?, lease_holder = NULL, lease_expires_at = NULL, last_heartbeat_at = NULL, ",
        &[&manifest_hash],
        " AND manifest_revision = ? AND manifest_hash IS NULL",
        &[&revision],
    )?;
    if changed == 1 {
        return Ok(());
    }

    let Some(raw) = read_raw_run(conn, id)? else {
        return Err(StorageError::KpiIngestRunNotFound { id: id.to_owned() });
    };
    let status = KpiIngestRunState::parse(&raw.status)?;
    if status != KpiIngestRunState::Staged {
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
    /// `committing` rows whose receipt disagrees with the run's manifest —
    /// left untouched, needs manual investigation (luna review P2: visible in
    /// the startup log, never silently absorbed into a no-op summary).
    pub mismatched: usize,
}

impl ReclaimSummary {
    pub fn is_noop(&self) -> bool {
        self.finalized == 0
            && self.reverted == 0
            && self.lease_cleared == 0
            && self.violations == 0
            && self.mismatched == 0
    }
}

/// The identity + discovery-known fields for a new run. `instruction_version`
/// is deliberately absent: nullable at creation, filled before `extracting`
/// (#360 invariant).
///
/// `period_fiscal_year`/`period_type` are the run's durable period
/// descriptor (ADR 0098 dec. 3, B2 sol review round 2, migration 0138): a
/// `financial_periods` row legally does not exist until the commit
/// transaction (`commit_manifest`, #362) creates one, so staging needs a natural-key descriptor
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

/// Set-once context values [`KpiIngestRunsStore::attach_run_context`] fills on
/// a claimed run (#384): each `Some` either fills a NULL column or must equal
/// the stored value. `period` carries the all-or-none natural-key descriptor
/// `(fiscal_year, period_type)`.
#[derive(Debug, Clone, Default)]
pub struct RunContextAttach<'a> {
    pub scope: Option<&'a str>,
    pub data_quality: Option<&'a str>,
    pub period: Option<(i64, &'a str)>,
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

/// One immutable row from `kpi_ingest_validation_attempts` (migration 0139) —
/// append-only, never updated/deleted outside `ON DELETE CASCADE`.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ValidationAttempt {
    pub id: String,
    pub run_id: String,
    pub revision: i64,
    pub attempt: i64,
    /// `ready` | `failed`.
    pub outcome: String,
    pub manifest_hash: String,
    /// Canonical manifest bytes (`fundamentals::kpi_manifest::SealedManifest::manifest_json`).
    pub manifest_json: String,
    pub created_at: String,
}

fn map_validation_attempt_row(row: &rusqlite::Row<'_>) -> rusqlite::Result<ValidationAttempt> {
    Ok(ValidationAttempt {
        id: row.get(0)?,
        run_id: row.get(1)?,
        revision: row.get(2)?,
        attempt: row.get(3)?,
        outcome: row.get(4)?,
        manifest_hash: row.get(5)?,
        manifest_json: row.get(6)?,
        created_at: row.get(7)?,
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
    /// belongs to the given company first (`RunDocumentCompanyMismatch`), and
    /// `profile_version` against the extraction-profile registry (ADR 0099
    /// dec. 6). A fresh run is stamped here with its frozen `expected_kpis_json`
    /// denominator — the union of the company's live relevance and the
    /// profile's statement-type pack; validation consumes this stamp.
    pub fn create_run_if_absent(&self, new_run: &NewKpiIngestRun) -> StorageResult<KpiIngestRun> {
        let mut connection = self.db.checkout()?;
        let tx = connection.transaction_with_behavior(rusqlite::TransactionBehavior::Immediate)?;

        if let Some(scope) = new_run.scope.as_deref() {
            validate_scope_value(scope)?;
        }
        if let Some(quality) = new_run.data_quality.as_deref() {
            validate_data_quality_value(quality)?;
        }
        if !super::kpi_ingest_profiles::is_registered_profile_version(&new_run.profile_version) {
            return Err(StorageError::InvalidKpiIngestRunValue {
                key: "profile_version",
                value: new_run.profile_version.clone(),
            });
        }

        // Period descriptor (ADR 0098 dec. 3, B2 sol review round 2):
        // all-or-none, then vocabulary. Cross-checked against an explicit
        // period_id below once that row is resolved.
        match (new_run.period_fiscal_year, new_run.period_type.as_deref()) {
            (None, None) => {}
            (Some(_), Some(period_type)) => {
                validate_period_type_value(period_type)?;
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

        // Creation-time expected-KPI stamp (ADR 0099 dec. 6): relevance ∪
        // profile pack, frozen for the run's whole life — definitions minted
        // later can never widen the denominator (ADR 0093 dec. 4). Both reads
        // stay on THIS transaction (a pooled store wrapper here would take a
        // second checkout inside an Immediate tx).
        let statement_type = super::companies::get_statement_type(&tx, &new_run.company_id)?;
        let mut keys = super::financials::expected_primary_metric_keys(&tx, &new_run.company_id)?
            .unwrap_or_default();
        keys.extend(
            super::kpi_ingest_profiles::expected_pack(&new_run.profile_version, &statement_type)?
                .iter()
                .map(|k| (*k).to_owned()),
        );
        // Stamp-size invariant (#385): the MCP context read model returns the
        // stamp verbatim inside every RunStatus, so its response-budget
        // arithmetic (contracts.md § Budgets) requires a hard bound here — a
        // company with more than 256 primary expected KPIs is a data error,
        // not a run to create.
        if keys.len() > 256 {
            return Err(StorageError::InvalidKpiIngestRunValue {
                key: "expected_kpis",
                value: format!("{} keys exceed the 256-key stamp bound", keys.len()),
            });
        }
        let stamp = crate::fundamentals::kpi_manifest::ExpectedKpis {
            schema_version: 1,
            source: "kpi_relevance+profile_pack".to_owned(),
            pack_version: Some(new_run.profile_version.clone()),
            keys,
        };
        let expected_kpis_json =
            serde_json::to_string(&stamp).expect("ExpectedKpis serialization is total");

        let id = generate_run_id(
            &new_run.report_document_id,
            &new_run.company_id,
            &new_run.profile_version,
        );
        tx.execute(
            "INSERT INTO kpi_ingest_runs
                (id, report_document_id, company_id, period_id, profile_version, scope, data_quality,
                 period_fiscal_year, period_type, expected_kpis_json)
             VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10)",
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
                expected_kpis_json,
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

        // Set-once also holds across the retired lease-free path (luna review
        // B2): a legacy `discovered` row may already carry a hash — a different
        // one must refuse, the same one is idempotent.
        let changed = apply_transition(
            &tx,
            id,
            &[KpiIngestRunState::Discovered],
            KpiIngestRunState::SourceCaptured,
            "source_content_hash = ?, ",
            &[&source_content_hash],
            " AND lease_holder = ? AND lease_expires_at > strftime('%Y-%m-%dT%H:%M:%fZ', 'now') \
             AND (source_content_hash IS NULL OR source_content_hash = ?)",
            &[&holder, &source_content_hash],
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
            // Still `discovered`: either the guard's hash predicate refused a
            // DIFFERENT pre-existing hash (legacy lease-free capture — set-once
            // wins over the transition), or the lease is not live.
            match raw.source_content_hash.as_deref() {
                Some(existing) if existing != source_content_hash => {
                    Err(StorageError::RunSourceHashAlreadyRecorded { id: id.to_owned() })
                }
                _ => Err(lease_refusal(&tx, &raw, id, holder)?),
            }
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
            Err(lease_refusal(&tx, &raw, id, holder)?)
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

    /// LEGACY-NULL fallback stamp of `expected_kpis_json` (ADR 0099 dec. 6:
    /// the primary stamp is written by [`Self::create_run_if_absent`] at
    /// creation; only a raw-seeded legacy row with a NULL column reaches this
    /// writer). Set-once, status- and revision-guarded — `WHERE status='staged'
    /// AND manifest_revision=? AND manifest_hash IS NULL AND
    /// expected_kpis_json IS NULL`. A 0-row UPDATE is not automatically a
    /// refusal: if the column is already non-NULL the snapshot exists (from
    /// creation, or an earlier validation attempt on a legacy row) and this
    /// returns it unchanged — only a wrong status/revision is a typed refusal.
    /// Once stamped, the snapshot is frozen for the run's whole lifetime: the
    /// validator, #362 and #363 all read this column, never live
    /// `kpi_relevance`, so a later relevance change never changes any
    /// revision's denominator.
    pub fn stamp_expected_kpis(
        &self,
        id: &str,
        revision: i64,
        snapshot_json: &str,
    ) -> StorageResult<String> {
        let mut connection = self.db.checkout()?;
        let tx = connection.transaction_with_behavior(rusqlite::TransactionBehavior::Immediate)?;

        let changed = tx.execute(
            "UPDATE kpi_ingest_runs \
             SET expected_kpis_json = ?1, updated_at = strftime('%Y-%m-%dT%H:%M:%fZ', 'now') \
             WHERE id = ?2 AND status = 'staged' AND manifest_revision = ?3 \
               AND manifest_hash IS NULL AND expected_kpis_json IS NULL",
            params![snapshot_json, id, revision],
        )?;
        if changed == 1 {
            tx.commit()?;
            return Ok(snapshot_json.to_owned());
        }

        let Some(raw) = read_raw_run(&tx, id)? else {
            return Err(StorageError::KpiIngestRunNotFound { id: id.to_owned() });
        };
        let result = if let Some(existing) = raw.expected_kpis_json.clone() {
            Ok(existing)
        } else {
            let status = KpiIngestRunState::parse(&raw.status)?;
            let reason = if status != KpiIngestRunState::Staged {
                "run is not staged"
            } else {
                "revision is not the run's current staging revision"
            };
            Err(StorageError::InvalidStagingRevision {
                run_id: id.to_owned(),
                revision,
                reason,
            })
        };
        tx.commit()?;
        result
    }

    /// Every validation attempt ever recorded for `run_id`, oldest first —
    /// the append-only audit trail migration 0139 exists for: a `failed`
    /// attempt's diagnostics survive a re-stage/re-validate cycle.
    pub fn list_validation_attempts(&self, run_id: &str) -> StorageResult<Vec<ValidationAttempt>> {
        let connection = self.db.checkout()?;
        let mut statement = connection.prepare(
            "SELECT id, run_id, revision, attempt, outcome, manifest_hash, manifest_json, created_at \
             FROM kpi_ingest_validation_attempts WHERE run_id = ?1 ORDER BY revision, attempt",
        )?;
        let rows = statement
            .query_map([run_id], map_validation_attempt_row)?
            .collect::<Result<Vec<_>, _>>()?;
        Ok(rows)
    }

    /// The deterministic `ready` attempt for `(run_id, revision, manifest_hash)`
    /// — the read #362/#363 use to fetch the exact frozen manifest bytes the
    /// run's `manifest_hash` currently points at (`outcome='ready' ORDER BY
    /// attempt DESC LIMIT 1`, data-model.md § Kompatybilność: a `ready_to_commit`
    /// run whose (run, revision, hash) has no attempt row predates migration
    /// 0139 — `None` here, never a fabricated match).
    pub fn get_validation_attempt(
        &self,
        run_id: &str,
        revision: i64,
        manifest_hash: &str,
    ) -> StorageResult<Option<ValidationAttempt>> {
        let connection = self.db.checkout()?;
        connection
            .query_row(
                "SELECT id, run_id, revision, attempt, outcome, manifest_hash, manifest_json, created_at \
                 FROM kpi_ingest_validation_attempts \
                 WHERE run_id = ?1 AND revision = ?2 AND manifest_hash = ?3 AND outcome = 'ready' \
                 ORDER BY attempt DESC LIMIT 1",
                params![run_id, revision, manifest_hash],
                map_validation_attempt_row,
            )
            .optional()
            .map_err(StorageError::from)
    }

    /// The single most recent validation attempt for `run_id` — `ORDER BY
    /// revision DESC, attempt DESC LIMIT 1`, INCLUDING `failed` outcomes
    /// (#385). This is the repair-loop's "last manifest": a `validation_failed`
    /// run's row-level `manifest_hash` is NULL by design (staging re-zeroes
    /// it), so [`Self::get_validation_attempt`] (ready-only) cannot serve it.
    pub fn latest_validation_attempt(
        &self,
        run_id: &str,
    ) -> StorageResult<Option<ValidationAttempt>> {
        let connection = self.db.checkout()?;
        connection
            .query_row(
                "SELECT id, run_id, revision, attempt, outcome, manifest_hash, manifest_json, created_at \
                 FROM kpi_ingest_validation_attempts WHERE run_id = ?1 \
                 ORDER BY revision DESC, attempt DESC LIMIT 1",
                [run_id],
                map_validation_attempt_row,
            )
            .optional()
            .map_err(StorageError::from)
    }

    /// One validation attempt by its id, scoped to `run_id` (#385): the
    /// manifest-section cursor pins the attempt it started paginating, and the
    /// table is append-only — a pinned id always resolves for as long as the
    /// run exists.
    pub fn validation_attempt_by_id(
        &self,
        run_id: &str,
        attempt_id: &str,
    ) -> StorageResult<Option<ValidationAttempt>> {
        let connection = self.db.checkout()?;
        connection
            .query_row(
                "SELECT id, run_id, revision, attempt, outcome, manifest_hash, manifest_json, created_at \
                 FROM kpi_ingest_validation_attempts WHERE run_id = ?1 AND id = ?2",
                params![run_id, attempt_id],
                map_validation_attempt_row,
            )
            .optional()
            .map_err(StorageError::from)
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
            // ADR 0102 dec. 11: cancellation clears any open draft — a
            // cancelled run never leaves an orphan behind.
            super::kpi_ingest_drafts::clear_drafts_on_connection(&tx, id)?;
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
        // Write-boundary bound (#385): the MCP context read model embeds
        // `RunStatus` verbatim, so every stored variable-length field must be
        // bounded at its producer.
        let last_error = bound_last_error(last_error);
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
            // ADR 0102 dec. 11: failure clears any open draft too.
            super::kpi_ingest_drafts::clear_drafts_on_connection(&tx, id)?;
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

    /// Generation-guarded terminalization (#386, ADR 0099 dec. 1 / E1): fail
    /// the run ONLY while it still sits at the exact generation the caller
    /// selected — the guard lives in the UPDATE's WHERE clause, so the queue's
    /// check-then-act window (and startup reconcile's missing check) closes
    /// atomically. `Ok(false)` = the generation moved (or the run is gone) —
    /// a deliberate no-op: the run lives on in its NEW generation and the
    /// caller has nothing to do. `last_error` is bounded like [`Self::mark_failed`].
    pub fn mark_failed_for_generation(
        &self,
        id: &str,
        last_error: &str,
        generation: &IngestGeneration,
    ) -> StorageResult<bool> {
        let last_error = bound_last_error(last_error);
        let mut connection = self.db.checkout()?;
        let tx = connection.transaction_with_behavior(rusqlite::TransactionBehavior::Immediate)?;

        let changed = match generation {
            IngestGeneration::Staged { revision } => apply_transition(
                &tx,
                id,
                &[KpiIngestRunState::Staged],
                KpiIngestRunState::Failed,
                "last_error = ?, lease_holder = NULL, lease_expires_at = NULL, last_heartbeat_at = NULL, ",
                &[&last_error],
                " AND manifest_revision = ?",
                &[revision],
            )?,
            IngestGeneration::ReadyToCommit {
                revision,
                manifest_hash,
            } => apply_transition(
                &tx,
                id,
                &[KpiIngestRunState::ReadyToCommit],
                KpiIngestRunState::Failed,
                "last_error = ?, lease_holder = NULL, lease_expires_at = NULL, last_heartbeat_at = NULL, ",
                &[&last_error],
                " AND manifest_revision = ? AND manifest_hash = ?",
                &[revision, manifest_hash],
            )?,
        };
        tx.commit()?;
        Ok(changed == 1)
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

    /// Targeted atomic claim/renewal of ONE run (ADR 0099 dec. 4, #384) —
    /// `start_kpi_ingest`'s lease primitive. Three-way semantics: the same
    /// holder with a LIVE lease renews (expiry + heartbeat extended, NO
    /// `attempt_count` increment — the explicit keepalive); an absent or
    /// expired lease (any holder, including the caller's own expired one — an
    /// expired lease is never resurrected, mirroring [`Self::heartbeat`]) is
    /// claimed with `attempt_count + 1`; a LIVE foreign lease refuses with
    /// `RunTakenOver`. A non-claimable status refuses with `RunNotClaimable`
    /// (a claim is not a state transition — `InvalidRunTransition` would have
    /// to lie about a `to` state). The transaction reads `now` ONCE and binds
    /// it in every predicate/write — one consistent timestamp per call.
    pub fn claim_run(
        &self,
        id: &str,
        holder: &str,
        lease_seconds: i64,
    ) -> StorageResult<KpiIngestRun> {
        if lease_seconds <= 0 {
            return Err(StorageError::InvalidRunLeaseDuration {
                seconds: lease_seconds,
            });
        }
        let mut connection = self.db.checkout()?;
        let tx = connection.transaction_with_behavior(rusqlite::TransactionBehavior::Immediate)?;
        let now: String =
            tx.query_row("SELECT strftime('%Y-%m-%dT%H:%M:%fZ', 'now')", [], |row| {
                row.get(0)
            })?;
        let run = claim_run_on_connection(&tx, id, holder, lease_seconds, &now)?;
        tx.commit()?;
        Ok(run)
    }

    /// Set-once attach of the run's extraction context (ADR 0099, #384) —
    /// `start_kpi_ingest`'s resume path fills whatever `create_run_if_absent`
    /// left NULL. Requires a live lease for `holder`; legal in the claimable
    /// states. Each supplied value either fills a NULL column or matches the
    /// stored value (idempotent); a differing value is a typed
    /// `RunContextValueConflict`. The period descriptor is all-or-none by
    /// construction and cross-checked against an existing `period_id` pin.
    /// All-`None` input returns without touching the database.
    pub fn attach_run_context(
        &self,
        id: &str,
        holder: &str,
        ctx: &RunContextAttach<'_>,
    ) -> StorageResult<()> {
        if ctx.scope.is_none() && ctx.data_quality.is_none() && ctx.period.is_none() {
            return Ok(());
        }
        if let Some(scope) = ctx.scope {
            validate_scope_value(scope)?;
        }
        if let Some(quality) = ctx.data_quality {
            validate_data_quality_value(quality)?;
        }
        if let Some((_, period_type)) = ctx.period {
            validate_period_type_value(period_type)?;
        }
        let (fiscal_year, period_type) = match ctx.period {
            Some((fy, pt)) => (Some(fy), Some(pt)),
            None => (None, None),
        };

        let mut connection = self.db.checkout()?;
        let tx = connection.transaction_with_behavior(rusqlite::TransactionBehavior::Immediate)?;

        // Cross-check an attached descriptor against an existing period pin
        // BEFORE the write (inside the same Immediate transaction): the
        // descriptor must name the pinned period's natural key.
        if let Some((fy, pt)) = ctx.period {
            let pinned: Option<(i64, String)> = tx
                .query_row(
                    "SELECT p.fiscal_year, p.period_type
                     FROM kpi_ingest_runs r JOIN financial_periods p ON p.id = r.period_id
                     WHERE r.id = ?1",
                    [id],
                    |row| Ok((row.get(0)?, row.get(1)?)),
                )
                .optional()?;
            if let Some((pinned_fy, pinned_pt)) = pinned {
                if pinned_fy != fy || pinned_pt != pt {
                    return Err(StorageError::RunContextValueConflict {
                        id: id.to_owned(),
                        key: "period",
                        existing: format!("{pinned_fy}/{pinned_pt}"),
                        requested: format!("{fy}/{pt}"),
                    });
                }
            }
        }

        let sql = format!(
            "UPDATE kpi_ingest_runs SET
                scope = COALESCE(scope, ?2),
                data_quality = COALESCE(data_quality, ?3),
                period_fiscal_year = COALESCE(period_fiscal_year, ?4),
                period_type = COALESCE(period_type, ?5),
                updated_at = strftime('%Y-%m-%dT%H:%M:%fZ', 'now')
             WHERE id = ?1 AND status IN {CLAIMABLE_STATUSES_SQL}
               AND lease_holder = ?6
               AND lease_expires_at > strftime('%Y-%m-%dT%H:%M:%fZ', 'now')
               AND (?2 IS NULL OR scope IS NULL OR scope = ?2)
               AND (?3 IS NULL OR data_quality IS NULL OR data_quality = ?3)
               AND (?4 IS NULL OR (period_fiscal_year IS NULL AND period_type IS NULL)
                    OR (period_fiscal_year = ?4 AND period_type = ?5))"
        );
        let changed = tx.execute(
            &sql,
            params![
                id,
                ctx.scope,
                ctx.data_quality,
                fiscal_year,
                period_type,
                holder
            ],
        )?;
        if changed == 1 {
            tx.commit()?;
            return Ok(());
        }

        let Some(raw) = read_raw_run(&tx, id)? else {
            return Err(StorageError::KpiIngestRunNotFound { id: id.to_owned() });
        };
        let status = KpiIngestRunState::parse(&raw.status)?;
        let result = if !status.is_agent_claimable() {
            Err(StorageError::RunNotClaimable {
                id: id.to_owned(),
                status: status.as_str().to_owned(),
            })
        } else if raw.lease_holder.as_deref() != Some(holder)
            || !lease_is_live_on_connection(&tx, id, holder)?
        {
            Err(lease_refusal(&tx, &raw, id, holder)?)
        } else if let Some(conflict) = classify_context_conflict(&raw, ctx, id) {
            Err(conflict)
        } else {
            // Every guard re-reads as satisfied — a write raced the
            // classification window; surface the residual lease refusal.
            Err(lease_not_held(id, holder))
        };
        tx.commit()?;
        result
    }

    /// Keyset-paginated pending list (#384): pending = the claimable states.
    /// `after` is the exclusive `(created_at, id)` cursor of the previous
    /// page's last row; ordering is `created_at DESC, id DESC` (uniform
    /// direction — deliberately diverging from [`Self::list_runs`]' `id ASC`
    /// tie-break, which cannot express a single row-value comparison).
    pub fn list_pending_runs(
        &self,
        company_id: Option<&str>,
        after: Option<(&str, &str)>,
        limit: usize,
    ) -> StorageResult<Vec<KpiIngestRun>> {
        let connection = self.db.checkout()?;
        // ponytail: unfiltered pages scan a tens-of-rows table; add a
        // (created_at DESC, id DESC) index if this table ever grows hot.
        let mut sql = format!(
            "SELECT {RUN_COLUMNS} FROM kpi_ingest_runs
             WHERE status IN {CLAIMABLE_STATUSES_SQL}"
        );
        let mut parameters: Vec<Box<dyn rusqlite::ToSql>> = Vec::new();
        if let Some(company) = company_id {
            sql.push_str(&format!(" AND company_id = ?{}", parameters.len() + 1));
            parameters.push(Box::new(company.to_owned()));
        }
        if let Some((created_at, id)) = after {
            sql.push_str(&format!(
                " AND (created_at, id) < (?{}, ?{})",
                parameters.len() + 1,
                parameters.len() + 2
            ));
            parameters.push(Box::new(created_at.to_owned()));
            parameters.push(Box::new(id.to_owned()));
        }
        sql.push_str(&format!(
            " ORDER BY created_at DESC, id DESC LIMIT ?{}",
            parameters.len() + 1
        ));
        parameters.push(Box::new(limit as i64));

        let mut statement = connection.prepare(&sql)?;
        let raws: Vec<RawRun> = statement
            .query_map(
                rusqlite::params_from_iter(parameters.iter().map(|p| p.as_ref())),
                map_raw_row,
            )?
            .collect::<Result<_, _>>()?;
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
    /// leaves the row untouched but COUNTED in `mismatched` (surfaced by the
    /// startup log — needs manual intervention, not a crash-reclaim case); no receipt at all reverts to `ready_to_commit`
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
                // untouched but COUNTED (luna review P2), so the startup log
                // surfaces the row needing manual investigation.
                Some(_) => summary.mismatched += 1,
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
        // ADR 0102 dec. 11: startup reclaim clears drafts too — a draft
        // requires a live lease to be usable, so any draft left on a
        // currently leaseless run is orphaned regardless of whether ITS
        // lease expired before or during this reclaim pass.
        super::kpi_ingest_drafts::clear_orphaned_drafts_on_connection(&tx)?;
        tx.commit()?;
        Ok(summary)
    }

    /// Lease-gated create-or-reuse for `propose_kpi_definition` (ADR 0101 dec.
    /// 3/4/6/11, epic #399 S4). Re-checks the caller's live lease on `run_id`
    /// (the same three-way classification `stage_observations` uses), then
    /// forces `scope`/`company_id`/`sector`/`origin` from the run regardless
    /// of `input`'s values — a proposal is always company-scoped, `origin =
    /// agent`, never the caller's to set (dec. 6). Guard order (dec. 4): an
    /// exact `(metric_key, company)` duplicate is checked FIRST and returns
    /// typed (`created: false`, no alias lookup at all — a repeat proposal of
    /// an already-minted key must not suddenly redirect); then a curated
    /// `kpi_aliases` hit refuses with
    /// [`StorageError::KpiDefinitionSynonymRedirect`] (an alias source is
    /// deprecated even when its zero-fact canonical row still exists); then an
    /// exact match in the shared canon returns the CANONICAL row (`created:
    /// false` — never a company-scoped shadow); only a genuinely new key is
    /// created via [`financials::get_or_create_kpi_definition`] on the SAME
    /// connection.
    pub fn propose_kpi_definition(
        &self,
        run_id: &str,
        holder: &str,
        mut input: NewKpiDefinition,
    ) -> StorageResult<(KpiDefinition, bool)> {
        let mut connection = self.db.checkout()?;
        let tx = connection.transaction_with_behavior(rusqlite::TransactionBehavior::Immediate)?;

        let now: String =
            tx.query_row("SELECT strftime('%Y-%m-%dT%H:%M:%fZ', 'now')", [], |row| {
                row.get(0)
            })?;
        let row: Option<(String, Option<String>, Option<String>)> = tx
            .query_row(
                "SELECT company_id, lease_holder, lease_expires_at FROM kpi_ingest_runs WHERE id = ?1",
                [run_id],
                |row| Ok((row.get(0)?, row.get(1)?, row.get(2)?)),
            )
            .optional()?;
        let Some((company_id, lease_holder, lease_expires_at)) = row else {
            return Err(StorageError::KpiIngestRunNotFound {
                id: run_id.to_owned(),
            });
        };
        let lease_live = lease_holder.as_deref() == Some(holder)
            && lease_expires_at
                .as_deref()
                .is_some_and(|expires| expires > now.as_str());
        if !lease_live {
            // Three-way classification shared with staging (#386/#399):
            // own-expired -> RunLeaseExpired, live-foreign -> RunTakenOver,
            // residual -> RunLeaseNotHeld.
            return Err(lease_refusal_on_connection(&tx, run_id, holder)?);
        }

        input.scope = "company".to_owned();
        input.company_id = Some(company_id.clone());
        input.sector = None;
        input.origin = Some("agent".to_owned());
        let metric_key = input.metric_key.trim().to_owned();

        if financials::find_company_kpi_definition(&tx, &company_id, &metric_key)?.is_none() {
            // ADR 0101 dec. 4: the curated alias redirect is consulted BEFORE
            // the shared-canon reuse — an alias source is a deprecated key
            // whose zero-fact canonical row may still exist (inventory →
            // inventories), and reusing it would resurrect the fragmentation
            // the alias retires.
            if let Some(canonical_key) = crate::fundamentals::kpi_aliases::resolve(&metric_key) {
                return Err(StorageError::KpiDefinitionSynonymRedirect {
                    requested_key: metric_key,
                    canonical_key: canonical_key.to_owned(),
                    definition_id: financials::canonical_kpi_definition_id(canonical_key),
                });
            }
            // ADR 0101 dec. 3/4: exact-key reuse is against the WHOLE catalog
            // — a key the shared canon already carries returns the canonical
            // row (`created: false`), never a company-scoped shadow.
            if let Some(canonical) =
                financials::find_canonical_kpi_definition_by_key(&tx, &metric_key)?
            {
                return Ok((canonical, false));
            }
        }

        let result = financials::get_or_create_kpi_definition(&tx, input)?;
        tx.commit()?;
        Ok(result)
    }
}

/// `ready_to_commit -> committing` (#360, ADR 0098 dec. 6/5): connection-level
/// free fn `KpiIngestCommitStore::commit_manifest` (#362) calls under its own
/// externally-owned `&Connection` (the `record_structured_fact` pattern) —
/// composing the PUBLIC, connection-checking-out store methods inside an
/// outer transaction is prohibited (ADR 0098 dec. 5). Guard: status =
/// `ready_to_commit`, `manifest_hash`/`manifest_revision` match the caller's
/// (mismatch -> `StaleManifestForCommit`), lease is NULL (ADR 0098 dec. 6
/// invariant: `mark_ready_to_commit` clears it; a non-null lease here is a
/// structural bug, `RunLeaseInvariantViolation`, checked FIRST).
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
/// level free fn `KpiIngestCommitStore::commit_manifest` (#362) calls on the
/// SAME handle AFTER it writes the commit receipt (`kpi_ingest_staging::
/// record_commit_receipt`) in the same outer transaction. Deliberately takes
/// NO terminal-status parameter (B2 sol — a structural gate, not a
/// doc-comment promise): it reads and verifies the receipt ITSELF —
/// `manifest_hash`/`manifest_revision` must match the run row — and derives
/// `complete`/`partial` from `receipt.terminal_status`. No receipt yet ->
/// `RunTransitionPrerequisiteMissing`; a receipt that disagrees with the run
/// -> `StaleManifestForCommit`. Returns the terminal state it set.
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

/// Writes the diagnostic `progress_json` snapshot (#364, ADR 0098 dec. 2).
/// Called ONLY from inside the validation atom (`apply_validation_outcome`)
/// and the commit transaction — no writer exists outside those atoms, so a
/// stale writer can never overwrite newer progress and a rolled-back commit
/// leaves no `committed` snapshot behind. Diagnostic like `cost_json`: never
/// part of the trust verdict.
pub(super) fn write_progress_snapshot_on_connection(
    conn: &Connection,
    id: &str,
    step: &str,
    revision: i64,
    manifest_hash: Option<&str>,
    counts: serde_json::Value,
) -> StorageResult<()> {
    #[derive(serde::Serialize)]
    #[serde(rename_all = "camelCase")]
    struct ProgressSnapshot<'a> {
        schema_version: i64,
        step: &'a str,
        revision: i64,
        manifest_hash: Option<&'a str>,
        at: String,
        counts: serde_json::Value,
    }
    let at: String = conn.query_row("SELECT strftime('%Y-%m-%dT%H:%M:%fZ', 'now')", [], |row| {
        row.get(0)
    })?;
    let json = serde_json::to_string(&ProgressSnapshot {
        schema_version: 1,
        step,
        revision,
        manifest_hash,
        at,
        counts,
    })?;
    let changed = conn.execute(
        "UPDATE kpi_ingest_runs SET progress_json = ?1, \
         updated_at = strftime('%Y-%m-%dT%H:%M:%fZ', 'now') WHERE id = ?2",
        params![json, id],
    )?;
    if changed == 0 {
        return Err(StorageError::KpiIngestRunNotFound { id: id.to_owned() });
    }
    Ok(())
}

/// The single `cost_json` writer (#386, ADR 0099 dec. 8): shallow per-key
/// merge of the caller's `ExecutionMeta` payload over the stored object —
/// fields OMITTED by this call survive (the agent may report `tokensIn` at
/// stage and `costUsd` at commit; numerics are totals/snapshots, never
/// accumulated), `schemaVersion` stamped 1. Runs ONLY inside the stage and
/// commit transactions (no loose last-writer-wins update exists). A corrupt
/// stored object is replaced fresh — `cost_json` is diagnostic, never part of
/// the trust verdict (ADR 0098 dec. 2).
pub(super) fn merge_cost_json_on_connection(
    conn: &Connection,
    id: &str,
    execution: &serde_json::Value,
) -> StorageResult<()> {
    let stored: Option<String> = conn
        .query_row(
            "SELECT cost_json FROM kpi_ingest_runs WHERE id = ?1",
            [id],
            |row| row.get(0),
        )
        .optional()?
        .flatten();
    let mut merged = stored
        .as_deref()
        .and_then(|raw| serde_json::from_str::<serde_json::Value>(raw).ok())
        .and_then(|value| value.as_object().cloned())
        .unwrap_or_default();
    if let Some(fields) = execution.as_object() {
        for (key, value) in fields {
            merged.insert(key.clone(), value.clone());
        }
    }
    merged.insert("schemaVersion".to_owned(), serde_json::json!(1));
    let json = serde_json::Value::Object(merged).to_string();
    let changed = conn.execute(
        "UPDATE kpi_ingest_runs SET cost_json = ?1, \
         updated_at = strftime('%Y-%m-%dT%H:%M:%fZ', 'now') WHERE id = ?2",
        params![json, id],
    )?;
    if changed == 0 {
        return Err(StorageError::KpiIngestRunNotFound { id: id.to_owned() });
    }
    Ok(())
}

/// Attaches the commit transaction's resolved period id onto the run (#362
/// step 4, the `(manifest.periodId=None, run.period_id=None)` branch): a
/// same-or-set guarded write under `status='committing'` — 0 rows updated
/// (including a raced non-NULL mismatch) is a typed `CommitPeriodConflict`,
/// never silent acceptance. The other three period-match branches (#362) run
/// entirely in the caller and never reach this write.
pub(super) fn attach_period_on_connection(
    conn: &Connection,
    id: &str,
    period_id: &str,
) -> StorageResult<()> {
    let changed = conn.execute(
        "UPDATE kpi_ingest_runs SET period_id = ?1, \
         updated_at = strftime('%Y-%m-%dT%H:%M:%fZ', 'now') \
         WHERE id = ?2 AND status = 'committing' AND (period_id IS NULL OR period_id = ?1)",
        params![period_id, id],
    )?;
    if changed == 1 {
        return Ok(());
    }
    Err(StorageError::CommitPeriodConflict {
        run: id.to_owned(),
        reason: "period attach raced or the run left committing before it landed",
    })
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
            .create_run_if_absent(&new_run(doc, company, "gpw_ifrs_annual@v1"))
            .expect("create");
        let second = store
            .create_run_if_absent(&new_run(doc, company, "gpw_ifrs_annual@v1"))
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
            .create_run_if_absent(&new_run(doc, company, "gpw_ifrs_annual@v1"))
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
            .create_run_if_absent(&new_run(doc, company, "gpw_ifrs_annual@v1"))
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
            .create_run_if_absent(&new_run(doc, company, "gpw_ifrs_annual@v1"))
            .expect("create p1");
        let p2 = store
            .create_run_if_absent(&new_run(doc, company, "gpw_interim@v1"))
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
            .create_run_if_absent(&new_run("doc1", "c2", "gpw_ifrs_annual@v1"))
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
            .create_run_if_absent(&new_run(doc, company, "gpw_ifrs_annual@v1"))
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
            .create_run_if_absent(&new_run(doc, company, "gpw_ifrs_annual@v1"))
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
            .create_run_if_absent(&new_run(doc, company, "gpw_ifrs_annual@v1"))
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
            .create_run_if_absent(&new_run(doc, company, "gpw_ifrs_annual@v1"))
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
            .create_run_if_absent(&new_run("doc1", "c1", "gpw_ifrs_annual@v1"))
            .expect("r1");
        let r2 = store
            .create_run_if_absent(&new_run("doc2", "c2", "gpw_ifrs_annual@v1"))
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
        let missing_doc = new_run("doc-missing", company, "gpw_ifrs_annual@v1");
        let error = store
            .create_run_if_absent(&missing_doc)
            .expect_err("unknown document");
        assert!(matches!(error, StorageError::MissingIngestReference { .. }));

        // Unknown period.
        let mut unknown_period = new_run(doc, company, "gpw_ifrs_annual@v1");
        unknown_period.period_id = Some("finper_missing".to_owned());
        let error = store
            .create_run_if_absent(&unknown_period)
            .expect_err("unknown period");
        assert!(matches!(error, StorageError::MissingIngestReference { .. }));

        // Period owned by another company.
        let mut foreign_period = new_run(doc, company, "gpw_ifrs_annual@v1");
        foreign_period.period_id = Some("finper_other".to_owned());
        let error = store
            .create_run_if_absent(&foreign_period)
            .expect_err("foreign period");
        assert!(matches!(
            error,
            StorageError::RunPeriodCompanyMismatch { .. }
        ));

        // Invalid vocabulary values are typed refusals, not raw CHECK conflicts.
        let mut bad_scope = new_run(doc, company, "gpw_ifrs_annual@v1");
        bad_scope.scope = Some("group".to_owned());
        assert!(matches!(
            store.create_run_if_absent(&bad_scope).expect_err("scope"),
            StorageError::InvalidKpiIngestRunValue { key: "scope", .. }
        ));
        let mut bad_quality = new_run(doc, company, "gpw_ifrs_annual@v1");
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

    /// ADR 0099 dec. 6: `profile_version` is validated against the extraction-
    /// profile registry at creation — stale, future, bare and free-form
    /// strings are all typed refusals.
    #[test]
    fn create_run_rejects_a_profile_version_outside_the_registry() {
        let connection = open_in_memory_database().expect("db");
        seed_company(&connection, "c1");
        seed_document(&connection, "doc1", "c1");
        let state = AppState::new(connection);
        let store = state.kpi_ingest_runs();

        for bad in [
            "p1",
            "gpw_ifrs_annual@v0",
            "gpw_ifrs_annual@v2",
            "gpw_ifrs_annual",
        ] {
            assert!(
                matches!(
                    store
                        .create_run_if_absent(&new_run("doc1", "c1", bad))
                        .expect_err("must refuse"),
                    StorageError::InvalidKpiIngestRunValue {
                        key: "profile_version",
                        ..
                    }
                ),
                "profile_version {bad} must be refused"
            );
        }
    }

    fn parsed_stamp(run: &KpiIngestRun) -> crate::fundamentals::kpi_manifest::ExpectedKpis {
        serde_json::from_str(run.expected_kpis_json.as_deref().expect("stamp present"))
            .expect("stamp parses")
    }

    fn seed_relevance(connection: &Connection, company: &str, definition_id: &str) {
        connection
            .execute(
                "INSERT INTO kpi_relevance (id, company_id, definition_id, status, source, rank)
                 VALUES (?1, ?2, ?3, 'active', 'curated', 'primary')",
                params![
                    format!("kpirel_{company}_{definition_id}"),
                    company,
                    definition_id
                ],
            )
            .expect("relevance");
    }

    /// The pack contributes independently of relevance: a raw banking company
    /// with ZERO relevance rows still gets the full 7-key banking floor.
    #[test]
    fn creation_stamps_the_banking_floor_independent_of_relevance() {
        let connection = open_in_memory_database().expect("db");
        seed_company(&connection, "c1");
        seed_document(&connection, "doc1", "c1");
        connection
            .execute(
                "UPDATE companies SET statement_type = 'banking' WHERE id = 'c1'",
                [],
            )
            .expect("classify");
        let state = AppState::new(connection);

        let run = state
            .kpi_ingest_runs()
            .create_run_if_absent(&new_run("doc1", "c1", "gpw_ifrs_annual@v1"))
            .expect("create");
        let stamp = parsed_stamp(&run);
        assert_eq!(stamp.source, "kpi_relevance+profile_pack");
        assert_eq!(stamp.pack_version.as_deref(), Some("gpw_ifrs_annual@v1"));
        let keys: Vec<&str> = stamp.keys.iter().map(String::as_str).collect();
        assert_eq!(
            keys,
            [
                "net_fee_commission_income",
                "net_interest_income",
                "net_profit",
                "total_assets",
                "total_deposits",
                "total_equity",
                "total_loans",
            ],
            "exactly the banking floor — no revenue, no operating_profit"
        );
    }

    /// The relevance side of the union: a curated primary relevance row for a
    /// key OUTSIDE the industrial floor (ebitda) joins the stamp.
    #[test]
    fn creation_stamp_unions_live_relevance_with_the_pack() {
        let connection = open_in_memory_database().expect("db");
        seed_company(&connection, "c1");
        seed_document(&connection, "doc1", "c1");
        seed_relevance(&connection, "c1", "kpidef_ebitda");
        let state = AppState::new(connection);

        let run = state
            .kpi_ingest_runs()
            .create_run_if_absent(&new_run("doc1", "c1", "gpw_ifrs_annual@v1"))
            .expect("create");
        let stamp = parsed_stamp(&run);
        assert!(
            stamp.keys.contains("ebitda"),
            "relevance key joins the union"
        );
        for floor_key in [
            "revenue",
            "operating_profit",
            "net_profit",
            "total_assets",
            "total_equity",
        ] {
            assert!(
                stamp.keys.contains(floor_key),
                "floor key {floor_key} present"
            );
        }
    }

    /// Owner decision 2026-08-14: the union applies to EVERY profile —
    /// `company_characteristic`'s empty pack never zeroes out live relevance.
    #[test]
    fn company_characteristic_unions_relevance_with_an_empty_pack() {
        let connection = open_in_memory_database().expect("db");
        seed_company(&connection, "c1");
        seed_document(&connection, "doc1", "c1");
        seed_relevance(&connection, "c1", "kpidef_ebitda");
        let state = AppState::new(connection);

        let run = state
            .kpi_ingest_runs()
            .create_run_if_absent(&new_run("doc1", "c1", "company_characteristic@v1"))
            .expect("create");
        let stamp = parsed_stamp(&run);
        assert_eq!(
            stamp.pack_version.as_deref(),
            Some("company_characteristic@v1")
        );
        let keys: Vec<&str> = stamp.keys.iter().map(String::as_str).collect();
        assert_eq!(keys, ["ebitda"], "relevance union with an empty pack");
    }

    /// The idempotent hit returns the run with its FROZEN stamp — relevance
    /// changes after creation never restamp (ADR 0093 dec. 4).
    #[test]
    fn an_idempotent_hit_never_restamps() {
        let connection = open_in_memory_database().expect("db");
        seed_company(&connection, "c1");
        seed_document(&connection, "doc1", "c1");
        let state = AppState::new(connection);
        let store = state.kpi_ingest_runs();

        let first = store
            .create_run_if_absent(&new_run("doc1", "c1", "gpw_ifrs_annual@v1"))
            .expect("create");
        {
            let connection = state.checkout_for_tests().expect("raw connection");
            seed_relevance(&connection, "c1", "kpidef_ebitda");
        }
        let second = store
            .create_run_if_absent(&new_run("doc1", "c1", "gpw_ifrs_annual@v1"))
            .expect("idempotent");
        assert_eq!(second.id, first.id);
        assert_eq!(
            second.expected_kpis_json, first.expected_kpis_json,
            "byte-identical frozen stamp"
        );
        assert!(!parsed_stamp(&second).keys.contains("ebitda"));
    }

    /// The legacy-NULL fallback writer: refuses a `discovered` row, and is
    /// set-once against an existing (creation-time) stamp.
    #[test]
    fn stamp_expected_kpis_refuses_wrong_status_and_is_set_once() {
        let connection = open_in_memory_database().expect("db");
        seed_company(&connection, "c1");
        seed_document(&connection, "doc1", "c1");
        seed_run_raw(
            &connection,
            "run-legacy",
            "doc1",
            "c1",
            "p1",
            "discovered",
            None,
            None,
            None,
        );
        let state = AppState::new(connection);
        let store = state.kpi_ingest_runs();

        // Wrong status (discovered, NULL column) → typed refusal.
        assert!(matches!(
            store
                .stamp_expected_kpis("run-legacy", 1, "{}")
                .expect_err("discovered must refuse"),
            StorageError::InvalidStagingRevision { .. }
        ));

        // A created (creation-stamped) run ignores a competing snapshot and
        // returns the frozen bytes unchanged.
        let run = store
            .create_run_if_absent(&new_run("doc1", "c1", "gpw_ifrs_annual@v1"))
            .expect("create");
        let frozen = run.expected_kpis_json.clone().expect("stamped at creation");
        let returned = store
            .stamp_expected_kpis(&run.id, 1, "{\"schemaVersion\":9}")
            .expect("set-once read-back");
        assert_eq!(returned, frozen);
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
        let mut with_descriptor = new_run("doc1", "c1", "gpw_ifrs_annual@v1");
        with_descriptor.period_fiscal_year = Some(2025);
        with_descriptor.period_type = Some("FY".to_owned());
        let created = store
            .create_run_if_absent(&with_descriptor)
            .expect("descriptor-only run must be creatable");
        assert_eq!(created.period_fiscal_year, Some(2025));
        assert_eq!(created.period_type.as_deref(), Some("FY"));
        assert!(created.period_id.is_none());

        // A partial descriptor is refused.
        let mut partial = new_run("doc1", "c1", "gpw_interim@v1");
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
        let mut contradictory = new_run("doc1", "c1", "gpw_preliminary@v1");
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

        // A second create on the SAME active triple (one profile) with a DIFFERENT
        // descriptor conflicts with the already-running one.
        let mut conflicting = new_run("doc1", "c1", "gpw_ifrs_annual@v1");
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
        let mut with_id = new_run("doc1", "c1", "gpw_ifrs_annual@v1");
        with_id.period_id = Some("finper_fy2025".to_owned());
        let created = store.create_run_if_absent(&with_id).expect("create");
        let mut descriptor_mismatch = new_run("doc1", "c1", "gpw_ifrs_annual@v1");
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
        let mut descriptor_match = new_run("doc1", "c1", "gpw_ifrs_annual@v1");
        descriptor_match.period_fiscal_year = Some(2025);
        descriptor_match.period_type = Some("FY".to_owned());
        let same = store
            .create_run_if_absent(&descriptor_match)
            .expect("the matching descriptor is not a conflict");
        assert_eq!(same.id, created.id);

        // Reverse direction: existing run holds a descriptor; request carries
        // a period_id resolving to a different natural key — must conflict.
        let mut with_descriptor = new_run("doc2", "c1", "gpw_ifrs_annual@v1");
        with_descriptor.period_fiscal_year = Some(2024);
        with_descriptor.period_type = Some("FY".to_owned());
        let created2 = store
            .create_run_if_absent(&with_descriptor)
            .expect("create");
        let mut id_mismatch = new_run("doc2", "c1", "gpw_ifrs_annual@v1");
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
            .create_run_if_absent(&ready_new_run(doc, company, "gpw_ifrs_annual@v1"))
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
            .stage_observations(
                run_id,
                holder,
                vec![one_test_observation()],
                &std::collections::BTreeMap::new(),
                None,
            )
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
        let connection = store.db.checkout().expect("checkout");
        mark_ready_to_commit_on_connection(&connection, &run.id, revision, manifest_hash)
            .expect("ready");
        drop(connection);
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
            .create_run_if_absent(&ready_new_run(doc, company, "gpw_ifrs_annual@v1"))
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
            let mut run_def = new_run(doc, company, "gpw_ifrs_annual@v1");
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
            .create_run_if_absent(&ready_new_run(doc, company, "gpw_ifrs_annual@v1"))
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

    // `mark_validation_failed`/`mark_ready_to_commit` no longer have a public
    // seam (#361 sol r4: the only caller is `kpi_ingest_staging::
    // apply_validation_outcome`'s atom, which inserts an immutable attempt
    // row before ever calling either connection-level primitive below — a
    // hash-bearing `ready_to_commit`/`validation_failed` row with no
    // attempt evidence is structurally uncreatable). These tests now drive
    // the primitives directly on a checked-out connection, the same
    // guarantees the atom's own test suite (`kpi_ingest_staging::tests`)
    // re-exercises end to end.

    #[test]
    fn mark_validation_failed_on_connection_happy_path_targets_the_current_revision() {
        let (state, doc, company) = setup_one_company_doc();
        let run = advance_to_extracting(&state, doc, company, "worker-a");
        let revision = stage_once(&state, &run.id, "worker-a");

        let store = state.kpi_ingest_runs();
        let connection = store.db.checkout().expect("checkout");
        mark_validation_failed_on_connection(&connection, &run.id, revision).expect("mark failed");
        drop(connection);
        let after = store.get_run(&run.id).expect("get").expect("some");
        assert_eq!(after.status, KpiIngestRunState::ValidationFailed);
    }

    #[test]
    fn mark_validation_failed_on_connection_refuses_a_stale_revision() {
        let (state, doc, company) = setup_one_company_doc();
        let run = advance_to_extracting(&state, doc, company, "worker-a");
        let revision = stage_once(&state, &run.id, "worker-a");

        let store = state.kpi_ingest_runs();
        let connection = store.db.checkout().expect("checkout");
        let error = mark_validation_failed_on_connection(&connection, &run.id, revision + 1)
            .expect_err("stale revision");
        assert!(matches!(error, StorageError::InvalidStagingRevision { .. }));
    }

    #[test]
    fn mark_validation_failed_on_connection_refuses_wrong_state_with_unchanged_snapshot() {
        let (state, doc, company) = setup_one_company_doc();
        let store = state.kpi_ingest_runs();
        let run = store
            .create_run_if_absent(&ready_new_run(doc, company, "gpw_ifrs_annual@v1"))
            .expect("create");
        let before = store.get_run(&run.id).expect("get").expect("some");
        let connection = store.db.checkout().expect("checkout");
        let error = mark_validation_failed_on_connection(&connection, &run.id, 0)
            .expect_err("still discovered");
        assert!(matches!(error, StorageError::InvalidRunTransition { .. }));
        drop(connection);
        assert_eq!(before, store.get_run(&run.id).expect("get").expect("some"));
    }

    #[test]
    fn mark_ready_to_commit_on_connection_happy_path_freezes_manifest_and_clears_lease() {
        let (state, doc, company) = setup_one_company_doc();
        let run = advance_to_extracting(&state, doc, company, "worker-a");
        let revision = stage_once(&state, &run.id, "worker-a");

        let store = state.kpi_ingest_runs();
        let connection = store.db.checkout().expect("checkout");
        mark_ready_to_commit_on_connection(&connection, &run.id, revision, "hash-abc")
            .expect("ready");
        drop(connection);
        let after = store.get_run(&run.id).expect("get").expect("some");
        assert_eq!(after.status, KpiIngestRunState::ReadyToCommit);
        assert_eq!(after.manifest_hash.as_deref(), Some("hash-abc"));
        assert!(after.lease_holder.is_none());
        assert!(after.lease_expires_at.is_none());
        assert!(after.last_heartbeat_at.is_none());
    }

    #[test]
    fn mark_ready_to_commit_on_connection_refuses_a_stale_revision() {
        let (state, doc, company) = setup_one_company_doc();
        let run = advance_to_extracting(&state, doc, company, "worker-a");
        let revision = stage_once(&state, &run.id, "worker-a");

        let store = state.kpi_ingest_runs();
        let connection = store.db.checkout().expect("checkout");
        let error =
            mark_ready_to_commit_on_connection(&connection, &run.id, revision + 1, "hash-abc")
                .expect_err("stale revision");
        assert!(matches!(error, StorageError::InvalidStagingRevision { .. }));
    }

    /// Structurally unreachable via any production seam (the atom's own guard
    /// requires `manifest_hash IS NULL` before ever calling this primitive) —
    /// raw-seeded defensively, the same carve-out `kpi_ingest_staging`'s
    /// frozen-revision coverage uses.
    #[test]
    fn mark_ready_to_commit_on_connection_refuses_when_a_manifest_is_already_issued() {
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
        let error = mark_ready_to_commit_on_connection(&connection, "run1", 1, "new-hash")
            .expect_err("already issued");
        assert!(matches!(error, StorageError::InvalidStagingRevision { .. }));
    }

    #[test]
    fn invalidate_manifest_clears_hash_and_returns_to_staged() {
        let (state, doc, company) = setup_one_company_doc();
        let run = advance_to_extracting(&state, doc, company, "worker-a");
        let revision = stage_once(&state, &run.id, "worker-a");
        let store = state.kpi_ingest_runs();
        let connection = store.db.checkout().expect("checkout");
        mark_ready_to_commit_on_connection(&connection, &run.id, revision, "hash-abc")
            .expect("ready");
        drop(connection);

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
            .create_run_if_absent(&ready_new_run(doc, company, "gpw_ifrs_annual@v1"))
            .expect("create");
        let before = store.get_run(&run.id).expect("get").expect("some");
        let error = store
            .invalidate_manifest(&run.id)
            .expect_err("still discovered");
        assert!(matches!(error, StorageError::InvalidRunTransition { .. }));
        assert_eq!(before, store.get_run(&run.id).expect("get").expect("some"));
    }

    // --- Test 3: caller/lease — shared table-driven corpus for the three
    // agent-facing intents (F9 r2; #384 splits the refined classification:
    // a live FOREIGN lease → RunTakenOver and the caller's own EXPIRED lease
    // → RunLeaseExpired on the lease_refusal-classified intents, while
    // staging keeps the residual RunLeaseNotHeld until #386's tools land) ---

    #[test]
    fn agent_intents_share_the_same_three_lease_failure_scenarios() {
        fn assert_expected(scenario: Scenario, error: &StorageError, refined: bool) {
            match (scenario, refined) {
                (Scenario::WrongHolder, true) => assert!(
                    matches!(error, StorageError::RunTakenOver { .. }),
                    "live foreign lease → RunTakenOver, got {error:?}"
                ),
                (Scenario::Expired, true) => assert!(
                    matches!(error, StorageError::RunLeaseExpired { .. }),
                    "own expired lease → RunLeaseExpired, got {error:?}"
                ),
                _ => assert!(
                    matches!(error, StorageError::RunLeaseNotHeld { .. }),
                    "residual shape, got {error:?}"
                ),
            }
        }
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
                    .create_run_if_absent(&new_run(doc, company, "gpw_ifrs_annual@v1"))
                    .expect("create");
                apply(&state, &run.id, scenario, "worker-a");
                let error = store
                    .mark_source_captured(&run.id, "worker-a", "hash1")
                    .expect_err("lease scenario must refuse");
                assert_expected(scenario, &error, true);
            }
            // begin_extracting.
            {
                let (state, doc, company) = setup_one_company_doc();
                let store = state.kpi_ingest_runs();
                let run = store
                    .create_run_if_absent(&ready_new_run(doc, company, "gpw_ifrs_annual@v1"))
                    .expect("create");
                store.claim_next("worker-a", 3600).expect("claim");
                store
                    .mark_source_captured(&run.id, "worker-a", "hash1")
                    .expect("capture");
                apply(&state, &run.id, scenario, "worker-a");
                let error = store
                    .begin_extracting(&run.id, "worker-a", "instr-1")
                    .expect_err("lease scenario must refuse");
                assert_expected(scenario, &error, true);
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
                    .stage_observations(
                        "run1",
                        "worker-a",
                        vec![one_test_observation()],
                        &std::collections::BTreeMap::new(),
                        None,
                    )
                    .expect_err("lease scenario must refuse");
                // Since #386 staging classifies through the shared three-way
                // `lease_refusal` — the refined vocabulary all three
                // agent-facing intents now share.
                assert_expected(scenario, &error, true);
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

    /// A receipt with the CORRECT hash but a different revision must also
    /// refuse; and a `partial` receipt finalizes to `Partial`, derived only
    /// from the receipt (luna review test gap).
    #[test]
    fn finalize_committing_checks_revision_and_derives_partial_from_the_receipt() {
        let (state, doc, company) = setup_one_company_doc();
        let ready = advance_to_ready_to_commit(&state, doc, company, "worker-a", "hash-abc");
        let mut connection = state.checkout_for_tests().expect("raw connection");
        let tx = connection
            .transaction_with_behavior(rusqlite::TransactionBehavior::Immediate)
            .expect("tx");
        begin_committing(&tx, &ready.id, "hash-abc", ready.manifest_revision).expect("begin");

        // Correct hash, wrong revision.
        kpi_ingest_staging::record_commit_receipt(
            &tx,
            NewCommitReceipt {
                run_id: ready.id.clone(),
                manifest_hash: "hash-abc".to_owned(),
                manifest_revision: ready.manifest_revision + 7,
                terminal_status: "partial".to_owned(),
                period_id: None,
                accepted_count: 0,
                outcomes_schema_version: 1,
                outcomes_json: "[]".to_owned(),
            },
        )
        .expect("receipt");
        let error = finalize_committing(&tx, &ready.id).expect_err("revision mismatch");
        assert!(matches!(error, StorageError::StaleManifestForCommit { .. }));
        tx.rollback().expect("rollback");

        // Fresh transaction: matching receipt with terminal_status='partial'.
        let tx = connection
            .transaction_with_behavior(rusqlite::TransactionBehavior::Immediate)
            .expect("tx2");
        begin_committing(&tx, &ready.id, "hash-abc", ready.manifest_revision).expect("begin2");
        kpi_ingest_staging::record_commit_receipt(
            &tx,
            NewCommitReceipt {
                run_id: ready.id.clone(),
                manifest_hash: "hash-abc".to_owned(),
                manifest_revision: ready.manifest_revision,
                terminal_status: "partial".to_owned(),
                period_id: None,
                accepted_count: 0,
                outcomes_schema_version: 1,
                outcomes_json: "[]".to_owned(),
            },
        )
        .expect("receipt2");
        let terminal = finalize_committing(&tx, &ready.id).expect("finalize");
        assert_eq!(terminal, KpiIngestRunState::Partial);
        tx.commit().expect("commit");
        drop(connection);
        let after = state
            .kpi_ingest_runs()
            .get_run(&ready.id)
            .expect("get")
            .expect("some");
        assert_eq!(after.status, KpiIngestRunState::Partial);
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
        assert_eq!(
            summary.mismatched, 1,
            "the mismatch must be counted so the startup log surfaces it"
        );
        assert!(!summary.is_noop(), "a mismatch is never a silent no-op");
        let after = store.get_run("run1").expect("get").expect("some");
        assert_eq!(
            before, after,
            "a mismatched receipt must leave the run untouched"
        );
    }

    /// A `committing` row that BOTH violates the lease invariant AND has a
    /// matching receipt must NOT be finalized — the violation wins and the
    /// row stays untouched for manual investigation.
    #[test]
    fn reclaim_violation_wins_over_a_matching_receipt() {
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
            Some("2099-01-01T00:00:00.000Z"),
            Some("2026-01-01T00:00:00.000Z"),
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
                 VALUES ('r1', 'run1', 'hash-abc', 1, 'complete', 0, '[]')",
                [],
            )
            .expect("matching receipt");
        let state = AppState::new(connection);
        let store = state.kpi_ingest_runs();
        let before = store.get_run("run1").expect("get").expect("some");
        let summary = store.reclaim_ingest_runs_on_startup().expect("reclaim");
        assert_eq!(summary.violations, 1);
        assert_eq!(
            summary.finalized, 0,
            "a lease-violating committing row must never be finalized"
        );
        let after = store.get_run("run1").expect("get").expect("some");
        assert_eq!(before, after);
    }

    /// Legacy set-once semantics survive the retired lease-free capture path
    /// (luna review B2): a `discovered` row already carrying a hash refuses a
    /// DIFFERENT hash and stays put; the SAME hash transitions idempotently.
    #[test]
    fn mark_source_captured_respects_a_legacy_pre_existing_hash() {
        let (state, doc, company) = setup_one_company_doc();
        let store = state.kpi_ingest_runs();
        let run = store
            .create_run_if_absent(&ready_new_run(doc, company, "gpw_ifrs_annual@v1"))
            .expect("create");
        store.claim_next("worker-a", 60).expect("claim");
        // Simulate the retired lease-free path: hash present, still discovered.
        {
            let db = state.checkout_for_tests().expect("raw connection");
            db.execute(
                "UPDATE kpi_ingest_runs SET source_content_hash = 'legacy-hash' WHERE id = ?1",
                [&run.id],
            )
            .expect("legacy hash");
        }
        let error = store
            .mark_source_captured(&run.id, "worker-a", "different-hash")
            .expect_err("a different hash must refuse, never overwrite");
        assert!(matches!(
            error,
            StorageError::RunSourceHashAlreadyRecorded { .. }
        ));
        let unchanged = store.get_run(&run.id).expect("get").expect("some");
        assert_eq!(
            unchanged.source_content_hash.as_deref(),
            Some("legacy-hash")
        );
        assert_eq!(unchanged.status, KpiIngestRunState::Discovered);

        store
            .mark_source_captured(&run.id, "worker-a", "legacy-hash")
            .expect("the same hash transitions idempotently");
        let after = store.get_run(&run.id).expect("get").expect("some");
        assert_eq!(after.status, KpiIngestRunState::SourceCaptured);
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

    // ---- claim_run / lease classification / attach / list_pending (#384) ----

    fn expire_lease_raw(state: &AppState, id: &str) {
        let connection = state.checkout_for_tests().expect("raw connection");
        connection
            .execute(
                "UPDATE kpi_ingest_runs SET lease_expires_at = \
                 strftime('%Y-%m-%dT%H:%M:%fZ', 'now', '-1 seconds') WHERE id = ?1",
                [id],
            )
            .expect("expire lease");
    }

    #[test]
    fn claim_run_claims_renews_and_counts_attempts() {
        let (state, doc, company) = setup_one_company_doc();
        let store = state.kpi_ingest_runs();
        let run = store
            .create_run_if_absent(&new_run(doc, company, "gpw_ifrs_annual@v1"))
            .expect("create");
        assert_eq!(run.attempt_count, 0);

        // First claim: attempt 0 -> 1.
        let claimed = store.claim_run(&run.id, "mcp:full", 3600).expect("claim");
        assert_eq!(claimed.attempt_count, 1);
        assert_eq!(claimed.lease_holder.as_deref(), Some("mcp:full"));
        let first_expiry = claimed.lease_expires_at.clone().expect("lease set");

        // Same-holder live re-claim = renewal (the keepalive): NO attempt++.
        let renewed = store.claim_run(&run.id, "mcp:full", 7200).expect("renew");
        assert_eq!(renewed.attempt_count, 1, "keepalive never increments");
        assert!(
            renewed.lease_expires_at.expect("lease") > first_expiry,
            "renewal extends the lease"
        );

        // Same-holder EXPIRED re-claim is a fresh claim (never resurrected).
        expire_lease_raw(&state, &run.id);
        let reclaimed = store.claim_run(&run.id, "mcp:full", 3600).expect("reclaim");
        assert_eq!(reclaimed.attempt_count, 2);
    }

    /// The issue-named takeover collision, on the deterministic fixed clock:
    /// just BEFORE expiry the foreign claim refuses (`RunTakenOver`), AT the
    /// exact expiry instant it succeeds (the `<=` claim branch) with one
    /// increment, and the ousted holder then gets `RunTakenOver` back.
    #[test]
    fn claim_run_takeover_collision_around_the_expiry_instant() {
        let (state, doc, company) = setup_one_company_doc();
        let store = state.kpi_ingest_runs();
        let run = store
            .create_run_if_absent(&new_run(doc, company, "gpw_ifrs_annual@v1"))
            .expect("create");
        {
            let connection = state.checkout_for_tests().expect("raw connection");
            connection
                .execute(
                    "UPDATE kpi_ingest_runs SET lease_holder = 'mcp:full', \
                     lease_expires_at = '2026-01-01T00:00:00.000Z', \
                     last_heartbeat_at = '2026-01-01T00:00:00.000Z', \
                     attempt_count = 1 WHERE id = ?1",
                    [&run.id],
                )
                .expect("seed lease");

            // Just before expiry: the lease is live -> RunTakenOver.
            let error = claim_run_on_connection(
                &connection,
                &run.id,
                "mcp:kpi_acquisition",
                60,
                "2025-12-31T23:59:59.999Z",
            )
            .expect_err("live foreign lease must refuse");
            assert!(
                matches!(error, StorageError::RunTakenOver { ref holder, .. } if holder == "mcp:full"),
                "got {error:?}"
            );

            // AT the expiry instant: `lease_expires_at <= now` claims.
            let claimed = claim_run_on_connection(
                &connection,
                &run.id,
                "mcp:kpi_acquisition",
                60,
                "2026-01-01T00:00:00.000Z",
            )
            .expect("equality is the claim branch");
            assert_eq!(claimed.attempt_count, 2, "exactly one increment");
            assert_eq!(claimed.lease_holder.as_deref(), Some("mcp:kpi_acquisition"));

            // The ousted holder now sees the live takeover.
            let error = claim_run_on_connection(
                &connection,
                &run.id,
                "mcp:full",
                60,
                "2026-01-01T00:00:30.000Z",
            )
            .expect_err("live foreign lease");
            assert!(matches!(
                error,
                StorageError::RunTakenOver { ref holder, .. } if holder == "mcp:kpi_acquisition"
            ));
        }
    }

    #[test]
    fn claim_run_refuses_non_claimable_unknown_and_bad_duration() {
        let (state, doc, company) = setup_one_company_doc();
        let store = state.kpi_ingest_runs();
        {
            let connection = state.checkout_for_tests().expect("raw connection");
            seed_run_raw(
                &connection,
                "run-staged",
                doc,
                company,
                "p1",
                "staged",
                None,
                None,
                None,
            );
            seed_document(&connection, "doc2", company);
            seed_run_raw(
                &connection,
                "run-done",
                "doc2",
                company,
                "p1",
                "complete",
                None,
                None,
                None,
            );
        }
        assert!(matches!(
            store
                .claim_run("run-staged", "mcp:full", 60)
                .expect_err("staged is not claimable"),
            StorageError::RunNotClaimable { ref status, .. } if status == "staged"
        ));
        assert!(matches!(
            store
                .claim_run("run-done", "mcp:full", 60)
                .expect_err("terminal is not claimable"),
            StorageError::RunNotClaimable { .. }
        ));
        assert!(matches!(
            store
                .claim_run("missing", "mcp:full", 60)
                .expect_err("unknown id"),
            StorageError::KpiIngestRunNotFound { .. }
        ));
        assert!(matches!(
            store
                .claim_run("run-staged", "mcp:full", 0)
                .expect_err("bad duration"),
            StorageError::InvalidRunLeaseDuration { seconds: 0 }
        ));
    }

    /// The full three-way refusal classification, exercised through every
    /// guarded step that uses `lease_refusal` (claim_run itself can only ever
    /// demonstrate `RunTakenOver` — absent/expired leases are successful
    /// claims by design).
    #[test]
    fn lease_refusals_classify_three_ways_through_attach_mark_and_begin() {
        let (state, doc, company) = setup_one_company_doc();
        let store = state.kpi_ingest_runs();
        let run = store
            .create_run_if_absent(&new_run(doc, company, "gpw_ifrs_annual@v1"))
            .expect("create");
        let attach_scope = RunContextAttach {
            scope: Some("standalone"),
            ..RunContextAttach::default()
        };

        // (a) Same holder, expired -> RunLeaseExpired.
        store.claim_run(&run.id, "mcp:full", 3600).expect("claim");
        expire_lease_raw(&state, &run.id);
        assert!(matches!(
            store
                .attach_run_context(&run.id, "mcp:full", &attach_scope)
                .expect_err("expired own lease"),
            StorageError::RunLeaseExpired { .. }
        ));
        assert!(matches!(
            store
                .mark_source_captured(&run.id, "mcp:full", "hash-x")
                .expect_err("expired own lease"),
            StorageError::RunLeaseExpired { .. }
        ));

        // (b) Live foreign lease -> RunTakenOver.
        store
            .claim_run(&run.id, "mcp:kpi_acquisition", 3600)
            .expect("takeover after expiry succeeds");
        assert!(matches!(
            store
                .attach_run_context(&run.id, "mcp:full", &attach_scope)
                .expect_err("live foreign lease"),
            StorageError::RunTakenOver { ref holder, .. } if holder == "mcp:kpi_acquisition"
        ));
        assert!(matches!(
            store
                .mark_source_captured(&run.id, "mcp:full", "hash-x")
                .expect_err("live foreign lease"),
            StorageError::RunTakenOver { .. }
        ));

        // (c) Absent lease -> RunLeaseNotHeld.
        store
            .release_lease(&run.id, "mcp:kpi_acquisition")
            .expect("release");
        assert!(matches!(
            store
                .attach_run_context(&run.id, "mcp:full", &attach_scope)
                .expect_err("no lease at all"),
            StorageError::RunLeaseNotHeld { .. }
        ));

        // begin_extracting classifies the same three ways (fresh run driven to
        // source_captured first).
        let connection = state.checkout_for_tests().expect("raw connection");
        seed_document(&connection, "doc-b", company);
        drop(connection);
        let run2 = store
            .create_run_if_absent(&NewKpiIngestRun {
                report_document_id: "doc-b".to_owned(),
                company_id: company.to_owned(),
                period_id: None,
                profile_version: "gpw_ifrs_annual@v1".to_owned(),
                scope: Some("standalone".to_owned()),
                data_quality: Some("final".to_owned()),
                period_fiscal_year: Some(2025),
                period_type: Some("FY".to_owned()),
            })
            .expect("create 2");
        store.claim_run(&run2.id, "mcp:full", 3600).expect("claim");
        store
            .mark_source_captured(&run2.id, "mcp:full", "hash-2")
            .expect("capture");
        expire_lease_raw(&state, &run2.id);
        assert!(matches!(
            store
                .begin_extracting(&run2.id, "mcp:full", "v1")
                .expect_err("expired own lease"),
            StorageError::RunLeaseExpired { .. }
        ));
        store
            .claim_run(&run2.id, "mcp:kpi_acquisition", 3600)
            .expect("takeover");
        assert!(matches!(
            store
                .begin_extracting(&run2.id, "mcp:full", "v1")
                .expect_err("live foreign lease"),
            StorageError::RunTakenOver { .. }
        ));
    }

    #[test]
    fn attach_run_context_fills_set_once_and_reports_the_first_conflict() {
        let (state, doc, company) = setup_one_company_doc();
        let store = state.kpi_ingest_runs();
        let run = store
            .create_run_if_absent(&new_run(doc, company, "gpw_ifrs_annual@v1"))
            .expect("create");
        store.claim_run(&run.id, "mcp:full", 3600).expect("claim");

        // All-None is a pure no-op.
        store
            .attach_run_context(&run.id, "mcp:full", &RunContextAttach::default())
            .expect("no-op");

        // Fill scope; same-value re-attach is idempotent; different conflicts.
        let scope_only = RunContextAttach {
            scope: Some("standalone"),
            ..RunContextAttach::default()
        };
        store
            .attach_run_context(&run.id, "mcp:full", &scope_only)
            .expect("fill scope");
        store
            .attach_run_context(&run.id, "mcp:full", &scope_only)
            .expect("idempotent same value");
        assert!(matches!(
            store
                .attach_run_context(
                    &run.id,
                    "mcp:full",
                    &RunContextAttach {
                        scope: Some("consolidated"),
                        ..RunContextAttach::default()
                    },
                )
                .expect_err("scope conflict"),
            StorageError::RunContextValueConflict { key: "scope", .. }
        ));

        // Fill the rest; a later differing period conflicts.
        store
            .attach_run_context(
                &run.id,
                "mcp:full",
                &RunContextAttach {
                    data_quality: Some("final"),
                    period: Some((2025, "FY")),
                    ..RunContextAttach::default()
                },
            )
            .expect("fill rest");
        assert!(matches!(
            store
                .attach_run_context(
                    &run.id,
                    "mcp:full",
                    &RunContextAttach {
                        period: Some((2024, "FY")),
                        ..RunContextAttach::default()
                    },
                )
                .expect_err("period conflict"),
            StorageError::RunContextValueConflict { key: "period", .. }
        ));
        let stored = store.get_run(&run.id).expect("get").expect("some");
        assert_eq!(stored.scope.as_deref(), Some("standalone"));
        assert_eq!(stored.data_quality.as_deref(), Some("final"));
        assert_eq!(stored.period_fiscal_year, Some(2025));
        assert_eq!(stored.period_type.as_deref(), Some("FY"));

        // Invalid vocabulary refuses before any lease consideration.
        assert!(matches!(
            store
                .attach_run_context(
                    &run.id,
                    "mcp:full",
                    &RunContextAttach {
                        scope: Some("group"),
                        ..RunContextAttach::default()
                    },
                )
                .expect_err("bad scope token"),
            StorageError::InvalidKpiIngestRunValue { key: "scope", .. }
        ));
    }

    #[test]
    fn attach_run_context_cross_checks_an_existing_period_pin() {
        let (state, doc, company) = setup_one_company_doc();
        {
            let connection = state.checkout_for_tests().expect("raw connection");
            connection
                .execute(
                    "INSERT INTO financial_periods (id, company_id, fiscal_year, period_type)
                     VALUES ('finper-fy', ?1, 2025, 'FY')",
                    [company],
                )
                .expect("seed period");
        }
        let store = state.kpi_ingest_runs();
        let mut with_pin = new_run(doc, company, "gpw_ifrs_annual@v1");
        with_pin.period_id = Some("finper-fy".to_owned());
        let run = store.create_run_if_absent(&with_pin).expect("create");
        store.claim_run(&run.id, "mcp:full", 3600).expect("claim");

        // Matching descriptor attaches cleanly.
        store
            .attach_run_context(
                &run.id,
                "mcp:full",
                &RunContextAttach {
                    period: Some((2025, "FY")),
                    ..RunContextAttach::default()
                },
            )
            .expect("matching descriptor");

        // A descriptor contradicting the pin refuses BEFORE any write.
        let connection = state.checkout_for_tests().expect("raw connection");
        connection
            .execute(
                "UPDATE kpi_ingest_runs SET period_fiscal_year = NULL, period_type = NULL \
                 WHERE id = ?1",
                [&run.id],
            )
            .expect("clear descriptor");
        drop(connection);
        assert!(matches!(
            store
                .attach_run_context(
                    &run.id,
                    "mcp:full",
                    &RunContextAttach {
                        period: Some((2024, "FY")),
                        ..RunContextAttach::default()
                    },
                )
                .expect_err("pin mismatch"),
            StorageError::RunContextValueConflict { key: "period", .. }
        ));
    }

    #[test]
    fn list_pending_runs_pages_the_claimable_set_with_a_keyset_cursor() {
        let (state, _doc, company) = setup_one_company_doc();
        let store = state.kpi_ingest_runs();
        {
            let connection = state.checkout_for_tests().expect("raw connection");
            for (index, id) in ["run-a", "run-b", "run-c", "run-d", "run-e"]
                .iter()
                .enumerate()
            {
                // One document per run — the active-triple uniqueness index
                // allows only one live run per (doc, company, profile).
                let doc_id = format!("doc-{id}");
                seed_document(&connection, &doc_id, company);
                seed_run_raw(
                    &connection,
                    id,
                    &doc_id,
                    company,
                    "p1",
                    "discovered",
                    None,
                    None,
                    None,
                );
                connection
                    .execute(
                        "UPDATE kpi_ingest_runs SET created_at = ?2 WHERE id = ?1",
                        params![id, format!("2026-01-0{}T00:00:00.000Z", index + 1)],
                    )
                    .expect("stamp created_at");
            }
            // Terminal and mid-pipeline rows never appear.
            seed_document(&connection, "doc-x", company);
            seed_run_raw(
                &connection,
                "run-f",
                "doc-x",
                company,
                "p1",
                "complete",
                None,
                None,
                None,
            );
            seed_run_raw(
                &connection,
                "run-g",
                "doc-x",
                company,
                "p1",
                "staged",
                None,
                None,
                None,
            );
        }

        let page1 = store.list_pending_runs(None, None, 2).expect("page 1");
        assert_eq!(
            page1.iter().map(|r| r.id.as_str()).collect::<Vec<_>>(),
            ["run-e", "run-d"],
            "newest first, id DESC tie-break"
        );
        let cursor = (page1[1].created_at.as_str(), page1[1].id.as_str());
        let page2 = store
            .list_pending_runs(None, Some(cursor), 2)
            .expect("page 2");
        assert_eq!(
            page2.iter().map(|r| r.id.as_str()).collect::<Vec<_>>(),
            ["run-c", "run-b"]
        );
        let cursor = (page2[1].created_at.as_str(), page2[1].id.as_str());
        let page3 = store
            .list_pending_runs(None, Some(cursor), 2)
            .expect("page 3");
        assert_eq!(
            page3.iter().map(|r| r.id.as_str()).collect::<Vec<_>>(),
            ["run-a"],
            "the tail page; every claimable row seen exactly once"
        );
        let cursor = (page3[0].created_at.as_str(), page3[0].id.as_str());
        assert!(
            store
                .list_pending_runs(None, Some(cursor), 2)
                .expect("page 4")
                .is_empty(),
            "a page ending exactly at the last row yields an empty next page"
        );

        // Company filter: a foreign company's rows are absent.
        let filtered = store
            .list_pending_runs(Some("nope"), None, 10)
            .expect("filtered");
        assert!(filtered.is_empty());
    }

    // -----------------------------------------------------------------------
    // Validation-attempt readers (#385).
    // -----------------------------------------------------------------------

    fn seed_attempt_raw(
        connection: &Connection,
        id: &str,
        run_id: &str,
        revision: i64,
        attempt: i64,
        outcome: &str,
    ) {
        connection
            .execute(
                "INSERT INTO kpi_ingest_validation_attempts
                    (id, run_id, revision, attempt, outcome, manifest_hash, manifest_json)
                 VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7)",
                params![
                    id,
                    run_id,
                    revision,
                    attempt,
                    outcome,
                    format!("hash-{id}"),
                    format!("{{\"runId\":\"{run_id}\",\"revision\":{revision}}}"),
                ],
            )
            .expect("attempt row");
    }

    #[test]
    fn latest_validation_attempt_is_highest_revision_then_attempt_including_failed() {
        let connection = open_in_memory_database().expect("db");
        seed_company(&connection, "c1");
        seed_document(&connection, "d1", "c1");
        seed_run_raw(
            &connection,
            "r1",
            "d1",
            "c1",
            "gpw_ifrs_annual@v1",
            "staged",
            None,
            None,
            None,
        );
        seed_attempt_raw(&connection, "a1", "r1", 1, 1, "ready");
        seed_attempt_raw(&connection, "a2", "r1", 2, 1, "failed");
        seed_attempt_raw(&connection, "a3", "r1", 2, 2, "failed");
        let state = AppState::new(connection);
        let store = state.kpi_ingest_runs();

        let latest = store
            .latest_validation_attempt("r1")
            .expect("read")
            .expect("some");
        assert_eq!(
            latest.id, "a3",
            "revision DESC, attempt DESC — failed counts"
        );
        assert_eq!(latest.outcome, "failed");

        assert!(store
            .latest_validation_attempt("nope")
            .expect("read")
            .is_none());
    }

    #[test]
    fn validation_attempt_by_id_is_scoped_to_the_run() {
        let connection = open_in_memory_database().expect("db");
        seed_company(&connection, "c1");
        seed_document(&connection, "d1", "c1");
        seed_document(&connection, "d2", "c1");
        seed_run_raw(
            &connection,
            "r1",
            "d1",
            "c1",
            "gpw_ifrs_annual@v1",
            "staged",
            None,
            None,
            None,
        );
        seed_run_raw(
            &connection,
            "r2",
            "d2",
            "c1",
            "gpw_ifrs_annual@v1",
            "staged",
            None,
            None,
            None,
        );
        seed_attempt_raw(&connection, "a1", "r1", 1, 1, "ready");
        let state = AppState::new(connection);
        let store = state.kpi_ingest_runs();

        assert_eq!(
            store
                .validation_attempt_by_id("r1", "a1")
                .expect("read")
                .expect("some")
                .id,
            "a1"
        );
        assert!(
            store
                .validation_attempt_by_id("r2", "a1")
                .expect("read")
                .is_none(),
            "another run's attempt id never resolves"
        );
    }

    // -----------------------------------------------------------------------
    // Write-time bounds (#385).
    // -----------------------------------------------------------------------

    #[test]
    fn mark_failed_bounds_last_error_at_2048_bytes_on_a_char_boundary() {
        let (state, doc, company) = setup_one_company_doc();
        let store = state.kpi_ingest_runs();
        let run = store
            .create_run_if_absent(&new_run(doc, company, "gpw_ifrs_annual@v1"))
            .expect("create");

        // 2-byte chars force the boundary walk; 1500 of them = 3000 bytes.
        let long = "ą".repeat(1500);
        store.mark_failed(&run.id, &long).expect("mark failed");

        let after = store.get_run(&run.id).expect("read").expect("run");
        let stored = after.last_error.expect("last_error");
        assert!(
            stored.len() <= 2048,
            "stored {} bytes — the write boundary must bound it",
            stored.len()
        );
        assert!(stored.ends_with('…'), "truncation carries the marker");

        // A short error is stored verbatim (the existing "boom" contract).
        {
            let connection = state.checkout_for_tests().expect("raw connection");
            seed_document(&connection, "d2", company);
        }
        let run2 = store
            .create_run_if_absent(&new_run("d2", company, "gpw_ifrs_annual@v1"))
            .expect("create");
        store.mark_failed(&run2.id, "boom").expect("mark failed");
        assert_eq!(
            store
                .get_run(&run2.id)
                .expect("read")
                .expect("run")
                .last_error
                .as_deref(),
            Some("boom")
        );
    }

    #[test]
    fn create_run_refuses_an_expected_stamp_beyond_256_keys() {
        let connection = open_in_memory_database().expect("db");
        seed_company(&connection, "c1");
        seed_document(&connection, "d1", "c1");
        for i in 0..257 {
            connection
                .execute(
                    "INSERT INTO kpi_definitions (id, scope, metric_key, label, value_kind)
                     VALUES (?1, 'user', ?2, ?3, 'currency')",
                    params![
                        format!("kd{i}"),
                        format!("custom_metric_{i}"),
                        format!("Custom {i}"),
                    ],
                )
                .expect("definition");
            connection
                .execute(
                    "INSERT INTO kpi_relevance (id, company_id, definition_id, source, rank)
                     VALUES (?1, 'c1', ?2, 'manual', 'primary')",
                    params![format!("kr{i}"), format!("kd{i}")],
                )
                .expect("relevance");
        }
        let state = AppState::new(connection);

        let error = state
            .kpi_ingest_runs()
            .create_run_if_absent(&new_run("d1", "c1", "gpw_ifrs_annual@v1"))
            .expect_err("a 257+-key stamp is a data error, not a run");
        assert!(
            matches!(
                error,
                StorageError::InvalidKpiIngestRunValue {
                    key: "expected_kpis",
                    ..
                }
            ),
            "got {error:?}"
        );
    }

    // -----------------------------------------------------------------------
    // Generation-guarded terminalization (#386, E1).
    // -----------------------------------------------------------------------

    #[test]
    fn mark_failed_for_generation_matrix() {
        let connection = open_in_memory_database().expect("db");
        seed_company(&connection, "c1");
        for doc in ["d1", "d2", "d3", "d4"] {
            seed_document(&connection, doc, "c1");
        }
        // Staged run at revision 2.
        seed_run_raw(
            &connection,
            "r1",
            "d1",
            "c1",
            "gpw_ifrs_annual@v1",
            "staged",
            None,
            None,
            None,
        );
        connection
            .execute(
                "UPDATE kpi_ingest_runs SET manifest_revision = 2 WHERE id = 'r1'",
                [],
            )
            .expect("rev");
        // Ready-to-commit run at revision 3 with a frozen hash.
        seed_run_raw(
            &connection,
            "r2",
            "d2",
            "c1",
            "gpw_ifrs_annual@v1",
            "ready_to_commit",
            None,
            None,
            None,
        );
        connection
            .execute(
                "UPDATE kpi_ingest_runs SET manifest_revision = 3, manifest_hash = 'h3' WHERE id = 'r2'",
                [],
            )
            .expect("tuple");
        let state = AppState::new(connection);
        let store = state.kpi_ingest_runs();

        // Revision moved → Ok(false), run untouched.
        assert!(!store
            .mark_failed_for_generation("r1", "boom", &IngestGeneration::Staged { revision: 1 })
            .expect("guarded"));
        assert_eq!(
            store.get_run("r1").expect("get").expect("run").status,
            KpiIngestRunState::Staged
        );
        // Wrong expected state → Ok(false).
        assert!(!store
            .mark_failed_for_generation(
                "r1",
                "boom",
                &IngestGeneration::ReadyToCommit {
                    revision: 2,
                    manifest_hash: "h".into()
                }
            )
            .expect("guarded"));
        // Unknown run → Ok(false).
        assert!(!store
            .mark_failed_for_generation("nope", "boom", &IngestGeneration::Staged { revision: 1 })
            .expect("guarded"));
        // Hash mismatch on ready_to_commit → Ok(false).
        assert!(!store
            .mark_failed_for_generation(
                "r2",
                "boom",
                &IngestGeneration::ReadyToCommit {
                    revision: 3,
                    manifest_hash: "other".into()
                }
            )
            .expect("guarded"));

        // Exact staged generation → failed, lease released, Ok(true).
        assert!(store
            .mark_failed_for_generation("r1", "boom", &IngestGeneration::Staged { revision: 2 })
            .expect("guarded"));
        let failed = store.get_run("r1").expect("get").expect("run");
        assert_eq!(failed.status, KpiIngestRunState::Failed);
        assert_eq!(failed.last_error.as_deref(), Some("boom"));

        // Exact ready generation → failed too.
        assert!(store
            .mark_failed_for_generation(
                "r2",
                "boom",
                &IngestGeneration::ReadyToCommit {
                    revision: 3,
                    manifest_hash: "h3".into()
                }
            )
            .expect("guarded"));
        assert_eq!(
            store.get_run("r2").expect("get").expect("run").status,
            KpiIngestRunState::Failed
        );
    }

    #[test]
    fn mark_failed_for_generation_bounds_long_utf8_errors() {
        let connection = open_in_memory_database().expect("db");
        seed_company(&connection, "c1");
        seed_document(&connection, "d1", "c1");
        seed_run_raw(
            &connection,
            "r1",
            "d1",
            "c1",
            "gpw_ifrs_annual@v1",
            "staged",
            None,
            None,
            None,
        );
        connection
            .execute(
                "UPDATE kpi_ingest_runs SET manifest_revision = 1 WHERE id = 'r1'",
                [],
            )
            .expect("rev");
        let state = AppState::new(connection);
        let store = state.kpi_ingest_runs();

        let long = "ł".repeat(1500); // 3000 bytes of 2-byte chars
        assert!(store
            .mark_failed_for_generation("r1", &long, &IngestGeneration::Staged { revision: 1 })
            .expect("guarded"));
        let stored = store
            .get_run("r1")
            .expect("get")
            .expect("run")
            .last_error
            .expect("last_error");
        assert!(stored.len() <= 2048, "bounded: {} bytes", stored.len());
        assert!(stored.ends_with('…'), "marker present");
    }
}
