//! Insider overview read-model tests (ADR 0083 D7, plan v0.57 T6): the window
//! math (inclusive boundaries, the min-2 rule, the undetermined bucket, the
//! coverage note, the signal-date fallback), the latest-per-person holdings fold,
//! and an insta golden of the full overview for a multi-transaction company.

use rust_decimal::Decimal;
use time::macros::date;

use super::*;
use crate::storage::InsiderTransactionRow;

// --- builders ---------------------------------------------------------------

/// A parsed transaction with everything null but the identity fields; tests set
/// only what they exercise.
fn tx(id: &str, date: Option<&str>) -> InsiderTransactionRow {
    InsiderTransactionRow {
        id: id.to_owned(),
        company_id: "c1".to_owned(),
        feed_item_id: format!("feed_{id}"),
        unit_index: 0,
        person_name_raw: "Jan Testowy".to_owned(),
        person_normalized: "JAN TESTOWY".to_owned(),
        role: Some("management".to_owned()),
        related_pdmr_raw: None,
        related_pdmr_normalized: None,
        related_pdmr_role: None,
        direction: None,
        instrument: Some("shares".to_owned()),
        volume: None,
        price: None,
        currency: None,
        tx_date: date.map(|d| d.to_owned()),
        created_at: "2026-01-01T00:00:00Z".to_owned(),
    }
}

fn src(tx: InsiderTransactionRow) -> InsiderOverviewSource {
    InsiderOverviewSource {
        tx,
        signal_date: None,
        source_url: Some("https://example.test/filing".to_owned()),
    }
}

fn with_dir(mut t: InsiderTransactionRow, dir: &str) -> InsiderTransactionRow {
    t.direction = Some(dir.to_owned());
    t
}

fn with_vol(mut t: InsiderTransactionRow, vol: &str) -> InsiderTransactionRow {
    t.volume = Some(vol.to_owned());
    t
}

const ANCHOR: Date = date!(2026 - 07 - 01);

fn agg_of(overview: &InsiderOverview, window: &str) -> WindowAggregate {
    match window {
        "90d" => overview.window90d.clone(),
        _ => overview.window12m.clone(),
    }
}

fn build(sources: Vec<InsiderOverviewSource>) -> InsiderOverview {
    build_overview("c1", ANCHOR, sources, Vec::new())
}

// --- boundary inclusivity ---------------------------------------------------

#[test]
fn ninety_day_boundary_is_inclusive() {
    // 2026-04-02 is EXACTLY 90 days before 2026-07-01; 2026-04-01 is 91 days.
    let sources = vec![
        src(with_dir(tx("t_anchor", Some("2026-07-01")), "buy")),
        src(with_dir(tx("t_boundary", Some("2026-04-02")), "buy")),
        src(with_dir(tx("t_outside", Some("2026-04-01")), "sell")),
    ];
    let overview = build(sources);
    // Boundary tx is IN, the 91-day-old one is OUT → 2 in-window, both buys.
    match agg_of(&overview, "90d") {
        WindowAggregate::Computed {
            count, buys, sells, ..
        } => {
            assert_eq!(count, 2, "boundary inclusive, 91-day excluded");
            assert_eq!(buys, 2);
            assert_eq!(sells, 0);
        }
        other => panic!("expected computed, got {other:?}"),
    }
}

#[test]
fn twelve_month_boundary_is_inclusive() {
    // 2025-07-01 is exactly 12 months before the anchor (inclusive); 2025-06-30 out.
    let sources = vec![
        src(with_dir(tx("t_anchor", Some("2026-07-01")), "buy")),
        src(with_dir(tx("t_boundary", Some("2025-07-01")), "buy")),
        src(with_dir(tx("t_outside", Some("2025-06-30")), "buy")),
    ];
    let overview = build(sources);
    match agg_of(&overview, "12m") {
        WindowAggregate::Computed { count, buys, .. } => {
            assert_eq!(count, 2, "12m boundary inclusive, day-before excluded");
            assert_eq!(buys, 2);
        }
        other => panic!("expected computed, got {other:?}"),
    }
}

