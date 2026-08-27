//! Unit tests for [`super`] (`get_company_view`, F3a S1) — split out under
//! the file-size ratchet (ADR 0103), mirroring `today_tests.rs`.
use super::*;
use crate::source_adapters::knf_short_selling::KnfShortEntry;
use crate::source_adapters::yahoo_eod::DailyQuote;
use crate::storage::{
    open_in_memory_database, AppState, NewCompany, NewCompanyEvent, NewFinancialFact,
    NewFinancialPeriod, NewManagementClaim,
};

fn state() -> AppState {
    AppState::new(open_in_memory_database().expect("in-memory db"))
}

fn company(state: &AppState, ticker: &str) -> String {
    company_with_isin(state, ticker, None)
}

fn company_with_isin(state: &AppState, ticker: &str, isin: Option<&str>) -> String {
    state
        .create_company(NewCompany {
            exchange: "GPW".to_owned(),
            ticker: ticker.to_owned(),
            display_name: format!("{ticker} S.A."),
            isin: isin.map(str::to_owned),
            cik: None,
            lei: None,
        })
        .expect("company")
        .id
}

#[test]
fn unknown_company_is_a_top_level_error() {
    let state = state();
    let result = compute_company_view(&state, "company_does_not_exist");
    assert!(result.is_err());
}

#[test]
fn empty_company_composes_every_section_empty_without_section_errors() {
    let state = state();
    let company_id = company(&state, "EMP");

    let view = compute_company_view(&state, &company_id).expect("view");

    assert_eq!(view.company_id, company_id);
    let sections = serde_json::to_value(&view.section_errors).expect("serialize");
    assert_eq!(sections, serde_json::json!({}), "no section should error");

    let counters = view.counters.expect("counters present");
    assert_eq!(counters.signals.unacked, 0);
    assert!(counters.signals.by_category.is_empty());
    assert_eq!(counters.claims.open, 0);
    assert!(counters.claims.nearest_due.is_none());
    assert_eq!(counters.shorts.active_sum_pct, 0.0);
    assert!(counters.shorts.largest_holder.is_none());
    assert_eq!(counters.events.upcoming, 0);

    let kpi = view.kpi.expect("kpi present");
    assert!(kpi.currency.is_none());
    assert!(kpi.years.is_empty());
    assert_eq!(kpi.rows.len(), 3, "one row per KPI metric even when empty");
    assert!(kpi.rows.iter().all(|row| row.cells.is_empty()));

    assert!(view.feed.is_empty());
    assert!(view.coverage.is_empty());
    assert!(view.recommendations.is_empty());

    // GPW-mapped, no quotes ingested yet -> the price section's own empty
    // state, not a section error.
    let price = view.price.expect("price present");
    assert_eq!(price.empty_reason.as_deref(), Some("no_quotes"));
}

fn feed_row(state: &AppState, company_id: &str, id: &str, published_at: &str, item_type: &str) {
    let connection = state.checkout_for_tests().expect("connection");
    connection
        .execute(
            "INSERT INTO feed_items (id, type, source_adapter_id, source_name, source_url,
                     title, fetched_at, dedupe_key, published_at)
                 VALUES (?1, ?2, 'gpw-espi-ebi', 'ESPI', 'https://x', ?1, ?3, ?1, ?3)",
            rusqlite::params![id, item_type, published_at],
        )
        .expect("feed item insert");
    connection
        .execute(
            "INSERT INTO feed_item_companies (feed_item_id, company_id, match_type)
                 VALUES (?1, ?2, 'test')",
            rusqlite::params![id, company_id],
        )
        .expect("feed item company link");
}

