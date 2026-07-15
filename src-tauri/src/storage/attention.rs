//! Alert rules + attention events (ADR 0068, plan v0.54-attention-routing §T2).
//!
//! A user-owned [`AlertRule`] pairs a trigger type with a scope (a single
//! company or a whole watchlist). Rule evaluation runs INLINE in the job stages
//! that produce the evidence — signal classification (`storage::signals`),
//! autopilot `finalize_notify`, and the post-daily-pull price check — never on a
//! new worker lane (the evaluation is a handful of indexed reads; plan §T2). Each
//! fired rule persists an [`AttentionEvent`] linked to its evidence.
//!
//! Dedup + throttle:
//! - **Dedup**: at most one event per `(rule, evidence_type, evidence_ref)`.
//!   Re-running evaluation over the same evidence is a no-op (mirrors
//!   `classify_and_store_signal`), so no re-fire on re-ingest.
//! - **Daily throttle**: at most [`DAILY_THROTTLE_PER_RULE`] event(s) per rule
//!   per day, keyed on the evidence's DOMAIN date (`fired_at`), not wall-clock
//!   ingestion time — a rule pings at most once a day however many distinct
//!   pieces of evidence it matches, keeping the attention surface quiet.

use serde::{Deserialize, Serialize};

use super::database::Database;
use super::*;

/// Max attention events a single rule may fire on one domain day. One keeps the
/// attention surface to at most one ping per rule per day; additional matching
/// evidence that day is suppressed (the underlying signals/quotes stay in their
/// own feeds). Chosen per plan §T2 ("1/day"); a future per-trigger override can
/// widen this without a schema change.
pub const DAILY_THROTTLE_PER_RULE: i64 = 1;

pub const TRIGGER_SIGNAL_CATEGORY: &str = "signal_category";
pub const TRIGGER_AUTOPILOT_RUN_COMPLETED: &str = "autopilot_run_completed";
pub const TRIGGER_PRICE_ENTERS_RANGE: &str = "price_enters_range";
pub const TRIGGER_PRICE_WEEK52_LOW: &str = "price_week52_low";

