//! Morning briefing (ADR 0068 decision 4, plan v0.54-attention-routing-briefing §T5).
//!
//! A daily/on-demand briefing has two layers:
//!
//! 1. A **deterministic composed item list** — new confirmed signals + completed
//!    autopilot runs + fired attention events whose DOMAIN date is strictly after
//!    the previous briefing's boundary, plus the current claims-due and
//!    upcoming-report snapshot. Ordered by domain date (NEVER `created_at`). This
//!    layer is a pure function ([`compose_briefing`]) over reads gathered by the
//!    job, so it is snapshot-stable and unit-testable offline.
//! 2. (removed) The AI narrative half — retired with the in-app AI layer
//!    (ADR 0084 decision 5; its three columns are dropped by migration 0102).
//!    contract (capability `morning_briefing`, ADR 0060). With no provider
//!    configured the briefing still persists as the structured list (narrative
//!    absent) — never blocked, never an error. A narrative is stored ONLY when
//!    every citation it references resolves to a composed item
//!    ([`build_briefing_narrative`]); an uncited/unknown-citation narrative is
//!    rejected and the briefing is stored without it (tripwire, ADR 0068).

use rusqlite::{params, Connection};
use serde::Serialize;

use super::database::Database;
use super::*;

/// Composed item-type tags (also the persisted `item_type` and the wire union).
pub const BRIEFING_ITEM_SIGNAL: &str = "signal";
pub const BRIEFING_ITEM_AUTOPILOT_RUN: &str = "autopilot_run";
pub const BRIEFING_ITEM_CLAIM_DUE: &str = "claim_due";
pub const BRIEFING_ITEM_REPORT_DATE: &str = "report_date";
pub const BRIEFING_ITEM_ATTENTION_EVENT: &str = "attention_event";

/// Terminal autopilot-run statuses that count as "completed" for the briefing
/// (a failed run is not a result worth surfacing as "what changed").
const AUTOPILOT_COMPLETED_STATUSES: [&str; 2] = ["succeeded", "partial"];

// ---------------------------------------------------------------------------
// Wire types (persisted read model)
// ---------------------------------------------------------------------------

/// A persisted, composed morning briefing plus its items (read model).
#[derive(Debug, Clone, PartialEq, Serialize)]
#[cfg_attr(feature = "ts-export", derive(ts_rs::TS))]
#[cfg_attr(
    feature = "ts-export",
    ts(export, export_to = "../../src/api/generated/")
)]
#[serde(rename_all = "camelCase")]
pub struct MorningBriefing {
    pub id: String,
    pub composed_at: String,
    /// Domain-date lower bound the composer used (`YYYY-MM-DD`; `''` on the
    /// first-ever briefing = everything).
    pub since: String,
    pub language: Option<String>,
    pub created_at: String,
    pub items: Vec<MorningBriefingItem>,
}

/// One persisted composed item — both the structured-list row the Today card
/// renders and the citeable evidence the narrative resolves against.
#[derive(Debug, Clone, PartialEq, Serialize)]
#[cfg_attr(feature = "ts-export", derive(ts_rs::TS))]
#[cfg_attr(
    feature = "ts-export",
    ts(export, export_to = "../../src/api/generated/")
)]
#[serde(rename_all = "camelCase")]
pub struct MorningBriefingItem {
    pub id: String,
    pub briefing_id: String,
    pub position: i64,
    #[cfg_attr(
        feature = "ts-export",
        ts(
            type = "\"signal\" | \"autopilot_run\" | \"claim_due\" | \"report_date\" | \"attention_event\""
        )
    )]
    pub item_type: String,
    pub company_id: String,
    pub domain_date: String,
    pub citation_key: String,
    pub evidence_type: String,
    pub evidence_ref: String,
    pub title: String,
    pub detail: Option<String>,
    pub created_at: String,
}

// ---------------------------------------------------------------------------
// Deterministic composer (pure — no clock, no DB)
// ---------------------------------------------------------------------------

