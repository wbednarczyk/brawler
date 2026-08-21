//! Dziś v2 composed read model (F2 S1; ADR 0106 dec. 3;
//! `docs/plans/frontend-v2-f2.md` decisions 1-2). One command replaces the
//! frontend's app-root pulse hook: a FLAT `items[]` (day-bucketing is a
//! frontend concern, S3) built from four independent sections — feed
//! (filings + media clusters), non-arrival (shared `report_delay` predicate),
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
    AutopilotRun, CompanyEventListInput, ListAutopilotRunsInput, ManagementClaim, PresentationKind,
    TodayFeedRow,
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
    /// `Public media` items grouped per (company, UTC day of `publishedAt`)
    /// (plan decision 3). An item matched to multiple companies joins EACH
    /// company's cluster — the join happens upstream in
    /// [`crate::storage::TodayStore::list_recent_feed_rows`].
    #[serde(rename_all = "camelCase")]
    MediaCluster {
        company_id: String,
        qualified_ticker: String,
        /// UTC calendar day (`YYYY-MM-DD`) the cluster's items published on.
        day: String,
        count: i64,
        earliest_published_at: String,
        latest_published_at: String,
        /// The 3 most recent titles in the cluster (rows arrive latest-first).
        top_titles: Vec<String>,
        feed_item_ids: Vec<String>,
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
/// the `calendar` slot — both read `company_events`/`red_flags` state, and
/// the plan's DTO enumerates exactly these four keys.
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
}

/// Counts of items newer than `previousVisitAt` (plan decision 1's delta
/// header). Raw item counts, not cluster counts — a 5-item media cluster
/// contributes 5 to `mediaCount`.
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

/// Split raw joined feed rows into flat `Filing` items and grouped
/// `MediaCluster` items (plan decision 3). Rows arrive `published_at` DESC
/// (the store's own ordering), so accumulating titles/ids in encounter order
/// yields "most recent first" per cluster with no extra sort.
fn split_feed_rows(rows: Vec<TodayFeedRow>) -> (Vec<TodayItem>, Vec<TodayItem>) {
    struct ClusterAcc {
        qualified_ticker: String,
        day: String,
        earliest: String,
        latest: String,
        top_titles: Vec<String>,
        feed_item_ids: Vec<String>,
    }

    let mut filings = Vec::new();
    let mut clusters: std::collections::BTreeMap<(String, String), ClusterAcc> =
        std::collections::BTreeMap::new();

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
            "Public media" => {
                let day: String = row.published_at.chars().take(10).collect();
                let key = (row.company_id.clone(), day.clone());
                let acc = clusters.entry(key).or_insert_with(|| ClusterAcc {
                    qualified_ticker: row.qualified_ticker.clone(),
                    day: day.clone(),
                    earliest: row.published_at.clone(),
                    latest: row.published_at.clone(),
                    top_titles: Vec::new(),
                    feed_item_ids: Vec::new(),
                });
                if row.published_at < acc.earliest {
                    acc.earliest = row.published_at.clone();
                }
                if row.published_at > acc.latest {
                    acc.latest = row.published_at.clone();
                }
                if acc.top_titles.len() < 3 {
                    acc.top_titles.push(row.title.clone());
                }
                acc.feed_item_ids.push(row.feed_item_id.clone());
            }
            _ => {}
        }
    }

    let media_items = clusters
        .into_iter()
        .map(|((company_id, _day), acc)| TodayItem::MediaCluster {
            company_id,
            qualified_ticker: acc.qualified_ticker,
            day: acc.day,
            count: acc.feed_item_ids.len() as i64,
            earliest_published_at: acc.earliest,
            latest_published_at: acc.latest,
            top_titles: acc.top_titles,
            feed_item_ids: acc.feed_item_ids,
        })
        .collect();

    (filings, media_items)
}

