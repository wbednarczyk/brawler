//! Cross-company KPI comparison — the **pure** read-model core (ADR 0089
//! decision 1, v0.61 §A2). No I/O: it takes already-selected canonical facts,
//! per-metric value-kind metadata, and an [`FxLookup`] (backed by
//! `storage::fx_rates` in production), and produces the aligned period axis +
//! per-company series the `get_kpi_comparison` command serializes. Keeping the
//! transform pure is what lets the ADR 0049 golden/proptest suite pin its
//! correctness DB-free, and lets the N=1 Fundamentals surface and the N-company
//! MCP read share the exact same code.
//!
//! What it does NOT do: pick the canonical fact per `(company, period, metric)`
//! slot — that selection (consolidated / total / reported / final / confirmed,
//! matching `storage::quality_frameworks::load_period_facts`) lives in the
//! command layer's DB query, and is validated end-to-end by the real-data-gated
//! test. This module receives one fact per coordinate and aligns/labels/derives.
//!
//! Deltas (server-side, ADR 0089 dec. 1):
//! - **monetary** value kinds → percent change vs the same period-type prior
//!   year (YoY) / prior sequential quarter (QoQ), rounded to 2 dp;
//! - **ratio / percentage** value kinds → a **p.p.** delta (plain difference);
//! - a percent change is **`delta_undefined`** when the prior value is `<= 0`
//!   (zero or negative base) or the sign flips positive→negative — emitting a
//!   misleading percentage across a sign change or from a non-positive base is
//!   worse than an honest typed absence (the flag names which delta);
//! - no prior period (first period, or a `no_fact` gap in the prior slot) →
//!   the delta is simply `null`, with **no** `delta_undefined` flag.
//!
//! FX (ADR 0089 dec. 2, via [`crate::fx`]): a PLN fact passes through
//! (`native_pln`); a non-PLN ISO fact converts at the flow/stock basis its
//! `measure_window` selects; a missing rate is a typed `fx_missing` flag (the
//! native value is still returned); a NULL or non-ISO-4217-looking currency
//! (e.g. the known `currency='shares'` EPS data bug, card 47a21f1) is a typed
//! `currency_unknown` flag **for monetary/per-share kinds** — never a silent
//! PLN guess. For **ratio/percentage** kinds a NULL currency is CORRECT (a ratio
//! carries none): no flag, the native value stands (ADR 0089 dec. 8, card b875e69).

use std::collections::{BTreeMap, BTreeSet, HashMap};

use rust_decimal::Decimal;
use serde::Serialize;

use crate::fx::{FxBasis, MeasureWindow};

// ============================================================================
// Inputs (the command layer builds these from the DB; tests build them directly)
// ============================================================================

/// Reporting granularity — which `period_type`s populate the axis.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Granularity {
    /// Fiscal-year (`FY`) periods only.
    Annual,
    /// Quarterly (`Q1`–`Q4`) periods only.
    Quarterly,
}

impl Granularity {
    /// Parse the wire value; anything unrecognized falls back to `Annual`
    /// (the corpus is annual-dominant, ADR 0089 §A0 finding) rather than erroring.
    pub fn parse(raw: &str) -> Self {
        match raw.trim().to_ascii_lowercase().as_str() {
            "quarterly" => Granularity::Quarterly,
            _ => Granularity::Annual,
        }
    }

    fn as_str(self) -> &'static str {
        match self {
            Granularity::Annual => "annual",
            Granularity::Quarterly => "quarterly",
        }
    }

    /// Does this `period_type` belong on the axis for this granularity?
    fn admits(self, period_type: &str) -> bool {
        match self {
            Granularity::Annual => period_type == "FY",
            Granularity::Quarterly => {
                matches!(period_type, "Q1" | "Q2" | "Q3" | "Q4")
            }
        }
    }
}

/// One already-selected canonical fact for a `(company, metric, period)` slot.
/// The command layer resolves exactly one of these per coordinate; this module
/// never sees the competing variants.
#[derive(Debug, Clone)]
pub struct ComparisonFact {
    pub company_id: String,
    pub metric_key: String,
    pub fiscal_year: i64,
    pub period_type: String,
    pub period_end_date: Option<String>,
    pub fact_id: String,
    pub value: Decimal,
    pub currency: Option<String>,
    /// The fact's `measure_window` (`flow` → flow FX basis; anything else →
    /// stock basis). Not the KPI's — a fact carries its own window.
    pub measure_window: String,
    /// Provenance `validation_status` (the evidence link), if a provenance row
    /// exists. `None` for facts predating the provenance table.
    pub validation_status: Option<String>,
}

/// Per-metric metadata: the catalog `value_kind` drives % vs p.p. deltas.
#[derive(Debug, Clone)]
pub struct MetricMeta {
    pub value_kind: Option<String>,
}

/// Resolve the PLN mid for a currency/window/period, or `None` when no rate
/// exists in the window the basis needs. Production wraps `storage::fx_rates`;
/// tests inject an in-memory map. PLN never reaches here (handled upstream).
pub trait FxLookup {
    fn resolve(
        &self,
        currency: &str,
        window: MeasureWindow,
        start: &str,
        end: &str,
    ) -> Option<(Decimal, FxBasis)>;
}

/// An [`FxLookup`] that resolves nothing — every non-PLN fact becomes
/// `fx_missing`. The default for surfaces with no FX substrate (and the golden
/// pairs, which are all PLN).
pub struct NoFx;

impl FxLookup for NoFx {
    fn resolve(&self, _: &str, _: MeasureWindow, _: &str, _: &str) -> Option<(Decimal, FxBasis)> {
        None
    }
}

// ============================================================================
// Output DTOs (ts-rs → ../../src/api/generated/; camelCase IPC contract)
// ============================================================================

