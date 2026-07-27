//! Sector-percentile read model — the **pure** core (ADR 0089 dec. 3, v0.61
//! §B1). No I/O: it takes the target company id, its sector, the ordered set of
//! metrics to rank, and one [`PeerSample`] per tracked company sharing the
//! sector (the target included), and produces the per-metric percentile rows the
//! `get_sector_percentiles` command serializes. Keeping the transform pure lets
//! the ADR 0049 golden/proptest suite pin the percentile math DB-free, and keeps
//! the peer-order-independence + determinism guarantees provable.
//!
//! What it does NOT do: derive market ratios or KPI values — those come from the
//! existing derived-metrics paths (`compute_price_context` for the level-0 market
//! ratios, the metrics resolver for the selected canonical KPIs), assembled in
//! the command layer. This module receives already-evaluated values per company.
//!
//! ## Peer set (ADR 0089 dec. 3)
//! Peers are the **tracked** companies sharing the target's `companies.sector`
//! (the target itself included) — derived at read time, no stored projection.
//! `peer_count` is that set's size and is **always** returned; the set is
//! **thin** (`thin = true`) when `peer_count < 4` (GPW-honest: most GPW sectors
//! never reach four tracked names). A company with no sector yields a typed
//! [`SectorPercentileEmptyReason::NoSector`] — never a silent absence.
//!
//! ## Percentile method (documented, deterministic, total)
//! For each metric, over the companies with a **defined** value for it (the
//! target plus its defined peers), the target's standing is the **rank-based
//! inclusive (mid-rank) percentile**:
//!
//! ```text
//! percentile = (2·L + E) / (2·N) · 100
//! ```
//!
//! where `N` = count of defined values (target + defined peers), `L` = count
//! strictly less than the target's value, `E` = count equal to it (≥ 1, the
//! target itself). This is order-independent, handles ties by their mid-rank,
//! and is always in `(0, 100]`. Rounded to 2 dp. The row also carries the peer
//! set's `median` (same defined set) and `sample_size` (= `N`).
//!
//! The function is **total**: a metric the target has no value for →
//! [`MetricAbsentReason::NoCompanyValue`]; fewer than **2** defined peer values
//! (a percentile over one other name is noise) →
//! [`MetricAbsentReason::InsufficientPeers`]. Neither ever emits `0`/`NaN`.

use std::collections::BTreeMap;

use rust_decimal::Decimal;
use serde::Serialize;

/// Thin peer-set threshold (ADR 0089 dec. 3): fewer than this many tracked
/// companies in the sector flags the whole read `thin`.
pub const THIN_PEER_THRESHOLD: u32 = 4;

/// Minimum **other** peers with a defined value before a percentile is meaningful
/// (a single other name is noise, not a distribution).
const MIN_DEFINED_PEERS: usize = 2;

// ============================================================================
// Output DTOs (ts-rs → ../../src/api/generated/; camelCase IPC contract)
// ============================================================================

/// Which metric family a percentile row belongs to (UI grouping; also records
/// where the value came from — market-data path vs derived-metrics resolver).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[cfg_attr(feature = "ts-export", derive(ts_rs::TS))]
#[cfg_attr(
    feature = "ts-export",
    ts(export, export_to = "../../src/api/generated/")
)]
#[serde(rename_all = "snake_case")]
pub enum SectorMetricKind {
    /// A level-0 market ratio (P/E, P/BV, EV/EBITDA, dividend yield, FCF yield),
    /// sourced from the `compute_price_context` path.
    MarketRatio,
    /// A selected canonical KPI, sourced from the derived-metrics resolver.
    CanonicalKpi,
}

/// Why a whole percentile read is empty — never a silent absence (ADR 0089).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[cfg_attr(feature = "ts-export", derive(ts_rs::TS))]
#[cfg_attr(
    feature = "ts-export",
    ts(export, export_to = "../../src/api/generated/")
)]
#[serde(rename_all = "snake_case")]
pub enum SectorPercentileEmptyReason {
    /// The company has no classified sector, so no peer set can be derived.
    NoSector,
}

