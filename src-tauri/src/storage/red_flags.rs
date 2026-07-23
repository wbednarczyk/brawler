//! Company red-flags: derived-at-read panel state, signal-raised detections, and
//! per-flag acknowledgement (ADR 0083 Decision 8/9, plan v0.57 T7).
//!
//! Active flags are a **computed read model** ([`load_red_flags_view`]) — never a
//! stored projection (ADR 0083 "Rejected: a stored red_flags table"). Only two
//! things persist: the user's acknowledgements (`red_flag_acks`) and the
//! raise-events (synthetic `feed_items` + confirmed `company_signals`, for
//! alerting + dedup).
//!
//! Two flag families:
//!
//! - **Raised** (`report_delay`, `fund_exit`, `score_deterioration`): detected
//!   inline at the producing seams and raised via the KNF pattern — one synthetic
//!   `feed_items` row + one `confirmed` `company_signals` row in the empty-pattern
//!   category (migration 0092), so existing `signal_category` alert rules fire
//!   with zero new plumbing (ADR 0068 additive rule). Deterministic flag id
//!   (`rf:<type>:<company>:<evidence>`) makes every raise idempotent; an
//!   acknowledged flag is never re-raised (and its signal, already written, never
//!   re-fires because `INSERT OR IGNORE` no-ops on the second pass).
//! - **Composed at read** (`auditor_red_flag`, `short_spike`): assembled from
//!   signals / read models that already exist and already alert — the auditor
//!   opinion signal (ADR 0079) and the KNF short-position view (ADR 0069). No new
//!   raising: they carry their own alert path already, so composing here avoids a
//!   duplicate signal / duplicate alert.

use std::collections::{HashMap, HashSet};
use std::str::FromStr;

use rust_decimal::Decimal;

use super::database::Database;
use super::*;

/// Feed-item display type for a raised flag's synthetic item.
const FEED_ITEM_TYPE: &str = "Red flag";
/// Internal adapter that owns the synthetic feed items (seeded, `enabled = 0`).
const ADAPTER_ID: &str = "brawler-red-flags";
const SOURCE_NAME: &str = "Brawler";
const ATTRIBUTION: &str = "Brawler — sygnał pochodny";

/// Flag types (stable keys; the three raised ones are also `signal_categories`).
const FLAG_REPORT_DELAY: &str = "report_delay";
const FLAG_FUND_EXIT: &str = "fund_exit";
const FLAG_SCORE_DETERIORATION: &str = "score_deterioration";
const FLAG_AUDITOR: &str = "auditor_red_flag";
const FLAG_SHORT_SPIKE: &str = "short_spike";

/// Grace period after an expected periodic-report date before a `report_delay`
/// fires (ADR 0083 Decision 8).
const REPORT_DELAY_GRACE_DAYS: i64 = 3;
/// `short_spike` threshold: a KNF net-short 30-day change above this many
/// percentage points composes a flag (documented constant, ADR 0083 Decision 8 —
/// the KNF `short_position_change` signals already alert on each register move, so
/// this only surfaces the *aggregate* spike in the panel; no new raising).
const SHORT_SPIKE_THRESHOLD_PP: f64 = 0.5;
/// The MAR / Art. 69 ownership disclosure threshold (percent). An `espi_filing`
/// snapshot whose resulting stake crosses **below** this (or states zero) is an
/// exit from the disclosure picture (data-model Ownership interface note, refined
/// 2026-07-17). A decrease that stays at/above it is not an exit.
fn disclosure_threshold() -> Decimal {
    Decimal::from(5)
}

/// Per-type severity (ADR 0083 Decision 8 — a fixed static map, not
/// user-configurable in v1).
fn severity_of(flag_type: &str) -> &'static str {
    match flag_type {
        FLAG_AUDITOR | FLAG_REPORT_DELAY => "high",
        _ => "medium", // fund_exit, score_deterioration, short_spike
    }
}

/// Sort rank so `high` orders before `medium`.
fn severity_rank(severity: &str) -> u8 {
    match severity {
        "high" => 0,
        _ => 1,
    }
}

// ============================================================================
// DTOs
// ============================================================================

/// One red flag, active or acknowledged. Decision support only (ADR 0083) — the
/// title states the observed condition, never advice language.
#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize)]
#[cfg_attr(feature = "ts-export", derive(ts_rs::TS))]
#[cfg_attr(
    feature = "ts-export",
    ts(export, export_to = "../../src/api/generated/")
)]
#[serde(rename_all = "camelCase")]
pub struct RedFlag {
    /// Deterministic instance id (`rf:<type>:<company>:<evidence>`); the ack key.
    pub flag_id: String,
    /// One of `auditor_red_flag | report_delay | fund_exit | score_deterioration | short_spike`.
    pub flag_type: String,
    /// `high | medium` (the ADR 0083 static severity map).
    pub severity: String,
    /// Human evidence description (Polish; the UI shows it verbatim).
    pub title: String,
    /// Domain date the condition was observed (`YYYY-MM-DD`).
    pub raised_date: String,
    /// Link to the underlying evidence item, when one exists.
    pub evidence_url: Option<String>,
    /// The synthetic/underlying feed item id, so the panel can open it in-app.
    pub evidence_feed_item_id: Option<String>,
    /// Set only on acknowledged (history) flags.
    pub acked_at: Option<String>,
}