#[test]
fn feed_caps_at_six_newest_with_id_tiebreaker() {
    let state = state();
    let company_id = company(&state, "FEE");

    // 8 items; "same_a"/"same_b" share published_at and must tie-break on
    // id DESC ("same_b" wins).
    feed_row(
        &state,
        &company_id,
        "item_1",
        "2026-01-01T00:00:00Z",
        "Official report",
    );
    feed_row(
        &state,
        &company_id,
        "item_2",
        "2026-01-02T00:00:00Z",
        "Public media",
    );
    feed_row(
        &state,
        &company_id,
        "item_3",
        "2026-01-03T00:00:00Z",
        "Official report",
    );
    feed_row(
        &state,
        &company_id,
        "item_4",
        "2026-01-04T00:00:00Z",
        "Public media",
    );
    feed_row(
        &state,
        &company_id,
        "same_a",
        "2026-01-05T00:00:00Z",
        "Official report",
    );
    feed_row(
        &state,
        &company_id,
        "same_b",
        "2026-01-05T00:00:00Z",
        "Public media",
    );
    feed_row(
        &state,
        &company_id,
        "item_7",
        "2026-01-06T00:00:00Z",
        "Official report",
    );
    feed_row(
        &state,
        &company_id,
        "item_8",
        "2026-01-07T00:00:00Z",
        "Public media",
    );
    // A non-feed type (synthetic red-flag item) must never surface here.
    feed_row(
        &state,
        &company_id,
        "flag_1",
        "2026-01-08T00:00:00Z",
        "Red flag",
    );

    let view = compute_company_view(&state, &company_id).expect("view");
    assert_eq!(view.feed.len(), 6);
    let ids: Vec<&str> = view
        .feed
        .iter()
        .map(|item| item.feed_item_id.as_str())
        .collect();
    assert_eq!(
        ids,
        vec!["item_8", "item_7", "same_b", "same_a", "item_4", "item_3"],
        "newest published_at first, id DESC tiebreaker, capped at 6"
    );
}

fn bar(date: &str, close: f64) -> DailyQuote {
    DailyQuote {
        date: date.to_owned(),
        open: close - 1.0,
        high: close + 2.0,
        low: close - 2.0,
        close,
        volume: 1000,
    }
}

/// `n` consecutive daily bars starting at `start_date` (`YYYY-MM-DD`),
/// close rising by 1.0 per session — enough spread that delta assertions
/// can pin an exact expected value.
fn daily_bars(start_date: time::Date, n: i64) -> Vec<DailyQuote> {
    (0..n)
        .map(|i| {
            let date = start_date.saturating_add(time::Duration::days(i));
            let format = time::macros::format_description!("[year]-[month]-[day]");
            bar(&date.format(&format).expect("format"), 100.0 + i as f64)
        })
        .collect()
}

#[test]
fn price_slices_last_66_sessions_and_delta_1m_is_21_sessions_back() {
    let state = state();
    let company_id = company(&state, "PRC");
    let start = time::macros::date!(2025 - 01 - 01);
    let quotes = daily_bars(start, 90);
    state
        .market_data()
        .upsert_quotes(&company_id, &quotes, "yahoo-eod", "2025-04-01T18:00:00Z")
        .expect("upsert");

    let view = compute_company_view(&state, &company_id).expect("view");
    let price = view.price.expect("price present");
    assert!(price.empty_reason.is_none());
    assert_eq!(price.candles.len(), 66);
    assert_eq!(price.as_of, quotes.last().expect("last bar").date);
    assert_eq!(price.last_close, 189.0); // 100.0 + (90 - 1)
                                         // 21 sessions back from the last (index 89) is index 68 -> close 168.0.
    let expected_delta_1m = (189.0 - 168.0) / 168.0 * 100.0;
    assert!((price.delta_1m_pct.expect("delta1m") - expected_delta_1m).abs() < 1e-9);
    // Chart's oldest candle is the 66th-from-end bar (index 24).
    assert_eq!(price.candles[0].close, 124.0);
}

#[test]
fn price_delta_ytd_uses_last_session_of_prior_year_and_is_none_without_one() {
    let state = state();

    // Case 1: bars cross the year boundary -> delta vs the last prior-year
    // session.
    let crossing = company(&state, "YTA");
    // One continuous run of 5 sessions crossing the year boundary:
    // 2025-12-29..31 then 2026-01-01..02 (closes 100..104).
    let quotes = daily_bars(time::macros::date!(2025 - 12 - 29), 5);
    state
        .market_data()
        .upsert_quotes(&crossing, &quotes, "yahoo-eod", "2026-01-02T18:00:00Z")
        .expect("upsert");
    let view = compute_company_view(&state, &crossing).expect("view");
    let price = view.price.expect("price present");
    // Last session close = 104.0 (2026-01-02); last 2025 session
    // (2025-12-31, index 2) close = 102.0.
    let expected = (104.0 - 102.0) / 102.0 * 100.0;
    assert!((price.delta_ytd_pct.expect("delta ytd") - expected).abs() < 1e-9);

    // Case 2: bars only within one calendar year -> no prior-year
    // session, delta is None (never 0.0).
    let single_year = company(&state, "YTB");
    let quotes = daily_bars(time::macros::date!(2026 - 01 - 02), 5);
    state
        .market_data()
        .upsert_quotes(&single_year, &quotes, "yahoo-eod", "2026-01-06T18:00:00Z")
        .expect("upsert");
    let view = compute_company_view(&state, &single_year).expect("view");
    let price = view.price.expect("price present");
    assert!(price.delta_ytd_pct.is_none());
}