/// A typed per-cell flag — every gap or non-conversion is one of these, never a
/// silent `0`/`—`/PLN-guess (ADR 0089 dec. 1 "no silently absent value").
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[cfg_attr(feature = "ts-export", derive(ts_rs::TS))]
#[cfg_attr(
    feature = "ts-export",
    ts(export, export_to = "../../src/api/generated/")
)]
#[serde(rename_all = "snake_case")]
pub enum CellFlag {
    /// No canonical fact exists for this company/metric at this period.
    NoFact,
    /// A non-PLN ISO fact whose period had no NBP rate to convert at.
    FxMissing,
    /// A NULL or non-ISO-4217-looking currency (e.g. `shares`) — unconvertible.
    CurrencyUnknown,
    /// The YoY percent change is mathematically undefined (non-positive base or
    /// a positive→negative sign flip).
    DeltaYoyUndefined,
    /// The QoQ percent change is mathematically undefined (see above).
    DeltaQoqUndefined,
}

/// One aligned axis coordinate shared across every series.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[cfg_attr(feature = "ts-export", derive(ts_rs::TS))]
#[cfg_attr(
    feature = "ts-export",
    ts(export, export_to = "../../src/api/generated/")
)]
#[serde(rename_all = "camelCase")]
pub struct KpiComparisonPeriod {
    pub fiscal_year: i64,
    pub period_type: String,
    /// Stable `"<year>:<periodType>"` key (e.g. `"2024:FY"`).
    pub key: String,
}

/// One `(company, metric)` series, its cells aligned 1:1 with the shared axis.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[cfg_attr(feature = "ts-export", derive(ts_rs::TS))]
#[cfg_attr(
    feature = "ts-export",
    ts(export, export_to = "../../src/api/generated/")
)]
#[serde(rename_all = "camelCase")]
pub struct KpiComparisonSeries {
    pub company_id: String,
    pub metric_key: String,
    /// The KPI's catalog `value_kind`, or `None` when the metric has no
    /// definition (drives % vs p.p. rendering; also decides delta semantics).
    pub value_kind: Option<String>,
    /// One cell per axis coordinate, in the same order as `axis`.
    pub cells: Vec<KpiComparisonCell>,
}

/// One cell: a fact (with FX + provenance + deltas) or a typed absence.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[cfg_attr(feature = "ts-export", derive(ts_rs::TS))]
#[cfg_attr(
    feature = "ts-export",
    ts(export, export_to = "../../src/api/generated/")
)]
#[serde(rename_all = "camelCase")]
pub struct KpiComparisonCell {
    pub fiscal_year: i64,
    pub period_type: String,
    /// The canonical fact's id (evidence link). `None` on a `no_fact` gap.
    pub fact_id: Option<String>,
    /// Native decimal-exact value (base units), signed. `None` on a gap.
    pub value: Option<String>,
    pub currency: Option<String>,
    /// PLN-converted decimal-exact value. `None` when the cell is a gap, the
    /// currency is unknown, or the rate is missing (see `flags`).
    pub value_pln: Option<String>,
    /// The conversion basis label (`period_average` / `latest_on_or_before` /
    /// `native_pln`). `None` when no conversion was produced.
    pub fx_basis: Option<String>,
    pub validation_status: Option<String>,
    /// QoQ delta (% for monetary, p.p. for ratio/percentage). `null` for annual
    /// granularity, the first period, or a gap in the prior slot.
    pub delta_qo_q: Option<String>,
    /// YoY delta (% for monetary, p.p. for ratio/percentage). `null` for the
    /// first period or a gap in the prior slot.
    pub delta_yo_y: Option<String>,
    /// Typed flags — empty for a clean, converted, delta-defined cell.
    pub flags: Vec<CellFlag>,
}

/// The full comparison response.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[cfg_attr(feature = "ts-export", derive(ts_rs::TS))]
#[cfg_attr(
    feature = "ts-export",
    ts(export, export_to = "../../src/api/generated/")
)]
#[serde(rename_all = "camelCase")]
pub struct KpiComparison {
    /// Echo of the requested granularity (`annual` / `quarterly`).
    pub granularity: String,
    /// Echo of the requested metric keys (deduped, in request order).
    pub metric_keys: Vec<String>,
    /// The aligned axis, ascending by `(fiscal_year, period ordinal)`.
    pub axis: Vec<KpiComparisonPeriod>,
    /// One series per `(company, metric)`, in request order (companies outer,
    /// metrics inner), deduped.
    pub series: Vec<KpiComparisonSeries>,
}

// ============================================================================
// Core transform
// ============================================================================

/// Ordinal of a `period_type` within a fiscal year, for axis sorting and the
/// QoQ sequence (`FY`/`H1`/… collapse harmlessly for non-quarterly use).
fn period_ordinal(period_type: &str) -> i64 {
    match period_type {
        "Q1" => 1,
        "Q2" => 2,
        "Q3" => 3,
        "Q4" => 4,
        _ => 0,
    }
}

/// The prior YoY coordinate: same period_type, previous fiscal year.
fn yoy_prior(fiscal_year: i64, period_type: &str) -> (i64, String) {
    (fiscal_year - 1, period_type.to_owned())
}

/// The prior QoQ coordinate: the immediately preceding quarter in sequence
/// (Q1 → prior-year Q4). `None` for a non-quarterly period_type.
fn qoq_prior(fiscal_year: i64, period_type: &str) -> Option<(i64, String)> {
    match period_type {
        "Q1" => Some((fiscal_year - 1, "Q4".to_owned())),
        "Q2" => Some((fiscal_year, "Q1".to_owned())),
        "Q3" => Some((fiscal_year, "Q2".to_owned())),
        "Q4" => Some((fiscal_year, "Q3".to_owned())),
        _ => None,
    }
}

/// A 3-uppercase-ASCII-letter token *looks* like an ISO-4217 code. Deliberately
/// not a closed enum (ADR 0089: needed currencies are table-driven), and
/// deliberately not special-casing `shares` — anything that is not 3 uppercase
/// letters (NULL, `shares`, `pln ` with junk) is `currency_unknown`.
fn is_iso_currency(currency: &str) -> bool {
    let c = currency.trim();
    c.len() == 3 && c.chars().all(|ch| ch.is_ascii_uppercase())
}

