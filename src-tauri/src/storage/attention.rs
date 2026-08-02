//! Alert rules + attention events (ADR 0068, plan v0.54-attention-routing §T2).
//!
//! A user-owned [`AlertRule`] pairs a trigger type with a scope (a single
//! company or a whole watchlist). Rule evaluation runs INLINE in the job stages
//! that produce the evidence — signal classification (`storage::signals`),
//! autopilot `finalize_notify`, and the post-daily-pull price check — never on a
//! new worker lane (the evaluation is a handful of indexed reads; plan §T2). Each
//! fired rule persists an [`AttentionEvent`] linked to its evidence.
//!
//! Dedup + throttle + freshness:
//! - **Dedup**: at most one event per `(rule, evidence_type, evidence_ref)`.
//!   Re-running evaluation over the same evidence is a no-op (mirrors
//!   `classify_and_store_signal`), so no re-fire on re-ingest.
//! - **Daily throttle**: at most [`DAILY_THROTTLE_PER_RULE`] event(s) per rule
//!   per **wall-clock** day, keyed on `fired_at` (the actual firing time). A rule
//!   pings at most once a day however many distinct pieces of evidence it matches
//!   that day, keeping the attention surface quiet. `fired_at` is the wall-clock
//!   firing time — NOT the evidence's domain date (ADR 0068 amendment): keying
//!   the throttle on the domain date let a history backfill re-ingesting years of
//!   filings raise one event per historical date (each a "different day"), which
//!   is the toast-wall regression this repair closes.
//! - **Freshness gate** ([`SIGNAL_FRESHNESS_DAYS`] via [`signal_is_stale`]): the
//!   historical-ingest seam (`classify_and_store_signal`) does not evaluate alert
//!   rules for evidence whose DOMAIN date is older than the window relative to
//!   wall-clock now — "historical ingest never impersonates the present". A
//!   backfill of old filings is silent; only genuinely new signals alert. Applied
//!   at the ingest seam (not inside `evaluate_signal_rules`) so present-detection
//!   callers — derived red flags, KNF short-position changes — still alert on a
//!   current condition even when its underlying period is old.

use serde::{Deserialize, Serialize};

use super::database::Database;
use super::*;

/// Max attention events a single rule may fire on one domain day. One keeps the
/// attention surface to at most one ping per rule per day; additional matching
/// evidence that day is suppressed (the underlying signals/quotes stay in their
/// own feeds). Chosen per plan §T2 ("1/day"); a future per-trigger override can
/// widen this without a schema change.
pub const DAILY_THROTTLE_PER_RULE: i64 = 1;

/// Freshness window for signal-rule evaluation (ADR 0068 amendment). A confirmed
/// signal whose DOMAIN date (publication/signal date) is older than this many days
/// relative to wall-clock now never fires an attention event: a history backfill
/// re-ingesting years of filings must not raise a wall of alerts for evidence that
/// is not actually new. A signal with no/unparseable domain date is treated as
/// fresh — we cannot prove it is old, and suppressing an undated fresh signal
/// would be the worse failure.
pub const SIGNAL_FRESHNESS_DAYS: i64 = 14;

pub const TRIGGER_SIGNAL_CATEGORY: &str = "signal_category";
pub const TRIGGER_AUTOPILOT_RUN_COMPLETED: &str = "autopilot_run_completed";
pub const TRIGGER_PRICE_ENTERS_RANGE: &str = "price_enters_range";
pub const TRIGGER_PRICE_WEEK52_LOW: &str = "price_week52_low";
/// System trigger (no user rule) for a reconciliation `espi_only` result — the
/// primary channel missed an official report the witness saw (ADR 0069 D2
/// amendment, plan v0.55 T3). Not user-creatable, so it is deliberately absent
/// from [`TRIGGER_TYPES`] (which validates user-owned alert rules).
pub const TRIGGER_SOURCE_RECONCILIATION: &str = "source_reconciliation";
/// System trigger (no user rule) for a background job that failed TERMINALLY —
/// its retries are exhausted and it will not run again (ADR 0091 dec. 1, epic #40
/// S3). One generic, always-on failure surface for the job kinds that have no
/// richer domain surface of their own (`jobs::failure_surface`); transient
/// hiccups (a retry still pending) never fire. Not user-creatable, so it is
/// deliberately absent from [`TRIGGER_TYPES`].
pub const TRIGGER_JOB_FAILED: &str = "job_failed";

/// The user-creatable trigger types (validates user-owned alert rules). The
/// system [`TRIGGER_SOURCE_RECONCILIATION`] is deliberately absent (not
/// user-creatable). `pub(crate)` so the severity classification gate
/// (`storage::severity`) derives its inventory from this source of truth rather
/// than a hand copy.
pub(crate) const TRIGGER_TYPES: &[&str] = &[
    TRIGGER_SIGNAL_CATEGORY,
    TRIGGER_AUTOPILOT_RUN_COMPLETED,
    TRIGGER_PRICE_ENTERS_RANGE,
    TRIGGER_PRICE_WEEK52_LOW,
];

const SCOPE_COMPANY: &str = "company";
const SCOPE_WATCHLIST: &str = "watchlist";
const SCOPE_TYPES: &[&str] = &[SCOPE_COMPANY, SCOPE_WATCHLIST];

/// Evidence-type tags recorded on an [`AttentionEvent`].
pub const EVIDENCE_COMPANY_SIGNAL: &str = "company_signal";
pub const EVIDENCE_AUTOPILOT_RUN: &str = "autopilot_run";
pub const EVIDENCE_DAILY_QUOTE: &str = "daily_quote";
/// Evidence tag for a reconciliation `espi_only` event — `evidence_ref` is the
/// `source_reconciliation_results.id`, so Today click-through / the diagnostics
/// ledger resolve the missed report (witness title + GPW URL).
pub const EVIDENCE_SOURCE_RECONCILIATION: &str = "source_reconciliation";
/// Evidence tag for a terminally failed background job — `evidence_ref` is the
/// `job_queue.id`, so the read model resolves the job's `kind` and `last_error`
/// (ADR 0091 dec. 1). The job row is the durable record of WHAT failed.
pub const EVIDENCE_JOB: &str = "job";