/// Why a single metric has no percentile (typed absence, ADR 0089 dec. 3).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[cfg_attr(feature = "ts-export", derive(ts_rs::TS))]
#[cfg_attr(
    feature = "ts-export",
    ts(export, export_to = "../../src/api/generated/")
)]
#[serde(rename_all = "snake_case")]
pub enum MetricAbsentReason {
    /// The target company has no defined value for this metric.
    NoCompanyValue,
    /// Fewer than two peers have a defined value — too thin to rank against.
    InsufficientPeers,
}

/// One metric's percentile row.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[cfg_attr(feature = "ts-export", derive(ts_rs::TS))]
#[cfg_attr(
    feature = "ts-export",
    ts(export, export_to = "../../src/api/generated/")
)]
#[serde(rename_all = "camelCase")]
pub struct SectorPercentileMetric {
    /// Canonical metric key (e.g. `pe_ratio`, `roe`).
    pub metric_key: String,
    pub kind: SectorMetricKind,
    /// The target company's value, decimal-exact TEXT. `None` on a typed absence
    /// with reason `no_company_value`.
    pub value: Option<String>,
    /// Rank-based inclusive percentile in `0..=100`, 2 dp, decimal-exact TEXT.
    /// `None` on any typed absence.
    pub percentile: Option<String>,
    /// The defined set's median (target + defined peers), decimal-exact TEXT.
    /// `None` on any typed absence.
    pub median: Option<String>,
    /// Number of companies with a defined value for this metric (target
    /// included) — the `N` behind `percentile`/`median`.
    pub sample_size: u32,
    /// Set on a typed absence; `None` for a ranked row.
    pub absent_reason: Option<MetricAbsentReason>,
}

/// The full sector-percentile response.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[cfg_attr(feature = "ts-export", derive(ts_rs::TS))]
#[cfg_attr(
    feature = "ts-export",
    ts(export, export_to = "../../src/api/generated/")
)]
#[serde(rename_all = "camelCase")]
pub struct SectorPercentiles {
    pub company_id: String,
    /// The company's sector (echoed). `None` when unclassified (see `emptyReason`).
    pub sector: Option<String>,
    /// Tracked companies sharing the sector, the company itself included.
    /// **Always** returned (ADR 0089 dec. 3).
    pub peer_count: u32,
    /// `true` when `peer_count < 4` (GPW-honest thin flag).
    pub thin: bool,
    /// Set only when there is nothing to rank (no sector); `metrics` is then empty.
    pub empty_reason: Option<SectorPercentileEmptyReason>,
    /// One row per requested metric, in request order.
    pub metrics: Vec<SectorPercentileMetric>,
}

// ============================================================================
// Inputs (the command layer builds these from the DB; tests build them directly)
// ============================================================================

/// A metric to rank, carrying its family for the output grouping.
#[derive(Debug, Clone)]
pub struct MetricSpec {
    pub metric_key: String,
    pub kind: SectorMetricKind,
}

impl MetricSpec {
    pub fn new(metric_key: &str, kind: SectorMetricKind) -> Self {
        Self {
            metric_key: metric_key.to_owned(),
            kind,
        }
    }
}

/// One tracked company in the sector (the target included), with whatever
/// defined values it has for the requested metrics.
#[derive(Debug, Clone)]
pub struct PeerSample {
    pub company_id: String,
    /// `metric_key -> value`. A missing key means "no defined value".
    pub values: BTreeMap<String, Decimal>,
}

impl PeerSample {
    pub fn new(company_id: &str) -> Self {
        Self {
            company_id: company_id.to_owned(),
            values: BTreeMap::new(),
        }
    }

