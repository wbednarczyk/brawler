//! Deterministic fundamentals validation — the objective "good" gate (ADR 0061).
//!
//! This is the backbone of the structured-first extraction pipeline. A fact
//! set extracted from a report (whether from ESEF/iXBRL, a PDF parser, or an
//! HTML aggregator) is **auto-accepted only if it passes this gate**; a failure
//! escalates the pipeline to a second witness and, failing that, raises a
//! "structure changed" notification — it is never silently emitted. The gate is
//! objective and human-free: it checks accounting identities that must hold in
//! any correct financial statement, plus a comparative-period cross-check that
//! catches column-misalignment (the dominant extraction failure mode).
//!
//! Pure: it takes loaded facts (`metric_key -> value` in signed base units) and
//! returns a report. No IO, no connection — fully unit- and property-testable.
//! It shares the value convention of the derived-metrics service
//! ([`super::metrics`]): decimal-exact, signed, base units.
//!
//! ## Extensibility
//!
//! Identities are data, not control flow: [`balance_sheet`], [`cash_flow_tie`],
//! and [`free_cash_flow`] each name the `metric_key`s they consume and yield an
//! [`Outcome`]. Adding an identity as more `kpi_definitions` gain the required
//! keys (e.g. current/non-current subtotals) is a local addition — register a
//! new checker in [`validate`]. A check whose inputs are absent is
//! [`Outcome::NotApplicable`], never a failure: absence is not a contradiction.

use rust_decimal::Decimal;
use std::collections::{BTreeMap, BTreeSet};

// ---------------------------------------------------------------------------
// Metric keys consumed by the identities (must match `kpi_definitions.metric_key`)
// ---------------------------------------------------------------------------

const TOTAL_ASSETS: &str = "total_assets";
const TOTAL_LIABILITIES: &str = "total_liabilities";
const TOTAL_EQUITY: &str = "total_equity";
const CASH: &str = "cash";
const OPERATING_CASH_FLOW: &str = "operating_cash_flow";
const INVESTING_CASH_FLOW: &str = "investing_cash_flow";
const FINANCING_CASH_FLOW: &str = "financing_cash_flow";
const FREE_CASH_FLOW: &str = "free_cash_flow";
const CAPEX: &str = "capex";

/// One (company, period, statement_basis) group's reported facts, keyed by
/// `metric_key`, in signed base units. `BTreeMap` so iteration/debug output is
/// deterministic (order-independent by construction).
pub type FactSet = BTreeMap<String, Decimal>;

// ---------------------------------------------------------------------------
// Tolerance
// ---------------------------------------------------------------------------

/// Slack allowed when comparing two decimal quantities. Real filings round to
/// thousands/millions and carry sub-unit residuals; an exact equality gate
/// would reject correct statements. The allowance is `max(absolute, relative *
/// scale)` where `scale` is the magnitude of the quantities being compared, so
/// it is meaningful for both a 40 bn-PLN balance sheet and a 3 k-PLN one.
#[derive(Debug, Clone, Copy)]
pub struct Tolerance {
    /// Relative allowance as a fraction of the compared scale (e.g. `0.005` =
    /// 0.5%).
    pub relative: Decimal,
    /// Absolute floor in base units, so near-zero scales don't demand
    /// impossible precision.
    pub absolute: Decimal,
}

impl Default for Tolerance {
    fn default() -> Self {
        // 0.5% relative, 1 base-unit absolute floor. Loose enough to absorb
        // reporting rounding, tight enough that a swapped column or a missing
        // line (typically off by >>0.5%) fails.
        Self {
            relative: Decimal::new(5, 3),
            absolute: Decimal::ONE,
        }
    }
}

impl Tolerance {
    /// Whether `residual` is within the allowance for the given `scale`.
    fn accepts(&self, residual: Decimal, scale: Decimal) -> bool {
        let allowed = (scale.abs() * self.relative).max(self.absolute);
        residual.abs() <= allowed
    }
}

// ---------------------------------------------------------------------------
// Outcomes
// ---------------------------------------------------------------------------

/// The result of one identity or cross-check.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Outcome {
    /// The relationship holds within tolerance.
    Pass,
    /// The relationship is violated beyond tolerance.
    Fail {
        /// What the left side should have equalled.
        expected: Decimal,
        /// What it actually was.
        actual: Decimal,
        /// `actual - expected`.
        residual: Decimal,
    },
    /// One or more required inputs were absent — the check could not be
    /// evaluated. This is **not** a failure: absence is not a contradiction.
    NotApplicable {
        /// Which `metric_key`s were missing.
        missing: Vec<String>,
    },
}

