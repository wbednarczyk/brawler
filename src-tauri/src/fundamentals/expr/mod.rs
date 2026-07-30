//! The quality-framework / metric-formula expression engine (ADR 0046).
//!
//! One grammar, two consumers: the arithmetic subset evaluates
//! `kpi_definitions.formula` into a metric value (the metrics service); the
//! full grammar — adding comparators and boolean logic — evaluates a
//! framework criterion into a verdict (the rule engine). The user-facing
//! rendering of this grammar is `wiki/dsl-reference.md`.

mod ast;
mod eval;
mod lexer;
mod parser;

pub use ast::{BinOp, Expr, Func, UnaryOp};
pub use eval::{eval, MetricResolver, Value};
pub use parser::parse;

use std::collections::BTreeSet;
use std::fmt;

/// A lex/parse error, surfaced to the criteria editor as a human-readable message.
#[derive(Debug, Clone, PartialEq)]
pub enum ExprError {
    Lex(String),
    Parse(String),
}

impl fmt::Display for ExprError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            ExprError::Lex(msg) => write!(f, "syntax error: {msg}"),
            ExprError::Parse(msg) => write!(f, "parse error: {msg}"),
        }
    }
}

impl std::error::Error for ExprError {}

/// Distinct metric keys referenced anywhere in `expr` (sorted, deduped). Drives
/// the editor's "this criterion uses …" affordance and the export dependency walk.
pub fn referenced_metrics(expr: &Expr) -> Vec<String> {
    let mut keys = BTreeSet::new();
    collect_metrics(expr, &mut keys);
    keys.into_iter().collect()
}

fn collect_metrics(expr: &Expr, out: &mut BTreeSet<String>) {
    match expr {
        Expr::Metric { key } => {
            out.insert(key.clone());
        }
        Expr::Call { args, .. } => {
            for a in args {
                collect_metrics(a, out);
            }
        }
        Expr::Unary { operand, .. } => collect_metrics(operand, out),
        Expr::Binary { left, right, .. } => {
            collect_metrics(left, out);
            collect_metrics(right, out);
        }
        Expr::Number { .. } | Expr::Percent { .. } => {}
    }
}

/// Whether `expr`'s top level is a predicate (a comparison or boolean), i.e. a
/// valid *criterion*. A bare arithmetic expression is a valid *formula* but not
/// a criterion; `validate_criterion_expression` rejects the latter.
pub fn is_predicate(expr: &Expr) -> bool {
    match expr {
        Expr::Binary { op, .. } => op.is_comparison() || op.is_boolean(),
        Expr::Unary {
            op: UnaryOp::Not, ..
        } => true,
        _ => false,
    }
}

