//! Red-flags detection + raising + read model + acknowledgement (ADR 0083
//! Decision 8, plan v0.57 T7). One deterministic test per detection incl. the
//! negative cases, dedup/idempotence, ack-never-re-raises, and the alert-rule
//! firing through the KNF raising path.

use super::super::red_flags::deterioration_reasons;
use super::*;
use crate::fundamentals::health::{AltmanBand, AltmanScore, HealthPeriodScores, PiotroskiScore};

fn tracked_company(state: &AppState) -> Company {
    state
        .create_company(NewCompany {
            exchange: "GPW".to_owned(),
            ticker: "RFT".to_owned(),
            display_name: "Red Flag Test S.A.".to_owned(),
            isin: Some("PLRFT0000011".to_owned()),
            cik: None,
            lei: None,
        })
        .expect("company should be created")
}

fn setup() -> (AppState, Company) {
    let connection = open_in_memory_database().expect("database should initialize");
    let state = AppState::new(connection);
    let company = tracked_company(&state);
    (state, company)
}

fn count(state: &AppState, sql: &str) -> i64 {
    let connection = state.checkout().expect("connection");
    connection
        .query_row(sql, [], |row| row.get::<_, i64>(0))
        .expect("count query")
}

fn signals_of(state: &AppState, category: &str) -> i64 {
    count(
        state,
        &format!("SELECT COUNT(*) FROM company_signals WHERE category = '{category}'"),
    )
}

// ---------------------------------------------------------------------------
// report_delay
// ---------------------------------------------------------------------------

fn seed_periodic_event(state: &AppState, company_id: &str, date: &str) {
    state
        .create_company_event(NewCompanyEvent {
            company_id: company_id.to_owned(),
            event_type: "periodic_report".to_owned(),
            title: "Raport roczny za 2025".to_owned(),
            event_date: date.to_owned(),
            event_time: None,
            status: Some("scheduled".to_owned()),
            source_type: Some("manual".to_owned()),
            source_adapter_id: None,
            source_event_key: None,
            source_url: None,
            attribution: None,
            fetched_at: None,
        })
        .expect("event should create");
}

/// Insert an official-report feed item published on `published_at` for a company.
fn seed_official_report(state: &AppState, company_id: &str, published_at: &str) {
    let connection = state.checkout().expect("connection");
    let id = format!("fi-official-{company_id}-{published_at}");
    connection
        .execute(
            "INSERT INTO feed_items (id, type, source_adapter_id, source_name, source_url,
                 title, fetched_at, dedupe_key, published_at)
             VALUES (?1, 'Official report', 'brawler-red-flags', 'ESPI', 'https://x',
                 'Raport okresowy', '2026-01-01T00:00:00Z', ?1, ?2)",
            params![id, published_at],
        )
        .expect("feed item insert");
    connection
        .execute(
            "INSERT INTO feed_item_companies (feed_item_id, company_id, match_type)
             VALUES (?1, ?2, 'test')",
            params![id, company_id],
        )
        .expect("feed item company link");
}

#[test]
fn report_delay_fires_after_grace_and_raises_signal() {
    let (state, company) = setup();
    seed_periodic_event(&state, &company.id, "2026-07-01"); // well past the grace

    let raised = state
        .red_flags()
        .detect_report_delays()
        .expect("detection runs");

    assert_eq!(raised, 1, "one delayed report should raise a flag");
    assert_eq!(signals_of(&state, "report_delay"), 1);
    assert_eq!(
        count(
            &state,
            "SELECT COUNT(*) FROM feed_items WHERE source_adapter_id = 'brawler-red-flags'"
        ),
        1,
        "a synthetic feed item backs the raised signal"
    );

    let view = state.red_flags().red_flags_view(&company.id).expect("view");
    assert_eq!(view.active.len(), 1);
    assert_eq!(view.active[0].flag_type, "report_delay");
    assert_eq!(view.active[0].severity, "high");
}

#[test]
fn report_delay_suppressed_within_grace() {
    let (state, company) = setup();
    let today: String = time::OffsetDateTime::now_utc()
        .format(&time::format_description::well_known::Iso8601::DATE)
        .unwrap();
    // Event dated today: within the 3-day grace, never a delay.
    seed_periodic_event(&state, &company.id, &today);

    let raised = state
        .red_flags()
        .detect_report_delays()
        .expect("detection runs");
    assert_eq!(raised, 0, "an event inside the grace never fires");
    assert_eq!(signals_of(&state, "report_delay"), 0);
}