/// The computed per-company panel state (ADR 0083 Decision 8).
#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize)]
#[cfg_attr(feature = "ts-export", derive(ts_rs::TS))]
#[cfg_attr(
    feature = "ts-export",
    ts(export, export_to = "../../src/api/generated/")
)]
#[serde(rename_all = "camelCase")]
pub struct RedFlagsView {
    /// Active flags, highest severity first then newest raised.
    pub active: Vec<RedFlag>,
    /// Acknowledged flags, newest ack first.
    pub history: Vec<RedFlag>,
}

#[derive(Debug, Clone, serde::Deserialize)]
#[cfg_attr(feature = "ts-export", derive(ts_rs::TS))]
#[cfg_attr(
    feature = "ts-export",
    ts(export, export_to = "../../src/api/generated/")
)]
#[serde(rename_all = "camelCase")]
pub struct RedFlagsInput {
    pub company_id: String,
}

#[derive(Debug, Clone, serde::Deserialize, schemars::JsonSchema)]
#[cfg_attr(feature = "ts-export", derive(ts_rs::TS))]
#[cfg_attr(
    feature = "ts-export",
    ts(export, export_to = "../../src/api/generated/")
)]
#[serde(rename_all = "camelCase")]
pub struct AcknowledgeRedFlagInput {
    pub flag_id: String,
}

// ============================================================================
// Flag id
// ============================================================================

/// Deterministic flag id: `rf:<type>:<company>:<evidence>`. `type` and
/// `company_id` are colon-free tokens, so [`parse_flag_id`] splits them back out
/// (the ack command receives only the id, per the contract).
fn flag_id(flag_type: &str, company_id: &str, evidence: &str) -> String {
    format!("rf:{flag_type}:{company_id}:{evidence}")
}

/// Recover `(flag_type, company_id)` from a flag id (the evidence tail may itself
/// contain colons, so split into at most four parts).
fn parse_flag_id(id: &str) -> Option<(String, String)> {
    let mut parts = id.splitn(4, ':');
    match (parts.next(), parts.next(), parts.next(), parts.next()) {
        (Some("rf"), Some(flag_type), Some(company_id), Some(_evidence)) => {
            Some((flag_type.to_owned(), company_id.to_owned()))
        }
        _ => None,
    }
}

// ============================================================================
// Raising (KNF pattern) — the three detected-and-raised flag types
// ============================================================================

/// Raise one derived flag: a synthetic feed item + a confirmed signal in the
/// flag's category, firing any matching alert rules. Idempotent by deterministic
/// id; an **acknowledged** flag is never re-raised (and its already-written signal
/// never re-fires — `INSERT OR IGNORE` no-ops). Returns `true` when a signal was
/// newly written (i.e. a genuinely new raise).
fn raise_flag(
    connection: &Connection,
    company_id: &str,
    flag_type: &str,
    evidence: &str,
    title: &str,
    published_at: &str,
) -> StorageResult<bool> {
    let id = flag_id(flag_type, company_id, evidence);
    if is_acked(connection, &id)? {
        return Ok(false); // acked → never re-raise
    }

    let fetched_at = now_iso();
    let feed_item_id = super::feed_item_id(&id);
    let source_url = format!("brawler://red-flag/{id}");
    super::ingestion::upsert_feed_item(
        connection,
        &super::ingestion::NormalizedFeedItem {
            id: &feed_item_id,
            item_type: FEED_ITEM_TYPE,
            source_adapter_id: ADAPTER_ID,
            source_name: SOURCE_NAME,
            source_url: &source_url,
            title,
            summary: None,
            body_text: None,
            language: "pl",
            published_at: Some(published_at),
            fetched_at: &fetched_at,
            dedupe_key: &id,
            attribution: ATTRIBUTION,
            display_company: &company_ticker(connection, company_id)?,
            duplicate_signature: None,
        },
    )?;
    connection.execute(
        "INSERT OR IGNORE INTO feed_item_companies (feed_item_id, company_id, match_type)
         VALUES (?1, ?2, 'derived')",
        params![feed_item_id, company_id],
    )?;

    let signal_id = super::signals::signal_id(&feed_item_id, flag_type);
    let signal_date: String = published_at.chars().take(10).collect();
    let affected = connection.execute(
        "INSERT INTO company_signals (
            id, company_id, feed_item_id, category, confidence,
            classified_by, status, signal_date
         ) VALUES (?1, ?2, ?3, ?4, 1.0, 'rule', 'confirmed', ?5)
         ON CONFLICT(feed_item_id, category) DO NOTHING",
        params![signal_id, company_id, feed_item_id, flag_type, signal_date],
    )?;

    if affected > 0 {
        if let Err(error) = super::attention::evaluate_signal_rules(
            connection,
            company_id,
            flag_type,
            &signal_id,
            Some(&signal_date),
        ) {
            log::warn!("module=red_flags stage=signal_eval flagId={id} error={error}");
        }
    }
    Ok(affected > 0)
}