/// 52 weeks expressed in days — the trailing window for the `price_week52_low`
/// trigger. Bars are scanned by domain `date`, never `fetched_at`/`created_at`.
const WEEK52_DAYS: i64 = 52 * 7;

/// A user-owned alert rule (persisted `alert_rules` row).
#[derive(Debug, Clone, PartialEq, Serialize)]
#[cfg_attr(feature = "ts-export", derive(ts_rs::TS))]
#[cfg_attr(
    feature = "ts-export",
    ts(export, export_to = "../../src/api/generated/")
)]
#[serde(rename_all = "camelCase")]
pub struct AlertRule {
    pub id: String,
    #[cfg_attr(
        feature = "ts-export",
        ts(
            type = "\"signal_category\" | \"autopilot_run_completed\" | \"price_enters_range\" | \"price_week52_low\""
        )
    )]
    pub trigger_type: String,
    pub signal_category: Option<String>,
    pub price_min: Option<f64>,
    pub price_max: Option<f64>,
    #[cfg_attr(feature = "ts-export", ts(type = "\"company\" | \"watchlist\""))]
    pub scope_type: String,
    pub scope_ref: String,
    pub enabled: bool,
    pub created_at: String,
    pub updated_at: String,
}

/// New-rule input (enabled defaults to true).
#[derive(Debug, Clone, Deserialize, schemars::JsonSchema)]
#[cfg_attr(feature = "ts-export", derive(ts_rs::TS))]
#[cfg_attr(
    feature = "ts-export",
    ts(export, export_to = "../../src/api/generated/")
)]
#[serde(rename_all = "camelCase")]
pub struct NewAlertRule {
    #[cfg_attr(
        feature = "ts-export",
        ts(
            type = "\"signal_category\" | \"autopilot_run_completed\" | \"price_enters_range\" | \"price_week52_low\""
        )
    )]
    pub trigger_type: String,
    #[serde(default)]
    pub signal_category: Option<String>,
    #[serde(default)]
    pub price_min: Option<f64>,
    #[serde(default)]
    pub price_max: Option<f64>,
    #[cfg_attr(feature = "ts-export", ts(type = "\"company\" | \"watchlist\""))]
    pub scope_type: String,
    pub scope_ref: String,
}

/// Mutable fields of an existing rule. `None` (absent) leaves a field unchanged.
#[derive(Debug, Clone, Default, Deserialize, schemars::JsonSchema)]
#[cfg_attr(feature = "ts-export", derive(ts_rs::TS))]
#[cfg_attr(
    feature = "ts-export",
    ts(export, export_to = "../../src/api/generated/", optional_fields)
)]
#[serde(rename_all = "camelCase")]
pub struct AlertRuleUpdate {
    pub id: String,
    #[serde(default)]
    pub enabled: Option<bool>,
    #[serde(default)]
    pub signal_category: Option<String>,
    #[serde(default)]
    pub price_min: Option<f64>,
    #[serde(default)]
    pub price_max: Option<f64>,
    #[serde(default)]
    #[cfg_attr(
        feature = "ts-export",
        ts(optional, type = "\"company\" | \"watchlist\"")
    )]
    pub scope_type: Option<String>,
    #[serde(default)]
    pub scope_ref: Option<String>,
}

/// A fired attention event (persisted `attention_events` row), joined to its
/// rule's trigger type for the surfaces.
#[derive(Debug, Clone, PartialEq, Serialize)]
#[cfg_attr(feature = "ts-export", derive(ts_rs::TS))]
#[cfg_attr(
    feature = "ts-export",
    ts(export, export_to = "../../src/api/generated/")
)]
#[serde(rename_all = "camelCase")]
pub struct AttentionEvent {
    pub id: String,
    /// The owning alert rule, or `None` for a SYSTEM event (e.g. a reconciliation
    /// `source_reconciliation` event, raised without a user rule — ADR 0069 D2).
    #[cfg_attr(feature = "ts-export", ts(type = "string | null"))]
    pub rule_id: Option<String>,
    #[cfg_attr(
        feature = "ts-export",
        ts(
            type = "\"signal_category\" | \"autopilot_run_completed\" | \"price_enters_range\" | \"price_week52_low\" | \"source_reconciliation\" | \"job_failed\""
        )
    )]
    pub trigger_type: String,
    /// The company this event is about, or `None` for a SYSTEM event with no
    /// company scope — a workspace-wide background job (morning briefing, history
    /// sweep, aggregator pull) that failed terminally (ADR 0091 dec. 1/2, nullable
    /// since migration 0118). Company-scoped events always carry their company.
    #[cfg_attr(feature = "ts-export", ts(type = "string | null"))]
    pub company_id: Option<String>,
    #[cfg_attr(
        feature = "ts-export",
        ts(
            type = "\"company_signal\" | \"autopilot_run\" | \"daily_quote\" | \"source_reconciliation\" | \"job\""
        )
    )]
    pub evidence_type: String,
    pub evidence_ref: String,
    pub fired_at: String,
    pub seen: bool,
    pub dismissed: bool,
    /// Typed importance (ADR 0087 dec. 2), **computed at read** from
    /// `trigger_type` + the signal's category (for `signal_category` events) by
    /// the single backend mapping [`super::severity`] — never stored. The
    /// frontend routes on this value and never re-infers importance from strings.
    pub severity: AttentionSeverity,
    /// The specific title of the event's evidence, resolved by LEFT JOIN at read
    /// (ADR 0087 dec. 4 — a raw source datum, never composed prose): the filing
    /// title (`company_signal` → `feed_items.title`), the missed report's witness
    /// title (`source_reconciliation` → `witness_title`), the processed report's
    /// document title (`autopilot_run` → `report_documents.title`), or a failed
    /// job's subject — its fire-time snapshot, else the job's own `last_error`
    /// (`job` → `job_queue.last_error`). `None` for a legacy row or pruned evidence — the frontend falls back to
    /// generic copy. So a stream row can state WHAT concretely happened, never a
    /// bare category (v0.60 D6, owner dogfooding 2026-07-23).
    pub evidence_title: Option<String>,
    /// A secondary raw datum whose meaning depends on `evidence_type`: for a
    /// `source_reconciliation` event, the display name of the source that missed
    /// the report (adapter id → its registry display name); for an `autopilot_run`
    /// event, the run's raw status; for a `job` event, the failed job's raw `kind`
    /// (all translated by the frontend). `None` otherwise or when the evidence row
    /// is gone.
    pub evidence_detail: Option<String>,
}