#[test]
fn report_delay_suppressed_when_report_arrived() {
    let (state, company) = setup();
    seed_periodic_event(&state, &company.id, "2026-07-01");
    // The official report was ingested on the expected date — no delay.
    seed_official_report(&state, &company.id, "2026-07-02");

    let raised = state
        .red_flags()
        .detect_report_delays()
        .expect("detection runs");
    assert_eq!(raised, 0, "an ingested report suppresses the delay");
    assert_eq!(signals_of(&state, "report_delay"), 0);
}

#[test]
fn report_delay_is_idempotent() {
    let (state, company) = setup();
    seed_periodic_event(&state, &company.id, "2026-07-01");

    assert_eq!(state.red_flags().detect_report_delays().unwrap(), 1);
    // A second sweep detects the same evidence → deterministic id → no new raise.
    assert_eq!(state.red_flags().detect_report_delays().unwrap(), 0);
    assert_eq!(signals_of(&state, "report_delay"), 1);
    let _ = company;
}

// ---------------------------------------------------------------------------
// fund_exit
// ---------------------------------------------------------------------------

fn append_basis(state: &AppState, company_id: &str, holder: &str, as_of: &str, source: &str) {
    state
        .ownership()
        .append_snapshot(NewOwnershipStake {
            company_id: company_id.to_owned(),
            holder_name_raw: holder.to_owned(),
            holder_type: None,
            capital_pct: Some("10".to_owned()),
            votes_pct: Some("10".to_owned()),
            as_of: as_of.to_owned(),
            source: source.to_owned(),
            report_document_id: None,
            feed_item_id: None,
        })
        .expect("stake append");
}

/// Append a stake with an explicit capital pct + source (ESPI-crossing tests).
fn append_stake(
    state: &AppState,
    company_id: &str,
    holder: &str,
    capital_pct: &str,
    as_of: &str,
    source: &str,
) {
    state
        .ownership()
        .append_snapshot(NewOwnershipStake {
            company_id: company_id.to_owned(),
            holder_name_raw: holder.to_owned(),
            holder_type: None,
            capital_pct: Some(capital_pct.to_owned()),
            votes_pct: None,
            as_of: as_of.to_owned(),
            source: source.to_owned(),
            report_document_id: None,
            feed_item_id: None,
        })
        .expect("stake append");
}

#[test]
fn fund_exit_fires_when_holder_vanishes_from_newest_basis() {
    let (state, company) = setup();
    // Previous full-picture basis: two funds.
    append_basis(
        &state,
        &company.id,
        "Fundusz Alfa OFE",
        "2025-03-31",
        "report_document",
    );
    append_basis(
        &state,
        &company.id,
        "Fundusz Beta OFE",
        "2025-03-31",
        "report_document",
    );
    // Newest basis: Beta gone.
    append_basis(
        &state,
        &company.id,
        "Fundusz Alfa OFE",
        "2025-12-31",
        "report_document",
    );

    let raised = state
        .red_flags()
        .detect_fund_exits(&company.id)
        .expect("detection runs");

    assert_eq!(raised, 1, "the vanished holder raises one fund_exit");
    assert_eq!(signals_of(&state, "fund_exit"), 1);
    let view = state.red_flags().red_flags_view(&company.id).expect("view");
    assert_eq!(view.active.len(), 1);
    assert_eq!(view.active[0].flag_type, "fund_exit");
    assert_eq!(view.active[0].severity, "medium");
    assert!(view.active[0].title.contains("Beta"));
}

#[test]
fn fund_exit_no_flag_when_holder_stays() {
    let (state, company) = setup();
    append_basis(
        &state,
        &company.id,
        "Fundusz Alfa OFE",
        "2025-03-31",
        "report_document",
    );
    append_basis(
        &state,
        &company.id,
        "Fundusz Alfa OFE",
        "2025-12-31",
        "report_document",
    );

    assert_eq!(state.red_flags().detect_fund_exits(&company.id).unwrap(), 0);
    assert_eq!(signals_of(&state, "fund_exit"), 0);
}

#[test]
fn fund_exit_ignores_single_espi_snapshot() {
    let (state, company) = setup();
    // One full-picture basis + one single-holder Art. 69 espi update — the espi
    // snapshot is NOT a basis, so no basis diff exists and nobody "exits".
    append_basis(
        &state,
        &company.id,
        "Fundusz Alfa OFE",
        "2025-03-31",
        "report_document",
    );
    append_basis(
        &state,
        &company.id,
        "Fundusz Beta OFE",
        "2025-06-30",
        "espi_filing",
    );

    assert_eq!(
        state.red_flags().detect_fund_exits(&company.id).unwrap(),
        0,
        "a lone espi single-holder update never reads as everyone else exiting"
    );
    assert_eq!(signals_of(&state, "fund_exit"), 0);
}

