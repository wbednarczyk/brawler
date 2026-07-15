# ADR 0067: Market Data Foundation — EOD Quotes, Sector Classification, Level-0 Market Ratios

Status: Accepted

> **Amended 2026-07-14 by [ADR 0082](0082-market-data-source-selection.md).** Decision #1 below named **Stooq** as the EOD source; live probes showed Stooq now gates automated access (proof-of-work) and BiznesRadar's quote-history path is robots-disallowed. The source is now **Yahoo Finance v8 (primary), with BiznesRadar/Bankier/StockWatch as robots-allowed quote witnesses** (the Twelve Data secondary was selected and then removed the same day — GPW is paid-plan-only there; ADR 0082 amendments) — the source-neutral adapter boundary anticipated exactly this swap, so decisions #2–#5 (storage, sector, ratios, framing) stand unchanged. See ADR 0082 for the probe evidence and the deliberate ToS-gray acceptance of the keyless Yahoo primary.

The app stores no market prices and no company sector classification. Without them the valuation & decision arc cannot deliver its promise: fair value without a current price yields no upside, the screener cannot say "cheap/expensive", and peer sets for comparison/multiples have no data basis. This ADR is also the source ADR resolving the roadmap's open Stooq question.

## Context

- A 2026-07-03 audit confirmed zero price/quote data anywhere in the data model, and "sector" existing only as a KPI-definition scope (ADR 0046) — not as a company attribute.
- Downstream dependents: comparative valuation (v0.60), DCF upside (v0.61), screener/heatmap (v0.63), price alerts (ADR 0068), calibration loop and framework backtest (north stars) — the last two need **historical** prices, which cannot be backfilled from a from-now-only feed cheaply later.
- Brawler is decision support, not a trading tool: end-of-day granularity is sufficient; realtime is a non-goal.

## Decision

1. **EOD price source** (~~Stooq~~ → **Yahoo v8 primary**, [ADR 0082](0082-market-data-source-selection.md)) as a new `market_data`-type source adapter. Conservative, policy-clean fetching: one full-history backfill when a company is added (throttled), then one pull per company per day after session close via a durable-queue job, with robots-allowed witnesses cross-checking the close. Source attribution and fetch timestamps preserved like every adapter. The adapter boundary allows a replacement without touching consumers — exercised here (the original Stooq choice was swapped after probes without disturbing the storage/ratio/UI decisions below).
2. **`daily_quotes` storage, append-only**: `(company_id, date, open, high, low, close, volume)`, unique on `(company_id, date)`; corrections overwrite by key, never rewrite history wholesale. Full available history ingested from day one (enables 52-week stats, percentiles, and future backtests/calibration).
3. **Sector classification on the company**: a sector/industry field in the company registry populated from the GPW/NewConnect directory data where available, with a manual override (`source: registry | manual`). This feeds peer-set derivation for v0.60/v0.61; the existing KPI-definition `sector` scope (ADR 0046) keys off the same taxonomy.
4. **Level-0 market ratios as canonical derived metrics** evaluated by the existing derived-metrics engine (ADR 0046, scope `canonical`): market cap (`close × shares_outstanding`), P/E, P/BV, EV/EBITDA, dividend yield, FCF yield, 52-week high/low distance, and own-history valuation percentile. Every ratio traces to its inputs (quote date + confirmed facts) — sourced-and-cited like all fundamentals.
5. **Decision-support framing**: ratios and percentiles state facts and context; no cheap/expensive verdicts, no buy/sell/hold language (CLAUDE.md rule; enforcement posture per ADR 0042).

## Consequences

- The valuation arc (v0.60–v0.63) gains its missing data substrate; price alerts (ADR 0068) become possible.
- New surface in the company workspace: price context beside fundamentals (chart reuses `TrendChart`).
- Daily quote history grows the DB modestly (~250 rows/company/year); retention is not needed — quotes are the cheapest data the app stores.
- Real-data validation (docs/testing.md): ticker-mapping and ratio correctness are validated against the maintainer's tracked companies before the feature is called complete.
