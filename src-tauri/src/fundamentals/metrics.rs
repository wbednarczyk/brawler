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
//! Quote-derived inputs (ADR 0067 Decision 4, v0.53 T4): level-0 market
//! ratios (market cap, P/E, P/BV, EV/EBITDA, dividend yield, FCF yield,
//! 52-week distance, own-history percentile) are seeded as ordinary canonical
//! `kpi_definitions` formulas (migration `0072`) referencing quote-derived
//! variables -- `close`, `week52_high`, `week52_low`,
//! `valuation_percentile_52w` -- the same way any formula references a
//! reported fact. Those four variables are not `financial_facts`; the caller
//! computes them once from bounded, indexed reads
//! (`MarketDataStore::latest_quote` / `quotes_since` -- an as-of close plus a
//! trailing-52-week range scan, never a corpus recompute) into a
//! `QuoteFacts` snapshot and attaches it via `MetricsContext::with_quotes`.
//! The resolver treats it as one period-independent as-of-date fact set,
//! resolved before period-bound reported facts.
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

/// The canonical fiscal label of an annual period, as stored in
/// `financial_periods.period_type`.
pub const ANNUAL_PERIOD_TYPE: &str = "FY";

/// Metric keys measured **at a point in time** rather than accumulated over
/// one — balance-sheet lines, share/asset counts and quote scalars. A TTM
/// window is undefined over them (see [`MetricsContext::is_flow_key`]).
///
/// **Interim home.** `kpi_definitions` carries no flow/stock axis: `value_kind`
/// separates `monetary` from `ratio`, not a revenue from a cash balance. Until
/// the catalog grows that column (at which point this list becomes a seeded
/// migration and this const goes away), the classification lives here, covering
/// every seeded key across `canonical` and `sector` scope. Keys genuinely
/// ambiguous on the axis are deliberately **absent** — `_ttm` keeps working for
/// them rather than being refused on a guess.
///
/// Ratios and percentages are *not* listed: they are refused by `value_kind`
/// instead, which also covers user-defined ones.
pub(crate) const STOCK_METRIC_KEYS: &[&str] = &[
    // Balance sheet — canonical reported (migration 0034).
    "cash",
    "current_assets",
    "current_liabilities",
    "inventories",
    "inventory",
    "long_term_debt",
    "net_debt",
    "retained_earnings",
    "total_assets",
    "total_debt",
    "total_equity",
    "total_liabilities",
    // Balance sheet — WDF issuer rows (migration 0112).
    "wdf_equity_parent",
    "wdf_noncurrent_assets",
    "wdf_noncurrent_liabilities",
    "wdf_share_capital",
    // Book value per share is equity-at-a-date divided by a share count at the
    // same date — a balance expressed per share is still a balance, so span
    // arithmetic over it is the same category error (migration 0131, #309).
    "wdf_book_value_per_share",
    "wdf_rozwodniona_wartosc_ksiegowa_na_jedna_akcje",
    // Derived aggregates of the above — sums of balances are still balances
    // (migrations 0034/0050/0072).
    "capital_employed",
    "invested_capital",
    "market_cap",
    "working_capital",
    // Counts and durations measured on a date, not over a span.
    "properties_count",
    "shares_outstanding",
    "walt",
    // Quote scalars: an as-of-date price is the definition of point-in-time
    // (ADR 0067 Decision 4).
    "close",
    "week52_high",
    "week52_low",
    // Sector packs — banking / specialty-finance balances (migration 0034).
    "erc",
    "total_deposits",
    "total_loans",
];

/// Whether `metric_key` names a flow (accumulates over a span) rather than a
/// stock (a balance at a point in time) — the same TTM-eligibility axis
/// [`MetricsContext::is_flow_key`] uses, pulled out pure so a caller with no
/// [`MetricsContext`] (`fundamentals::kpi_manifest`'s `period.window_kind_mismatch`
/// check, #361) can classify a metric it already has the `value_kind` for
/// without building one. `value_kind` is `None` for a key with no resolved
/// catalog definition — treated as a flow, the same default the method form
/// uses for a key absent from both [`STOCK_METRIC_KEYS`] and the catalog.
pub(crate) fn is_flow_key(metric_key: &str, value_kind: Option<&str>) -> bool {
    if STOCK_METRIC_KEYS.contains(&metric_key) {
        return false;
    }
    !matches!(value_kind, Some("ratio") | Some("percentage"))
}

/// Reported facts for one period. `index 0` is the latest period.
#[derive(Debug, Clone)]
pub struct PeriodFacts {
    pub period_id: String,
    pub fiscal_year: i64,
    /// The canonical fiscal label (`FY`, `H1`, `Q1`–`Q4`, `9M`, …) exactly as
    /// stored in `financial_periods.period_type`. Drives the annual/interim
    /// split and the same-label prior-year lookup that TTM span arithmetic
    /// needs — which is why the period's *label* is carried here rather than a
    /// derived `is_annual` flag.
    pub period_type: String,
    pub reported: HashMap<String, Decimal>,
}

impl PeriodFacts {
    /// True for an annual (`FY`) period; false for every interim label.
    pub fn is_annual(&self) -> bool {
        self.period_type == ANNUAL_PERIOD_TYPE
    }
}