/// The attachment-PDF tier (T4b) fills the NULL `volume` figures the cover note
/// omits. The read model needs no change: its coverage note (`volume_known` /
/// `volume_total`) rises automatically as those NULLs become known. This pins that
/// contract — a fill raises coverage without touching the read model.
#[test]
fn volume_coverage_note_rises_after_t4b_fill() {
    // Two in-window buys, figures still NULL (cover-note-only substrate).
    let before = build(vec![
        src(with_dir(tx("t_a", Some("2026-06-20")), "buy")),
        src(with_dir(tx("t_b", Some("2026-06-25")), "buy")),
    ]);
    match agg_of(&before, "90d") {
        WindowAggregate::Computed {
            volume_known,
            volume_total,
            buy_volume,
            ..
        } => {
            assert_eq!(
                volume_known, 0,
                "no volumes known before the attachment tier"
            );
            assert_eq!(volume_total, 2);
            assert!(buy_volume.is_none());
        }
        other => panic!("expected computed, got {other:?}"),
    }

    // After T4b fills both volumes (same rows, now with a known volume).
    let after = build(vec![
        src(with_vol(
            with_dir(tx("t_a", Some("2026-06-20")), "buy"),
            "1000",
        )),
        src(with_vol(
            with_dir(tx("t_b", Some("2026-06-25")), "buy"),
            "500",
        )),
    ]);
    match agg_of(&after, "90d") {
        WindowAggregate::Computed {
            volume_known,
            volume_total,
            buy_volume,
            ..
        } => {
            assert_eq!(volume_known, 2, "coverage rose as the NULLs became known");
            assert_eq!(volume_total, 2);
            assert_eq!(buy_volume.as_deref(), Some("1500"), "buy volumes summed");
        }
        other => panic!("expected computed, got {other:?}"),
    }
}

// --- min-2 rule -------------------------------------------------------------

#[test]
fn single_transaction_is_below_minimum() {
    let overview = build(vec![src(with_dir(tx("t1", Some("2026-06-01")), "buy"))]);
    assert_eq!(
        agg_of(&overview, "90d"),
        WindowAggregate::BelowMinimum { count: 1 },
        "one in-window tx renders no aggregate"
    );
    // The transaction is still listed in the timeline.
    assert_eq!(overview.transactions.len(), 1);
}

#[test]
fn empty_window_is_below_minimum_zero() {
    let overview = build(Vec::new());
    assert_eq!(
        agg_of(&overview, "90d"),
        WindowAggregate::BelowMinimum { count: 0 }
    );
    assert_eq!(
        agg_of(&overview, "12m"),
        WindowAggregate::BelowMinimum { count: 0 }
    );
}

// --- undetermined bucket ----------------------------------------------------

#[test]
fn null_and_other_direction_land_in_undetermined_not_net() {
    let sources = vec![
        src(with_dir(tx("t_buy", Some("2026-06-10")), "buy")),
        src(tx("t_null", Some("2026-06-11"))), // direction NULL
        src(with_dir(tx("t_other", Some("2026-06-12")), "other")),
    ];
    match agg_of(&build(sources), "90d") {
        WindowAggregate::Computed {
            count,
            buys,
            sells,
            undetermined,
            net,
            ..
        } => {
            assert_eq!(count, 3);
            assert_eq!(buys, 1);
            assert_eq!(sells, 0);
            assert_eq!(undetermined, 2, "NULL + other both undetermined");
            assert_eq!(net, 1, "directionless never moves the net");
        }
        other => panic!("expected computed, got {other:?}"),
    }
}

// --- volume coverage + sums -------------------------------------------------

