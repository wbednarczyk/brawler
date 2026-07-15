# Price context, sectors, and company basics

When you track a company, Brawler now pulls its **daily closing prices**
automatically and turns them into the picture you actually reason about: where
the price sits in its year, what it's worth, and the level-0 valuation ratios —
all beside the reported fundamentals, all **local**, all **decision support
only** (Brawler never tells you to buy, sell, or hold).

It is all automatic: there is **no manual import**. Track a company → its price
history backfills from its market debut, and each session-close day appends one
bar on its own.

## Where the data comes from

- **Prices:** Yahoo Finance end-of-day quotes for GPW tickers (`<ticker>.WA`),
  in PLN, from the company's first trading day. Keyless, watchlist-only,
  throttled. If Yahoo is briefly unreachable, Brawler records a source-health
  note and skips that day; the history self-heals on the next successful pull.
- **Sectors:** classified automatically from the GPW / NewConnect company
  directory. You can override a wrong one by hand (below); a directory refresh
  never overwrites your override.

_(A paid Twelve Data fallback was evaluated and dropped — GPW quotes need a paid
plan there, so it added no free resilience. A free degraded fallback is planned
for a later release.)_

## The Price context section

Open a company's dashboard → the **Fundamentals** panel leads with **Price
context**:

- **Latest close** and the day's change (absolute + percent), colored
  up/down.
- **52-week range** — the high and low with your distance from each, plus
  **market cap** (once shares outstanding is recorded).
- **Level-0 ratios**, computed from the latest close × your confirmed
  facts: **P/E, P/BV, EV/EBITDA, dividend yield, FCF yield**, and **price vs
  52-week range (percentile)** — where today's price sits within its own
  trailing year (only shown once there are at least 20 sessions of history, so
  it's context, not noise).
- A **candlestick chart** of the session history, with a round-number price
  scale (e.g. 80 / 100 / 120 / 140 PLN) and the covered date range.

Any ratio whose inputs are missing shows a dash (`—`), never a guess.

### Ratios compute from whatever you have

Brawler follows a simple rule: **if a ratio can be computed from the numbers you
have, it is — through a fallback chain — and only if nothing works does it stay
empty.** P/E, for example, tries market cap ÷ net profit first, then falls back
to price ÷ diluted EPS, then price ÷ basic EPS. Record more facts and more
ratios light up on their own.

## The Basic info panel

Open a company's dashboard → the **Basic info** panel shows the identity facts
at a glance: **name, ticker, ISIN, sector** (with a chip showing whether it came
from the registry or your manual override), and the latest recorded **shares
outstanding** with its reporting period.

It is **read-only by default** — no edit buttons cluttering every field. To
change the sector or the investor-relations reports URL, click **Edit** once;
the edit fields appear, and **Done editing** puts it back to a clean read view.

### Overriding a sector

In **Basic info → Edit**, the **Sector** field suggests matching values from the
registry as you type (not a wall of every sector — start typing to filter).
Pick one or type your own, then **Save**. **Clear override** reverts to the
registry-sourced sector. Your manual value survives every future registry
refresh.
