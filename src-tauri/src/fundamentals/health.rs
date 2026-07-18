//! Deterministic company-health scores — Piotroski F (2000) and Altman Z″
//! emerging-markets variant (1995) — computed as pure functions over a
//! [`MetricsContext`] (ADR 0083 Decisions 2/3/4).
//!
//! Scores are computed **as-of an annual (`FY`) period** using that period plus
//! the prior FY period (interim quarters are skipped, so "prior FY" is always
//! the next annual report, never an intervening quarter). Nothing is persisted:
//! recomputing history is free and deterministic.
//!
//! **Strict completeness (Decision 3):** a headline number renders only when
//! *every* input for that score is present; anything less is
//! [`InsufficientData`](PiotroskiScore::InsufficientData) carrying whatever
//! components did compute plus the missing-input list. A missing balance-sheet
//! input (e.g. `current_liabilities`) is treated as **missing, never zero**. A
//! financial `statement_type` (bank / insurer / broker) is
//! [`NotApplicable`](PiotroskiScore::NotApplicable) — Altman and Piotroski do
//! not apply to financial statements. Never a partial or rescaled headline.
//!
//! Each component carries its measured inputs so the UI can explain every point
//! (9 Piotroski signals, 4 Altman ratios). Types serialize with decimal-exact
//! `String` values (the `OwnershipOverview` convention).

use super::metrics::{HealthScalars, MetricsContext};
use rust_decimal::prelude::*;
use rust_decimal::Decimal;
use serde::Serialize;

/// Company `statement_type` for which the scores apply; every other value
/// (bank / insurance / broker …) is `NotApplicable` (Decision 4).
pub const INDUSTRIAL_STATEMENT_TYPE: &str = "industrial";

/// The published-formula citation labels (Decision 3 — always visible).
pub const PIOTROSKI_VARIANT: &str = "Piotroski F (2000)";
pub const ALTMAN_VARIANT: &str = "Altman Z\u{2033} EM (1995)";

// ============================================================================
// Shared value types
// ============================================================================

/// One measured input used by a component, for the UI's per-point explanation.
/// `key` is `"<metric>@FY<year>"`; `value` is the decimal-exact reading.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[cfg_attr(feature = "ts-export", derive(ts_rs::TS))]
#[cfg_attr(
    feature = "ts-export",
    ts(export, export_to = "../../src/api/generated/")
)]
#[serde(rename_all = "camelCase")]
pub struct MeasuredInput {
    pub key: String,
    pub value: String,
}

/// A required input that was absent (Decision 3 — the strict-completeness list).
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[cfg_attr(feature = "ts-export", derive(ts_rs::TS))]
#[cfg_attr(
    feature = "ts-export",
    ts(export, export_to = "../../src/api/generated/")
)]
#[serde(rename_all = "camelCase")]
pub struct MissingInput {
    /// The metric key (e.g. `long_term_debt`) or a structural reason
    /// (`prior_fy_period`).
    pub metric: String,
    /// The period the input was expected in (`FY2024`, or a reason phrase).
    pub period: String,
}

// ============================================================================
// Piotroski F
// ============================================================================

/// One of the nine Piotroski binary signals, with the inputs it measured.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[cfg_attr(feature = "ts-export", derive(ts_rs::TS))]
#[cfg_attr(
    feature = "ts-export",
    ts(export, export_to = "../../src/api/generated/")
)]
#[serde(rename_all = "camelCase")]
pub struct PiotroskiSignal {
    /// Stable signal code, `F1`..`F9`.
    pub code: String,
    /// Stable name key the UI maps to localized copy (`roa_positive`, …).
    pub name: String,
    /// Whether the signal scored its point.
    pub passed: bool,
    /// `1` when passed, else `0`.
    pub points: u8,
    /// The measured inputs behind this signal.
    pub inputs: Vec<MeasuredInput>,
}

/// The Piotroski F result for one FY period (Decision 3 strict states).
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[cfg_attr(feature = "ts-export", derive(ts_rs::TS))]
#[cfg_attr(
    feature = "ts-export",
    ts(export, export_to = "../../src/api/generated/")
)]
#[serde(tag = "state", rename_all = "snake_case")]
pub enum PiotroskiScore {
    /// All nine signals computed: `score` is their sum (0..=9).
    Headline {
        score: u8,
        signals: Vec<PiotroskiSignal>,
    },
    /// At least one required input absent: the signals that did compute, plus
    /// the missing list. Never a partial headline.
    InsufficientData {
        signals: Vec<PiotroskiSignal>,
        missing: Vec<MissingInput>,
    },
    /// Financial `statement_type` — the score does not apply.
    NotApplicable { reason: String },
}

