//! Comparative-valuation transform tests (ADR 0049): a hand-computed golden over
//! a seeded 5-peer sector, range-ordering + determinism proptests, and the typed
//! absence paths. All numbers in the golden are computed by hand in the comments.

use super::*;
use rust_decimal::prelude::FromStr;

fn d(s: &str) -> Decimal {
    Decimal::from_str(s).expect("decimal")
}

/// Target drivers used by the golden: EPS = 100/50 = 2, BVPS = 1000/50 = 20,
/// EBITDA = 200, net debt = 300, shares = 50.
fn golden_drivers() -> ValuationDrivers {
    ValuationDrivers {
        shares_outstanding: Some(d("50")),
        net_profit_ttm: Some(d("100")),
        total_equity: Some(d("1000")),
        ebitda_ttm: Some(d("200")),
        net_debt: Some(d("300")),
    }
}

/// Target `t` plus four peers a,b,c,d. Peer multiples (excluding the target):
/// P/E {8,10,12,14}, EV/EBITDA {4,5,6,7}, P/BV {1,1.5,2,2.5}.
fn golden_peers() -> Vec<PeerMultiples> {
    let mut t = PeerMultiples::new("t");
    // The target's own multiples must NOT influence the peer medians.
    t.pe = Some(d("99"));
    t.ev_ebitda = Some(d("99"));
    t.pbv = Some(d("99"));
    let peer = |id: &str, pe: &str, ev: &str, pbv: &str| {
        let mut p = PeerMultiples::new(id);
        p.pe = Some(d(pe));
        p.ev_ebitda = Some(d(ev));
        p.pbv = Some(d(pbv));
        p
    };
    vec![
        t,
        peer("a", "8", "4", "1"),
        peer("b", "10", "5", "1.5"),
        peer("c", "12", "6", "2"),
        peer("d", "14", "7", "2.5"),
    ]
}

fn golden() -> ComparativeValuation {
    compute_comparative_valuation(
        "t",
        Some("software"),
        Some(d("30")),
        "2026-07-27",
        5,
        &golden_drivers(),
        &golden_peers(),
        d("1"),
    )
}

#[test]
fn golden_pe_method_matches_hand_computation() {
    let result = golden();
    let pe = method_of(&result, ValuationMethod::PeMultiple).expect("pe");
    // Peer P/E sorted [8,10,12,14], linear percentiles:
    //   P25 h=(4-1)*0.25=0.75 → 8 + 0.75*(10-8) = 9.5
    //   P50 h=1.5             → 10 + 0.5*(12-10) = 11
    //   P75 h=2.25            → 12 + 0.25*(14-12) = 12.5
    assert_eq!(pe.peer_multiple_low.as_deref(), Some("9.5"));
    assert_eq!(pe.peer_multiple_base.as_deref(), Some("11"));
    assert_eq!(pe.peer_multiple_high.as_deref(), Some("12.5"));
    // fair/share = multiple × net_profit_ttm / shares = multiple × 100/50 = ×2.
    assert_eq!(pe.fair_low.as_deref(), Some("19"));
    assert_eq!(pe.fair_base.as_deref(), Some("22"));
    assert_eq!(pe.fair_high.as_deref(), Some("25"));
    assert_eq!(pe.driver_key, "net_profit_ttm");
    assert_eq!(pe.driver_value.as_deref(), Some("100"));
    assert_eq!(pe.peer_sample_size, 4);
    assert_eq!(pe.absent_reason, None);
}

#[test]
fn golden_pbv_method_matches_hand_computation() {
    let result = golden();
    let pbv = method_of(&result, ValuationMethod::PbvMultiple).expect("pbv");
    // Peer P/BV sorted [1,1.5,2,2.5]: P25=1.375, P50=1.75, P75=2.125.
    assert_eq!(pbv.peer_multiple_low.as_deref(), Some("1.375"));
    assert_eq!(pbv.peer_multiple_base.as_deref(), Some("1.75"));
    assert_eq!(pbv.peer_multiple_high.as_deref(), Some("2.125"));
    // fair/share = multiple × total_equity / shares = multiple × 1000/50 = ×20.
    assert_eq!(pbv.fair_low.as_deref(), Some("27.5"));
    assert_eq!(pbv.fair_base.as_deref(), Some("35"));
    assert_eq!(pbv.fair_high.as_deref(), Some("42.5"));
}

