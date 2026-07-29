//! Typed severity taxonomy (ADR 0087 dec. 2): the single backend mapping + the
//! classification gate. The gate derives its trigger/category inventory from the
//! real sources of truth (`attention::TRIGGER_TYPES` + the system trigger, and the
//! seeded `signal_categories` registry), so a new trigger type or category added
//! without a severity classification reddens here — same posture as the MCP
//! registry classification gate (ADR 0088).

use super::*;
use crate::source_adapters::yahoo_eod::DailyQuote;
use crate::storage::attention::{
    TRIGGER_JOB_FAILED, TRIGGER_SIGNAL_CATEGORY, TRIGGER_SOURCE_RECONCILIATION, TRIGGER_TYPES,
};
use crate::storage::severity::{
    aged_attention_severity, classify_signal_category, severity_for_trigger, URGENT_AGING_THRESHOLD,
};

// --- the classification gate -------------------------------------------------

#[test]
fn every_trigger_type_is_classified() {
    // The full trigger inventory: the user-creatable triggers plus the system
    // triggers (reconciliation, terminal job failure). `signal_category` is
    // category-dependent (covered by `every_seeded_signal_category_is_classified`);
    // every other trigger must have an explicit severity arm, so a NEW trigger
    // reddens until it is classified.
    let triggers: Vec<&str> = TRIGGER_TYPES
        .iter()
        .copied()
        .chain([TRIGGER_SOURCE_RECONCILIATION, TRIGGER_JOB_FAILED])
        .collect();

    let unclassified: Vec<&str> = triggers
        .iter()
        .copied()
        .filter(|trigger| *trigger != TRIGGER_SIGNAL_CATEGORY)
        .filter(|trigger| severity_for_trigger(trigger).is_none())
        .collect();

    assert!(
        unclassified.is_empty(),
        "these trigger types have no severity classification (ADR 0087 dec. 2 — \
         classify them in storage::severity): {unclassified:?}"
    );
}

#[test]
fn every_seeded_signal_category_is_classified() {
    // The category inventory is derived from the REAL source of truth — the seeded
    // `signal_categories` registry in a migrated DB — never a hand-copied list, so
    // a new category added by a migration reddens until it is classified.
    let connection = open_in_memory_database().expect("database should initialize");
    let mut statement = connection
        .prepare("SELECT key FROM signal_categories ORDER BY key")
        .expect("prepare");
    let categories: Vec<String> = statement
        .query_map([], |row| row.get::<_, String>(0))
        .expect("query")
        .collect::<Result<Vec<_>, _>>()
        .expect("collect");

    assert!(
        categories.len() >= 15,
        "sanity: expected the full seeded taxonomy, got {}",
        categories.len()
    );

    let unclassified: Vec<&String> = categories
        .iter()
        .filter(|key| classify_signal_category(key).is_none())
        .collect();

    assert!(
        unclassified.is_empty(),
        "these seeded signal categories have no severity classification (ADR 0087 \
         dec. 2 — classify them in storage::severity): {unclassified:?}"
    );
}

// --- the mapping (unit) ------------------------------------------------------

#[test]
fn signal_category_severity_matches_the_taxonomy() {
    use AttentionSeverity::*;
    let cases = [
        ("insider_transaction", Urgent),
        ("profit_warning", Urgent),
        ("auditor_opinion", Urgent),
        ("dividend", Notable),
        ("significant_contract", Notable),
        ("guidance_change", Notable),
        ("general_meeting", Notable),
        ("own_shares", Notable),
        ("short_position_change", Notable),
        ("major_holdings_change", Notable),
        ("report_delay", Notable),
        ("fund_exit", Notable),
        ("score_deterioration", Notable),
        ("recommendation_change", Notable),
        ("other", Routine),
    ];
    for (category, expected) in cases {
        assert_eq!(
            severity_for_attention_event(TRIGGER_SIGNAL_CATEGORY, Some(category)),
            expected,
            "signal category {category} should map to {expected:?}"
        );
    }
}

#[test]
fn unknown_or_absent_category_is_notable_never_routine() {
    assert_eq!(
        severity_for_attention_event(TRIGGER_SIGNAL_CATEGORY, None),
        AttentionSeverity::Notable,
        "an absent category is notable, never silently routine"
    );
    assert_eq!(
        severity_for_attention_event(TRIGGER_SIGNAL_CATEGORY, Some("brand_new_unseen")),
        AttentionSeverity::Notable,
        "an unknown category is notable, never silently routine"
    );
}

#[test]
fn non_signal_trigger_severities_match_the_taxonomy() {
    use AttentionSeverity::*;
    assert_eq!(
        severity_for_attention_event(TRIGGER_SOURCE_RECONCILIATION, None),
        Urgent,
        "a missed-report reconciliation is urgent"
    );
    assert_eq!(
        severity_for_attention_event("price_enters_range", None),
        Notable
    );
    assert_eq!(
        severity_for_attention_event("price_week52_low", None),
        Notable
    );
    assert_eq!(
        severity_for_attention_event("autopilot_run_completed", None),
        Notable
    );
    // A terminally failed background job is Notable for EVERY kind (ADR 0091 dec. 1,
    // owner decision): worth surfacing in the stream, never a persistent alarm.
    assert_eq!(
        severity_for_attention_event(TRIGGER_JOB_FAILED, None),
        Notable,
        "a terminal job failure is notable for every job kind"
    );
}