// ============================================================================
// Altman Z″
// ============================================================================

/// The Altman Z″ distress band (Decision 4 thresholds: >2.6 / 1.1–2.6 / <1.1).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[cfg_attr(feature = "ts-export", derive(ts_rs::TS))]
#[cfg_attr(
    feature = "ts-export",
    ts(export, export_to = "../../src/api/generated/")
)]
#[serde(rename_all = "snake_case")]
pub enum AltmanBand {
    Safe,
    Grey,
    Distress,
}

/// One of the four Altman Z″ ratio inputs, with its weight and contribution.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[cfg_attr(feature = "ts-export", derive(ts_rs::TS))]
#[cfg_attr(
    feature = "ts-export",
    ts(export, export_to = "../../src/api/generated/")
)]
#[serde(rename_all = "camelCase")]
pub struct AltmanComponent {
    /// `X1`..`X4`.
    pub code: String,
    /// Stable name key (`working_capital_to_assets`, …).
    pub name: String,
    /// The published weight for this term (decimal-exact text).
    pub weight: String,
    /// The computed ratio (decimal text).
    pub ratio: String,
    /// `weight * ratio`, this term's contribution to Z″.
    pub contribution: String,
    pub inputs: Vec<MeasuredInput>,
}

/// The Altman Z″ result for one FY period (Decision 3 strict states).
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[cfg_attr(feature = "ts-export", derive(ts_rs::TS))]
#[cfg_attr(
    feature = "ts-export",
    ts(export, export_to = "../../src/api/generated/")
)]
#[serde(tag = "state", rename_all = "snake_case")]
pub enum AltmanScore {
    /// All four ratios computed: the Z″ value and its band.
    Headline {
        #[serde(rename = "zScore")]
        z_score: String,
        band: AltmanBand,
        components: Vec<AltmanComponent>,
    },
    /// At least one required input absent (or a degenerate zero denominator).
    InsufficientData {
        components: Vec<AltmanComponent>,
        missing: Vec<MissingInput>,
    },
    /// Financial `statement_type` — the score does not apply.
    NotApplicable { reason: String },
}

// ============================================================================
// Per-period bundle + company read model
// ============================================================================

/// Both scores for one FY period.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[cfg_attr(feature = "ts-export", derive(ts_rs::TS))]
#[cfg_attr(
    feature = "ts-export",
    ts(export, export_to = "../../src/api/generated/")
)]
#[serde(rename_all = "camelCase")]
pub struct HealthPeriodScores {
    pub period_id: String,
    pub fiscal_year: i64,
    pub piotroski: PiotroskiScore,
    pub altman: AltmanScore,
}

// ============================================================================
// Formatting helpers
// ============================================================================

/// Decimal-exact display of a raw measured value.
fn fmt(d: Decimal) -> String {
    d.normalize().to_string()
}

/// Ratios and the Z″ value are rounded to 4 dp — deterministic and readable,
/// still ample for band thresholds and criterion comparisons.
fn fmt_ratio(d: Decimal) -> String {
    d.round_dp(4).normalize().to_string()
}

/// Division that yields `None` on a zero denominator (no panic, no invented
/// value) — the total/no-panic contract.
fn safe_div(num: Decimal, den: Decimal) -> Option<Decimal> {
    if den.is_zero() {
        None
    } else {
        Some(num / den)
    }
}

fn measured(key: &str, year: i64, value: Decimal) -> MeasuredInput {
    MeasuredInput {
        key: format!("{key}@FY{year}"),
        value: fmt(value),
    }
}

// ============================================================================
// Input gathering (strict completeness)
// ============================================================================

/// A required reading: `(metric_key, series_index, fiscal_year)`.
type Spec<'a> = (&'a str, usize, i64);