/// Filter for listing attention events.
#[derive(Debug, Clone, Default, Deserialize)]
#[cfg_attr(feature = "ts-export", derive(ts_rs::TS))]
#[cfg_attr(
    feature = "ts-export",
    ts(
        export,
        export_to = "../../src/api/generated/",
        rename = "AttentionEventListInput"
    )
)]
#[serde(rename_all = "camelCase")]
pub struct AttentionEventListInput {
    #[serde(default)]
    pub company_id: Option<String>,
    /// Include dismissed events (default: only non-dismissed).
    #[serde(default)]
    pub include_dismissed: bool,
}

// ---------------------------------------------------------------------------
// Rule CRUD
// ---------------------------------------------------------------------------

fn validate_new_rule(input: &NewAlertRule) -> StorageResult<()> {
    if !TRIGGER_TYPES.contains(&input.trigger_type.as_str()) {
        return Err(StorageError::InvalidAlertRuleValue {
            key: "trigger_type",
            value: input.trigger_type.clone(),
        });
    }
    if !SCOPE_TYPES.contains(&input.scope_type.as_str()) {
        return Err(StorageError::InvalidAlertRuleValue {
            key: "scope_type",
            value: input.scope_type.clone(),
        });
    }
    if input.scope_ref.trim().is_empty() {
        return Err(StorageError::InvalidAlertRuleValue {
            key: "scope_ref",
            value: input.scope_ref.clone(),
        });
    }
    if input.trigger_type == TRIGGER_SIGNAL_CATEGORY
        && input
            .signal_category
            .as_deref()
            .map(str::trim)
            .filter(|value| !value.is_empty())
            .is_none()
    {
        return Err(StorageError::InvalidAlertRuleValue {
            key: "signal_category",
            value: String::new(),
        });
    }
    if input.trigger_type == TRIGGER_PRICE_ENTERS_RANGE {
        match (input.price_min, input.price_max) {
            (Some(min), Some(max)) if min <= max => {}
            _ => {
                return Err(StorageError::InvalidAlertRuleValue {
                    key: "price_range",
                    value: format!("{:?}..{:?}", input.price_min, input.price_max),
                });
            }
        }
    }
    Ok(())
}

// Content-derived id (the repo's collision-safe id convention — see
// notebook_entry_id): the base id encodes the rule's meaning, so an EXACT
// duplicate (same trigger+scope+prices) is detected and rejected as a typed
// error instead of inserting a meaningless twin. A base-id collision whose
// content differs (a survivor edited its prices after creation) gets a count
// suffix. Never COUNT(*)-of-table — deleting a rule made the next create
// reuse a survivor's id (live crash, 2026-07-15).
fn alert_rule_id(connection: &Connection, input: &NewAlertRule) -> StorageResult<String> {
    let mut base = format!(
        "alert_{}_{}_{}",
        super::slug_part(&input.trigger_type),
        super::slug_part(&input.scope_type),
        super::slug_part(&input.scope_ref)
    );
    if let Some(category) = &input.signal_category {
        base.push('_');
        base.push_str(&super::slug_part(category));
    }
    if let (Some(min), Some(max)) = (input.price_min, input.price_max) {
        base.push_str(&format!("_{}_{}", min.round() as i64, max.round() as i64));
    }

    let identical: Option<String> = connection
        .query_row(
            "SELECT id FROM alert_rules
             WHERE trigger_type = ?1 AND scope_type = ?2 AND scope_ref = ?3
               AND signal_category IS ?4 AND price_min IS ?5 AND price_max IS ?6",
            params![
                input.trigger_type,
                input.scope_type,
                input.scope_ref,
                input.signal_category,
                input.price_min,
                input.price_max,
            ],
            |row| row.get(0),
        )
        .optional()?;
    if let Some(id) = identical {
        return Err(StorageError::DuplicateAlertRule { id });
    }

    let taken: i64 = connection.query_row(
        "SELECT COUNT(*) FROM alert_rules WHERE id = ?1 OR id LIKE ?2",
        params![&base, format!("{base}_%")],
        |row| row.get(0),
    )?;
    Ok(if taken == 0 {
        base
    } else {
        format!("{base}_{}", taken + 1)
    })
}

pub(super) fn create_alert_rule(
    connection: &Connection,
    input: NewAlertRule,
) -> StorageResult<AlertRule> {
    validate_new_rule(&input)?;
    let id = alert_rule_id(connection, &input)?;
    connection.execute(
        "
        INSERT INTO alert_rules (
            id, trigger_type, signal_category, price_min, price_max, scope_type, scope_ref
        ) VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7)
        ",
        params![
            id,
            input.trigger_type,
            input.signal_category,
            input.price_min,
            input.price_max,
            input.scope_type,
            input.scope_ref,
        ],
    )?;
    get_alert_rule(connection, &id)
}

pub(super) fn get_alert_rule(connection: &Connection, id: &str) -> StorageResult<AlertRule> {
    connection
        .query_row(
            "
            SELECT id, trigger_type, signal_category, price_min, price_max,
                   scope_type, scope_ref, enabled, created_at, updated_at
            FROM alert_rules WHERE id = ?1
            ",
            [id],
            alert_rule_from_row,
        )
        .optional()?
        .ok_or_else(|| StorageError::AlertRuleNotFound { id: id.to_owned() })
}

pub(super) fn list_alert_rules(connection: &Connection) -> StorageResult<Vec<AlertRule>> {
    let mut statement = connection.prepare(
        "
        SELECT id, trigger_type, signal_category, price_min, price_max,
               scope_type, scope_ref, enabled, created_at, updated_at
        FROM alert_rules
        ORDER BY created_at, id
        ",
    )?;
    let rows = statement.query_map([], alert_rule_from_row)?;
    rows.collect::<Result<Vec<_>, _>>()
        .map_err(StorageError::from)
}

