//! Alert-rule evaluation + attention-event behavior (ADR 0068, plan §T2).

use super::*;
use crate::source_adapters::yahoo_eod::DailyQuote;

fn company(state: &AppState, ticker: &str, name: &str, isin: &str) -> Company {
    state
        .create_company(NewCompany {
            exchange: "GPW".to_owned(),
            ticker: ticker.to_owned(),
            display_name: name.to_owned(),
            isin: Some(isin.to_owned()),
            cik: None,
            lei: None,
        })
        .expect("company should be created")
}

fn tracked_company(state: &AppState) -> Company {
    company(state, "CDR", "CD PROJEKT S.A.", "PLOPTTC00011")
}

/// A date `n` days before wall-clock now, formatted as a publication timestamp
/// (`YYYY-MM-DDT12:00:00`). Tests compute dates relative to now so the
/// [`crate::storage::attention::SIGNAL_FRESHNESS_DAYS`] freshness gate stays
/// meaningful as the clock advances (a hard-coded date would silently drift past
/// the window and change what the test proves).
fn days_ago(n: i64) -> String {
    use time::macros::format_description;
    let format = format_description!("[year]-[month]-[day]");
    let day = (time::OffsetDateTime::now_utc() - time::Duration::days(n))
        .date()
        .format(&format)
        .expect("date should format");
    format!("{day}T12:00:00")
}

/// An insider filing published `days` days ago (fresh by default).
fn insider_item_dated(
    company: &Company,
    article_id: &str,
    published_at: &str,
) -> BankierCompanyItem {
    BankierCompanyItem {
        company_id: company.id.clone(),
        qualified_ticker: company.qualified_ticker.clone(),
        title: "Powiadomienie o transakcjach, o których mowa w art. 19 ust. 1 MAR".to_owned(),
        link: format!("https://www.bankier.pl/wiadomosc/CD-PROJEKT-SA-{article_id}.html"),
        summary: "Komunikat ESPI/EBI".to_owned(),
        published_at: Some(published_at.to_owned()),
        fetched_at: days_ago(0),
        article_id: article_id.to_owned(),
        pub_id: 3,
        dedupe_key: format!("bankier-company-komunikaty:article:{article_id}"),
        duplicate_signature: format!("official-secondary:GPW:CDR:{article_id}"),
        body_text: Some("Official Bankier report body.".to_owned()),
        attachments: Vec::new(),
        detail_fetch_attempted: true,
    }
}

/// A fresh insider filing (published 3 days ago) — inside the freshness window, so
/// it alerts. The default helper for the behavioral tests below.
fn insider_item(company: &Company, article_id: &str) -> BankierCompanyItem {
    insider_item_dated(company, article_id, &days_ago(3))
}

fn new_signal_rule(company_id: &str) -> NewAlertRule {
    NewAlertRule {
        trigger_type: "signal_category".to_owned(),
        signal_category: Some("insider_transaction".to_owned()),
        price_min: None,
        price_max: None,
        scope_type: "company".to_owned(),
        scope_ref: company_id.to_owned(),
    }
}

fn events(state: &AppState) -> Vec<AttentionEvent> {
    state
        .attention()
        .list_attention_events(AttentionEventListInput::default())
        .expect("events should list")
}

fn bar(date: &str, close: f64) -> DailyQuote {
    DailyQuote {
        date: date.to_owned(),
        open: close,
        high: close + 1.0,
        low: close - 1.0,
        close,
        volume: 1_000,
    }
}

/// Wall-clock now as RFC3339 — a fresh `fired_at` for events seeded via raw SQL,
/// so the severity aging demotion never fires during a title assertion.
fn fresh_fired_at() -> String {
    time::OffsetDateTime::now_utc()
        .format(&time::format_description::well_known::Rfc3339)
        .expect("format rfc3339")
}

// --- signal-category trigger -------------------------------------------------

#[test]
fn signal_category_rule_fires_on_matching_confirmed_signal() {
    let connection = open_in_memory_database().expect("database should initialize");
    let state = AppState::new(connection);
    let company = tracked_company(&state);

    state
        .attention()
        .create_alert_rule(new_signal_rule(&company.id))
        .expect("rule should be created");

    state
        .ingest_bankier_company_items(&[insider_item(&company, "9300001")])
        .expect("ingestion should classify and evaluate");

    let events = events(&state);
    assert_eq!(events.len(), 1, "matching signal fires one attention event");
    assert_eq!(events[0].trigger_type, "signal_category");
    assert_eq!(events[0].evidence_type, "company_signal");
    assert_eq!(events[0].company_id.as_deref(), Some(company.id.as_str()));
    assert!(!events[0].dismissed);
}