/// Resolve every spec; on success return the values paired with their
/// [`MeasuredInput`], else the full list of missing inputs (all of them, so the
/// UI shows the complete gap, not just the first).
fn gather(
    ctx: &MetricsContext,
    specs: &[Spec],
) -> Result<Vec<(Decimal, MeasuredInput)>, Vec<MissingInput>> {
    let mut values = Vec::with_capacity(specs.len());
    let mut missing = Vec::new();
    for (key, idx, year) in specs {
        match ctx.value_for(key, *idx) {
            Some(v) => values.push((v, measured(key, *year, v))),
            None => missing.push(MissingInput {
                metric: (*key).to_owned(),
                period: format!("FY{year}"),
            }),
        }
    }
    if missing.is_empty() {
        Ok(values)
    } else {
        Err(missing)
    }
}

fn inputs_of(values: &[(Decimal, MeasuredInput)]) -> Vec<MeasuredInput> {
    values.iter().map(|(_, m)| m.clone()).collect()
}

/// The two-year-averaging basis for the leverage / turnover deltas (F5 / F9):
/// the published definition averages `(TA_t, TA_{t-1})` and `(TA_{t-1},
/// TA_{t-2})`, but `TA_{t-2}` needs a third FY period. When it is absent both
/// years fall back to period-end assets — **consistently, never mixed** — and
/// the chosen basis is recorded in the component detail.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum AvgBasis {
    TwoYearAverage,
    PeriodEnd,
}

impl AvgBasis {
    fn label(self) -> &'static str {
        match self {
            AvgBasis::TwoYearAverage => "two_year_average",
            AvgBasis::PeriodEnd => "period_end",
        }
    }
}

// ============================================================================
// Piotroski F computation
// ============================================================================

/// The FY coordinates a Piotroski F is scored over: period `t`, its prior FY
/// `t-1`, and optionally `t-2` (when a third annual report enables the
/// two-year-averaging basis for F5 / F9).
pub struct FyCoords {
    pub idx_t: usize,
    pub year_t: i64,
    pub idx_tm1: usize,
    pub year_tm1: i64,
    pub tm2: Option<(usize, i64)>,
}

/// Piotroski F for the FY period at `coords`, over `ctx`. Callers pass
/// `statement_type` for the `NotApplicable` gate.
pub fn piotroski_f(
    ctx: &MetricsContext,
    coords: &FyCoords,
    statement_type: &str,
) -> PiotroskiScore {
    if statement_type != INDUSTRIAL_STATEMENT_TYPE {
        return PiotroskiScore::NotApplicable {
            reason: statement_type.to_owned(),
        };
    }

    let basis = leverage_basis(ctx, coords);
    let mut signals = Vec::with_capacity(9);
    let mut missing = Vec::new();

    // Collect each signal; a signal missing its inputs contributes to `missing`.
    let computed: [Result<PiotroskiSignal, Vec<MissingInput>>; 9] = [
        f1_roa_positive(ctx, coords),
        f2_cfo_positive(ctx, coords),
        f3_roa_improved(ctx, coords),
        f4_accrual_quality(ctx, coords),
        f5_leverage_improved(ctx, coords, basis),
        f6_liquidity_improved(ctx, coords),
        f7_no_dilution(ctx, coords),
        f8_margin_improved(ctx, coords),
        f9_turnover_improved(ctx, coords, basis),
    ];
    for result in computed {
        match result {
            Ok(signal) => signals.push(signal),
            Err(mut m) => missing.append(&mut m),
        }
    }

    if missing.is_empty() {
        let score = signals.iter().map(|s| s.points).sum();
        PiotroskiScore::Headline { score, signals }
    } else {
        PiotroskiScore::InsufficientData { signals, missing }
    }
}

/// Decide the shared F5/F9 averaging basis: two-year averages when `TA_{t-2}`
/// resolves, else period-end for both years.
fn leverage_basis(ctx: &MetricsContext, coords: &FyCoords) -> AvgBasis {
    match coords.tm2 {
        Some((idx, _)) if ctx.value_for("total_assets", idx).is_some() => AvgBasis::TwoYearAverage,
        _ => AvgBasis::PeriodEnd,
    }
}

fn signal(code: &str, name: &str, passed: bool, inputs: Vec<MeasuredInput>) -> PiotroskiSignal {
    PiotroskiSignal {
        code: code.to_owned(),
        name: name.to_owned(),
        passed,
        points: passed as u8,
        inputs,
    }
}

/// F1 — ROA > 0 (net_profit / total_assets), period t.
fn f1_roa_positive(
    ctx: &MetricsContext,
    c: &FyCoords,
) -> Result<PiotroskiSignal, Vec<MissingInput>> {
    let v = gather(
        ctx,
        &[
            ("net_profit", c.idx_t, c.year_t),
            ("total_assets", c.idx_t, c.year_t),
        ],
    )?;
    let roa = safe_div(v[0].0, v[1].0);
    Ok(signal(
        "F1",
        "roa_positive",
        roa.is_some_and(|r| r > Decimal::ZERO),
        inputs_of(&v),
    ))
}