#[test]
fn autopilot_run_severity_matches_the_taxonomy() {
    use AttentionSeverity::*;
    assert_eq!(severity_for_autopilot_run("failed"), Notable);
    assert_eq!(severity_for_autopilot_run("partial"), Notable);
    assert_eq!(severity_for_autopilot_run("succeeded"), Routine);
    assert_eq!(severity_for_autopilot_run("pending"), Routine);
    assert_eq!(severity_for_autopilot_run("running"), Routine);
}

// --- urgency ages (ADR 0087 dec. 2 amendment, 2026-07-23 live-checkpoint) -----

#[test]
fn aged_attention_severity_demotes_a_stale_urgent_to_notable() {
    use time::format_description::well_known::Rfc3339;
    use AttentionSeverity::*;
    let now = time::OffsetDateTime::now_utc();
    let four_days_ago = (now - time::Duration::days(4))
        .format(&Rfc3339)
        .expect("format");
    let one_hour_ago = (now - time::Duration::hours(1))
        .format(&Rfc3339)
        .expect("format");

    assert_eq!(
        aged_attention_severity(Urgent, &four_days_ago, now),
        Notable,
        "a 4-day-old urgent event demotes to notable"
    );
    assert_eq!(
        aged_attention_severity(Urgent, &one_hour_ago, now),
        Urgent,
        "a fresh urgent event stays urgent"
    );
}

#[test]
fn aged_attention_severity_boundary_at_72h_belongs_to_urgent() {
    use time::format_description::well_known::Rfc3339;
    use AttentionSeverity::*;
    let now = time::OffsetDateTime::now_utc();
    let exactly_72h = (now - URGENT_AGING_THRESHOLD)
        .format(&Rfc3339)
        .expect("format");
    let just_past_72h = (now - URGENT_AGING_THRESHOLD - time::Duration::seconds(1))
        .format(&Rfc3339)
        .expect("format");

    assert_eq!(
        aged_attention_severity(Urgent, &exactly_72h, now),
        Urgent,
        "at exactly 72h the event still shouts — the boundary belongs to urgent"
    );
    assert_eq!(
        aged_attention_severity(Urgent, &just_past_72h, now),
        Notable,
        "one second past 72h demotes"
    );
}

#[test]
fn aged_attention_severity_only_touches_urgent_and_tolerates_bad_input() {
    use time::format_description::well_known::Rfc3339;
    use AttentionSeverity::*;
    let now = time::OffsetDateTime::now_utc();
    let ancient = (now - time::Duration::days(30))
        .format(&Rfc3339)
        .expect("format");

    // A non-urgent base never changes, however old.
    assert_eq!(aged_attention_severity(Notable, &ancient, now), Notable);
    assert_eq!(aged_attention_severity(Routine, &ancient, now), Routine);
    // An unparseable fired_at never demotes (we never demote what we can't prove is old).
    assert_eq!(aged_attention_severity(Urgent, "not-a-date", now), Urgent);
}

#[test]
fn list_attention_events_demotes_a_stale_unacted_urgent_reconciliation() {
    // Purely age-based: an UNSEEN, undismissed 5-day-old urgent reconciliation
    // reads notable, while a fresh reconciliation reads urgent — proving the read
    // path (`list_attention_events`) applies the aging demotion.
    use time::format_description::well_known::Rfc3339;
    let connection = open_in_memory_database().expect("database should initialize");
    connection
        .execute(
            "INSERT INTO companies (id, exchange, ticker, qualified_ticker, display_name)
             VALUES ('c1', 'GPW', 'CDR', 'GPW:CDR', 'CD PROJEKT S.A.')",
            [],
        )
        .expect("seed company");

    let now = time::OffsetDateTime::now_utc();
    let five_days_ago = (now - time::Duration::days(5))
        .format(&Rfc3339)
        .expect("format");
    let fresh = now.format(&Rfc3339).expect("format");

    // A stale reconciliation event — system-raised (no rule), unseen, undismissed.
    connection
        .execute(
            "INSERT INTO attention_events
                (id, rule_id, trigger_type, company_id, evidence_type, evidence_ref, fired_at, seen, dismissed)
             VALUES ('stale', NULL, 'source_reconciliation', 'c1', 'source_reconciliation', 'ev_old', ?1, 0, 0)",
            [&five_days_ago],
        )
        .expect("seed stale event");
    connection
        .execute(
            "INSERT INTO attention_events
                (id, rule_id, trigger_type, company_id, evidence_type, evidence_ref, fired_at, seen, dismissed)
             VALUES ('fresh', NULL, 'source_reconciliation', 'c1', 'source_reconciliation', 'ev_new', ?1, 0, 0)",
            [&fresh],
        )
        .expect("seed fresh event");

    let events = crate::storage::attention::list_attention_events(
        &connection,
        AttentionEventListInput::default(),
    )
    .expect("events");

    let severity_of = |id: &str| {
        events
            .iter()
            .find(|event| event.id == id)
            .map(|event| event.severity)
            .expect("event present")
    };
    assert_eq!(
        severity_of("stale"),
        AttentionSeverity::Notable,
        "a 5-day-old unacted urgent reconciliation stops shouting (notable)"
    );
    assert_eq!(
        severity_of("fresh"),
        AttentionSeverity::Urgent,
        "a fresh reconciliation still leads (urgent)"
    );
}