/// Serde string for an [`FxBasis`] (kept here so `crate::fx` needs no ts-rs).
fn fx_basis_str(basis: FxBasis) -> &'static str {
    match basis {
        FxBasis::PeriodAverage => "period_average",
        FxBasis::LatestOnOrBefore => "latest_on_or_before",
        FxBasis::NativePln => "native_pln",
    }
}

/// Flow FX basis for `measure_window = flow`, stock basis otherwise.
fn window_of(measure_window: &str) -> MeasureWindow {
    if measure_window.eq_ignore_ascii_case("flow") {
        MeasureWindow::Flow
    } else {
        MeasureWindow::Stock
    }
}

/// `[start, end]` dates spanning the period, for the flow period-average window
/// and the stock cutoff. `end` prefers the stored `period_end_date`; both fall
/// back to a deterministic derivation from `(fiscal_year, period_type)`.
fn period_bounds(
    fiscal_year: i64,
    period_type: &str,
    period_end_date: Option<&str>,
) -> (String, String) {
    let start = match period_type {
        "Q2" => format!("{fiscal_year:04}-04-01"),
        "Q3" => format!("{fiscal_year:04}-07-01"),
        "Q4" => format!("{fiscal_year:04}-10-01"),
        "H2" => format!("{fiscal_year:04}-07-01"),
        // FY, H1, 9M, Q1, months, unknown → start of the fiscal year.
        _ => format!("{fiscal_year:04}-01-01"),
    };
    let end = period_end_date
        .map(str::to_owned)
        .filter(|d| !d.trim().is_empty())
        .unwrap_or_else(|| match period_type {
            "Q1" => format!("{fiscal_year:04}-03-31"),
            "Q2" | "H1" => format!("{fiscal_year:04}-06-30"),
            "Q3" | "9M" => format!("{fiscal_year:04}-09-30"),
            // Q4, FY, H2, months, unknown → year end.
            _ => format!("{fiscal_year:04}-12-31"),
        });
    (start, end)
}

/// Round a decimal to 2 dp and render as plain decimal text (no exponent).
fn round2(value: Decimal) -> String {
    value.round_dp(2).normalize().to_string()
}

/// Whether a value kind uses p.p. (plain difference) deltas rather than %.
fn is_points_kind(value_kind: Option<&str>) -> bool {
    matches!(value_kind, Some("ratio") | Some("percentage"))
}

/// Compute one delta between `current` and an optional `prior`, per value kind.
/// Returns `(delta_text, undefined)`:
/// - `prior = None` → `(None, false)` (no prior; not undefined).
/// - p.p. kinds → `(Some(curr - prev), false)` (never undefined).
/// - monetary/% → undefined (`(None, true)`) when `prev <= 0` or a
///   positive→negative sign flip; else `(Some(pct), false)`.
fn compute_delta(
    current: Decimal,
    prior: Option<Decimal>,
    value_kind: Option<&str>,
) -> (Option<String>, bool) {
    let Some(prev) = prior else {
        return (None, false);
    };
    if is_points_kind(value_kind) {
        return (Some(round2(current - prev)), false);
    }
    // Percent change. A non-positive base, or a flip from positive to negative,
    // makes the percentage misleading — emit a typed undefined instead.
    if prev <= Decimal::ZERO || current < Decimal::ZERO {
        return (None, true);
    }
    let pct = (current - prev) / prev * Decimal::from(100);
    (Some(round2(pct)), false)
}

/// Coordinate key for the per-series fact index.
type Coord = (i64, String);

/// A total, order-independent tie-break key for two facts colliding on one
/// coordinate. Fact ids are unique in production, so this only ever fires on
/// synthetic duplicates; it guarantees the pure transform is permutation-safe.
fn fact_pick_key(fact: &ComparisonFact) -> (&str, Decimal, Option<&str>, &str) {
    (
        fact.fact_id.as_str(),
        fact.value,
        fact.currency.as_deref(),
        fact.measure_window.as_str(),
    )
}

/// Compute the full comparison. Pure and deterministic: the axis and every
/// cell depend only on the *set* of facts, never their order; series follow the
/// requested `(company, metric)` order (deduped).
pub fn compute_comparison(
    granularity: Granularity,
    company_ids: &[String],
    metric_keys: &[String],
    facts: &[ComparisonFact],
    metric_meta: &HashMap<String, MetricMeta>,
    fx: &dyn FxLookup,
) -> KpiComparison {
    // Deduped request order.
    let companies = dedup_preserving(company_ids);
    let metrics = dedup_preserving(metric_keys);
    let requested_companies: BTreeSet<&str> = companies.iter().map(String::as_str).collect();
    let requested_metrics: BTreeSet<&str> = metrics.iter().map(String::as_str).collect();

    // Index the relevant facts by (company, metric) → coordinate → fact, and
    // collect the axis coordinate set. Only facts matching the request +
    // granularity participate, so the axis never leaks unrelated periods.
    let mut by_series: HashMap<(&str, &str), HashMap<Coord, &ComparisonFact>> = HashMap::new();
    let mut axis_coords: BTreeSet<(i64, i64, String)> = BTreeSet::new();
    for fact in facts {
        if !granularity.admits(&fact.period_type) {
            continue;
        }
        if !requested_companies.contains(fact.company_id.as_str())
            || !requested_metrics.contains(fact.metric_key.as_str())
        {
            continue;
        }
        let coord = (fact.fiscal_year, fact.period_type.clone());
        let slot = by_series
            .entry((fact.company_id.as_str(), fact.metric_key.as_str()))
            .or_default()
            .entry(coord);
        // Production passes exactly one canonical fact per coordinate (slot
        // selection is upstream). Should a duplicate arrive, pick a canonical
        // winner *independent of order* so the transform stays permutation-safe.
        match slot {
            std::collections::hash_map::Entry::Vacant(v) => {
                v.insert(fact);
            }
            std::collections::hash_map::Entry::Occupied(mut o) => {
                if fact_pick_key(fact) > fact_pick_key(o.get()) {
                    o.insert(fact);
                }
            }
        }
        axis_coords.insert((
            fact.fiscal_year,
            period_ordinal(&fact.period_type),
            fact.period_type.clone(),
        ));
    }

    let axis: Vec<KpiComparisonPeriod> = axis_coords
        .into_iter()
        .map(|(fiscal_year, _ord, period_type)| KpiComparisonPeriod {
            key: format!("{fiscal_year}:{period_type}"),
            fiscal_year,
            period_type,
        })
        .collect();

    let mut series = Vec::with_capacity(companies.len() * metrics.len());
    for company_id in &companies {
        for metric_key in &metrics {
            let value_kind = metric_meta
                .get(metric_key)
                .and_then(|m| m.value_kind.clone());
            let index = by_series
                .get(&(company_id.as_str(), metric_key.as_str()))
                .cloned()
                .unwrap_or_default();

            let cells = axis
                .iter()
                .map(|period| build_cell(period, granularity, value_kind.as_deref(), &index, fx))
                .collect();

            series.push(KpiComparisonSeries {
                company_id: company_id.clone(),
                metric_key: metric_key.clone(),
                value_kind,
                cells,
            });
        }
    }

    KpiComparison {
        granularity: granularity.as_str().to_owned(),
        metric_keys: metrics,
        axis,
        series,
    }
}