// ============================================================================
// Detection: report_delay
// ============================================================================

/// Raise a `report_delay` for every tracked company whose expected
/// `periodic_report` calendar date passed the 3-day grace with no official report
/// ingested since (ADR 0083 Decision 8). Evidence = the calendar event. Runs at
/// refresh completion; idempotent. Returns the number of newly-raised flags.
pub(super) fn detect_report_delays(connection: &Connection, today: &str) -> StorageResult<usize> {
    let cutoff = date_minus_days(today, REPORT_DELAY_GRACE_DAYS);
    let mut statement = connection.prepare(
        "SELECT ce.id, ce.company_id, ce.event_date, ce.title
         FROM company_events ce
         WHERE ce.event_type = 'periodic_report'
           AND ce.event_date <= ?1
           AND NOT EXISTS (
             SELECT 1 FROM feed_items fi
             JOIN feed_item_companies fic ON fic.feed_item_id = fi.id
             WHERE fic.company_id = ce.company_id
               AND fi.type = 'Official report'
               AND fi.published_at >= ce.event_date
           )
         ORDER BY ce.event_date ASC, ce.id ASC",
    )?;
    let rows: Vec<(String, String, String, String)> = statement
        .query_map(params![cutoff], |row| {
            Ok((row.get(0)?, row.get(1)?, row.get(2)?, row.get(3)?))
        })?
        .collect::<Result<Vec<_>, _>>()?;

    let mut raised = 0;
    for (event_id, company_id, event_date, title) in rows {
        let flag_title = format!("Opóźniony raport okresowy: {title} (planowany {event_date})");
        if raise_flag(
            connection,
            &company_id,
            FLAG_REPORT_DELAY,
            &event_id,
            &flag_title,
            &event_date,
        )? {
            raised += 1;
        }
    }
    Ok(raised)
}

// ============================================================================
// Detection: fund_exit
// ============================================================================

/// Raise a `fund_exit` for a company (ADR 0083 Decision 8, data-model Ownership
/// interface note). Read-only over `ownership_stakes`; runs at ownership ingest;
/// idempotent. Two evidence forms, deduped so one real-world exit flags once:
///
/// 1. **Basis vanish** — a holder present in the PREVIOUS full-picture disclosure
///    basis (`report_document` / `aggregator`) and absent from the NEWEST one.
/// 2. **ESPI threshold crossing** — the holder's newest stake is an `espi_filing`
///    snapshot whose resulting pct crosses BELOW the 5% disclosure threshold (or
///    states zero) while the immediately-prior known stake was ≥ 5%. A mere
///    decrease that stays ≥ 5% raises nothing.
///
/// **Dedup rule:** both forms route through [`raise_fund_exit`], which skips a
/// holder that already has an **active** (raised, not acknowledged) `fund_exit`
/// flag — so the fresh ESPI crossing and a later basis-vanish for the same holder
/// never produce two active flags. (An acknowledged prior flag is out of the
/// active window and does not block a genuinely fresh later observation.)
pub(super) fn detect_fund_exits(connection: &Connection, company_id: &str) -> StorageResult<usize> {
    let mut active = active_fund_exit_holders(connection, company_id)?;
    let mut raised = 0;
    raised += detect_basis_vanish_exits(connection, company_id, &mut active)?;
    raised += detect_espi_threshold_crossings(connection, company_id, &mut active)?;
    Ok(raised)
}

