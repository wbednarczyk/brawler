//! Comparative valuation — the **pure** core (ADR 0041 home; ADR 0089 dec. 4–5,
//! v0.61 §B2). No I/O: it takes the target company id, its sector, current price,
//! a `data_as_of` domain date, the target's per-share value **drivers**, one
//! [`PeerMultiples`] per tracked company sharing the sector, and a provenance
//! `validation` component (0..1) from the command layer, and produces the
//! per-method implied fair-value ranges, the method-convergence spread, and the
//! deterministic confidence grade the `compute_comparative_valuation` command
//! serializes and persists. Keeping the transform pure lets the ADR 0049
//! golden/proptest suite pin the valuation math DB-free, with determinism and
//! range-ordering guarantees provable.
//!
//! ## What it does NOT do
//! It never derives ratios or reads facts — peer multiples come pre-evaluated
//! from the level-0 ratio path (`compute_price_context`, the B1 reuse pattern),
//! and the target drivers come pre-resolved from the derived-metrics resolver.
//! The DCF engine (v0.62) joins this slice as new methods, not new columns.
//!
//! ## Methods (ADR 0089 dec. 4)
//! Peer-median-multiple implied fair value per share, one range per method:
//!
//! - **P/E** (`pe_multiple`): `fair_equity = peer_multiple × net_profit_ttm`,
//!   per share `= fair_equity / shares_outstanding`.
//! - **P/BV** (`pbv_multiple`): `fair_equity = peer_multiple × total_equity`,
//!   per share `= fair_equity / shares_outstanding`.
//! - **EV/EBITDA** (`ev_ebitda_multiple`): `implied_ev = peer_multiple ×
//!   ebitda_ttm`; `implied_equity = implied_ev − net_debt` (the equity bridge),
//!   floored at 0 (negative equity value is not meaningful); per share
//!   `= implied_equity / shares_outstanding`.
//!
//! ## Range rule (documented)
//! Each method's range is driven by the **dispersion of the peer multiples**:
//! `low` = peers' **25th** percentile multiple, `base` = **median** (50th),
//! `high` = **75th** percentile — each carried through the method's driver (with
//! the EV→equity bridge for EV/EBITDA). Percentiles use **linear interpolation**
//! between the two nearest order statistics (numpy "linear" / Excel PERCENTILE.INC
//! / Hyndman–Fan type 7): position `h = (N−1)·p`, value `= sorted[⌊h⌋] +
//! (h−⌊h⌋)·(sorted[⌈h⌉] − sorted[⌊h⌋])`. Because the percentiles are ordered and
//! every driver is positive, `low ≤ base ≤ high` always holds.
//!
//! **Peer set excludes the target itself** — valuing a company against its own
//! multiple is circular. A method needs **≥ 2** *other* peers with a defined
//! multiple (one other name is noise, not a distribution); fewer →
//! [`MethodAbsentReason::InsufficientPeers`].
//!
//! ## Confidence grade (ADR 0089 dec. 4; components inspectable)
//! A deterministic composite of four components, each carried in the payload as
//! a `0..1` decimal:
//!
//! | component            | definition                                            |
//! |----------------------|-------------------------------------------------------|
//! | `data_completeness`  | methods with a computed range ÷ 3                      |
//! | `peer_depth`         | `min(1, peer_count / 4)` (the thin threshold)          |
//! | `method_convergence` | `(100 − spread%) / 100` when ≥ 2 methods, else 0       |
//! | `validation`         | fraction of driver facts validated (from the command)  |
//!
//! `composite = 0.30·completeness + 0.25·peer_depth + 0.25·convergence +
//! 0.20·validation`; grade **A** ≥ 0.85, **B** ≥ 0.65, **C** ≥ 0.40, else **D**.
//!
//! ## Typed honesty (never NaN/0)
//! A missing required driver (incl. `net_debt` for the EV bridge, or
//! `shares_outstanding`) → [`MethodAbsentReason::NoDriver`]; a driver ≤ 0 (a
//! multiple over a loss / negative book is meaningless) →
//! [`MethodAbsentReason::NonPositiveDriver`]; fewer than two defined peer
//! multiples → [`MethodAbsentReason::InsufficientPeers`]. A company with no
//! sector → top-level [`ValuationEmptyReason::NoSector`] (every method then
//! `InsufficientPeers`).

