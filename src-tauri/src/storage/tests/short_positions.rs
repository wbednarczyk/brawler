//! KNF short-selling diff/persist behavior (ADR 0069 decision 3, plan v0.55 T4).

use super::*;
use crate::source_adapters::knf_short_selling::KnfShortEntry;

const CDR_ISIN: &str = "PLOPTTC00011";

fn tracked_company(state: &AppState) -> Company {
    state
        .create_company(NewCompany {
            exchange: "GPW".to_owned(),
            ticker: "CDR".to_owned(),
            display_name: "CD PROJEKT S.A.".to_owned(),
            isin: Some(CDR_ISIN.to_owned()),
            cik: None,
            lei: None,
        })
        .expect("company should be created")
}

/// The adapter's mutable catalog row (seeded by the registry wiring in
/// production; inserted here so the feed-item FK and run-outcome update resolve).
fn seed_adapter(state: &AppState) {
    let connection = state.checkout().expect("connection");
    connection
        .execute(
            "
            INSERT INTO source_adapters (
                id, display_name, source_type, fetch_mode, enabled, default_poll_interval_seconds
            ) VALUES ('knf-short-selling', 'KNF Short Selling Register', 'disclosure', 'public_json', 1, 86400)
            ON CONFLICT(id) DO NOTHING
            ",
            [],
        )
        .expect("adapter row should seed");
}

fn entry(holder: &str, isin: &str, pct: f64, date: &str) -> KnfShortEntry {
    KnfShortEntry {
        holder_name: holder.to_owned(),
        issuer_name: "ISSUER".to_owned(),
        isin: isin.to_owned(),
        net_position_pct: pct,
        position_date: date.to_owned(),
        modify_date: None,
    }
}

fn count(state: &AppState, sql: &str) -> i64 {
    let connection = state.checkout().expect("connection");
    connection
        .query_row(sql, [], |row| row.get::<_, i64>(0))
        .expect("count query")
}

fn count_events(state: &AppState) -> i64 {
    count(state, "SELECT COUNT(*) FROM short_position_events")
}

fn count_signals(state: &AppState) -> i64 {
    count(
        state,
        "SELECT COUNT(*) FROM company_signals WHERE category = 'short_position_change'",
    )
}

fn count_feed(state: &AppState) -> i64 {
    count(
        state,
        "SELECT COUNT(*) FROM feed_items WHERE source_adapter_id = 'knf-short-selling'",
    )
}

fn latest_event_kind(state: &AppState) -> String {
    let connection = state.checkout().expect("connection");
    connection
        .query_row(
            "SELECT kind FROM short_position_events ORDER BY rowid DESC LIMIT 1",
            [],
            |row| row.get::<_, String>(0),
        )
        .expect("latest event")
}

fn setup() -> (AppState, Company) {
    let connection = open_in_memory_database().expect("database should initialize");
    let state = AppState::new(connection);
    let company = tracked_company(&state);
    seed_adapter(&state);
    (state, company)
}

#[test]
fn entered_creates_event_feed_and_signal() {
    let (state, company) = setup();

    let outcome = state
        .short_positions()
        .ingest_knf_short_positions(&[entry("AKO Capital LLP", CDR_ISIN, 0.53, "2026-07-14")])
        .expect("ingest should succeed");

    assert_eq!(outcome.items_matched, 1);
    assert_eq!(outcome.items_created, 1);
    assert_eq!(count_events(&state), 1);
    assert_eq!(count_signals(&state), 1);
    assert_eq!(count_feed(&state), 1);
    assert_eq!(latest_event_kind(&state), "entered");

    let connection = state.checkout().expect("connection");
    let (pct, exited): (f64, Option<String>) = connection
        .query_row(
            "SELECT net_position_pct, exited_at FROM short_positions WHERE company_id = ?1",
            [&company.id],
            |row| Ok((row.get(0)?, row.get(1)?)),
        )
        .expect("stored position");
    assert!((pct - 0.53).abs() < 1e-9);
    assert!(exited.is_none());
}

#[test]
fn increased_then_decreased_records_history() {
    let (state, _company) = setup();
    let store = state.short_positions();

    store
        .ingest_knf_short_positions(&[entry("AKO", CDR_ISIN, 0.53, "2026-07-14")])
        .expect("entered");
    store
        .ingest_knf_short_positions(&[entry("AKO", CDR_ISIN, 0.80, "2026-07-15")])
        .expect("increased");
    assert_eq!(latest_event_kind(&state), "increased");

    store
        .ingest_knf_short_positions(&[entry("AKO", CDR_ISIN, 0.60, "2026-07-16")])
        .expect("decreased");
    assert_eq!(latest_event_kind(&state), "decreased");

    assert_eq!(count_events(&state), 3);
    assert_eq!(count_signals(&state), 3);
    assert_eq!(count_feed(&state), 3);
}

