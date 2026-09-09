//! #194 S4: one test per `Fetcher`-arm runtime adapter, proving each pinned
//! entry in [`crate::jobs::source_refresh::OUTCOME_RECORDERS`] really stamps
//! `last_success_at` (DoD §C). Each test calls the real storage entry with a
//! minimal payload and reads the row back via `storage::sources`.

use super::common::*;
use super::*;
use crate::source_adapters::knf_short_selling::KnfShortEntry;

fn tracked_company(state: &AppState) -> Company {
    state
        .create_company(NewCompany {
            exchange: "GPW".to_owned(),
            ticker: "CDR".to_owned(),
            display_name: "CD PROJEKT S.A.".to_owned(),
            isin: Some("PLOPTTC00011".to_owned()),
            cik: None,
            lei: None,
        })
        .expect("company should be created")
}

/// The adapter's mutable catalog row, the way the Sources screen reads it.
fn adapter_row(state: &AppState, adapter_id: &str) -> SourceAdapter {
    state
        .list_source_adapters()
        .expect("adapters should list")
        .into_iter()
        .find(|entry| entry.id == adapter_id)
        .unwrap_or_else(|| panic!("adapter `{adapter_id}` is not registered in the catalog"))
}

fn assert_recorded_success(state: &AppState, adapter_id: &str) {
    let row = adapter_row(state, adapter_id);
    assert!(
        row.last_success_at.is_some(),
        "adapter `{adapter_id}` should have last_success_at set after a successful run"
    );
}

#[test]
fn bankier_company_komunikaty_records_last_success_at() {
    let state = AppState::new(open_in_memory_database().expect("db"));
    let company = tracked_company(&state);
    state
        .ingest_bankier_company_items(&sample_bankier_company_items(&company))
        .expect("bankier company items should ingest");
    assert_recorded_success(&state, "bankier-company-komunikaty");
}

#[test]
fn bankier_kalendarium_html_records_last_success_at() {
    let state = AppState::new(open_in_memory_database().expect("db"));
    let company = tracked_company(&state);
    state
        .ingest_bankier_calendar_event_items(&[BankierCalendarEventItem {
            ticker: company.ticker.clone(),
            event_type: "dividend".to_owned(),
            title: "CDR: Dzień ustalenia prawa do dywidendy.".to_owned(),
            description: "Dzień ustalenia prawa do dywidendy.".to_owned(),
            category: "Dywidendy".to_owned(),
            link: "https://www.bankier.pl/gielda/notowania/akcje/CDR/kalendarium".to_owned(),
            event_date: "2099-06-01".to_owned(),
            fetched_at: "2026-06-01T08:00:00Z".to_owned(),
            source_event_key: "bankier-kalendarium-html:cdr:dywidendy:dywidenda".to_owned(),
        }])
        .expect("bankier calendar events should ingest");
    assert_recorded_success(&state, "bankier-kalendarium-html");
}

#[test]
fn gpw_market_events_rss_records_last_success_at() {
    let state = AppState::new(open_in_memory_database().expect("db"));
    state
        .ingest_gpw_market_event_items(&[GpwMarketEventItem {
            market: "Main Market".to_owned(),
            event_label: "Corporate actions".to_owned(),
            instrument_type: "Equity".to_owned(),
            ticker: "CDR".to_owned(),
            event_type: "corporate_action".to_owned(),
            title: "Main Market - Corporate actions - Equity - CDR".to_owned(),
            link: "https://www.gpw.pl/market-events-calendar?date=2026-06-01".to_owned(),
            event_date: "2099-06-01".to_owned(),
            fetched_at: "2026-06-01T08:00:00Z".to_owned(),
            source_event_key: "gpw-market-events-rss:2099-06-01:corporate-actions:equity:cdr"
                .to_owned(),
        }])
        .expect("gpw market events should ingest");
    assert_recorded_success(&state, "gpw-market-events-rss");
}

#[test]
fn bankier_market_rss_records_last_success_at() {
    let state = AppState::new(open_in_memory_database().expect("db"));
    state
        .ingest_bankier_rss_items(&sample_bankier_items())
        .expect("bankier rss items should ingest");
    assert_recorded_success(&state, "bankier-market-rss");
}