#[test]
fn signal_rule_does_not_fire_when_disabled() {
    let connection = open_in_memory_database().expect("database should initialize");
    let state = AppState::new(connection);
    let company = tracked_company(&state);

    let rule = state
        .attention()
        .create_alert_rule(new_signal_rule(&company.id))
        .expect("rule");
    state
        .attention()
        .set_alert_rule_enabled(&rule.id, false)
        .expect("disable");

    state
        .ingest_bankier_company_items(&[insider_item(&company, "9300010")])
        .expect("ingestion");

    assert!(events(&state).is_empty(), "a disabled rule never fires");
}

#[test]
fn signal_rule_does_not_fire_on_scope_mismatch() {
    let connection = open_in_memory_database().expect("database should initialize");
    let state = AppState::new(connection);
    let company_a = tracked_company(&state);
    let company_b = company(&state, "PKN", "PKN ORLEN S.A.", "PLPKN0000018");

    // Rule scoped to company B; the signal lands on company A.
    state
        .attention()
        .create_alert_rule(new_signal_rule(&company_b.id))
        .expect("rule");

    state
        .ingest_bankier_company_items(&[insider_item(&company_a, "9300020")])
        .expect("ingestion");

    assert!(
        events(&state).is_empty(),
        "a rule scoped to another company never fires"
    );
}

#[test]
fn signal_rule_dedups_across_reingestion() {
    let connection = open_in_memory_database().expect("database should initialize");
    let state = AppState::new(connection);
    let company = tracked_company(&state);

    state
        .attention()
        .create_alert_rule(new_signal_rule(&company.id))
        .expect("rule");

    let items = vec![insider_item(&company, "9300030")];
    state.ingest_bankier_company_items(&items).expect("first");
    state.ingest_bankier_company_items(&items).expect("second");

    assert_eq!(
        events(&state).len(),
        1,
        "re-ingesting the same evidence must not re-fire"
    );
}

#[test]
fn signal_rule_daily_throttle_coalesces_distinct_fresh_evidence_per_wall_clock_day() {
    let connection = open_in_memory_database().expect("database should initialize");
    let state = AppState::new(connection);
    let company = tracked_company(&state);

    state
        .attention()
        .create_alert_rule(new_signal_rule(&company.id))
        .expect("rule");

    // Two DISTINCT fresh insider filings with DIFFERENT domain dates (3 and 6 days
    // ago), ingested in the same pass. The throttle keys on the WALL-CLOCK
    // firing day, not the evidence's domain date, so distinct recent dates
    // coalesce to a single ping.
    state
        .ingest_bankier_company_items(&[
            insider_item_dated(&company, "9300040", &days_ago(3)),
            insider_item_dated(&company, "9300041", &days_ago(6)),
        ])
        .expect("ingestion");

    assert_eq!(
        events(&state).len(),
        1,
        "the per-rule throttle coalesces distinct same-wall-clock-day evidence at one event"
    );
}

#[test]
fn signal_rule_does_not_fire_for_stale_backfilled_evidence() {
    // Freshness gate (ADR 0068 amendment): a history backfill re-ingesting old
    // filings must raise NO alerts. An insider filing published 60 days ago — well
    // outside the 14-day window — fires nothing, even though a matching enabled
    // rule exists. This is the root-cause fix for the owner's toast wall.
    let connection = open_in_memory_database().expect("database should initialize");
    let state = AppState::new(connection);
    let company = tracked_company(&state);

    state
        .attention()
        .create_alert_rule(new_signal_rule(&company.id))
        .expect("rule");

    state
        .ingest_bankier_company_items(&[insider_item_dated(&company, "9300070", &days_ago(60))])
        .expect("ingestion");

    assert!(
        events(&state).is_empty(),
        "a stale (backfilled) signal must not fire an attention event"
    );
}

#[test]
fn signal_rule_fires_for_fresh_evidence_and_stamps_wall_clock_fired_at() {
    // The complement of the freshness gate: a fresh signal (3 days ago) DOES fire,
    // and its `fired_at` is the wall-clock firing day (today), never the evidence's
    // older domain date.
    let connection = open_in_memory_database().expect("database should initialize");
    let state = AppState::new(connection);
    let company = tracked_company(&state);

    state
        .attention()
        .create_alert_rule(new_signal_rule(&company.id))
        .expect("rule");

    state
        .ingest_bankier_company_items(&[insider_item_dated(&company, "9300071", &days_ago(3))])
        .expect("ingestion");

    let events = events(&state);
    assert_eq!(events.len(), 1, "a fresh signal fires one event");
    // `days_ago(0)` is the wall-clock day; its first 10 chars are today's date.
    let today = days_ago(0);
    assert_eq!(
        &events[0].fired_at[..10],
        &today[..10],
        "fired_at is the wall-clock firing day, not the evidence's domain date"
    );
}

