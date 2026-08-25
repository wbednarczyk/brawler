//! Dziś v2 composed read model (F2 S1; ADR 0106 dec. 3;
//! `docs/plans/frontend-v2-f2.md` decisions 1-2). One command replaces the
//! frontend's app-root pulse hook: a FLAT `items[]` (day-bucketing is a
//! frontend concern, S3) built from four independent sections — feed
//! (filings + flat media items, one row per matched company — FIX WAVE A
//! finding 8: no backend day-clustering, the frontend owns the local
//! display-day boundary), non-arrival (shared `report_delay` predicate),
//! calendar, autopilot runs — plus a bulk claims-to-verify list across every
//! tracked company (kills `useTodayPulse`'s N-per-company IPC loop). Each
//! section degrades independently into `sectionErrors` (typed, closed enum)
//! instead of failing the whole read (Partial state, ADR 0081 experience
//! contract). Offloaded off the UI thread via `spawn_blocking`, mirroring
//! [`crate::commands::company_context`]. `get_today_view` itself is
//! read-only; `mark_today_visited` (F2 S2, plan decision 4) is the one write
//! — it stamps the visit anchor `get_today_view` reads, called by the
//! frontend after a successful render, never on unmount.

use serde::Serialize;

use crate::app_state::AppState;
use crate::storage::{
    AutopilotRun, ListAutopilotRunsInput, ManagementClaim, PresentationKind, TodayFeedRow,
};

/// Server-side clamp on the requested window (plan decision 1) — a hand-typed
/// or stale frontend value can never fetch an unbounded history.
const DAY_LIMIT_MIN: i64 = 1;
const DAY_LIMIT_MAX: i64 = 7;

/// Bounded query: plenty of headroom for a `dayLimit`-week unread-run window.
const MAX_AUTOPILOT_RUNS: i64 = 200;

/// One flat Dziś item — a tagged union so the frontend switches on `kind`
/// (plan decision 1, "PŁASKA lista items[]").
#[derive(Debug, Clone, Serialize)]
#[cfg_attr(feature = "ts-export", derive(ts_rs::TS))]
#[cfg_attr(
    feature = "ts-export",
    ts(export, export_to = "../../src/api/generated/")
)]
#[serde(tag = "kind", rename_all = "camelCase")]
pub enum TodayItem {
    /// One official filing (`Official report` feed item), 1:1 with its feed
    /// row — reuses the existing feed read-model's field names/shape (never
    /// invented), including [`PresentationKind`] (F1).
    #[serde(rename_all = "camelCase")]
    Filing {
        feed_item_id: String,
        company_id: String,
        qualified_ticker: String,
        title: String,
        published_at: String,
        read: bool,
        presentation_kind: PresentationKind,
    },
    /// One `Public media` feed item matched to one company — FLAT, one row
    /// per (media feed item × matched company); day-bucketing/clustering is a
    /// frontend concern (local display-day boundary, wave B — review finding
    /// 8: the backend previously pre-clustered by UTC day, which the
    /// frontend then mis-assigned across local midnight). An item matched to
    /// multiple companies yields multiple rows, one per company (membership
    /// preserved) — the join happens upstream in
    /// [`crate::storage::TodayStore::list_recent_feed_rows`].
    #[serde(rename_all = "camelCase")]
    MediaItem {
        feed_item_id: String,
        company_id: String,
        qualified_ticker: String,
        title: String,
        published_at: String,
        read: bool,
        source_name: String,
    },
    /// A tracked company's `periodic_report` event past its date with no
    /// witnessing report and no `report_delay` flag yet (plan decision 2) —
    /// the row vanishes exactly when the flag takes over (root-fed attention).
    #[serde(rename_all = "camelCase")]
    NonArrival {
        event_key: String,
        company_id: String,
        qualified_ticker: String,
        event_date: String,
        title: String,
    },
    /// An upcoming `company_events` row within `[today, today + dayLimit]`.
    #[serde(rename_all = "camelCase")]
    Calendar {
        event_key: String,
        event_date: String,
        event_type: String,
        title: String,
        company_id: String,
        qualified_ticker: String,
    },
    /// An unnotified autopilot run within the window — reuses the existing
    /// autopilot run listing shape verbatim (`AutopilotRun`, `storage::autopilot`).
    #[serde(rename_all = "camelCase")]
    AutopilotRun { run: Box<AutopilotRun> },
}