fn seed_periodic_event(state: &AppState, company_id: &str, date: &str) {
    state
        .create_company_event(NewCompanyEvent {
            company_id: company_id.to_owned(),
            event_type: "periodic_report".to_owned(),
            title: "Raport roczny".to_owned(),
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

/// Direct-SQL auditor_opinion signal seed — same idiom
/// `storage::tests::red_flags::view_composes_auditor_flag_at_read_without_raising`
/// uses: the auditor flag is composed at read from an existing confirmed
/// signal, never raised via a store method.
fn seed_auditor_signal(state: &AppState, company_id: &str) {
    let connection = state.checkout_for_tests().expect("connection");
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
            rusqlite::params![company_id],
        )
        .expect("link");
    connection
            .execute(
                "INSERT INTO company_signals (id, company_id, feed_item_id, category, confidence,
                     classified_by, status, signal_date)
                 VALUES ('sig-aud', ?1, 'fi-aud', 'auditor_opinion', 1.0, 'rule', 'confirmed', '2026-05-01')",
                rusqlite::params![company_id],
            )
            .expect("signal");
}

#[test]
fn counters_signals_count_only_unacked_with_category_breakdown() {
    let state = state();
    let company_id = company(&state, "SIG");

    // Two report_delay flags (two overdue periodic-report events) and one
    // auditor_red_flag -> report_delay wins count DESC despite sorting
    // alphabetically after auditor_red_flag.
    seed_periodic_event(&state, &company_id, "2020-01-01");
    seed_periodic_event(&state, &company_id, "2020-02-01");
    state
        .red_flags()
        .detect_report_delays()
        .expect("detection runs");
    seed_auditor_signal(&state, &company_id);

    let view = compute_company_view(&state, &company_id).expect("view");
    let counters = view.counters.expect("counters");
    assert_eq!(counters.signals.unacked, 3);
    assert_eq!(
        counters.signals.by_category,
        vec![
            CompanyViewCategoryCount {
                category: "report_delay".to_owned(),
                count: 2
            },
            CompanyViewCategoryCount {
                category: "auditor_red_flag".to_owned(),
                count: 1
            },
        ]
    );

    // Acknowledging one report_delay flag drops the unacked count.
    let flags = state.red_flags().red_flags_view(&company_id).expect("view");
    let one = flags
        .active
        .iter()
        .find(|f| f.flag_type == "report_delay")
        .expect("a report_delay flag");
    state.red_flags().acknowledge(&one.flag_id).expect("ack");
    let view = compute_company_view(&state, &company_id).expect("view");
    assert_eq!(view.counters.expect("counters").signals.unacked, 2);
}

fn claim(
    state: &AppState,
    company_id: &str,
    fiscal_year: i64,
    period_type: &str,
    status: Option<&str>,
) {
    state
        .create_management_claim(NewManagementClaim {
            company_id: company_id.to_owned(),
            statement: format!("Claim {fiscal_year} {period_type}"),
            body: None,
            made_at: None,
            source_period_id: None,
            due_fiscal_year: Some(fiscal_year),
            due_period_type: Some(period_type.to_owned()),
            status: status.map(str::to_owned),
            source_evidence_type: None,
            source_evidence_id: None,
            target_metric_key: None,
            target_comparator: None,
            target_value_numeric: None,
            target_unit: None,
        })
        .expect("claim");
}

#[test]
fn counters_claims_open_and_nearest_due() {
    let state = state();
    let company_id = company(&state, "CLM");

    claim(&state, &company_id, 2025, "FY", None); // pending (default)
    claim(&state, &company_id, 2024, "H1", None); // pending, earlier -> nearest
    state
        .set_claim_verdict(crate::storage::SetClaimVerdictInput {
            claim_id: {
                // Re-fetch to grab an id: create a distinct delivered claim.
                claim(&state, &company_id, 2023, "FY", None);
                state
                    .list_management_claims(&company_id)
                    .expect("claims")
                    .iter()
                    .find(|c| c.due_fiscal_year == Some(2023))
                    .expect("2023 claim")
                    .id
                    .clone()
            },
            status: "delivered".to_owned(),
            verifying_fact_id: None,
            verifying_relation: None,
            revises_claim_id: None,
        })
        .expect("verify");

    let view = compute_company_view(&state, &company_id).expect("view");
    let claims = view.counters.expect("counters").claims;
    // 2025 FY + 2024 H1 open (delivered 2023 excluded).
    assert_eq!(claims.open, 2);
    assert_eq!(claims.nearest_due.as_deref(), Some("H1 2024"));
}

/// Same fiscal year, domain period order (Q1 < FY) — alphabetical order
/// would wrongly pick "FY 2026" (`F` < `Q`).
#[test]
fn nearest_due_orders_periods_by_domain_order_not_alphabetically() {
    let state = state();
    let company_id = company(&state, "ORD");

    claim(&state, &company_id, 2026, "FY", None);
    claim(&state, &company_id, 2026, "Q1", None);

    let view = compute_company_view(&state, &company_id).expect("view");
    let claims = view.counters.expect("counters").claims;
    assert_eq!(claims.nearest_due.as_deref(), Some("Q1 2026"));
}

fn seed_short_adapter(state: &AppState) {
    let connection = state.checkout_for_tests().expect("connection");
    connection
            .execute(
                "INSERT INTO source_adapters (
                    id, display_name, source_type, fetch_mode, enabled, default_poll_interval_seconds
                ) VALUES ('knf-short-selling', 'KNF Short Selling Register', 'disclosure', 'public_json', 1, 86400)
                ON CONFLICT(id) DO NOTHING",
                [],
            )
            .expect("adapter row should seed");
}