#[test]
fn signal_rule_fires_for_watchlist_scope() {
    let connection = open_in_memory_database().expect("database should initialize");
    let state = AppState::new(connection);
    let company = tracked_company(&state);

    let watchlist = state
        .watchlists()
        .create_watchlist(NewWatchlist {
            name: "Core".to_owned(),
            description: None,
        })
        .expect("watchlist");
    state
        .watchlists()
        .add_company_to_watchlist(WatchlistCompanyInput {
            watchlist_id: watchlist.id.clone(),
            company_id: company.id.clone(),
        })
        .expect("membership");

    state
        .attention()
        .create_alert_rule(NewAlertRule {
            trigger_type: "signal_category".to_owned(),
            signal_category: Some("insider_transaction".to_owned()),
            price_min: None,
            price_max: None,
            scope_type: "watchlist".to_owned(),
            scope_ref: watchlist.id.clone(),
        })
        .expect("rule");

    state
        .ingest_bankier_company_items(&[insider_item(&company, "9300050")])
        .expect("ingestion");

    assert_eq!(
        events(&state).len(),
        1,
        "a watchlist-scoped rule fires for a member company"
    );
}

// --- autopilot trigger -------------------------------------------------------

#[test]
fn autopilot_completion_rule_fires_and_dedups() {
    let connection = open_in_memory_database().expect("database should initialize");
    let state = AppState::new(connection);
    let company = tracked_company(&state);

    state
        .attention()
        .create_alert_rule(NewAlertRule {
            trigger_type: "autopilot_run_completed".to_owned(),
            signal_category: None,
            price_min: None,
            price_max: None,
            scope_type: "company".to_owned(),
            scope_ref: company.id.clone(),
        })
        .expect("rule");

    let fired = state
        .attention()
        .evaluate_autopilot_completion(&company.id, "run_abc")
        .expect("evaluate");
    assert_eq!(fired, 1, "a completed run fires the rule");

    // Re-evaluating the same run does not re-fire.
    let again = state
        .attention()
        .evaluate_autopilot_completion(&company.id, "run_abc")
        .expect("evaluate");
    assert_eq!(again, 0, "same run is deduped");
    assert_eq!(events(&state).len(), 1);
}

// --- price triggers ----------------------------------------------------------

#[test]
fn price_enters_range_rule_fires_when_close_inside_band() {
    let connection = open_in_memory_database().expect("database should initialize");
    let state = AppState::new(connection);
    let company = tracked_company(&state);

    state
        .attention()
        .create_alert_rule(NewAlertRule {
            trigger_type: "price_enters_range".to_owned(),
            signal_category: None,
            price_min: Some(10.0),
            price_max: Some(20.0),
            scope_type: "company".to_owned(),
            scope_ref: company.id.clone(),
        })
        .expect("rule");

    // A close ABOVE the band does not fire.
    state
        .market_data()
        .upsert_quotes(
            &company.id,
            &[bar("2026-07-01", 25.0)],
            "yahoo-eod",
            "2026-07-01T18:00:00Z",
        )
        .expect("upsert");
    let fired = state
        .attention()
        .evaluate_price_rules(&company.id)
        .expect("evaluate");
    assert_eq!(fired, 0, "a close outside the band does not fire");

    // A later close INSIDE the band fires once.
    state
        .market_data()
        .upsert_quotes(
            &company.id,
            &[bar("2026-07-02", 15.0)],
            "yahoo-eod",
            "2026-07-02T18:00:00Z",
        )
        .expect("upsert");
    let fired = state
        .attention()
        .evaluate_price_rules(&company.id)
        .expect("evaluate");
    assert_eq!(fired, 1, "a close inside the band fires");
    assert_eq!(events(&state).len(), 1);

    // Re-evaluating the same latest bar does not re-fire.
    let again = state
        .attention()
        .evaluate_price_rules(&company.id)
        .expect("evaluate");
    assert_eq!(again, 0, "same-day close is deduped");
}

#[test]
fn price_week52_low_rule_fires_on_new_low() {
    let connection = open_in_memory_database().expect("database should initialize");
    let state = AppState::new(connection);
    let company = tracked_company(&state);

    state
        .attention()
        .create_alert_rule(NewAlertRule {
            trigger_type: "price_week52_low".to_owned(),
            signal_category: None,
            price_min: None,
            price_max: None,
            scope_type: "company".to_owned(),
            scope_ref: company.id.clone(),
        })
        .expect("rule");

    // Seed a window whose LATEST bar is not a new low (a lower close sits
    // earlier in the window), so evaluation does not fire yet.
    state
        .market_data()
        .upsert_quotes(
            &company.id,
            &[bar("2026-06-01", 90.0), bar("2026-06-15", 100.0)],
            "yahoo-eod",
            "2026-06-15T18:00:00Z",
        )
        .expect("upsert");
    let fired = state
        .attention()
        .evaluate_price_rules(&company.id)
        .expect("evaluate");
    assert_eq!(
        fired, 0,
        "a latest bar above the window low is not a new low"
    );

    state
        .market_data()
        .upsert_quotes(
            &company.id,
            &[bar("2026-07-01", 80.0)],
            "yahoo-eod",
            "2026-07-01T18:00:00Z",
        )
        .expect("upsert");
    let fired = state
        .attention()
        .evaluate_price_rules(&company.id)
        .expect("evaluate");
    assert_eq!(fired, 1, "a new 52-week low fires");
    assert_eq!(events(&state).len(), 1);
    assert_eq!(events(&state)[0].evidence_ref, "2026-07-01");
}