/// F2 — CFO > 0 (operating_cash_flow), period t.
fn f2_cfo_positive(
    ctx: &MetricsContext,
    c: &FyCoords,
) -> Result<PiotroskiSignal, Vec<MissingInput>> {
    let v = gather(ctx, &[("operating_cash_flow", c.idx_t, c.year_t)])?;
    Ok(signal(
        "F2",
        "cfo_positive",
        v[0].0 > Decimal::ZERO,
        inputs_of(&v),
    ))
}

/// F3 — ΔROA > 0 (ROA_t > ROA_{t-1}).
fn f3_roa_improved(
    ctx: &MetricsContext,
    c: &FyCoords,
) -> Result<PiotroskiSignal, Vec<MissingInput>> {
    let v = gather(
        ctx,
        &[
            ("net_profit", c.idx_t, c.year_t),
            ("total_assets", c.idx_t, c.year_t),
            ("net_profit", c.idx_tm1, c.year_tm1),
            ("total_assets", c.idx_tm1, c.year_tm1),
        ],
    )?;
    let roa_t = safe_div(v[0].0, v[1].0);
    let roa_tm1 = safe_div(v[2].0, v[3].0);
    let passed = match (roa_t, roa_tm1) {
        (Some(a), Some(b)) => a > b,
        _ => false,
    };
    Ok(signal("F3", "roa_improved", passed, inputs_of(&v)))
}

/// F4 — accruals: CFO/total_assets > ROA, period t.
fn f4_accrual_quality(
    ctx: &MetricsContext,
    c: &FyCoords,
) -> Result<PiotroskiSignal, Vec<MissingInput>> {
    let v = gather(
        ctx,
        &[
            ("operating_cash_flow", c.idx_t, c.year_t),
            ("total_assets", c.idx_t, c.year_t),
            ("net_profit", c.idx_t, c.year_t),
        ],
    )?;
    let cfo_ta = safe_div(v[0].0, v[1].0);
    let roa = safe_div(v[2].0, v[1].0);
    let passed = match (cfo_ta, roa) {
        (Some(a), Some(b)) => a > b,
        _ => false,
    };
    Ok(signal("F4", "accrual_quality", passed, inputs_of(&v)))
}

/// F5 — Δleverage: **total non-current (long-term) liabilities** / assets did
/// not rise (`lev_t ≤ lev_{t-1}`), on the shared averaging `basis`. The leverage
/// numerator is `total_liabilities − current_liabilities` (both full-coverage
/// facts) — the BiznesRadar-published "Spadek zadłużenia" basis (ADR 0083
/// Decision 4 amendment, 2026-07-17), broader than interest-bearing
/// `long_term_debt` and free of that tag's sparse ESEF coverage. The
/// `leverage_input` detail records the basis explicitly.
fn f5_leverage_improved(
    ctx: &MetricsContext,
    c: &FyCoords,
    basis: AvgBasis,
) -> Result<PiotroskiSignal, Vec<MissingInput>> {
    let (lev_t, lev_tm1, mut inputs) = leverage_ratios(ctx, c, basis)?;
    inputs.push(MeasuredInput {
        key: "leverage_input".to_owned(),
        value: "non_current_liabilities".to_owned(),
    });
    inputs.push(MeasuredInput {
        key: "basis".to_owned(),
        value: basis.label().to_owned(),
    });
    let passed = match (lev_t, lev_tm1) {
        (Some(a), Some(b)) => a <= b,
        // A zero-asset denominator is pathological; without a defined ratio the
        // "did not increase" test cannot pass.
        _ => false,
    };
    Ok(signal("F5", "leverage_improved", passed, inputs))
}