/// One pending claim awaiting verification, decorated with its company and
/// urgency bucket for the flat "DO WERYFIKACJI" list — the bulk counterpart of
/// `useTodayPulse`'s per-company `PulseClaim` (kills the N-per-company loop).
#[derive(Debug, Clone, Serialize)]
#[cfg_attr(feature = "ts-export", derive(ts_rs::TS))]
#[cfg_attr(
    feature = "ts-export",
    ts(export, export_to = "../../src/api/generated/")
)]
#[serde(rename_all = "camelCase")]
pub struct TodayClaim {
    pub claim: ManagementClaim,
    pub qualified_ticker: String,
    /// `due | overdue` — the two buckets `list_claims_to_verify` resurfaces
    /// (an `upcoming` claim carries no "act now" urgency for Dziś).
    #[cfg_attr(feature = "ts-export", ts(type = "\"due\" | \"overdue\""))]
    pub bucket: String,
}

/// A closed enum so a section failure can only ever mean "unavailable" — no
/// error-message string leaks into the read model (ADR 0081 typed Partial).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[cfg_attr(feature = "ts-export", derive(ts_rs::TS))]
#[cfg_attr(
    feature = "ts-export",
    ts(export, export_to = "../../src/api/generated/")
)]
#[serde(rename_all = "camelCase")]
pub enum SectionErrorKind {
    Unavailable,
}

/// Per-section degradation (plan decision 1): a storage error in one section
/// fills its slot instead of failing the whole command. `nonArrival` shares
/// the `calendar` slot — both read `company_events`/`red_flags` state.
/// `anchor` (FIX WAVE A finding 5) covers a KV read error on the visit
/// anchor — previously silently swallowed into "first visit"
/// (`unwrap_or(None)`), now surfaced instead of faked.
#[derive(Debug, Clone, Default, Serialize)]
#[cfg_attr(feature = "ts-export", derive(ts_rs::TS))]
#[cfg_attr(
    feature = "ts-export",
    ts(export, export_to = "../../src/api/generated/")
)]
#[serde(rename_all = "camelCase")]
pub struct TodaySectionErrors {
    #[serde(skip_serializing_if = "Option::is_none")]
    #[cfg_attr(feature = "ts-export", ts(optional))]
    pub feed: Option<SectionErrorKind>,
    #[serde(skip_serializing_if = "Option::is_none")]
    #[cfg_attr(feature = "ts-export", ts(optional))]
    pub calendar: Option<SectionErrorKind>,
    #[serde(skip_serializing_if = "Option::is_none")]
    #[cfg_attr(feature = "ts-export", ts(optional))]
    pub claims: Option<SectionErrorKind>,
    #[serde(skip_serializing_if = "Option::is_none")]
    #[cfg_attr(feature = "ts-export", ts(optional))]
    pub autopilot: Option<SectionErrorKind>,
    #[serde(skip_serializing_if = "Option::is_none")]
    #[cfg_attr(feature = "ts-export", ts(optional))]
    pub anchor: Option<SectionErrorKind>,
}

/// Counts of items newer than `previousVisitAt` (plan decision 1's delta
/// header). One count per matching row — `mediaCount` counts flat
/// `mediaItem` rows, one per (feed item × matched company).
#[derive(Debug, Clone, Copy, Default, Serialize)]
#[cfg_attr(feature = "ts-export", derive(ts_rs::TS))]
#[cfg_attr(
    feature = "ts-export",
    ts(export, export_to = "../../src/api/generated/")
)]
#[serde(rename_all = "camelCase")]
pub struct TodayDeltaSummary {
    /// `Official report` items WITH attachments (`PresentationKind::Report`).
    pub report_count: i64,
    /// `Official report` items with no attachments (a bare filing notice).
    pub filing_count: i64,
    pub media_count: i64,
}

/// Everything the Dziś v2 screen renders, composed in one read (ADR 0106 dec.
/// 3). `previousVisitAt` is read from KV here (S1); `mark_today_visited` (S2)
/// writes it — one source of truth, no duplicate anchor parameter.
#[derive(Debug, Clone, Serialize)]
#[cfg_attr(feature = "ts-export", derive(ts_rs::TS))]
#[cfg_attr(
    feature = "ts-export",
    ts(export, export_to = "../../src/api/generated/")
)]
#[serde(rename_all = "camelCase")]
pub struct TodayView {
    pub items: Vec<TodayItem>,
    pub to_verify: Vec<TodayClaim>,
    pub delta_summary: TodayDeltaSummary,
    pub previous_visit_at: Option<String>,
    pub section_errors: TodaySectionErrors,
}

fn now_iso() -> String {
    time::OffsetDateTime::now_utc()
        .format(&time::format_description::well_known::Rfc3339)
        .unwrap_or_else(|_| "1970-01-01T00:00:00Z".to_owned())
}

fn today_iso() -> String {
    now_iso().chars().take(10).collect()
}