// --- trigger_type stamping (W4) ----------------------------------------------

#[test]
fn attention_event_writer_stamps_trigger_type_on_the_row() {
    // W4: `trigger_type` is stamped directly on the row so it can be read
    // directly — without joining `alert_rules` — by grouping/diagnostics.
    let connection = open_in_memory_database().expect("database should initialize");
    connection
        .execute(
            "INSERT INTO companies (id, exchange, ticker, qualified_ticker, display_name)
             VALUES ('c1', 'GPW', 'CDR', 'GPW:CDR', 'CD PROJEKT S.A.')",
            [],
        )
        .expect("seed company");
    connection
        .execute(
            "INSERT INTO alert_rules (id, trigger_type, signal_category, scope_type, scope_ref)
             VALUES ('r1', 'signal_category', 'insider_transaction', 'company', 'c1')",
            [],
        )
        .expect("seed rule");

    let created = crate::storage::attention::insert_attention_event(
        &connection,
        "r1",
        "signal_category",
        "c1",
        crate::storage::attention::EVIDENCE_COMPANY_SIGNAL,
        "sig1",
        None,
    )
    .expect("insert event");
    assert!(created, "a new event row is created");

    let stored: Option<String> = connection
        .query_row(
            "SELECT trigger_type FROM attention_events WHERE rule_id = 'r1'",
            [],
            |row| row.get(0),
        )
        .expect("read stored trigger_type");
    assert_eq!(
        stored.as_deref(),
        Some("signal_category"),
        "the writer stamps trigger_type directly on the row, not NULL"
    );
}

// --- event lifecycle ---------------------------------------------------------

#[test]
fn mark_seen_and_dismiss_update_event_flags() {
    let connection = open_in_memory_database().expect("database should initialize");
    let state = AppState::new(connection);
    let company = tracked_company(&state);

    state
        .attention()
        .create_alert_rule(new_signal_rule(&company.id))
        .expect("rule");
    state
        .ingest_bankier_company_items(&[insider_item(&company, "9300060")])
        .expect("ingestion");

    let event = events(&state).remove(0);
    assert!(!event.seen && !event.dismissed);

    state
        .attention()
        .mark_attention_event_seen(&event.id)
        .expect("mark seen");
    assert!(events(&state)[0].seen);

    state
        .attention()
        .dismiss_attention_event(&event.id)
        .expect("dismiss");
    // Dismissed events are hidden from the default list.
    assert!(events(&state).is_empty());
    let all = state
        .attention()
        .list_attention_events(AttentionEventListInput {
            company_id: None,
            include_dismissed: true,
        })
        .expect("list all");
    assert!(all[0].dismissed);
}

#[test]
fn batch_mark_seen_flags_exactly_the_given_ids() {
    // ADR 0097 dec. 5: Today marks every loaded unseen event in ONE call when
    // its stream renders, so the sidebar badge clears on a visit. Rows are
    // seeded raw (system events, no rule) — the per-rule daily throttle would
    // otherwise cap an ingestion at one event.
    let connection = open_in_memory_database().expect("database should initialize");
    connection
        .execute(
            "INSERT INTO companies (id, exchange, ticker, qualified_ticker, display_name)
             VALUES ('c1', 'GPW', 'CDR', 'GPW:CDR', 'CD PROJEKT S.A.')",
            [],
        )
        .expect("seed company");
    for n in 1..=3 {
        crate::storage::attention::insert_system_attention_event(
            &connection,
            crate::storage::attention::TRIGGER_SOURCE_RECONCILIATION,
            Some("c1"),
            crate::storage::attention::EVIDENCE_SOURCE_RECONCILIATION,
            &format!("recon_{n}"),
            Some("Raport bieżący"),
            &format!("2026-08-0{n}T00:00:00Z"),
        )
        .expect("seed event");
    }
    let state = AppState::new(connection);

    let listed = events(&state);
    assert_eq!(listed.len(), 3, "three seeded events");
    assert!(listed.iter().all(|event| !event.seen));

    // Mark all but the first, plus an unknown id (ignored, not an error).
    let mut ids: Vec<String> = listed
        .iter()
        .skip(1)
        .map(|event| event.id.clone())
        .collect();
    ids.push("no_such_event".to_owned());
    state
        .attention()
        .mark_attention_events_seen(&ids)
        .expect("batch mark seen");

    let after = events(&state);
    let seen_of = |id: &str| after.iter().find(|event| event.id == id).expect("row").seen;
    assert!(!seen_of(&listed[0].id), "unlisted event stays unseen");
    for event in listed.iter().skip(1) {
        assert!(seen_of(&event.id), "listed event is seen");
    }

    // An empty batch is a no-op, never an error.
    state
        .attention()
        .mark_attention_events_seen(&[])
        .expect("empty batch is fine");

    // A batch far past the SQLite bound-parameter floor (999) must not error:
    // the statement is chunked, so an unbounded unseen backlog can never fail
    // the very command meant to clear it. The unlisted event stays untouched.
    let mut oversized: Vec<String> = (0..2_500).map(|n| format!("bulk_{n}")).collect();
    oversized.push(listed[0].id.clone());
    state
        .attention()
        .mark_attention_events_seen(&oversized)
        .expect("oversized batch is chunked, never a parameter-limit error");
    assert!(
        events(&state).iter().all(|event| event.seen),
        "the real id buried in the oversized batch was marked"
    );
}