use rust_decimal::Decimal;
use serde::Serialize;

/// Thin peer-set threshold (ADR 0089 dec. 3/4): fewer than this many tracked
/// companies in the sector flags the whole read `thin` and caps `peer_depth`.
pub const THIN_PEER_THRESHOLD: u32 = 4;

/// Minimum **other** peers with a defined multiple before a method's range is
/// meaningful (a single other name is noise, not a distribution).
const MIN_DEFINED_PEERS: usize = 2;

/// Decimal display precision for per-share fair values / percentages (division
/// by shares can be non-terminating; a fixed dp keeps the output deterministic).
const FAIR_VALUE_DP: u32 = 4;

// ============================================================================
// Output DTOs (ts-rs → ../../src/api/generated/; camelCase IPC contract)
// ============================================================================

/// The three level-1 comparative-valuation methods (ADR 0089 dec. 4). DCF adds
/// **new variants** in v0.62, never new columns.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[cfg_attr(feature = "ts-export", derive(ts_rs::TS))]
#[cfg_attr(
    feature = "ts-export",
    ts(export, export_to = "../../src/api/generated/")
)]
#[serde(rename_all = "snake_case")]
pub enum ValuationMethod {
    /// Peer median P/E × the company's TTM net profit, per share.
    PeMultiple,
    /// Peer median EV/EBITDA × the company's TTM EBITDA, less net debt, per share.
    EvEbitdaMultiple,
    /// Peer median P/BV × the company's book equity, per share.
    PbvMultiple,
}

impl ValuationMethod {
    /// The stable `method` token stored in `valuation_runs.method`.
    pub fn as_str(self) -> &'static str {
        match self {
            ValuationMethod::PeMultiple => "pe_multiple",
            ValuationMethod::EvEbitdaMultiple => "ev_ebitda_multiple",
            ValuationMethod::PbvMultiple => "pbv_multiple",
        }
    }

    /// The canonical driver metric key this method values off.
    pub fn driver_key(self) -> &'static str {
        match self {
            ValuationMethod::PeMultiple => "net_profit_ttm",
            ValuationMethod::EvEbitdaMultiple => "ebitda_ttm",
            ValuationMethod::PbvMultiple => "total_equity",
        }
    }
}

/// Why a single method has no fair-value range (typed absence, never NaN/0).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[cfg_attr(feature = "ts-export", derive(ts_rs::TS))]
#[cfg_attr(
    feature = "ts-export",
    ts(export, export_to = "../../src/api/generated/")
)]
#[serde(rename_all = "snake_case")]
pub enum MethodAbsentReason {
    /// A required driver is missing (the primary driver, `shares_outstanding`,
    /// or — for EV/EBITDA — `net_debt` for the equity bridge).
    NoDriver,
    /// A required driver is ≤ 0 (a multiple over a loss / negative book is
    /// meaningless), so no defensible range exists.
    NonPositiveDriver,
    /// Fewer than two *other* peers have a defined multiple for this method.
    InsufficientPeers,
}

/// Why the whole valuation read is empty — never a silent absence.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[cfg_attr(feature = "ts-export", derive(ts_rs::TS))]
#[cfg_attr(
    feature = "ts-export",
    ts(export, export_to = "../../src/api/generated/")
)]
#[serde(rename_all = "snake_case")]
pub enum ValuationEmptyReason {
    /// The company has no classified sector, so no peer set can be derived.
    NoSector,
}

/// The ordered confidence grade (A best … D weakest).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[cfg_attr(feature = "ts-export", derive(ts_rs::TS))]
#[cfg_attr(
    feature = "ts-export",
    ts(export, export_to = "../../src/api/generated/")
)]
pub enum ConfidenceGradeLetter {
    A,
    B,
    C,
    D,
}

impl ConfidenceGradeLetter {
    /// The stable single-letter token stored in `valuation_runs.confidence_grade`.
    pub fn as_str(self) -> &'static str {
        match self {
            ConfidenceGradeLetter::A => "A",
            ConfidenceGradeLetter::B => "B",
            ConfidenceGradeLetter::C => "C",
            ConfidenceGradeLetter::D => "D",
        }
    }
}