#[test]
fn exited_when_holder_absent_from_snapshot() {
    let (state, company) = setup();
    let store = state.short_positions();

    store
        .ingest_knf_short_positions(&[entry("AKO", CDR_ISIN, 0.53, "2026-07-14")])
        .expect("entered");
    // Empty register snapshot: the holder dropped out (below 0.5%).
    store.ingest_knf_short_positions(&[]).expect("exit sweep");

    assert_eq!(latest_event_kind(&state), "exited");
    assert_eq!(count_events(&state), 2);

    let connection = state.checkout().expect("connection");
    let exited: Option<String> = connection
        .query_row(
            "SELECT exited_at FROM short_positions WHERE company_id = ?1",
            [&company.id],
            |row| row.get(0),
        )
        .expect("stored position");
    assert!(exited.is_some());
}

#[test]
fn unchanged_snapshot_is_idempotent() {
    let (state, _company) = setup();
    let store = state.short_positions();
    let snapshot = [entry("AKO", CDR_ISIN, 0.53, "2026-07-14")];

    store
        .ingest_knf_short_positions(&snapshot)
        .expect("first poll");
    let outcome = store
        .ingest_knf_short_positions(&snapshot)
        .expect("re-poll");

    // Re-polling the identical snapshot detects zero changes.
    assert_eq!(outcome.items_created, 0);
    assert_eq!(count_events(&state), 1);
    assert_eq!(count_signals(&state), 1);
    assert_eq!(count_feed(&state), 1);
}

#[test]
fn untracked_company_is_skipped() {
    let (state, _company) = setup();

    let outcome = state
        .short_positions()
        .ingest_knf_short_positions(&[entry("AKO", "LU0000000000", 0.53, "2026-07-14")])
        .expect("ingest");

    assert_eq!(outcome.items_matched, 0);
    assert_eq!(outcome.items_unmatched, 1);
    assert_eq!(count_events(&state), 0);
    assert_eq!(count_signals(&state), 0);
    assert_eq!(count_feed(&state), 0);
}

/// Direct read-fn access with a fixed `today` so the 30-day window is
/// deterministic (the store method uses the real UTC date).
fn view_as_of(
    state: &AppState,
    company_id: &str,
    today: &str,
) -> crate::storage::ShortPositionsView {
    let connection = state.checkout().expect("connection");
    super::super::short_positions::load_short_positions_view(&connection, company_id, today)
        .expect("view should load")
}

#[test]
fn view_reports_active_positions_aggregate_and_delta() {
    let (state, company) = setup();
    let store = state.short_positions();

    // Two holders enter, one later grows — all within the 30-day window.
    store
        .ingest_knf_short_positions(&[
            entry(
                "Qube Research & Technologies Ltd",
                CDR_ISIN,
                1.49,
                "2026-06-28",
            ),
            entry("Marshall Wace LLP", CDR_ISIN, 0.61, "2026-07-02"),
        ])
        .expect("both enter");
    store
        .ingest_knf_short_positions(&[
            entry(
                "Qube Research & Technologies Ltd",
                CDR_ISIN,
                1.81,
                "2026-07-10",
            ),
            entry("Marshall Wace LLP", CDR_ISIN, 0.59, "2026-07-13"),
        ])
        .expect("qube increases, marshall decreases");

    let view = view_as_of(&state, &company.id, "2026-07-15");

    // Active positions, largest first, aggregate = 1.81 + 0.59.
    assert_eq!(view.positions.len(), 2);
    assert_eq!(
        view.positions[0].holder_name,
        "Qube Research & Technologies Ltd"
    );
    assert!((view.positions[0].net_position_pct - 1.81).abs() < 1e-9);
    assert!(view.positions[0].recently_changed, "changed within 30 days");
    assert!((view.aggregate_pct - 2.40).abs() < 1e-9);

    // delta_30d_pp = entered 1.49 + entered 0.61 + (1.81-1.49) + (0.59-0.61) = 2.40.
    assert!(
        (view.delta_30d_pp - 2.40).abs() < 1e-9,
        "delta was {}",
        view.delta_30d_pp
    );

    // History is newest-first by the domain date; the newest change is Marshall's.
    assert_eq!(view.events.len(), 4);
    assert_eq!(view.events[0].position_date, "2026-07-13");
    assert_eq!(view.events[0].kind, "decreased");
    assert!(view.last_exit.is_none());
}