// --- rule id lifecycle (regression: live crash 2026-07-15) --------------------

#[test]
fn create_after_delete_never_reuses_an_id() {
    // Live bug: ids were COUNT+1, so deleting a rule made the next create
    // collide with a survivor's id (UNIQUE constraint failed: alert_rules.id).
    let connection = open_in_memory_database().expect("database should initialize");
    let state = AppState::new(connection);
    let company = tracked_company(&state);

    let first = state
        .attention()
        .create_alert_rule(new_signal_rule(&company.id))
        .expect("first rule");
    let mut second_input = new_signal_rule(&company.id);
    second_input.signal_category = Some("profit_warning".to_owned());
    let second = state
        .attention()
        .create_alert_rule(second_input)
        .expect("second rule");
    state
        .attention()
        .delete_alert_rule(&first.id)
        .expect("delete first");

    let mut third_input = new_signal_rule(&company.id);
    third_input.trigger_type = "autopilot_run_completed".to_owned();
    third_input.signal_category = None;
    let third = state
        .attention()
        .create_alert_rule(third_input)
        .expect("create after delete must not collide with a surviving id");
    assert_ne!(third.id, second.id);
}

#[test]
fn creating_an_identical_rule_is_a_typed_duplicate_error() {
    // The same trigger+scope(+prices) twice is meaningless duplication — a
    // typed error, never a raw sqlite failure and never a silent twin row.
    let connection = open_in_memory_database().expect("database should initialize");
    let state = AppState::new(connection);
    let company = tracked_company(&state);

    state
        .attention()
        .create_alert_rule(new_signal_rule(&company.id))
        .expect("first rule");
    let error = state
        .attention()
        .create_alert_rule(new_signal_rule(&company.id))
        .expect_err("identical rule must be rejected");
    assert!(
        matches!(error, StorageError::DuplicateAlertRule { .. }),
        "expected DuplicateAlertRule, got: {error:?}"
    );

    let rules = state.attention().list_alert_rules().expect("list");
    assert_eq!(rules.len(), 1, "no twin row was inserted");
}

// --- evidence specifics (v0.60 D6: every row states WHAT happened) ------------

#[test]
fn list_attention_events_carries_the_signal_filing_title_as_evidence_title() {
    // A signal-category event must state its own filing title (the specific
    // statement), not just its category.
    let connection = open_in_memory_database().expect("database should initialize");
    let state = AppState::new(connection);
    let company = tracked_company(&state);

    state
        .attention()
        .create_alert_rule(new_signal_rule(&company.id))
        .expect("rule");
    state
        .ingest_bankier_company_items(&[insider_item(&company, "9300080")])
        .expect("ingestion classifies and fires");

    let events = events(&state);
    assert_eq!(events.len(), 1, "one insider event fired");
    assert_eq!(
        events[0].evidence_title.as_deref(),
        Some("Powiadomienie o transakcjach, o których mowa w art. 19 ust. 1 MAR"),
        "a signal event carries its filing title as the concrete statement"
    );
    assert_eq!(
        events[0].evidence_detail, None,
        "a signal event has no secondary detail datum"
    );
}