/// Raw reads the composer consumes, gathered by the job from the domain stores.
/// Keeping the composer pure over this struct makes it snapshot-stable and
/// unit-testable offline.
#[derive(Debug, Default)]
pub struct BriefingSources {
    pub signals: Vec<CompanySignal>,
    pub autopilot_runs: Vec<AutopilotRun>,
    /// Currently due + overdue claims (flattened), a state snapshot — not
    /// bounded by `since` (a due claim stays due until the user acts).
    pub claims_due: Vec<ManagementClaim>,
    /// Upcoming report dates (state snapshot, not `since`-bounded).
    pub upcoming_reports: Vec<ReportSeasonEntry>,
    pub attention_events: Vec<AttentionEvent>,
}

/// One composed, citeable briefing item before persistence.
#[derive(Debug, Clone, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ComposedBriefingItem {
    pub item_type: &'static str,
    pub company_id: String,
    pub domain_date: String,
    pub citation_key: String,
    pub evidence_type: String,
    pub evidence_ref: String,
    pub title: String,
    pub detail: Option<String>,
}

/// The deterministic output of [`compose_briefing`]: `since` echoed back for
/// persistence plus the ordered item list (`citation_key` = `b{position+1}`).
#[derive(Debug, Clone, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ComposedBriefing {
    pub since: String,
    pub items: Vec<ComposedBriefingItem>,
}

/// First 10 chars of an ISO datetime = its `YYYY-MM-DD` domain date.
fn date10(value: &str) -> String {
    value.chars().take(10).collect()
}

/// Stable secondary sort rank per item type (primary sort is domain date).
fn item_type_rank(item_type: &str) -> u8 {
    match item_type {
        BRIEFING_ITEM_SIGNAL => 0,
        BRIEFING_ITEM_ATTENTION_EVENT => 1,
        BRIEFING_ITEM_AUTOPILOT_RUN => 2,
        BRIEFING_ITEM_CLAIM_DUE => 3,
        BRIEFING_ITEM_REPORT_DATE => 4,
        _ => 5,
    }
}