/// The two leverage ratios (year t and year t-1) on the chosen basis, plus the
/// measured inputs used. The numerator is total non-current liabilities
/// (`total_liabilities − current_liabilities`) per year (ADR 0083 D4 amendment);
/// strict completeness requires **both** balance-sheet inputs for each year, so
/// the missing list names whichever is absent, never a silent zero.
#[allow(clippy::type_complexity)]
fn leverage_ratios(
    ctx: &MetricsContext,
    c: &FyCoords,
    basis: AvgBasis,
) -> Result<(Option<Decimal>, Option<Decimal>, Vec<MeasuredInput>), Vec<MissingInput>> {
    match basis {
        AvgBasis::PeriodEnd => {
            let v = gather(
                ctx,
                &[
                    ("total_liabilities", c.idx_t, c.year_t),
                    ("current_liabilities", c.idx_t, c.year_t),
                    ("total_assets", c.idx_t, c.year_t),
                    ("total_liabilities", c.idx_tm1, c.year_tm1),
                    ("current_liabilities", c.idx_tm1, c.year_tm1),
                    ("total_assets", c.idx_tm1, c.year_tm1),
                ],
            )?;
            let ncl_t = v[0].0 - v[1].0;
            let ncl_tm1 = v[3].0 - v[4].0;
            let lev_t = safe_div(ncl_t, v[2].0);
            let lev_tm1 = safe_div(ncl_tm1, v[5].0);
            Ok((lev_t, lev_tm1, inputs_of(&v)))
        }
        AvgBasis::TwoYearAverage => {
            let (idx_tm2, year_tm2) = c.tm2.expect("two-year basis requires t-2");
            let v = gather(
                ctx,
                &[
                    ("total_liabilities", c.idx_t, c.year_t),
                    ("current_liabilities", c.idx_t, c.year_t),
                    ("total_assets", c.idx_t, c.year_t),
                    ("total_liabilities", c.idx_tm1, c.year_tm1),
                    ("current_liabilities", c.idx_tm1, c.year_tm1),
                    ("total_assets", c.idx_tm1, c.year_tm1),
                    ("total_assets", idx_tm2, year_tm2),
                ],
            )?;
            let ncl_t = v[0].0 - v[1].0;
            let ncl_tm1 = v[3].0 - v[4].0;
            let avg_t = (v[2].0 + v[5].0) / Decimal::TWO;
            let avg_tm1 = (v[5].0 + v[6].0) / Decimal::TWO;
            let lev_t = safe_div(ncl_t, avg_t);
            let lev_tm1 = safe_div(ncl_tm1, avg_tm1);
            Ok((lev_t, lev_tm1, inputs_of(&v)))
        }
    }
}

/// F6 — Δcurrent_ratio > 0 (`CR_t > CR_{t-1}`).
fn f6_liquidity_improved(
    ctx: &MetricsContext,
    c: &FyCoords,
) -> Result<PiotroskiSignal, Vec<MissingInput>> {
    let v = gather(
        ctx,
        &[
            ("current_ratio", c.idx_t, c.year_t),
            ("current_ratio", c.idx_tm1, c.year_tm1),
        ],
    )?;
    Ok(signal(
        "F6",
        "liquidity_improved",
        v[0].0 > v[1].0,
        inputs_of(&v),
    ))
}

/// F7 — no dilution: `shares_t ≤ shares_{t-1}`.
fn f7_no_dilution(
    ctx: &MetricsContext,
    c: &FyCoords,
) -> Result<PiotroskiSignal, Vec<MissingInput>> {
    let v = gather(
        ctx,
        &[
            ("shares_outstanding", c.idx_t, c.year_t),
            ("shares_outstanding", c.idx_tm1, c.year_tm1),
        ],
    )?;
    Ok(signal("F7", "no_dilution", v[0].0 <= v[1].0, inputs_of(&v)))
}

/// F8 — Δgross_margin > 0 (gross_profit/revenue rose).
fn f8_margin_improved(
    ctx: &MetricsContext,
    c: &FyCoords,
) -> Result<PiotroskiSignal, Vec<MissingInput>> {
    let v = gather(
        ctx,
        &[
            ("gross_profit", c.idx_t, c.year_t),
            ("revenue", c.idx_t, c.year_t),
            ("gross_profit", c.idx_tm1, c.year_tm1),
            ("revenue", c.idx_tm1, c.year_tm1),
        ],
    )?;
    let gm_t = safe_div(v[0].0, v[1].0);
    let gm_tm1 = safe_div(v[2].0, v[3].0);
    let passed = match (gm_t, gm_tm1) {
        (Some(a), Some(b)) => a > b,
        _ => false,
    };
    Ok(signal("F8", "margin_improved", passed, inputs_of(&v)))
}