pub(super) fn update_alert_rule(
    connection: &Connection,
    input: AlertRuleUpdate,
) -> StorageResult<AlertRule> {
    // Read-modify-write so the trigger-type invariants (price range validity,
    // signal category present) are re-checked against the resulting rule.
    let current = get_alert_rule(connection, &input.id)?;
    let merged = NewAlertRule {
        trigger_type: current.trigger_type.clone(),
        signal_category: input
            .signal_category
            .clone()
            .or_else(|| current.signal_category.clone()),
        price_min: input.price_min.or(current.price_min),
        price_max: input.price_max.or(current.price_max),
        scope_type: input
            .scope_type
            .clone()
            .unwrap_or_else(|| current.scope_type.clone()),
        scope_ref: input
            .scope_ref
            .clone()
            .unwrap_or_else(|| current.scope_ref.clone()),
    };
    validate_new_rule(&merged)?;

    let enabled = input.enabled.unwrap_or(current.enabled);
    connection.execute(
        "
        UPDATE alert_rules
        SET signal_category = ?2,
            price_min = ?3,
            price_max = ?4,
            scope_type = ?5,
            scope_ref = ?6,
            enabled = ?7,
            updated_at = strftime('%Y-%m-%dT%H:%M:%fZ', 'now')
        WHERE id = ?1
        ",
        params![
            input.id,
            merged.signal_category,
            merged.price_min,
            merged.price_max,
            merged.scope_type,
            merged.scope_ref,
            enabled as i64,
        ],
    )?;
    get_alert_rule(connection, &input.id)
}

pub(super) fn set_alert_rule_enabled(
    connection: &Connection,
    id: &str,
    enabled: bool,
) -> StorageResult<AlertRule> {
    let affected = connection.execute(
        "
        UPDATE alert_rules
        SET enabled = ?2, updated_at = strftime('%Y-%m-%dT%H:%M:%fZ', 'now')
        WHERE id = ?1
        ",
        params![id, enabled as i64],
    )?;
    if affected == 0 {
        return Err(StorageError::AlertRuleNotFound { id: id.to_owned() });
    }
    get_alert_rule(connection, id)
}

pub(super) fn delete_alert_rule(connection: &Connection, id: &str) -> StorageResult<()> {
    connection.execute("DELETE FROM alert_rules WHERE id = ?1", [id])?;
    Ok(())
}

fn alert_rule_from_row(row: &rusqlite::Row<'_>) -> rusqlite::Result<AlertRule> {
    Ok(AlertRule {
        id: row.get(0)?,
        trigger_type: row.get(1)?,
        signal_category: row.get(2)?,
        price_min: row.get(3)?,
        price_max: row.get(4)?,
        scope_type: row.get(5)?,
        scope_ref: row.get(6)?,
        enabled: row.get::<_, i64>(7)? != 0,
        created_at: row.get(8)?,
        updated_at: row.get(9)?,
    })
}

// ---------------------------------------------------------------------------
// Attention events
// ---------------------------------------------------------------------------

pub(super) fn list_attention_events(
    connection: &Connection,
    input: AttentionEventListInput,
) -> StorageResult<Vec<AttentionEvent>> {
    let mut statement = connection.prepare(
        "
        SELECT
            attention_events.id,
            attention_events.rule_id,
            COALESCE(attention_events.trigger_type, alert_rules.trigger_type) AS trigger_type,
            attention_events.company_id,
            attention_events.evidence_type,
            attention_events.evidence_ref,
            attention_events.fired_at,
            attention_events.seen,
            attention_events.dismissed,
            company_signals.category AS signal_category,
            -- Evidence specifics (v0.60 D6): the concrete title behind each event,
            -- resolved by evidence-type-guarded LEFT JOINs so every row can state
            -- WHAT happened, not just its category. Single query, no N+1.
            feed_items.title AS signal_title,
            -- The missed report's title. The registry parser fills
            -- `witness_title` again since issue #191 (its selectors had never
            -- matched GPW's live markup), so fresh CURRENT reports carry one —
            -- but the fallback stays, for two standing cases: GPW publishes no
            -- title at all for PERIODIC reports (empty `<p>`, nothing to parse),
            -- and rows reconciled before the fix keep their empty title unless
            -- the report is still inside GPW's 15-row listing window. Fall back
            -- to `report_type`+`report_number` trimmed and concatenated (two raw
            -- registry strings, no invented words; both absent → NULL → generic
            -- FE copy). Owner ledger 2026-07-23.
            NULLIF(
                COALESCE(
                    NULLIF(recon.witness_title, ''),
                    NULLIF(TRIM(
                        COALESCE(recon.report_type, '') || ' ' ||
                        COALESCE(recon.report_number, '')
                    ), '')
                ), ''
            ) AS recon_title,
            recon.witness_adapter_id AS recon_adapter,
            report_documents.title AS run_document_title,
            autopilot_run.status AS run_status,
            -- Durable fire-time title snapshot (v0.60 D7): preferred over the live
            -- join so a `company_signal` title survives the feed prune that
            -- cascade-deletes its signal row. NULL on legacy rows → live join.
            attention_events.evidence_title AS snapshot_title,
            -- The terminally failed job behind a `job_failed` event (ADR 0091
            -- dec. 1): its kind (the enum token the frontend translates) and the
            -- queue's own last_error, so the row states WHICH job failed and HOW.
            failed_job.kind AS job_kind,
            failed_job.last_error AS job_last_error
        FROM attention_events
        LEFT JOIN alert_rules ON alert_rules.id = attention_events.rule_id
        -- Resolve the signal category for `signal_category` events (evidence_ref
        -- is the signal id) so severity is computed at read; the evidence_type
        -- guard keeps quote/run refs from spuriously joining.
        LEFT JOIN company_signals
            ON company_signals.id = attention_events.evidence_ref
            AND attention_events.evidence_type = 'company_signal'
        -- The signal's own filing title (its statement on the surface).
        LEFT JOIN feed_items ON feed_items.id = company_signals.feed_item_id
        -- The missed report a `source_reconciliation` event points at (evidence_ref
        -- is the reconciliation-result id): its witness title + witness source.
        LEFT JOIN source_reconciliation_results recon
            ON recon.id = attention_events.evidence_ref
            AND attention_events.evidence_type = 'source_reconciliation'
        -- The run behind an `autopilot_run` event, and that run's report document,
        -- so the event states which report was processed and how the run ended.
        LEFT JOIN autopilot_run
            ON autopilot_run.id = attention_events.evidence_ref
            AND attention_events.evidence_type = 'autopilot_run'
        LEFT JOIN report_documents ON report_documents.id = autopilot_run.report_document_id
        -- The job row behind a `job_failed` event (evidence_ref is the job id); the
        -- evidence_type guard keeps other refs from spuriously joining.
        LEFT JOIN job_queue failed_job
            ON failed_job.id = attention_events.evidence_ref
            AND attention_events.evidence_type = 'job'
        WHERE (?1 IS NULL OR attention_events.company_id = ?1)
          AND (?2 = 1 OR attention_events.dismissed = 0)
        ORDER BY attention_events.fired_at DESC, attention_events.id DESC
        ",
    )?;
    // Read-time now: the aging demotion (ADR 0087 dec. 2 amendment) is computed
    // against wall-clock now, once per list, so a stale `urgent` stops leading.
    let now = time::OffsetDateTime::now_utc();
    let rows = statement.query_map(
        params![input.company_id, input.include_dismissed as i64],
        |row| attention_event_from_row(row, now),
    )?;
    rows.collect::<Result<Vec<_>, _>>()
        .map_err(StorageError::from)
}