fn short_entry(holder: &str, isin: &str, pct: f64, date: &str) -> KnfShortEntry {
    KnfShortEntry {
        holder_name: holder.to_owned(),
        issuer_name: "ISSUER".to_owned(),
        isin: isin.to_owned(),
        net_position_pct: pct,
        position_date: date.to_owned(),
        modify_date: None,
    }
}

#[test]
fn counters_shorts_sum_active_and_largest_holder_tie_alphabetical() {
    let state = state();
    let isin = "PLSHT0000019";
    let company_id = company_with_isin(&state, "SHT", Some(isin));
    seed_short_adapter(&state);
    state
        .short_positions()
        .ingest_knf_short_positions(&[
            short_entry("Zeta Fund", isin, 2.5, "2026-01-01"),
            short_entry("Alpha Fund", isin, 2.5, "2026-01-01"), // tie -> alphabetically first
            short_entry("Beta Fund", isin, 1.0, "2026-01-01"),
        ])
        .expect("ingest");

    let view = compute_company_view(&state, &company_id).expect("view");
    let shorts = view.counters.expect("counters").shorts;
    assert!((shorts.active_sum_pct - 6.0).abs() < 1e-9);
    assert_eq!(shorts.largest_holder.as_deref(), Some("Alpha Fund"));
}

#[test]
fn counters_events_scheduled_within_closed_30_day_window() {
    let state = state();
    let company_id = company(&state, "EVT");
    let today = format_date(today_date());
    let plus_30 = format_date(today_date().saturating_add(time::Duration::days(30)));
    let plus_31 = format_date(today_date().saturating_add(time::Duration::days(31)));
    let yesterday = format_date(today_date().saturating_add(time::Duration::days(-1)));

    seed_periodic_event(&state, &company_id, &today); // in window (boundary)
    seed_periodic_event(&state, &company_id, &plus_30); // in window (boundary)
    seed_periodic_event(&state, &company_id, &plus_31); // outside window
    seed_periodic_event(&state, &company_id, &yesterday); // outside window (past)
    state
        .create_company_event(NewCompanyEvent {
            company_id: company_id.clone(),
            event_type: "custom".to_owned(),
            title: "Not scheduled".to_owned(),
            event_date: today.clone(),
            event_time: None,
            status: Some("completed".to_owned()),
            source_type: None,
            source_adapter_id: None,
            source_event_key: None,
            source_url: None,
            attribution: None,
            fetched_at: None,
        })
        .expect("non-scheduled event");

    let view = compute_company_view(&state, &company_id).expect("view");
    assert_eq!(view.counters.expect("counters").events.upcoming, 2);
}