    pub fn with(mut self, metric_key: &str, value: Decimal) -> Self {
        self.values.insert(metric_key.to_owned(), value);
        self
    }
}

// ============================================================================
// Core transform
// ============================================================================

/// Render a decimal as plain decimal text (no exponent), trailing zeros trimmed.
fn plain(value: Decimal) -> String {
    value.normalize().to_string()
}

/// The defined set's median (values already collected; not required sorted).
fn median_of(values: &[Decimal]) -> Decimal {
    let mut sorted = values.to_vec();
    sorted.sort();
    let n = sorted.len();
    if n % 2 == 1 {
        sorted[n / 2]
    } else {
        (sorted[n / 2 - 1] + sorted[n / 2]) / Decimal::from(2)
    }
}

/// Rank-based inclusive (mid-rank) percentile of `target` among `all` (which
/// includes `target`), rounded to 2 dp. `all` is non-empty and contains at least
/// one value equal to `target`.
fn mid_rank_percentile(target: Decimal, all: &[Decimal]) -> Decimal {
    let n = all.len();
    let l = all.iter().filter(|v| **v < target).count();
    let e = all.iter().filter(|v| **v == target).count();
    // (2L + E) / (2N) * 100 — integer numerator/denominator, one exact division.
    let numerator = Decimal::from((2 * l + e) as i64) * Decimal::from(100);
    let denominator = Decimal::from((2 * n) as i64);
    (numerator / denominator).round_dp(2)
}

/// Compute the sector-percentile read model. Pure and deterministic: every row
/// depends only on the *set* of samples, never their order; metric rows follow
/// `metric_specs` order.
pub fn compute_sector_percentiles(
    company_id: &str,
    sector: Option<&str>,
    metric_specs: &[MetricSpec],
    samples: &[PeerSample],
) -> SectorPercentiles {
    let sector = sector
        .map(str::trim)
        .filter(|s| !s.is_empty())
        .map(str::to_owned);

    let Some(sector) = sector else {
        return SectorPercentiles {
            company_id: company_id.to_owned(),
            sector: None,
            peer_count: 0,
            thin: true,
            empty_reason: Some(SectorPercentileEmptyReason::NoSector),
            metrics: Vec::new(),
        };
    };

    // Dedup by company id, order-independently. Tracked company ids are unique
    // in production, so this only fires on synthetic duplicates; keeping the
    // deterministic winner (greatest `values` map) guarantees the whole read is
    // permutation-safe regardless.
    let mut by_company: BTreeMap<&str, &PeerSample> = BTreeMap::new();
    for sample in samples {
        by_company
            .entry(sample.company_id.as_str())
            .and_modify(|kept| {
                if sample.values > kept.values {
                    *kept = sample;
                }
            })
            .or_insert(sample);
    }
    let deduped: Vec<&PeerSample> = by_company.values().copied().collect();

    let peer_count = deduped.len() as u32;
    let thin = peer_count < THIN_PEER_THRESHOLD;

    let target = deduped.iter().find(|s| s.company_id == company_id).copied();

    let metrics = metric_specs
        .iter()
        .map(|spec| {
            let target_value = target.and_then(|t| t.values.get(&spec.metric_key)).copied();
            let peer_values: Vec<Decimal> = deduped
                .iter()
                .filter(|s| s.company_id != company_id)
                .filter_map(|s| s.values.get(&spec.metric_key).copied())
                .collect();

            match target_value {
                None => SectorPercentileMetric {
                    metric_key: spec.metric_key.clone(),
                    kind: spec.kind,
                    value: None,
                    percentile: None,
                    median: None,
                    // No company value ⇒ the company is not part of the defined
                    // set, so the sample is the defined peers alone.
                    sample_size: peer_values.len() as u32,
                    absent_reason: Some(MetricAbsentReason::NoCompanyValue),
                },
                Some(value) if peer_values.len() < MIN_DEFINED_PEERS => SectorPercentileMetric {
                    metric_key: spec.metric_key.clone(),
                    kind: spec.kind,
                    value: Some(plain(value)),
                    percentile: None,
                    median: None,
                    sample_size: (peer_values.len() + 1) as u32,
                    absent_reason: Some(MetricAbsentReason::InsufficientPeers),
                },
                Some(value) => {
                    let mut all = peer_values;
                    all.push(value);
                    let percentile = mid_rank_percentile(value, &all);
                    let median = median_of(&all);
                    SectorPercentileMetric {
                        metric_key: spec.metric_key.clone(),
                        kind: spec.kind,
                        value: Some(plain(value)),
                        percentile: Some(plain(percentile)),
                        median: Some(plain(median)),
                        sample_size: all.len() as u32,
                        absent_reason: None,
                    }
                }
            }
        })
        .collect();

    SectorPercentiles {
        company_id: company_id.to_owned(),
        sector: Some(sector),
        peer_count,
        thin,
        empty_reason: None,
        metrics,
    }
}