const TRIGGER_TYPES: &[&str] = &[
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
#[derive(Debug, Clone, Deserialize)]
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
#[derive(Debug, Clone, Default, Deserialize)]
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
    pub rule_id: String,
    #[cfg_attr(
        feature = "ts-export",
        ts(
            type = "\"signal_category\" | \"autopilot_run_completed\" | \"price_enters_range\" | \"price_week52_low\""
        )
    )]
    pub trigger_type: String,
    pub company_id: String,
    #[cfg_attr(
        feature = "ts-export",
        ts(type = "\"company_signal\" | \"autopilot_run\" | \"daily_quote\"")
    )]
    pub evidence_type: String,
    pub evidence_ref: String,
    pub fired_at: String,
    pub seen: bool,
    pub dismissed: bool,
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
            alert_rules.trigger_type,
            attention_events.company_id,
            attention_events.evidence_type,
            attention_events.evidence_ref,
            attention_events.fired_at,
            attention_events.seen,
            attention_events.dismissed
        FROM attention_events
        JOIN alert_rules ON alert_rules.id = attention_events.rule_id
        WHERE (?1 IS NULL OR attention_events.company_id = ?1)
          AND (?2 = 1 OR attention_events.dismissed = 0)
        ORDER BY attention_events.fired_at DESC, attention_events.id DESC
        ",
    )?;
    let rows = statement.query_map(
        params![input.company_id, input.include_dismissed as i64],
        attention_event_from_row,
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

fn attention_event_from_row(row: &rusqlite::Row<'_>) -> rusqlite::Result<AttentionEvent> {
    Ok(AttentionEvent {
        id: row.get(0)?,
        rule_id: row.get(1)?,
        trigger_type: row.get(2)?,
        company_id: row.get(3)?,
        evidence_type: row.get(4)?,
        evidence_ref: row.get(5)?,
        fired_at: row.get(6)?,
        seen: row.get::<_, i64>(7)? != 0,
        dismissed: row.get::<_, i64>(8)? != 0,
    })
}

/// Persist one attention event, applying dedup + the per-rule daily throttle.
/// Returns `true` only when a new event row was created.
///
/// - Dedup: if `(rule_id, evidence_type, evidence_ref)` already fired, this is a
///   no-op and returns `false` (never counts against the throttle).
/// - Throttle: otherwise, if the rule already fired [`DAILY_THROTTLE_PER_RULE`]
///   event(s) on `fire_date`, the new evidence is suppressed (returns `false`).
///
/// `fire_date` is the evidence's domain date (`YYYY-MM-DD`); it is stored in
/// `fired_at` so both dedup and throttle are deterministic and never keyed on
/// wall-clock ingestion time.
pub(super) fn insert_attention_event(
    connection: &Connection,
    rule_id: &str,
    company_id: &str,
    evidence_type: &str,
    evidence_ref: &str,
    fire_date: &str,
) -> StorageResult<bool> {
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

    let fired_today: i64 = connection.query_row(
        "SELECT COUNT(*) FROM attention_events
         WHERE rule_id = ?1 AND substr(fired_at, 1, 10) = ?2",
        params![rule_id, fire_date],
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
    let fired_at = format!("{fire_date}T00:00:00Z");
    let affected = connection.execute(
        "
        INSERT INTO attention_events (id, rule_id, company_id, evidence_type, evidence_ref, fired_at)
        VALUES (?1, ?2, ?3, ?4, ?5, ?6)
        ON CONFLICT(rule_id, evidence_type, evidence_ref) DO NOTHING
        ",
        params![id, rule_id, company_id, evidence_type, evidence_ref, fired_at],
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

/// Today's date (`YYYY-MM-DD`, UTC) — the fire date for evidence without a
/// domain date (autopilot completion).
fn today_iso() -> String {
    time::OffsetDateTime::now_utc()
        .format(&time::format_description::well_known::Iso8601::DATE)
        .unwrap_or_else(|_| "0000-01-01".to_owned())
}

/// A confirmed signal was just created for `company_id`. Fire every enabled
/// `signal_category` rule whose category matches and whose scope covers the
/// company. Returns the number of new attention events.
pub(super) fn evaluate_signal_rules(
    connection: &Connection,
    company_id: &str,
    category: &str,
    signal_id: &str,
    signal_date: Option<&str>,
) -> StorageResult<usize> {
    let fire_date = normalize_fire_date(signal_date);
    let mut fired = 0;
    for rule in enabled_rules_for_company(connection, TRIGGER_SIGNAL_CATEGORY, company_id)? {
        if rule.signal_category.as_deref() != Some(category) {
            continue;
        }
        if insert_attention_event(
            connection,
            &rule.id,
            company_id,
            EVIDENCE_COMPANY_SIGNAL,
            signal_id,
            &fire_date,
        )? {
            fired += 1;
        }
    }
    Ok(fired)
}

/// An autopilot run for `company_id` reached `finalize_notify`. Fire every
/// enabled `autopilot_run_completed` rule whose scope covers the company.
pub(super) fn evaluate_autopilot_completion(
    connection: &Connection,
    company_id: &str,
    run_id: &str,
) -> StorageResult<usize> {
    let fire_date = today_iso();
    let mut fired = 0;
    for rule in enabled_rules_for_company(connection, TRIGGER_AUTOPILOT_RUN_COMPLETED, company_id)?
    {
        if insert_attention_event(
            connection,
            &rule.id,
            company_id,
            EVIDENCE_AUTOPILOT_RUN,
            run_id,
            &fire_date,
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
    let fire_date = latest.date.clone();
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
                company_id,
                EVIDENCE_DAILY_QUOTE,
                &fire_date,
                &fire_date,
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
                company_id,
                EVIDENCE_DAILY_QUOTE,
                &fire_date,
                &fire_date,
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

/// Normalize a domain date to `YYYY-MM-DD`, falling back to today when absent.
fn normalize_fire_date(date: Option<&str>) -> String {
    date.map(str::trim)
        .filter(|value| !value.is_empty())
        .map(|value| value.chars().take(10).collect::<String>())
        .unwrap_or_else(today_iso)
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

    /// Evaluate price rules for a company after a fresh EOD bar (job hook).
    pub fn evaluate_price_rules(&self, company_id: &str) -> StorageResult<usize> {
        let connection = self.db.checkout()?;
        evaluate_price_rules(&connection, company_id)
    }
}
