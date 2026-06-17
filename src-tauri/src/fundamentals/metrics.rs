//! Shared derived-metrics service (ADR 0046, Decision 2).
//!
//! Builds an in-memory, never-persisted view of every metric value for one
//! company across its period series, by evaluating `kpi_definitions.formula`
//! with the shared expression engine. It is catalog-driven: it computes from
//! whatever definitions exist, across all scopes, so adding a definition row
//! (including a global `user`-scope custom metric) extends what is computable
//! with no code change. Reused by quality frameworks now and by cross-company
//! comparison (v0.53) and the valuation engine (v0.54).
//!
//! Pure: it takes loaded data, not a live connection, so it is unit-testable.
//! A missing or uncomputable input yields `None` (→ `Unavailable` at the engine
//! boundary); the service never invents a value.

use super::expr::{eval, parse, Expr, Func, MetricResolver, Value};
use rust_decimal::prelude::*;
use rust_decimal::Decimal;
use std::cell::RefCell;
use std::collections::{HashMap, HashSet};

/// A catalog definition, with its formula pre-parsed (derived metrics only).
#[derive(Debug, Clone)]
pub struct MetricDef {
    pub computation: Computation,
    pub formula: Option<Expr>,
    pub value_kind: String,
    pub unit: Option<String>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Computation {
    Reported,
    Derived,
}

/// Reported facts for one period. `index 0` is the latest period.
#[derive(Debug, Clone)]
pub struct PeriodFacts {
    pub period_id: String,
    pub fiscal_year: i64,
    /// True for an annual (`FY`) period; drives TTM degradation.
    pub is_annual: bool,
    pub reported: HashMap<String, Decimal>,
}

/// The loaded, immutable view the metrics service computes over.
#[derive(Debug, Clone)]
pub struct MetricsContext {
    definitions: HashMap<String, MetricDef>,
    /// Periods newest-first.
    periods: Vec<PeriodFacts>,
}

impl MetricsContext {
    pub fn new(definitions: HashMap<String, MetricDef>, periods: Vec<PeriodFacts>) -> Self {
        Self {
            definitions,
            periods,
        }
    }

    pub fn latest_period_id(&self) -> Option<&str> {
        self.periods.first().map(|p| p.period_id.as_str())
    }

    pub fn has_periods(&self) -> bool {
        !self.periods.is_empty()
    }

    /// The display unit for a metric key (suffix-stripped), if known.
    pub fn unit_of(&self, key: &str) -> Option<String> {
        let base = strip_suffix(key).0;
        self.definitions.get(base).and_then(|d| d.unit.clone())
    }

    /// The value kind (`monetary`/`percentage`/`ratio`/…) for a metric key.
    pub fn kind_of(&self, key: &str) -> Option<String> {
        let base = strip_suffix(key).0;
        self.definitions.get(base).map(|d| d.value_kind.clone())
    }

    /// All computable metric keys (catalog keys; reported + derived).
    pub fn known_keys(&self) -> Vec<String> {
        let mut keys: Vec<String> = self.definitions.keys().cloned().collect();
        keys.sort();
        keys
    }

    /// A resolver bound to the latest period — the entry point the rule engine evaluates against.
    pub fn resolver(&self) -> ContextResolver<'_> {
        ContextResolver {
            ctx: self,
            idx: 0,
            visiting: RefCell::new(HashSet::new()),
        }
    }

    // ---- period-parameterized resolution -------------------------------------

    fn value_at(
        &self,
        key: &str,
        idx: usize,
        visiting: &RefCell<HashSet<(String, usize)>>,
    ) -> Option<Decimal> {
        let (base, suffix) = strip_suffix(key);
        match suffix {
            Suffix::Ttm => return self.ttm_at(base, idx, visiting),
            Suffix::Avg => return self.avg2_at(base, idx, visiting),
            Suffix::None => {}
        }

        let period = self.periods.get(idx)?;

        // Reported fact takes precedence.
        if let Some(v) = period.reported.get(base) {
            return Some(*v);
        }

        // Derived: evaluate the formula at this period, guarding against cycles.
        let def = self.definitions.get(base)?;
        if def.computation != Computation::Derived {
            return None;
        }
        let formula = def.formula.as_ref()?;
        let visit_key = (base.to_owned(), idx);
        if visiting.borrow().contains(&visit_key) {
            return None; // cycle
        }
        visiting.borrow_mut().insert(visit_key.clone());
        let resolver = ContextResolver {
            ctx: self,
            idx,
            visiting: RefCell::new(visiting.borrow().clone()),
        };
        let result = match eval(formula, &resolver) {
            Value::Num(d) => Some(d),
            _ => None,
        };
        visiting.borrow_mut().remove(&visit_key);
        result
    }