#[test]
fn volume_coverage_and_directional_sums() {
    let sources = vec![
        src(with_vol(
            with_dir(tx("t_b1", Some("2026-06-01")), "buy"),
            "100",
        )),
        src(with_vol(
            with_dir(tx("t_b2", Some("2026-06-02")), "buy"),
            "50",
        )),
        // A sell with a known volume.
        src(with_vol(
            with_dir(tx("t_s1", Some("2026-06-03")), "sell"),
            "30",
        )),
        // A buy with NO known volume — counted, but not in the coverage numerator.
        src(with_dir(tx("t_b3", Some("2026-06-04")), "buy")),
    ];
    match agg_of(&build(sources), "90d") {
        WindowAggregate::Computed {
            count,
            buys,
            sells,
            buy_volume,
            sell_volume,
            volume_known,
            volume_total,
            ..
        } => {
            assert_eq!(count, 4);
            assert_eq!(buys, 3);
            assert_eq!(sells, 1);
            assert_eq!(buy_volume.as_deref(), Some("150"), "100 + 50");
            assert_eq!(sell_volume.as_deref(), Some("30"));
            assert_eq!(volume_known, 3, "3 of 4 disclosed a volume");
            assert_eq!(volume_total, 4);
        }
        other => panic!("expected computed, got {other:?}"),
    }
}

#[test]
fn no_known_volume_leaves_sums_null() {
    let sources = vec![
        src(with_dir(tx("t_b1", Some("2026-06-01")), "buy")),
        src(with_dir(tx("t_b2", Some("2026-06-02")), "buy")),
    ];
    match agg_of(&build(sources), "90d") {
        WindowAggregate::Computed {
            buy_volume,
            sell_volume,
            volume_known,
            ..
        } => {
            assert!(buy_volume.is_none(), "no known volume → null, never 0");
            assert!(sell_volume.is_none());
            assert_eq!(volume_known, 0);
        }
        other => panic!("expected computed, got {other:?}"),
    }
}

// --- signal-date fallback ---------------------------------------------------

#[test]
fn signal_date_fallback_places_and_labels_transaction() {
    // Two txs with NO tx_date but a filing signal_date inside the window.
    let mut a = tx("t_a", None);
    a = with_dir(a, "buy");
    let mut sa = src(a);
    sa.signal_date = Some("2026-06-01".to_owned());

    let mut b = tx("t_b", None);
    b = with_dir(b, "sell");
    let mut sb = src(b);
    sb.signal_date = Some("2026-06-02".to_owned());

    let overview = build(vec![sa, sb]);
    // Both placed via the filing date → an aggregate renders.
    match agg_of(&overview, "90d") {
        WindowAggregate::Computed {
            count,
            buys,
            sells,
            net,
            ..
        } => {
            assert_eq!(count, 2);
            assert_eq!(buys, 1);
            assert_eq!(sells, 1);
            assert_eq!(net, 0);
        }
        other => panic!("expected computed, got {other:?}"),
    }
    // The timeline labels the fallback honestly.
    let entry = overview
        .transactions
        .iter()
        .find(|e| e.id == "t_a")
        .unwrap();
    assert_eq!(entry.date_source, "filing");
    assert_eq!(entry.effective_date.as_deref(), Some("2026-06-01"));
    assert!(entry.tx_date.is_none());
}

#[test]
fn no_date_at_all_is_listed_but_excluded_from_windows() {
    // One dateless tx + one in-window dated tx: timeline shows both, but the
    // dateless one never counts toward a window (would be a fabricated placement).
    let mut dateless = src(with_dir(tx("t_none", None), "buy"));
    dateless.signal_date = None;
    let sources = vec![
        dateless,
        src(with_dir(tx("t_dated", Some("2026-06-01")), "buy")),
    ];
    let overview = build(sources);
    assert_eq!(overview.transactions.len(), 2, "both in the timeline");
    // Only one is in-window → below minimum.
    assert_eq!(
        agg_of(&overview, "90d"),
        WindowAggregate::BelowMinimum { count: 1 }
    );
    let none = overview
        .transactions
        .iter()
        .find(|e| e.id == "t_none")
        .unwrap();
    assert_eq!(none.date_source, "unknown");
    assert!(none.effective_date.is_none());
}

// --- holdings fold ----------------------------------------------------------

fn holding(person: &str, as_of: &str, shares: Option<&str>) -> ManagementHoldingRow {
    ManagementHoldingRow {
        id: format!("mh_{person}_{as_of}"),
        company_id: "c1".to_owned(),
        report_document_id: format!("doc_{as_of}"),
        person_name_raw: person.to_owned(),
        person_normalized: person.to_uppercase(),
        role: Some("management".to_owned()),
        shares: shares.map(|s| s.to_owned()),
        indirect_via_raw: None,
        indirect_via_normalized: None,
        prior_shares: None,
        prior_as_of: None,
        as_of: as_of.to_owned(),
        is_zero_aggregate: false,
    }
}