impl Outcome {
    pub fn is_fail(&self) -> bool {
        matches!(self, Outcome::Fail { .. })
    }
    pub fn is_pass(&self) -> bool {
        matches!(self, Outcome::Pass)
    }
}

/// A named identity check and its outcome.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct IdentityCheck {
    /// Stable machine id, e.g. `"balance_sheet_identity"`.
    pub id: &'static str,
    /// Human-readable label for the notification/diff surface.
    pub label: &'static str,
    pub outcome: Outcome,
}

/// One metric's comparative cross-check: the prior-period column read out of
/// the current report vs. the value already stored for that prior period.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CrossCheck {
    pub metric_key: String,
    pub outcome: Outcome,
}

/// Overall gate verdict for a fact set.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Status {
    /// At least one check evaluated and none failed — safe to auto-accept.
    Passed,
    /// A contradiction was found — must escalate, never emit silently.
    Failed,
    /// Nothing could be evaluated (no identity had its inputs). Not proof of
    /// correctness; the pipeline treats this as "needs a second witness".
    Inconclusive,
}

/// The full validation report for one fact set.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ValidationReport {
    pub identities: Vec<IdentityCheck>,
    pub cross_checks: Vec<CrossCheck>,
    pub status: Status,
    /// Coverage of the fact set against the company's expected primary KPIs
    /// (ADR 0061 dec. 4d). `None` when no expected set was supplied. Set by
    /// the pipeline after `validate` returns — `validate` itself has no
    /// `expected_keys` input and never populates this field, so it can never
    /// influence [`Status`].
    pub completeness: Option<Completeness>,
}

impl ValidationReport {
    /// Derives the overall [`Status`] from the individual outcomes: any failure
    /// ⇒ `Failed`; otherwise any pass ⇒ `Passed`; otherwise `Inconclusive`.
    fn from_parts(identities: Vec<IdentityCheck>, cross_checks: Vec<CrossCheck>) -> Self {
        let any_fail = identities.iter().any(|c| c.outcome.is_fail())
            || cross_checks.iter().any(|c| c.outcome.is_fail());
        let any_pass = identities.iter().any(|c| c.outcome.is_pass())
            || cross_checks.iter().any(|c| c.outcome.is_pass());
        let status = if any_fail {
            Status::Failed
        } else if any_pass {
            Status::Passed
        } else {
            Status::Inconclusive
        };
        Self {
            identities,
            cross_checks,
            status,
            completeness: None,
        }
    }
}

// ---------------------------------------------------------------------------
// Completeness (ADR 0061 dec. 4d)
// ---------------------------------------------------------------------------

/// One fact set's coverage against the company's expected primary-KPI set.
/// Report-only: a parse that misses every expected metric is not falsified —
/// only unproven — so this never flips [`Status`]/[`Outcome`]. The pipeline
/// uses it solely to downgrade an otherwise-`Accepted` set to
/// `AcceptedUnreviewed` when coverage is zero, the strongest deterministic
/// signal that the parse read the wrong table entirely.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Completeness {
    /// Size of the expected-keys set that was checked.
    pub expected: usize,
    /// How many of `expected` are present in the fact set.
    pub present: usize,
    /// Which expected keys are absent, for the notification/diff surface.
    pub missing: Vec<String>,
}

/// Computes [`Completeness`] of `current` against `expected`. Pure, no
/// side-effects on `current`/`expected`.
pub fn completeness(current: &FactSet, expected: &BTreeSet<String>) -> Completeness {
    let present = expected.iter().filter(|k| current.contains_key(*k)).count();
    let missing = expected
        .iter()
        .filter(|k| !current.contains_key(*k))
        .cloned()
        .collect();
    Completeness {
        expected: expected.len(),
        present,
        missing,
    }
}

// ---------------------------------------------------------------------------
// Individual identities
// ---------------------------------------------------------------------------

/// Collects the values for `keys`, returning `Err(missing)` if any is absent.
fn require(facts: &FactSet, keys: &[&str]) -> Result<Vec<Decimal>, Vec<String>> {
    let mut vals = Vec::with_capacity(keys.len());
    let mut missing = Vec::new();
    for &k in keys {
        match facts.get(k) {
            Some(v) => vals.push(*v),
            None => missing.push(k.to_string()),
        }
    }
    if missing.is_empty() {
        Ok(vals)
    } else {
        Err(missing)
    }
}