#[test]
fn list_attention_events_carries_reconciliation_title_and_source_name() {
    // A missed-report reconciliation event must state the missed report's title
    // and the source that missed it (owner: "jaki dokładniej raport? które źródło?").
    let connection = open_in_memory_database().expect("database should initialize");
    connection
        .execute(
            "INSERT INTO companies (id, exchange, ticker, qualified_ticker, display_name)
             VALUES ('c1', 'GPW', 'CDR', 'GPW:CDR', 'CD PROJEKT S.A.')",
            [],
        )
        .expect("seed company");
    connection
        .execute(
            "INSERT INTO source_reconciliation_results
                (id, witness_adapter_id, company_id, disclosure_date, witness_title, status)
             VALUES ('recon1', 'gpw-espi-ebi', 'c1', '2026-07-14',
                     'Raport bieżący 15/2026 — zawarcie znaczącej umowy', 'espi_only')",
            [],
        )
        .expect("seed reconciliation result");
    let fired_at = fresh_fired_at();
    connection
        .execute(
            "INSERT INTO attention_events
                (id, rule_id, trigger_type, company_id, evidence_type, evidence_ref, fired_at)
             VALUES ('attn1', NULL, 'source_reconciliation', 'c1',
                     'source_reconciliation', 'recon1', ?1)",
            [&fired_at],
        )
        .expect("seed reconciliation event");

    let events = crate::storage::attention::list_attention_events(
        &connection,
        AttentionEventListInput::default(),
    )
    .expect("events");
    assert_eq!(events.len(), 1);
    assert_eq!(
        events[0].evidence_title.as_deref(),
        Some("Raport bieżący 15/2026 — zawarcie znaczącej umowy"),
        "a reconciliation event carries the missed report's witness title"
    );
    assert_eq!(
        events[0].evidence_detail.as_deref(),
        Some("GPW ESPI/EBI"),
        "a reconciliation event carries the missed source's display name (adapter id resolved)"
    );
}

#[test]
fn list_attention_events_reconciliation_title_falls_back_to_report_type_and_number() {
    // Live gap (owner ledger 2026-07-23): the GPW registry parser leaves
    // `witness_title` empty on fresh rows, but `report_type` + `report_number`
    // ARE parsed — and they answer "jaki raport?". The read model falls back to
    // their concatenation (two raw registry strings, no invented words).
    let connection = open_in_memory_database().expect("database should initialize");
    connection
        .execute(
            "INSERT INTO companies (id, exchange, ticker, qualified_ticker, display_name)
             VALUES ('c1', 'GPW', 'CDR', 'GPW:CDR', 'CD PROJEKT S.A.')",
            [],
        )
        .expect("seed company");
    // Empty witness_title, but report_type + report_number present.
    connection
        .execute(
            "INSERT INTO source_reconciliation_results
                (id, witness_adapter_id, company_id, disclosure_date,
                 witness_title, report_type, report_number, status)
             VALUES ('recon1', 'gpw-espi-ebi', 'c1', '2026-07-14',
                     '', 'Raport bieżący', '15/2026', 'espi_only')",
            [],
        )
        .expect("seed titleless reconciliation");
    let fired_at = fresh_fired_at();
    connection
        .execute(
            "INSERT INTO attention_events
                (id, rule_id, trigger_type, company_id, evidence_type, evidence_ref, fired_at)
             VALUES ('attn1', NULL, 'source_reconciliation', 'c1',
                     'source_reconciliation', 'recon1', ?1)",
            [&fired_at],
        )
        .expect("seed event");

    let events = crate::storage::attention::list_attention_events(
        &connection,
        AttentionEventListInput::default(),
    )
    .expect("events");
    assert_eq!(events.len(), 1);
    assert_eq!(
        events[0].evidence_title.as_deref(),
        Some("Raport bieżący 15/2026"),
        "an empty witness_title falls back to report_type + report_number"
    );
    assert_eq!(events[0].evidence_detail.as_deref(), Some("GPW ESPI/EBI"));
}

#[test]
fn list_attention_events_reconciliation_title_none_when_no_title_source_at_all() {
    // Neither a witness_title nor report_type/number → NULL, so the frontend keeps
    // its generic reconciliation copy (tolerant fallback, never blank/crash).
    let connection = open_in_memory_database().expect("database should initialize");
    connection
        .execute(
            "INSERT INTO companies (id, exchange, ticker, qualified_ticker, display_name)
             VALUES ('c1', 'GPW', 'CDR', 'GPW:CDR', 'CD PROJEKT S.A.')",
            [],
        )
        .expect("seed company");
    connection
        .execute(
            "INSERT INTO source_reconciliation_results
                (id, witness_adapter_id, company_id, disclosure_date, witness_title, status)
             VALUES ('recon1', 'gpw-espi-ebi', 'c1', '2026-07-14', '', 'espi_only')",
            [],
        )
        .expect("seed empty reconciliation");
    let fired_at = fresh_fired_at();
    connection
        .execute(
            "INSERT INTO attention_events
                (id, rule_id, trigger_type, company_id, evidence_type, evidence_ref, fired_at)
             VALUES ('attn1', NULL, 'source_reconciliation', 'c1',
                     'source_reconciliation', 'recon1', ?1)",
            [&fired_at],
        )
        .expect("seed event");

    let events = crate::storage::attention::list_attention_events(
        &connection,
        AttentionEventListInput::default(),
    )
    .expect("events");
    assert_eq!(events.len(), 1);
    assert_eq!(
        events[0].evidence_title, None,
        "no title source → NULL → generic FE fallback"
    );
}