// --- read models carry the computed severity ---------------------------------

fn company(state: &AppState) -> Company {
    state
        .create_company(NewCompany {
            exchange: "GPW".to_owned(),
            ticker: "CDR".to_owned(),
            display_name: "CD PROJEKT S.A.".to_owned(),
            isin: Some("PLOPTTC00011".to_owned()),
            cik: None,
            lei: None,
        })
        .expect("company should create")
}

fn days_ago(n: i64) -> String {
    use time::macros::format_description;
    let format = format_description!("[year]-[month]-[day]");
    let day = (time::OffsetDateTime::now_utc() - time::Duration::days(n))
        .date()
        .format(&format)
        .expect("date should format");
    format!("{day}T12:00:00")
}

fn insider_item(company: &Company, article_id: &str) -> BankierCompanyItem {
    BankierCompanyItem {
        company_id: company.id.clone(),
        qualified_ticker: company.qualified_ticker.clone(),
        title: "Powiadomienie o transakcjach, o których mowa w art. 19 ust. 1 MAR".to_owned(),
        link: format!("https://www.bankier.pl/wiadomosc/CD-PROJEKT-SA-{article_id}.html"),
        summary: "Komunikat ESPI/EBI".to_owned(),
        published_at: Some(days_ago(3)),
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

#[test]
fn list_attention_events_stamps_urgent_for_an_insider_signal_event() {
    let state = AppState::new(open_in_memory_database().expect("db"));
    let company = company(&state);

    state
        .attention()
        .create_alert_rule(NewAlertRule {
            trigger_type: "signal_category".to_owned(),
            signal_category: Some("insider_transaction".to_owned()),
            price_min: None,
            price_max: None,
            scope_type: "company".to_owned(),
            scope_ref: company.id.clone(),
        })
        .expect("rule");
    state
        .ingest_bankier_company_items(&[insider_item(&company, "9300001")])
        .expect("ingestion classifies and fires");

    let events = state
        .attention()
        .list_attention_events(AttentionEventListInput::default())
        .expect("events");
    assert_eq!(events.len(), 1, "one insider event fired");
    assert_eq!(events[0].trigger_type, "signal_category");
    assert_eq!(
        events[0].severity,
        AttentionSeverity::Urgent,
        "an insider-transaction signal event is urgent (severity computed at read)"
    );
}

#[test]
fn list_attention_events_stamps_notable_for_a_price_event() {
    let state = AppState::new(open_in_memory_database().expect("db"));
    let company = company(&state);

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

    let events = state
        .attention()
        .list_attention_events(AttentionEventListInput::default())
        .expect("events");
    assert_eq!(events.len(), 1);
    assert_eq!(
        events[0].severity,
        AttentionSeverity::Notable,
        "a fired price alert is notable"
    );
}

#[test]
fn list_autopilot_runs_derives_severity_from_status() {
    let state = AppState::new(open_in_memory_database().expect("db"));
    let company = company(&state);

    state
        .autopilot()
        .create_run_if_absent(
            "run_failed",
            &company.id,
            "doc1",
            "manual",
            MODE_ASSIST,
            None,
        )
        .expect("create failed run");
    state
        .autopilot()
        .finalize_run("run_failed", "failed", "extract", None, Some("boom"))
        .expect("finalize failed");

    state
        .autopilot()
        .create_run_if_absent("run_ok", &company.id, "doc2", "manual", MODE_ASSIST, None)
        .expect("create ok run");
    state
        .autopilot()
        .finalize_run("run_ok", "succeeded", "notify", Some("done"), None)
        .expect("finalize succeeded");

    let failed = state.autopilot().get_run("run_failed").expect("run");
    assert_eq!(
        failed.severity,
        AttentionSeverity::Notable,
        "a failed run is notable"
    );
    let succeeded = state.autopilot().get_run("run_ok").expect("run");
    assert_eq!(
        succeeded.severity,
        AttentionSeverity::Routine,
        "a succeeded run is routine"
    );

    // The list read model carries the same computed severity.
    let runs = state
        .autopilot()
        .list_runs(&ListAutopilotRunsInput::default())
        .expect("list");
    let severity_of = |id: &str| {
        runs.iter()
            .find(|run| run.id == id)
            .map(|run| run.severity)
            .expect("run present")
    };
    assert_eq!(severity_of("run_failed"), AttentionSeverity::Notable);
    assert_eq!(severity_of("run_ok"), AttentionSeverity::Routine);
}