fn build_cell(
    period: &KpiComparisonPeriod,
    granularity: Granularity,
    value_kind: Option<&str>,
    index: &HashMap<Coord, &ComparisonFact>,
    fx: &dyn FxLookup,
) -> KpiComparisonCell {
    let coord = (period.fiscal_year, period.period_type.clone());
    let Some(fact) = index.get(&coord) else {
        return KpiComparisonCell {
            fiscal_year: period.fiscal_year,
            period_type: period.period_type.clone(),
            fact_id: None,
            value: None,
            currency: None,
            value_pln: None,
            fx_basis: None,
            validation_status: None,
            delta_qo_q: None,
            delta_yo_y: None,
            flags: vec![CellFlag::NoFact],
        };
    };

    let mut flags = Vec::new();

    // ---- FX conversion ----------------------------------------------------
    let (value_pln, fx_basis) = convert(fact, value_kind, fx, &mut flags);

    // ---- Deltas -----------------------------------------------------------
    let prior_value = |prior: Option<(i64, String)>| -> Option<Decimal> {
        prior.and_then(|c| index.get(&c)).map(|f| f.value)
    };
    let (delta_yo_y, yoy_undef) = compute_delta(
        fact.value,
        prior_value(Some(yoy_prior(period.fiscal_year, &period.period_type))),
        value_kind,
    );
    if yoy_undef {
        flags.push(CellFlag::DeltaYoyUndefined);
    }
    // QoQ only exists for quarterly granularity (ADR 0089 §A0: annual has no QoQ).
    let (delta_qo_q, qoq_undef) = match granularity {
        Granularity::Quarterly => {
            let prior = qoq_prior(period.fiscal_year, &period.period_type);
            compute_delta(fact.value, prior_value(prior), value_kind)
        }
        Granularity::Annual => (None, false),
    };
    if qoq_undef {
        flags.push(CellFlag::DeltaQoqUndefined);
    }

    KpiComparisonCell {
        fiscal_year: period.fiscal_year,
        period_type: period.period_type.clone(),
        fact_id: Some(fact.fact_id.clone()),
        value: Some(fact.value.normalize().to_string()),
        currency: fact.currency.clone(),
        value_pln,
        fx_basis,
        validation_status: fact.validation_status.clone(),
        delta_qo_q,
        delta_yo_y,
        flags,
    }
}

/// Resolve `(value_pln, fx_basis)` and push any FX flag. PLN passes through;
/// a missing rate is `fx_missing`. A non-ISO/NULL currency is `currency_unknown`
/// **only for monetary/per-share** kinds — for ratio/percentage kinds a NULL
/// currency is CORRECT (ratios carry none), so no flag is emitted and the native
/// value stands as the comparable number (ADR 0089 dec. 8; card b875e69 A7).
fn convert(
    fact: &ComparisonFact,
    value_kind: Option<&str>,
    fx: &dyn FxLookup,
    flags: &mut Vec<CellFlag>,
) -> (Option<String>, Option<String>) {
    let currency = fact.currency.as_deref().map(str::trim).unwrap_or("");
    if !is_iso_currency(currency) {
        // Ratio/percentage: a currency-less value is expected, not a bug.
        if !is_points_kind(value_kind) {
            flags.push(CellFlag::CurrencyUnknown);
        }
        return (None, None);
    }
    if currency.eq_ignore_ascii_case(crate::fx::PLN) {
        return (
            Some(fact.value.normalize().to_string()),
            Some(fx_basis_str(FxBasis::NativePln).to_owned()),
        );
    }
    let (start, end) = period_bounds(
        fact.fiscal_year,
        &fact.period_type,
        fact.period_end_date.as_deref(),
    );
    match fx.resolve(currency, window_of(&fact.measure_window), &start, &end) {
        Some((mid, basis)) => (
            Some((fact.value * mid).normalize().to_string()),
            Some(fx_basis_str(basis).to_owned()),
        ),
        None => {
            flags.push(CellFlag::FxMissing);
            (None, None)
        }
    }
}

/// Dedup while preserving first-seen order (trims blanks).
fn dedup_preserving(values: &[String]) -> Vec<String> {
    let mut seen = BTreeSet::new();
    let mut out = Vec::new();
    for value in values {
        let trimmed = value.trim();
        if trimmed.is_empty() {
            continue;
        }
        if seen.insert(trimmed.to_owned()) {
            out.push(trimmed.to_owned());
        }
    }
    out
}

/// Look up a series' cell for one axis key — the shared convenience both the
/// golden and the real-data-gated test read expectations through.
pub fn cell_at<'a>(series: &'a KpiComparisonSeries, key: &str) -> Option<&'a KpiComparisonCell> {
    series
        .cells
        .iter()
        .find(|cell| format!("{}:{}", cell.fiscal_year, cell.period_type) == key)
}