#[test]
fn fund_exit_is_idempotent() {
    let (state, company) = setup();
    append_basis(
        &state,
        &company.id,
        "Fundusz Alfa OFE",
        "2025-03-31",
        "report_document",
    );
    append_basis(
        &state,
        &company.id,
        "Fundusz Beta OFE",
        "2025-03-31",
        "report_document",
    );
    append_basis(
        &state,
        &company.id,
        "Fundusz Alfa OFE",
        "2025-12-31",
        "report_document",
    );

    assert_eq!(state.red_flags().detect_fund_exits(&company.id).unwrap(), 1);
    assert_eq!(state.red_flags().detect_fund_exits(&company.id).unwrap(), 0);
    assert_eq!(signals_of(&state, "fund_exit"), 1);
}

// --- fund_exit form 2: ESPI threshold crossing (below 5% disclosure) ----------

#[test]
fn fund_exit_fires_on_espi_threshold_crossing() {
    let (state, company) = setup();
    // Holder disclosed 6% in a report, then an ESPI filing crosses to 4.9%.
    append_stake(
        &state,
        &company.id,
        "Fundusz Gamma OFE",
        "6",
        "2025-06-30",
        "report_document",
    );
    append_stake(
        &state,
        &company.id,
        "Fundusz Gamma OFE",
        "4.9",
        "2025-09-30",
        "espi_filing",
    );

    assert_eq!(
        state.red_flags().detect_fund_exits(&company.id).unwrap(),
        1,
        "crossing below the 5% threshold raises a fund_exit"
    );
    assert_eq!(signals_of(&state, "fund_exit"), 1);
    let view = state.red_flags().red_flags_view(&company.id).expect("view");
    assert_eq!(view.active.len(), 1);
    assert_eq!(view.active[0].flag_type, "fund_exit");
    assert!(view.active[0].title.contains("poniżej progu 5%"));
}

#[test]
fn fund_exit_no_crossing_on_decrease_above_threshold() {
    let (state, company) = setup();
    // 20% → 15%: a decrease that stays at/above the threshold is not an exit.
    append_stake(
        &state,
        &company.id,
        "Fundusz Gamma OFE",
        "20",
        "2025-06-30",
        "report_document",
    );
    append_stake(
        &state,
        &company.id,
        "Fundusz Gamma OFE",
        "15",
        "2025-09-30",
        "espi_filing",
    );

    assert_eq!(state.red_flags().detect_fund_exits(&company.id).unwrap(), 0);
    assert_eq!(signals_of(&state, "fund_exit"), 0);
}

#[test]
fn fund_exit_no_crossing_when_never_above_threshold() {
    let (state, company) = setup();
    // 4% → 3%: the holder was never at/above 5%, so a further drop is no crossing.
    append_stake(
        &state,
        &company.id,
        "Fundusz Gamma OFE",
        "4",
        "2025-06-30",
        "report_document",
    );
    append_stake(
        &state,
        &company.id,
        "Fundusz Gamma OFE",
        "3",
        "2025-09-30",
        "espi_filing",
    );

    assert_eq!(state.red_flags().detect_fund_exits(&company.id).unwrap(), 0);
    assert_eq!(signals_of(&state, "fund_exit"), 0);
}

#[test]
fn fund_exit_espi_crossing_is_idempotent() {
    let (state, company) = setup();
    append_stake(
        &state,
        &company.id,
        "Fundusz Gamma OFE",
        "6",
        "2025-06-30",
        "report_document",
    );
    append_stake(
        &state,
        &company.id,
        "Fundusz Gamma OFE",
        "0",
        "2025-09-30",
        "espi_filing",
    );

    assert_eq!(state.red_flags().detect_fund_exits(&company.id).unwrap(), 1);
    assert_eq!(state.red_flags().detect_fund_exits(&company.id).unwrap(), 0);
    assert_eq!(signals_of(&state, "fund_exit"), 1);
}