/// F9 — Δasset_turnover > 0 (revenue / avg total_assets rose), on the shared
/// averaging `basis`.
fn f9_turnover_improved(
    ctx: &MetricsContext,
    c: &FyCoords,
    basis: AvgBasis,
) -> Result<PiotroskiSignal, Vec<MissingInput>> {
    let (ato_t, ato_tm1, mut inputs) = turnover_ratios(ctx, c, basis)?;
    inputs.push(MeasuredInput {
        key: "basis".to_owned(),
        value: basis.label().to_owned(),
    });
    let passed = match (ato_t, ato_tm1) {
        (Some(a), Some(b)) => a > b,
        _ => false,
    };
    Ok(signal("F9", "turnover_improved", passed, inputs))
}

#[allow(clippy::type_complexity)]
fn turnover_ratios(
    ctx: &MetricsContext,
    c: &FyCoords,
    basis: AvgBasis,
) -> Result<(Option<Decimal>, Option<Decimal>, Vec<MeasuredInput>), Vec<MissingInput>> {
    match basis {
        AvgBasis::PeriodEnd => {
            let v = gather(
                ctx,
                &[
                    ("revenue", c.idx_t, c.year_t),
                    ("total_assets", c.idx_t, c.year_t),
                    ("revenue", c.idx_tm1, c.year_tm1),
                    ("total_assets", c.idx_tm1, c.year_tm1),
                ],
            )?;
            let ato_t = safe_div(v[0].0, v[1].0);
            let ato_tm1 = safe_div(v[2].0, v[3].0);
            Ok((ato_t, ato_tm1, inputs_of(&v)))
        }
        AvgBasis::TwoYearAverage => {
            let (idx_tm2, year_tm2) = c.tm2.expect("two-year basis requires t-2");
            let v = gather(
                ctx,
                &[
                    ("revenue", c.idx_t, c.year_t),
                    ("total_assets", c.idx_t, c.year_t),
                    ("total_assets", c.idx_tm1, c.year_tm1),
                    ("revenue", c.idx_tm1, c.year_tm1),
                    ("total_assets", idx_tm2, year_tm2),
                ],
            )?;
            let avg_t = (v[1].0 + v[2].0) / Decimal::TWO;
            let avg_tm1 = (v[2].0 + v[4].0) / Decimal::TWO;
            let ato_t = safe_div(v[0].0, avg_t);
            let ato_tm1 = safe_div(v[3].0, avg_tm1);
            Ok((ato_t, ato_tm1, inputs_of(&v)))
        }
    }
}

// ============================================================================
// Altman Z″ computation
// ============================================================================

const W_X1: (i64, u32) = (656, 2); // 6.56
const W_X2: (i64, u32) = (326, 2); // 3.26
const W_X3: (i64, u32) = (672, 2); // 6.72
const W_X4: (i64, u32) = (105, 2); // 1.05

/// Altman Z″ for the FY period at series index `idx` / `year`, over `ctx`.
pub fn altman_z(ctx: &MetricsContext, idx: usize, year: i64, statement_type: &str) -> AltmanScore {
    if statement_type != INDUSTRIAL_STATEMENT_TYPE {
        return AltmanScore::NotApplicable {
            reason: statement_type.to_owned(),
        };
    }

    let mut components = Vec::with_capacity(4);
    let mut missing = Vec::new();

    // (code, name, weight (num, scale), numerator key, denominator key).
    type Term = (
        &'static str,
        &'static str,
        (i64, u32),
        &'static str,
        &'static str,
    );
    let terms: [Term; 4] = [
        (
            "X1",
            "working_capital_to_assets",
            W_X1,
            "working_capital",
            "total_assets",
        ),
        (
            "X2",
            "retained_earnings_to_assets",
            W_X2,
            "retained_earnings",
            "total_assets",
        ),
        (
            "X3",
            "ebit_to_assets",
            W_X3,
            "operating_profit",
            "total_assets",
        ),
        (
            "X4",
            "equity_to_liabilities",
            W_X4,
            "total_equity",
            "total_liabilities",
        ),
    ];

    let mut z = Decimal::ZERO;
    let mut all_ok = true;
    for (code, name, weight_parts, num_key, den_key) in terms {
        let weight = Decimal::new(weight_parts.0, weight_parts.1);
        match altman_component(ctx, idx, year, code, name, weight, num_key, den_key) {
            Ok((component, contribution)) => {
                z += contribution;
                components.push(component);
            }
            Err(mut m) => {
                all_ok = false;
                missing.append(&mut m);
            }
        }
    }

    if all_ok && missing.is_empty() {
        AltmanScore::Headline {
            z_score: fmt_ratio(z),
            band: band_of(z),
            components,
        }
    } else {
        AltmanScore::InsufficientData {
            components,
            missing,
        }
    }
}