/// Compose the deterministic briefing item list. `since` is the previous
/// briefing's domain-date boundary (`''` = first ever); `today` is the compose
/// date, used only as the fallback domain date for undated items. Event-like
/// items (signals, completed runs, fired alerts) are included only when their
/// domain date is **strictly after** `since`; claims-due and upcoming reports are
/// a current-state snapshot. Items are deduped to at most one per
/// (company, type, evidence-ref) — keeping the newest by domain date (ADR 0087
/// dec. 1) — then ordered by domain date, then a stable (type, company,
/// evidence-ref) tiebreak — never `created_at`. `title`/`detail` carry only
/// verbatim source data or typed codes/tokens, never composed prose (dec. 4).
pub fn compose_briefing(sources: &BriefingSources, since: &str, today: &str) -> ComposedBriefing {
    let since10 = date10(since);
    let today10 = date10(today);
    let mut items: Vec<ComposedBriefingItem> = Vec::new();

    // 1. New confirmed signals (domain date = signal_date).
    for signal in &sources.signals {
        if signal.status != "confirmed" {
            continue;
        }
        let date = signal
            .signal_date
            .as_deref()
            .map(date10)
            .unwrap_or_else(|| today10.clone());
        if date <= since10 {
            continue;
        }
        items.push(ComposedBriefingItem {
            item_type: BRIEFING_ITEM_SIGNAL,
            company_id: signal.company_id.clone(),
            domain_date: date,
            citation_key: String::new(),
            evidence_type: "company_signal".to_owned(),
            evidence_ref: signal.id.clone(),
            title: signal.title.clone(),
            // detail = the raw category CODE (typed); the frontend translates it
            // via its signal-category display formatter (ADR 0087 dec. 4).
            detail: Some(signal.category.clone()),
        });
    }

    // 2. Fired attention events (domain date = fired_at), non-dismissed.
    for event in &sources.attention_events {
        if event.dismissed {
            continue;
        }
        let date = date10(&event.fired_at);
        if date <= since10 {
            continue;
        }
        items.push(ComposedBriefingItem {
            item_type: BRIEFING_ITEM_ATTENTION_EVENT,
            company_id: event.company_id.clone(),
            domain_date: date,
            citation_key: String::new(),
            evidence_type: "attention_event".to_owned(),
            evidence_ref: event.id.clone(),
            // Typed codes only (ADR 0087 dec. 4): title = trigger_type code,
            // detail = evidence_type code. The frontend maps both to localized
            // labels — the backend never composes the "Alert fired: a · b" prose.
            title: event.trigger_type.clone(),
            detail: Some(event.evidence_type.clone()),
        });
    }

    // 3. Completed autopilot runs (domain date = updated_at).
    for run in &sources.autopilot_runs {
        if !AUTOPILOT_COMPLETED_STATUSES.contains(&run.status.as_str()) {
            continue;
        }
        let date = date10(&run.updated_at);
        if date <= since10 {
            continue;
        }
        items.push(ComposedBriefingItem {
            item_type: BRIEFING_ITEM_AUTOPILOT_RUN,
            company_id: run.company_id.clone(),
            domain_date: date,
            citation_key: String::new(),
            evidence_type: "autopilot_run".to_owned(),
            evidence_ref: run.id.clone(),
            // title = the run's summary_text AS STORED (a typed token stream for
            // new runs; legacy prose rows pass through — the frontend reads both
            // tolerantly). The `report_processed` fallback is itself a typed token
            // that renders "New report processed." — never English prose here.
            title: run
                .summary_text
                .clone()
                .unwrap_or_else(|| "report_processed".to_owned()),
            detail: Some(run.status.clone()),
        });
    }

    // 4. Claims due/overdue (state snapshot; domain date = made_at or today).
    for claim in &sources.claims_due {
        let date = claim
            .made_at
            .as_deref()
            .map(date10)
            .unwrap_or_else(|| today10.clone());
        // detail = a typed due token `due:<period_type>:<fiscal_year>` (ADR 0087
        // dec. 4); the frontend parses and localizes it (and tolerates the legacy
        // "Due <period> <year>" prose on older stored rows).
        let detail = match (claim.due_period_type.as_deref(), claim.due_fiscal_year) {
            (Some(period), Some(year)) => Some(format!("due:{period}:{year}")),
            _ => None,
        };
        items.push(ComposedBriefingItem {
            item_type: BRIEFING_ITEM_CLAIM_DUE,
            company_id: claim.company_id.clone(),
            domain_date: date,
            citation_key: String::new(),
            evidence_type: "claim".to_owned(),
            evidence_ref: claim.id.clone(),
            title: claim.statement.clone(),
            detail,
        });
    }

    // 5. Upcoming report dates (state snapshot; domain date = event_date).
    for entry in &sources.upcoming_reports {
        items.push(ComposedBriefingItem {
            item_type: BRIEFING_ITEM_REPORT_DATE,
            company_id: entry.company_id.clone(),
            domain_date: date10(&entry.event_date),
            citation_key: String::new(),
            evidence_type: "report_date".to_owned(),
            evidence_ref: format!("{}:{}", entry.company_id, entry.event_key),
            title: entry.title.clone(),
            detail: Some(entry.qualified_ticker.clone()),
        });
    }

    // Dedup (ADR 0087 dec. 1): at most one item per (company, type, evidence-ref).
    // A gather that surfaces the same evidence twice (e.g. a join fan-out, or the
    // same condition stated per row) collapses to a single item, keeping the
    // newest by domain date. Runs before ordering; the sort below re-establishes
    // deterministic order regardless of collapse.
    let mut deduped: Vec<ComposedBriefingItem> = Vec::with_capacity(items.len());
    let mut index_by_key: std::collections::HashMap<(String, &'static str, String), usize> =
        std::collections::HashMap::new();
    for item in items {
        let key = (
            item.company_id.clone(),
            item.item_type,
            item.evidence_ref.clone(),
        );
        match index_by_key.get(&key) {
            Some(&existing) if item.domain_date > deduped[existing].domain_date => {
                deduped[existing] = item;
            }
            Some(_) => {}
            None => {
                index_by_key.insert(key, deduped.len());
                deduped.push(item);
            }
        }
    }
    let mut items = deduped;

    // Order by domain date, then a stable (type, company, evidence-ref) tiebreak.
    items.sort_by(|a, b| {
        a.domain_date
            .cmp(&b.domain_date)
            .then(item_type_rank(a.item_type).cmp(&item_type_rank(b.item_type)))
            .then(a.company_id.cmp(&b.company_id))
            .then(a.evidence_ref.cmp(&b.evidence_ref))
    });
    for (index, item) in items.iter_mut().enumerate() {
        item.citation_key = format!("b{}", index + 1);
    }

    ComposedBriefing {
        since: since.to_owned(),
        items,
    }
}

// ---------------------------------------------------------------------------
// Persistence
// ---------------------------------------------------------------------------

fn next_morning_briefing_id(connection: &Connection) -> StorageResult<String> {
    let count: i64 = connection.query_row("SELECT COUNT(*) FROM morning_briefings", [], |row| {
        row.get(0)
    })?;
    Ok(format!("morning_briefing_{:04}", count + 1))
}

pub(super) fn insert_morning_briefing(
    connection: &Connection,
    composed: &ComposedBriefing,
) -> StorageResult<MorningBriefing> {
    let id = next_morning_briefing_id(connection)?;
    let transaction = connection.unchecked_transaction()?;
    transaction.execute(
        "INSERT INTO morning_briefings (id, since) VALUES (?1, ?2)",
        params![id, composed.since],
    )?;
    for (index, item) in composed.items.iter().enumerate() {
        transaction.execute(
            "
            INSERT INTO morning_briefing_items (
                id, briefing_id, position, item_type, company_id, domain_date,
                citation_key, evidence_type, evidence_ref, title, detail
            ) VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11)
            ",
            params![
                format!("{id}_{}", item.citation_key),
                id,
                index as i64,
                item.item_type,
                item.company_id,
                item.domain_date,
                item.citation_key,
                item.evidence_type,
                item.evidence_ref,
                item.title,
                item.detail,
            ],
        )?;
    }
    transaction.commit()?;
    get_morning_briefing(connection, &id)
}