/// Locate a metric row by key (test convenience).
pub fn metric_of<'a>(
    result: &'a SectorPercentiles,
    metric_key: &str,
) -> Option<&'a SectorPercentileMetric> {
    result.metrics.iter().find(|m| m.metric_key == metric_key)
}

#[cfg(test)]
mod tests {
    use super::*;
    use rust_decimal::prelude::FromStr;

    fn d(s: &str) -> Decimal {
        Decimal::from_str(s).expect("decimal")
    }

    fn specs() -> Vec<MetricSpec> {
        vec![
            MetricSpec::new("pe_ratio", SectorMetricKind::MarketRatio),
            MetricSpec::new("roe", SectorMetricKind::CanonicalKpi),
        ]
    }

    /// A 5-company software sector with hand-computed percentiles.
    fn five_peer_samples() -> Vec<PeerSample> {
        vec![
            PeerSample::new("t")
                .with("pe_ratio", d("20"))
                .with("roe", d("2.0")),
            PeerSample::new("a")
                .with("pe_ratio", d("10"))
                .with("roe", d("2.0")),
            PeerSample::new("b")
                .with("pe_ratio", d("15"))
                .with("roe", d("1.0")),
            PeerSample::new("c")
                .with("pe_ratio", d("25"))
                .with("roe", d("3.0")),
            PeerSample::new("d")
                .with("pe_ratio", d("30"))
                .with("roe", d("4.0")),
        ]
    }

    #[test]
    fn no_sector_is_typed_empty_reason_never_silent() {
        let result = compute_sector_percentiles("t", None, &specs(), &[]);
        assert_eq!(
            result.empty_reason,
            Some(SectorPercentileEmptyReason::NoSector)
        );
        assert_eq!(result.peer_count, 0);
        assert!(result.thin);
        assert!(result.metrics.is_empty());
        assert_eq!(result.sector, None);
    }

    #[test]
    fn blank_sector_is_treated_as_no_sector() {
        let result = compute_sector_percentiles("t", Some("   "), &specs(), &[]);
        assert_eq!(
            result.empty_reason,
            Some(SectorPercentileEmptyReason::NoSector)
        );
    }

    #[test]
    fn thin_flag_flips_at_four_tracked_companies() {
        // N = 3 → thin.
        let three: Vec<PeerSample> = five_peer_samples().into_iter().take(3).collect();
        let r3 = compute_sector_percentiles("t", Some("software"), &specs(), &three);
        assert_eq!(r3.peer_count, 3);
        assert!(r3.thin, "3 companies is thin");
        // N = 4 → not thin.
        let four: Vec<PeerSample> = five_peer_samples().into_iter().take(4).collect();
        let r4 = compute_sector_percentiles("t", Some("software"), &specs(), &four);
        assert_eq!(r4.peer_count, 4);
        assert!(!r4.thin, "4 companies is not thin");
    }