/// Quote-derived scalars for one company as of a given date (ADR 0067
/// Decision 4). Computed by the caller from bounded/indexed range reads
/// (`MarketDataStore::latest_quote` / `quotes_since`) -- never a corpus scan
/// -- and injected into the resolver as one period-independent as-of-date
/// fact set (available uniformly across every period index, since a daily
/// quote does not line up 1:1 with a fiscal period). A field left `None`
/// (e.g. no quote history yet) simply drops out; any formula referencing it
/// resolves to `None` per the engine's existing missing-input contract --
/// never a panic.
#[derive(Debug, Clone, Copy, Default, PartialEq)]
pub struct QuoteFacts {
    /// Latest close on or before the as-of date.
    pub close: Option<Decimal>,
    /// High of the trailing 52-week close-price range (including `close`).
    pub week52_high: Option<Decimal>,
    /// Low of the trailing 52-week close-price range (including `close`).
    pub week52_low: Option<Decimal>,
    /// `close`'s percentile within its own trailing-52-week close-price
    /// range, in `[0, 1]` (0 = at/below the 52w low, 1 = at/above the 52w
    /// high). An own-history *price* percentile, not a full historical
    /// ratio recompute: point-in-time fundamentals are not stored at daily
    /// granularity, so a true ratio percentile has no data substrate yet;
    /// price is the dominant driver of ratio movement over a 52-week window,
    /// so this is the closest computable proxy for "where does today's
    /// valuation sit vs its own history" (flagged assumption -- see T4
    /// report).
    pub valuation_percentile_52w: Option<Decimal>,
}

impl QuoteFacts {
    /// Look up one of the four quote-derived variable names. Unknown keys
    /// (i.e. any key that is not a quote variable) return `None`, deferring
    /// to the period-fact/derived-formula resolution path.
    fn get(&self, key: &str) -> Option<Decimal> {
        match key {
            "close" => self.close,
            "week52_high" => self.week52_high,
            "week52_low" => self.week52_low,
            "valuation_percentile_52w" => self.valuation_percentile_52w,
            _ => None,
        }
    }

    /// Build a snapshot from an as-of close plus a bounded trailing-52-week
    /// close-price series (the caller's `quotes_since(company_id, from_date)`
    /// range read, one indexed scan -- this function does no I/O and no
    /// corpus recompute). `history` may or may not include the as-of bar
    /// itself; both are folded into the high/low/percentile range.
    pub fn from_close_history(as_of_close: Decimal, history: &[Decimal]) -> Self {
        let mut high = as_of_close;
        let mut low = as_of_close;
        for &c in history {
            if c > high {
                high = c;
            }
            if c < low {
                low = c;
            }
        }
        let percentile = if high > low {
            ((as_of_close - low) / (high - low)).clamp(Decimal::ZERO, Decimal::ONE)
        } else {
            // A flat or single-point range: the as-of close sits at the top
            // of its own (degenerate) range.
            Decimal::ONE
        };
        Self {
            close: Some(as_of_close),
            week52_high: Some(high),
            week52_low: Some(low),
            valuation_percentile_52w: Some(percentile),
        }
    }
}

/// Composite health-score scalars for one company as of its latest FY period
/// (ADR 0083 Decision 2, the scoped ADR 0046 amendment). Injected into metric
/// resolution the same way [`QuoteFacts`] market scalars are, so scorecard
/// criteria can reference `piotroski_f` / `altman_z` (`piotroski_f >= 7`).
///
/// The injected value is the **latest-FY headline** only; when the score is
/// `InsufficientData` or `NotApplicable` the field is `None`, so the key
/// resolves `Unavailable` at the engine boundary (rendered as an `unavailable`
/// verdict) — never a partial or rescaled number. These are composite scores
/// with indicator logic and prior-period references that do not fit the
/// catalog grammar; quotes and health scores are the only two injection seams,
/// the open-catalog rule stands for everything else.
#[derive(Debug, Clone, Copy, Default, PartialEq)]
pub struct HealthScalars {
    /// Piotroski F (0..=9) as a scalar, or `None` when not a headline.
    pub piotroski_f: Option<Decimal>,
    /// Altman Z″ (EM) value as a scalar, or `None` when not a headline.
    pub altman_z: Option<Decimal>,
}

impl HealthScalars {
    /// Look up one of the two health-score scalar names. Unknown keys return
    /// `None`, deferring to the period-fact/derived-formula resolution path.
    fn get(&self, key: &str) -> Option<Decimal> {
        match key {
            "piotroski_f" => self.piotroski_f,
            "altman_z" => self.altman_z,
            _ => None,
        }
    }
}

/// One annual (`FY`) period's position in the context's newest-first series,
/// the coordinate the health engine scores over.
#[derive(Debug, Clone)]
pub struct FyPeriodRef {
    /// Index into the newest-first `periods` series.
    pub idx: usize,
    pub period_id: String,
    pub fiscal_year: i64,
}

/// The loaded, immutable view the metrics service computes over.
#[derive(Debug, Clone)]
pub struct MetricsContext {
    definitions: HashMap<String, MetricDef>,
    /// Periods newest-first.
    periods: Vec<PeriodFacts>,
    /// One as-of-date quote snapshot, if attached via [`Self::with_quotes`].
    quotes: Option<QuoteFacts>,
    /// Latest-FY health-score scalars, if attached via
    /// [`Self::with_health_scalars`].
    health: Option<HealthScalars>,
}

impl MetricsContext {
    pub fn new(definitions: HashMap<String, MetricDef>, periods: Vec<PeriodFacts>) -> Self {
        Self {
            definitions,
            periods,
            quotes: None,
            health: None,
        }
    }

    /// Attach the latest-FY health scalars (ADR 0083 Decision 2) so criteria
    /// referencing `piotroski_f` / `altman_z` resolve. Additive: a context that
    /// never calls this keeps resolving those keys to `None`.
    pub fn with_health_scalars(mut self, health: HealthScalars) -> Self {
        self.health = Some(health);
        self
    }