#[allow(clippy::too_many_arguments)]
fn altman_component(
    ctx: &MetricsContext,
    idx: usize,
    year: i64,
    code: &str,
    name: &str,
    weight: Decimal,
    num_key: &str,
    den_key: &str,
) -> Result<(AltmanComponent, Decimal), Vec<MissingInput>> {
    let v = gather(ctx, &[(num_key, idx, year), (den_key, idx, year)])?;
    let Some(ratio) = safe_div(v[0].0, v[1].0) else {
        // Present but degenerate (zero denominator): treat as a missing input on
        // the denominator rather than inventing a ratio.
        return Err(vec![MissingInput {
            metric: den_key.to_owned(),
            period: format!("FY{year} (zero)"),
        }]);
    };
    let contribution = weight * ratio;
    Ok((
        AltmanComponent {
            code: code.to_owned(),
            name: name.to_owned(),
            weight: fmt(weight),
            ratio: fmt_ratio(ratio),
            contribution: fmt_ratio(contribution),
            inputs: inputs_of(&v),
        },
        contribution,
    ))
}

fn band_of(z: Decimal) -> AltmanBand {
    let safe = Decimal::new(26, 1); // 2.6
    let distress = Decimal::new(11, 1); // 1.1
    if z > safe {
        AltmanBand::Safe
    } else if z >= distress {
        AltmanBand::Grey
    } else {
        AltmanBand::Distress
    }
}

// ============================================================================
// Company read model + scalar injection
// ============================================================================

/// Score every FY period (newest-first) into a per-period bundle. The `t-1`
/// requirement is explicit: a company's oldest FY (no prior FY) yields a
/// Piotroski `InsufficientData` with a `prior_fy_period` reason, while Altman
/// still computes (it needs only period t).
pub fn company_health(ctx: &MetricsContext, statement_type: &str) -> Vec<HealthPeriodScores> {
    let fys = ctx.fy_period_refs();
    let mut out = Vec::with_capacity(fys.len());
    for (pos, fy) in fys.iter().enumerate() {
        let piotroski = match fys.get(pos + 1) {
            Some(prior) => {
                let coords = FyCoords {
                    idx_t: fy.idx,
                    year_t: fy.fiscal_year,
                    idx_tm1: prior.idx,
                    year_tm1: prior.fiscal_year,
                    tm2: fys.get(pos + 2).map(|p| (p.idx, p.fiscal_year)),
                };
                piotroski_f(ctx, &coords, statement_type)
            }
            None if statement_type != INDUSTRIAL_STATEMENT_TYPE => PiotroskiScore::NotApplicable {
                reason: statement_type.to_owned(),
            },
            None => PiotroskiScore::InsufficientData {
                signals: Vec::new(),
                missing: vec![MissingInput {
                    metric: "prior_fy_period".to_owned(),
                    period: format!("before FY{}", fy.fiscal_year),
                }],
            },
        };
        let altman = altman_z(ctx, fy.idx, fy.fiscal_year, statement_type);
        out.push(HealthPeriodScores {
            period_id: fy.period_id.clone(),
            fiscal_year: fy.fiscal_year,
            piotroski,
            altman,
        });
    }
    out
}

/// The latest-FY headline scalars for scorecard-criterion injection (Decision
/// 2): the Piotroski F integer and the Altman Z″ value, each only when the
/// latest FY is a headline — else `None`, so the key resolves `Unavailable`.
pub fn latest_fy_scalars(ctx: &MetricsContext, statement_type: &str) -> HealthScalars {
    let scores = company_health(ctx, statement_type);
    let Some(latest) = scores.first() else {
        return HealthScalars::default();
    };
    let piotroski_f = match &latest.piotroski {
        PiotroskiScore::Headline { score, .. } => Some(Decimal::from(*score)),
        _ => None,
    };
    let altman_z = match &latest.altman {
        AltmanScore::Headline { z_score, .. } => Decimal::from_str(z_score).ok(),
        _ => None,
    };
    HealthScalars {
        piotroski_f,
        altman_z,
    }
}

#[cfg(test)]
mod tests;