pub(super) fn get_morning_briefing(
    connection: &Connection,
    id: &str,
) -> StorageResult<MorningBriefing> {
    let mut briefing = connection.query_row(
        "
        SELECT id, composed_at, since, language, created_at
        FROM morning_briefings WHERE id = ?1
        ",
        [id],
        morning_briefing_from_row,
    )?;
    briefing.items = list_morning_briefing_items(connection, id)?;
    Ok(briefing)
}

pub(super) fn latest_morning_briefing(
    connection: &Connection,
) -> StorageResult<Option<MorningBriefing>> {
    let id: Option<String> = connection
        .query_row(
            "SELECT id FROM morning_briefings ORDER BY composed_at DESC, id DESC LIMIT 1",
            [],
            |row| row.get(0),
        )
        .optional()?;
    match id {
        Some(id) => Ok(Some(get_morning_briefing(connection, &id)?)),
        None => Ok(None),
    }
}

fn list_morning_briefing_items(
    connection: &Connection,
    briefing_id: &str,
) -> StorageResult<Vec<MorningBriefingItem>> {
    let mut statement = connection.prepare(
        "
        SELECT id, briefing_id, position, item_type, company_id, domain_date,
               citation_key, evidence_type, evidence_ref, title, detail, created_at
        FROM morning_briefing_items
        WHERE briefing_id = ?1
        ORDER BY position
        ",
    )?;
    let rows = statement.query_map([briefing_id], morning_briefing_item_from_row)?;
    rows.collect::<Result<Vec<_>, _>>()
        .map_err(StorageError::from)
}

