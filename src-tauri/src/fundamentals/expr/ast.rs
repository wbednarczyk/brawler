//! The criterion / formula expression AST (ADR 0046).
//!
//! One grammar serves two consumers: the arithmetic subset evaluates
//! `kpi_definitions.formula` into a metric value; the full grammar (adding
//! comparators and boolean logic) evaluates a quality-framework criterion into
//! a verdict. See `wiki/dsl-reference.md` for the user-facing rendering.

use rust_decimal::Decimal;
use serde::{Deserialize, Serialize};

/// A parsed expression node.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(tag = "node", rename_all = "snake_case")]
pub enum Expr {
    /// A plain number literal, e.g. `2.5`.
    Number { value: Decimal },
    /// A percent literal, e.g. `15%` — evaluates to the ratio `0.15`.
    Percent { value: Decimal },
    /// A bare metric key, e.g. `roic` or `total_equity_avg`.
    Metric { key: String },
    /// A function call, e.g. `cagr(revenue, 5)`.
    Call { func: Func, args: Vec<Expr> },
    /// A unary operation, e.g. `-x` or `NOT a`.
    Unary { op: UnaryOp, operand: Box<Expr> },
    /// A binary operation, e.g. `a + b` or `roic >= 15%`.
    Binary {
        op: BinOp,
        left: Box<Expr>,
        right: Box<Expr>,
    },
}

/// Window/aggregation functions over a metric's period series.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Func {
    /// `cagr(metric, n)` — compound annual growth rate over `n` years.
    Cagr,
    /// `ttm(metric)` — trailing-twelve-months sum (flow) or latest (stock).
    Ttm,
    /// `avg(metric, n)` — arithmetic mean over the last `n` periods.
    Avg,
    /// `trend(metric, n)` — signed slope over the last `n` periods (per period).
    Trend,
    /// `coalesce(a, b, ...)` — the first argument that evaluates to an
    /// available value; unavailable only when every argument is. Encodes
    /// ratio fallback recipes ("compute from whichever inputs exist") in a
    /// single formula (owner decision 2026-07-14).
    Coalesce,
}

impl Func {
    pub fn from_name(name: &str) -> Option<Self> {
        match name {
            "cagr" => Some(Func::Cagr),
            "ttm" => Some(Func::Ttm),
            "avg" => Some(Func::Avg),
            "trend" => Some(Func::Trend),
            "coalesce" => Some(Func::Coalesce),
            _ => None,
        }
    }

    pub fn name(self) -> &'static str {
        match self {
            Func::Coalesce => "coalesce",
            Func::Cagr => "cagr",
            Func::Ttm => "ttm",
            Func::Avg => "avg",
            Func::Trend => "trend",
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum UnaryOp {
    /// Arithmetic negation, `-x`.
    Neg,
    /// Boolean negation, `NOT a`.
    Not,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum BinOp {
    // Arithmetic
    Add,
    Sub,
    Mul,
    Div,
    // Comparison
    Gte,
    Lte,
    Gt,
    Lt,
    Eq,
    Approx,
    // Boolean
    And,
    Or,
}

impl BinOp {
    pub fn is_comparison(self) -> bool {
        matches!(
            self,
            BinOp::Gte | BinOp::Lte | BinOp::Gt | BinOp::Lt | BinOp::Eq | BinOp::Approx
        )
    }

    pub fn is_boolean(self) -> bool {
        matches!(self, BinOp::And | BinOp::Or)
    }
}