#[test]
fn view_excludes_out_of_window_changes_from_delta() {
    let (state, company) = setup();
    let store = state.short_positions();

    store
        .ingest_knf_short_positions(&[entry("AKO Capital LLP", CDR_ISIN, 0.80, "2026-01-05")])
        .expect("entered long ago");

    // As of mid-July, the January entry is outside the 30-day window.
    let view = view_as_of(&state, &company.id, "2026-07-15");
    assert_eq!(view.positions.len(), 1);
    assert!((view.aggregate_pct - 0.80).abs() < 1e-9);
    assert!(
        view.delta_30d_pp.abs() < 1e-9,
        "no in-window change => zero delta, got {}",
        view.delta_30d_pp
    );
    assert!(
        !view.positions[0].recently_changed,
        "entry predates the window => no zmiana chip"
    );
}

#[test]
fn view_empty_for_company_with_no_positions() {
    let (state, company) = setup();

    let view = view_as_of(&state, &company.id, "2026-07-15");
    assert!(view.positions.is_empty());
    assert!(view.events.is_empty());
    assert!(view.last_exit.is_none());
    assert!(view.aggregate_pct.abs() < 1e-9);
    assert!(view.delta_30d_pp.abs() < 1e-9);
}

#[test]
fn view_remembers_last_exit_when_no_active_positions() {
    let (state, company) = setup();
    let store = state.short_positions();

    store
        .ingest_knf_short_positions(&[entry(
            "Point72 Asset Management",
            CDR_ISIN,
            0.62,
            "2024-10-20",
        )])
        .expect("entered");
    // The holder drops out of the register (below threshold).
    store.ingest_knf_short_positions(&[]).expect("exit sweep");

    let view = view_as_of(&state, &company.id, "2026-07-15");
    assert!(view.positions.is_empty(), "no active positions");
    let exit = view.last_exit.expect("remembers the last presence");
    assert_eq!(exit.holder_name, "Point72 Asset Management");
    assert_eq!(exit.exited_on.len(), 10, "date is YYYY-MM-DD");
}

#[test]
fn signal_category_rule_fires_on_entered_position() {
    let (state, company) = setup();

    state
        .attention()
        .create_alert_rule(NewAlertRule {
            trigger_type: "signal_category".to_owned(),
            signal_category: Some("short_position_change".to_owned()),
            price_min: None,
            price_max: None,
            scope_type: "company".to_owned(),
            scope_ref: company.id.clone(),
        })
        .expect("rule should be created");

    state
        .short_positions()
        .ingest_knf_short_positions(&[entry("AKO", CDR_ISIN, 0.53, "2026-07-14")])
        .expect("ingest should classify and evaluate");

    let events = state
        .attention()
        .list_attention_events(AttentionEventListInput::default())
        .expect("events should list");
    assert_eq!(events.len(), 1, "matching signal fires one attention event");
    assert_eq!(events[0].trigger_type, "signal_category");
    assert_eq!(events[0].evidence_type, "company_signal");
    assert_eq!(events[0].company_id, company.id);
}

#[test]
fn empty_register_snapshot_is_rejected_not_treated_as_mass_exit() {
    // Orchestrator guard (v0.55 T4 wiring): a transient empty response must not be
    // diffed — it would mark every stored holder `exited` and spray spurious
    // signals/alerts, then mirror-image `entered` spam on recovery.
    use crate::source_adapters::knf_short_selling::{
        refresh_with, KnfShortError, KnfShortSellingFetcher,
    };

    struct EmptyRegisterFetcher;

    impl KnfShortSellingFetcher for EmptyRegisterFetcher {
        fn fetch_register(&self, _source_url: &str) -> Result<String, KnfShortError> {
            Ok(r#"{"total":0,"records":[],"status":"success"}"#.to_owned())
        }
    }

    let (state, _company) = setup();
    state
        .short_positions()
        .ingest_knf_short_positions(&[entry("AKO", CDR_ISIN, 0.53, "2026-07-14")])
        .expect("seed one active position");
    let events_before = count_events(&state);

    let ctx = crate::jobs::source_refresh::RefreshContext {
        trigger: "test",
        date: None,
    };
    let outcome = refresh_with(&EmptyRegisterFetcher, &state, &ctx);

    let error = match outcome {
        Err(error) => error,
        Ok(_) => panic!("empty register snapshot must be rejected"),
    };
    assert!(error.contains("zero rows"), "unexpected error: {error}");
    assert_eq!(
        count_events(&state),
        events_before,
        "no exit events inferred"
    );
    assert_eq!(
        count(
            &state,
            "SELECT COUNT(*) FROM short_positions WHERE exited_at IS NOT NULL"
        ),
        0,
        "stored position must stay active"
    );
}