/// Locate the series for a `(company_id, metric_key)` pair.
pub fn series_of<'a>(
    result: &'a KpiComparison,
    company_id: &str,
    metric_key: &str,
) -> Option<&'a KpiComparisonSeries> {
    result
        .series
        .iter()
        .find(|s| s.company_id == company_id && s.metric_key == metric_key)
}

/// Convenience map keyed by axis year → cell for one series (annual golden use).
pub fn cells_by_year(series: &KpiComparisonSeries) -> BTreeMap<i64, &KpiComparisonCell> {
    series
        .cells
        .iter()
        .map(|cell| (cell.fiscal_year, cell))
        .collect()
}

// ============================================================================
// Shared test support — the committed sample + ground-truth schema, reused by
// the DB-free golden here and the real-data-gated command test.
// ============================================================================

#[cfg(test)]
pub(crate) mod testsupport {
    use super::*;
    use serde::Deserialize;

    /// The committed hermetic sample (public GPW figures). Same schema as the
    /// gitignored `private/realdata/v061-alignment-ground-truth.json`.
    pub const COMMITTED_SAMPLE: &str = include_str!("comparison_alignment_sample.json");

    #[derive(Debug, Deserialize)]
    pub struct GroundTruth {
        pub pairs: Vec<Pair>,
    }

    #[derive(Debug, Deserialize)]
    pub struct Pair {
        pub id: String,
        pub kpi: String,
        pub value_kind: String,
        pub granularity: String,
        pub companies: Vec<PairCompany>,
        pub expected_axis: Vec<i64>,
        /// Ticker → { year(string) → value }.
        pub expected_cells:
            std::collections::BTreeMap<String, std::collections::BTreeMap<String, i64>>,
        #[serde(default)]
        pub expected_yoy_verified: std::collections::BTreeMap<String, String>,
    }

    #[derive(Debug, Deserialize)]
    pub struct PairCompany {
        pub id: String,
        pub ticker: String,
    }

    impl GroundTruth {
        pub fn parse(raw: &str) -> Self {
            serde_json::from_str(raw).expect("ground-truth JSON parses")
        }

        pub fn committed() -> Self {
            Self::parse(COMMITTED_SAMPLE)
        }
    }

    impl Pair {
        /// The internal company id for a ticker in this pair.
        pub fn company_id(&self, ticker: &str) -> &str {
            &self
                .companies
                .iter()
                .find(|c| c.ticker == ticker)
                .expect("ticker in pair")
                .id
        }

        pub fn company_ids(&self) -> Vec<String> {
            self.companies.iter().map(|c| c.id.clone()).collect()
        }

        /// Build the already-canonical `ComparisonFact`s this pair implies —
        /// all PLN, FY, flow, `passed` — so the DB-free golden feeds the pure
        /// transform exactly what the command layer would after slot selection.
        pub fn facts(&self) -> Vec<ComparisonFact> {
            let mut facts = Vec::new();
            for company in &self.companies {
                let cells = self
                    .expected_cells
                    .get(&company.ticker)
                    .expect("ticker cells");
                for (year, value) in cells {
                    let fiscal_year: i64 = year.parse().expect("year");
                    facts.push(ComparisonFact {
                        company_id: company.id.clone(),
                        metric_key: self.kpi.clone(),
                        fiscal_year,
                        period_type: "FY".to_owned(),
                        period_end_date: Some(format!("{fiscal_year:04}-12-31")),
                        fact_id: format!("fact_{}_{}_{}", company.ticker, self.kpi, fiscal_year),
                        value: Decimal::from(*value),
                        currency: Some("PLN".to_owned()),
                        measure_window: "flow".to_owned(),
                        validation_status: Some("passed".to_owned()),
                    });
                }
            }
            facts
        }

        pub fn metric_meta(&self) -> HashMap<String, MetricMeta> {
            let mut meta = HashMap::new();
            meta.insert(
                self.kpi.clone(),
                MetricMeta {
                    value_kind: Some(self.value_kind.clone()),
                },
            );
            meta
        }
    }
}

#[cfg(test)]
mod tests {
    use super::testsupport::GroundTruth;
    use super::*;
    use rust_decimal::prelude::FromStr;

    fn d(s: &str) -> Decimal {
        Decimal::from_str(s).expect("decimal")
    }

    fn fy_fact(company: &str, metric: &str, year: i64, value: &str) -> ComparisonFact {
        ComparisonFact {
            company_id: company.to_owned(),
            metric_key: metric.to_owned(),
            fiscal_year: year,
            period_type: "FY".to_owned(),
            period_end_date: Some(format!("{year:04}-12-31")),
            fact_id: format!("fact_{company}_{metric}_{year}"),
            value: d(value),
            currency: Some("PLN".to_owned()),
            measure_window: "flow".to_owned(),
            validation_status: Some("passed".to_owned()),
        }
    }

    fn monetary_meta(metric: &str) -> HashMap<String, MetricMeta> {
        let mut m = HashMap::new();
        m.insert(
            metric.to_owned(),
            MetricMeta {
                value_kind: Some("monetary".to_owned()),
            },
        );
        m
    }

    fn kind_meta(metric: &str, kind: &str) -> HashMap<String, MetricMeta> {
        let mut m = HashMap::new();
        m.insert(
            metric.to_owned(),
            MetricMeta {
                value_kind: Some(kind.to_owned()),
            },
        );
        m
    }

    // ---- Delta rules (unit) ------------------------------------------------

    #[test]
    fn monetary_yoy_is_percent_rounded_to_two_dp() {
        let (delta, undef) = compute_delta(d("533234000"), Some(d("482013000")), Some("monetary"));
        assert_eq!(delta.as_deref(), Some("10.63"));
        assert!(!undef);
        let (delta, undef) = compute_delta(d("118134000"), Some(d("128338000")), Some("monetary"));
        assert_eq!(delta.as_deref(), Some("-7.95"));
        assert!(!undef);
    }

