//! Deterministic evaluator for the DSL (ADR 0046).
//!
//! Evaluates an [`Expr`] over a [`MetricResolver`]. The resolver is the only
//! seam to the data: it resolves a metric key to a value (interpreting the
//! `_ttm` / `_avg` aggregation suffixes) and computes window functions over a
//! metric's period series. A missing input yields [`Value::Unavailable`] which
//! propagates — the evaluator never panics or errors on missing data.

use super::ast::{BinOp, Expr, Func, UnaryOp};
use rust_decimal::prelude::ToPrimitive;
use rust_decimal::Decimal;

/// The seam between the pure evaluator and the fundamentals data.
pub trait MetricResolver {
    /// The current-period value of `key` (may carry an `_ttm`/`_avg` suffix the
    /// resolver interprets). `None` when it cannot be computed (missing fact).
    fn value(&self, key: &str) -> Option<Decimal>;

    /// A window function over `key`'s period series. `ttm` ignores `n`.
    fn window(&self, func: Func, key: &str, n: i64) -> Option<Decimal>;
}

/// The result of evaluating a (sub)expression.
#[derive(Debug, Clone, PartialEq)]
pub enum Value {
    Num(Decimal),
    Bool(bool),
    /// A required input could not be computed. Distinct from a `false` verdict.
    Unavailable,
}

impl Value {
    pub fn as_num(&self) -> Option<Decimal> {
        match self {
            Value::Num(d) => Some(*d),
            _ => None,
        }
    }
}

pub fn eval(expr: &Expr, resolver: &dyn MetricResolver) -> Value {
    match expr {
        Expr::Number { value } => Value::Num(*value),
        Expr::Percent { value } => Value::Num(*value / Decimal::ONE_HUNDRED),
        Expr::Metric { key } => match resolver.value(key) {
            Some(d) => Value::Num(d),
            None => Value::Unavailable,
        },
        Expr::Call { func, args } => eval_call(*func, args, resolver),
        Expr::Unary { op, operand } => eval_unary(*op, operand, resolver),
        Expr::Binary { op, left, right } => eval_binary(*op, left, right, resolver),
    }
}

fn eval_call(func: Func, args: &[Expr], resolver: &dyn MetricResolver) -> Value {
    // coalesce takes full expressions: the first one that evaluates to an
    // available value wins; every-unavailable stays Unavailable (never 0).
    if func == Func::Coalesce {
        for arg in args {
            match eval(arg, resolver) {
                Value::Unavailable => continue,
                available => return available,
            }
        }
        return Value::Unavailable;
    }

    // First arg is always a metric key; cagr/avg/trend take a second arg `n`.
    let Some(Expr::Metric { key }) = args.first() else {
        return Value::Unavailable;
    };
    let n = match func {
        Func::Ttm => 0,
        _ => match args.get(1) {
            Some(e) => match eval(e, resolver) {
                Value::Num(d) => d.trunc().to_i64().unwrap_or(0),
                _ => return Value::Unavailable,
            },
            None => return Value::Unavailable,
        },
    };
    match resolver.window(func, key, n) {
        Some(d) => Value::Num(d),
        None => Value::Unavailable,
    }
}

fn eval_unary(op: UnaryOp, operand: &Expr, resolver: &dyn MetricResolver) -> Value {
    let v = eval(operand, resolver);
    match (op, v) {
        (UnaryOp::Neg, Value::Num(d)) => Value::Num(-d),
        (UnaryOp::Not, Value::Bool(b)) => Value::Bool(!b),
        _ => Value::Unavailable,
    }
}

fn eval_binary(op: BinOp, left: &Expr, right: &Expr, resolver: &dyn MetricResolver) -> Value {
    // Boolean operators short-circuit on a decisive operand so that
    // `false AND unavailable` is `false` and `true OR unavailable` is `true`.
    if op.is_boolean() {
        let l = eval(left, resolver);
        match (op, &l) {
            (BinOp::And, Value::Bool(false)) => return Value::Bool(false),
            (BinOp::Or, Value::Bool(true)) => return Value::Bool(true),
            _ => {}
        }
        let r = eval(right, resolver);
        return match (op, &l, &r) {
            (BinOp::And, _, Value::Bool(false)) => Value::Bool(false),
            (BinOp::Or, _, Value::Bool(true)) => Value::Bool(true),
            (BinOp::And, Value::Bool(a), Value::Bool(b)) => Value::Bool(*a && *b),
            (BinOp::Or, Value::Bool(a), Value::Bool(b)) => Value::Bool(*a || *b),
            _ => Value::Unavailable,
        };
    }

    let (Value::Num(a), Value::Num(b)) = (eval(left, resolver), eval(right, resolver)) else {
        return Value::Unavailable;
    };

    if op.is_comparison() {
        return Value::Bool(compare(op, a, b));
    }

    match op {
        BinOp::Add => Value::Num(a + b),
        BinOp::Sub => Value::Num(a - b),
        BinOp::Mul => Value::Num(a * b),
        BinOp::Div => {
            if b.is_zero() {
                Value::Unavailable
            } else {
                Value::Num(a / b)
            }
        }
        _ => Value::Unavailable,
    }
}

fn compare(op: BinOp, a: Decimal, b: Decimal) -> bool {
    match op {
        BinOp::Gte => a >= b,
        BinOp::Lte => a <= b,
        BinOp::Gt => a > b,
        BinOp::Lt => a < b,
        BinOp::Eq => a == b,
        // Approximately equal: within 1% of the larger magnitude (or exact at 0).
        BinOp::Approx => {
            let tol = a.abs().max(b.abs()) * Decimal::new(1, 2);
            (a - b).abs() <= tol
        }
        _ => false,
    }
}