/// Balance-sheet identity: `total_assets = total_liabilities + total_equity`.
/// The single most robust statement-level invariant.
pub fn balance_sheet(facts: &FactSet, tol: &Tolerance) -> IdentityCheck {
    let outcome = match require(facts, &[TOTAL_ASSETS, TOTAL_LIABILITIES, TOTAL_EQUITY]) {
        Ok(v) => {
            let (assets, liabilities, equity) = (v[0], v[1], v[2]);
            let expected = liabilities + equity;
            let residual = assets - expected;
            if tol.accepts(residual, assets) {
                Outcome::Pass
            } else {
                Outcome::Fail {
                    expected,
                    actual: assets,
                    residual,
                }
            }
        }
        Err(missing) => Outcome::NotApplicable { missing },
    };
    IdentityCheck {
        id: "balance_sheet_identity",
        label: "Assets = Liabilities + Equity",
        outcome,
    }
}

/// Free-cash-flow definition: `free_cash_flow = operating_cash_flow - capex`.
/// Only evaluated when FCF is a *reported* fact (it is usually derived, in
/// which case this is skipped as not-applicable). `capex` is stored as a
/// positive outflow, matching the seeded derived formula.
pub fn free_cash_flow(facts: &FactSet, tol: &Tolerance) -> IdentityCheck {
    let outcome = match require(facts, &[FREE_CASH_FLOW, OPERATING_CASH_FLOW, CAPEX]) {
        Ok(v) => {
            let (fcf, ocf, capex) = (v[0], v[1], v[2]);
            let expected = ocf - capex;
            let residual = fcf - expected;
            let scale = fcf.abs().max(ocf.abs());
            if tol.accepts(residual, scale) {
                Outcome::Pass
            } else {
                Outcome::Fail {
                    expected,
                    actual: fcf,
                    residual,
                }
            }
        }
        Err(missing) => Outcome::NotApplicable { missing },
    };
    IdentityCheck {
        id: "free_cash_flow_definition",
        label: "Free cash flow = Operating cash flow − Capex",
        outcome,
    }
}

/// Cash-flow tie (cross-period): the sum of operating, investing and financing
/// cash flows equals the change in the cash balance,
/// `OCF + ICF + FCF = cash_now − cash_prior`. Requires the prior period's cash.
/// A residual here is usually FX-on-cash (unmodelled), so the tolerance is the
/// same relative band — a genuine misread is far larger.
pub fn cash_flow_tie(facts: &FactSet, prior: Option<&FactSet>, tol: &Tolerance) -> IdentityCheck {
    let outcome = (|| {
        let cur = match require(
            facts,
            &[
                CASH,
                OPERATING_CASH_FLOW,
                INVESTING_CASH_FLOW,
                FINANCING_CASH_FLOW,
            ],
        ) {
            Ok(v) => v,
            Err(missing) => return Outcome::NotApplicable { missing },
        };
        let prior_cash = match prior.and_then(|p| p.get(CASH)) {
            Some(v) => *v,
            None => {
                return Outcome::NotApplicable {
                    missing: vec![format!("prior:{CASH}")],
                }
            }
        };
        let (cash_now, ocf, icf, fcf) = (cur[0], cur[1], cur[2], cur[3]);
        let flows = ocf + icf + fcf;
        let delta = cash_now - prior_cash;
        let residual = flows - delta;
        let scale = cash_now.abs().max(prior_cash.abs()).max(flows.abs());
        if tol.accepts(residual, scale) {
            Outcome::Pass
        } else {
            Outcome::Fail {
                expected: delta,
                actual: flows,
                residual,
            }
        }
    })();
    IdentityCheck {
        id: "cash_flow_tie",
        label: "Operating + Investing + Financing cash flow = Δ cash",
        outcome,
    }
}

// ---------------------------------------------------------------------------
// Comparative-period cross-check
// ---------------------------------------------------------------------------

/// Cross-checks the prior-period comparatives read out of the *current* report
/// against the values already stored for that prior period. A mismatch is the
/// strongest available signal that the extractor read the wrong column — the
/// dominant PDF failure mode — because the two numbers were produced by
/// independent reads of independent documents and must agree.
///
/// Only metrics present in **both** sets are checked; a metric in one but not
/// the other yields nothing (it is neither confirmation nor contradiction).
pub fn cross_check_prior(
    extracted_prior: &FactSet,
    stored_prior: &FactSet,
    tol: &Tolerance,
) -> Vec<CrossCheck> {
    extracted_prior
        .iter()
        .filter_map(|(key, &extracted)| {
            stored_prior.get(key).map(|&stored| {
                let residual = extracted - stored;
                let scale = extracted.abs().max(stored.abs());
                let outcome = if tol.accepts(residual, scale) {
                    Outcome::Pass
                } else {
                    Outcome::Fail {
                        expected: stored,
                        actual: extracted,
                        residual,
                    }
                };
                CrossCheck {
                    metric_key: key.clone(),
                    outcome,
                }
            })
        })
        .collect()
}