#[test]
fn fund_exit_basis_vanish_after_espi_crossing_does_not_double_flag() {
    let (state, company) = setup();
    // 1) A full-picture basis discloses Gamma at 6%, then an ESPI crossing to 4.9%.
    append_stake(
        &state,
        &company.id,
        "Fundusz Gamma OFE",
        "6",
        "2025-03-31",
        "report_document",
    );
    append_stake(
        &state,
        &company.id,
        "Fundusz Gamma OFE",
        "4.9",
        "2025-06-30",
        "espi_filing",
    );
    assert_eq!(state.red_flags().detect_fund_exits(&company.id).unwrap(), 1);

    // 2) A later full-picture basis (another holder) no longer lists Gamma — the
    //    basis-vanish form would flag Gamma, but it already has an active flag.
    append_stake(
        &state,
        &company.id,
        "Fundusz Delta OFE",
        "8",
        "2025-12-31",
        "report_document",
    );
    assert_eq!(
        state.red_flags().detect_fund_exits(&company.id).unwrap(),
        0,
        "the basis-vanish must not add a second active flag for the same holder"
    );

    assert_eq!(signals_of(&state, "fund_exit"), 1);
    let view = state.red_flags().red_flags_view(&company.id).expect("view");
    assert_eq!(
        view.active.len(),
        1,
        "one real-world exit flags exactly once"
    );
}

// ---------------------------------------------------------------------------
// score_deterioration
// ---------------------------------------------------------------------------

fn headline_piotroski(score: u8) -> PiotroskiScore {
    PiotroskiScore::Headline {
        score,
        signals: Vec::new(),
    }
}

fn headline_altman(band: AltmanBand) -> AltmanScore {
    AltmanScore::Headline {
        z_score: "1.0".to_owned(),
        band,
        components: Vec::new(),
    }
}

fn period(year: i64, piotroski: PiotroskiScore, altman: AltmanScore) -> HealthPeriodScores {
    HealthPeriodScores {
        period_id: format!("FY{year}"),
        fiscal_year: year,
        piotroski,
        altman,
    }
}

#[test]
fn deterioration_decision_covers_the_taxonomy() {
    // Piotroski drop of exactly 2 → fires.
    let latest = period(
        2025,
        headline_piotroski(4),
        headline_altman(AltmanBand::Grey),
    );
    let prior = period(
        2024,
        headline_piotroski(6),
        headline_altman(AltmanBand::Grey),
    );
    assert!(!deterioration_reasons(&latest, &prior).is_empty());

    // Piotroski drop of 1 → no.
    let latest = period(
        2025,
        headline_piotroski(5),
        headline_altman(AltmanBand::Grey),
    );
    let prior = period(
        2024,
        headline_piotroski(6),
        headline_altman(AltmanBand::Grey),
    );
    assert!(deterioration_reasons(&latest, &prior).is_empty());

    // Altman band downgrade (safe → distress) → fires.
    let latest = period(
        2025,
        headline_piotroski(7),
        headline_altman(AltmanBand::Distress),
    );
    let prior = period(
        2024,
        headline_piotroski(7),
        headline_altman(AltmanBand::Safe),
    );
    assert!(!deterioration_reasons(&latest, &prior).is_empty());

    // Band improved, F stable → no.
    let latest = period(
        2025,
        headline_piotroski(7),
        headline_altman(AltmanBand::Safe),
    );
    let prior = period(
        2024,
        headline_piotroski(7),
        headline_altman(AltmanBand::Grey),
    );
    assert!(deterioration_reasons(&latest, &prior).is_empty());

    // Missing prior FY headline (insufficient) → never fires.
    let latest = period(
        2025,
        headline_piotroski(2),
        headline_altman(AltmanBand::Distress),
    );
    let prior = period(
        2024,
        PiotroskiScore::InsufficientData {
            signals: Vec::new(),
            missing: Vec::new(),
        },
        AltmanScore::InsufficientData {
            components: Vec::new(),
            missing: Vec::new(),
        },
    );
    assert!(deterioration_reasons(&latest, &prior).is_empty());
}

