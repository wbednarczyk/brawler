# Compare: line companies up side by side, with relative valuation

**Compare** (Polish **Porównaj**) is the screen where you stop looking at one
company in isolation and put several tracked companies next to each other: the
same financial figures aligned across them, and — for one company at a time — how
its valuation multiples sit against its sector peers.

It is built for the buy/pass moment. Instead of pulling each peer's numbers into
a spreadsheet by hand, you pick a set of companies and read the comparison in the
app, every figure still linked back to its source report. Like everything in
Brawler it is **decision support only** — it states facts and ranges, never
"cheap" or "expensive", and never buy/sell/hold.

## Where to find it

**Porównaj** is its own entry in the left sidebar, in the modes group directly
under **Dashboard**. Open it and you start with an empty invite, not a blank
screen.

## Picking the set of companies (*zestaw spółek*)

At the top you choose which companies to compare:

- **+ Dodaj spółkę** (*Add company*) — a multi-select over the companies you
  already track. Each one you add appears as a colored chip; the color follows
  that company through the table and the chart.
- **Z watchlisty** (*From watchlist*) — a quick-pick that pulls in a whole
  watchlist at once instead of adding companies one by one.
- **Peerzy z sektora** (*Sector peers*) — a helper that offers the other tracked
  companies sharing the first company's sector. It only appears when that sector
  actually has other tracked companies, and it tells you how many.

There is **no "Porównaj" button**. The comparison recomputes on its own the
moment you have at least two companies, and again on every change to the set, the
metric, or the period. Removing a company (the **✕** on its chip) recalculates
immediately. Until you have two companies with confirmed data, the screen shows a
plain invitation — *pick at least two companies with confirmed data* — never an
unexplained empty view.

## The Profil view (the default)

With two or more companies selected you land in **Profil**: a single period, all
the KPIs down the rows, the companies across the columns.

- **Rows** are every canonical KPI that has data for at least one of the selected
  companies — including the level-0 valuation ratios computed from prices (P/E,
  P/BV, EV/EBITDA…).
- **Period selector** (**Okres**) — one period at a time, defaulting to the
  latest complete year. Switch it to compare a different year or quarter.
- **Różnica** (*Difference*) column — shown **only when exactly two companies**
  are selected. It expresses the gap as a **multiple** for monetary figures (e.g.
  `9.0×`), in **percentage points** for ratios and margins (e.g. `+5.8 p.p.`),
  and a dash (**—**) where the two values aren't meaningfully comparable.
- Every value carries an **evidence link** (**⧉**) — click it to jump to the
  underlying fact, its source report, and its validation status.

## The Trend view

Toggle **Widok: Profil | Trend** to switch to **Trend**: one KPI across the
period axis, so you watch a single metric move over time for all the selected
companies at once.

- Pick the KPI and the period type (annual/quarterly) from the chips.
- A **multi-series chart** overlays the companies on a shared scale, series
  colors matching the chips and the table.
- The chart draws **at most four colored series**. If you compare more than four
  companies, the *table* still shows them all — the chart plots the first four
  and says so explicitly, rather than turning into an unreadable tangle.

## Fundamentals: one company's periods × deltas

The same comparison engine also powers a single-company table inside a company's
own **Fundamentals** panel (open a company → **Dashboard** → **Fundamentals**):
**Pozycje × okresy** (*line items × periods*).

Each line item is shown across the recent periods with its **QoQ** (quarter over
quarter) and **YoY** (year over year) change inline — percentages for monetary
items, percentage points for ratios, colored by direction. Every number links to
its source fact. The deltas are computed from the confirmed facts, not by
re-reading the reports, so they stay consistent with the rest of the app.

## Comparative valuation (level 1)

Below the comparison, Compare shows **Wycena porównawcza L1** — comparative
valuation, level 1 — for **one company at a time** (defaulting to the first in
your set; a selector switches the scope). It places that company against its
sector peers:

- **Percentile chips** — where the company sits among its peers on each level-0
  multiple, e.g. *P/E: p37 among peers*. Every chip shows the **peer count
  (N)** it was computed from — a percentile is never shown without saying how
  many companies stand behind it.
- **Football field** — one horizontal bar per method (P/E × peer median,
  EV/EBITDA × median, P/BV × median), each showing the **implied price range**
  from applying the peer median multiple, with a marker for the **current
  price**. You see at a glance where today's price falls relative to what each
  method implies.
- **Confidence grade (Wiarygodność) A–D** — a single letter summarizing how much
  weight the section deserves. Its components are shown in the tooltip: **data
  completeness**, **peer depth**, **convergence** of the methods (how tightly the
  ranges agree), and the **validation state** of the inputs.

Everything here is decision context, never a recommendation: the ranges and
percentiles state facts, with zero "cheap/expensive" language. Each computed
multiple traces its inputs (the fact, the price quote, and the FX rate used), and
every run is recorded append-only so the numbers behind a past decision stay
reproducible.

### The thin-peer threshold (N ≥ 4)

Comparative valuation needs enough peers to be honest. When a company's sector
has **fewer than four** tracked peers, the section does **not** quietly vanish —
it states the reason and the threshold, e.g. *N=2 — too few peers for percentiles
and median multiples (threshold: 4)*. The comparison table and the trend chart
keep working from two companies; only the percentiles and median-based ranges
wait for a deeper peer set. If a company has **no sector** at all, the
percentiles are skipped with that reason named explicitly.

## When something is missing (typed gaps)

Compare never leaves a value silently blank — every gap is a **typed, translated
chip** that says what is missing, never a raw error:

- **brak danych** (*no data*) — the company has no confirmed fact for that KPI in
  that period.
- **brak kursu FX** (*no FX rate*) — a company that reports in a foreign currency
  needs an NBP exchange rate for the date, and none was available; the app flags
  the cell instead of guessing a PLN number.

### Foreign-currency companies (EUR → PLN)

A company that reports in euros carries an **EUR→PLN** chip. Its figures are
converted to PLN using the **NBP mid rate**, so all companies compare in one
currency. The conversion basis is shown in the chip's tooltip: **flow** figures
(revenue, profit) use the **period-average** rate, **stock** figures (assets,
equity) use the rate **at the period end**. Ratios are never converted — they are
already unit-free. When the rate for a needed date is missing, you get the *brak
kursu FX* flag rather than a silent conversion.

## Privacy

The comparison, the valuation runs, and every figure behind them are computed
**locally** from data already on your machine. Nothing about which companies you
compare, or the results, leaves your computer.