/// Form 1: holders that vanished from the newest full-picture basis.
fn detect_basis_vanish_exits(
    connection: &Connection,
    company_id: &str,
    active: &mut HashSet<String>,
) -> StorageResult<usize> {
    let mut basis_stmt = connection.prepare(
        "SELECT DISTINCT as_of FROM ownership_stakes
         WHERE company_id = ?1 AND source IN ('report_document', 'aggregator')
         ORDER BY as_of DESC LIMIT 2",
    )?;
    let bases: Vec<String> = basis_stmt
        .query_map([company_id], |row| row.get::<_, String>(0))?
        .collect::<Result<Vec<_>, _>>()?;
    if bases.len() < 2 {
        return Ok(0); // need a previous basis to diff against
    }
    let newest = &bases[0];
    let previous = &bases[1];

    let mut exit_stmt = connection.prepare(
        "SELECT DISTINCT o.holder_name_normalized, o.holder_name_raw
         FROM ownership_stakes o
         WHERE o.company_id = ?1
           AND o.source IN ('report_document', 'aggregator')
           AND o.as_of = ?2
           AND o.holder_name_normalized NOT IN (
             SELECT n.holder_name_normalized FROM ownership_stakes n
             WHERE n.company_id = ?1
               AND n.source IN ('report_document', 'aggregator')
               AND n.as_of = ?3
           )
         ORDER BY o.holder_name_normalized ASC",
    )?;
    let exits: Vec<(String, String)> = exit_stmt
        .query_map(params![company_id, previous, newest], |row| {
            Ok((row.get(0)?, row.get(1)?))
        })?
        .collect::<Result<Vec<_>, _>>()?;

    let mut raised = 0;
    for (normalized, raw) in exits {
        let title = format!("Wyjście z akcjonariatu: {raw} (nieobecny w stanie na {newest})");
        if raise_fund_exit(connection, company_id, active, &normalized, &title, newest)? {
            raised += 1;
        }
    }
    Ok(raised)
}

/// One stake row for the ESPI-crossing scan (newest-first per holder).
struct StakeRow {
    holder_normalized: String,
    holder_raw: String,
    as_of: String,
    source: String,
    effective_pct: Option<Decimal>,
}

/// The disclosure-effective pct: `capital_pct`, else `votes_pct` when capital is
/// absent (data-model interface note). `None` when neither is a parseable number.
fn effective_pct(capital: Option<String>, votes: Option<String>) -> Option<Decimal> {
    capital
        .or(votes)
        .and_then(|value| Decimal::from_str(value.trim()).ok())
}

/// Form 2: the holder's newest stake is an `espi_filing` snapshot that crossed
/// below the 5% threshold while the immediately-prior known stake was ≥ 5%.
fn detect_espi_threshold_crossings(
    connection: &Connection,
    company_id: &str,
    active: &mut HashSet<String>,
) -> StorageResult<usize> {
    let mut statement = connection.prepare(
        "SELECT holder_name_normalized, holder_name_raw, as_of, source, capital_pct, votes_pct
         FROM ownership_stakes
         WHERE company_id = ?1
         ORDER BY holder_name_normalized ASC, as_of DESC, created_at DESC, id DESC",
    )?;
    let rows: Vec<StakeRow> = statement
        .query_map([company_id], |row| {
            Ok(StakeRow {
                holder_normalized: row.get(0)?,
                holder_raw: row.get(1)?,
                as_of: row.get(2)?,
                source: row.get(3)?,
                effective_pct: effective_pct(row.get(4)?, row.get(5)?),
            })
        })?
        .collect::<Result<Vec<_>, _>>()?;

    let threshold = disclosure_threshold();
    let mut raised = 0;
    let mut index = 0;
    while index < rows.len() {
        // The group for this holder is the contiguous run of equal identities
        // (already newest-first). `newest` is rows[index].
        let holder = &rows[index].holder_normalized;
        let group_end = rows[index..]
            .iter()
            .position(|r| &r.holder_normalized != holder)
            .map(|offset| index + offset)
            .unwrap_or(rows.len());
        let newest = &rows[index];

        // A crossing requires: newest is an ESPI snapshot, effective < 5, and the
        // immediately-prior stake with a strictly earlier as_of was ≥ 5.
        if newest.source == "espi_filing" {
            if let Some(current) = newest.effective_pct {
                if current < threshold {
                    let prior = rows[index + 1..group_end]
                        .iter()
                        .find(|r| r.as_of < newest.as_of);
                    if let Some(prior) = prior {
                        if prior.effective_pct.is_some_and(|p| p >= threshold) {
                            let title = format!(
                                "Wyjście z akcjonariatu: {} — poniżej progu 5% (na {})",
                                newest.holder_raw, newest.as_of
                            );
                            if raise_fund_exit(
                                connection,
                                company_id,
                                active,
                                &newest.holder_normalized,
                                &title,
                                &newest.as_of,
                            )? {
                                raised += 1;
                            }
                        }
                    }
                }
            }
        }
        index = group_end;
    }
    Ok(raised)
}

