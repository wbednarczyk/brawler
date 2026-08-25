//! Raw storage layer for the Dziś v2 composed read model (F2 S1, ADR 0106 dec.
//! 3, `docs/plans/frontend-v2-f2.md` decisions 1-2). New SQL only — the
//! composition into `TodayView`/`TodayItem` lives in
//! `commands::today::compute_today_view`, mirroring `compute_company_context`
//! for existing sections and adding the queries no other store has: feed
//! items joined to their matched companies (multi-company membership for
//! media clusters) and non-arrival candidates (periodic-report events past
//! their date, filtered by the shared `red_flags` witness/flag-existence
//! predicates).

use super::database::Database;
use super::*;

/// One (feed item, matched company) row — filings and public-media items in
/// the Dziś time window, one row per company a multi-matched item joins (so a
/// media item matched to two companies yields two rows, one per company's
/// cluster).
#[derive(Debug, Clone)]
pub struct TodayFeedRow {
    pub feed_item_id: String,
    pub company_id: String,
    pub qualified_ticker: String,
    pub item_type: String,
    pub title: String,
    pub published_at: String,
    pub read: bool,
    pub source_name: String,
    pub presentation_kind: PresentationKind,
}

/// One non-arrival candidate: a tracked company's `periodic_report` event
/// whose date has passed, already filtered by the shared witness/flag-exists
/// predicates (`red_flags::has_no_witnessing_report` /
/// `red_flags::report_delay_flag_raised`) — every row the caller gets back is
/// a real non-arrival, no further filtering needed.
#[derive(Debug, Clone)]
pub struct NonArrivalCandidate {
    pub event_key: String,
    pub company_id: String,
    pub qualified_ticker: String,
    pub event_date: String,
    pub title: String,
}

/// Dziś-specific storage (Architecture v2 / ADR 0050). Reach it via
/// `AppState::today()`.
#[derive(Clone)]
pub struct TodayStore {
    db: Database,
}

impl TodayStore {
    pub(super) fn new(db: Database) -> Self {
        Self { db }
    }

    /// Filing (`Official report`) + media (`Public media`) items published
    /// on/after `since`, scoped to tracked companies via `feed_item_companies`
    /// (the same join table the multi-company-match writers populate — so an
    /// item matched to N companies yields N rows here, one per company's
    /// section/cluster). Ordered by `published_at` DESC (the domain date,
    /// never `created_at`).
    pub fn list_recent_feed_rows(&self, since: &str) -> StorageResult<Vec<TodayFeedRow>> {
        let connection = self.db.checkout()?;
        list_recent_feed_rows(&connection, since)
    }

    /// Periodic-report non-arrival candidates as of `today` (UTC `YYYY-MM-DD`,
    /// no grace period — see plan decision 2: the grace lives inside the
    /// `report_delay` detector, not here). Filtered in-store by the shared
    /// witness/flag-existence predicates so the caller only ever sees real
    /// non-arrivals.
    pub fn list_non_arrivals(&self, today: &str) -> StorageResult<Vec<NonArrivalCandidate>> {
        let connection = self.db.checkout()?;
        list_non_arrivals(&connection, today)
    }

    /// `company_events` rows within `[date_from, date_to]` inclusive (finding
    /// 7b): a date-bounded SQL path for the Dziś calendar window, instead of
    /// `events::list_company_events` loading the WHOLE table and filtering in
    /// process. Does not touch that function's public contract — this is a
    /// separate, narrower query for exactly the Dziś use case.
    pub fn list_events_in_window(
        &self,
        date_from: &str,
        date_to: &str,
    ) -> StorageResult<Vec<CompanyEvent>> {
        let connection = self.db.checkout()?;
        list_events_in_window(&connection, date_from, date_to)
    }
}

fn list_recent_feed_rows(connection: &Connection, since: &str) -> StorageResult<Vec<TodayFeedRow>> {
    // `has_attachments` is a correlated EXISTS, not a per-row follow-up query
    // (finding 7b): the old code called `feed::feed_item_attachments` once per
    // result row (N+1). Same truth value as that helper's "is the attachment
    // list non-empty" check, computed in the one bulk query instead.
    let mut statement = connection.prepare(
        "
        SELECT
            fi.id,
            fic.company_id,
            c.qualified_ticker,
            fi.type,
            fi.title,
            fi.published_at,
            fi.read,
            fi.source_name,
            EXISTS(SELECT 1 FROM feed_item_attachments fa WHERE fa.feed_item_id = fi.id)
        FROM feed_items fi
        JOIN feed_item_companies fic ON fic.feed_item_id = fi.id
        JOIN companies c ON c.id = fic.company_id
        WHERE fi.type IN ('Official report', 'Public media')
          AND fi.published_at >= ?1
        ORDER BY fi.published_at DESC, fi.id, fic.company_id
        ",
    )?;

    let rows = statement.query_map(params![since], |row| {
        Ok((
            row.get::<_, String>(0)?,
            row.get::<_, String>(1)?,
            row.get::<_, String>(2)?,
            row.get::<_, String>(3)?,
            row.get::<_, String>(4)?,
            row.get::<_, String>(5)?,
            row.get::<_, bool>(6)?,
            row.get::<_, String>(7)?,
            row.get::<_, bool>(8)?,
        ))
    })?;

    let mut result = Vec::new();
    for row in rows {
        let (
            feed_item_id,
            company_id,
            qualified_ticker,
            item_type,
            title,
            published_at,
            read,
            source_name,
            has_attachments,
        ) = row?;
        result.push(TodayFeedRow {
            presentation_kind: PresentationKind::derive(&item_type, has_attachments),
            feed_item_id,
            company_id,
            qualified_ticker,
            item_type,
            title,
            published_at,
            read,
            source_name,
        });
    }
    Ok(result)
}