/// The first comparison node in a left-to-right walk: `(lhs, op, rhs)`. Used to
/// derive a criterion's measured value and to relax the threshold for a
/// partial-band verdict. `None` for a criterion with no comparison.
pub fn leading_comparison(expr: &Expr) -> Option<(&Expr, BinOp, &Expr)> {
    match expr {
        Expr::Binary { op, left, right } if op.is_comparison() => Some((left, *op, right)),
        Expr::Binary { left, right, .. } => {
            leading_comparison(left).or_else(|| leading_comparison(right))
        }
        Expr::Unary { operand, .. } => leading_comparison(operand),
        _ => None,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use rust_decimal::Decimal;
    use std::collections::HashMap;

    /// A simple map-backed resolver for evaluator unit tests.
    struct MapResolver {
        values: HashMap<String, Decimal>,
        windows: HashMap<(String, String, i64), Decimal>,
    }

    impl MapResolver {
        fn new() -> Self {
            Self {
                values: HashMap::new(),
                windows: HashMap::new(),
            }
        }
        fn with(mut self, key: &str, value: i64) -> Self {
            self.values.insert(key.to_owned(), Decimal::new(value, 0));
            self
        }
        fn with_dec(mut self, key: &str, value: Decimal) -> Self {
            self.values.insert(key.to_owned(), value);
            self
        }
        fn with_window(mut self, func: Func, key: &str, n: i64, value: Decimal) -> Self {
            self.windows
                .insert((func.name().to_owned(), key.to_owned(), n), value);
            self
        }
    }

    impl MetricResolver for MapResolver {
        fn value(&self, key: &str) -> Option<Decimal> {
            self.values.get(key).copied()
        }
        fn window(&self, func: Func, key: &str, n: i64) -> Option<Decimal> {
            self.windows
                .get(&(func.name().to_owned(), key.to_owned(), n))
                .copied()
        }
    }

    fn eval_str(input: &str, resolver: &dyn MetricResolver) -> Value {
        eval(&parse(input).expect("parse"), resolver)
    }

    #[test]
    fn parses_and_evaluates_simple_comparison() {
        let r = MapResolver::new().with_dec("roic", Decimal::new(182, 3)); // 0.182
        assert_eq!(eval_str("roic >= 15%", &r), Value::Bool(true));
        assert_eq!(eval_str("roic >= 20%", &r), Value::Bool(false));
    }

    #[test]
    fn percent_literal_is_a_ratio() {
        let r = MapResolver::new();
        assert_eq!(
            eval(&parse("15%").unwrap(), &r),
            Value::Num(Decimal::new(15, 2))
        );
    }

    #[test]
    fn arithmetic_precedence_and_parens() {
        let r = MapResolver::new();
        assert_eq!(eval_str("2 + 3 * 4", &r), Value::Num(Decimal::new(14, 0)));
        assert_eq!(eval_str("(2 + 3) * 4", &r), Value::Num(Decimal::new(20, 0)));
    }

    #[test]
    fn boolean_composition() {
        let r = MapResolver::new()
            .with_dec("roic", Decimal::new(18, 2))
            .with("fcf", 5)
            .with_dec("net_debt_to_ebitda", Decimal::new(14, 1));
        assert_eq!(
            eval_str("net_debt_to_ebitda < 2.5 AND fcf > 0", &r),
            Value::Bool(true)
        );
        assert_eq!(eval_str("roic >= 25% OR fcf > 0", &r), Value::Bool(true));
        assert_eq!(eval_str("NOT fcf > 0", &r), Value::Bool(false));
    }

    #[test]
    fn missing_metric_is_unavailable_not_error() {
        let r = MapResolver::new();
        assert_eq!(eval_str("roic >= 15%", &r), Value::Unavailable);
        // Short-circuit still resolves a decisive boolean operand.
        let r2 = MapResolver::new().with("fcf", -1);
        assert_eq!(eval_str("fcf > 0 AND roic >= 15%", &r2), Value::Bool(false));
    }

    #[test]
    fn divide_by_zero_is_unavailable() {
        let r = MapResolver::new().with("a", 5).with("b", 0);
        assert_eq!(eval_str("a / b > 1", &r), Value::Unavailable);
    }

    #[test]
    fn window_function_cagr() {
        let r = MapResolver::new().with_window(Func::Cagr, "revenue", 5, Decimal::new(12, 2));
        assert_eq!(eval_str("cagr(revenue, 5) > 10%", &r), Value::Bool(true));
    }

    #[test]
    fn referenced_metrics_are_collected() {
        let expr = parse("net_debt_to_ebitda < 2.5 AND cagr(revenue, 5) > 10%").unwrap();
        assert_eq!(
            referenced_metrics(&expr),
            vec!["net_debt_to_ebitda".to_owned(), "revenue".to_owned()]
        );
    }

    #[test]
    fn predicate_detection() {
        assert!(is_predicate(&parse("roic >= 15%").unwrap()));
        assert!(is_predicate(&parse("a > 1 AND b < 2").unwrap()));
        assert!(!is_predicate(&parse("a + b").unwrap()));
        assert!(!is_predicate(&parse("roic").unwrap()));
    }

    #[test]
    fn leading_comparison_for_measured_value() {
        let expr = parse("roic >= 15% AND fcf > 0").unwrap();
        let (lhs, op, _rhs) = leading_comparison(&expr).expect("comparison");
        assert_eq!(op, BinOp::Gte);
        assert_eq!(
            lhs,
            &Expr::Metric {
                key: "roic".to_owned()
            }
        );
    }

    #[test]
    fn empty_and_garbage_inputs_error() {
        assert!(parse("").is_err());
        assert!(parse("roic >=").is_err());
        assert!(parse("@@@").is_err());
        assert!(parse("foo(revenue)").is_err()); // unknown function
    }

    #[test]
    fn covers_all_comparison_operators() {
        let r = MapResolver::new().with("a", 5).with("b", 5).with("c", 7);
        assert_eq!(eval_str("a <= b", &r), Value::Bool(true));
        assert_eq!(eval_str("a < c", &r), Value::Bool(true));
        assert_eq!(eval_str("a == b", &r), Value::Bool(true));
        assert_eq!(eval_str("a = b", &r), Value::Bool(true)); // single '=' ergonomic alias
        assert_eq!(eval_str("a == c", &r), Value::Bool(false));
        assert_eq!(eval_str("c > a", &r), Value::Bool(true));
    }

    #[test]
    fn approx_operator_uses_one_percent_tolerance() {
        let r = MapResolver::new();
        // |100 - 100.5| = 0.5 <= 1% of 100.5 (1.005) -> within tolerance.
        assert_eq!(eval_str("100 ~= 100.5", &r), Value::Bool(true));
        // |100 - 102| = 2 > 1% of 102 (1.02) -> outside tolerance.
        assert_eq!(eval_str("100 ~= 102", &r), Value::Bool(false));
        assert_eq!(eval_str("0 ~= 0", &r), Value::Bool(true));
    }

    #[test]
    fn unary_negation_and_subtraction_and_division() {
        let r = MapResolver::new().with("roic", 3);
        assert_eq!(eval_str("-5", &r), Value::Num(Decimal::new(-5, 0)));
        assert_eq!(eval_str("-roic < 0", &r), Value::Bool(true));
        assert_eq!(eval_str("10 - 3", &r), Value::Num(Decimal::new(7, 0)));
        assert_eq!(eval_str("10 / 4", &r), Value::Num(Decimal::new(25, 1)));
    }

    #[test]
    fn percent_participates_in_arithmetic() {
        let r = MapResolver::new().with("revenue", 200);
        // 200 * 0.50 == 100; compared rather than asserted by scale.
        assert_eq!(eval_str("revenue * 50% == 100", &r), Value::Bool(true));
    }

    #[test]
    fn ttm_avg_and_trend_window_functions() {
        let r = MapResolver::new()
            .with_window(Func::Ttm, "revenue", 0, Decimal::new(150, 0))
            .with_window(Func::Avg, "eps", 4, Decimal::new(12, 1)) // 1.2
            .with_window(Func::Trend, "revenue", 3, Decimal::new(5, 0));
        assert_eq!(eval_str("ttm(revenue) > 100", &r), Value::Bool(true));
        assert_eq!(eval_str("avg(eps, 4) > 1", &r), Value::Bool(true));
        assert_eq!(eval_str("trend(revenue, 3) > 0", &r), Value::Bool(true));
    }

    #[test]
    fn unavailable_propagates_through_arithmetic_and_comparison() {
        let r = MapResolver::new();
        assert_eq!(eval_str("missing + 1 > 0", &r), Value::Unavailable);
        assert_eq!(eval_str("missing > 1", &r), Value::Unavailable);
    }

    #[test]
    fn or_short_circuits_to_true_on_unavailable_operand() {
        // fcf > 0 is decisively true, so the unavailable right operand never
        // turns the verdict into Unavailable.
        let r = MapResolver::new().with("fcf", 1);
        assert_eq!(eval_str("fcf > 0 OR roic >= 15%", &r), Value::Bool(true));
    }

    #[test]
    fn type_mismatches_are_unavailable_not_panics() {
        let r = MapResolver::new().with("fcf", 1);
        assert_eq!(eval_str("NOT 5", &r), Value::Unavailable); // NOT on a number
        assert_eq!(eval_str("5 AND fcf > 0", &r), Value::Unavailable); // AND on a number
                                                                       // A window function whose first argument is not a metric key.
        assert_eq!(eval_str("cagr(2 + 3, 5) > 1", &r), Value::Unavailable);
    }

    #[test]
    fn boolean_mixed_operand_combinations() {
        // AND/OR with one decisive and one opposite operand. (The && / ||
        // swaps in eval_binary's Kleene arms are equivalent mutants — the
        // decisive-operand guard arms above them make the only reachable
        // inputs degenerate — and are excluded in .cargo/mutants.toml.)
        // fcf > 0 is true; roic > 100% is false.
        let r = MapResolver::new()
            .with("fcf", 1)
            .with_dec("roic", Decimal::new(18, 2)); // 0.18
        assert_eq!(eval_str("fcf > 0 AND roic > 100%", &r), Value::Bool(false));
        assert_eq!(eval_str("roic > 100% OR fcf > 0", &r), Value::Bool(true));
    }

    #[test]
    fn referenced_metrics_dedupes_repeated_keys() {
        let expr = parse("roic + roic > roic").unwrap();
        assert_eq!(referenced_metrics(&expr), vec!["roic".to_owned()]);
    }

    #[test]
    fn single_equals_tokenizes_a_directly_adjacent_operand() {
        // The ergonomic single-'=' path must look at the NEXT character, not
        // the current one: a spaceless `roe=5` keeps its operand.
        assert_eq!(
            lexer::tokenize("roe=5").expect("tokenize"),
            vec![
                lexer::Token::Ident("roe".to_owned()),
                lexer::Token::Eq,
                lexer::Token::Number(Decimal::new(5, 0)),
            ]
        );
    }

    #[test]
    fn two_char_comparators_tokenize_off_the_coincidence_index() {
        // `<=` at char index 2 cannot distinguish `i += 2` from `i *= 2`;
        // `roe <= 5` puts the operator at index 4 where they diverge.
        assert_eq!(
            lexer::tokenize("roe <= 5").expect("tokenize"),
            vec![
                lexer::Token::Ident("roe".to_owned()),
                lexer::Token::Lte,
                lexer::Token::Number(Decimal::new(5, 0)),
            ]
        );
        assert_eq!(
            lexer::tokenize("roe >= 5").expect("tokenize"),
            vec![
                lexer::Token::Ident("roe".to_owned()),
                lexer::Token::Gte,
                lexer::Token::Number(Decimal::new(5, 0)),
            ]
        );
    }

    #[test]
    fn bare_boolean_keyword_as_value_is_a_named_parse_error() {
        // A bare `and`/`or` in value position must fail with the dedicated
        // message — not silently parse as a metric identifier (which would
        // surface as Unavailable at eval time instead of an editor error).
        for input in ["revenue > and", "revenue > or"] {
            let error = parse(input).expect_err("bare boolean keyword must not parse");
            assert!(
                error
                    .to_string()
                    .contains("is a binary operator, not a value"),
                "unexpected error for {input:?}: {error}"
            );
        }
    }

    #[test]
    fn func_names_are_stable_and_round_trip() {
        // Func::name feeds error rendering and the resolver's window keys —
        // pin the exact strings and the from_name round-trip.
        for (func, name) in [
            (Func::Cagr, "cagr"),
            (Func::Ttm, "ttm"),
            (Func::Avg, "avg"),
            (Func::Trend, "trend"),
            (Func::Coalesce, "coalesce"),
        ] {
            assert_eq!(func.name(), name);
            assert_eq!(Func::from_name(name), Some(func));
        }
    }

    #[test]
    fn expr_error_display_carries_the_message() {
        assert_eq!(
            ExprError::Lex("bad char".to_owned()).to_string(),
            "syntax error: bad char"
        );
        assert_eq!(
            ExprError::Parse("boom".to_owned()).to_string(),
            "parse error: boom"
        );
    }

    mod properties {
        use super::*;
        use proptest::prelude::*;

        proptest! {
            // The lexer/parser must never panic on arbitrary input — it only ever
            // returns Ok(expr) or Err(ExprError). This is the criteria editor's
            // contract: any string a user types is handled gracefully.
            #[test]
            fn parse_never_panics(input in ".*") {
                let _ = parse(&input);
            }

            // Any comparison of two numeric literals parses and evaluates to a
            // Bool (never Unavailable or a panic), across every comparison op.
            #[test]
            fn numeric_comparison_evaluates_to_bool(
                a in -100_000i64..100_000,
                b in -100_000i64..100_000,
                op in prop::sample::select(vec![">", "<", ">=", "<=", "==", "~="]),
            ) {
                let resolver = MapResolver::new();
                let expr = parse(&format!("{a} {op} {b}")).expect("generated comparison parses");
                prop_assert!(matches!(eval(&expr, &resolver), Value::Bool(_)));
            }
        }
    }
}