/// Raise one `fund_exit` for a holder, deduped against the active window: if the
/// holder already has an active (unacknowledged) `fund_exit`, skip (the two
/// evidence forms must not double-flag one real-world exit). Evidence =
/// `{holder}@{as_of}` (the holder identity is recoverable from the flag id, so the
/// dedup set can be rebuilt at read). Returns whether a NEW flag was raised.
fn raise_fund_exit(
    connection: &Connection,
    company_id: &str,
    active: &mut HashSet<String>,
    holder_normalized: &str,
    title: &str,
    as_of: &str,
) -> StorageResult<bool> {
    if active.contains(holder_normalized) {
        return Ok(false);
    }
    let evidence = format!("{holder_normalized}@{as_of}");
    let raised = raise_flag(
        connection,
        company_id,
        FLAG_FUND_EXIT,
        &evidence,
        title,
        as_of,
    )?;
    if raised {
        active.insert(holder_normalized.to_owned());
    }
    Ok(raised)
}

/// The set of holder identities that currently have an **active** (raised,
/// not-acknowledged) `fund_exit` flag for the company — the dedup window. Rebuilt
/// from the raised signals' feed-item dedupe keys (`rf:fund_exit:<company>:<holder>@<as_of>`).
fn active_fund_exit_holders(
    connection: &Connection,
    company_id: &str,
) -> StorageResult<HashSet<String>> {
    let prefix = format!("rf:fund_exit:{company_id}:");
    let mut statement = connection.prepare(
        "SELECT fi.dedupe_key
         FROM company_signals cs
         JOIN feed_items fi ON fi.id = cs.feed_item_id
         WHERE cs.company_id = ?1
           AND cs.status = 'confirmed'
           AND cs.category = 'fund_exit'",
    )?;
    let keys: Vec<String> = statement
        .query_map([company_id], |row| row.get::<_, String>(0))?
        .collect::<Result<Vec<_>, _>>()?;

    let mut holders = HashSet::new();
    for key in keys {
        if is_acked(connection, &key)? {
            continue; // acknowledged flags are out of the active window
        }
        if let Some(rest) = key.strip_prefix(&prefix) {
            if let Some((holder, _as_of)) = rest.rsplit_once('@') {
                holders.insert(holder.to_owned());
            }
        }
    }
    Ok(holders)
}

/// Run the ESPI threshold-crossing scan across every company with `espi_filing`
/// stakes — the global sweep for the ESPI-ingest seam (basis-vanish stays at the
/// report-document seam). Idempotent + best-effort.
pub(super) fn detect_all_espi_crossings(connection: &Connection) -> StorageResult<usize> {
    let mut statement = connection
        .prepare("SELECT DISTINCT company_id FROM ownership_stakes WHERE source = 'espi_filing'")?;
    let companies: Vec<String> = statement
        .query_map([], |row| row.get::<_, String>(0))?
        .collect::<Result<Vec<_>, _>>()?;
    let mut raised = 0;
    for company_id in companies {
        let mut active = active_fund_exit_holders(connection, &company_id)?;
        raised += detect_espi_threshold_crossings(connection, &company_id, &mut active)?;
    }
    Ok(raised)
}

// ============================================================================
// Detection: score_deterioration
// ============================================================================

/// Raise a `score_deterioration` when a company's latest FY health score worsens
/// materially vs the prior FY: Piotroski F drop ≥ 2, or an Altman Z″ band
/// downgrade (ADR 0083 Decision 8). Pure recompute over the metrics context (no
/// stored scores). Both periods must be `headline` states — a missing prior FY or
/// an `insufficient_data` reading never fires. Evidence = the two FY periods. Runs
/// at fact-confirmation recompute; idempotent.
pub(super) fn detect_score_deterioration(
    connection: &Connection,
    company_id: &str,
    today: &str,
) -> StorageResult<usize> {
    let context = super::quality_frameworks::load_metrics_context(connection, company_id)?;
    let statement_type = super::companies::get_statement_type(connection, company_id)?;
    let history = crate::fundamentals::health::company_health(&context, &statement_type);
    if history.len() < 2 {
        return Ok(0);
    }
    let latest = &history[0];
    let prior = &history[1];

    let reasons = deterioration_reasons(latest, prior);
    if reasons.is_empty() {
        return Ok(0);
    }

    let evidence = format!("{}->{}", latest.period_id, prior.period_id);
    let title = format!(
        "Pogorszenie kondycji (FY{} vs FY{}): {}",
        latest.fiscal_year,
        prior.fiscal_year,
        reasons.join(", ")
    );
    let raised = raise_flag(
        connection,
        company_id,
        FLAG_SCORE_DETERIORATION,
        &evidence,
        &title,
        today,
    )?;
    Ok(usize::from(raised))
}