// ---------------------------------------------------------------------------
// Magnitude plausibility against the metric's own history
// ---------------------------------------------------------------------------

/// The relative bound past which a value is implausible against its own history:
/// 100× the median in either direction. A genuine year-over-year move stays well
/// inside this; a dropped unit multiplier or a note reference read as the value
/// lands ~1000× out.
const HISTORY_PLAUSIBILITY_RATIO: i64 = 100;

/// Whether `metric_key`'s scale is set by corporate actions (splits) rather than
/// performance, so a >100× move is legitimate and must not be flagged.
fn is_split_sensitive_metric(metric_key: &str) -> bool {
    matches!(
        metric_key,
        "eps_basic" | "eps_diluted" | "dividend_per_share" | "shares_outstanding"
    )
}

/// Whether `value` is implausible for `metric_key` given the SAME metric's
/// values already stored for the company's OTHER periods — at least
/// [`HISTORY_PLAUSIBILITY_RATIO`]× the median, larger or smaller.
///
/// This is the guard for a uniform scale error that no same-period identity can
/// see and no comparative column exists to catch: a note reference read as the
/// cash value, or a dropped `w tys.` multiplier, in a company's first stored
/// period of that metric (card 22ac70c: Atrem cash 19 000 against siblings
/// ~3.8 M). It **abstains** (returns `false`) when there are fewer than two
/// historical values (no stable median), when the metric is split-sensitive, or
/// when either side is zero. It is a signal to **downgrade acceptance to
/// `flagged`**, never to reject — a genuine collapse or acquisition is surfaced
/// for review, not dropped.
pub fn implausible_against_history(metric_key: &str, value: Decimal, history: &[Decimal]) -> bool {
    if is_split_sensitive_metric(metric_key) {
        return false;
    }
    let Some(median) = history_median(history) else {
        return false;
    };
    let magnitude = value.abs();
    if median.is_zero() || magnitude.is_zero() {
        return false;
    }
    let ratio = Decimal::from(HISTORY_PLAUSIBILITY_RATIO);
    magnitude > median * ratio || magnitude * ratio < median
}

/// The magnitude median of a metric's history — the robust center the
/// plausibility bound in [`implausible_against_history`] is measured against, and
/// the figure the quarantine outcome cites. `None` when fewer than two non-zero
/// magnitudes exist (no stable median); zeros are excluded because a reported
/// zero is not a scale reference. Uses the upper-middle element so it agrees with
/// the check exactly.
pub fn history_median(history: &[Decimal]) -> Option<Decimal> {
    let mut magnitudes: Vec<Decimal> = history
        .iter()
        .filter(|v| !v.is_zero())
        .map(|v| v.abs())
        .collect();
    if magnitudes.len() < 2 {
        return None;
    }
    magnitudes.sort();
    Some(magnitudes[magnitudes.len() / 2])
}

// ---------------------------------------------------------------------------
// Top-level entry point
// ---------------------------------------------------------------------------

/// Runs every applicable identity over `current` (using `prior` for
/// cross-period identities and `comparatives`/`stored_prior` for the
/// comparative cross-check) and returns the combined [`ValidationReport`].
///
/// `prior` is the previously-stored fact set for the immediately preceding
/// period (for the cash-flow tie). `comparatives` is the prior-period column
/// read out of the *current* report, cross-checked against `stored_prior` (the
/// already-stored prior-period facts). All three are optional; identities and
/// cross-checks whose inputs are absent are simply not-applicable.
pub fn validate(
    current: &FactSet,
    prior: Option<&FactSet>,
    comparatives: Option<&FactSet>,
    stored_prior: Option<&FactSet>,
    tol: &Tolerance,
) -> ValidationReport {
    let identities = vec![
        balance_sheet(current, tol),
        free_cash_flow(current, tol),
        cash_flow_tie(current, prior, tol),
    ];
    let cross_checks = match (comparatives, stored_prior) {
        (Some(c), Some(s)) => cross_check_prior(c, s, tol),
        _ => Vec::new(),
    };
    ValidationReport::from_parts(identities, cross_checks)
}

/// Convenience wrapper for the common single-period case (no cross-period
/// inputs): runs only the same-period identities.
pub fn validate_period(current: &FactSet, tol: &Tolerance) -> ValidationReport {
    validate(current, None, None, None, tol)
}

#[cfg(test)]
mod tests;