    /// Resolve a metric at an explicit period index (0 = latest), starting a
    /// fresh cycle-guard. The entry point the health engine uses to read a
    /// prior FY period's reported and derived values.
    pub fn value_for(&self, key: &str, idx: usize) -> Option<Decimal> {
        let visiting = RefCell::new(HashSet::new());
        self.value_at(key, idx, &visiting)
    }

    /// The fiscal year of the period at `idx`, if it exists.
    pub fn fiscal_year_at(&self, idx: usize) -> Option<i64> {
        self.periods.get(idx).map(|p| p.fiscal_year)
    }

    /// Every annual (`FY`) period, newest-first, with its series index — the
    /// coordinates the health engine scores over (interim periods are skipped,
    /// so "prior FY" is the next FY, never an intervening quarter).
    pub fn fy_period_refs(&self) -> Vec<FyPeriodRef> {
        self.periods
            .iter()
            .enumerate()
            .filter(|(_, p)| p.is_annual())
            .map(|(idx, p)| FyPeriodRef {
                idx,
                period_id: p.period_id.clone(),
                fiscal_year: p.fiscal_year,
            })
            .collect()
    }

    /// Attach a quote snapshot (ADR 0067 Decision 4) so formulas referencing
    /// `close` / `week52_high` / `week52_low` / `valuation_percentile_52w`
    /// resolve. Additive: existing callers that never call this keep
    /// resolving those keys to `None`, exactly as before this variant
    /// existed (no behavior change for the quality-frameworks engine).
    pub fn with_quotes(mut self, quotes: QuoteFacts) -> Self {
        self.quotes = Some(quotes);
        self
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

        // Quote-derived scalars (ADR 0067 Decision 4): one as-of-date
        // snapshot, resolved ahead of period facts and available uniformly
        // at every period index -- a daily quote has no fiscal-period index
        // of its own.
        if let Some(v) = self.quotes.as_ref().and_then(|q| q.get(base)) {
            return Some(v);
        }

        // Health-score scalars (ADR 0083 Decision 2): the latest-FY composite
        // headlines, resolved ahead of period facts and available uniformly at
        // every period index (a company-level score, not a per-period fact).
        // `None` (InsufficientData / NotApplicable) falls through and the key
        // resolves `Unavailable`.
        if let Some(v) = self.health.as_ref().and_then(|h| h.get(base)) {
            return Some(v);
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
            // coalesce never reaches the window path: the evaluator resolves
            // it over full expressions before dispatching window functions.
            Func::Coalesce => None,
        }
    }

    /// TTM. For an **annual** period, that period's own figure. For an
    /// **interim** period, period-span arithmetic:
    ///
    /// ```text
    /// ttm = FY(year − 1) + YTD(period_type, year) − YTD(period_type, year − 1)
    /// ```
    ///
    /// `None` unless all three inputs exist — a span with a hole is not a
    /// trailing twelve months (#62), and neither is a sum of whatever rows
    /// happen to sit next to each other in the series (#271).
    ///
    /// Why arithmetic rather than "sum the last four rows": interim facts are
    /// **cumulative year-to-date**, and the series interleaves granularities. On
    /// the owner's database (measured 2026-07-31) GPW companies file `Q1` (3M),
    /// `H1` (6M), `Q3` (9M) and `FY` — no `Q2`/`Q4` row exists at all — so
    /// consecutive rows overlap (`H1 ⊃ Q1`) and a four-row sum double-counts,
    /// summing across granularities and across years. Subtracting the prior
    /// year's same-label year-to-date from the prior full year leaves exactly
    /// the trailing months the current year-to-date does not cover.
    ///
    /// The formula assumes interim figures are cumulative. A future adapter
    /// filing *discrete* quarters (US 10-Q) is correct only at the first
    /// interim period of a fiscal year and must revisit this — see the
    /// window-semantics note in `docs/data-model.md`.
    fn ttm_at(
        &self,
        key: &str,
        idx: usize,
        visiting: &RefCell<HashSet<(String, usize)>>,
    ) -> Option<Decimal> {
        if !self.is_flow_key(key) {
            return None;
        }
        let period = self.periods.get(idx)?;
        if period.is_annual() {
            return self.value_at(key, idx, visiting);
        }
        let prior_year = period.fiscal_year - 1;
        let prior_ytd_idx = self.period_index(&period.period_type, prior_year)?;
        let prior_fy_idx = self.period_index(ANNUAL_PERIOD_TYPE, prior_year)?;

        let ytd = self.value_at(key, idx, visiting)?;
        let prior_ytd = self.value_at(key, prior_ytd_idx, visiting)?;
        let prior_fy = self.value_at(key, prior_fy_idx, visiting)?;
        Some(prior_fy + ytd - prior_ytd)
    }

    /// Whether a TTM window is defined over this key at all. "Trailing twelve
    /// months" only means something for a quantity that **accumulates over
    /// time**; two classes never do, and for them the honest answer is no value
    /// rather than a computed one:
    ///
    /// * **point-in-time stocks** — balance-sheet lines, share counts, quotes
    ///   ([`STOCK_METRIC_KEYS`]). Span arithmetic over a balance produces a
    ///   number that is neither a balance nor a flow. Use the bare key, or
    ///   `_avg` for the balance-sheet averaging that return ratios want.
    /// * **ratios and percentages** — never additive across periods, whatever
    ///   their inputs. Recognised from the catalog's `value_kind`, so a
    ///   `user`/`sector`-scope ratio is covered without listing it.
    ///
    /// A key absent from both the list and the catalog (an ad-hoc reported
    /// fact) is treated as a flow — the pre-existing default.
    fn is_flow_key(&self, key: &str) -> bool {
        is_flow_key(
            key,
            self.definitions.get(key).map(|d| d.value_kind.as_str()),
        )
    }