pub(super) fn mark_attention_event_seen(connection: &Connection, id: &str) -> StorageResult<()> {
    connection.execute("UPDATE attention_events SET seen = 1 WHERE id = ?1", [id])?;
    Ok(())
}

pub(super) fn dismiss_attention_event(connection: &Connection, id: &str) -> StorageResult<()> {
    connection.execute(
        "UPDATE attention_events SET dismissed = 1, seen = 1 WHERE id = ?1",
        [id],
    )?;
    Ok(())
}

fn attention_event_from_row(
    row: &rusqlite::Row<'_>,
    now: time::OffsetDateTime,
) -> rusqlite::Result<AttentionEvent> {
    let trigger_type: String = row.get(2)?;
    let evidence_type: String = row.get(4)?;
    let signal_category: Option<String> = row.get(9)?;
    let fired_at: String = row.get(6)?;
    // Base mapping, then the age demotion (ADR 0087 dec. 2 amendment): an `urgent`
    // event unacted past the threshold stops shouting. Purely age-based.
    let base =
        super::severity::severity_for_attention_event(&trigger_type, signal_category.as_deref());
    let severity = super::severity::aged_attention_severity(base, &fired_at, now);
    // Compose the concrete evidence specifics from the joined columns (v0.60 D6),
    // dispatching on the evidence type so each event states WHAT happened. Titles
    // are raw source data; the frontend translates/composes any prose (ADR 0087
    // dec. 4). An absent join column stays `None` → generic frontend fallback.
    // The durable fire-time snapshot (v0.60 D7) is preferred over the live join for
    // every evidence type: it is the event's own journal of WHAT happened and
    // outlives evidence pruning. An empty/NULL snapshot (legacy rows, or a fire
    // site with no title in scope) falls back to the read-time join.
    let snapshot_title: Option<String> = row
        .get::<_, Option<String>>(15)?
        .map(|title| title.trim().to_owned())
        .filter(|title| !title.is_empty());
    let (evidence_title, evidence_detail) = match evidence_type.as_str() {
        EVIDENCE_COMPANY_SIGNAL => (snapshot_title.or(row.get::<_, Option<String>>(10)?), None),
        EVIDENCE_SOURCE_RECONCILIATION => {
            let title: Option<String> = snapshot_title.or(row.get(11)?);
            let adapter: Option<String> = row.get(12)?;
            (title, adapter.map(|id| adapter_display_name(&id)))
        }
        EVIDENCE_AUTOPILOT_RUN => (
            snapshot_title.or(row.get::<_, Option<String>>(13)?),
            row.get(14)?,
        ),
        // A terminally failed background job (ADR 0091 dec. 1). The statement is
        // the fire-time SUBJECT snapshot when the handler had one (a document
        // title, a ticker — `JobHandler::failure_subject`), otherwise the queue's
        // own `last_error`: both are raw source data, so the row always states
        // something concrete instead of "a job failed". The detail carries the raw
        // job `kind` for the frontend to translate (the autopilot-status pattern).
        EVIDENCE_JOB => (
            snapshot_title.or(row.get::<_, Option<String>>(17)?),
            row.get(16)?,
        ),
        _ => (None, None),
    };
    Ok(AttentionEvent {
        id: row.get(0)?,
        rule_id: row.get::<_, Option<String>>(1)?,
        trigger_type,
        company_id: row.get(3)?,
        evidence_type,
        evidence_ref: row.get(5)?,
        fired_at,
        seen: row.get::<_, i64>(7)? != 0,
        dismissed: row.get::<_, i64>(8)? != 0,
        severity,
        evidence_title,
        evidence_detail,
    })
}

/// A source adapter's user-facing display name from its id, via the source-adapter
/// registry (the single source of truth). Falls back to the raw id for an
/// unregistered/legacy adapter — a source proper noun, never silently blanked.
fn adapter_display_name(adapter_id: &str) -> String {
    crate::source_adapters::registry::descriptor(adapter_id)
        .map(|descriptor| descriptor.display_name.to_owned())
        .unwrap_or_else(|| adapter_id.to_owned())
}