/// One method's implied fair-value range plus the peer-multiple dispersion it
/// was derived from.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[cfg_attr(feature = "ts-export", derive(ts_rs::TS))]
#[cfg_attr(
    feature = "ts-export",
    ts(export, export_to = "../../src/api/generated/")
)]
#[serde(rename_all = "camelCase")]
pub struct ValuationMethodResult {
    pub method: ValuationMethod,
    /// The canonical driver metric key (`net_profit_ttm` / `ebitda_ttm` /
    /// `total_equity`).
    pub driver_key: String,
    /// The company's driver value, decimal-exact TEXT. `None` when absent.
    pub driver_value: Option<String>,
    /// Peers' 25th-percentile multiple (drives `fairLow`). `None` on absence.
    pub peer_multiple_low: Option<String>,
    /// Peers' median multiple (drives `fairBase`). `None` on absence.
    pub peer_multiple_base: Option<String>,
    /// Peers' 75th-percentile multiple (drives `fairHigh`). `None` on absence.
    pub peer_multiple_high: Option<String>,
    /// Implied fair value per share at the low / base / high multiple, decimal
    /// TEXT (`FAIR_VALUE_DP` dp). `None` on a typed absence.
    pub fair_low: Option<String>,
    pub fair_base: Option<String>,
    pub fair_high: Option<String>,
    /// Number of *other* peers with a defined multiple behind the range.
    pub peer_sample_size: u32,
    /// Set on a typed absence; `None` for a computed range.
    pub absent_reason: Option<MethodAbsentReason>,
}

/// Method-convergence spread across the methods that produced a base value.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[cfg_attr(feature = "ts-export", derive(ts_rs::TS))]
#[cfg_attr(
    feature = "ts-export",
    ts(export, export_to = "../../src/api/generated/")
)]
#[serde(rename_all = "camelCase")]
pub struct ConvergenceSpread {
    /// The lowest / highest method base fair value, decimal TEXT.
    pub base_low: String,
    pub base_high: String,
    /// `(base_high − base_low) / median_of_bases · 100`, 2 dp, decimal TEXT.
    pub spread_pct: String,
    /// How many methods produced a base value (≥ 2).
    pub method_count: u32,
}

/// The deterministic confidence grade with its inspectable components.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[cfg_attr(feature = "ts-export", derive(ts_rs::TS))]
#[cfg_attr(
    feature = "ts-export",
    ts(export, export_to = "../../src/api/generated/")
)]
#[serde(rename_all = "camelCase")]
pub struct ConfidenceGrade {
    pub grade: ConfidenceGradeLetter,
    /// The weighted composite in `0..1`, decimal TEXT.
    pub composite: String,
    /// Each component in `0..1`, decimal TEXT (inspectable, ADR 0089 dec. 4).
    pub data_completeness: String,
    pub peer_depth: String,
    pub method_convergence: String,
    pub validation: String,
}

/// The full comparative-valuation response (also the shape persisted per method
/// into `valuation_runs`).
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[cfg_attr(feature = "ts-export", derive(ts_rs::TS))]
#[cfg_attr(
    feature = "ts-export",
    ts(export, export_to = "../../src/api/generated/")
)]
#[serde(rename_all = "camelCase")]
pub struct ComparativeValuation {
    pub company_id: String,
    /// The company's sector (echoed). `None` when unclassified (see `emptyReason`).
    pub sector: Option<String>,
    /// Tracked companies sharing the sector, the company itself included.
    pub peer_count: u32,
    /// `true` when `peer_count < 4` (GPW-honest thin flag).
    pub thin: bool,
    /// The current price (football-field marker), decimal TEXT. `None` when no
    /// quote resolves.
    pub current_price: Option<String>,
    /// The domain as-of date of the valuation inputs (ISO `YYYY-MM-DD`); the
    /// newest-run ordering key (never `created_at`).
    pub data_as_of: String,
    /// Set when there is no peer set to value against (no sector).
    pub empty_reason: Option<ValuationEmptyReason>,
    /// One row per method, always all three (P/E, EV/EBITDA, P/BV) in that order.
    pub methods: Vec<ValuationMethodResult>,
    /// Present only when ≥ 2 methods produced a base value.
    pub convergence: Option<ConvergenceSpread>,
    pub confidence: ConfidenceGrade,
}