    /// The series index of the period carrying this exact fiscal label and year
    /// (`financial_periods` is unique on `(company, fiscal_year, period_type)`,
    /// so at most one matches).
    fn period_index(&self, period_type: &str, fiscal_year: i64) -> Option<usize> {
        self.periods
            .iter()
            .position(|p| p.fiscal_year == fiscal_year && p.period_type == period_type)
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
            period_type: "FY".to_owned(),
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

    /// Honest metric windows — T6 (#62) then #271. Every other test in this
    /// module runs on annual periods; production carries the raw
    /// `financial_periods.period_type` (`storage/quality_frameworks.rs`), so
    /// these pin the interim branch and the documented degradations of `_avg` /
    /// `cagr`.
    ///
    /// Interim facts on the real DB are **cumulative year-to-date** (Polish
    /// reporting convention, measured 2026-07-31 on the owner's database: CDR
    /// FY2024 revenue Q1 226.8M < H1 424.8M < FY 799.6M, same shape for
    /// net_profit; no `Q2`/`Q4` row exists anywhere — GPW files Q1 / H1 / Q3-as-9M
    /// / FY). So a trailing twelve months at an interim period is span
    /// arithmetic, never a sum of the last four rows.
    mod window_semantics {
        use super::*;

        /// An interim (year-to-date) period carrying its real fiscal label.
        fn interim(year: i64, period_type: &str, facts: &[(&str, Decimal)]) -> PeriodFacts {
            PeriodFacts {
                period_id: format!("p{year}{period_type}"),
                fiscal_year: year,
                period_type: period_type.to_owned(),
                reported: facts.iter().map(|(k, v)| (k.to_string(), *v)).collect(),
            }
        }

        /// The real GPW shape (DBC on the owner's DB): Q1 of the current year on
        /// top of an FY history that still holds last year's Q1.
        fn dbc_shaped_series() -> Vec<PeriodFacts> {
            vec![
                interim(2026, "Q1", &[("net_profit", dec(100, 0))]),
                period(2025, &[("net_profit", dec(400, 0))]),
                interim(2025, "Q3", &[("net_profit", dec(300, 0))]),
                interim(2025, "Q1", &[("net_profit", dec(80, 0))]),
                period(2024, &[("net_profit", dec(380, 0))]),
            ]
        }

        #[test]
        fn ttm_at_an_interim_period_is_prior_fy_plus_ytd_minus_prior_year_ytd() {
            // 400 (FY2025) + 100 (YTD Q1 2026) − 80 (YTD Q1 2025) = 420.
            let ctx = ctx_with(dbc_shaped_series());
            assert_eq!(ctx.resolver().value("net_profit_ttm"), Some(dec(420, 0)));
        }

        #[test]
        fn ttm_never_sums_across_period_granularities() {
            let ctx = ctx_with(dbc_shaped_series());
            let ttm = ctx.resolver().value("net_profit_ttm");
            assert_ne!(ttm, Some(dec(880, 0)), "index-walk sum must be dead");
            assert_eq!(ttm, Some(dec(420, 0)));
        }

        #[test]
        fn ttm_is_none_when_the_prior_year_interim_period_is_absent() {
            // The dominant shape today (KTY, PEO, CDR): one interim period on
            // top of FY history, with no same-label period a year earlier. The
            // span cannot be closed, so the answer is "no data" — never the
            // year-to-date figure wearing a TTM label.
            let ctx = ctx_with(vec![
                interim(2026, "H1", &[("net_profit", dec(386, 0))]),
                period(2025, &[("net_profit", dec(569, 0))]),
                period(2024, &[("net_profit", dec(561, 0))]),
                period(2023, &[("net_profit", dec(540, 0))]),
            ]);
            assert_eq!(ctx.resolver().value("net_profit_ttm"), None);
        }

        #[test]
        fn ttm_is_none_when_the_prior_annual_period_is_absent() {
            let ctx = ctx_with(vec![
                interim(2026, "Q1", &[("net_profit", dec(100, 0))]),
                interim(2025, "Q1", &[("net_profit", dec(80, 0))]),
                period(2024, &[("net_profit", dec(380, 0))]), // FY2025 missing
            ]);
            assert_eq!(ctx.resolver().value("net_profit_ttm"), None);
        }

        #[test]
        fn ttm_is_none_when_any_span_input_lacks_the_fact() {
            // #62's rule survives the #271 rewrite: a span with a hole is not a
            // trailing twelve months. Each of the three inputs in turn.
            let holes = [
                ("net_profit", "revenue", "revenue"),
                ("revenue", "net_profit", "revenue"),
                ("revenue", "revenue", "net_profit"),
            ];
            for (now, prior_fy, prior_ytd) in holes {
                let ctx = ctx_with(vec![
                    interim(2026, "Q1", &[(now, dec(100, 0))]),
                    period(2025, &[(prior_fy, dec(400, 0))]),
                    interim(2025, "Q1", &[(prior_ytd, dec(80, 0))]),
                ]);
                assert_eq!(ctx.resolver().value("net_profit_ttm"), None);
            }
        }

        #[test]
        fn ttm_matches_the_prior_year_period_on_its_fiscal_label_not_its_position() {
            // A half-year YTD is not comparable to a Q1 YTD: the prior-year row
            // must carry the same fiscal label or there is no span.
            let ctx = ctx_with(vec![
                interim(2026, "Q1", &[("net_profit", dec(100, 0))]),
                period(2025, &[("net_profit", dec(400, 0))]),
                interim(2025, "H1", &[("net_profit", dec(200, 0))]),
            ]);
            assert_eq!(ctx.resolver().value("net_profit_ttm"), None);
        }

        #[test]
        fn ttm_on_an_annual_period_is_that_periods_own_figure() {
            // Mixed series: the annual branch never computes a span, even when
            // interim periods follow the annual period in the series.
            let ctx = ctx_with(vec![
                period(2025, &[("net_profit", dec(100, 0))]),
                interim(2025, "Q3", &[("net_profit", dec(40, 0))]),
                interim(2025, "H1", &[("net_profit", dec(30, 0))]),
                interim(2025, "Q1", &[("net_profit", dec(20, 0))]),
            ]);
            assert_eq!(ctx.resolver().value("net_profit_ttm"), Some(dec(100, 0)));
        }

        /// A trailing *twelve months* only means something for a quantity that
        /// accumulates over time. Balance-sheet lines and ratios do not, so
        /// `_ttm` over them is refused rather than computed (#271 residue).
        mod non_flow_keys {
            use super::*;

            /// The DBC span shape, carrying a balance-sheet key instead of a
            /// flow one: every span input is present, so only the flow/stock
            /// classification can make this empty.
            fn stock_series() -> Vec<PeriodFacts> {
                vec![
                    interim(2026, "Q1", &[("total_equity", dec(600, 0))]),
                    period(2025, &[("total_equity", dec(500, 0))]),
                    interim(2025, "Q1", &[("total_equity", dec(450, 0))]),
                ]
            }

            #[test]
            fn ttm_on_a_balance_sheet_key_is_none_at_an_interim_period() {
                // Span arithmetic would happily return 500 + 600 − 450 = 650:
                // a number that is not a balance, not a flow, and not anything.
                let ctx = ctx_with(stock_series());
                assert_eq!(ctx.resolver().value("total_equity_ttm"), None);
            }

            #[test]
            fn ttm_on_a_balance_sheet_key_is_none_at_an_annual_period_too() {
                // The annual branch would return the period's own equity —
                // a real figure, but "TTM equity" is still a category error.
                // The honest answers are the bare key or `_avg`.
                let ctx = ctx_with(vec![period(2025, &[("total_equity", dec(500, 0))])]);
                assert_eq!(ctx.resolver().value("total_equity_ttm"), None);
                assert_eq!(ctx.resolver().value("total_equity"), Some(dec(500, 0)));
                assert_eq!(ctx.resolver().value("total_equity_avg"), Some(dec(500, 0)));
            }

            #[test]
            fn the_ttm_function_form_refuses_stock_keys_exactly_like_the_suffix() {
                let ctx = ctx_with(stock_series());
                assert_eq!(ctx.resolver().window(Func::Ttm, "total_equity", 0), None);
            }

            #[test]
            fn ttm_on_a_ratio_or_percentage_key_is_none() {
                // Ratios never add up across periods, whatever their inputs.
                let ctx = ctx_with(vec![
                    period(
                        2025,
                        &[("gross_profit", dec(350, 0)), ("revenue", dec(1000, 0))],
                    ),
                    period(
                        2024,
                        &[("gross_profit", dec(300, 0)), ("revenue", dec(1000, 0))],
                    ),
                ]);
                assert_eq!(ctx.resolver().value("gross_margin"), Some(dec(35, 2)));
                assert_eq!(ctx.resolver().value("gross_margin_ttm"), None);
            }

            #[test]
            fn flow_keys_are_untouched_by_the_refusal() {
                // Guard against over-blocking: the same annual period still
                // resolves flow TTMs, and `roe` (net_profit_ttm /
                // total_equity_avg) still computes end to end.
                let ctx = ctx_with(vec![period(
                    2025,
                    &[("net_profit", dec(100, 0)), ("total_equity", dec(500, 0))],
                )]);
                assert_eq!(ctx.resolver().value("net_profit_ttm"), Some(dec(100, 0)));
                assert_eq!(ctx.resolver().value("roe"), Some(dec(2, 1)));
            }
        }

        #[test]
        fn avg2_degrades_to_the_current_value_when_the_prior_period_lacks_the_fact() {
            // Documented degradation (balance-sheet averaging): a two-point
            // average with no prior point is the current point, not missing.
            let ctx = ctx_with(vec![
                interim(2026, "Q1", &[("total_equity", dec(600, 0))]),
                interim(2025, "Q3", &[("revenue", dec(300, 0))]), // no total_equity
            ]);
            assert_eq!(ctx.resolver().value("total_equity_avg"), Some(dec(600, 0)));
        }

        #[test]
        fn cagr_is_none_when_the_target_fiscal_year_is_absent() {
            // Fiscal-year-label lookup: no period labeled 2020 → no baseline.
            let ctx = ctx_with(vec![
                period(2025, &[("revenue", dec(1610, 0))]),
                period(2024, &[("revenue", dec(1400, 0))]),
            ]);
            assert_eq!(ctx.resolver().window(Func::Cagr, "revenue", 5), None);
        }

        #[test]
        fn cagr_is_none_when_an_endpoint_is_not_positive() {
            let ctx = ctx_with(vec![
                period(2025, &[("net_profit", dec(500, 0))]),
                period(2020, &[("net_profit", dec(-100, 0))]),
            ]);
            assert_eq!(ctx.resolver().window(Func::Cagr, "net_profit", 5), None);
        }
    }

    /// v0.53 T4 (ADR 0067 Decision 4): level-0 market ratios + market cap,
    /// mirroring the canonical formulas seeded by migration `0072`.
    mod market_ratios {
        use super::*;

        /// A catalog matching the canonical rows seeded by
        /// `migrations/0072_level0_market_ratios.sql` -- kept in this test
        /// module (not loaded from SQL) per the existing `ctx_with` style;
        /// `every_canonical_derived_metric_is_computable_from_a_representative_fact_set`
        /// (`storage/tests/quality_frameworks.rs`) is the DB-driven drift
        /// guard that the real seeded rows stay computable.
        fn market_ratio_defs() -> HashMap<String, MetricDef> {
            let mut defs = HashMap::new();
            // Quote-derived (reported-shaped; resolved via `QuoteFacts`, not
            // `financial_facts`).
            defs.insert("close".to_owned(), def_reported("monetary"));
            defs.insert("week52_high".to_owned(), def_reported("monetary"));
            defs.insert("week52_low".to_owned(), def_reported("monetary"));
            defs.insert("valuation_percentile_52w".to_owned(), def_reported("ratio"));
            // Reported facts the ratios depend on.
            defs.insert("shares_outstanding".to_owned(), def_reported("count"));
            defs.insert("eps_diluted".to_owned(), def_reported("monetary"));
            defs.insert("eps_basic".to_owned(), def_reported("monetary"));
            defs.insert("total_equity".to_owned(), def_reported("monetary"));
            defs.insert("net_debt".to_owned(), def_reported("monetary"));
            defs.insert("net_profit".to_owned(), def_reported("monetary"));
            defs.insert("ebitda".to_owned(), def_reported("monetary"));
            defs.insert("dividend_per_share".to_owned(), def_reported("monetary"));
            defs.insert("operating_cash_flow".to_owned(), def_reported("monetary"));
            defs.insert("capex".to_owned(), def_reported("monetary"));
            defs.insert(
                "free_cash_flow".to_owned(),
                def_derived("monetary", "operating_cash_flow - capex"),
            );
            // Level-0 market ratios (migration 0072).
            defs.insert(
                "market_cap".to_owned(),
                def_derived("monetary", "close * shares_outstanding"),
            );
            defs.insert(
                "pe_ratio".to_owned(),
                // Migration 0075: fallback chain — whichever inputs exist.
                def_derived(
                    "ratio",
                    "coalesce(market_cap / net_profit_ttm, close / eps_diluted_ttm, close / eps_basic_ttm)",
                ),
            );
            defs.insert(
                "pbv_ratio".to_owned(),
                def_derived("ratio", "market_cap / total_equity"),
            );
            defs.insert(
                "ev_ebitda".to_owned(),
                def_derived("ratio", "(market_cap + net_debt) / ebitda_ttm"),
            );
            defs.insert(
                "dividend_yield".to_owned(),
                def_derived("percentage", "dividend_per_share / close"),
            );
            defs.insert(
                "fcf_yield".to_owned(),
                def_derived("percentage", "free_cash_flow_ttm / market_cap"),
            );
            defs.insert(
                "distance_from_52w_high".to_owned(),
                def_derived("percentage", "(close - week52_high) / week52_high"),
            );
            defs.insert(
                "distance_from_52w_low".to_owned(),
                def_derived("percentage", "(close - week52_low) / week52_low"),
            );
            defs
        }

        fn period_with(year: i64, facts: &[(&str, Decimal)]) -> PeriodFacts {
            PeriodFacts {
                period_id: format!("p{year}"),
                fiscal_year: year,
                period_type: "FY".to_owned(),
                reported: facts.iter().map(|(k, v)| (k.to_string(), *v)).collect(),
            }
        }

        fn ctx_with_quotes(facts: &[(&str, Decimal)], quotes: QuoteFacts) -> MetricsContext {
            MetricsContext::new(market_ratio_defs(), vec![period_with(2025, facts)])
                .with_quotes(quotes)
        }

        fn quotes(close: Decimal) -> QuoteFacts {
            QuoteFacts {
                close: Some(close),
                week52_high: None,
                week52_low: None,
                valuation_percentile_52w: None,
            }
        }

        // ---- (a) formula unit tests with known fixtures ----------------------

        #[test]
        fn market_cap_from_close_and_shares_outstanding() {
            let ctx = ctx_with_quotes(&[("shares_outstanding", dec(100, 0))], quotes(dec(231, 0)));
            assert_eq!(ctx.resolver().value("market_cap"), Some(dec(23100, 0)));
        }

        #[test]
        fn pe_ratio_falls_back_to_eps_when_market_cap_inputs_are_missing() {
            // No shares_outstanding (no market_cap) — the chain falls through
            // to close / eps_diluted_ttm.
            let ctx = ctx_with_quotes(&[("eps_diluted", dec(10, 0))], quotes(dec(231, 0)));
            assert_eq!(ctx.resolver().value("pe_ratio"), Some(dec(231, 1))); // 23.1
        }

        #[test]
        fn pe_ratio_is_none_only_when_every_recipe_fails() {
            let ctx = ctx_with_quotes(&[], quotes(dec(231, 0)));
            assert_eq!(ctx.resolver().value("pe_ratio"), None);
        }

        #[test]
        fn pe_ratio_from_market_cap_and_net_profit() {
            // P/E over the best-covered inputs (migration 0074): market_cap /
            // net_profit_ttm == (close x shares) / profit; 231*100 / 1000 = 23.1.
            let ctx = ctx_with_quotes(
                &[
                    ("shares_outstanding", dec(100, 0)),
                    ("net_profit", dec(1000, 0)),
                ],
                quotes(dec(231, 0)),
            );
            assert_eq!(ctx.resolver().value("pe_ratio"), Some(dec(231, 1))); // 23.1
        }

        #[test]
        fn pbv_ratio_from_market_cap_and_equity() {
            let ctx = ctx_with_quotes(
                &[
                    ("shares_outstanding", dec(100, 0)),
                    ("total_equity", dec(11550, 0)),
                ],
                quotes(dec(231, 0)),
            );
            // market_cap = 23100; pbv = 23100 / 11550 = 2
            assert_eq!(ctx.resolver().value("pbv_ratio"), Some(dec(2, 0)));
        }

        #[test]
        fn ev_ebitda_from_market_cap_net_debt_and_ebitda() {
            let ctx = ctx_with_quotes(
                &[
                    ("shares_outstanding", dec(100, 0)),
                    ("net_debt", dec(900, 0)),
                    ("ebitda", dec(2000, 0)),
                ],
                quotes(dec(231, 0)),
            );
            // ev = 23100 + 900 = 24000; ev_ebitda = 24000 / 2000 = 12
            assert_eq!(ctx.resolver().value("ev_ebitda"), Some(dec(12, 0)));
        }

        #[test]
        fn dividend_yield_from_dividend_per_share_and_close() {
            let ctx = ctx_with_quotes(
                &[("dividend_per_share", dec(462, 2))], // 4.62
                quotes(dec(231, 0)),
            );
            // 4.62 / 231 = 0.02
            assert_eq!(ctx.resolver().value("dividend_yield"), Some(dec(2, 2)));
        }

        #[test]
        fn fcf_yield_from_free_cash_flow_and_market_cap() {
            let ctx = ctx_with_quotes(
                &[
                    ("shares_outstanding", dec(100, 0)),
                    ("operating_cash_flow", dec(1155, 0)),
                    ("capex", dec(0, 0)),
                ],
                quotes(dec(231, 0)),
            );
            // fcf = 1155; market_cap = 23100; fcf_yield = 0.05
            assert_eq!(ctx.resolver().value("fcf_yield"), Some(dec(5, 2)));
        }

        #[test]
        fn distance_from_52w_high_and_low() {
            let mut q = quotes(dec(90, 0));
            q.week52_high = Some(dec(100, 0));
            q.week52_low = Some(dec(60, 0));
            let ctx = ctx_with_quotes(&[], q);
            // (90 - 100) / 100 = -0.1
            assert_eq!(
                ctx.resolver().value("distance_from_52w_high"),
                Some(dec(-1, 1))
            );
            // (90 - 60) / 60 = 0.5
            assert_eq!(
                ctx.resolver().value("distance_from_52w_low"),
                Some(dec(5, 1))
            );
        }

        #[test]
        fn valuation_percentile_from_close_history() {
            let history = [dec(60, 0), dec(75, 0), dec(100, 0), dec(90, 0)];
            let q = QuoteFacts::from_close_history(dec(90, 0), &history);
            assert_eq!(q.week52_high, Some(dec(100, 0)));
            assert_eq!(q.week52_low, Some(dec(60, 0)));
            // (90 - 60) / (100 - 60) = 0.75
            assert_eq!(q.valuation_percentile_52w, Some(dec(75, 2)));
        }

        // ---- missing-fact-never-panics (part of the (c) contract, unit form) --

        #[test]
        fn market_cap_missing_shares_outstanding_is_none_not_panic() {
            let ctx = ctx_with_quotes(&[], quotes(dec(231, 0)));
            assert_eq!(ctx.resolver().value("market_cap"), None);
        }

        #[test]
        fn ratios_without_any_quotes_attached_are_none_not_panic() {
            let ctx = MetricsContext::new(
                market_ratio_defs(),
                vec![period_with(
                    2025,
                    &[
                        ("shares_outstanding", dec(100, 0)),
                        ("total_equity", dec(11550, 0)),
                    ],
                )],
            ); // no `.with_quotes(..)`
            for key in [
                "market_cap",
                "pe_ratio",
                "pbv_ratio",
                "ev_ebitda",
                "dividend_yield",
                "fcf_yield",
                "distance_from_52w_high",
                "distance_from_52w_low",
            ] {
                assert_eq!(ctx.resolver().value(key), None, "{key} without quotes");
            }
        }

        // ---- (b) golden snapshot of the full seeded canonical ratio set -------

        #[test]
        fn seeded_canonical_ratio_set_is_stable() {
            let q = QuoteFacts::from_close_history(
                dec(231, 0),
                &[dec(180, 0), dec(200, 0), dec(260, 0), dec(231, 0)],
            );
            // valuation_percentile_52w already set by from_close_history.
            let ctx = ctx_with_quotes(
                &[
                    ("shares_outstanding", dec(1_000_000, 0)),
                    ("eps_diluted", dec(3, 1)), // 0.3
                    ("total_equity", dec(50_000_000, 0)),
                    ("net_debt", dec(5_000_000, 0)),
                    ("ebitda", dec(20_000_000, 0)),
                    ("dividend_per_share", dec(4, 1)), // 0.4
                    ("operating_cash_flow", dec(9_000_000, 0)),
                    ("capex", dec(2_000_000, 0)),
                ],
                q,
            );
            let resolver = ctx.resolver();
            let mut values: Vec<(&str, Option<Decimal>)> = vec![
                "market_cap",
                "pe_ratio",
                "pbv_ratio",
                "ev_ebitda",
                "dividend_yield",
                "fcf_yield",
                "distance_from_52w_high",
                "distance_from_52w_low",
                "valuation_percentile_52w",
                "close",
                "week52_high",
                "week52_low",
            ]
            .into_iter()
            .map(|key| (key, resolver.value(key)))
            .collect();
            // Deterministic order regardless of the list literal above.
            values.sort_by_key(|(k, _)| *k);
            insta::assert_debug_snapshot!("seeded_canonical_ratio_set", values);
        }

        // ---- (c) proptest invariants -------------------------------------------

        mod properties {
            use super::*;
            use proptest::prelude::*;

            fn cents(v: i64) -> Decimal {
                Decimal::new(v, 2)
            }

            proptest! {
                /// `valuation_percentile_52w` is always in `[0, 1]`, for any
                /// as-of close and any bounded close-price history.
                #[test]
                fn percentile_is_always_in_unit_range(
                    as_of in 1i64..10_000_000,
                    history in prop::collection::vec(1i64..10_000_000, 0..64),
                ) {
                    let q = QuoteFacts::from_close_history(
                        cents(as_of),
                        &history.iter().map(|v| cents(*v)).collect::<Vec<_>>(),
                    );
                    let p = q.valuation_percentile_52w.expect("always computed");
                    prop_assert!(p >= Decimal::ZERO && p <= Decimal::ONE);
                }

                /// `market_cap = close * shares_outstanding` is never negative
                /// for non-negative inputs (both facts are physically
                /// non-negative quantities).
                #[test]
                fn market_cap_is_never_negative(
                    close in 0i64..10_000_000,
                    shares in 0i64..10_000_000_000i64,
                ) {
                    let ctx = ctx_with_quotes(
                        &[("shares_outstanding", Decimal::from(shares))],
                        quotes(cents(close)),
                    );
                    let v = ctx.resolver().value("market_cap");
                    prop_assert!(v.is_some());
                    prop_assert!(v.unwrap() >= Decimal::ZERO);
                }

                /// No ratio ever panics when an arbitrary subset of its
                /// inputs is missing -- it resolves to `None` (`Unavailable`
                /// at the engine boundary), matching the derived-metrics
                /// contract for every other formula in the catalog.
                #[test]
                fn no_panic_on_arbitrary_missing_inputs(
                    include_shares in any::<bool>(),
                    include_equity in any::<bool>(),
                    include_net_debt in any::<bool>(),
                    include_ebitda in any::<bool>(),
                    include_dividend in any::<bool>(),
                    include_ocf in any::<bool>(),
                    include_capex in any::<bool>(),
                    include_eps in any::<bool>(),
                    include_quotes in any::<bool>(),
                ) {
                    let mut facts: Vec<(&str, Decimal)> = Vec::new();
                    if include_shares { facts.push(("shares_outstanding", dec(100, 0))); }
                    if include_equity { facts.push(("total_equity", dec(1000, 0))); }
                    if include_net_debt { facts.push(("net_debt", dec(100, 0))); }
                    if include_ebitda { facts.push(("ebitda", dec(500, 0))); }
                    if include_dividend { facts.push(("dividend_per_share", dec(1, 0))); }
                    if include_ocf { facts.push(("operating_cash_flow", dec(200, 0))); }
                    if include_capex { facts.push(("capex", dec(50, 0))); }
                    if include_eps { facts.push(("eps_diluted", dec(2, 0))); }

                    let ctx = if include_quotes {
                        ctx_with_quotes(&facts, quotes(dec(100, 0)))
                    } else {
                        MetricsContext::new(market_ratio_defs(), vec![period_with(2025, &facts)])
                    };
                    let resolver = ctx.resolver();
                    for key in [
                        "market_cap", "pe_ratio", "pbv_ratio", "ev_ebitda",
                        "dividend_yield", "fcf_yield",
                        "distance_from_52w_high", "distance_from_52w_low",
                    ] {
                        // Must not panic; Some/None both acceptable.
                        let _ = resolver.value(key);
                    }
                }
            }
        }

        // ---- (d) behavioral: bounded-range 52w/percentile computation at scale --

        #[test]
        fn week52_stats_computed_over_a_full_trading_year_of_bars() {
            // Mirrors what `MarketDataStore::quotes_since(company_id, from_date)`
            // returns for a 52-week window: one bounded, indexed range read
            // (PK-scoped, `docs/engineering-workflow.md` DoD §C) -- never a
            // corpus scan. This proves the Rust-side aggregation over that
            // bounded result (~252 trading days) is correct and O(n), not a
            // recompute over the whole quote corpus.
            let mut history = Vec::with_capacity(252);
            for i in 0..252i64 {
                // A gentle oscillation so min/max are not at the boundary.
                let cents = 20000 + ((i * 37) % 5000);
                history.push(Decimal::new(cents, 2));
            }
            // Plant a known min and max inside the window.
            history[10] = Decimal::new(15000, 2); // 150.00 (low)
            history[200] = Decimal::new(30000, 2); // 300.00 (high)
            let as_of = Decimal::new(25000, 2); // 250.00

            let q = QuoteFacts::from_close_history(as_of, &history);
            assert_eq!(q.week52_low, Some(Decimal::new(15000, 2)));
            assert_eq!(q.week52_high, Some(Decimal::new(30000, 2)));
            let p = q.valuation_percentile_52w.expect("computed");
            assert!(p > Decimal::ZERO && p < Decimal::ONE);
        }
    }
}