/// The domain-date sort key for one item — never `created_at`
/// (data-model.md § Model principles).
fn today_item_sort_key(item: &TodayItem) -> &str {
    match item {
        TodayItem::Filing { published_at, .. } => published_at,
        TodayItem::MediaCluster {
            latest_published_at,
            ..
        } => latest_published_at,
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

    let previous_visit_at = state.settings().today_last_visit_at().unwrap_or(None);
    // "" sorts before every real ISO timestamp, so an absent anchor (first
    // visit) naturally counts every in-window item as delta.
    let delta_cutoff = previous_visit_at.clone().unwrap_or_default();

    let mut items = Vec::new();
    let mut section_errors = TodaySectionErrors::default();
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
            let (filings, clusters) = split_feed_rows(rows);
            items.extend(filings);
            items.extend(clusters);
        }
        Err(_) => section_errors.feed = Some(SectionErrorKind::Unavailable),
    }

    // Non-arrival shares the calendar slot (both company_events-backed).
    let mut non_arrival_failed = false;
    match state.today().list_non_arrivals(&today) {
        Ok(candidates) => items.extend(candidates.into_iter().map(|c| TodayItem::NonArrival {
            event_key: c.event_key,
            company_id: c.company_id,
            qualified_ticker: c.qualified_ticker,
            event_date: c.event_date,
            title: c.title,
        })),
        Err(_) => non_arrival_failed = true,
    }

    match state.events().list_company_events(CompanyEventListInput {
        mode: None,
        company_id: None,
        watchlist_id: None,
        event_type: None,
        status: None,
        date_from: Some(today.clone()),
        date_to: Some(horizon),
    }) {
        Ok(events) => items.extend(events.into_iter().map(|event| TodayItem::Calendar {
            event_key: event.id,
            event_date: event.event_date,
            event_type: event.event_type,
            title: event.title,
            company_id: event.company_id,
            qualified_ticker: event.company,
        })),
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
mod tests {
    use super::*;
    use crate::storage::{
        open_in_memory_database, NewCompany, NewCompanyEvent, NewFinancialPeriod,
        NewManagementClaim, SetClaimVerdictInput,
    };

    fn state() -> AppState {
        AppState::new(open_in_memory_database().expect("in-memory db"))
    }

    fn company(state: &AppState, ticker: &str) -> String {
        state
            .create_company(NewCompany {
                exchange: "GPW".to_owned(),
                ticker: ticker.to_owned(),
                display_name: format!("{ticker} S.A."),
                isin: None,
                cik: None,
                lei: None,
            })
            .expect("company")
            .id
    }

    /// Raw insert — no source-fetch machinery needed for these tests, mirrors
    /// the seeding idiom already used in `commands::tagged_fact_promotion`
    /// tests. `created_at` is set explicitly so ordering tests are
    /// deterministic (never left to wall-clock insert timing).
    fn seed_feed_item(
        state: &AppState,
        id: &str,
        item_type: &str,
        company_ids: &[&str],
        title: &str,
        published_at: &str,
        created_at: &str,
    ) {
        let connection = state.checkout_for_tests().expect("checkout");
        connection
            .execute(
                "INSERT INTO feed_items
                    (id, type, source_adapter_id, source_name, source_url, title, language,
                     published_at, fetched_at, dedupe_key, created_at, updated_at)
                 VALUES (?1, ?2, 'gpw-espi-ebi', 'Test', ?3, ?4, 'pl', ?5, ?5, ?1, ?6, ?6)",
                rusqlite::params![
                    id,
                    item_type,
                    format!("https://example.test/{id}"),
                    title,
                    published_at,
                    created_at
                ],
            )
            .expect("feed item");
        for company_id in company_ids {
            connection
                .execute(
                    "INSERT INTO feed_item_companies (feed_item_id, company_id, match_type)
                     VALUES (?1, ?2, 'exact')",
                    rusqlite::params![id, company_id],
                )
                .expect("feed item company link");
        }
    }

    /// (a) Flat items with correct kinds and fields: one filing + one
    /// calendar entry both surface, tagged correctly, with the expected
    /// fields.
    #[test]
    fn flat_items_carry_the_right_kind_and_fields() {
        let state = state();
        let company_id = company(&state, "FLT");
        let today = today_iso();

        seed_feed_item(
            &state,
            "feed_filing_1",
            "Official report",
            &[&company_id],
            "Raport bieżący",
            &format!("{today}T09:00:00Z"),
            &format!("{today}T09:00:00Z"),
        );
        state
            .create_company_event(NewCompanyEvent {
                company_id: company_id.clone(),
                event_type: "shareholder_meeting".to_owned(),
                title: "WZA".to_owned(),
                event_date: today.clone(),
                event_time: None,
                status: None,
                source_type: None,
                source_adapter_id: None,
                source_event_key: None,
                source_url: None,
                attribution: None,
                fetched_at: None,
            })
            .expect("event");

        let view = compute_today_view(&state, 3);

        let filing = view
            .items
            .iter()
            .find(|item| matches!(item, TodayItem::Filing { .. }))
            .expect("filing item");
        match filing {
            TodayItem::Filing {
                feed_item_id,
                company_id: cid,
                title,
                read,
                ..
            } => {
                assert_eq!(feed_item_id, "feed_filing_1");
                assert_eq!(cid, &company_id);
                assert_eq!(title, "Raport bieżący");
                assert!(!read);
            }
            _ => unreachable!(),
        }

        let calendar = view
            .items
            .iter()
            .find(|item| matches!(item, TodayItem::Calendar { .. }))
            .expect("calendar item");
        match calendar {
            TodayItem::Calendar {
                event_type, title, ..
            } => {
                assert_eq!(event_type, "shareholder_meeting");
                assert_eq!(title, "WZA");
            }
            _ => unreachable!(),
        }
    }

    /// (b) Media-cluster grouping: two `Public media` items on the same UTC
    /// day for the same company cluster into one item; an item matched to TWO
    /// companies joins EACH company's cluster (multi-company membership).
    #[test]
    fn media_items_cluster_per_company_per_day_including_multi_company_membership() {
        let state = state();
        let company_a = company(&state, "MDA");
        let company_b = company(&state, "MDB");
        let today = today_iso();

        seed_feed_item(
            &state,
            "feed_media_1",
            "Public media",
            &[&company_a],
            "Artykuł 1",
            &format!("{today}T08:00:00Z"),
            &format!("{today}T08:00:00Z"),
        );
        // Matched to BOTH companies — must join both clusters.
        seed_feed_item(
            &state,
            "feed_media_2",
            "Public media",
            &[&company_a, &company_b],
            "Artykuł 2",
            &format!("{today}T09:00:00Z"),
            &format!("{today}T09:00:00Z"),
        );

        let view = compute_today_view(&state, 3);
        let clusters: Vec<_> = view
            .items
            .iter()
            .filter(|item| matches!(item, TodayItem::MediaCluster { .. }))
            .collect();

        // Company A: one cluster with both items (count 2). Company B: one
        // cluster with only the shared item (count 1).
        let cluster_a = clusters
            .iter()
            .find(|item| matches!(item, TodayItem::MediaCluster { company_id, .. } if company_id == &company_a))
            .expect("company A cluster");
        let cluster_b = clusters
            .iter()
            .find(|item| matches!(item, TodayItem::MediaCluster { company_id, .. } if company_id == &company_b))
            .expect("company B cluster");

        match cluster_a {
            TodayItem::MediaCluster { count, day, .. } => {
                assert_eq!(*count, 2);
                assert_eq!(day, &today);
            }
            _ => unreachable!(),
        }
        match cluster_b {
            TodayItem::MediaCluster { count, .. } => assert_eq!(*count, 1),
            _ => unreachable!(),
        }
    }

    /// (c) Non-arrival appears for an unwitnessed past `periodic_report`, and
    /// is suppressed the moment a `report_delay` flag exists for that event
    /// (flag-existence suppression, plan decision 2).
    #[test]
    fn non_arrival_is_suppressed_once_the_report_delay_flag_exists() {
        let state = state();
        let company_id = company(&state, "NAV");
        // Past the detector's 3-day grace (real wall-clock — both
        // `compute_today_view` and `detect_report_delays()` anchor to the
        // same `today_iso()`), so a single real detection pass raises it.
        let four_days_ago = date_shift_days(&today_iso(), -4);
        state
            .create_company_event(NewCompanyEvent {
                company_id: company_id.clone(),
                event_type: "periodic_report".to_owned(),
                title: "Raport okresowy".to_owned(),
                event_date: four_days_ago,
                event_time: None,
                status: None,
                source_type: None,
                source_adapter_id: None,
                source_event_key: None,
                source_url: None,
                attribution: None,
                fetched_at: None,
            })
            .expect("event");

        let has_non_arrival = |view: &TodayView| {
            view.items.iter().any(
                |item| matches!(item, TodayItem::NonArrival { company_id: cid, .. } if cid == &company_id),
            )
        };

        let before = compute_today_view(&state, 7);
        assert!(
            has_non_arrival(&before),
            "non-arrival must appear before the flag exists"
        );

        state
            .red_flags()
            .detect_report_delays()
            .expect("detect report delays");

        let after = compute_today_view(&state, 7);
        assert!(
            !has_non_arrival(&after),
            "flag existence must suppress the non-arrival row (plan decision 2)"
        );
    }

    /// (d) `created_at` ordering contradicts `published_at` ordering — the
    /// output must follow `published_at` (repo guardrail: domain date, never
    /// `created_at`).
    #[test]
    fn items_follow_published_at_order_even_when_created_at_disagrees() {
        let state = state();
        let company_id = company(&state, "ORD");
        let today = today_iso();

        // Inserted FIRST (earlier created_at) but has the EARLIER
        // published_at — domain-older.
        seed_feed_item(
            &state,
            "feed_ord_older",
            "Official report",
            &[&company_id],
            "Older",
            &format!("{today}T06:00:00Z"),
            &format!("{today}T06:00:00.000Z"),
        );
        // Inserted SECOND (later created_at) and has the LATER published_at —
        // domain-newer. created_at order and published_at order agree here by
        // construction of the insert sequence, so flip it explicitly below.
        seed_feed_item(
            &state,
            "feed_ord_newer",
            "Official report",
            &[&company_id],
            "Newer",
            &format!("{today}T18:00:00Z"),
            &format!("{today}T05:00:00.000Z"), // earlier created_at than "older"
        );

        let view = compute_today_view(&state, 3);
        let filing_ids: Vec<&str> = view
            .items
            .iter()
            .filter_map(|item| match item {
                TodayItem::Filing { feed_item_id, .. } => Some(feed_item_id.as_str()),
                _ => None,
            })
            .collect();

        assert_eq!(
            filing_ids,
            vec!["feed_ord_newer", "feed_ord_older"],
            "must order by published_at DESC, not created_at (feed_ord_newer has the EARLIER created_at)"
        );
    }

    /// (e) sectionErrors: a storage error in one section (claims table
    /// dropped) fills only that section's slot; other sections still
    /// populate.
    #[test]
    fn a_failing_claims_table_fills_only_the_claims_section_error() {
        let state = state();
        let company_id = company(&state, "ERR");
        let today = today_iso();
        seed_feed_item(
            &state,
            "feed_err_1",
            "Official report",
            &[&company_id],
            "Still here",
            &format!("{today}T09:00:00Z"),
            &format!("{today}T09:00:00Z"),
        );

        state
            .checkout_for_tests()
            .expect("checkout")
            .execute_batch("DROP TABLE management_claims;")
            .expect("drop table");

        let view = compute_today_view(&state, 3);

        assert_eq!(
            view.section_errors.claims,
            Some(SectionErrorKind::Unavailable)
        );
        assert!(view.section_errors.feed.is_none());
        assert!(view.section_errors.calendar.is_none());
        assert!(view.section_errors.autopilot.is_none());
        assert!(
            view.items
                .iter()
                .any(|item| matches!(item, TodayItem::Filing { .. })),
            "the feed section must still populate despite the claims failure"
        );
    }

    /// (f) Claims bulk: pending claims for tracked companies come back
    /// bucketed `due`/`overdue`, decorated with the company ticker.
    #[test]
    fn claims_bulk_returns_pending_claims_for_tracked_companies() {
        let state = state();
        let company_id = company(&state, "CLM");

        // A LATER fiscal year's period has already arrived while the claim
        // (due FY2020) is still pending — that's what makes it "overdue"
        // (`list_claims_to_verify`'s `later_year_arrived` rule).
        state
            .financials()
            .create_financial_period(NewFinancialPeriod {
                company_id: company_id.clone(),
                fiscal_year: 2021,
                period_type: "FY".to_owned(),
                period_end_date: Some("2021-12-31".to_owned()),
                report_evidence_ref: None,
            })
            .expect("period");
        state
            .create_management_claim(NewManagementClaim {
                company_id: company_id.clone(),
                statement: "Overdue claim".to_owned(),
                body: None,
                made_at: None,
                source_period_id: None,
                due_fiscal_year: Some(2020),
                due_period_type: Some("FY".to_owned()),
                status: None,
                source_evidence_type: None,
                source_evidence_id: None,
                target_metric_key: None,
                target_comparator: None,
                target_value_numeric: None,
                target_unit: None,
            })
            .expect("claim");
        let verified = state
            .create_management_claim(NewManagementClaim {
                company_id: company_id.clone(),
                statement: "Verified claim".to_owned(),
                body: None,
                made_at: None,
                source_period_id: None,
                due_fiscal_year: Some(2025),
                due_period_type: Some("FY".to_owned()),
                status: None,
                source_evidence_type: None,
                source_evidence_id: None,
                target_metric_key: None,
                target_comparator: None,
                target_value_numeric: None,
                target_unit: None,
            })
            .expect("claim");
        state
            .set_claim_verdict(SetClaimVerdictInput {
                claim_id: verified.id,
                status: "delivered".to_owned(),
                verifying_fact_id: None,
                verifying_relation: None,
                revises_claim_id: None,
            })
            .expect("verify");

        let view = compute_today_view(&state, 3);
        assert_eq!(view.to_verify.len(), 1);
        assert_eq!(view.to_verify[0].bucket, "overdue");
        assert_eq!(view.to_verify[0].qualified_ticker, "GPW:CLM");
    }

    /// (g) `previousVisitAt`: absent KV row reads back `None`; a present row
    /// is echoed verbatim.
    #[test]
    fn previous_visit_at_is_null_when_absent_and_echoed_when_present() {
        let state = state();
        let view = compute_today_view(&state, 3);
        assert!(view.previous_visit_at.is_none());

        state
            .checkout_for_tests()
            .expect("checkout")
            .execute(
                "INSERT INTO settings (key, value, value_type) VALUES ('today_last_visit_at', ?1, 'string')",
                rusqlite::params!["2026-08-15T07:00:00Z"],
            )
            .expect("seed anchor");

        let view = compute_today_view(&state, 3);
        assert_eq!(
            view.previous_visit_at.as_deref(),
            Some("2026-08-15T07:00:00Z")
        );
    }

    /// Server-side clamp (plan decision 1): an out-of-range `dayLimit` never
    /// crashes and never widens the window past 7 days — proven indirectly
    /// via the "since" cutoff excluding an item published 10 days ago even
    /// when `dayLimit` is requested far above the max.
    #[test]
    fn day_limit_is_clamped_server_side() {
        let state = state();
        let company_id = company(&state, "CLP");
        let ten_days_ago = date_shift_days(&today_iso(), -10);
        seed_feed_item(
            &state,
            "feed_old",
            "Official report",
            &[&company_id],
            "Too old",
            &format!("{ten_days_ago}T09:00:00Z"),
            &format!("{ten_days_ago}T09:00:00Z"),
        );

        let view = compute_today_view(&state, 999);
        assert!(
            view.items.is_empty(),
            "dayLimit=999 must clamp to 7, excluding a 10-day-old item"
        );
    }
}