// ============================================================================
// Inputs (the command layer builds these from the DB; tests build them directly)
// ============================================================================

/// The target company's per-share value drivers, pre-resolved from the
/// derived-metrics engine over confirmed facts. A `None` field is an honest
/// "no confirmed value".
#[derive(Debug, Clone, Default)]
pub struct ValuationDrivers {
    /// Divisor for every per-share figure.
    pub shares_outstanding: Option<Decimal>,
    /// P/E driver — TTM net profit.
    pub net_profit_ttm: Option<Decimal>,
    /// P/BV driver — book equity.
    pub total_equity: Option<Decimal>,
    /// EV/EBITDA driver — TTM EBITDA.
    pub ebitda_ttm: Option<Decimal>,
    /// EV→equity bridge — net debt (may legitimately be negative: net cash).
    pub net_debt: Option<Decimal>,
}

/// One tracked company's level-0 multiples (the target included; the target's
/// own row is excluded from every method's peer set).
#[derive(Debug, Clone)]
pub struct PeerMultiples {
    pub company_id: String,
    pub pe: Option<Decimal>,
    pub ev_ebitda: Option<Decimal>,
    pub pbv: Option<Decimal>,
}

impl PeerMultiples {
    pub fn new(company_id: &str) -> Self {
        Self {
            company_id: company_id.to_owned(),
            pe: None,
            ev_ebitda: None,
            pbv: None,
        }
    }

    fn multiple_for(&self, method: ValuationMethod) -> Option<Decimal> {
        match method {
            ValuationMethod::PeMultiple => self.pe,
            ValuationMethod::EvEbitdaMultiple => self.ev_ebitda,
            ValuationMethod::PbvMultiple => self.pbv,
        }
    }
}

// ============================================================================
// Core transform
// ============================================================================

/// Render a decimal as plain decimal text (no exponent), trailing zeros trimmed.
fn plain(value: Decimal) -> String {
    value.normalize().to_string()
}

/// Round to the fair-value display precision, then render plain.
fn plain_dp(value: Decimal) -> String {
    plain(value.round_dp(FAIR_VALUE_DP))
}

/// Linear-interpolation percentile (numpy "linear" / type 7) over an already
/// **sorted** ascending slice. `p ∈ [0, 1]`. `sorted` is non-empty.
fn percentile_linear(sorted: &[Decimal], p: Decimal) -> Decimal {
    let n = sorted.len();
    if n == 1 {
        return sorted[0];
    }
    // h = (N-1)·p
    let h = Decimal::from((n - 1) as i64) * p;
    let lower = h.floor();
    let lower_idx = lower.to_string().parse::<usize>().unwrap_or(0).min(n - 1);
    let upper_idx = (lower_idx + 1).min(n - 1);
    let frac = h - lower;
    sorted[lower_idx] + frac * (sorted[upper_idx] - sorted[lower_idx])
}

/// The median of an unsorted slice (used for the convergence denominator).
fn median_of(values: &[Decimal]) -> Decimal {
    let mut sorted = values.to_vec();
    sorted.sort();
    percentile_linear(&sorted, Decimal::new(5, 1)) // 0.5
}

/// The required driver for a method, gated on positivity. Returns the driver
/// value (`> 0`) or the reason it is absent.
fn method_driver(
    method: ValuationMethod,
    drivers: &ValuationDrivers,
) -> Result<Decimal, MethodAbsentReason> {
    let primary = match method {
        ValuationMethod::PeMultiple => drivers.net_profit_ttm,
        ValuationMethod::EvEbitdaMultiple => drivers.ebitda_ttm,
        ValuationMethod::PbvMultiple => drivers.total_equity,
    };
    let primary = primary.ok_or(MethodAbsentReason::NoDriver)?;
    if primary <= Decimal::ZERO {
        return Err(MethodAbsentReason::NonPositiveDriver);
    }
    Ok(primary)
}