fn list_non_arrivals(
    connection: &Connection,
    today: &str,
) -> StorageResult<Vec<NonArrivalCandidate>> {
    let mut statement = connection.prepare(
        "
        SELECT ce.id, ce.company_id, c.qualified_ticker, ce.event_date, ce.title
        FROM company_events ce
        JOIN companies c ON c.id = ce.company_id
        WHERE ce.event_type = 'periodic_report'
          AND ce.event_date <= ?1
        ORDER BY ce.event_date ASC, ce.id ASC
        ",
    )?;

    let rows = statement.query_map(params![today], |row| {
        Ok((
            row.get::<_, String>(0)?,
            row.get::<_, String>(1)?,
            row.get::<_, String>(2)?,
            row.get::<_, String>(3)?,
            row.get::<_, String>(4)?,
        ))
    })?;

    let mut result = Vec::new();
    for row in rows {
        let (event_key, company_id, qualified_ticker, event_date, title) = row?;
        // Shared with the `report_delay` detector (ADR 0083 D8, F2 S1
        // decision 2) — a non-arrival exists exactly when no witnessing
        // report exists AND the flag hasn't taken over yet.
        if !super::red_flags::has_no_witnessing_report(connection, &company_id, &event_date)? {
            continue;
        }
        if super::red_flags::report_delay_flag_raised(connection, &company_id, &event_key)? {
            continue;
        }
        result.push(NonArrivalCandidate {
            event_key,
            company_id,
            qualified_ticker,
            event_date,
            title,
        });
    }
    Ok(result)
}

fn list_events_in_window(
    connection: &Connection,
    date_from: &str,
    date_to: &str,
) -> StorageResult<Vec<CompanyEvent>> {
    let mut statement = connection.prepare(
        "
        SELECT
            company_events.id,
            company_events.company_id,
            companies.qualified_ticker,
            companies.display_name,
            company_events.event_type,
            company_events.title,
            company_events.event_date,
            company_events.event_time,
            company_events.status,
            company_events.source_type,
            company_events.source_adapter_id,
            company_events.source_event_key,
            company_events.source_url,
            company_events.attribution,
            company_events.fetched_at,
            company_events.manual,
            company_events.created_at,
            company_events.updated_at
        FROM company_events
        JOIN companies ON companies.id = company_events.company_id
        WHERE company_events.event_date >= ?1 AND company_events.event_date <= ?2
        ORDER BY company_events.event_date ASC, company_events.event_time ASC, company_events.title ASC
        ",
    )?;
    let rows = statement.query_map(
        params![date_from, date_to],
        super::events::company_event_from_row,
    )?;
    Ok(rows.collect::<Result<Vec<_>, _>>()?)
}

#[cfg(test)]
mod tests {
    use crate::storage::{open_in_memory_database, AppState, NewCompany, NewCompanyEvent};

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