    #[test]
    fn ratio_delta_is_points_never_undefined() {
        let (delta, undef) = compute_delta(d("12.5"), Some(d("10.0")), Some("ratio"));
        assert_eq!(delta.as_deref(), Some("2.5"));
        assert!(!undef);
        // Even a negative/zero prior is a plain difference, never undefined.
        let (delta, undef) = compute_delta(d("3.0"), Some(d("0")), Some("percentage"));
        assert_eq!(delta.as_deref(), Some("3"));
        assert!(!undef);
    }

    #[test]
    fn percent_from_zero_or_negative_base_is_undefined() {
        assert_eq!(
            compute_delta(d("100"), Some(d("0")), Some("monetary")),
            (None, true)
        );
        assert_eq!(
            compute_delta(d("100"), Some(d("-50")), Some("monetary")),
            (None, true)
        );
    }

    #[test]
    fn percent_flip_positive_to_negative_is_undefined_but_to_zero_is_defined() {
        // +100 -> -50 : sign flip → undefined.
        assert_eq!(
            compute_delta(d("-50"), Some(d("100")), Some("monetary")),
            (None, true)
        );
        // +100 -> 0 : went to zero, a defined -100%.
        assert_eq!(
            compute_delta(d("0"), Some(d("100")), Some("monetary")),
            (Some("-100".to_owned()), false)
        );
    }

    #[test]
    fn no_prior_is_null_not_undefined() {
        assert_eq!(
            compute_delta(d("100"), None, Some("monetary")),
            (None, false)
        );
    }

    // ---- Alignment / flags -------------------------------------------------

    #[test]
    fn sparse_gap_is_typed_no_fact_never_zero() {
        // CRQ has 2022..2024; 2021 and 2025 are gaps SCW fills.
        let facts = vec![
            fy_fact("crq", "revenue", 2022, "7680000"),
            fy_fact("crq", "revenue", 2023, "13028000"),
            fy_fact("crq", "revenue", 2024, "15807000"),
            fy_fact("scw", "revenue", 2021, "1551000"),
            fy_fact("scw", "revenue", 2025, "21360000"),
        ];
        let result = compute_comparison(
            Granularity::Annual,
            &["crq".to_owned(), "scw".to_owned()],
            &["revenue".to_owned()],
            &facts,
            &monetary_meta("revenue"),
            &NoFx,
        );
        let axis_years: Vec<i64> = result.axis.iter().map(|p| p.fiscal_year).collect();
        assert_eq!(axis_years, vec![2021, 2022, 2023, 2024, 2025]);
        let crq = series_of(&result, "crq", "revenue").expect("crq series");
        let crq_2021 = cell_at(crq, "2021:FY").expect("cell");
        assert_eq!(crq_2021.value, None);
        assert_eq!(crq_2021.flags, vec![CellFlag::NoFact]);
        // YoY for 2022 has a gap prior (2021) → null, not undefined.
        let crq_2022 = cell_at(crq, "2022:FY").expect("cell");
        assert_eq!(crq_2022.delta_yo_y, None);
        assert!(crq_2022.flags.is_empty());
    }

    #[test]
    fn pln_passes_through_and_annual_has_no_qoq() {
        let facts = vec![fy_fact("acp", "revenue", 2005, "533234000")];
        let result = compute_comparison(
            Granularity::Annual,
            &["acp".to_owned()],
            &["revenue".to_owned()],
            &facts,
            &monetary_meta("revenue"),
            &NoFx,
        );
        let cell = cell_at(series_of(&result, "acp", "revenue").unwrap(), "2005:FY").unwrap();
        assert_eq!(cell.value_pln.as_deref(), Some("533234000"));
        assert_eq!(cell.fx_basis.as_deref(), Some("native_pln"));
        assert_eq!(cell.delta_qo_q, None); // annual → no QoQ ever
    }

    #[test]
    fn non_iso_currency_is_currency_unknown_shares_bug() {
        let mut fact = fy_fact("xyz", "eps_basic", 2024, "3.14");
        fact.currency = Some("shares".to_owned()); // card 47a21f1 data bug
        let result = compute_comparison(
            Granularity::Annual,
            &["xyz".to_owned()],
            &["eps_basic".to_owned()],
            &[fact],
            &monetary_meta("eps_basic"),
            &NoFx,
        );
        let cell = cell_at(series_of(&result, "xyz", "eps_basic").unwrap(), "2024:FY").unwrap();
        assert_eq!(cell.value.as_deref(), Some("3.14")); // native value still there
        assert_eq!(cell.value_pln, None);
        assert!(cell.flags.contains(&CellFlag::CurrencyUnknown));
    }

    #[test]
    fn ratio_null_currency_carries_no_currency_unknown_flag() {
        // A ratio/percentage KPI carries no currency — a NULL (or non-ISO)
        // currency is CORRECT for it, not the unconvertible-monetary data bug.
        // The read model must NOT tag it currency_unknown (card b875e69 A7 scope,
        // ADR 0089 dec. 8). The native value is the comparable number.
        for kind in ["ratio", "percentage"] {
            let mut fact = fy_fact("acp", "gross_margin", 2024, "41.5");
            fact.currency = None; // ratios have no currency
            let result = compute_comparison(
                Granularity::Annual,
                &["acp".to_owned()],
                &["gross_margin".to_owned()],
                &[fact],
                &kind_meta("gross_margin", kind),
                &NoFx,
            );
            let cell = cell_at(
                series_of(&result, "acp", "gross_margin").unwrap(),
                "2024:FY",
            )
            .unwrap();
            assert_eq!(
                cell.value.as_deref(),
                Some("41.5"),
                "[{kind}] native value kept"
            );
            assert!(
                !cell.flags.contains(&CellFlag::CurrencyUnknown),
                "[{kind}] ratio NULL currency must NOT flag currency_unknown, got {:?}",
                cell.flags
            );
            assert!(
                cell.flags.is_empty(),
                "[{kind}] clean ratio cell has no flags"
            );
        }
    }