#[test]
fn golden_ev_ebitda_method_applies_the_equity_bridge() {
    let result = golden();
    let ev = method_of(&result, ValuationMethod::EvEbitdaMultiple).expect("ev");
    // Peer EV/EBITDA sorted [4,5,6,7]: P25=4.75, P50=5.5, P75=6.25.
    assert_eq!(ev.peer_multiple_low.as_deref(), Some("4.75"));
    assert_eq!(ev.peer_multiple_base.as_deref(), Some("5.5"));
    assert_eq!(ev.peer_multiple_high.as_deref(), Some("6.25"));
    // implied_ev = multiple × ebitda(200); equity = ev − net_debt(300); /shares(50):
    //   low  4.75*200=950 −300=650 /50 = 13
    //   base 5.5*200=1100 −300=800 /50 = 16
    //   high 6.25*200=1250 −300=950 /50 = 19
    assert_eq!(ev.fair_low.as_deref(), Some("13"));
    assert_eq!(ev.fair_base.as_deref(), Some("16"));
    assert_eq!(ev.fair_high.as_deref(), Some("19"));
    assert_eq!(ev.driver_key, "ebitda_ttm");
}

#[test]
fn golden_convergence_and_grade_match_hand_computation() {
    let result = golden();
    // Bases P/E=22, EV/EBITDA=16, P/BV=35 → min 16, max 35, median 22.
    // spread% = (35-16)/22*100 = 86.36.
    let conv = result.convergence.as_ref().expect("convergence");
    assert_eq!(conv.base_low, "16");
    assert_eq!(conv.base_high, "35");
    assert_eq!(conv.spread_pct, "86.36");
    assert_eq!(conv.method_count, 3);
    // completeness=3/3=1; peer_depth=min(1,5/4)=1; convergence=(100-86.36)/100=0.1364;
    // validation=1. composite = .30 + .25 + .25*0.1364 + .20 = 0.7841 → grade B.
    let c = &result.confidence;
    assert_eq!(c.data_completeness, "1");
    assert_eq!(c.peer_depth, "1");
    assert_eq!(c.method_convergence, "0.1364");
    assert_eq!(c.validation, "1");
    assert_eq!(c.composite, "0.7841");
    assert_eq!(c.grade, ConfidenceGradeLetter::B);
}

#[test]
fn ranges_are_ordered_low_le_base_le_high() {
    let result = golden();
    for m in &result.methods {
        if let (Some(low), Some(base), Some(high)) = (
            m.fair_low.as_deref(),
            m.fair_base.as_deref(),
            m.fair_high.as_deref(),
        ) {
            let (low, base, high) = (d(low), d(base), d(high));
            assert!(low <= base, "{:?}: low {low} <= base {base}", m.method);
            assert!(base <= high, "{:?}: base {base} <= high {high}", m.method);
        }
    }
}

#[test]
fn no_sector_is_typed_empty_reason() {
    let result = compute_comparative_valuation(
        "t",
        None,
        None,
        "2026-07-27",
        0,
        &golden_drivers(),
        &[],
        d("1"),
    );
    assert_eq!(result.empty_reason, Some(ValuationEmptyReason::NoSector));
    assert_eq!(result.peer_count, 0);
    assert!(result.thin);
    // Three method rows still present, each an insufficient-peers absence.
    assert_eq!(result.methods.len(), 3);
    for m in &result.methods {
        assert_eq!(m.absent_reason, Some(MethodAbsentReason::InsufficientPeers));
    }
    assert_eq!(result.confidence.grade, ConfidenceGradeLetter::D);
}