#[test]
fn list_attention_events_carries_autopilot_run_document_title_and_status() {
    // An autopilot-completion event must name the report the run processed and
    // how it ended (owner: "Autopilot zakończony → co to oznacza?").
    let connection = open_in_memory_database().expect("database should initialize");
    connection
        .execute(
            "INSERT INTO companies (id, exchange, ticker, qualified_ticker, display_name)
             VALUES ('c1', 'GPW', 'CDR', 'GPW:CDR', 'CD PROJEKT S.A.')",
            [],
        )
        .expect("seed company");
    connection
        .execute(
            "INSERT INTO report_documents (id, company_id, source_type, url, title)
             VALUES ('doc1', 'c1', 'user_url', 'https://example.test/doc1',
                     'Skonsolidowany raport kwartalny Q2 2026')",
            [],
        )
        .expect("seed report document");
    connection
        .execute(
            "INSERT INTO autopilot_run (id, company_id, report_document_id, trigger, mode, status)
             VALUES ('run1', 'c1', 'doc1', 'manual', 'assist', 'succeeded')",
            [],
        )
        .expect("seed run");
    let fired_at = fresh_fired_at();
    connection
        .execute(
            "INSERT INTO attention_events
                (id, rule_id, trigger_type, company_id, evidence_type, evidence_ref, fired_at)
             VALUES ('attn1', NULL, 'autopilot_run_completed', 'c1',
                     'autopilot_run', 'run1', ?1)",
            [&fired_at],
        )
        .expect("seed autopilot event");

    let events = crate::storage::attention::list_attention_events(
        &connection,
        AttentionEventListInput::default(),
    )
    .expect("events");
    assert_eq!(events.len(), 1);
    assert_eq!(
        events[0].evidence_title.as_deref(),
        Some("Skonsolidowany raport kwartalny Q2 2026"),
        "an autopilot event names the processed report document"
    );
    assert_eq!(
        events[0].evidence_detail.as_deref(),
        Some("succeeded"),
        "an autopilot event carries the run's raw status for the frontend to translate"
    );
}

#[test]
fn list_attention_events_resolves_a_failed_jobs_kind_and_error() {
    // Epic #40 S3 / ADR 0091 dec. 1: a `job_failed` event's evidence_ref is the
    // `job_queue.id`, and the guarded LEFT JOIN resolves the job row so the stream
    // can state WHICH job failed and HOW. Raw source data only (ADR 0087 dec. 4):
    // the kind is the enum token the frontend translates, the failure text is the
    // queue's own `last_error`.
    let connection = open_in_memory_database().expect("database should initialize");
    connection
        .execute(
            "INSERT INTO companies (id, exchange, ticker, qualified_ticker, display_name)
             VALUES ('c1', 'GPW', 'CDR', 'GPW:CDR', 'CD PROJEKT S.A.')",
            [],
        )
        .expect("seed company");
    connection
        .execute(
            "INSERT INTO job_queue (id, kind, payload, status, attempts, max_attempts, last_error)
             VALUES ('job1', 'quote_backfill', '{\"companyId\":\"c1\"}', 'failed', 3, 3,
                     'HTTP 503 from query1.finance.yahoo.com')",
            [],
        )
        .expect("seed terminally failed job");
    let fired_at = fresh_fired_at();
    connection
        .execute(
            "INSERT INTO attention_events
                (id, rule_id, trigger_type, company_id, evidence_type, evidence_ref, fired_at)
             VALUES ('attn_job', NULL, 'job_failed', 'c1', 'job', 'job1', ?1)",
            [&fired_at],
        )
        .expect("seed job-failure event");

    let events = crate::storage::attention::list_attention_events(
        &connection,
        AttentionEventListInput::default(),
    )
    .expect("events");
    assert_eq!(events.len(), 1);
    assert_eq!(
        events[0].severity,
        AttentionSeverity::Notable,
        "a terminal job failure is notable"
    );
    assert_eq!(
        events[0].evidence_title.as_deref(),
        Some("HTTP 503 from query1.finance.yahoo.com"),
        "with no fire-time subject, the job's own last_error is the statement"
    );
    assert_eq!(
        events[0].evidence_detail.as_deref(),
        Some("quote_backfill"),
        "the raw job kind travels as the detail for the frontend to translate"
    );
}

