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

fn insider_item(company: &Company, article_id: &str) -> BankierCompanyItem {
    BankierCompanyItem {
        company_id: company.id.clone(),
        qualified_ticker: company.qualified_ticker.clone(),
        title: "Powiadomienie o transakcjach, o których mowa w art. 19 ust. 1 MAR".to_owned(),
        link: format!("https://www.bankier.pl/wiadomosc/CD-PROJEKT-SA-{article_id}.html"),
        summary: "Komunikat ESPI/EBI".to_owned(),
        published_at: Some("2026-05-28T17:33:09".to_owned()),
        fetched_at: "2026-05-31T10:00:00Z".to_owned(),
        article_id: article_id.to_owned(),
        pub_id: 3,
        dedupe_key: format!("bankier-company-komunikaty:article:{article_id}"),
        duplicate_signature: format!("official-secondary:GPW:CDR:{article_id}"),
        body_text: Some("Official Bankier report body.".to_owned()),
        attachments: Vec::new(),
        detail_fetch_attempted: true,
    }
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
    assert_eq!(events[0].company_id, company.id);
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
fn signal_rule_daily_throttle_caps_one_event_per_day() {
    let connection = open_in_memory_database().expect("database should initialize");
    let state = AppState::new(connection);
    let company = tracked_company(&state);

    state
        .attention()
        .create_alert_rule(new_signal_rule(&company.id))
        .expect("rule");

    // Two DISTINCT insider filings on the same domain day (2026-05-28).
    state
        .ingest_bankier_company_items(&[
            insider_item(&company, "9300040"),
            insider_item(&company, "9300041"),
        ])
        .expect("ingestion");

    assert_eq!(
        events(&state).len(),
        1,
        "the per-rule daily throttle caps distinct same-day evidence at one event"
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
