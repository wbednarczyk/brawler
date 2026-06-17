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
| `ttm(metric)`     | trailing-twelve-months value (sums the last four quarters, or uses the latest annual figure) |
| `avg(metric, n)`  | average of `metric` over the last `n` periods |
| `trend(metric, n)`| average change per period of `metric` over the last `n` periods (positive = rising) |

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