/// Pure deterioration decision (ADR 0083 Decision 8): Piotroski F drop ≥ 2, or an
/// Altman Z″ band downgrade, both requiring `headline` states on both periods (a
/// missing prior FY or `insufficient_data` reading never fires). Returns the
/// human reasons (empty = no deterioration).
pub(super) fn deterioration_reasons(
    latest: &crate::fundamentals::health::HealthPeriodScores,
    prior: &crate::fundamentals::health::HealthPeriodScores,
) -> Vec<String> {
    use crate::fundamentals::health::{AltmanBand, AltmanScore, PiotroskiScore};

    let mut reasons = Vec::new();

    let f_drop = matches!(
        (&latest.piotroski, &prior.piotroski),
        (
            PiotroskiScore::Headline { score: now, .. },
            PiotroskiScore::Headline { score: was, .. },
        ) if *was >= now.saturating_add(2)
    );
    if f_drop {
        reasons.push("spadek Piotroski F ≥ 2".to_owned());
    }

    fn band_rank(band: AltmanBand) -> u8 {
        match band {
            AltmanBand::Safe => 2,
            AltmanBand::Grey => 1,
            AltmanBand::Distress => 0,
        }
    }
    let band_down = matches!(
        (&latest.altman, &prior.altman),
        (
            AltmanScore::Headline { band: now, .. },
            AltmanScore::Headline { band: was, .. },
        ) if band_rank(*now) < band_rank(*was)
    );
    if band_down {
        reasons.push("obniżenie pasma Altman Z″".to_owned());
    }

    reasons
}

// ============================================================================
// Read model (computed)
// ============================================================================

/// Compute the per-company red-flags view (ADR 0083 Decision 8). Active flags =
/// raised signals (three categories) + composed auditor / short-spike flags, minus
/// acknowledged ones; history = acknowledged flags (reconstructed from the ack
/// rows so they persist even after the condition clears). `today` anchors the
/// short-spike window.
pub(super) fn load_red_flags_view(
    connection: &Connection,
    company_id: &str,
    today: &str,
) -> StorageResult<RedFlagsView> {
    let acked = load_acks(connection, company_id)?;

    // All currently-detectable candidate flags, keyed by flag id.
    let mut candidates: HashMap<String, RedFlag> = HashMap::new();
    for flag in raised_flags(connection, company_id)? {
        candidates.insert(flag.flag_id.clone(), flag);
    }
    for flag in composed_auditor_flags(connection, company_id)? {
        candidates.insert(flag.flag_id.clone(), flag);
    }
    if let Some(flag) = composed_short_spike_flag(connection, company_id, today)? {
        candidates.insert(flag.flag_id.clone(), flag);
    }

    // Active = candidates the user has not acknowledged.
    let mut active: Vec<RedFlag> = candidates
        .values()
        .filter(|flag| !acked.contains_key(&flag.flag_id))
        .cloned()
        .collect();
    active.sort_by(|a, b| {
        severity_rank(&a.severity)
            .cmp(&severity_rank(&b.severity))
            .then_with(|| b.raised_date.cmp(&a.raised_date))
            .then_with(|| a.flag_id.cmp(&b.flag_id))
    });

    // History = every acknowledgement, newest first. Prefer the live candidate
    // detail; else reconstruct a minimal entry from the ack row (the condition
    // may have since cleared, but the acknowledgement is durable).
    let mut history: Vec<RedFlag> = acked
        .values()
        .map(|ack| match candidates.get(&ack.flag_id) {
            Some(flag) => RedFlag {
                acked_at: Some(ack.acked_at.clone()),
                ..flag.clone()
            },
            None => RedFlag {
                flag_id: ack.flag_id.clone(),
                flag_type: ack.flag_type.clone(),
                severity: severity_of(&ack.flag_type).to_owned(),
                title: flag_type_label(&ack.flag_type).to_owned(),
                raised_date: ack.acked_at.chars().take(10).collect(),
                evidence_url: None,
                evidence_feed_item_id: None,
                acked_at: Some(ack.acked_at.clone()),
            },
        })
        .collect();
    history.sort_by(|a, b| {
        b.acked_at
            .cmp(&a.acked_at)
            .then_with(|| a.flag_id.cmp(&b.flag_id))
    });

    Ok(RedFlagsView { active, history })
}

/// Raised flags (the three signal-backed categories): recover the flag id from
/// the synthetic feed item's dedupe key.
fn raised_flags(connection: &Connection, company_id: &str) -> StorageResult<Vec<RedFlag>> {
    let mut statement = connection.prepare(
        "SELECT cs.category, fi.dedupe_key, fi.title, fi.source_url, fi.id, cs.signal_date
         FROM company_signals cs
         JOIN feed_items fi ON fi.id = cs.feed_item_id
         WHERE cs.company_id = ?1
           AND cs.status = 'confirmed'
           AND cs.category IN ('report_delay', 'fund_exit', 'score_deterioration')
         ORDER BY cs.signal_date DESC, cs.id DESC",
    )?;
    let rows = statement.query_map([company_id], |row| {
        let flag_type: String = row.get(0)?;
        let flag_id: String = row.get(1)?;
        let title: String = row.get(2)?;
        let source_url: Option<String> = row.get(3)?;
        let feed_item_id: String = row.get(4)?;
        let signal_date: String = row.get(5)?;
        Ok(RedFlag {
            severity: severity_of(&flag_type).to_owned(),
            flag_type,
            flag_id,
            title,
            raised_date: signal_date,
            evidence_url: source_url,
            evidence_feed_item_id: Some(feed_item_id),
            acked_at: None,
        })
    })?;
    rows.collect::<Result<Vec<_>, _>>()
        .map_err(StorageError::from)
}

