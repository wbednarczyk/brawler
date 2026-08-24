use super::*;
use crate::storage::{
    open_in_memory_database, NewCompany, NewCompanyEvent, NewFinancialPeriod, NewManagementClaim,
    SetClaimVerdictInput,
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

/// (b) Flat media rows (review finding 8 — no backend clustering): a
/// `Public media` item matched to TWO companies yields TWO rows, one per
/// company (multi-company membership preserved), each carrying the
/// `read` flag and `sourceName` verbatim.
#[test]
fn media_items_are_flat_rows_with_multi_company_membership_and_read_flag_carried() {
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
    // Matched to BOTH companies — must yield one row per company.
    seed_feed_item(
        &state,
        "feed_media_2",
        "Public media",
        &[&company_a, &company_b],
        "Artykuł 2",
        &format!("{today}T09:00:00Z"),
        &format!("{today}T09:00:00Z"),
    );
    state
        .checkout_for_tests()
        .expect("checkout")
        .execute(
            "UPDATE feed_items SET read = 1 WHERE id = 'feed_media_2'",
            [],
        )
        .expect("mark read");

    let view = compute_today_view(&state, 3);
    let media_rows: Vec<_> = view
        .items
        .iter()
        .filter(|item| matches!(item, TodayItem::MediaItem { .. }))
        .collect();

    // 3 rows total: feed_media_1×company_a, feed_media_2×company_a,
    // feed_media_2×company_b — no clustering, no collapsing.
    assert_eq!(media_rows.len(), 3, "one row per (feed item x company)");

    let row_for = |feed_item_id: &str, company_id: &str| {
        media_rows
                .iter()
                .find(|item| matches!(item, TodayItem::MediaItem { feed_item_id: id, company_id: cid, .. } if id == feed_item_id && cid == company_id))
                .unwrap_or_else(|| panic!("row for {feed_item_id}/{company_id}"))
    };

    match row_for("feed_media_2", &company_a) {
        TodayItem::MediaItem {
            read, source_name, ..
        } => {
            assert!(*read, "read flag must carry through to the flat row");
            assert_eq!(source_name, "Test");
        }
        _ => unreachable!(),
    }
    match row_for("feed_media_2", &company_b) {
        TodayItem::MediaItem { read, .. } => {
            assert!(
                *read,
                "the shared item's read flag carries to EVERY matched company's row"
            );
        }
        _ => unreachable!(),
    }
    match row_for("feed_media_1", &company_a) {
        TodayItem::MediaItem { read, .. } => assert!(!read),
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

/// (h) Duplicate-day dedup (review finding 7a): a `periodic_report` event
/// due TODAY with no witness must yield EXACTLY ONE row (the nonArrival
/// kind), never also a calendar row for the same event; once witnessed it
/// stays exactly one row (the calendar kind, since it's no longer a
/// non-arrival candidate at all).
#[test]
fn non_arrival_and_calendar_never_double_up_for_the_same_event() {
    let state = state();
    let today = today_iso();

    let unwitnessed = company(&state, "DUP");
    state
        .create_company_event(NewCompanyEvent {
            company_id: unwitnessed.clone(),
            event_type: "periodic_report".to_owned(),
            title: "Raport okresowy".to_owned(),
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

    let rows_for = |view: &TodayView, company_id: &str| {
        view.items
            .iter()
            .filter(|item| {
                matches!(
                    item,
                    TodayItem::NonArrival { company_id: cid, .. }
                    | TodayItem::Calendar { company_id: cid, .. }
                        if cid == company_id
                )
            })
            .count()
    };

    let view = compute_today_view(&state, 3);
    assert_eq!(
        rows_for(&view, &unwitnessed),
        1,
        "exactly one row for an unwitnessed today-due event, never zero and never two"
    );
    assert!(
            view.items.iter().any(
                |item| matches!(item, TodayItem::NonArrival { company_id, .. } if company_id == &unwitnessed)
            ),
            "the single row must be the nonArrival kind"
        );

    // A witnessed event: never a non-arrival candidate in the first
    // place (has_no_witnessing_report excludes it), so it must land
    // exactly once, as the calendar kind.
    let witnessed = company(&state, "WIT");
    state
        .create_company_event(NewCompanyEvent {
            company_id: witnessed.clone(),
            event_type: "periodic_report".to_owned(),
            title: "Raport okresowy".to_owned(),
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
    seed_feed_item(
        &state,
        "feed_witness",
        "Official report",
        &[&witnessed],
        "Raport okresowy",
        &format!("{today}T09:00:00Z"),
        &format!("{today}T09:00:00Z"),
    );

    let view = compute_today_view(&state, 3);
    assert_eq!(rows_for(&view, &witnessed), 1);
    assert!(
            view.items.iter().any(
                |item| matches!(item, TodayItem::Calendar { company_id, .. } if company_id == &witnessed)
            ),
            "a witnessed event's single row must be the calendar kind"
        );
}

/// (i) Anchor honesty (review finding 5): a storage error reading the
/// visit anchor must surface via `sectionErrors.anchor` — the old
/// `.unwrap_or(None)` silently collapsed a real read error into "first
/// visit", indistinguishable from a genuinely absent KV row.
#[test]
fn a_broken_anchor_read_surfaces_via_section_errors_never_silently_as_first_visit() {
    let state = state();
    state
        .checkout_for_tests()
        .expect("checkout")
        .execute_batch("DROP TABLE settings;")
        .expect("drop table");

    let view = compute_today_view(&state, 3);

    assert!(view.previous_visit_at.is_none());
    assert_eq!(
        view.section_errors.anchor,
        Some(SectionErrorKind::Unavailable),
        "a KV read error must surface via sectionErrors.anchor"
    );
}

/// (j) Seen-signal round-trip (review finding 6, backend half):
/// `notificationState` on the wrapped `AutopilotRun` must survive
/// `compute_today_view` verbatim — the frontend derives seen/collapse
/// state from it.
#[test]
fn autopilot_run_item_carries_its_notification_state_verbatim() {
    let state = state();
    let company_id = company(&state, "AUT");
    state
        .autopilot()
        .create_run_if_absent("run_today", &company_id, "doc1", "manual", "assist", None)
        .expect("create run");

    let view = compute_today_view(&state, 3);
    let run_item = view
        .items
        .iter()
        .find(|item| matches!(item, TodayItem::AutopilotRun { .. }))
        .expect("autopilot run item");
    match run_item {
        TodayItem::AutopilotRun { run } => assert_eq!(
            run.notification_state, "unread",
            "notification_state must round-trip through the DTO verbatim"
        ),
        _ => unreachable!(),
    }
}
