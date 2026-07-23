# ADR 0058: Investor Week Calendar — Event Layers, Whole-Market Scope, Macro & Holiday Domains

Status: Accepted (2026-06-30)

Extends [ADR 0034](0034-espi-event-classification.md) (signal→event derivation, event types) and the Company Events Calendar in [product-spec.md](../product-spec.md). Relates to [ADR 0044](0044-report-season-cockpit.md) (the report-specific cockpit read model) and the accepted GPW/Bankier calendar adapters in [source-strategy.md](../source-strategy.md).

## Context

The maintainer wants a weekly "investor week" digest in the spirit of the Koomberg (Mariusz Hojda) calendar posted on X: a single working-week view (Mon–Fri columns) that answers *"what matters this week"* — company reports, IPO debuts, ex-dividend (`ODCIĘCIE DYWIDENDY`) dates, macroeconomic releases (CPI, PMI, payrolls, JOLTS, ADP) with their time and country, and market holidays (`USA WOLNE`). The UI is ours; the **functional and UX target** is that digest.

Brawler already ships most of the skeleton. The **Company Events Calendar** (product-spec) is watchlist-first, date-first, and renders the current week as working-day columns with prev/next navigation — the same shape. `company_events` carries report, dividend, shareholder-meeting, conference, and corporate-action types; the `gpw-market-events-rss` and `bankier-kalendarium-html` adapters ingest them. So this is **not "build a calendar"** — it is *extend the existing calendar with three new content layers and one scope toggle*.

What is genuinely new (confirmed against the code/docs):

- **Macro and holidays have no company.** `company_events.company_id` is required and "events belong to exactly one canonical company." Macroeconomic releases and exchange holidays are market-wide — they do not fit `company_events` and must be their own domains.
- **Both calendar adapters are watchlist-gated** (`Scope: tracked GPW companies only`, exact-ticker matching). A whole-market layer (every GPW debut/dividend that week, not only tracked tickers) is a real scope change, not a config flag.
- **Event-type coverage is partial.** `bankier_calendar.rs` maps only `dywidendy → dividend` and `wza → shareholder_meeting`; report dates, IPO debut, and the ex-dividend (ex-date) semantics are intended-but-unmapped.
- **No clean local macro source exists.** Aggregated economic calendars are paid (Trading Economics, FMP) or fragile/restricted scraping (investing.com) — against the `AGENTS.md` source policy ("prefer official/public/RSS; avoid fragile or restricted scraping; no paid APIs without an ADR").

Foreign/international earnings (the Koomberg "globe" row — Nike, FactSet, GM) are **explicitly out of scope** for this epic: Brawler is GPW-first and a foreign-earnings adapter is a separate source expansion.

## Decision

Build an **Investor Week Calendar** by extending the existing Company Events Calendar with composable **layers**, unioned at read time. Default scope stays **watchlist-first** with an opt-in **whole-market** toggle; macro and holiday layers are opt-in toggles on top.

### 1. The week is a backend-owned read model that unions layers

A new `list_investor_week(input)` read model assembles working-day columns (Mon–Fri; weekend columns only when populated) by **unioning** the layers below from their canonical domains — **no stored weekly projection**, mirroring the `list_report_season` pattern ([ADR 0044](0044-report-season-cockpit.md)). `input` carries the week anchor date, the scope (`watchlist | market`, optional `watchlistId`), and which layers are enabled. Each column groups items by layer (company / macro / holiday) so the UI can render lanes. Calendar freshness is surfaced per layer (a stale layer is visible, never silently empty).

The existing **Events** view (a cockpit panel since ADR 0054/0057) is the host; the report-season cockpit ([ADR 0044](0044-report-season-cockpit.md)) stays a separate report-focused surface and is **not** duplicated here.

### 2. Company layer — new event types, fuller adapter mapping

`company_events` is extended (no schema change — `event_type` is a string) with:

- `ipo_debut` — primary-market debut (`DEBIUT` / `NEW ARRIVAL`).
- `ex_dividend` — the **ex-dividend (cut-off) date** (`ODCIĘCIE DYWIDENDY`), distinct from the existing `dividend` (record/payment) type so the calendar can show the date that actually moves the price.

The `bankier-kalendarium-html` adapter mapping is widened to emit `periodic_report` (report dates), `ipo_debut`, and `ex_dividend` in addition to today's `dividend`/`shareholder_meeting`. Ex-dividend derivation from ESPI dividend filings (ADR 0034 `event_derivation`) is extended to emit the ex-date when the body states it.

### 3. Whole-market layer — a separate `market_calendar_events` domain

The canonical-company invariant on `company_events` is **kept** (it backs report-season and entity resolution). Whole-market rows — events for **untracked tickers** — go in a new `market_calendar_events` table (ticker + issuer name, **no** `company_id`), populated by a relaxed whole-page Bankier kalendarium ingest that is **opt-in** (only fetched when the user enables the market scope). The week read model unions tracked `company_events` ∪ untracked `market_calendar_events`, **deduped by ticker** so a tracked company is never shown twice; a market row whose ticker matches a tracked company links into its workspace. Cache-first week navigation (the existing Bankier dated-week pattern) applies.

