//! Analyst-recommendations ingest/dedupe/signal/derivation behavior
//! (ADR 0073, plan v0.58 A1).

use super::*;
use crate::storage::AnalystRecommendationEntry;
use proptest::prelude::*;

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

fn setup() -> (AppState, Company) {
    let state = AppState::new(open_in_memory_database().expect("database should initialize"));
    let company = tracked_company(&state);
    (state, company)
}

fn entry(
    firm: &str,
    rating: &str,
    target: Option<&str>,
    published_at: &str,
) -> AnalystRecommendationEntry {
    AnalystRecommendationEntry {
        firm: firm.to_owned(),
        analyst: Some("Jan Kowalski".to_owned()),
        rating: rating.to_owned(),
        target_price: target.map(str::to_owned),
        target_currency: Some("PLN".to_owned()),
        price_at_issue: Some("100.00".to_owned()),
        published_at: published_at.to_owned(),
        source_url: "https://www.biznesradar.pl/rekomendacje-spolki/CDPROJEKT".to_owned(),
        report_url: Some("https://www.biznesradar.pl/storage/report.pdf".to_owned()),
    }
}

fn count(state: &AppState, sql: &str) -> i64 {
    let connection = state.checkout().expect("connection");
    connection
        .query_row(sql, [], |row| row.get::<_, i64>(0))
        .expect("count query")
}

fn count_rows(state: &AppState) -> i64 {
    count(state, "SELECT COUNT(*) FROM analyst_recommendations")
}

fn count_signals(state: &AppState) -> i64 {
    count(
        state,
        "SELECT COUNT(*) FROM company_signals WHERE category = 'recommendation_change'",
    )
}

fn count_feed(state: &AppState) -> i64 {
    count(
        state,
        "SELECT COUNT(*) FROM feed_items WHERE source_adapter_id = 'biznesradar-rekomendacje'",
    )
}

#[test]
fn ingest_creates_row_feed_and_signal() {
    let (state, company) = setup();

    let outcome = state
        .analyst_recommendations()
        .ingest_analyst_recommendations(
            &company.id,
            &[entry(
                "DM BOŚ",
                "akumuluj",
                Some("120.00"),
                "2026-06-18T08:40:00Z",
            )],
        )
        .expect("ingest should succeed");

    assert_eq!(outcome.items_created, 1);
    assert_eq!(count_rows(&state), 1);
    assert_eq!(count_feed(&state), 1);
    assert_eq!(count_signals(&state), 1);

    let rows = state
        .analyst_recommendations()
        .list_analyst_recommendations(&company.id)
        .expect("list");
    assert_eq!(rows.len(), 1);
    assert_eq!(rows[0].direction, "initiate");
    assert_eq!(rows[0].rating, "akumuluj");
    assert!(rows[0].rating_prev.is_none());
}

#[test]
fn re_ingesting_the_same_page_is_a_no_op() {
    let (state, company) = setup();
    let page = [
        entry("DM BOŚ", "akumuluj", Some("120.00"), "2026-06-18T08:40:00Z"),
        entry("Trigon", "kupuj", Some("150.00"), "2026-06-10T09:00:00Z"),
    ];
    let store = state.analyst_recommendations();

    store
        .ingest_analyst_recommendations(&company.id, &page)
        .expect("first ingest");
    assert_eq!(count_rows(&state), 2);
    assert_eq!(count_feed(&state), 2);
    assert_eq!(count_signals(&state), 2);

    // Same page again: no new rows, feed items, or signals.
    let outcome = store
        .ingest_analyst_recommendations(&company.id, &page)
        .expect("re-ingest");
    assert_eq!(outcome.items_created, 0);
    assert_eq!(count_rows(&state), 2);
    assert_eq!(count_feed(&state), 2);
    assert_eq!(count_signals(&state), 2);
}

#[test]
fn exactly_one_signal_per_new_entry() {
    let (state, company) = setup();
    let store = state.analyst_recommendations();

    store
        .ingest_analyst_recommendations(
            &company.id,
            &[entry(
                "DM BOŚ",
                "akumuluj",
                Some("120.00"),
                "2026-06-18T08:40:00Z",
            )],
        )
        .expect("first");
    assert_eq!(count_signals(&state), 1);

    // A genuinely new entry (later date, different rating) adds exactly one more.
    store
        .ingest_analyst_recommendations(
            &company.id,
            &[entry(
                "DM BOŚ",
                "kupuj",
                Some("140.00"),
                "2026-07-01T08:40:00Z",
            )],
        )
        .expect("second");
    assert_eq!(count_rows(&state), 2);
    assert_eq!(count_signals(&state), 2);
}