#[test]
fn holdings_keep_latest_disclosure_per_person() {
    let rows = vec![
        holding("Anna Kowalska", "2024-12-31", Some("1000")),
        holding("Anna Kowalska", "2025-12-31", Some("1500")),
        holding("Bob Nowak", "2025-12-31", Some("0")),
    ];
    let entries = latest_holdings(rows);
    assert_eq!(entries.len(), 2, "one row per person");
    let anna = entries
        .iter()
        .find(|e| e.person == "Anna Kowalska")
        .unwrap();
    assert_eq!(anna.shares.as_deref(), Some("1500"), "latest as_of wins");
    assert_eq!(anna.as_of, "2025-12-31");
    let bob = entries.iter().find(|e| e.person == "Bob Nowak").unwrap();
    assert_eq!(bob.shares.as_deref(), Some("0"), "explicit zero is kept");
}

#[test]
fn holdings_preserve_null_shares_and_indirect_via() {
    let mut founder = holding("Marek Góral", "2025-12-31", None);
    founder.indirect_via_raw = Some("Góral Fundacja Rodzinna".to_owned());
    let entries = latest_holdings(vec![founder]);
    assert_eq!(entries.len(), 1);
    assert!(entries[0].shares.is_none(), "NULL stays NULL, never 0");
    assert_eq!(
        entries[0].indirect_via.as_deref(),
        Some("Góral Fundacja Rodzinna")
    );
}

// --- decimal exactness guard ------------------------------------------------

#[test]
fn volume_sum_is_decimal_exact() {
    // Two fractional volumes that would drift under f64 addition.
    let sources = vec![
        src(with_vol(
            with_dir(tx("t1", Some("2026-06-01")), "buy"),
            "0.1",
        )),
        src(with_vol(
            with_dir(tx("t2", Some("2026-06-02")), "buy"),
            "0.2",
        )),
    ];
    match agg_of(&build(sources), "90d") {
        WindowAggregate::Computed { buy_volume, .. } => {
            assert_eq!(buy_volume.as_deref(), Some("0.3"), "exact, not 0.30000004");
            // Sanity: the value parses back to exactly 0.3.
            assert_eq!(Decimal::from_str("0.3").unwrap(), Decimal::new(3, 1));
        }
        other => panic!("expected computed, got {other:?}"),
    }
}

// --- insta golden: full overview for a multi-tx company ---------------------

#[test]
fn insider_overview_full_sample_golden() {
    let founder = {
        let mut h = holding("Marek Góral", "2025-12-31", None);
        h.indirect_via_raw = Some("Góral Fundacja Rodzinna".to_owned());
        h
    };
    let holdings = vec![
        founder,
        holding("Anna Kowalska", "2025-12-31", Some("12000")),
        holding("Anna Kowalska", "2024-12-31", Some("10000")),
    ];
    let sources = vec![
        src(with_vol(
            with_dir(tx("t1", Some("2026-06-20")), "buy"),
            "500",
        )),
        src(with_dir(tx("t2", Some("2026-06-10")), "buy")),
        src(with_vol(
            with_dir(tx("t3", Some("2026-05-15")), "sell"),
            "200",
        )),
        src(with_dir(tx("t4", Some("2025-08-01")), "buy")), // in 12m, out of 90d
        {
            // A closely-associated filing anchored to a PDMR, dated via signal_date.
            let mut t = tx("t5", None);
            t.role = Some("closely_associated".to_owned());
            t.related_pdmr_raw = Some("Marek Góral".to_owned());
            t.direction = Some("buy".to_owned());
            let mut s = src(t);
            s.signal_date = Some("2026-06-25".to_owned());
            s
        },
    ];
    let overview = build_overview("company_gpw_gtn", ANCHOR, sources, holdings);
    insta::assert_debug_snapshot!("insider_overview_full_sample", overview);
}