fn kpi_period(state: &AppState, company_id: &str, fiscal_year: i64) -> String {
    state
        .financials()
        .create_financial_period(NewFinancialPeriod {
            company_id: company_id.to_owned(),
            fiscal_year,
            period_type: "FY".to_owned(),
            period_end_date: Some(format!("{fiscal_year:04}-12-31")),
            report_evidence_ref: None,
        })
        .expect("period")
        .id
}

fn kpi_fact(
    state: &AppState,
    company_id: &str,
    period_id: &str,
    metric_key: &str,
    value: &str,
    source_document_ref: &str,
) {
    state
        .financials()
        .create_financial_fact(NewFinancialFact {
            company_id: company_id.to_owned(),
            period_id: period_id.to_owned(),
            definition_id: format!("kpidef_{metric_key}"),
            value_numeric: value.to_owned(),
            currency: Some("PLN".to_owned()),
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
            source_document_ref: Some(source_document_ref.to_owned()),
            annotation: None,
        })
        .expect("fact");
}

#[test]
fn kpi_last_four_fy_with_yoy_and_document_tickets() {
    let state = state();
    let company_id = company(&state, "KPI");

    // 5 FYs seeded; only the newest 4 (2022..2025) should surface.
    for (year, revenue) in [
        (2021, "500"),
        (2022, "600"),
        (2023, "700"),
        (2024, "800"),
        (2025, "1000"),
    ] {
        let period_id = kpi_period(&state, &company_id, year);
        kpi_fact(
            &state,
            &company_id,
            &period_id,
            "revenue",
            revenue,
            &format!("doc_{year}"),
        );
    }

    let view = compute_company_view(&state, &company_id).expect("view");
    let kpi = view.kpi.expect("kpi");
    assert_eq!(kpi.years, vec![2022, 2023, 2024, 2025]);
    assert_eq!(kpi.currency.as_deref(), Some("PLN"));

    let revenue_row = kpi
        .rows
        .iter()
        .find(|row| row.metric_key == "revenue")
        .expect("revenue row");
    assert_eq!(revenue_row.cells.len(), 4);
    assert_eq!(revenue_row.cells[0].fiscal_year, 2022);
    assert_eq!(revenue_row.cells[0].value_numeric.as_deref(), Some("600"));
    assert_eq!(
        revenue_row.cells[0].source_document_ref.as_deref(),
        Some("doc_2022")
    );
    assert_eq!(revenue_row.cells[3].fiscal_year, 2025);
    assert_eq!(revenue_row.cells[3].value_numeric.as_deref(), Some("1000"));
    // yoy over the two newest (2024: 800, 2025: 1000).
    let expected_yoy = (1000.0 - 800.0) / 800.0 * 100.0;
    assert!((revenue_row.yoy_pct.expect("yoy") - expected_yoy).abs() < 1e-9);

    // Other two metrics never got facts -> empty-valued cells, still one
    // row each (closed metric set).
    let op_row = kpi
        .rows
        .iter()
        .find(|row| row.metric_key == "operating_profit")
        .expect("operating_profit row");
    assert!(op_row.cells.iter().all(|cell| cell.value_numeric.is_none()));
    assert!(op_row.yoy_pct.is_none());
}

// sol-review finding 6: yoy must span the two NEWEST POPULATED cells, not
// just the final two array positions — a gapped penultimate FY must not
// collapse yoy to `None` when an older populated FY exists.
#[test]
fn yoy_skips_a_gapped_year_and_uses_the_two_newest_populated_cells() {
    let state = state();
    let company_id = company(&state, "GAP");

    // 2022 and 2023 populated, 2024 gapped (no fact), 2025 populated.
    for (year, revenue) in [(2022, "600"), (2023, "700"), (2025, "1000")] {
        let period_id = kpi_period(&state, &company_id, year);
        kpi_fact(
            &state,
            &company_id,
            &period_id,
            "revenue",
            revenue,
            &format!("doc_{year}"),
        );
    }
    // 2024 exists as an FY period but with no revenue fact -> a gapped cell.
    kpi_period(&state, &company_id, 2024);

    let view = compute_company_view(&state, &company_id).expect("view");
    let kpi = view.kpi.expect("kpi");
    assert_eq!(kpi.years, vec![2022, 2023, 2024, 2025]);

    let revenue_row = kpi
        .rows
        .iter()
        .find(|row| row.metric_key == "revenue")
        .expect("revenue row");
    assert_eq!(
        revenue_row
            .cells
            .iter()
            .find(|cell| cell.fiscal_year == 2024)
            .expect("2024 cell present")
            .value_numeric,
        None,
        "2024 is the gapped year"
    );
    // yoy skips the gapped 2024 and uses the two newest POPULATED cells:
    // 2023 (700) and 2025 (1000) — not cells[2]/cells[3] (2024/2025).
    let expected_yoy = (1000.0 - 700.0) / 700.0 * 100.0;
    assert!((revenue_row.yoy_pct.expect("yoy") - expected_yoy).abs() < 1e-9);
}