    #[test]
    fn monetary_null_currency_still_currency_unknown() {
        // Monetary/per-share stay strict: a NULL currency on a monetary fact is
        // still an unconvertible bug and MUST flag currency_unknown.
        let mut fact = fy_fact("acp", "revenue", 2024, "100");
        fact.currency = None;
        let result = compute_comparison(
            Granularity::Annual,
            &["acp".to_owned()],
            &["revenue".to_owned()],
            &[fact],
            &monetary_meta("revenue"),
            &NoFx,
        );
        let cell = cell_at(series_of(&result, "acp", "revenue").unwrap(), "2024:FY").unwrap();
        assert_eq!(cell.value.as_deref(), Some("100"));
        assert_eq!(cell.value_pln, None);
        assert!(cell.flags.contains(&CellFlag::CurrencyUnknown));
    }

    #[test]
    fn non_pln_iso_without_rate_is_fx_missing_value_kept() {
        let mut fact = fy_fact("de", "revenue", 2024, "100");
        fact.currency = Some("EUR".to_owned());
        let result = compute_comparison(
            Granularity::Annual,
            &["de".to_owned()],
            &["revenue".to_owned()],
            &[fact],
            &monetary_meta("revenue"),
            &NoFx, // no rates
        );
        let cell = cell_at(series_of(&result, "de", "revenue").unwrap(), "2024:FY").unwrap();
        assert_eq!(cell.value.as_deref(), Some("100"));
        assert_eq!(cell.value_pln, None);
        assert!(cell.flags.contains(&CellFlag::FxMissing));
    }

    #[test]
    fn non_pln_iso_with_rate_converts_and_labels_basis() {
        struct FixedFx(Decimal, FxBasis);
        impl FxLookup for FixedFx {
            fn resolve(
                &self,
                _: &str,
                _: MeasureWindow,
                _: &str,
                _: &str,
            ) -> Option<(Decimal, FxBasis)> {
                Some((self.0, self.1))
            }
        }
        let mut fact = fy_fact("de", "revenue", 2024, "100");
        fact.currency = Some("EUR".to_owned());
        let result = compute_comparison(
            Granularity::Annual,
            &["de".to_owned()],
            &["revenue".to_owned()],
            &[fact],
            &monetary_meta("revenue"),
            &FixedFx(d("4.30"), FxBasis::PeriodAverage),
        );
        let cell = cell_at(series_of(&result, "de", "revenue").unwrap(), "2024:FY").unwrap();
        assert_eq!(cell.value_pln.as_deref(), Some("430"));
        assert_eq!(cell.fx_basis.as_deref(), Some("period_average"));
        assert!(cell.flags.is_empty());
    }

    #[test]
    fn n1_serves_fundamentals_identically() {
        // N=1 is the same read model (ADR 0089 dec. 1 "one read model, two surfaces").
        let facts = vec![
            fy_fact("acp", "revenue", 2004, "482013000"),
            fy_fact("acp", "revenue", 2005, "533234000"),
        ];
        let result = compute_comparison(
            Granularity::Annual,
            &["acp".to_owned()],
            &["revenue".to_owned()],
            &facts,
            &monetary_meta("revenue"),
            &NoFx,
        );
        assert_eq!(result.series.len(), 1);
        let cell = cell_at(&result.series[0], "2005:FY").unwrap();
        assert_eq!(cell.delta_yo_y.as_deref(), Some("10.63"));
    }

    // ---- Ground-truth golden (all 3 hand-labeled pairs) --------------------

    #[test]
    fn ground_truth_pairs_reproduce_axis_values_and_deltas() {
        let gt = GroundTruth::committed();
        for pair in &gt.pairs {
            let granularity = Granularity::parse(&pair.granularity);
            let result = compute_comparison(
                granularity,
                &pair.company_ids(),
                std::slice::from_ref(&pair.kpi),
                &pair.facts(),
                &pair.metric_meta(),
                &NoFx,
            );

            // Axis matches the hand-labeled union axis exactly.
            let axis_years: Vec<i64> = result.axis.iter().map(|p| p.fiscal_year).collect();
            assert_eq!(axis_years, pair.expected_axis, "[{}] axis", pair.id);

            for company in &pair.companies {
                let series = series_of(&result, &company.id, &pair.kpi).expect("series present");
                let expected = pair.expected_cells.get(&company.ticker).unwrap();
                let cells = cells_by_year(series);
                for year in &pair.expected_axis {
                    let cell = cells.get(year).expect("cell for axis year");
                    match expected.get(&year.to_string()) {
                        Some(value) => {
                            assert_eq!(
                                cell.value.as_deref(),
                                Some(value.to_string().as_str()),
                                "[{}] {} {} value",
                                pair.id,
                                company.ticker,
                                year
                            );
                            // All-PLN sample: valuePln mirrors native, native_pln basis.
                            assert_eq!(cell.value_pln.as_deref(), Some(value.to_string().as_str()));
                            assert_eq!(cell.fx_basis.as_deref(), Some("native_pln"));
                        }
                        None => {
                            assert_eq!(
                                cell.value, None,
                                "[{}] {} {} gap",
                                pair.id, company.ticker, year
                            );
                            assert_eq!(cell.flags, vec![CellFlag::NoFact]);
                        }
                    }
                }

                // Hand-verified YoY expectations, exact.
                for (label, expectation) in &pair.expected_yoy_verified {
                    let Some(rest) = label.strip_prefix(&format!("{}_", company.ticker)) else {
                        continue;
                    };
                    let year: i64 = rest.parse().expect("year in yoy label");
                    let cell = cells.get(&year).expect("cell for yoy year");
                    match expectation.as_str() {
                        "null" => assert_eq!(
                            cell.delta_yo_y, None,
                            "[{}] {} {} yoy null",
                            pair.id, company.ticker, year
                        ),
                        e if e.starts_with("delta_undefined") => {
                            assert_eq!(
                                cell.delta_yo_y, None,
                                "[{}] {} {} yoy undefined value",
                                pair.id, company.ticker, year
                            );
                            assert!(
                                cell.flags.contains(&CellFlag::DeltaYoyUndefined),
                                "[{}] {} {} yoy undefined flag",
                                pair.id,
                                company.ticker,
                                year
                            );
                        }
                        pct => assert_eq!(
                            cell.delta_yo_y.as_deref(),
                            Some(pct),
                            "[{}] {} {} yoy pct",
                            pair.id,
                            company.ticker,
                            year
                        ),
                    }
                }
            }
        }
    }