/// Persist one attention event, applying dedup + the per-rule wall-clock daily
/// throttle, stamping the row with its `trigger_type`. Returns `true` only when a
/// new event row was created.
///
/// - Dedup: if `(rule_id, evidence_type, evidence_ref)` already fired, this is a
///   no-op and returns `false` (never counts against the throttle).
/// - Throttle: otherwise, if the rule already fired [`DAILY_THROTTLE_PER_RULE`]
///   event(s) on the current **wall-clock** day, the new evidence is suppressed
///   (returns `false`) — so however many distinct pieces of evidence a rule
///   matches in one ingestion pass, it pings at most once that day.
///
/// `fired_at` is the wall-clock firing time (not the evidence's domain date): the
/// evidence's own date lives on its linked signal/quote/run. `trigger_type` is
/// stamped directly on the row (ADR 0068 / W4) so grouping does not depend on a
/// join back to the rule.
pub(super) fn insert_attention_event(
    connection: &Connection,
    rule_id: &str,
    trigger_type: &str,
    company_id: &str,
    evidence_type: &str,
    evidence_ref: &str,
    // The event's concrete "what happened" title, snapshotted at fire time so it
    // survives evidence pruning (v0.60 D7). `company_signal` evidence is fatal to
    // feed pruning — the signal row is `ON DELETE CASCADE` off `feed_items` — so
    // the durable snapshot is the only thing that keeps such a row from degrading
    // to a bare category. `None` when no title is in scope; the read model still
    // falls back to the live join for evidence rows that outlive pruning.
    evidence_title: Option<&str>,
) -> StorageResult<bool> {
    let evidence_title = evidence_title
        .map(str::trim)
        .filter(|title| !title.is_empty());
    let already: bool = connection.query_row(
        "SELECT EXISTS(
            SELECT 1 FROM attention_events
            WHERE rule_id = ?1 AND evidence_type = ?2 AND evidence_ref = ?3
        )",
        params![rule_id, evidence_type, evidence_ref],
        |row| row.get(0),
    )?;
    if already {
        return Ok(false);
    }

    let today = today_iso();
    let fired_today: i64 = connection.query_row(
        "SELECT COUNT(*) FROM attention_events
         WHERE rule_id = ?1 AND substr(fired_at, 1, 10) = ?2",
        params![rule_id, today],
        |row| row.get(0),
    )?;
    if fired_today >= DAILY_THROTTLE_PER_RULE {
        return Ok(false);
    }

    let id = format!(
        "attn_{}_{}_{}",
        slug_part(rule_id),
        slug_part(evidence_type),
        slug_part(evidence_ref)
    );
    let fired_at = now_rfc3339();
    let affected = connection.execute(
        "
        INSERT INTO attention_events (id, rule_id, trigger_type, company_id, evidence_type, evidence_ref, fired_at, evidence_title)
        VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8)
        ON CONFLICT(rule_id, evidence_type, evidence_ref) DO NOTHING
        ",
        params![id, rule_id, trigger_type, company_id, evidence_type, evidence_ref, fired_at, evidence_title],
    )?;
    Ok(affected > 0)
}

/// Persist one SYSTEM attention event (no owning rule) — e.g. a reconciliation
/// `source_reconciliation` event (ADR 0069 D2 amendment, plan v0.55 T3). Dedup is
/// by the partial unique index on `(trigger_type, evidence_type, evidence_ref)`
/// WHERE `rule_id IS NULL`: the reconciliation-record id (a stable `evidence_ref`)
/// yields at most one event per record, so a re-run never re-fires. No per-rule
/// daily throttle applies (there is no rule); each distinct missed report is its
/// own event. Returns `true` only when a new row was created. Intended to run in
/// the caller's transaction so the event lands atomically with its evidence row.
pub(super) fn insert_system_attention_event(
    connection: &Connection,
    trigger_type: &str,
    // `None` for a system event with no company scope (ADR 0091 dec. 2, migration
    // 0118): a workspace-wide background job failure belongs to no single issuer.
    company_id: Option<&str>,
    evidence_type: &str,
    evidence_ref: &str,
    // Fire-time title snapshot (v0.60 D7) — for reconciliation the missed report's
    // witness title is trivially in scope at the insert site. See
    // `insert_attention_event` for the durability rationale.
    evidence_title: Option<&str>,
    // Full RFC-3339 firing instant. Evidence-dated events (reconciliation) pass
    // their disclosure day at midnight; a job failure passes wall-clock now, so the
    // stream orders it against the rest of the day (ADR 0068: `fired_at` is when
    // the event fired).
    fired_at: &str,
) -> StorageResult<bool> {
    let evidence_title = evidence_title
        .map(str::trim)
        .filter(|title| !title.is_empty());
    let id = format!(
        "attn_sys_{}_{}_{}",
        slug_part(trigger_type),
        slug_part(evidence_type),
        slug_part(evidence_ref)
    );
    let affected = connection.execute(
        "
        INSERT OR IGNORE INTO attention_events
            (id, rule_id, trigger_type, company_id, evidence_type, evidence_ref, fired_at, evidence_title)
        VALUES (?1, NULL, ?2, ?3, ?4, ?5, ?6, ?7)
        ",
        params![
            id,
            trigger_type,
            company_id,
            evidence_type,
            evidence_ref,
            fired_at,
            evidence_title
        ],
    )?;
    Ok(affected > 0)
}

// ---------------------------------------------------------------------------
// Scope resolution
// ---------------------------------------------------------------------------

/// Does `rule` apply to `company_id`? A `company`-scoped rule matches its exact
/// company; a `watchlist`-scoped rule matches every company in that watchlist.
fn rule_matches_company(
    connection: &Connection,
    rule: &AlertRule,
    company_id: &str,
) -> StorageResult<bool> {
    match rule.scope_type.as_str() {
        SCOPE_COMPANY => Ok(rule.scope_ref == company_id),
        SCOPE_WATCHLIST => {
            watchlists::company_is_in_watchlist(connection, &rule.scope_ref, company_id)
        }
        _ => Ok(false),
    }
}

/// Enabled rules of one trigger type that apply to `company_id`.
fn enabled_rules_for_company(
    connection: &Connection,
    trigger_type: &str,
    company_id: &str,
) -> StorageResult<Vec<AlertRule>> {
    let mut statement = connection.prepare(
        "
        SELECT id, trigger_type, signal_category, price_min, price_max,
               scope_type, scope_ref, enabled, created_at, updated_at
        FROM alert_rules
        WHERE trigger_type = ?1 AND enabled = 1
        ",
    )?;
    let rows = statement.query_map([trigger_type], alert_rule_from_row)?;
    let mut matching = Vec::new();
    for rule in rows {
        let rule = rule?;
        if rule_matches_company(connection, &rule, company_id)? {
            matching.push(rule);
        }
    }
    Ok(matching)
}