#[test]
fn last_success_at_is_set_after_ingest() {
    let (state, company) = setup();

    let before: Option<String> = {
        let connection = state.checkout().expect("connection");
        connection
            .query_row(
                "SELECT last_success_at FROM source_adapters WHERE id = 'biznesradar-rekomendacje'",
                [],
                |row| row.get(0),
            )
            .expect("adapter row seeded by migration 0100")
    };
    assert!(before.is_none(), "no success before the first ingest");

    state
        .analyst_recommendations()
        .ingest_analyst_recommendations(
            &company.id,
            &[entry(
                "DM BOŚ",
                "akumuluj",
                Some("120.00"),
                "2026-06-18T08:40:00Z",
            )],
        )
        .expect("ingest");

    let after: Option<String> = {
        let connection = state.checkout().expect("connection");
        connection
            .query_row(
                "SELECT last_success_at FROM source_adapters WHERE id = 'biznesradar-rekomendacje'",
                [],
                |row| row.get(0),
            )
            .expect("adapter row")
    };
    assert!(
        after.is_some(),
        "record_source_outcome must set last_success_at"
    );
}

#[test]
fn list_orders_by_published_at_not_created_at() {
    let (state, company) = setup();
    let store = state.analyst_recommendations();

    // Insert in an order where created_at (insertion) differs from published_at:
    // the NEWEST publication is ingested FIRST (earliest created_at).
    store
        .ingest_analyst_recommendations(
            &company.id,
            &[entry(
                "Trigon",
                "kupuj",
                Some("150.00"),
                "2026-07-15T09:00:00Z",
            )],
        )
        .expect("newest published, ingested first");
    store
        .ingest_analyst_recommendations(
            &company.id,
            &[entry(
                "DM BOŚ",
                "trzymaj",
                Some("100.00"),
                "2026-05-01T09:00:00Z",
            )],
        )
        .expect("oldest published, ingested second");

    let rows = store
        .list_analyst_recommendations(&company.id)
        .expect("list");
    assert_eq!(rows.len(), 2);
    // published_at DESC: the July entry first despite being created earlier.
    assert_eq!(rows[0].published_at, "2026-07-15T09:00:00Z");
    assert_eq!(rows[1].published_at, "2026-05-01T09:00:00Z");
}

#[test]
fn derives_direction_from_prior_same_firm_entry() {
    let (state, company) = setup();
    let store = state.analyst_recommendations();

    // Same firm timeline: trzymaj -> kupuj (upgrade), then kupuj -> akumuluj?
    // Use trzymaj(3) -> kupuj(5) = upgrade; akumuluj(4) -> redukuj(2) = downgrade.
    store
        .ingest_analyst_recommendations(
            &company.id,
            &[entry(
                "DM BOŚ",
                "trzymaj",
                Some("100.00"),
                "2026-05-01T09:00:00Z",
            )],
        )
        .expect("initiate");
    store
        .ingest_analyst_recommendations(
            &company.id,
            &[entry(
                "DM BOŚ",
                "kupuj",
                Some("130.00"),
                "2026-06-01T09:00:00Z",
            )],
        )
        .expect("upgrade");
    // A separate firm to exercise downgrade independently.
    store
        .ingest_analyst_recommendations(
            &company.id,
            &[entry(
                "Trigon",
                "akumuluj",
                Some("140.00"),
                "2026-05-01T09:00:00Z",
            )],
        )
        .expect("initiate trigon");
    store
        .ingest_analyst_recommendations(
            &company.id,
            &[entry(
                "Trigon",
                "redukuj",
                Some("90.00"),
                "2026-06-01T09:00:00Z",
            )],
        )
        .expect("downgrade trigon");
    // Reiterate: same firm, same rating, same target.
    store
        .ingest_analyst_recommendations(
            &company.id,
            &[entry(
                "DM BOŚ",
                "kupuj",
                Some("130.00"),
                "2026-07-01T09:00:00Z",
            )],
        )
        .expect("reiterate");

    let rows = store
        .list_analyst_recommendations(&company.id)
        .expect("list");
    let by = |firm: &str, date: &str| {
        rows.iter()
            .find(|r| r.firm == firm && r.published_at == date)
            .cloned()
            .unwrap_or_else(|| panic!("missing {firm} {date}"))
    };

    let bos_initiate = by("DM BOŚ", "2026-05-01T09:00:00Z");
    assert_eq!(bos_initiate.direction, "initiate");
    assert!(bos_initiate.rating_prev.is_none());

    let bos_upgrade = by("DM BOŚ", "2026-06-01T09:00:00Z");
    assert_eq!(bos_upgrade.direction, "upgrade");
    assert_eq!(bos_upgrade.rating_prev.as_deref(), Some("trzymaj"));
    assert_eq!(bos_upgrade.target_prev.as_deref(), Some("100.00"));

    let trigon_downgrade = by("Trigon", "2026-06-01T09:00:00Z");
    assert_eq!(trigon_downgrade.direction, "downgrade");
    assert_eq!(trigon_downgrade.rating_prev.as_deref(), Some("akumuluj"));

    let bos_reiterate = by("DM BOŚ", "2026-07-01T09:00:00Z");
    assert_eq!(bos_reiterate.direction, "reiterate");
    assert_eq!(bos_reiterate.rating_prev.as_deref(), Some("kupuj"));
}

