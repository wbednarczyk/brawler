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
}

fn list_recent_feed_rows(connection: &Connection, since: &str) -> StorageResult<Vec<TodayFeedRow>> {
    let mut statement = connection.prepare(
        "
        SELECT
            fi.id,
            fic.company_id,
            c.qualified_ticker,
            fi.type,
            fi.title,
            fi.published_at,
            fi.read
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
        ))
    })?;

    let mut result = Vec::new();
    for row in rows {
        let (feed_item_id, company_id, qualified_ticker, item_type, title, published_at, read) =
            row?;
        // Mirrors `feed::feed_item_from_row`'s own attachment-derived
        // presentation kind (feed.rs) — same helper, so the two never drift.
        let has_attachments =
            !super::feed::feed_item_attachments(connection, &feed_item_id)?.is_empty();
        result.push(TodayFeedRow {
            presentation_kind: PresentationKind::derive(&item_type, has_attachments),
            feed_item_id,
            company_id,
            qualified_ticker,
            item_type,
            title,
            published_at,
            read,
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
}