    #[test]
    fn mid_rank_percentile_matches_hand_computation() {
        let result =
            compute_sector_percentiles("t", Some("software"), &specs(), &five_peer_samples());
        assert_eq!(result.peer_count, 5);
        assert!(!result.thin);

        // pe_ratio: sorted 10,15,20,25,30; target 20 → L=2, E=1, N=5 →
        // (2*2+1)/(2*5)*100 = 50.00; median = 20.
        let pe = metric_of(&result, "pe_ratio").expect("pe row");
        assert_eq!(pe.value.as_deref(), Some("20"));
        assert_eq!(pe.percentile.as_deref(), Some("50"));
        assert_eq!(pe.median.as_deref(), Some("20"));
        assert_eq!(pe.sample_size, 5);
        assert_eq!(pe.absent_reason, None);
        assert_eq!(pe.kind, SectorMetricKind::MarketRatio);

        // roe: sorted 1,2,2,3,4; target 2.0 → L=1, E=2, N=5 →
        // (2*1+2)/(2*5)*100 = 40.00; median = 2.
        let roe = metric_of(&result, "roe").expect("roe row");
        assert_eq!(roe.percentile.as_deref(), Some("40"));
        assert_eq!(roe.median.as_deref(), Some("2"));
        assert_eq!(roe.kind, SectorMetricKind::CanonicalKpi);
    }

    #[test]
    fn missing_company_value_is_typed_no_company_value() {
        // Target has roe but no pe_ratio; peers have pe_ratio.
        let mut samples = five_peer_samples();
        samples[0].values.remove("pe_ratio");
        let result = compute_sector_percentiles("t", Some("software"), &specs(), &samples);
        let pe = metric_of(&result, "pe_ratio").expect("pe row");
        assert_eq!(pe.value, None);
        assert_eq!(pe.percentile, None);
        assert_eq!(pe.absent_reason, Some(MetricAbsentReason::NoCompanyValue));
        // roe is unaffected.
        let roe = metric_of(&result, "roe").expect("roe row");
        assert_eq!(roe.absent_reason, None);
    }

    #[test]
    fn fewer_than_two_defined_peers_is_insufficient_peers() {
        // Target + one peer have pe_ratio; the rest do not.
        let samples = vec![
            PeerSample::new("t").with("pe_ratio", d("20")),
            PeerSample::new("a").with("pe_ratio", d("10")),
            PeerSample::new("b"),
            PeerSample::new("c"),
        ];
        let result = compute_sector_percentiles("t", Some("software"), &specs(), &samples);
        let pe = metric_of(&result, "pe_ratio").expect("pe row");
        assert_eq!(pe.value.as_deref(), Some("20")); // company value still surfaced
        assert_eq!(pe.percentile, None);
        assert_eq!(pe.sample_size, 2); // target + the one defined peer
        assert_eq!(
            pe.absent_reason,
            Some(MetricAbsentReason::InsufficientPeers)
        );
    }

    #[test]
    fn company_at_the_top_and_bottom_stays_within_bounds() {
        let samples = vec![
            PeerSample::new("t").with("pe_ratio", d("100")),
            PeerSample::new("a").with("pe_ratio", d("10")),
            PeerSample::new("b").with("pe_ratio", d("20")),
            PeerSample::new("c").with("pe_ratio", d("30")),
        ];
        let top = compute_sector_percentiles("t", Some("software"), &specs(), &samples);
        // sorted 10,20,30,100; target 100 → L=3,E=1,N=4 → (7)/(8)*100 = 87.50.
        assert_eq!(
            metric_of(&top, "pe_ratio").unwrap().percentile.as_deref(),
            Some("87.5")
        );
        // Bottom: target 5 below everyone → L=0,E=1,N=4 → 12.5.
        let mut low = samples;
        low[0] = PeerSample::new("t").with("pe_ratio", d("5"));
        let bottom = compute_sector_percentiles("t", Some("software"), &specs(), &low);
        assert_eq!(
            metric_of(&bottom, "pe_ratio")
                .unwrap()
                .percentile
                .as_deref(),
            Some("12.5")
        );
    }