    #[test]
    fn ground_truth_pair_snapshot_is_stable() {
        // Golden snapshot of the full response for the sparse pair (axis + cells
        // + deltas + flags), so any drift in alignment/labeling reddens.
        let gt = GroundTruth::committed();
        let pair = gt
            .pairs
            .iter()
            .find(|p| p.id == "pair3_sparse_mismatched_revenue")
            .unwrap();
        let result = compute_comparison(
            Granularity::parse(&pair.granularity),
            &pair.company_ids(),
            std::slice::from_ref(&pair.kpi),
            &pair.facts(),
            &pair.metric_meta(),
            &NoFx,
        );
        insta::assert_debug_snapshot!("kpi_comparison_sparse_pair", result);
    }
}

#[cfg(test)]
mod proptests {
    //! Data-transform invariants (ADR 0049): the axis and every cell depend only
    //! on the *set* of facts (company-order & insertion-order independence),
    //! the transform is idempotent, and it never panics on sparse / mixed /
    //! currency-dirty inputs.
    use super::*;
    use proptest::prelude::*;

    fn company_strategy() -> impl Strategy<Value = String> {
        prop::sample::select(vec!["c1", "c2", "c3"]).prop_map(str::to_owned)
    }

    fn currency_strategy() -> impl Strategy<Value = Option<String>> {
        prop::sample::select(vec![
            Some("PLN".to_owned()),
            Some("EUR".to_owned()),
            Some("USD".to_owned()),
            Some("shares".to_owned()), // the data bug
            None,
            Some("".to_owned()),
        ])
    }

    fn fact_strategy() -> impl Strategy<Value = ComparisonFact> {
        (
            company_strategy(),
            prop::sample::select(vec!["revenue", "net_profit"]),
            2000i64..2026,
            prop::sample::select(vec!["FY", "Q1", "Q2", "Q3", "Q4"]),
            -1_000_000i64..1_000_000,
            currency_strategy(),
            prop::sample::select(vec!["flow", "point_in_time"]),
        )
            .prop_map(|(company, metric, year, ptype, value, currency, window)| {
                ComparisonFact {
                    company_id: company,
                    metric_key: metric.to_owned(),
                    fiscal_year: year,
                    period_type: ptype.to_owned(),
                    period_end_date: Some(format!("{year:04}-12-31")),
                    fact_id: format!("f{year}{ptype}"),
                    value: Decimal::from(value),
                    currency,
                    measure_window: window.to_owned(),
                    validation_status: Some("passed".to_owned()),
                }
            })
    }

    fn facts_strategy() -> impl Strategy<Value = Vec<ComparisonFact>> {
        proptest::collection::vec(fact_strategy(), 0..40)
    }

    fn meta() -> HashMap<String, MetricMeta> {
        let mut m = HashMap::new();
        for (k, kind) in [("revenue", "monetary"), ("net_profit", "monetary")] {
            m.insert(
                k.to_owned(),
                MetricMeta {
                    value_kind: Some(kind.to_owned()),
                },
            );
        }
        m
    }

    fn run(companies: &[String], facts: &[ComparisonFact], gran: Granularity) -> KpiComparison {
        compute_comparison(
            gran,
            companies,
            &["revenue".to_owned(), "net_profit".to_owned()],
            facts,
            &meta(),
            &NoFx,
        )
    }

    proptest! {
        #[test]
        fn never_panics_on_arbitrary_facts(
            facts in facts_strategy(),
            gran in prop::sample::select(vec![Granularity::Annual, Granularity::Quarterly]),
        ) {
            let out = run(&["c1".to_owned(), "c2".to_owned(), "c3".to_owned()], &facts, gran);
            // Every series' cells align 1:1 with the axis.
            for series in &out.series {
                prop_assert_eq!(series.cells.len(), out.axis.len());
            }
        }

        #[test]
        fn idempotent(facts in facts_strategy()) {
            let companies = vec!["c1".to_owned(), "c2".to_owned()];
            let a = run(&companies, &facts, Granularity::Annual);
            let b = run(&companies, &facts, Granularity::Annual);
            prop_assert_eq!(a, b);
        }

        #[test]
        fn fact_insertion_order_does_not_change_the_result(
            mut facts in facts_strategy(),
            seed in any::<u64>(),
        ) {
            let companies = vec!["c1".to_owned(), "c2".to_owned(), "c3".to_owned()];
            let original = run(&companies, &facts, Granularity::Annual);
            // Deterministic shuffle.
            let mut s = seed;
            for i in (1..facts.len()).rev() {
                s = s.wrapping_mul(6364136223846793005).wrapping_add(1442695040888963407);
                let j = (s >> 33) as usize % (i + 1);
                facts.swap(i, j);
            }
            let permuted = run(&companies, &facts, Granularity::Annual);
            prop_assert_eq!(original, permuted);
        }

        #[test]
        fn company_order_independence_of_axis_and_cells(facts in facts_strategy()) {
            let forward = run(&["c1".to_owned(), "c2".to_owned(), "c3".to_owned()], &facts, Granularity::Annual);
            let reversed = run(&["c3".to_owned(), "c2".to_owned(), "c1".to_owned()], &facts, Granularity::Annual);
            // Axis is identical regardless of company order.
            prop_assert_eq!(&forward.axis, &reversed.axis);
            // Each (company, metric) series' cells are identical regardless of order.
            for series in &forward.series {
                let other = series_of(&reversed, &series.company_id, &series.metric_key)
                    .expect("same series present under reversed order");
                prop_assert_eq!(&series.cells, &other.cells);
            }
        }
    }
}