    fn window_at(
        &self,
        func: Func,
        key: &str,
        n: i64,
        idx: usize,
        visiting: &RefCell<HashSet<(String, usize)>>,
    ) -> Option<Decimal> {
        match func {
            Func::Ttm => self.ttm_at(key, idx, visiting),
            Func::Avg => {
                let n = n.max(1) as usize;
                let mut sum = Decimal::ZERO;
                let mut count = 0usize;
                for k in 0..n {
                    if let Some(v) = self.value_at(key, idx + k, visiting) {
                        sum += v;
                        count += 1;
                    }
                }
                if count == 0 {
                    None
                } else {
                    Some(sum / Decimal::from(count as i64))
                }
            }
            Func::Trend => {
                let n = n.max(2) as usize;
                let newest = self.value_at(key, idx, visiting)?;
                let oldest = self.value_at(key, idx + n - 1, visiting)?;
                Some((newest - oldest) / Decimal::from((n - 1) as i64))
            }
            Func::Cagr => self.cagr_at(key, n, idx, visiting),
        }
    }

    /// TTM: for an annual period, the value itself; otherwise the sum of the four
    /// most recent periods, degrading to the latest value when fewer exist.
    fn ttm_at(
        &self,
        key: &str,
        idx: usize,
        visiting: &RefCell<HashSet<(String, usize)>>,
    ) -> Option<Decimal> {
        let period = self.periods.get(idx)?;
        if period.is_annual {
            return self.value_at(key, idx, visiting);
        }
        let mut sum = Decimal::ZERO;
        let mut count = 0usize;
        for k in 0..4 {
            if let Some(v) = self.value_at(key, idx + k, visiting) {
                sum += v;
                count += 1;
            }
        }
        if count == 0 {
            None
        } else if count < 4 {
            // Not enough quarters for a true TTM; degrade to the latest value.
            self.value_at(key, idx, visiting)
        } else {
            Some(sum)
        }
    }

    /// Two-point average of the current and prior period (balance-sheet averaging),
    /// degrading to the single value when no prior period exists.
    fn avg2_at(
        &self,
        key: &str,
        idx: usize,
        visiting: &RefCell<HashSet<(String, usize)>>,
    ) -> Option<Decimal> {
        let current = self.value_at(key, idx, visiting)?;
        match self.value_at(key, idx + 1, visiting) {
            Some(prior) => Some((current + prior) / Decimal::TWO),
            None => Some(current),
        }
    }

    /// CAGR over `n` years: `(end / begin)^(1/n) - 1`. The fractional root uses a
    /// floating intermediate (a growth-rate threshold does not need decimal-exactness).
    fn cagr_at(
        &self,
        key: &str,
        n: i64,
        idx: usize,
        visiting: &RefCell<HashSet<(String, usize)>>,
    ) -> Option<Decimal> {
        if n <= 0 {
            return None;
        }
        let end = self.value_at(key, idx, visiting)?;
        let target_year = self.periods.get(idx)?.fiscal_year - n;
        let begin_idx = self
            .periods
            .iter()
            .position(|p| p.fiscal_year == target_year)?;
        let begin = self.value_at(key, begin_idx, visiting)?;
        if begin <= Decimal::ZERO || end <= Decimal::ZERO {
            return None;
        }
        let ratio = (end / begin).to_f64()?;
        let cagr = ratio.powf(1.0 / n as f64) - 1.0;
        Decimal::from_f64(cagr)
    }
}

/// A [`MetricResolver`] bound to one period of a [`MetricsContext`].
pub struct ContextResolver<'a> {
    ctx: &'a MetricsContext,
    idx: usize,
    visiting: RefCell<HashSet<(String, usize)>>,
}

impl MetricResolver for ContextResolver<'_> {
    fn value(&self, key: &str) -> Option<Decimal> {
        self.ctx.value_at(key, self.idx, &self.visiting)
    }
    fn window(&self, func: Func, key: &str, n: i64) -> Option<Decimal> {
        self.ctx.window_at(func, key, n, self.idx, &self.visiting)
    }
}

#[derive(PartialEq, Clone, Copy)]
enum Suffix {
    None,
    Ttm,
    Avg,
}

/// Split a `_ttm` / `_avg` aggregation suffix off a metric key.
fn strip_suffix(key: &str) -> (&str, Suffix) {
    if let Some(base) = key.strip_suffix("_ttm") {
        (base, Suffix::Ttm)
    } else if let Some(base) = key.strip_suffix("_avg") {
        (base, Suffix::Avg)
    } else {
        (key, Suffix::None)
    }
}

/// Parse a definition's formula text into an `Expr` (derived metrics only).
/// A formula that fails to parse is treated as having no formula (the metric is
/// then uncomputable rather than crashing the service); the grammar-drift gate
/// keeps shipped formulas parseable.
pub fn parse_formula(formula: &str) -> Option<Expr> {
    parse(formula).ok()
}

#[cfg(test)]
mod tests {
    use super::*;

    fn dec(n: i64, scale: u32) -> Decimal {
        Decimal::new(n, scale)
    }