#[test]
fn thin_flag_flips_at_four_tracked_companies() {
    let three = compute_comparative_valuation(
        "t",
        Some("software"),
        None,
        "2026-07-27",
        3,
        &golden_drivers(),
        &golden_peers(),
        d("1"),
    );
    assert!(three.thin, "3 tracked is thin");
    let four = compute_comparative_valuation(
        "t",
        Some("software"),
        None,
        "2026-07-27",
        4,
        &golden_drivers(),
        &golden_peers(),
        d("1"),
    );
    assert!(!four.thin, "4 tracked is not thin");
}

#[test]
fn missing_primary_driver_is_no_driver() {
    let mut drivers = golden_drivers();
    drivers.net_profit_ttm = None;
    let result = compute_comparative_valuation(
        "t",
        Some("software"),
        None,
        "d",
        5,
        &drivers,
        &golden_peers(),
        d("1"),
    );
    let pe = method_of(&result, ValuationMethod::PeMultiple).expect("pe");
    assert_eq!(pe.absent_reason, Some(MethodAbsentReason::NoDriver));
    assert_eq!(pe.fair_base, None);
    // The other methods are unaffected.
    let pbv = method_of(&result, ValuationMethod::PbvMultiple).expect("pbv");
    assert_eq!(pbv.absent_reason, None);
}

#[test]
fn non_positive_driver_is_typed_never_nan() {
    let mut drivers = golden_drivers();
    drivers.net_profit_ttm = Some(d("-100")); // loss-making
    let result = compute_comparative_valuation(
        "t",
        Some("software"),
        None,
        "d",
        5,
        &drivers,
        &golden_peers(),
        d("1"),
    );
    let pe = method_of(&result, ValuationMethod::PeMultiple).expect("pe");
    assert_eq!(
        pe.absent_reason,
        Some(MethodAbsentReason::NonPositiveDriver)
    );
}

#[test]
fn ev_ebitda_without_net_debt_drops_to_typed_absence() {
    let mut drivers = golden_drivers();
    drivers.net_debt = None; // no equity-bridge data
    let result = compute_comparative_valuation(
        "t",
        Some("software"),
        None,
        "d",
        5,
        &drivers,
        &golden_peers(),
        d("1"),
    );
    let ev = method_of(&result, ValuationMethod::EvEbitdaMultiple).expect("ev");
    assert_eq!(ev.absent_reason, Some(MethodAbsentReason::NoDriver));
    // The multiple-only methods still compute (honest subset).
    let pe = method_of(&result, ValuationMethod::PeMultiple).expect("pe");
    assert_eq!(pe.absent_reason, None);
}

#[test]
fn fewer_than_two_peers_is_insufficient_peers() {
    // Only one other peer has a P/E defined.
    let mut t = PeerMultiples::new("t");
    t.pe = Some(d("99"));
    let mut a = PeerMultiples::new("a");
    a.pe = Some(d("10"));
    let peers = vec![t, a, PeerMultiples::new("b"), PeerMultiples::new("c")];
    let result = compute_comparative_valuation(
        "t",
        Some("software"),
        None,
        "d",
        4,
        &golden_drivers(),
        &peers,
        d("1"),
    );
    let pe = method_of(&result, ValuationMethod::PeMultiple).expect("pe");
    assert_eq!(
        pe.absent_reason,
        Some(MethodAbsentReason::InsufficientPeers)
    );
    assert_eq!(pe.peer_sample_size, 1);
}

#[test]
fn ev_ebitda_equity_bridge_floors_negative_equity_at_zero() {
    // Net debt dwarfs implied EV: implied equity would be negative → floor 0.
    let mut drivers = golden_drivers();
    drivers.net_debt = Some(d("100000"));
    let result = compute_comparative_valuation(
        "t",
        Some("software"),
        None,
        "d",
        5,
        &drivers,
        &golden_peers(),
        d("1"),
    );
    let ev = method_of(&result, ValuationMethod::EvEbitdaMultiple).expect("ev");
    assert_eq!(ev.fair_low.as_deref(), Some("0"));
    assert_eq!(ev.fair_base.as_deref(), Some("0"));
    assert_eq!(ev.fair_high.as_deref(), Some("0"));
}