/// Seed one FY period with the Altman inputs (working_capital derives from
/// current_assets − current_liabilities); returns nothing.
fn seed_altman_fy(state: &AppState, company_id: &str, year: i64, values: &[(&str, &str)]) {
    let period = state
        .create_financial_period(NewFinancialPeriod {
            company_id: company_id.to_owned(),
            fiscal_year: year,
            period_type: "FY".to_owned(),
            period_end_date: Some(format!("{year}-12-31")),
            report_evidence_ref: None,
        })
        .expect("period create");
    let defs = state
        .list_kpi_definitions(ListKpiDefinitionsInput {
            scope: Some("canonical".to_owned()),
            sector: None,
            company_id: None,
        })
        .expect("definitions");
    for (key, value) in values {
        let def = defs
            .iter()
            .find(|d| &d.metric_key == key)
            .unwrap_or_else(|| panic!("definition {key} should exist"));
        state
            .create_financial_fact(NewFinancialFact {
                company_id: company_id.to_owned(),
                period_id: period.id.clone(),
                definition_id: def.id.clone(),
                value_numeric: (*value).to_owned(),
                currency: None,
                statement_basis: None,
                attribution: None,
                variant: None,
                measure_window: None,
                data_quality: None,
                as_reported_value: None,
                as_reported_scale: None,
                reporting_standard: None,
                extraction_method: None,
                confidence: None,
                confirmation_state: None,
                supersedes_id: None,
                source_document_ref: None,
                annotation: None,
            })
            .expect("fact create");
    }
}

#[test]
fn score_deterioration_fires_on_altman_band_downgrade() {
    let (state, company) = setup();
    // Prior FY: healthy (Z well into the safe band).
    seed_altman_fy(
        &state,
        &company.id,
        2024,
        &[
            ("current_assets", "800"),
            ("current_liabilities", "100"),
            ("retained_earnings", "500"),
            ("operating_profit", "300"),
            ("total_assets", "1000"),
            ("total_equity", "800"),
            ("total_liabilities", "200"),
        ],
    );
    // Latest FY: distressed (negative working capital, thin equity → distress band).
    seed_altman_fy(
        &state,
        &company.id,
        2025,
        &[
            ("current_assets", "50"),
            ("current_liabilities", "500"),
            ("retained_earnings", "-200"),
            ("operating_profit", "-100"),
            ("total_assets", "1000"),
            ("total_equity", "100"),
            ("total_liabilities", "900"),
        ],
    );

    let raised = state
        .red_flags()
        .detect_score_deterioration(&company.id)
        .expect("detection runs");
    assert_eq!(raised, 1, "a band downgrade raises one score_deterioration");
    assert_eq!(signals_of(&state, "score_deterioration"), 1);
    let view = state.red_flags().red_flags_view(&company.id).expect("view");
    assert_eq!(view.active.len(), 1);
    assert_eq!(view.active[0].flag_type, "score_deterioration");
    assert_eq!(view.active[0].severity, "medium");
}

#[test]
fn score_deterioration_needs_two_periods() {
    let (state, company) = setup();
    seed_altman_fy(
        &state,
        &company.id,
        2025,
        &[
            ("current_assets", "50"),
            ("current_liabilities", "500"),
            ("retained_earnings", "-200"),
            ("operating_profit", "-100"),
            ("total_assets", "1000"),
            ("total_equity", "100"),
            ("total_liabilities", "900"),
        ],
    );
    assert_eq!(
        state
            .red_flags()
            .detect_score_deterioration(&company.id)
            .unwrap(),
        0,
        "a single FY period has no prior to compare against"
    );
}

// ---------------------------------------------------------------------------
// Acknowledgement + alert firing + composition
// ---------------------------------------------------------------------------

fn signal_rule(company_id: &str, category: &str) -> NewAlertRule {
    NewAlertRule {
        trigger_type: "signal_category".to_owned(),
        signal_category: Some(category.to_owned()),
        price_min: None,
        price_max: None,
        scope_type: "company".to_owned(),
        scope_ref: company_id.to_owned(),
    }
}

#[test]
fn fund_exit_raising_fires_a_signal_category_alert_rule() {
    let (state, company) = setup();
    state
        .attention()
        .create_alert_rule(signal_rule(&company.id, "fund_exit"))
        .expect("rule create");

    append_basis(
        &state,
        &company.id,
        "Fundusz Alfa OFE",
        "2025-03-31",
        "report_document",
    );
    append_basis(
        &state,
        &company.id,
        "Fundusz Beta OFE",
        "2025-03-31",
        "report_document",
    );
    append_basis(
        &state,
        &company.id,
        "Fundusz Alfa OFE",
        "2025-12-31",
        "report_document",
    );

    state
        .red_flags()
        .detect_fund_exits(&company.id)
        .expect("detection runs");

    let events = state
        .attention()
        .list_attention_events(AttentionEventListInput::default())
        .expect("events list");
    assert_eq!(events.len(), 1, "the derived flag fires the alert rule");
}