### 4. Macro layer — `macro_events` domain, live source deferred

A new `macro_events` table (no company): indicator key + title, `country`, `event_date`, `event_time`, `importance`, optional `actual`/`forecast`/`previous`, source provenance, and a `manual` flag. **This milestone ships the model, manual add/edit, a sample/seed set, and the macro lane in the week view — but no live macro source.** Choosing a policy-clean live feed (official primary calendars — GUS, NBP, US BLS, Fed — where machine-readable, vs. a curated dataset) is **deferred to a follow-up ADR**; macro releases are low-volume and regular, so a curated/official-primary path is viable and a fragile/paid aggregator is rejected up front. This is the "build the layer first, pick the source later" posture the maintainer chose.

### 5. Holiday layer — `market_holidays` domain, curated static dataset

A new `market_holidays` table (`market`/exchange, `holiday_date`, `name`, `closed | half_day`), seeded deterministically for the relevant exchanges (GPW; US NYSE/Nasdaq) as a curated, refreshable dataset — **not** a live source. The week view renders a per-market `WOLNE` badge on closed days. Reads of holiday data tolerate a missing/empty table (safe default = no holidays) so an un-seeded year never crashes the week view.

### 6. Scope & layer toggles persist in user settings

The active scope (`watchlist | market`) and enabled layers (macro, holidays) persist in `user_settings` (the pattern used for pinned companies in ADR 0054) with tolerant defaults, so the digest opens the way the user left it.

## Consequences

- **Mostly additive.** One read model + three small domain tables + adapter mapping widening + view layering. `company_events`, report-season, and entity resolution are untouched structurally.
- **Source policy stays intact.** No paid API, no fragile scraping is added; macro's live source is an explicit later decision, holidays are curated, whole-market reuses the accepted Bankier adapter.
- **Narrow-window constraint applies** — day columns and layer lanes must stack/degrade in tall-narrow windows (Playwright viewport matrix).
- **Real-data validation** before locking the whole-market dedup-by-ticker and the adapter mapping (per the real-data-validation guardrail, [ADR 0045](0045-guardrail-harvest-loop.md)).
- Migrations are append-only and idempotent (`CREATE TABLE IF NOT EXISTS`, tolerant reads), per `AGENTS.md`.

## Build order (slotted as milestone `v0.67.0`)

1. Read model + view layering: `list_investor_week`, the Mon–Fri layered week, scope + layer toggles (watchlist-first default).
2. Company layer event types `ipo_debut` + `ex_dividend`; widen the Bankier mapping + ESPI ex-date derivation.
3. Whole-market layer: `market_calendar_events` + relaxed opt-in whole-page ingest + dedup-by-ticker union.
4. Holidays: `market_holidays` + curated GPW/US seed + `WOLNE` badge.
5. Macro: `macro_events` + manual add/edit + sample seed + macro lane (live source deferred to a follow-up ADR).

Each task ships its tests (migration invariants, adapter mapping, read-model union, view behavior across the viewport matrix) and a reachable UI entry point, per the Definition of Done.

## Alternatives considered

- **Generalize `company_events` to a polymorphic calendar** (nullable company, a `layer` column for macro/holiday). Rejected: it breaks the strong one-canonical-company invariant that report-season and entity resolution rely on, and forces company-shaped fields onto company-less rows.
- **A single aggregated economic-calendar adapter for macro.** Rejected up front: paid or fragile/restricted scraping, against source policy.
- **Foreign/international earnings layer** (the Koomberg globe row). Out of scope this epic — a separate GPW→foreign source expansion.

## Status notes

Accepted 2026-06-30 after an inspiration-driven scoping discussion. Locked: watchlist-first + opt-in market layer; layers = macro (model + manual + sample now, live source deferred to a follow-up ADR), holidays (curated static), IPO debut + ex-dividend; foreign earnings out of scope. Slotted as a new milestone `v0.59.0` (the booked arc runs through `v0.58.0`; no renumbering); re-slotted to `v0.66.0` at the 2026-07-03 numbering finalization (stale `v0.59.0` references in the propagated docs corrected 2026-07-19), then to `v0.68.0` at the 2026-07-20 arc re-sequencing (ADR 0084 retirement + `v0.60.0` Today reinvention + `v0.61.0` MCP surface v2 inserted ahead), then to `v0.67.0` at the 2026-07-22 v0.60 planning (MCP surface v2 folded into `v0.60.0`, arc shifted back one; [roadmap](../roadmap.md), ADR 0088). Docs propagated in the same planning step: roadmap, data-model, contracts, source-strategy, ui-flows, ui-information-architecture. Implementation deferred — sequenced after the booked milestones.