#[test]
fn golden_snapshot_is_stable() {
    insta::assert_debug_snapshot!("comparative_valuation_five_peers", golden());
}

mod proptests {
    //! Data-transform invariants (ADR 0049): the result depends only on the
    //! *set* of peers (order-independent), is idempotent, ranges are ordered
    //! `low ≤ base ≤ high`, the grade composite stays in `[0,1]`, and it never
    //! panics on degenerate / missing-data / permuted inputs.
    use super::*;
    use proptest::prelude::*;
    use rust_decimal::prelude::FromPrimitive;

    fn opt_dec() -> impl Strategy<Value = Option<Decimal>> {
        proptest::option::of((-50i64..200).prop_map(|n| Decimal::from_i64(n).unwrap()))
    }

    fn drivers_strategy() -> impl Strategy<Value = ValuationDrivers> {
        (opt_dec(), opt_dec(), opt_dec(), opt_dec(), opt_dec()).prop_map(
            |(shares, np, eq, ebitda, nd)| ValuationDrivers {
                shares_outstanding: shares,
                net_profit_ttm: np,
                total_equity: eq,
                ebitda_ttm: ebitda,
                net_debt: nd,
            },
        )
    }

    fn peer_strategy() -> impl Strategy<Value = PeerMultiples> {
        (
            prop::sample::select(vec!["t", "a", "b", "c", "d", "e"]),
            opt_dec(),
            opt_dec(),
            opt_dec(),
        )
            .prop_map(|(id, pe, ev, pbv)| PeerMultiples {
                company_id: id.to_owned(),
                pe,
                ev_ebitda: ev,
                pbv,
            })
    }

    fn peers_strategy() -> impl Strategy<Value = Vec<PeerMultiples>> {
        proptest::collection::vec(peer_strategy(), 0..12)
    }

    proptest! {
        #[test]
        fn never_panics_ranges_ordered_composite_bounded(
            drivers in drivers_strategy(),
            peers in peers_strategy(),
            has_sector in any::<bool>(),
            peer_count in 0u32..12,
        ) {
            let sector = if has_sector { Some("software") } else { None };
            let result = compute_comparative_valuation(
                "t", sector, None, "2026-07-27", peer_count, &drivers, &peers, Decimal::ONE,
            );
            prop_assert_eq!(result.methods.len(), 3);
            for m in &result.methods {
                if let (Some(low), Some(base), Some(high)) =
                    (&m.fair_low, &m.fair_base, &m.fair_high)
                {
                    let low = low.parse::<Decimal>().unwrap();
                    let base = base.parse::<Decimal>().unwrap();
                    let high = high.parse::<Decimal>().unwrap();
                    prop_assert!(low <= base, "low {low} <= base {base}");
                    prop_assert!(base <= high, "base {base} <= high {high}");
                }
            }
            let composite = result.confidence.composite.parse::<Decimal>().unwrap();
            prop_assert!(composite >= Decimal::ZERO && composite <= Decimal::ONE);
        }

        #[test]
        fn idempotent(drivers in drivers_strategy(), peers in peers_strategy()) {
            let go = || compute_comparative_valuation(
                "t", Some("software"), None, "d", 5, &drivers, &peers, Decimal::ONE,
            );
            prop_assert_eq!(go(), go());
        }

        #[test]
        fn peer_order_does_not_change_the_result(
            drivers in drivers_strategy(),
            mut peers in peers_strategy(),
            seed in any::<u64>(),
        ) {
            let original = compute_comparative_valuation(
                "t", Some("software"), None, "d", 5, &drivers, &peers, Decimal::ONE,
            );
            let mut s = seed;
            for i in (1..peers.len()).rev() {
                s = s.wrapping_mul(6364136223846793005).wrapping_add(1442695040888963407);
                let j = (s >> 33) as usize % (i + 1);
                peers.swap(i, j);
            }
            let permuted = compute_comparative_valuation(
                "t", Some("software"), None, "d", 5, &drivers, &peers, Decimal::ONE,
            );
            prop_assert_eq!(original, permuted);
        }
    }
}