/// Compose `auditor_red_flag`s from confirmed `auditor_opinion` signals (ADR 0079,
/// which only raises for adverse/qualified/disclaimer/going-concern opinions). No
/// new raising — the auditor signal already alerts.
fn composed_auditor_flags(
    connection: &Connection,
    company_id: &str,
) -> StorageResult<Vec<RedFlag>> {
    let mut statement = connection.prepare(
        "SELECT cs.feed_item_id, fi.title, fi.source_url, cs.signal_date
         FROM company_signals cs
         JOIN feed_items fi ON fi.id = cs.feed_item_id
         WHERE cs.company_id = ?1
           AND cs.status = 'confirmed'
           AND cs.category = 'auditor_opinion'
         ORDER BY cs.signal_date DESC, cs.id DESC",
    )?;
    let rows = statement.query_map([company_id], |row| {
        let feed_item_id: String = row.get(0)?;
        let title: String = row.get(1)?;
        let source_url: Option<String> = row.get(2)?;
        let signal_date: String = row.get(3)?;
        Ok((feed_item_id, title, source_url, signal_date))
    })?;
    let mut flags = Vec::new();
    for row in rows {
        let (feed_item_id, title, source_url, signal_date) = row?;
        flags.push(RedFlag {
            flag_id: flag_id(FLAG_AUDITOR, company_id, &feed_item_id),
            flag_type: FLAG_AUDITOR.to_owned(),
            severity: severity_of(FLAG_AUDITOR).to_owned(),
            title,
            raised_date: signal_date,
            evidence_url: source_url,
            evidence_feed_item_id: Some(feed_item_id),
            acked_at: None,
        });
    }
    Ok(flags)
}

/// Compose a `short_spike` flag when the KNF 30-day net-short change exceeds the
/// threshold (ADR 0083 Decision 8). No new raising — the underlying
/// `short_position_change` signals already alert on each register move; this only
/// surfaces the aggregate spike. Evidence = the most recent in-window register
/// change date (a stable anchor while the window holds).
fn composed_short_spike_flag(
    connection: &Connection,
    company_id: &str,
    today: &str,
) -> StorageResult<Option<RedFlag>> {
    let view = super::short_positions::load_short_positions_view(connection, company_id, today)?;
    if view.delta_30d_pp <= SHORT_SPIKE_THRESHOLD_PP {
        return Ok(None);
    }
    let cutoff = date_minus_days(today, 30);
    let anchor: Option<String> = connection
        .query_row(
            "SELECT max(position_date) FROM short_position_events
             WHERE company_id = ?1 AND position_date >= ?2",
            params![company_id, cutoff],
            |row| row.get::<_, Option<String>>(0),
        )
        .optional()?
        .flatten();
    let anchor = anchor.unwrap_or_else(|| today.to_owned());
    let delta = format!("{:.2}", view.delta_30d_pp).replace('.', ",");
    Ok(Some(RedFlag {
        flag_id: flag_id(FLAG_SHORT_SPIKE, company_id, &anchor),
        flag_type: FLAG_SHORT_SPIKE.to_owned(),
        severity: severity_of(FLAG_SHORT_SPIKE).to_owned(),
        title: format!("Skok krótkiej sprzedaży: +{delta} pp / 30 dni"),
        raised_date: anchor,
        evidence_url: None,
        evidence_feed_item_id: None,
        acked_at: None,
    }))
}

// ============================================================================
// Acknowledgement
// ============================================================================

struct Ack {
    flag_id: String,
    flag_type: String,
    acked_at: String,
}

fn load_acks(connection: &Connection, company_id: &str) -> StorageResult<HashMap<String, Ack>> {
    let mut statement = connection
        .prepare("SELECT flag_id, flag_type, acked_at FROM red_flag_acks WHERE company_id = ?1")?;
    let rows = statement.query_map([company_id], |row| {
        Ok(Ack {
            flag_id: row.get(0)?,
            flag_type: row.get(1)?,
            acked_at: row.get(2)?,
        })
    })?;
    let mut map = HashMap::new();
    for ack in rows {
        let ack = ack?;
        map.insert(ack.flag_id.clone(), ack);
    }
    Ok(map)
}

