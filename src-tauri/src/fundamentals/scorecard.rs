//! The quality-framework rule engine (ADR 0046, Decision 4/5).
//!
//! Evaluates a criterion expression against a [`MetricsContext`] into a verdict
//! plus the measured value, for one immutable scorecard run. Pure over the
//! metrics context; the storage layer persists the outcome.

use super::expr::{eval, leading_comparison, parse, Expr, Value};
use super::metrics::MetricsContext;
use rust_decimal::Decimal;
use serde::{Deserialize, Serialize};

/// The engine version recorded on every evaluation, so a re-run with changed
/// engine semantics is distinguishable in history.
pub const ENGINE_VERSION: &str = "qf-1";

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Verdict {
    Pass,
    Partial,
    Fail,
    Unavailable,
}

impl Verdict {
    pub fn as_str(self) -> &'static str {
        match self {
            Verdict::Pass => "pass",
            Verdict::Partial => "partial",
            Verdict::Fail => "fail",
            Verdict::Unavailable => "unavailable",
        }
    }
}

/// The outcome of one criterion in one run.
#[derive(Debug, Clone)]
pub struct CriterionOutcome {
    pub verdict: Verdict,
    /// Decimal-exact text of the leading metric's measured value, if any.
    pub measured_value: Option<String>,
    pub measured_unit: Option<String>,
    /// Decimal-exact text of the leading comparison's threshold, if any.
    pub threshold: Option<String>,
}

/// Evaluate one criterion. `partial_band`, when set and parseable, relaxes the
/// leading comparison's threshold: a `false` primary verdict becomes `partial`
/// when the same left-hand side satisfies the relaxed threshold.
pub fn evaluate_criterion(
    expression: &str,
    partial_band: Option<&str>,
    ctx: &MetricsContext,
) -> CriterionOutcome {
    let Ok(expr) = parse(expression) else {
        // A stored criterion that no longer parses is reported as unavailable
        // rather than crashing a run; the grammar-drift gate prevents shipping these.
        return CriterionOutcome {
            verdict: Verdict::Unavailable,
            measured_value: None,
            measured_unit: None,
            threshold: None,
        };
    };

    let resolver = ctx.resolver();
    let (measured_value, measured_unit, threshold) = measurement(&expr, ctx);

    let verdict = match eval(&expr, &resolver) {
        Value::Bool(true) => Verdict::Pass,
        Value::Unavailable => Verdict::Unavailable,
        Value::Bool(false) => partial_or_fail(&expr, partial_band, ctx),
        // A non-predicate expression is not a valid criterion; treat as unavailable.
        Value::Num(_) => Verdict::Unavailable,
    };

    CriterionOutcome {
        verdict,
        measured_value,
        measured_unit,
        threshold,
    }
}

/// Derive the measured value (leading comparison's LHS), its unit, and the
/// threshold (leading comparison's RHS) for display.
fn measurement(
    expr: &Expr,
    ctx: &MetricsContext,
) -> (Option<String>, Option<String>, Option<String>) {
    let resolver = ctx.resolver();
    let Some((lhs, _op, rhs)) = leading_comparison(expr) else {
        return (None, None, None);
    };
    let measured = eval(lhs, &resolver).as_num().map(format_decimal);
    let threshold = eval(rhs, &resolver).as_num().map(format_decimal);
    let unit = leading_metric_key(lhs).and_then(|k| ctx.unit_of(&k));
    (measured, unit, threshold)
}

/// Relax the leading comparison's threshold by `partial_band` and re-test.
fn partial_or_fail(expr: &Expr, partial_band: Option<&str>, ctx: &MetricsContext) -> Verdict {
    let Some(band) = partial_band else {
        return Verdict::Fail;
    };
    let (Some((lhs, op, _rhs)), Ok(band_expr)) = (leading_comparison(expr), parse(band)) else {
        return Verdict::Fail;
    };
    if !op.is_comparison() {
        return Verdict::Fail;
    }
    let relaxed = Expr::Binary {
        op,
        left: Box::new(lhs.clone()),
        right: Box::new(band_expr),
    };
    match eval(&relaxed, &ctx.resolver()) {
        Value::Bool(true) => Verdict::Partial,
        _ => Verdict::Fail,
    }
}

/// The first metric key in a left-to-right walk (for the leading metric's unit).
fn leading_metric_key(expr: &Expr) -> Option<String> {
    match expr {
        Expr::Metric { key } => Some(key.clone()),
        Expr::Call { args, .. } => args.iter().find_map(leading_metric_key),
        Expr::Unary { operand, .. } => leading_metric_key(operand),
        Expr::Binary { left, right, .. } => {
            leading_metric_key(left).or_else(|| leading_metric_key(right))
        }
        _ => None,
    }
}