#[test]
fn list_attention_events_prefers_the_failure_subject_over_the_raw_error() {
    // The fire-time subject snapshot (`JobHandler::failure_subject`) is the better
    // statement when the job had one — a document title / ticker beats a transport
    // error string. The error stays reachable through the job row itself.
    let connection = open_in_memory_database().expect("database should initialize");
    connection
        .execute(
            "INSERT INTO job_queue (id, kind, payload, status, attempts, max_attempts, last_error)
             VALUES ('job2', 'ownership_extraction', '{}', 'failed', 3, 3, 'pdf parse failed')",
            [],
        )
        .expect("seed job");
    let fired_at = fresh_fired_at();
    connection
        .execute(
            "INSERT INTO attention_events
                (id, rule_id, trigger_type, company_id, evidence_type, evidence_ref, fired_at,
                 evidence_title)
             VALUES ('attn_job2', NULL, 'job_failed', NULL, 'job', 'job2', ?1,
                     'Skonsolidowany raport kwartalny Q2 2026')",
            [&fired_at],
        )
        .expect("seed job-failure event with a subject");

    let events = crate::storage::attention::list_attention_events(
        &connection,
        AttentionEventListInput::default(),
    )
    .expect("events");
    assert_eq!(events.len(), 1);
    assert_eq!(
        events[0].company_id, None,
        "a system-scoped job failure carries no company (migration 0118)"
    );
    assert_eq!(
        events[0].evidence_title.as_deref(),
        Some("Skonsolidowany raport kwartalny Q2 2026"),
        "the fire-time subject wins over the raw error"
    );
    assert_eq!(
        events[0].evidence_detail.as_deref(),
        Some("ownership_extraction")
    );
}

#[test]
fn list_attention_events_leaves_evidence_title_none_for_pruned_evidence() {
    // Tolerant fallback: an event whose evidence row is gone (legacy/pruned)
    // never crashes and never blanks — it simply has no title.
    let connection = open_in_memory_database().expect("database should initialize");
    connection
        .execute(
            "INSERT INTO companies (id, exchange, ticker, qualified_ticker, display_name)
             VALUES ('c1', 'GPW', 'CDR', 'GPW:CDR', 'CD PROJEKT S.A.')",
            [],
        )
        .expect("seed company");
    let fired_at = fresh_fired_at();
    connection
        .execute(
            "INSERT INTO attention_events
                (id, rule_id, trigger_type, company_id, evidence_type, evidence_ref, fired_at)
             VALUES ('attn1', NULL, 'source_reconciliation', 'c1',
                     'source_reconciliation', 'gone', ?1)",
            [&fired_at],
        )
        .expect("seed event with missing evidence");

    let events = crate::storage::attention::list_attention_events(
        &connection,
        AttentionEventListInput::default(),
    )
    .expect("events");
    assert_eq!(events.len(), 1);
    assert_eq!(events[0].evidence_title, None);
    assert_eq!(events[0].evidence_detail, None);
}

#[test]
fn legacy_pruned_database_still_renders_the_durable_evidence_title_snapshot() {
    // Legacy-state compatibility (v0.60 D7 snapshot, reframed after #329):
    // databases pruned by the RETIRED feed cleanup hold attention events whose
    // feed item — and, via `company_signals.feed_item_id` ON DELETE CASCADE,
    // their signal row — are gone. The read must render the fire-time
    // `evidence_title` snapshot, never a bare category from a dead join.
    const INSIDER_TITLE: &str = "Powiadomienie o transakcjach, o których mowa w art. 19 ust. 1 MAR";
    let connection = open_in_memory_database().expect("database should initialize");
    connection
        .execute(
            "INSERT INTO companies (id, exchange, ticker, qualified_ticker, display_name)
             VALUES ('c1', 'GPW', 'XTB', 'GPW:XTB', 'XTB S.A.')",
            [],
        )
        .expect("seed company");
    connection
        .execute(
            "INSERT INTO alert_rules (id, trigger_type, signal_category, scope_type, scope_ref)
             VALUES ('r1', 'signal_category', 'insider_transaction', 'company', 'c1')",
            [],
        )
        .expect("seed rule");
    // The signal's feed item was pruned pre-#329, cascading the signal away —
    // only the event row survives, with the snapshot title stamped at fire time.
    connection
        .execute(
            "INSERT INTO attention_events
                (id, rule_id, trigger_type, company_id, evidence_type, evidence_ref, fired_at,
                 evidence_title)
             VALUES ('attn1', 'r1', 'signal_category', 'c1', 'company_signal', 'sig_gone',
                     '2026-07-20T09:00:00Z', ?1)",
            [INSIDER_TITLE],
        )
        .expect("seed legacy event with a dead signal ref");

    let events = crate::storage::attention::list_attention_events(
        &connection,
        AttentionEventListInput::default(),
    )
    .expect("events");
    assert_eq!(events.len(), 1, "the legacy event is listed");
    assert_eq!(
        events[0].evidence_title.as_deref(),
        Some(INSIDER_TITLE),
        "the fire-time snapshot renders although the signal and feed item are long gone"
    );
}