fn is_acked(connection: &Connection, flag_id: &str) -> StorageResult<bool> {
    let found: Option<String> = connection
        .query_row(
            "SELECT flag_id FROM red_flag_acks WHERE flag_id = ?1",
            [flag_id],
            |row| row.get(0),
        )
        .optional()?;
    Ok(found.is_some())
}

/// Acknowledge a flag by its deterministic id (idempotent). The `company_id` /
/// `flag_type` are recovered from the id (the contract passes only `flagId`); an
/// id that does not parse is ignored (the ack table stays clean).
pub(super) fn acknowledge(connection: &Connection, flag_id: &str) -> StorageResult<()> {
    let Some((flag_type, company_id)) = parse_flag_id(flag_id) else {
        return Ok(());
    };
    connection.execute(
        "INSERT INTO red_flag_acks (flag_id, company_id, flag_type)
         VALUES (?1, ?2, ?3)
         ON CONFLICT(flag_id) DO NOTHING",
        params![flag_id, company_id, flag_type],
    )?;
    Ok(())
}

// ============================================================================
// Helpers
// ============================================================================

fn flag_type_label(flag_type: &str) -> &'static str {
    match flag_type {
        FLAG_REPORT_DELAY => "Opóźniony raport okresowy",
        FLAG_FUND_EXIT => "Wyjście z akcjonariatu",
        FLAG_SCORE_DETERIORATION => "Pogorszenie kondycji",
        FLAG_AUDITOR => "Zastrzeżenia audytora",
        FLAG_SHORT_SPIKE => "Skok krótkiej sprzedaży",
        _ => "Sygnał ostrzegawczy",
    }
}

fn company_ticker(connection: &Connection, company_id: &str) -> StorageResult<String> {
    connection
        .query_row(
            "SELECT qualified_ticker FROM companies WHERE id = ?1",
            [company_id],
            |row| row.get::<_, String>(0),
        )
        .optional()
        .map(|value| value.unwrap_or_else(|| company_id.to_owned()))
        .map_err(StorageError::from)
}

/// `today` (`YYYY-MM-DD`) minus `days`, same format; on a parse failure returns
/// `today` unchanged (a malformed anchor only narrows a window, never panics).
fn date_minus_days(today: &str, days: i64) -> String {
    let fmt = time::macros::format_description!("[year]-[month]-[day]");
    match time::Date::parse(today, &fmt) {
        Ok(date) => date
            .saturating_sub(time::Duration::days(days))
            .format(&fmt)
            .unwrap_or_else(|_| today.to_owned()),
        Err(_) => today.to_owned(),
    }
}

fn now_iso() -> String {
    time::OffsetDateTime::now_utc()
        .format(&time::format_description::well_known::Rfc3339)
        .unwrap_or_else(|_| "1970-01-01T00:00:00Z".to_owned())
}

fn today_iso() -> String {
    now_iso().chars().take(10).collect()
}

// ============================================================================
// Store
// ============================================================================

/// Red-flags domain store (ADR 0083). Reached via `AppState::red_flags()`.
#[derive(Clone)]
pub struct RedFlagStore {
    db: Database,
}

impl RedFlagStore {
    pub(super) fn new(db: Database) -> Self {
        Self { db }
    }

    /// The computed per-company panel state. The short-spike window is anchored to
    /// the real UTC date.
    pub fn red_flags_view(&self, company_id: &str) -> StorageResult<RedFlagsView> {
        let connection = self.db.checkout()?;
        load_red_flags_view(&connection, company_id, &today_iso())
    }

    /// Acknowledge a flag (idempotent) and return the company's refreshed view —
    /// the acked flag has moved from `active` to `history`. An unparseable id
    /// leaves the store untouched and yields an empty view.
    pub fn acknowledge(&self, flag_id: &str) -> StorageResult<RedFlagsView> {
        let connection = self.db.checkout()?;
        acknowledge(&connection, flag_id)?;
        match parse_flag_id(flag_id) {
            Some((_, company_id)) => load_red_flags_view(&connection, &company_id, &today_iso()),
            None => Ok(RedFlagsView {
                active: Vec::new(),
                history: Vec::new(),
            }),
        }
    }

    /// Detect `report_delay`s across all companies (refresh-completion seam).
    pub fn detect_report_delays(&self) -> StorageResult<usize> {
        let connection = self.db.checkout()?;
        detect_report_delays(&connection, &today_iso())
    }

    /// Detect `fund_exit`s for one company (ownership-ingest seam).
    pub fn detect_fund_exits(&self, company_id: &str) -> StorageResult<usize> {
        let connection = self.db.checkout()?;
        detect_fund_exits(&connection, company_id)
    }

    /// Detect `score_deterioration` for one company (fact-confirmation seam).
    pub fn detect_score_deterioration(&self, company_id: &str) -> StorageResult<usize> {
        let connection = self.db.checkout()?;
        detect_score_deterioration(&connection, company_id, &today_iso())
    }
}