// ---------------------------------------------------------------------------
// Evaluation (inline hooks — see module docs). Best-effort at call sites.
// ---------------------------------------------------------------------------

/// Today's date (`YYYY-MM-DD`, UTC) — the wall-clock day the per-rule throttle
/// counts against.
fn today_iso() -> String {
    time::OffsetDateTime::now_utc()
        .format(&time::format_description::well_known::Iso8601::DATE)
        .unwrap_or_else(|_| "0000-01-01".to_owned())
}

/// Wall-clock now as RFC3339 — the `fired_at` firing time stamped on every event.
fn now_rfc3339() -> String {
    time::OffsetDateTime::now_utc()
        .format(&time::format_description::well_known::Rfc3339)
        .unwrap_or_else(|_| format!("{}T00:00:00Z", today_iso()))
}

/// Is a signal too old to alert on? `true` when its domain date is strictly older
/// than [`SIGNAL_FRESHNESS_DAYS`] before wall-clock now. An absent or unparseable
/// date is treated as fresh (`false`) — we never suppress a signal we cannot prove
/// is stale (see [`SIGNAL_FRESHNESS_DAYS`]). Applied by the historical-ingest seam
/// (`classify_and_store_signal`) so a backfill of old official filings is silent,
/// while present-detection paths (red flags, KNF) are not gated.
pub(super) fn signal_is_stale(signal_date: Option<&str>) -> bool {
    use time::macros::format_description;
    let Some(raw) = signal_date.map(str::trim).filter(|value| !value.is_empty()) else {
        return false;
    };
    let day: String = raw.chars().take(10).collect();
    let format = format_description!("[year]-[month]-[day]");
    let Ok(parsed) = time::Date::parse(&day, &format) else {
        return false;
    };
    let cutoff = time::OffsetDateTime::now_utc()
        .date()
        .saturating_sub(time::Duration::days(SIGNAL_FRESHNESS_DAYS));
    parsed < cutoff
}

/// A confirmed signal was just created for `company_id`. Fire every enabled
/// `signal_category` rule whose category matches and whose scope covers the
/// company. Returns the number of new attention events.
///
/// Freshness is the CALLER's decision (see [`signal_is_stale`]). The gate lives on
/// the historical-ingest seam (`classify_and_store_signal`), NOT here: a
/// present-detection path (derived red flags, KNF short-position changes) raises a
/// signal NOW whose DOMAIN date is the underlying report's period — old, but not a
/// re-ingest — and must still alert. Gating here would wrongly suppress those.
pub(super) fn evaluate_signal_rules(
    connection: &Connection,
    company_id: &str,
    category: &str,
    signal_id: &str,
    _signal_date: Option<&str>,
) -> StorageResult<usize> {
    // The signal's own filing title, resolved once at fire time and snapshotted
    // onto every event this signal fires (v0.60 D7). At fire time the signal and
    // its feed item both exist; the snapshot is what survives the later feed prune
    // that cascade-deletes the signal row.
    let evidence_title = signal_filing_title(connection, signal_id)?;
    let mut fired = 0;
    for rule in enabled_rules_for_company(connection, TRIGGER_SIGNAL_CATEGORY, company_id)? {
        if rule.signal_category.as_deref() != Some(category) {
            continue;
        }
        if insert_attention_event(
            connection,
            &rule.id,
            &rule.trigger_type,
            company_id,
            EVIDENCE_COMPANY_SIGNAL,
            signal_id,
            evidence_title.as_deref(),
        )? {
            fired += 1;
        }
    }
    Ok(fired)
}

/// The filing title behind a `company_signal` (its feed item's title), resolved at
/// fire time so it can be snapshotted onto the attention event. Returns `None`
/// when the signal or its feed item is already absent — the read model then keeps
/// its live-join fallback.
fn signal_filing_title(connection: &Connection, signal_id: &str) -> StorageResult<Option<String>> {
    let mut statement = connection.prepare(
        "SELECT feed_items.title
         FROM company_signals
         JOIN feed_items ON feed_items.id = company_signals.feed_item_id
         WHERE company_signals.id = ?1",
    )?;
    let mut rows = statement.query([signal_id])?;
    match rows.next()? {
        Some(row) => Ok(row.get::<_, Option<String>>(0)?),
        None => Ok(None),
    }
}

/// An autopilot run for `company_id` reached `finalize_notify`. Fire every
/// enabled `autopilot_run_completed` rule whose scope covers the company.
pub(super) fn evaluate_autopilot_completion(
    connection: &Connection,
    company_id: &str,
    run_id: &str,
) -> StorageResult<usize> {
    let mut fired = 0;
    for rule in enabled_rules_for_company(connection, TRIGGER_AUTOPILOT_RUN_COMPLETED, company_id)?
    {
        // Autopilot evidence (`report_documents` via `autopilot_run`) is NOT
        // cascade-deleted by feed pruning, so its title outlives pruning through
        // the read-time join — no fire-time snapshot needed, and the title is not
        // in scope here without a fresh lookup (v0.60 D7: join fallback retained).
        if insert_attention_event(
            connection,
            &rule.id,
            &rule.trigger_type,
            company_id,
            EVIDENCE_AUTOPILOT_RUN,
            run_id,
            None,
        )? {
            fired += 1;
        }
    }
    Ok(fired)
}

