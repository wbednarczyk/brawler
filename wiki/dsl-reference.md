# DSL reference — quality-framework criteria

Quality-framework criteria are written in a small expression language (a "DSL").
A criterion is a **test** that evaluates to pass or fail against a company's
fundamentals — for example:

```
roic >= 15%
net_debt_to_ebitda < 2.5 AND fcf > 0
cagr(revenue, 5) > 10%
```

This page is the complete reference. It's the same expression language Brawler
uses internally for derived-metric formulas, so anything here works in both.

## The shape of a criterion

A criterion must be a **test** — a comparison, or several comparisons joined with
`AND` / `OR` / `NOT`. A bare value like `roe + 1` is not a valid criterion (it
doesn't decide anything); the editor will tell you so.

## Metric keys

Write a metric by its key — just the name, no quotes:

```
revenue        operating_cash_flow        roic        net_debt_to_ebitda
```

The criteria editor shows the metrics an expression references as you type, and
lists what's available. A metric that can't be computed for a company (because a
required figure hasn't been entered) makes the criterion read **No data** rather
than fail.

Common keys include: `revenue`, `net_profit`, `gross_profit`, `operating_profit`,
`ebitda`, `gross_margin`, `operating_margin`, `net_margin`, `free_cash_flow`,
`fcf_margin`, `fcf_conversion`, `roe`, `roa`, `roic`, `roce`, `net_debt`,
`net_debt_to_ebitda`, `debt_to_equity`, `current_ratio`, `quick_ratio`,
`interest_coverage`, `payout_ratio`. You can also define your own (see
[Custom metrics](#custom-metrics)).

## Numbers and percentages

| You write | It means |
|-----------|----------|
| `2.5`     | the number 2.5 |
| `15%`     | the ratio 0.15 |

**Percentages are ratios.** Brawler stores ratio-style metrics as decimals — a
return on equity of 18% is stored as `0.18`. So `roe >= 15%` compares `0.18`
against `0.15`. Write thresholds for margins, returns, and growth as percentages
(`>= 15%`), and they line up with the metric automatically.

For a plain ratio like net debt / EBITDA, use a plain number: `net_debt_to_ebitda
< 2.5`.

## Comparisons

| Operator | Meaning |
|----------|---------|
| `>=`     | greater than or equal |
| `<=`     | less than or equal |
| `>`      | greater than |
| `<`      | less than |
| `==`     | equal |
| `~=`     | approximately equal (within 1%) |

```
roe >= 15%
net_debt_to_ebitda < 2.5
payout_ratio ~= 50%
```

## Combining tests

Join comparisons with `AND`, `OR`, and `NOT` (case-insensitive), and group with
parentheses:

```
net_debt_to_ebitda < 2.5 AND fcf > 0
roe >= 15% OR roic >= 12%
NOT (current_ratio < 1)
```

## Arithmetic

You can do math inside a criterion with `+ - * /` and parentheses:

```
(gross_profit - operating_expenses) / revenue >= 20%
```

Division by zero doesn't error — it just makes the criterion **No data**.

## Functions

Some checks need more than the latest period. These functions look across the
company's period history:

| Function | What it does |
|----------|--------------|
| `cagr(metric, n)` | compound annual growth rate of `metric` over `n` years |
| `ttm(metric)`     | trailing-twelve-months value — see [Window semantics](#window-semantics) |
| `avg(metric, n)`  | average of `metric` over the last `n` periods |
| `trend(metric, n)`| average change per period of `metric` over the last `n` periods (positive = rising) |
| `coalesce(a, b, …)` | the first expression whose inputs are available — a fallback chain, e.g. `coalesce(market_cap / net_profit_ttm, close / eps_diluted_ttm)`; empty only when every recipe fails |

```
cagr(revenue, 5) > 10%
trend(operating_margin, 3) > 0
avg(roic, 3) >= 12%
```

If the history needed isn't there, the criterion reads **No data**.

## Aggregation suffixes

Two shorthands can be appended to a metric key:

| Suffix  | Meaning |
|---------|---------|
| `_ttm`  | trailing-twelve-months (same as `ttm(metric)`) |
| `_avg`  | average of the current and prior period (used for balance-sheet figures) |

For example `total_equity_avg` is the average equity across the latest two
periods. These mostly appear inside metric formulas, but you can use them in
criteria too.

## Window semantics

Exactly what each window does when history is thin:

| Window | On an annual period | On an interim (quarterly/half-year) period | Empty when |
|---|---|---|---|
| `ttm(metric)` / `_ttm` | that period's own annual figure — no summing | `last full year + this year's figure so far − the same point last year` | any one of those three is missing, **or the metric isn't a flow** |
| `_avg` | average of the current and prior period | same | the current period lacks the figure |
| `cagr(metric, n)` | `(end / begin)^(1/n) − 1` | same | no period is labelled `fiscal_year − n`, or either endpoint is ≤ 0 |

Notes worth knowing:

- **TTM at an interim period is arithmetic, not a sum of the last four rows.**
  Polish issuers report *cumulatively*: the half-year report already contains
  Q1, and the Q3 report is really nine months. Adding consecutive reports would
  count the same months twice. So for, say, a Q1 2026 report Brawler computes
  **FY 2025 + Q1 2026 − Q1 2025** — last full year, roll the new quarter on,
  roll the year-ago quarter off.
- **A TTM that can't be closed is empty, not a guess.** All three inputs must
  exist, and the year-ago period must carry the *same* fiscal label (a half-year
  figure can't stand in for a quarter). Miss one and `_ttm` reads **No data**
  rather than quietly presenting part of a year as a full one. Everything built
  on it — `roe`, `roa`, `pe_ratio`, `ev_ebitda`, `fcf_yield`, the
  comparative-valuation drivers — goes empty with it. Most companies get their
  TTM back one year after their first interim report, once there's a year-ago
  comparison point.
- **`_ttm` only applies to flows** — things that accumulate over months, like
  revenue, profit, EBITDA or cash flow. On a balance-sheet figure (equity,
  assets, cash, debt, share count, price) or on a ratio there is no such thing
  as a trailing twelve months, so `_ttm` yields **No data**. Use the bare key
  for the latest balance, or `_avg` for the two-period average that return
  ratios like ROE use.
- **`_avg` does degrade.** With no prior period (or none carrying the figure) it
  returns the current value alone. Balance-sheet averaging exists to smooth a
  point-in-time figure; one point is still a usable figure, unlike a partial sum.
- **`cagr` matches on the fiscal-year label**, not on position in the series: it
  looks for a period whose `fiscal_year` is exactly `n` years before the latest
  one. A gap year means **No data**. Negative or zero endpoints have no defined
  growth rate, so they're empty too. The fractional root is computed in floating
  point — a growth-rate threshold doesn't need decimal-exact arithmetic.

## The "No data" verdict

A criterion is **No data** — not Fail — whenever a metric it needs can't be
computed: a missing figure, not enough history for a `cagr`, or a divide-by-zero.
This keeps the scorecard honest: a company isn't penalised for a gap in the data,
it's simply flagged as not-yet-computable.

## Partial band (optional)

Each criterion can carry an optional **partial band** — a softer threshold. If a
criterion fails its main test but passes the softer one, the verdict is
**Partial** instead of **Fail**. For example, a criterion `roe >= 15%` with a
partial band of `10%`: an ROE of 12% is *Partial*, an ROE of 8% is *Fail*.

## Custom metrics

The metric list is open. If a metric you want isn't built in, it can be defined
as a formula over existing keys (e.g. a "rule of 40" as
`revenue_growth + fcf_margin`) and then referenced by your criteria like any
other metric. Defined metrics are computed by the same engine, so no new syntax
is involved.

## Decision support only

Criteria measure facts and return verdicts. They **cannot** express advice — there
is no way to write "if this passes, buy". Brawler is a decision-support tool: it
shows you the measurements and verdicts, and the decision stays yours.

## Worked examples

```
# Durable returns
roic >= 15%

# Conservative balance sheet with real cash generation
net_debt_to_ebitda < 2.5 AND fcf > 0

# Healthy and improving margins
operating_margin >= 15% AND trend(operating_margin, 3) > 0

# Steady top-line growth
cagr(revenue, 5) >= 8%

# Cash-backed earnings
fcf_conversion >= 80%

# Liquidity floor
current_ratio >= 1.5
```