/// Perf regression guard (owner P1, 2026-08-27: pool-checkout fan-out — not
/// query cost — was the real ~10s on the real DB; `real_data_section_timings`
/// below measured 53 pool checkouts for one CDR view). `compute_company_view`
/// must check out exactly ONE connection and thread it through every section
/// (same pattern/budget as `mcp::kpi_ingest::start_kpi_ingest_is_checkout_bounded`).
/// Populated across every section (claims/events/shorts/kpi/quotes/feed) so a
/// future regression that re-adds a per-row or per-section checkout — not just
/// an empty-company vacuous pass — reddens this.
#[test]
fn get_company_view_is_checkout_bounded() {
    let state = state();
    let isin = "PLCVB0000011";
    let company_id = company_with_isin(&state, "CVB", Some(isin));

    claim(&state, &company_id, 2026, "FY", None);
    claim(&state, &company_id, 2026, "Q1", None);
    seed_periodic_event(&state, &company_id, &format_date(today_date()));

    seed_short_adapter(&state);
    state
        .short_positions()
        .ingest_knf_short_positions(&[
            short_entry("Alpha Fund", isin, 2.5, "2026-01-01"),
            short_entry("Beta Fund", isin, 1.0, "2026-01-01"),
        ])
        .expect("ingest");

    for (year, revenue) in [(2024, "500"), (2025, "600")] {
        let period_id = kpi_period(&state, &company_id, year);
        kpi_fact(
            &state,
            &company_id,
            &period_id,
            "revenue",
            revenue,
            &format!("doc_{year}"),
        );
    }

    let quotes = daily_bars(time::macros::date!(2025 - 01 - 01), 90);
    state
        .market_data()
        .upsert_quotes(&company_id, &quotes, "yahoo-eod", "2025-04-01T18:00:00Z")
        .expect("upsert");

    feed_row(
        &state,
        &company_id,
        "item_1",
        "2026-01-01T00:00:00Z",
        "Official report",
    );
    feed_row(
        &state,
        &company_id,
        "item_2",
        "2026-01-02T00:00:00Z",
        "Public media",
    );

    let before = state.checkout_count();
    let view = compute_company_view(&state, &company_id).expect("view");
    let delta = state.checkout_count() - before;

    let sections = serde_json::to_value(&view.section_errors).expect("serialize");
    assert_eq!(
        sections,
        serde_json::json!({}),
        "no section should error in this fixture"
    );
    assert!(
        delta <= 2,
        "get_company_view took {delta} pool checkouts (budget: 2)"
    );
}

/// Contention proof: a `get_company_view` needing only ONE checkout must not
/// queue behind connections it doesn't need. Opens a real r2d2 pool
/// (`storage::open_pool`, `max_connections` default 4) on a scratch temp DB,
/// holds 3 of the 4 connections on other threads for 1.5s, and asserts the
/// view still returns in well under a second — proving it only ever needed
/// the 4th. Before the 2026-08-27 fix (53 checkouts for one CDR view) this
/// same setup would have serialized behind the held connections.
#[test]
fn get_company_view_completes_while_three_connections_are_held() {
    let dir = std::env::temp_dir().join(format!(
        "brawler-company-view-contention-{}",
        std::process::id()
    ));
    std::fs::create_dir_all(&dir).expect("temp dir");
    let state =
        crate::storage::open_pool(dir.join("brawler.sqlite3"), dir.clone()).expect("pool opens");

    let company_id = company(&state, "CTN");

    let barrier = std::sync::Arc::new(std::sync::Barrier::new(4));
    let handles: Vec<_> = (0..3)
        .map(|_| {
            let state = state.clone();
            let barrier = barrier.clone();
            std::thread::spawn(move || {
                let _guard = state.checkout_for_tests().expect("checkout");
                barrier.wait();
                std::thread::sleep(std::time::Duration::from_millis(1500));
            })
        })
        .collect();

    barrier.wait(); // all 3 workers hold a connection before we measure.
    let start = std::time::Instant::now();
    let view = compute_company_view(&state, &company_id).expect("view");
    let elapsed = start.elapsed();

    for handle in handles {
        handle.join().expect("worker thread");
    }
    let _ = std::fs::remove_dir_all(&dir);

    assert_eq!(view.company_id, company_id);
    assert!(
        elapsed < std::time::Duration::from_secs(1),
        "get_company_view took {elapsed:?} with 3/4 pool connections held — \
         it must need only the 4th, not queue behind checkouts it doesn't need"
    );
}