fn format_decimal(d: Decimal) -> String {
    d.normalize().to_string()
}

/// Roll up per-criterion verdicts into the evaluation summary counts.
#[derive(Debug, Clone, Copy, Default)]
pub struct VerdictCounts {
    pub pass: i64,
    pub partial: i64,
    pub fail: i64,
    pub unavailable: i64,
}

impl VerdictCounts {
    pub fn tally(verdicts: impl IntoIterator<Item = Verdict>) -> Self {
        let mut c = Self::default();
        for v in verdicts {
            match v {
                Verdict::Pass => c.pass += 1,
                Verdict::Partial => c.partial += 1,
                Verdict::Fail => c.fail += 1,
                Verdict::Unavailable => c.unavailable += 1,
            }
        }
        c
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::fundamentals::metrics::{Computation, MetricDef, MetricsContext, PeriodFacts};
    use std::collections::HashMap;

    fn ctx(facts: &[(&str, i64)]) -> MetricsContext {
        let mut defs = HashMap::new();
        for (k, _) in facts {
            defs.insert(
                (*k).to_owned(),
                MetricDef {
                    computation: Computation::Reported,
                    formula: None,
                    value_kind: "ratio".to_owned(),
                    unit: None,
                    period_nature: "duration".to_owned(),
                },
            );
        }
        let period = PeriodFacts {
            period_id: "p1".to_owned(),
            fiscal_year: 2025,
            period_type: "FY".to_owned(),
            reported: facts
                .iter()
                .map(|(k, v)| ((*k).to_owned(), Decimal::new(*v, 2)))
                .collect(),
        };
        MetricsContext::new(defs, vec![period])
    }

    #[test]
    fn pass_and_fail() {
        let c = ctx(&[("roic", 18)]); // 0.18
        assert_eq!(
            evaluate_criterion("roic >= 15%", None, &c).verdict,
            Verdict::Pass
        );
        assert_eq!(
            evaluate_criterion("roic >= 20%", None, &c).verdict,
            Verdict::Fail
        );
    }

    #[test]
    fn unavailable_when_metric_missing() {
        let c = ctx(&[]);
        assert_eq!(
            evaluate_criterion("roic >= 15%", None, &c).verdict,
            Verdict::Unavailable
        );
    }

    #[test]
    fn partial_band_relaxes_threshold() {
        let c = ctx(&[("roic", 13)]); // 0.13 — fails 15% but within a 12% band
        let outcome = evaluate_criterion("roic >= 15%", Some("12%"), &c);
        assert_eq!(outcome.verdict, Verdict::Partial);
        // Below the band → fail.
        let c2 = ctx(&[("roic", 10)]);
        assert_eq!(
            evaluate_criterion("roic >= 15%", Some("12%"), &c2).verdict,
            Verdict::Fail
        );
    }

    #[test]
    fn measured_value_and_threshold_recorded() {
        let c = ctx(&[("roic", 18)]);
        let outcome = evaluate_criterion("roic >= 15%", None, &c);
        assert_eq!(outcome.measured_value.as_deref(), Some("0.18"));
        assert_eq!(outcome.threshold.as_deref(), Some("0.15"));
    }

    #[test]
    fn counts_tally() {
        let counts = VerdictCounts::tally([
            Verdict::Pass,
            Verdict::Pass,
            Verdict::Fail,
            Verdict::Unavailable,
        ]);
        assert_eq!(counts.pass, 2);
        assert_eq!(counts.fail, 1);
        assert_eq!(counts.unavailable, 1);
    }

    /// Golden snapshot of the financial scoring derivation (ADR 0049, T2). Locks
    /// the whole `CriterionOutcome` shape — verdict, decimal-exact measured value,
    /// unit, threshold — across the representative bands (pass / fail / partial /
    /// unavailable), so a change to the scoring normalization is a reviewable diff.
    #[test]
    fn criterion_outcomes_are_stable() {
        let c = ctx(&[("roic", 18)]); // 0.18
        let partial_ctx = ctx(&[("roic", 13)]); // within a relaxed band
        let missing = ctx(&[]);

        let outcomes = vec![
            ("pass", evaluate_criterion("roic >= 15%", None, &c)),
            ("fail", evaluate_criterion("roic >= 20%", None, &c)),
            (
                "partial",
                evaluate_criterion("roic >= 15%", Some("12%"), &partial_ctx),
            ),
            (
                "unavailable",
                evaluate_criterion("roic >= 15%", None, &missing),
            ),
        ];

        insta::assert_debug_snapshot!("criterion_outcomes", outcomes);
    }
}