/// `today` (`YYYY-MM-DD`) shifted by `days` (negative = past), same format;
/// an unparseable anchor returns `today` unchanged (only narrows a window,
/// never panics). Mirrors `storage::red_flags::date_minus_days`, duplicated
/// per-module like `today_iso` (established local-helper idiom, e.g.
/// `storage::attention`/`storage::red_flags`).
fn date_shift_days(today: &str, days: i64) -> String {
    let fmt = time::macros::format_description!("[year]-[month]-[day]");
    match time::Date::parse(today, &fmt) {
        Ok(date) => date
            .saturating_add(time::Duration::days(days))
            .format(&fmt)
            .unwrap_or_else(|_| today.to_owned()),
        Err(_) => today.to_owned(),
    }
}

/// Split raw joined feed rows into flat `Filing` items and flat `MediaItem`
/// rows (plan decision 3, review finding 8 — no clustering: the frontend
/// owns the local display-day boundary).
fn split_feed_rows(rows: Vec<TodayFeedRow>) -> (Vec<TodayItem>, Vec<TodayItem>) {
    let mut filings = Vec::new();
    let mut media = Vec::new();

    for row in rows {
        match row.item_type.as_str() {
            "Official report" => filings.push(TodayItem::Filing {
                feed_item_id: row.feed_item_id,
                company_id: row.company_id,
                qualified_ticker: row.qualified_ticker,
                title: row.title,
                published_at: row.published_at,
                read: row.read,
                presentation_kind: row.presentation_kind,
            }),
            "Public media" => media.push(TodayItem::MediaItem {
                feed_item_id: row.feed_item_id,
                company_id: row.company_id,
                qualified_ticker: row.qualified_ticker,
                title: row.title,
                published_at: row.published_at,
                read: row.read,
                source_name: row.source_name,
            }),
            _ => {}
        }
    }

    (filings, media)
}

/// The domain-date sort key for one item — never `created_at`
/// (data-model.md § Model principles).
fn today_item_sort_key(item: &TodayItem) -> &str {
    match item {
        TodayItem::Filing { published_at, .. } => published_at,
        TodayItem::MediaItem { published_at, .. } => published_at,
        TodayItem::NonArrival { event_date, .. } => event_date,
        TodayItem::Calendar { event_date, .. } => event_date,
        TodayItem::AutopilotRun { run } => run.created_at.as_str(),
    }
}