/// A fresh EOD bar landed for `company_id`. Evaluate price rules against the
/// latest stored close and its trailing-52-week window (by domain `date`):
/// `price_enters_range` fires when the close is inside `[min, max]`;
/// `price_week52_low` fires when the close is a new low for the window.
pub(super) fn evaluate_price_rules(
    connection: &Connection,
    company_id: &str,
) -> StorageResult<usize> {
    let Some(latest) = market_data::latest_quote_for(connection, company_id)? else {
        return Ok(0);
    };
    // The quote's domain date is the event's dedup key (`evidence_ref`) — one
    // event per bar. `fired_at` is stamped wall-clock inside `insert_attention_event`.
    let quote_date = latest.date.clone();
    let mut fired = 0;

    for rule in enabled_rules_for_company(connection, TRIGGER_PRICE_ENTERS_RANGE, company_id)? {
        let (Some(min), Some(max)) = (rule.price_min, rule.price_max) else {
            continue;
        };
        if latest.close >= min
            && latest.close <= max
            && insert_attention_event(
                connection,
                &rule.id,
                &rule.trigger_type,
                company_id,
                EVIDENCE_DAILY_QUOTE,
                &quote_date,
                // Price events carry no filing title; the row states its own range.
                None,
            )?
        {
            fired += 1;
        }
    }

    let week52_rules = enabled_rules_for_company(connection, TRIGGER_PRICE_WEEK52_LOW, company_id)?;
    if !week52_rules.is_empty() && is_new_week52_low(connection, company_id, &latest)? {
        for rule in week52_rules {
            if insert_attention_event(
                connection,
                &rule.id,
                &rule.trigger_type,
                company_id,
                EVIDENCE_DAILY_QUOTE,
                &quote_date,
                None,
            )? {
                fired += 1;
            }
        }
    }

    Ok(fired)
}

/// Is `latest` a genuine new 52-week low — strictly below every prior close in
/// the trailing 52-week window (by domain date)? Requires at least one prior bar
/// in the window (a first-ever bar is not a "new" low).
fn is_new_week52_low(
    connection: &Connection,
    company_id: &str,
    latest: &market_data::QuoteBar,
) -> StorageResult<bool> {
    let window_start = shift_iso_date_back(&latest.date, WEEK52_DAYS);
    let prior_min: Option<f64> = connection.query_row(
        "SELECT MIN(close) FROM daily_quotes
         WHERE company_id = ?1 AND date >= ?2 AND date < ?3",
        params![company_id, window_start, latest.date],
        |row| row.get(0),
    )?;
    Ok(match prior_min {
        Some(prior) => latest.close < prior,
        None => false,
    })
}

/// Shift an ISO `YYYY-MM-DD` date back `delta_days` days, saturating rather than
/// panicking; falls back to the epoch (widest window) on a parse failure.
fn shift_iso_date_back(date: &str, delta_days: i64) -> String {
    use time::macros::format_description;
    let format = format_description!("[year]-[month]-[day]");
    let Ok(parsed) = time::Date::parse(date, &format) else {
        return "0000-01-01".to_owned();
    };
    parsed
        .saturating_sub(time::Duration::days(delta_days))
        .format(&format)
        .unwrap_or_else(|_| "0000-01-01".to_owned())
}

// ---------------------------------------------------------------------------
// Store
// ---------------------------------------------------------------------------

/// Attention (alert rules + events) domain store (Architecture v2 / ADR 0050).
/// Reach it via `AppState::attention()`.
#[derive(Clone)]
pub struct AttentionStore {
    db: Database,
}

impl AttentionStore {
    pub(super) fn new(db: Database) -> Self {
        Self { db }
    }

    pub fn create_alert_rule(&self, input: NewAlertRule) -> StorageResult<AlertRule> {
        let connection = self.db.checkout()?;
        create_alert_rule(&connection, input)
    }

    pub fn list_alert_rules(&self) -> StorageResult<Vec<AlertRule>> {
        let connection = self.db.checkout()?;
        list_alert_rules(&connection)
    }

    pub fn update_alert_rule(&self, input: AlertRuleUpdate) -> StorageResult<AlertRule> {
        let connection = self.db.checkout()?;
        update_alert_rule(&connection, input)
    }

    pub fn set_alert_rule_enabled(&self, id: &str, enabled: bool) -> StorageResult<AlertRule> {
        let connection = self.db.checkout()?;
        set_alert_rule_enabled(&connection, id, enabled)
    }

    pub fn delete_alert_rule(&self, id: &str) -> StorageResult<()> {
        let connection = self.db.checkout()?;
        delete_alert_rule(&connection, id)
    }

    pub fn list_attention_events(
        &self,
        input: AttentionEventListInput,
    ) -> StorageResult<Vec<AttentionEvent>> {
        let connection = self.db.checkout()?;
        list_attention_events(&connection, input)
    }

    pub fn mark_attention_event_seen(&self, id: &str) -> StorageResult<()> {
        let connection = self.db.checkout()?;
        mark_attention_event_seen(&connection, id)
    }

    pub fn dismiss_attention_event(&self, id: &str) -> StorageResult<()> {
        let connection = self.db.checkout()?;
        dismiss_attention_event(&connection, id)
    }

    /// Evaluate autopilot-completion rules for a finished run (job hook).
    pub fn evaluate_autopilot_completion(
        &self,
        company_id: &str,
        run_id: &str,
    ) -> StorageResult<usize> {
        let connection = self.db.checkout()?;
        evaluate_autopilot_completion(&connection, company_id, run_id)
    }

    /// Raise the system `job_failed` event for a background job that exhausted its
    /// retries (ADR 0091 dec. 1). Called from the ONE terminal point in the queue
    /// dispatch, and only for kinds classified `FailureSurface::TodayAttention` —
    /// kinds with a richer domain surface keep it exclusively, so a failure never
    /// fires twice.
    ///
    /// `job_id` is the `job_queue.id`: it is the event's `evidence_ref`, which the
    /// read model joins back for the job's kind + `last_error`, and the dedup key
    /// of the system partial index (one event per failed job row, however often it
    /// is reclaimed). `company_id` is `None` for workspace-wide work (nullable
    /// since migration 0118); `subject` is the handler's raw specific (a document
    /// title, a ticker), snapshotted at fire time. Returns `true` when a new event
    /// row was created.
    pub fn record_job_failure(
        &self,
        job_id: &str,
        company_id: Option<&str>,
        subject: Option<&str>,
    ) -> StorageResult<bool> {
        let connection = self.db.checkout()?;
        insert_system_attention_event(
            &connection,
            TRIGGER_JOB_FAILED,
            company_id,
            EVIDENCE_JOB,
            job_id,
            subject,
            &now_rfc3339(),
        )
    }

    /// Evaluate price rules for a company after a fresh EOD bar (job hook).
    pub fn evaluate_price_rules(&self, company_id: &str) -> StorageResult<usize> {
        let connection = self.db.checkout()?;
        evaluate_price_rules(&connection, company_id)
    }
}