#[test]
fn knf_short_selling_records_last_success_at() {
    let state = AppState::new(open_in_memory_database().expect("db"));
    let company = tracked_company(&state);
    state
        .short_positions()
        .ingest_knf_short_positions(&[KnfShortEntry {
            holder_name: "AKO Capital LLP".to_owned(),
            issuer_name: "CD PROJEKT".to_owned(),
            isin: company.isin.clone().expect("isin"),
            net_position_pct: 0.53,
            position_date: "2026-07-14".to_owned(),
            modify_date: None,
        }])
        .expect("knf short positions should ingest");
    assert_recorded_success(&state, "knf-short-selling");
}

#[test]
fn biznesradar_akcjonariat_records_last_success_at() {
    let state = AppState::new(open_in_memory_database().expect("db"));
    let company = tracked_company(&state);
    state
        .ownership()
        .record_witness_comparisons(
            "biznesradar-akcjonariat",
            &[WitnessComparison {
                company_id: company.id.clone(),
                status: "agree".to_owned(),
                holders_compared: 1,
                divergences: Vec::new(),
            }],
            "2026-07-16T10:00:00Z",
        )
        .expect("witness comparisons should record");
    assert_recorded_success(&state, "biznesradar-akcjonariat");
}

#[test]
fn biznesradar_rekomendacje_records_last_success_at() {
    let state = AppState::new(open_in_memory_database().expect("db"));
    let company = tracked_company(&state);
    state
        .analyst_recommendations()
        .ingest_analyst_recommendations(
            &company.id,
            &[AnalystRecommendationEntry {
                firm: "Noble Securities".to_owned(),
                analyst: Some("Mateusz Chrzanowski".to_owned()),
                rating: "akumuluj".to_owned(),
                target_price: Some("250.00".to_owned()),
                target_currency: Some("PLN".to_owned()),
                price_at_issue: Some("224.70".to_owned()),
                published_at: "2026-06-18T08:40:00".to_owned(),
                source_url: "https://www.biznesradar.pl/rekomendacje-spolki/CDR".to_owned(),
                report_url: None,
            }],
        )
        .expect("analyst recommendations should ingest");
    assert_recorded_success(&state, "biznesradar-rekomendacje");
}

#[test]
fn yahoo_eod_records_last_success_at() {
    let state = AppState::new(open_in_memory_database().expect("db"));
    state
        .market_data()
        .record_pull_outcome("yahoo-eod", "2026-07-16T10:00:00Z", 1, 1, 0)
        .expect("pull outcome should record");
    assert_recorded_success(&state, "yahoo-eod");
}

#[test]
fn gpw_espi_ebi_records_last_success_at() {
    let state = AppState::new(open_in_memory_database().expect("db"));
    tracked_company(&state); // ISIN matches sample_cdr_listing() below
    state
        .reconciliation()
        .reconcile_gpw_espi_witness(&[sample_cdr_listing()])
        .expect("witness reconciliation should record");
    assert_recorded_success(&state, "gpw-espi-ebi");
}

#[test]
fn gpw_company_registry_records_last_success_at() {
    let state = AppState::new(open_in_memory_database().expect("db"));
    state
        .refresh_gpw_company_registry(
            &[registry_entry("CDR", "CD PROJEKT S.A.", "PLOPTTC00011")],
            "2026-07-16T10:00:00Z",
        )
        .expect("gpw company registry should refresh");
    assert_recorded_success(&state, GPW_REGISTRY_ADAPTER_ID);
}

#[test]
fn newconnect_company_directory_records_last_success_at() {
    let state = AppState::new(open_in_memory_database().expect("db"));
    state
        .refresh_newconnect_company_directory(
            &[registry_entry("XYZ", "XYZ S.A.", "PLXYZ0000019")],
            "2026-07-16T10:00:00Z",
        )
        .expect("newconnect company directory should refresh");
    assert_recorded_success(&state, NEWCONNECT_DIRECTORY_ADAPTER_ID);
}

/// Negative pin: a deliberately failing outcome leaves `last_success_at`
/// null but sets the failure fields — the counterpart the positive tests
/// above don't cover.
#[test]
fn a_failing_outcome_leaves_last_success_at_null_and_sets_the_failure_fields() {
    let state = AppState::new(open_in_memory_database().expect("db"));
    state
        .record_source_adapter_error("bankier-market-rss", "HTTP 503 from the publisher")
        .expect("error should record");

    let row = adapter_row(&state, "bankier-market-rss");
    assert!(
        row.last_success_at.is_none(),
        "a failing run must not set last_success_at"
    );
    assert_eq!(
        row.last_error.as_deref(),
        Some("HTTP 503 from the publisher"),
        "the failure message is stated on the adapter's own row"
    );
    assert!(
        row.last_error_at.is_some(),
        "and stamped, so the screen can show when it broke"
    );
}