fn morning_briefing_from_row(row: &rusqlite::Row<'_>) -> rusqlite::Result<MorningBriefing> {
    Ok(MorningBriefing {
        id: row.get(0)?,
        composed_at: row.get(1)?,
        since: row.get(2)?,
        language: row.get(3)?,
        created_at: row.get(4)?,
        items: Vec::new(),
    })
}

fn morning_briefing_item_from_row(
    row: &rusqlite::Row<'_>,
) -> rusqlite::Result<MorningBriefingItem> {
    Ok(MorningBriefingItem {
        id: row.get(0)?,
        briefing_id: row.get(1)?,
        position: row.get(2)?,
        item_type: row.get(3)?,
        company_id: row.get(4)?,
        domain_date: row.get(5)?,
        citation_key: row.get(6)?,
        evidence_type: row.get(7)?,
        evidence_ref: row.get(8)?,
        title: row.get(9)?,
        detail: row.get(10)?,
        created_at: row.get(11)?,
    })
}

/// Domain date (`YYYY-MM-DD`) of the latest briefing's `composed_at`, or `''`
/// when no briefing exists yet — the `since` boundary for the next compose.
pub(super) fn latest_since_boundary(connection: &Connection) -> StorageResult<String> {
    let composed_at: Option<String> = connection
        .query_row(
            "SELECT composed_at FROM morning_briefings ORDER BY composed_at DESC, id DESC LIMIT 1",
            [],
            |row| row.get(0),
        )
        .optional()?;
    Ok(composed_at.map(|value| date10(&value)).unwrap_or_default())
}

/// Whether a briefing was already composed on `date` (`YYYY-MM-DD`). The daily
/// auto-trigger uses this to stay idempotent per day.
pub(super) fn briefing_exists_on(connection: &Connection, date: &str) -> StorageResult<bool> {
    let exists: bool = connection.query_row(
        "SELECT EXISTS(SELECT 1 FROM morning_briefings WHERE substr(composed_at, 1, 10) = ?1)",
        [date],
        |row| row.get(0),
    )?;
    Ok(exists)
}

// ---------------------------------------------------------------------------
// Store
// ---------------------------------------------------------------------------

/// Morning-briefing domain store (Architecture v2 / ADR 0050). Reach it via
/// `AppState::morning_briefings()`.
#[derive(Clone)]
pub struct MorningBriefingStore {
    db: Database,
}

impl MorningBriefingStore {
    pub(super) fn new(db: Database) -> Self {
        Self { db }
    }

    pub fn insert_morning_briefing(
        &self,
        composed: &ComposedBriefing,
    ) -> StorageResult<MorningBriefing> {
        let connection = self.db.checkout()?;
        insert_morning_briefing(&connection, composed)
    }

    pub fn get_morning_briefing(&self, id: &str) -> StorageResult<MorningBriefing> {
        let connection = self.db.checkout()?;
        get_morning_briefing(&connection, id)
    }

    pub fn latest_morning_briefing(&self) -> StorageResult<Option<MorningBriefing>> {
        let connection = self.db.checkout()?;
        latest_morning_briefing(&connection)
    }

    pub fn latest_since_boundary(&self) -> StorageResult<String> {
        let connection = self.db.checkout()?;
        latest_since_boundary(&connection)
    }