    #[test]
    fn golden_five_peer_sector_snapshot_is_stable() {
        let result =
            compute_sector_percentiles("t", Some("software"), &specs(), &five_peer_samples());
        insta::assert_debug_snapshot!("sector_percentiles_five_peers", result);
    }
}

#[cfg(test)]
mod proptests {
    //! Data-transform invariants (ADR 0049): every row depends only on the *set*
    //! of samples (peer-order & insertion-order independence), the transform is
    //! idempotent, percentiles stay in `[0, 100]`, and it never panics on empty /
    //! degenerate / missing-sector / duplicate-value inputs.
    use super::*;
    use proptest::prelude::*;
    use rust_decimal::prelude::FromPrimitive;

    fn company_strategy() -> impl Strategy<Value = String> {
        prop::sample::select(vec!["t", "a", "b", "c", "d", "e"]).prop_map(str::to_owned)
    }

    fn value_strategy() -> impl Strategy<Value = Decimal> {
        (-50i64..50).prop_map(|n| Decimal::from_i64(n).unwrap())
    }

    fn sample_strategy() -> impl Strategy<Value = PeerSample> {
        (
            company_strategy(),
            proptest::option::of(value_strategy()),
            proptest::option::of(value_strategy()),
        )
            .prop_map(|(company_id, pe, roe)| {
                let mut values = BTreeMap::new();
                if let Some(v) = pe {
                    values.insert("pe_ratio".to_owned(), v);
                }
                if let Some(v) = roe {
                    values.insert("roe".to_owned(), v);
                }
                PeerSample { company_id, values }
            })
    }

    fn samples_strategy() -> impl Strategy<Value = Vec<PeerSample>> {
        proptest::collection::vec(sample_strategy(), 0..12)
    }

    fn specs() -> Vec<MetricSpec> {
        vec![
            MetricSpec::new("pe_ratio", SectorMetricKind::MarketRatio),
            MetricSpec::new("roe", SectorMetricKind::CanonicalKpi),
        ]
    }

    fn sector_of(has: bool) -> Option<&'static str> {
        if has {
            Some("software")
        } else {
            None
        }
    }

    proptest! {
        #[test]
        fn never_panics_and_percentiles_are_bounded(
            samples in samples_strategy(),
            has_sector in any::<bool>(),
        ) {
            let result = compute_sector_percentiles("t", sector_of(has_sector), &specs(), &samples);
            for metric in &result.metrics {
                if let Some(pct) = &metric.percentile {
                    let p = pct.parse::<Decimal>().expect("percentile parses");
                    prop_assert!(p >= Decimal::ZERO, "percentile {p} >= 0");
                    prop_assert!(p <= Decimal::from(100), "percentile {p} <= 100");
                }
            }
        }

        #[test]
        fn idempotent(samples in samples_strategy()) {
            let a = compute_sector_percentiles("t", Some("software"), &specs(), &samples);
            let b = compute_sector_percentiles("t", Some("software"), &specs(), &samples);
            prop_assert_eq!(a, b);
        }

        #[test]
        fn peer_order_does_not_change_the_result(
            mut samples in samples_strategy(),
            seed in any::<u64>(),
        ) {
            let original = compute_sector_percentiles("t", Some("software"), &specs(), &samples);
            // Deterministic shuffle.
            let mut s = seed;
            for i in (1..samples.len()).rev() {
                s = s.wrapping_mul(6364136223846793005).wrapping_add(1442695040888963407);
                let j = (s >> 33) as usize % (i + 1);
                samples.swap(i, j);
            }
            let permuted = compute_sector_percentiles("t", Some("software"), &specs(), &samples);
            prop_assert_eq!(original, permuted);
        }
    }
}