/// Real-data timing probe (owner P1: Spółka screen ~10s on the real DB).
/// **Inert in CI** — skips unless `BRAWLER_REAL_DB` points at a copy, exactly
/// like the `storage::tests::real_data_*` probes. Opens the DB **read-only**
/// (`open_database_readonly` — no migration side effect) and times each
/// `compute_company_view` section independently, per company. Diagnostic
/// only: asserts nothing about the numbers.
///
/// Run it manually:
///
/// ```text
/// BRAWLER_REAL_DB=/path/to/a/throwaway/copy.sqlite3 \
///   cargo test -p brawler --lib company_view::tests::real_data_section_timings \
///   -- --ignored --nocapture
/// ```
#[test]
#[ignore = "real-data timing probe; needs BRAWLER_REAL_DB (opened read-only)"]
fn real_data_section_timings() {
    use std::time::Instant;

    let Ok(db_path) = std::env::var("BRAWLER_REAL_DB") else {
        eprintln!("SKIP real_data_section_timings: set BRAWLER_REAL_DB to a throwaway copy");
        return;
    };
    let connection =
        crate::storage::open_database_readonly(&db_path).expect("open real db (read-only)");
    let state = AppState::new(connection);

    let today = format_date(today_date());
    let horizon =
        format_date(today_date().saturating_add(time::Duration::days(EVENTS_WINDOW_DAYS)));

    for company_id in ["company_gpw_cdr", "company_gpw_abe", "company_gpw_ale"] {
        eprintln!("== {company_id} ==");

        // Per-section breakdown over ONE borrowed connection (mirrors what
        // `compute_company_view` itself now does) — dropped before the
        // TOTAL/checkout-count measurement below so that call's own checkout
        // isn't blocked behind this one (single-connection test `AppState`).
        let section_connection = state.checkout_for_tests().expect("checkout");

        let t = Instant::now();
        let counters = compute_counters(&section_connection, company_id, &today, &horizon);
        eprintln!("  counters (flags/claims/shorts/events): {:?}", t.elapsed());
        if let Err(error) = &counters {
            eprintln!("    error: {error}");
        }

        let t = Instant::now();
        let kpi = compute_kpi(&section_connection, company_id);
        eprintln!("  kpi: {:?}", t.elapsed());
        if let Err(error) = &kpi {
            eprintln!("    error: {error}");
        }

        let t = Instant::now();
        let feed = compute_feed(&section_connection, company_id);
        eprintln!("  feed: {:?}", t.elapsed());
        if let Err(error) = &feed {
            eprintln!("    error: {error}");
        }

        let company = reads::companies(&section_connection)
            .expect("companies")
            .into_iter()
            .find(|c| c.id == company_id)
            .expect("company present");
        let t = Instant::now();
        let price = compute_price(&section_connection, &company, company_id);
        eprintln!("  price: {:?}", t.elapsed());
        if let Err(error) = &price {
            eprintln!("    error: {error}");
        }

        let t = Instant::now();
        let coverage = reads::coverage_rows(&section_connection, &state, company_id);
        eprintln!("  coverage: {:?}", t.elapsed());
        if let Err(error) = &coverage {
            eprintln!("    error: {error}");
        }

        let t = Instant::now();
        let recommendations = reads::recommendations(&section_connection, company_id);
        eprintln!("  recommendations: {:?}", t.elapsed());
        if let Err(error) = &recommendations {
            eprintln!("    error: {error}");
        }
        drop(section_connection);

        let checkouts_before = state.checkout_count();
        let t = Instant::now();
        let view = compute_company_view(&state, company_id);
        eprintln!(
            "  TOTAL compute_company_view: {:?} ({} pool checkouts)",
            t.elapsed(),
            state.checkout_count() - checkouts_before
        );
        assert!(view.is_ok(), "compute_company_view failed for {company_id}");
    }
}
