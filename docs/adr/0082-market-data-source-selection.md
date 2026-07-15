# ADR 0082: Market-Data Source Selection — Yahoo primary, robots-allowed witnesses (Twelve Data secondary removed by amendment)

Status: Accepted

[ADR 0067](0067-market-data-foundation.md) decided a market-data foundation and named **Stooq** as the EOD price source (automated per-ticker CSV pull). Live source probes on 2026-07-13/14 invalidated that premise: Stooq now gates automated access, and the other free Polish portals either forbid the quote-history path or serve nothing machine-readable. This ADR is the source ADR that selects the concrete provider(s) and records the rejected options with evidence, so the choice is not re-litigated. It resolves the source question ADR 0067 left to a follow-up.

## Context — probe evidence (2026-07-13/14)

| Source | Result | Verdict |
|---|---|---|
| **Stooq** (`/db/`, `q/d/l` CSV) | anti-bot **proof-of-work** JS gate (SHA-256 nonce) on both paths; a plain HTTP client cannot pass | ❌ automating = circumventing a protection → outside source policy |
| **BiznesRadar** `/notowania-historyczne`, `/profile-history` | `robots.txt` **Disallow** on exactly the quote-history + chart-JSON paths | ❌ automated history = ignoring robots → not policy-clean |
| **Google Finance** | legacy history endpoints **404** (dead since 2012); current quote behind `consent.google.com` redirect wall + 1.1 MB undocumented blob | ❌ |
| **investing.com** | **403** Cloudflare | ❌ |
| **Bloomberg / MarketWatch / FT** | 403 / 401 / 302 paywall + anti-bot; real data only via paid enterprise terminals | ❌ not free, not local-first |
| **CNBC** open quote API | 200 but empty for `CDR-PL` | ❌ no GPW coverage |
| **Yahoo Finance v8 chart API** | 200 JSON; `<ticker>.WA`; PLN; full history to `firstTradeDate` (CDR/KGH ~6820 bars → 2000; each ticker back to its GPW debut); OHLCV + volume; `regularMarketPrice` cross-validates BiznesRadar to the grosz | ✅ |
| **Twelve Data** | public `/stocks?country=Poland` → **910 GPW symbols**; official free tier (800 req/day, 8/min). **Amended 2026-07-14 (live smoke `make smoke-twelvedata`)**: `time_series` for GPW returns `404 "available starting with the Grow or Venture plan"` — **GPW quote data is paywalled**; the free tier covers metadata only. The original probe validated `/stocks`, never a real pull. | ⚠️ paid-only for GPW |
| **BiznesRadar `/notowania`, Bankier, StockWatch** (robots-allowed) | 200, no anti-bot, clean current-quote fields | ✅ (witnesses only) |

Owner constraint (2026-07-14): Brawler must be an **automaton — no manual import, ever**; exhaust automatic, policy-clean sources rather than fall back to a human step.

## Decision

1. **Primary: `yahoo-eod`** — Yahoo Finance v8 chart API, `query1.finance.yahoo.com/v8/finance/chart/<ticker>.WA`, keyless. `period1=0&period2=…&interval=1d` for full backfill; `range=5d` daily. This is Yahoo's own **undocumented** website endpoint: their public finance API was retired in 2017, their ToS prohibits automated access + data **redistribution**, and they may add crumb/cookie auth or rate-limit (429/999) without notice. We accept this **ToS-gray** posture deliberately and narrowly — **local-first, personal, EOD-only, watchlist-only, zero redistribution** — the same low-risk personal use that the widely-used `yfinance` ecosystem relies on. "Pęknąć może wszystko"; resilience comes from the official secondary, not from pretending the primary is licensed.
2. **Secondary/fallback: none (Twelve Data selected, then REMOVED — both on 2026-07-14).** The original decision named Twelve Data's official free tier as the fallback. The first live smoke falsified that premise: GPW `time_series` is **paid-plan-only** (`404 "available starting with the Grow or Venture plan"`; the free tier covers the `/stocks` metadata list, not quotes). A fallback that cannot fire without a paid plan is dead weight and a false sense of resilience, so the owner removed the adapter entirely (code, descriptor, credential form, seeded row via migration 0076). Today a Yahoo failure raises a source-health diagnostic and skips the company for the run; the self-heal `quote_backfill` catches history up on recovery. A **free degraded fallback** (Bankier/StockWatch robots-allowed current close; a real-browser re-probe of GPW's own archive, which TLS-resets plain HTTP clients) is tracked as card `ee81afe` on the v0.55 source-reliability epic. Re-adding a paid Twelve Data integration would need a paid-API ADR.
3. **Witnesses (compare-only, never write quotes):** BiznesRadar `/notowania/<slug>`, Bankier, StockWatch — all robots-allowed HTML with clean current-quote fields. Cross-check the latest daily close; divergence beyond tolerance raises a **source-health diagnostic**, never a silent overwrite.
4. **Mitigations (binding):** watchlist-only, throttle + jitter, 429/999 backoff, aggressive cache (never re-fetch an unchanged bar), honest User-Agent, source-health **kill-switch** (no automatic fallback provider today — see decision 2). No mass crawl, no non-watchlist tickers.
5. **Source-neutral boundary:** the `market_data` adapter surface (ADR 0067) keeps the provider swappable; EODHD (PoC `212f9c1`) remains a drop-in candidate for multi-market without touching consumers.

## Consequences

- ADR 0067 decision #1 is amended (Stooq → this selection); `docs/source-strategy.md` §"Price And Fundamentals Context Sources" updated to match.
- Ticker→provider mapping (`<ticker>.WA` for Yahoo; plain ticker for TD) is validated against the owner's full real watchlist before closure (v0.53 T6); unmapped tickers surface as source-health warnings, never silent.
- BiznesRadar's other robots-allowed data (financial statements, `/rekomendacje`, `/akcjonariat`, `/dywidenda`, `/wskazniki`) is accepted as a source for other milestones (v0.55 fundamentals witness, v0.58 recommendations, v0.56 ownership, v0.57/v0.66) — each still gated on its own source-policy note. **Correction to card `cf0ea94`:** BR's free/anonymous access is **HTML only** (CSV export is Premium/login-gated), so the fundamentals witness parses allowed HTML via the existing aggregator tier, not a CSV endpoint.
- Rejected sources are recorded above with evidence; do not re-evaluate without new facts.