/// Compute the Dziś v2 read model (sync core, unit-testable). Infallible by
/// design: every storage-backed section catches its own error into
/// `sectionErrors` instead of failing the whole read (ADR 0081 Partial).
pub fn compute_today_view(state: &AppState, day_limit_raw: i64) -> TodayView {
    let day_limit = day_limit_raw.clamp(DAY_LIMIT_MIN, DAY_LIMIT_MAX);
    let today = today_iso();
    let since = date_shift_days(&today, -day_limit);
    let horizon = date_shift_days(&today, day_limit);

    let mut section_errors = TodaySectionErrors::default();
    // A KV read error must not masquerade as "first visit" (finding 5): the
    // old `.unwrap_or(None)` collapsed both cases to `None` indistinguishably.
    // A real error now surfaces via `sectionErrors.anchor` instead.
    let previous_visit_at = match state.settings().today_last_visit_at() {
        Ok(value) => value,
        Err(_) => {
            section_errors.anchor = Some(SectionErrorKind::Unavailable);
            None
        }
    };
    // "" sorts before every real ISO timestamp, so an absent anchor (first
    // visit, OR an anchor read error above) naturally counts every in-window
    // item as delta.
    let delta_cutoff = previous_visit_at.clone().unwrap_or_default();

    let mut items = Vec::new();
    let mut delta_summary = TodayDeltaSummary::default();

    match state.today().list_recent_feed_rows(&since) {
        Ok(rows) => {
            for row in &rows {
                if row.published_at.as_str() <= delta_cutoff.as_str() {
                    continue;
                }
                match (row.item_type.as_str(), row.presentation_kind) {
                    ("Official report", PresentationKind::Report) => {
                        delta_summary.report_count += 1
                    }
                    ("Official report", _) => delta_summary.filing_count += 1,
                    ("Public media", _) => delta_summary.media_count += 1,
                    _ => {}
                }
            }
            let (filings, media) = split_feed_rows(rows);
            items.extend(filings);
            items.extend(media);
        }
        Err(_) => section_errors.feed = Some(SectionErrorKind::Unavailable),
    }

    // Non-arrival shares the calendar slot (both company_events-backed).
    // Its (companyId, eventKey) pairs are tracked so the calendar section
    // below can exclude them (finding 7a) — a `periodic_report` event due
    // today would otherwise surface as BOTH a nonArrival row and a calendar
    // row for the same event.
    let mut non_arrival_failed = false;
    let mut non_arrival_keys: std::collections::HashSet<(String, String)> =
        std::collections::HashSet::new();
    match state.today().list_non_arrivals(&today) {
        Ok(candidates) => {
            for candidate in candidates {
                non_arrival_keys
                    .insert((candidate.company_id.clone(), candidate.event_key.clone()));
                items.push(TodayItem::NonArrival {
                    event_key: candidate.event_key,
                    company_id: candidate.company_id,
                    qualified_ticker: candidate.qualified_ticker,
                    event_date: candidate.event_date,
                    title: candidate.title,
                });
            }
        }
        Err(_) => non_arrival_failed = true,
    }

    // Date-bounded SQL path for the window (finding 7b) — replaces
    // `events::list_company_events`, which loads the whole table and filters
    // in process; narrower than that function's public contract on purpose.
    match state.today().list_events_in_window(&today, &horizon) {
        Ok(events) => items.extend(
            events
                .into_iter()
                .filter(|event| {
                    !non_arrival_keys.contains(&(event.company_id.clone(), event.id.clone()))
                })
                .map(|event| TodayItem::Calendar {
                    event_key: event.id,
                    event_date: event.event_date,
                    event_type: event.event_type,
                    title: event.title,
                    company_id: event.company_id,
                    qualified_ticker: event.company,
                }),
        ),
        Err(_) => section_errors.calendar = Some(SectionErrorKind::Unavailable),
    }
    if non_arrival_failed {
        section_errors.calendar = Some(SectionErrorKind::Unavailable);
    }

    // Bulk claims-to-verify across every tracked company (kills
    // `useTodayPulse`'s N-per-company IPC fan-out) — either the whole section
    // reads back or it's marked unavailable, never silently partial.
    let mut to_verify = Vec::new();
    match state.list_companies() {
        Ok(companies) => {
            let mut claims_failed = false;
            for company in &companies {
                match state.list_claims_to_verify(&company.id) {
                    Ok(claims) => {
                        let ticker = &company.qualified_ticker;
                        to_verify.extend(claims.overdue.into_iter().map(|c| TodayClaim {
                            claim: c.claim,
                            qualified_ticker: ticker.clone(),
                            bucket: "overdue".to_owned(),
                        }));
                        to_verify.extend(claims.due.into_iter().map(|c| TodayClaim {
                            claim: c.claim,
                            qualified_ticker: ticker.clone(),
                            bucket: "due".to_owned(),
                        }));
                    }
                    Err(_) => claims_failed = true,
                }
            }
            if claims_failed {
                section_errors.claims = Some(SectionErrorKind::Unavailable);
                to_verify.clear();
            }
        }
        Err(_) => section_errors.claims = Some(SectionErrorKind::Unavailable),
    }

    match state.autopilot().list_runs(&ListAutopilotRunsInput {
        company_id: None,
        notification_state: Some("unread".to_owned()),
        limit: Some(MAX_AUTOPILOT_RUNS),
    }) {
        Ok(runs) => items.extend(
            runs.into_iter()
                .filter(|run| run.created_at.as_str() >= since.as_str())
                .map(|run| TodayItem::AutopilotRun { run: Box::new(run) }),
        ),
        Err(_) => section_errors.autopilot = Some(SectionErrorKind::Unavailable),
    }

    items.sort_by(|a, b| today_item_sort_key(b).cmp(today_item_sort_key(a)));

    TodayView {
        items,
        to_verify,
        delta_summary,
        previous_visit_at,
        section_errors,
    }
}

/// Composed Dziś v2 read model (F2 S1). Offloaded off the UI thread — reads
/// four independent sections plus a bulk claims scan across tracked
/// companies. Read-only.
#[tauri::command]
pub async fn get_today_view(
    day_limit: i64,
    state: tauri::State<'_, AppState>,
) -> Result<TodayView, String> {
    let state = state.inner().clone();
    tauri::async_runtime::spawn_blocking(move || compute_today_view(&state, day_limit))
        .await
        .map_err(|error| format!("today view task failed: {error}"))
}

/// Stamp the Dziś v2 visit anchor with the backend's own clock (F2 plan
/// decision 4) and return the new value — called by the frontend after a
/// successful render, never on unmount (crash-safe: a visit that never
/// finished never moves the anchor).
#[tauri::command]
pub async fn mark_today_visited(state: tauri::State<'_, AppState>) -> Result<String, String> {
    let state = state.inner().clone();
    tauri::async_runtime::spawn_blocking(move || {
        state
            .settings()
            .mark_today_visited()
            .map_err(|error| error.to_string())
    })
    .await
    .map_err(|error| format!("mark_today_visited task failed: {error}"))?
}

#[cfg(test)]
#[path = "today_tests.rs"]
mod tests;