#[test]
fn acked_flag_moves_to_history_and_never_reraises() {
    let (state, company) = setup();
    state
        .attention()
        .create_alert_rule(signal_rule(&company.id, "fund_exit"))
        .expect("rule create");
    append_basis(
        &state,
        &company.id,
        "Fundusz Alfa OFE",
        "2025-03-31",
        "report_document",
    );
    append_basis(
        &state,
        &company.id,
        "Fundusz Beta OFE",
        "2025-03-31",
        "report_document",
    );
    append_basis(
        &state,
        &company.id,
        "Fundusz Alfa OFE",
        "2025-12-31",
        "report_document",
    );
    state.red_flags().detect_fund_exits(&company.id).unwrap();

    let view = state.red_flags().red_flags_view(&company.id).expect("view");
    let flag_id = view.active[0].flag_id.clone();

    // Acknowledge → leaves active, enters history.
    let after = state.red_flags().acknowledge(&flag_id).expect("ack");
    assert!(after.active.is_empty(), "acked flag leaves the active list");
    assert_eq!(after.history.len(), 1);
    assert_eq!(after.history[0].flag_id, flag_id);
    assert!(after.history[0].acked_at.is_some());

    let events_before = state
        .attention()
        .list_attention_events(AttentionEventListInput::default())
        .unwrap()
        .len();

    // Re-detect the same evidence → no re-raise, no new alert, stays in history.
    assert_eq!(state.red_flags().detect_fund_exits(&company.id).unwrap(), 0);
    let events_after = state
        .attention()
        .list_attention_events(AttentionEventListInput::default())
        .unwrap()
        .len();
    assert_eq!(events_before, events_after, "an acked flag never re-fires");

    let view = state.red_flags().red_flags_view(&company.id).expect("view");
    assert!(view.active.is_empty());
    assert_eq!(view.history.len(), 1);
}

#[test]
fn acknowledge_is_idempotent() {
    let (state, company) = setup();
    append_basis(
        &state,
        &company.id,
        "Fundusz Alfa OFE",
        "2025-03-31",
        "report_document",
    );
    append_basis(
        &state,
        &company.id,
        "Fundusz Beta OFE",
        "2025-03-31",
        "report_document",
    );
    append_basis(
        &state,
        &company.id,
        "Fundusz Alfa OFE",
        "2025-12-31",
        "report_document",
    );
    state.red_flags().detect_fund_exits(&company.id).unwrap();
    let view = state.red_flags().red_flags_view(&company.id).expect("view");
    let flag_id = view.active[0].flag_id.clone();

    state.red_flags().acknowledge(&flag_id).unwrap();
    state.red_flags().acknowledge(&flag_id).unwrap();
    assert_eq!(count(&state, "SELECT COUNT(*) FROM red_flag_acks"), 1);
}

#[test]
fn view_composes_auditor_flag_at_read_without_raising() {
    let (state, company) = setup();
    // An auditor_opinion signal already exists (ADR 0079) — the red-flags view
    // composes it at read; it is NOT re-raised (no new feed item / signal).
    let connection = state.checkout().expect("connection");
    connection
        .execute(
            "INSERT INTO feed_items (id, type, source_adapter_id, source_name, source_url,
                 title, fetched_at, dedupe_key, published_at)
             VALUES ('fi-aud', 'Official report', 'brawler-red-flags', 'ESPI', 'https://a',
                 'Opinia z zastrzeżeniem', '2026-01-01T00:00:00Z', 'fi-aud', '2026-05-01')",
            [],
        )
        .expect("feed item");
    connection
        .execute(
            "INSERT INTO feed_item_companies (feed_item_id, company_id, match_type)
             VALUES ('fi-aud', ?1, 'test')",
            params![company.id],
        )
        .expect("link");
    connection
        .execute(
            "INSERT INTO company_signals (id, company_id, feed_item_id, category, confidence,
                 classified_by, status, signal_date)
             VALUES ('sig-aud', ?1, 'fi-aud', 'auditor_opinion', 1.0, 'rule', 'confirmed', '2026-05-01')",
            params![company.id],
        )
        .expect("signal");
    drop(connection);

    let view = state.red_flags().red_flags_view(&company.id).expect("view");
    assert_eq!(view.active.len(), 1);
    assert_eq!(view.active[0].flag_type, "auditor_red_flag");
    assert_eq!(view.active[0].severity, "high");
    // No synthetic red-flag feed item was written (compose-at-read only).
    assert_eq!(signals_of(&state, "auditor_red_flag"), 0);
}