    fn def_reported(kind: &str) -> MetricDef {
        MetricDef {
            computation: Computation::Reported,
            formula: None,
            value_kind: kind.to_owned(),
            unit: None,
        }
    }

    fn def_derived(kind: &str, formula: &str) -> MetricDef {
        MetricDef {
            computation: Computation::Derived,
            formula: parse_formula(formula),
            value_kind: kind.to_owned(),
            unit: None,
        }
    }

    fn ctx_with(periods: Vec<PeriodFacts>) -> MetricsContext {
        let mut defs = HashMap::new();
        defs.insert("revenue".to_owned(), def_reported("monetary"));
        defs.insert("gross_profit".to_owned(), def_reported("monetary"));
        defs.insert("operating_cash_flow".to_owned(), def_reported("monetary"));
        defs.insert("capex".to_owned(), def_reported("monetary"));
        defs.insert("net_profit".to_owned(), def_reported("monetary"));
        defs.insert("total_equity".to_owned(), def_reported("monetary"));
        defs.insert(
            "gross_margin".to_owned(),
            def_derived("percentage", "gross_profit / revenue"),
        );
        defs.insert(
            "free_cash_flow".to_owned(),
            def_derived("monetary", "operating_cash_flow - capex"),
        );
        defs.insert(
            "fcf_conversion".to_owned(),
            def_derived("percentage", "free_cash_flow / net_profit"),
        );
        defs.insert(
            "roe".to_owned(),
            def_derived("percentage", "net_profit_ttm / total_equity_avg"),
        );
        MetricsContext::new(defs, periods)
    }

    fn period(year: i64, facts: &[(&str, Decimal)]) -> PeriodFacts {
        PeriodFacts {
            period_id: format!("p{year}"),
            fiscal_year: year,
            is_annual: true,
            reported: facts.iter().map(|(k, v)| (k.to_string(), *v)).collect(),
        }
    }

    #[test]
    fn reported_pass_through() {
        let ctx = ctx_with(vec![period(2025, &[("revenue", dec(1000, 0))])]);
        assert_eq!(ctx.resolver().value("revenue"), Some(dec(1000, 0)));
    }

    #[test]
    fn derived_simple_formula() {
        let ctx = ctx_with(vec![period(
            2025,
            &[("gross_profit", dec(350, 0)), ("revenue", dec(1000, 0))],
        )]);
        assert_eq!(ctx.resolver().value("gross_margin"), Some(dec(35, 2)));
    }

    #[test]
    fn derived_on_derived_topological() {
        // fcf_conversion -> free_cash_flow -> (ocf - capex)
        let ctx = ctx_with(vec![period(
            2025,
            &[
                ("operating_cash_flow", dec(120, 0)),
                ("capex", dec(20, 0)),
                ("net_profit", dec(100, 0)),
            ],
        )]);
        assert_eq!(ctx.resolver().value("free_cash_flow"), Some(dec(100, 0)));
        assert_eq!(ctx.resolver().value("fcf_conversion"), Some(dec(1, 0)));
    }

    #[test]
    fn missing_input_is_none() {
        let ctx = ctx_with(vec![period(2025, &[("revenue", dec(1000, 0))])]);
        // gross_profit missing → gross_margin uncomputable
        assert_eq!(ctx.resolver().value("gross_margin"), None);
    }

    #[test]
    fn ttm_and_avg_suffixes_degrade_on_annual_single_period() {
        let ctx = ctx_with(vec![period(
            2025,
            &[("net_profit", dec(100, 0)), ("total_equity", dec(500, 0))],
        )]);
        // roe = net_profit_ttm / total_equity_avg → 100 / 500 = 0.2
        assert_eq!(ctx.resolver().value("roe"), Some(dec(2, 1)));
    }

    #[test]
    fn avg_uses_two_periods_when_available() {
        let ctx = ctx_with(vec![
            period(
                2025,
                &[("net_profit", dec(120, 0)), ("total_equity", dec(600, 0))],
            ),
            period(
                2024,
                &[("net_profit", dec(80, 0)), ("total_equity", dec(400, 0))],
            ),
        ]);
        // total_equity_avg = (600 + 400)/2 = 500; net_profit_ttm (annual) = 120; roe = 0.24
        assert_eq!(ctx.resolver().value("roe"), Some(dec(24, 2)));
    }

    #[test]
    fn cagr_over_period_series() {
        let ctx = ctx_with(vec![
            period(2025, &[("revenue", dec(1610, 0))]),
            period(2024, &[("revenue", dec(1400, 0))]),
            period(2023, &[("revenue", dec(1200, 0))]),
            period(2020, &[("revenue", dec(1000, 0))]),
        ]);
        // cagr(revenue, 5): (1610/1000)^(1/5) - 1 ≈ 0.0999 → > 9%
        let v = ctx.resolver().window(Func::Cagr, "revenue", 5).unwrap();
        assert!(v > dec(9, 2) && v < dec(11, 2));
    }
}