    #[test]
    fn list_non_arrivals_reads_back_a_candidate_before_any_flag_or_witness() {
        let state = state();
        let company_id = company(&state, "NAR");
        state
            .create_company_event(NewCompanyEvent {
                company_id: company_id.clone(),
                event_type: "periodic_report".to_owned(),
                title: "Raport okresowy".to_owned(),
                event_date: "2026-01-01".to_owned(),
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

        let store = state.today();
        let candidates = store.list_non_arrivals("2026-01-02").expect("candidates");
        assert_eq!(candidates.len(), 1);
        assert_eq!(candidates[0].company_id, company_id);
        assert_eq!(candidates[0].event_date, "2026-01-01");
    }

    #[test]
    fn list_non_arrivals_is_suppressed_once_the_report_delay_flag_exists() {
        let state = state();
        let company_id = company(&state, "SUP");
        state
            .create_company_event(NewCompanyEvent {
                company_id: company_id.clone(),
                event_type: "periodic_report".to_owned(),
                title: "Raport okresowy".to_owned(),
                event_date: "2026-01-01".to_owned(),
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

        // Before the flag exists (still inside the detector's grace window):
        // the candidate is visible.
        assert_eq!(
            state
                .today()
                .list_non_arrivals("2026-01-02")
                .expect("before")
                .len(),
            1
        );

        // The real detector raises the flag past the grace window (3 days) —
        // reusing production detection rather than hand-inserting a row keeps
        // the suppression test honest to the real handover mechanics.
        {
            let connection = state.checkout_for_tests().expect("connection");
            super::red_flags::detect_report_delays(&connection, "2026-01-10").expect("detect");
        }

        assert_eq!(
            state
                .today()
                .list_non_arrivals("2026-01-10")
                .expect("after")
                .len(),
            0,
            "a raised report_delay flag must suppress the non-arrival row (F2 S1 decision 2)"
        );
    }

    /// Batched attachment lookup (finding 7b): `list_recent_feed_rows` used to
    /// call `feed::feed_item_attachments` once PER ROW (N+1). The bulk
    /// `EXISTS` correlated subquery must produce the SAME per-row
    /// `presentation_kind` — and, critically, must not duplicate a row when
    /// an item has MULTIPLE attachments (the risk a naive `LEFT JOIN`
    /// batching approach would introduce).
    #[test]
    fn list_recent_feed_rows_derives_has_attachments_in_bulk_without_duplicating_rows() {
        use crate::storage::PresentationKind;

        let state = state();
        let company_id = company(&state, "ATT");
        let connection = state.checkout_for_tests().expect("checkout");
        connection
            .execute(
                "INSERT INTO feed_items
                    (id, type, source_adapter_id, source_name, source_url, title, language,
                     published_at, fetched_at, dedupe_key, created_at, updated_at)
                 VALUES ('feed_with_att', 'Official report', 'gpw-espi-ebi', 'Test',
                     'https://example.test/att', 'Has attachments', 'pl',
                     '2026-01-05T09:00:00Z', '2026-01-05T09:00:00Z', 'feed_with_att',
                     '2026-01-05T09:00:00Z', '2026-01-05T09:00:00Z')",
                [],
            )
            .expect("insert item with attachments");
        connection
            .execute(
                "INSERT INTO feed_item_companies (feed_item_id, company_id, match_type)
                 VALUES ('feed_with_att', ?1, 'exact')",
                rusqlite::params![company_id],
            )
            .expect("link company");
        // TWO attachments on the same item — a naive LEFT JOIN batching
        // approach would duplicate this row into two.
        connection
            .execute(
                "INSERT INTO feed_item_attachments (id, feed_item_id, label, url, position)
                 VALUES ('att_1', 'feed_with_att', 'A', 'https://example.test/a.pdf', 0)",
                [],
            )
            .expect("attachment 1");
        connection
            .execute(
                "INSERT INTO feed_item_attachments (id, feed_item_id, label, url, position)
                 VALUES ('att_2', 'feed_with_att', 'B', 'https://example.test/b.pdf', 1)",
                [],
            )
            .expect("attachment 2");
        connection
            .execute(
                "INSERT INTO feed_items
                    (id, type, source_adapter_id, source_name, source_url, title, language,
                     published_at, fetched_at, dedupe_key, created_at, updated_at)
                 VALUES ('feed_no_att', 'Official report', 'gpw-espi-ebi', 'Test',
                     'https://example.test/noatt', 'No attachments', 'pl',
                     '2026-01-05T08:00:00Z', '2026-01-05T08:00:00Z', 'feed_no_att',
                     '2026-01-05T08:00:00Z', '2026-01-05T08:00:00Z')",
                [],
            )
            .expect("insert item without attachments");
        connection
            .execute(
                "INSERT INTO feed_item_companies (feed_item_id, company_id, match_type)
                 VALUES ('feed_no_att', ?1, 'exact')",
                rusqlite::params![company_id],
            )
            .expect("link company");
        drop(connection);

        let rows = state
            .today()
            .list_recent_feed_rows("2026-01-01")
            .expect("rows");
        assert_eq!(
            rows.len(),
            2,
            "no row duplication from the two-attachment item"
        );

        let with_att = rows
            .iter()
            .find(|r| r.feed_item_id == "feed_with_att")
            .expect("row with attachments");
        assert_eq!(with_att.presentation_kind, PresentationKind::Report);
        let no_att = rows
            .iter()
            .find(|r| r.feed_item_id == "feed_no_att")
            .expect("row without attachments");
        assert_eq!(no_att.presentation_kind, PresentationKind::Filing);
    }

    /// Bounded events window (finding 7b): `list_events_in_window` reads
    /// straight from SQL bounds instead of `events::list_company_events`'s
    /// whole-table-then-filter — an event outside `[date_from, date_to]`
    /// must never come back.
    #[test]
    fn list_events_in_window_excludes_rows_outside_the_bound() {
        let state = state();
        let company_id = company(&state, "WIN");
        for (id, date) in [
            ("ev_before", "2025-12-30"),
            ("ev_in", "2026-01-02"),
            ("ev_after", "2026-01-10"),
        ] {
            state
                .create_company_event(NewCompanyEvent {
                    company_id: company_id.clone(),
                    event_type: "shareholder_meeting".to_owned(),
                    title: id.to_owned(),
                    event_date: date.to_owned(),
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
        }

        let events = state
            .today()
            .list_events_in_window("2026-01-01", "2026-01-05")
            .expect("events");
        assert_eq!(events.len(), 1, "only the in-window event comes back");
        assert_eq!(events[0].title, "ev_in");
    }
}