    pub fn briefing_exists_on(&self, date: &str) -> StorageResult<bool> {
        let connection = self.db.checkout()?;
        briefing_exists_on(&connection, date)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::storage::open_in_memory_database;

    fn signal(id: &str, company: &str, date: Option<&str>, status: &str) -> CompanySignal {
        CompanySignal {
            id: id.to_owned(),
            company_id: company.to_owned(),
            company: company.to_owned(),
            company_name: company.to_owned(),
            feed_item_id: format!("feed_{id}"),
            category: "profit_warning".to_owned(),
            category_display_name: "Profit warning".to_owned(),
            confidence: 1.0,
            classified_by: "rule".to_owned(),
            status: status.to_owned(),
            signal_date: date.map(str::to_owned),
            provider_id: None,
            model_id: None,
            derived_event_id: None,
            title: format!("Signal {id}"),
            source_url: "https://example.test".to_owned(),
            created_at: "2026-07-01T00:00:00Z".to_owned(),
            updated_at: "2026-07-01T00:00:00Z".to_owned(),
        }
    }

    fn run(id: &str, company: &str, updated: &str, status: &str) -> AutopilotRun {
        AutopilotRun {
            id: id.to_owned(),
            company_id: company.to_owned(),
            report_document_id: "doc".to_owned(),
            trigger: "detection".to_owned(),
            mode: "autopilot".to_owned(),
            sweep_id: None,
            status: status.to_owned(),
            stage: "finalize_notify".to_owned(),
            summary_text: Some(format!("Run {id} summary")),
            kpi_delta_json: None,
            report_diff_ref: None,
            cross_refs_json: None,
            produced_fact_ids: Vec::new(),
            notification_state: "unnotified".to_owned(),
            last_error: None,
            created_at: "2026-07-01T00:00:00Z".to_owned(),
            updated_at: updated.to_owned(),
            severity: crate::storage::severity_for_autopilot_run(status),
            report_document_title: None,
        }
    }

    fn attention(id: &str, company: &str, fired: &str, dismissed: bool) -> AttentionEvent {
        AttentionEvent {
            id: id.to_owned(),
            rule_id: Some("rule_1".to_owned()),
            trigger_type: "signal_category".to_owned(),
            company_id: company.to_owned(),
            evidence_type: "company_signal".to_owned(),
            evidence_ref: "sig_ref".to_owned(),
            fired_at: fired.to_owned(),
            seen: false,
            dismissed,
            severity: crate::storage::severity_for_attention_event("signal_category", None),
            evidence_title: None,
            evidence_detail: None,
        }
    }

    fn claim(id: &str, company: &str, made: Option<&str>) -> ManagementClaim {
        ManagementClaim {
            id: id.to_owned(),
            company_id: company.to_owned(),
            statement: format!("Claim {id}"),
            body: String::new(),
            body_format: "markdown".to_owned(),
            made_at: made.map(str::to_owned),
            source_period_id: None,
            due_fiscal_year: Some(2026),
            due_period_type: Some("Q2".to_owned()),
            status: "pending".to_owned(),
            source_evidence_type: "manual".to_owned(),
            source_evidence_id: None,
            target_metric_key: None,
            target_comparator: None,
            target_value_numeric: None,
            target_unit: None,
            verifying_fact_id: None,
            revises_claim_id: None,
            created_at: "2026-07-01T00:00:00Z".to_owned(),
            updated_at: "2026-07-01T00:00:00Z".to_owned(),
        }
    }

    fn report(company: &str, event_key: &str, date: &str) -> ReportSeasonEntry {
        ReportSeasonEntry {
            company_id: company.to_owned(),
            qualified_ticker: format!("GPW:{company}"),
            display_name: company.to_owned(),
            event_key: event_key.to_owned(),
            event_date: date.to_owned(),
            event_time: None,
            title: format!("{event_key} report"),
            preparation_status: "not_started".to_owned(),
        }
    }

    fn full_sources() -> BriefingSources {
        BriefingSources {
            signals: vec![
                signal("sig_new", "acme", Some("2026-07-14"), "confirmed"),
                signal("sig_old", "acme", Some("2026-07-01"), "confirmed"),
                signal("sig_proposed", "acme", Some("2026-07-14"), "proposed"),
            ],
            autopilot_runs: vec![
                run("run_ok", "acme", "2026-07-13T09:00:00Z", "succeeded"),
                run("run_fail", "acme", "2026-07-14T09:00:00Z", "failed"),
                run("run_old", "acme", "2026-07-01T09:00:00Z", "succeeded"),
            ],
            claims_due: vec![claim("claim_1", "beta", Some("2026-06-01"))],
            upcoming_reports: vec![report("beta", "q2_2026", "2026-07-20")],
            attention_events: vec![
                attention("attn_new", "acme", "2026-07-12T00:00:00Z", false),
                attention("attn_dismissed", "acme", "2026-07-14T00:00:00Z", true),
            ],
        }
    }

    #[test]
    fn composer_orders_by_domain_date_and_filters_since() {
        // Golden ordering pin (stable, deterministic). `since = 2026-07-10` keeps
        // only event items strictly after that date; proposed signals, failed runs,
        // dismissed events are excluded; claims-due + upcoming reports are always
        // included (state snapshot). Ordering is by domain date, never created_at.
        let composed = compose_briefing(&full_sources(), "2026-07-10", "2026-07-15");
        insta::assert_debug_snapshot!("composed_morning_briefing", composed);
    }

    #[test]
    fn composer_excludes_items_at_or_before_since() {
        let composed = compose_briefing(&full_sources(), "2026-07-10", "2026-07-15");
        let refs: Vec<&str> = composed
            .items
            .iter()
            .map(|item| item.evidence_ref.as_str())
            .collect();
        assert!(refs.contains(&"sig_new"), "new signal included");
        assert!(
            !refs.contains(&"sig_old"),
            "signal on/before since excluded"
        );
        assert!(!refs.contains(&"sig_proposed"), "proposed signal excluded");
        assert!(!refs.contains(&"run_fail"), "failed run excluded");
        assert!(!refs.contains(&"run_old"), "old run excluded");
        assert!(
            !refs.contains(&"attn_dismissed"),
            "dismissed event excluded"
        );
    }

    #[test]
    fn since_boundary_yields_no_duplicates_on_the_next_run() {
        // A signal dated 2026-07-14 appears in a briefing with since < that date,
        // but NOT in the next briefing whose since is that date's day — no dup.
        let sources = BriefingSources {
            signals: vec![signal("sig_a", "acme", Some("2026-07-14"), "confirmed")],
            ..BriefingSources::default()
        };
        let first = compose_briefing(&sources, "2026-07-13", "2026-07-14");
        assert_eq!(
            first.items.len(),
            1,
            "first briefing includes the new signal"
        );

        let second = compose_briefing(&sources, "2026-07-14", "2026-07-15");
        assert!(
            second.items.is_empty(),
            "the same signal is not re-briefed once since has advanced past its date"
        );
    }

    #[test]
    fn citation_keys_are_sequential() {
        let composed = compose_briefing(&full_sources(), "2026-07-10", "2026-07-15");
        for (index, item) in composed.items.iter().enumerate() {
            assert_eq!(item.citation_key, format!("b{}", index + 1));
        }
    }

    #[test]
    fn persists_briefing_with_items_and_no_narrative() {
        // No-provider path at the persistence layer: a composed briefing with items
        // is stored and reads back intact — the
        // briefing completes as a structured list, never blocked.
        let state = crate::storage::AppState::new(open_in_memory_database().expect("db"));
        let composed = compose_briefing(&full_sources(), "2026-07-10", "2026-07-15");
        assert!(!composed.items.is_empty());

        let stored = state
            .morning_briefings()
            .insert_morning_briefing(&composed)
            .expect("insert briefing");
        assert_eq!(stored.items.len(), composed.items.len());
        assert_eq!(stored.since, "2026-07-10");

        let latest = state
            .morning_briefings()
            .latest_morning_briefing()
            .expect("latest")
            .expect("a briefing exists");
        assert_eq!(latest.id, stored.id);
        assert_eq!(latest.items.first().unwrap().citation_key, "b1");
    }

    #[test]
    fn briefing_items_carry_typed_payloads_never_composed_prose() {
        // Guardrail (ADR 0087 dec. 4, guardrail-harvest of card abd456e): the
        // composer writes ONLY verbatim source data or typed codes/tokens into
        // `title`/`detail` — NEVER composed English prose. The frontend
        // (`briefingItemText.ts`) is the sole place codes become localized copy.
        // Adding a builder that bakes an English sentence into a persisted column
        // is the defect class this pins shut.
        let composed = compose_briefing(&full_sources(), "2026-07-10", "2026-07-15");
        let by_type = |t: &str| {
            composed
                .items
                .iter()
                .find(|item| item.item_type == t)
                .unwrap_or_else(|| panic!("expected one {t} item"))
        };

        let signal = by_type(BRIEFING_ITEM_SIGNAL);
        assert_eq!(
            signal.title, "Signal sig_new",
            "signal title = its own title"
        );
        assert_eq!(
            signal.detail.as_deref(),
            Some("profit_warning"),
            "signal detail = the raw category CODE, never its display name"
        );

        let attn = by_type(BRIEFING_ITEM_ATTENTION_EVENT);
        assert_eq!(
            attn.title, "signal_category",
            "attention title = trigger_type code"
        );
        assert_eq!(
            attn.detail.as_deref(),
            Some("company_signal"),
            "attention detail = evidence_type code"
        );

        let run = by_type(BRIEFING_ITEM_AUTOPILOT_RUN);
        assert_eq!(
            run.detail.as_deref(),
            Some("succeeded"),
            "autopilot detail = status code"
        );

        let claim = by_type(BRIEFING_ITEM_CLAIM_DUE);
        assert_eq!(claim.title, "Claim claim_1", "claim title = its statement");
        assert_eq!(
            claim.detail.as_deref(),
            Some("due:Q2:2026"),
            "claim detail = typed due token due:<period>:<year>"
        );

        let report = by_type(BRIEFING_ITEM_REPORT_DATE);
        assert_eq!(
            report.detail.as_deref(),
            Some("GPW:beta"),
            "report detail = qualified ticker (source data)"
        );

        // The class assertion: no composed English prose in any column.
        for item in &composed.items {
            let blob = format!("{} {}", item.title, item.detail.clone().unwrap_or_default());
            assert!(
                !blob.contains("Alert fired"),
                "no 'Alert fired:' prose: {blob}"
            );
            assert!(
                !blob.contains("Due Q2 2026"),
                "no 'Due <p> <y>' prose: {blob}"
            );
            assert!(!blob.contains(" · "), "no composed ' · ' join: {blob}");
        }
    }

    #[test]
    fn dedup_collapses_repeated_evidence_keeps_newest() {
        // ADR 0087 dec. 1: at most one item per (company, type, evidence_ref).
        // A gather that returns the same run twice (e.g. a join fan-out) collapses
        // to a single row, keeping the newest by domain_date. Fails pre-dedup.
        let sources = BriefingSources {
            autopilot_runs: vec![
                run("run_dup", "acme", "2026-07-13T09:00:00Z", "succeeded"),
                run("run_dup", "acme", "2026-07-14T09:00:00Z", "succeeded"),
            ],
            ..BriefingSources::default()
        };
        let composed = compose_briefing(&sources, "2026-07-10", "2026-07-15");
        let dup: Vec<&ComposedBriefingItem> = composed
            .items
            .iter()
            .filter(|item| item.evidence_ref == "run_dup")
            .collect();
        assert_eq!(dup.len(), 1, "duplicate evidence collapses to one item");
        assert_eq!(
            dup[0].domain_date, "2026-07-14",
            "dedup keeps the newest by domain date"
        );
    }

    #[test]
    fn since_boundary_and_exists_track_the_latest_briefing() {
        let state = crate::storage::AppState::new(open_in_memory_database().expect("db"));
        assert_eq!(
            state.morning_briefings().latest_since_boundary().unwrap(),
            "",
            "no briefing yet => empty since (include everything)"
        );

        let composed = ComposedBriefing {
            since: "".to_owned(),
            items: Vec::new(),
        };
        let stored = state
            .morning_briefings()
            .insert_morning_briefing(&composed)
            .expect("insert");
        let today = &stored.composed_at[..10];
        assert!(state.morning_briefings().briefing_exists_on(today).unwrap());
        assert_eq!(
            state.morning_briefings().latest_since_boundary().unwrap(),
            today
        );
    }
}