/// Turn a peer multiple into an implied fair value per share for a method,
/// applying the EV→equity bridge for EV/EBITDA. `shares > 0`, `driver > 0`.
fn implied_fair_value(
    method: ValuationMethod,
    multiple: Decimal,
    driver: Decimal,
    shares: Decimal,
    net_debt: Decimal,
) -> Decimal {
    let implied_equity = match method {
        ValuationMethod::PeMultiple | ValuationMethod::PbvMultiple => multiple * driver,
        ValuationMethod::EvEbitdaMultiple => {
            let implied_ev = multiple * driver;
            // Floor at zero: a negative implied equity value is not meaningful.
            (implied_ev - net_debt).max(Decimal::ZERO)
        }
    };
    implied_equity / shares
}

/// Compute one method's result from its defined peer multiples and the target
/// drivers.
fn compute_method(
    method: ValuationMethod,
    drivers: &ValuationDrivers,
    peer_multiples: &[Decimal],
) -> ValuationMethodResult {
    let driver_key = method.driver_key().to_owned();
    let driver_text = match method {
        ValuationMethod::PeMultiple => drivers.net_profit_ttm,
        ValuationMethod::EvEbitdaMultiple => drivers.ebitda_ttm,
        ValuationMethod::PbvMultiple => drivers.total_equity,
    }
    .map(plain);

    let absent = |reason: MethodAbsentReason, sample: usize| ValuationMethodResult {
        method,
        driver_key: driver_key.clone(),
        driver_value: driver_text.clone(),
        peer_multiple_low: None,
        peer_multiple_base: None,
        peer_multiple_high: None,
        fair_low: None,
        fair_base: None,
        fair_high: None,
        peer_sample_size: sample as u32,
        absent_reason: Some(reason),
    };

    // Peer depth gate first (a range needs a distribution), then driver gates.
    if peer_multiples.len() < MIN_DEFINED_PEERS {
        return absent(MethodAbsentReason::InsufficientPeers, peer_multiples.len());
    }
    let shares = match drivers.shares_outstanding {
        Some(s) if s > Decimal::ZERO => s,
        Some(_) => return absent(MethodAbsentReason::NonPositiveDriver, peer_multiples.len()),
        None => return absent(MethodAbsentReason::NoDriver, peer_multiples.len()),
    };
    // EV/EBITDA needs net debt for the equity bridge (honest subset otherwise).
    let net_debt = match method {
        ValuationMethod::EvEbitdaMultiple => match drivers.net_debt {
            Some(nd) => nd,
            None => return absent(MethodAbsentReason::NoDriver, peer_multiples.len()),
        },
        _ => Decimal::ZERO,
    };
    let driver = match method_driver(method, drivers) {
        Ok(d) => d,
        Err(reason) => return absent(reason, peer_multiples.len()),
    };

    let mut sorted = peer_multiples.to_vec();
    sorted.sort();
    let p25 = percentile_linear(&sorted, Decimal::new(25, 2));
    let p50 = percentile_linear(&sorted, Decimal::new(50, 2));
    let p75 = percentile_linear(&sorted, Decimal::new(75, 2));

    let fair_low = implied_fair_value(method, p25, driver, shares, net_debt);
    let fair_base = implied_fair_value(method, p50, driver, shares, net_debt);
    let fair_high = implied_fair_value(method, p75, driver, shares, net_debt);

    ValuationMethodResult {
        method,
        driver_key,
        driver_value: driver_text,
        peer_multiple_low: Some(plain_dp(p25)),
        peer_multiple_base: Some(plain_dp(p50)),
        peer_multiple_high: Some(plain_dp(p75)),
        fair_low: Some(plain_dp(fair_low)),
        fair_base: Some(plain_dp(fair_base)),
        fair_high: Some(plain_dp(fair_high)),
        peer_sample_size: peer_multiples.len() as u32,
        absent_reason: None,
    }
}