#[test]
fn latest_target_returns_newest_entry_with_a_target() {
    let (state, company) = setup();
    let store = state.analyst_recommendations();

    store
        .ingest_analyst_recommendations(
            &company.id,
            &[entry(
                "DM BOŚ",
                "kupuj",
                Some("120.00"),
                "2026-05-01T09:00:00Z",
            )],
        )
        .expect("older with target");
    // A newer entry WITHOUT a target must not shadow the older target.
    store
        .ingest_analyst_recommendations(
            &company.id,
            &[entry("Trigon", "trzymaj", None, "2026-06-01T09:00:00Z")],
        )
        .expect("newer without target");
    store
        .ingest_analyst_recommendations(
            &company.id,
            &[entry(
                "Ipopema",
                "akumuluj",
                Some("155.50"),
                "2026-05-20T09:00:00Z",
            )],
        )
        .expect("middle with target");

    let target = store
        .latest_target(&company.id)
        .expect("latest_target")
        .expect("a target should exist");
    // Newest entry that HAS a target is Ipopema's (2026-05-20), not the targetless
    // 2026-06-01 Trigon one.
    assert_eq!(target.firm, "Ipopema");
    assert_eq!(target.target_price, "155.50");
    assert_eq!(target.published_at, "2026-05-20T09:00:00Z");
}

proptest! {
    /// Re-ingest idempotence (ADR 0049): ingesting a page, then ingesting the same
    /// page again, never changes the row / signal / feed counts.
    #[test]
    fn re_ingest_is_idempotent(
        entries in prop::collection::vec(
            (
                prop::sample::select(vec!["DM BOŚ", "Trigon", "Ipopema", "mBank"]),
                prop::sample::select(vec!["kupuj", "akumuluj", "trzymaj", "redukuj", "sprzedaj"]),
                prop::option::of(prop::sample::select(vec!["90.00", "120.00", "155.50"])),
                prop::sample::select(vec![
                    "2026-05-01T09:00:00Z",
                    "2026-06-01T09:00:00Z",
                    "2026-07-01T09:00:00Z",
                ]),
            ),
            0..8,
        ),
    ) {
        let state = AppState::new(open_in_memory_database().expect("db"));
        let company = tracked_company(&state);
        let store = state.analyst_recommendations();
        let page: Vec<AnalystRecommendationEntry> = entries
            .into_iter()
            .map(|(firm, rating, target, date)| entry(firm, rating, target, date))
            .collect();

        store
            .ingest_analyst_recommendations(&company.id, &page)
            .expect("first ingest");
        let rows = count_rows(&state);
        let signals = count_signals(&state);
        let feed = count_feed(&state);
        // Rows and signals stay in lockstep (one signal per stored row).
        prop_assert_eq!(rows, signals);
        prop_assert_eq!(rows, feed);

        store
            .ingest_analyst_recommendations(&company.id, &page)
            .expect("re-ingest");
        prop_assert_eq!(count_rows(&state), rows);
        prop_assert_eq!(count_signals(&state), signals);
        prop_assert_eq!(count_feed(&state), feed);
    }
}