/// Compute the comparative valuation. Pure and deterministic: the result depends
/// only on the *set* of peer multiples (order-independent) and the drivers.
#[allow(clippy::too_many_arguments)]
pub fn compute_comparative_valuation(
    company_id: &str,
    sector: Option<&str>,
    current_price: Option<Decimal>,
    data_as_of: &str,
    peer_count: u32,
    drivers: &ValuationDrivers,
    peer_multiples: &[PeerMultiples],
    validation: Decimal,
) -> ComparativeValuation {
    let sector = sector
        .map(str::trim)
        .filter(|s| !s.is_empty())
        .map(str::to_owned);
    let empty_reason = if sector.is_none() {
        Some(ValuationEmptyReason::NoSector)
    } else {
        None
    };

    let methods: Vec<ValuationMethodResult> = [
        ValuationMethod::PeMultiple,
        ValuationMethod::EvEbitdaMultiple,
        ValuationMethod::PbvMultiple,
    ]
    .into_iter()
    .map(|method| {
        // Peers with a defined multiple, EXCLUDING the target itself.
        let defined: Vec<Decimal> = peer_multiples
            .iter()
            .filter(|p| p.company_id != company_id)
            .filter_map(|p| p.multiple_for(method))
            .collect();
        compute_method(method, drivers, &defined)
    })
    .collect();

    // Convergence over the methods that produced a base value.
    let bases: Vec<Decimal> = methods
        .iter()
        .filter_map(|m| m.fair_base.as_deref())
        .filter_map(|t| t.parse::<Decimal>().ok())
        .collect();
    let convergence = if bases.len() >= 2 {
        let low = *bases.iter().min().expect("non-empty");
        let high = *bases.iter().max().expect("non-empty");
        let median = median_of(&bases);
        let spread_pct = if median != Decimal::ZERO {
            ((high - low) / median * Decimal::from(100)).round_dp(2)
        } else {
            Decimal::ZERO
        };
        Some(ConvergenceSpread {
            base_low: plain_dp(low),
            base_high: plain_dp(high),
            spread_pct: plain(spread_pct),
            method_count: bases.len() as u32,
        })
    } else {
        None
    };

    let confidence = grade(peer_count, &methods, convergence.as_ref(), validation);

    ComparativeValuation {
        company_id: company_id.to_owned(),
        sector,
        peer_count,
        thin: peer_count < THIN_PEER_THRESHOLD,
        current_price: current_price.map(plain_dp),
        data_as_of: data_as_of.to_owned(),
        empty_reason,
        methods,
        convergence,
        confidence,
    }
}

/// The deterministic confidence composite + grade (ADR 0089 dec. 4).
fn grade(
    peer_count: u32,
    methods: &[ValuationMethodResult],
    convergence: Option<&ConvergenceSpread>,
    validation: Decimal,
) -> ConfidenceGrade {
    let clamp01 = |v: Decimal| v.max(Decimal::ZERO).min(Decimal::ONE);

    let computed = methods.iter().filter(|m| m.absent_reason.is_none()).count();
    let data_completeness = clamp01(Decimal::from(computed as i64) / Decimal::from(3));
    let peer_depth = clamp01(Decimal::from(peer_count) / Decimal::from(THIN_PEER_THRESHOLD));
    let method_convergence = match convergence {
        Some(spread) => {
            let spread_pct = spread
                .spread_pct
                .parse::<Decimal>()
                .unwrap_or(Decimal::ZERO);
            clamp01((Decimal::from(100) - spread_pct) / Decimal::from(100))
        }
        None => Decimal::ZERO,
    };
    let validation = clamp01(validation);

    let composite = (Decimal::new(30, 2) * data_completeness
        + Decimal::new(25, 2) * peer_depth
        + Decimal::new(25, 2) * method_convergence
        + Decimal::new(20, 2) * validation)
        .round_dp(4);

    let letter = if composite >= Decimal::new(85, 2) {
        ConfidenceGradeLetter::A
    } else if composite >= Decimal::new(65, 2) {
        ConfidenceGradeLetter::B
    } else if composite >= Decimal::new(40, 2) {
        ConfidenceGradeLetter::C
    } else {
        ConfidenceGradeLetter::D
    };

    ConfidenceGrade {
        grade: letter,
        composite: plain(composite),
        data_completeness: plain(data_completeness.round_dp(4)),
        peer_depth: plain(peer_depth.round_dp(4)),
        method_convergence: plain(method_convergence.round_dp(4)),
        validation: plain(validation.round_dp(4)),
    }
}

/// Locate a method row (test convenience).
pub fn method_of(
    result: &ComparativeValuation,
    method: ValuationMethod,
) -> Option<&ValuationMethodResult> {
    result.methods.iter().find(|m| m.method == method)
}

#[cfg(test)]
mod tests;
