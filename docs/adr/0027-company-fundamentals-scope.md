# ADR 0027: Company Fundamentals Scope And KPI Taxonomy

## Status

Accepted.

## Context

Brawler tracks official company reports as feed items and evidence, but the numbers inside quarterly and annual reports stay locked in unstructured documents. The next product direction adds a fundamentals domain: structured financial facts per company, a fixed KPI taxonomy plus custom per-company KPIs, and simple KPI-over-time charts, with AI-assisted extraction from report documents.

This needs an explicit scope boundary before implementation. Brawler is not a portfolio tracker, trading tool, or market dashboard, and the roadmap excludes price/volume/technical charts. Report-derived fundamentals are decision support grounded in the same source-and-citation discipline as the rest of the app, not a market-data product. The boundary between the two must be unambiguous so later work does not drift into price tooling.

The feature also forces an open-core question: AI extraction calls a provider, and the app must decide whether that is a paid capability.

## Decision

### Scope

- Report-derived fundamental KPIs are in scope: values sourced from a company's periodic reports (and equivalent official disclosures), tracked per fiscal period, charted over time.
- Out of scope: price and volume series, technical-analysis indicators, market dashboards, valuation ratios that require live price data, and any screener over the broader market. These remain excluded as in `roadmap.md` "Not In V1".
- Every financial fact is an evidence-linked entity with provenance (source document reference, `extraction_method`, and confirmation state). A fact is never presented without a traceable source.
- Fundamentals are decision support, not advice. No buy/sell/hold or valuation recommendation is generated, consistent with [ADR 0016](0016-provider-neutral-ai-analysis-framework.md).

### KPI taxonomy

A single fixed list does not fit every company, so the taxonomy is organized as app-owned **canonical packs** plus custom per-company KPIs, selected per company by a relevance profile. Each company has a **statement type** — industrial, bank, insurer, specialty-finance, or REIT — that selects which packs apply; the universal pack applies to all. (The detailed cross-sector KPI study behind these packs is owner-private.)

Canonical packs (each entry has a stable key, value type, measure window, and display unit):

- **Universal** — apply to virtually any company: `net_profit`, `total_assets`, `total_equity`, `eps` (basic and diluted), `shares_outstanding`, `operating_cash_flow`, `cash`, `dividend_per_share`.
- **Industrial/commercial** — for companies with a conventional income statement: `revenue`, `gross_profit`, `operating_profit` (EBIT), `ebitda`, `net_debt`, and the margins `gross_margin`, `operating_margin`, `net_margin`.
- **Cash flow** — applies to virtually any company (cash is king for quality analysis): `investing_cash_flow`, `financing_cash_flow`, `capex` (reported), and `free_cash_flow` (derived = `operating_cash_flow` − `capex`).
- **Capital efficiency** — all derived: `roe`, `roa`, `roic`, `roce`, `net_debt_to_ebitda`, `fcf_conversion`. These power quality/return analysis and feed the quality-framework checks (the quantitative rule engine in the quality-frameworks epics).
- **Financial-sector** — financial companies report different IFRS statements; the company's statement type selects the sub-pack:
  - *Insurance* (PZU): `gross_written_premium`, `net_earned_premium`, `gross_insurance_revenue`, `claims_ratio`, `combined_ratio`, `technical_result`, `investment_result`.
  - *Banking* (PKO, Pekao): no revenue/EBITDA at all — `net_interest_income`, `net_fee_commission_income`, `operating_income`, `operating_expenses`, `net_profit`; the scale metrics are `total_assets`, `loans`, `deposits`; ratios `nim`, `cost_income_ratio`, `npl_ratio`, `cost_of_risk`, `roe`, and capital ratios (`cet1`, `tcr`, `tier1`).
  - *Specialty finance* (Kruk): an IFRS-9 model where recoveries are not revenue — `recoveries`, `erc` (estimated remaining collections, a multi-year forward figure), `cash_ebitda` (non-IFRS), and portfolio purchases (an investing figure reported as a lead operating KPI).
  - Brokerage/exchange (XTB, GPW) lean on the universal pack plus `operating_income` and custom KPIs (active clients, profit-per-lot, turnover, listed companies).
- **REIT / real estate** — for property trusts (e.g. Realty Income), where net income understates the economics: `ffo`, `affo`, `affo_payout_ratio`, `occupancy`, `properties_count`, `same_store_noi`, `walt` (weighted average lease term); dividends may be paid monthly.

Rules:

- Margins and other ratios may be stored directly when reported or derived from other facts; a derived value records how it was produced.
- **Derived metrics** are first-class: a definition can be `reported` (extracted from a document) or `derived` (computed by a formula over other metric keys, e.g. `free_cash_flow = operating_cash_flow − capex`, `roic = nopat / invested_capital`, `net_debt_to_ebitda = net_debt / ebitda`). Derived values are computed at read time from confirmed input facts, are unavailable for a period when an input is missing, and carry the formula plus the input facts as provenance. Return ratios that conventionally use a trailing-twelve-month flow and average balance (ROE, ROIC) record that window and averaging convention in the definition; the read model computes TTM by summing the last four quarterly flow facts and averaging the relevant stock facts.
- EBITDA and similar are frequently company-defined alternative performance measures (cyber_Folks reports "EBITDA Operacyjna"); `extraction_method`/provenance records reported-as-APM versus derived.
- Custom per-company KPIs are supported from day one for the operating metrics that actually differentiate a company — these are usually the ones management leads with (cyber_Folks GMV and ARPU LTM, XTB active clients and profit-per-lot, Synektik backlog, CD Projekt lifetime units sold, Diagnostyka collection points):
  - a custom KPI is scoped to one company and independent across companies (company A's `subscribers` is unrelated to company B's)
  - it carries a namespaced user key, a display name, a value type, a measure window, and a display format
  - custom KPI facts follow the same provenance, confirmation, and dimension rules as canonical facts
- App-seeded sharing (canonical and sector packs) is in scope now. User-driven promotion of a company KPI to shared scope and a user-managed shared vocabulary are deferred until there is real product pressure, mirroring the source-trust taxonomy approach in [ADR 0020](0020-sources-visibility-and-directory-boundaries.md).
- Industry-specific classified stocks — mining/O&G reserves and resources, with proven/probable, 1P/2P/3P, or measured/indicated/inferred categories — are represented as custom KPIs (e.g. `reserves_2p`) rather than core enums, because the category vocabulary differs by industry.

### Model shape (three layers)

KPI relevance, definitions, and values are deliberately separated so the model stays flexible as a company's reporting evolves. Fusing them is what would make it rigid.

1. **Catalog (definitions)** — what a metric *is*: key, label, value type, measure window, unit. Each definition carries a `scope`: `canonical` (app-owned, global), `sector` (shared within a sector, e.g. the insurance pack defines `combined_ratio` once for all insurers), or `company` (bespoke, e.g. cyber_Folks `gmv`). Custom KPIs default to company scope; a definition can later be promoted to a shared scope without a migration. This is why the catalog need not choose between company- and sector-orientation — scope is data, and both coexist.
2. **Relevance (selection over time)** — a lifecycle-tracked link between a company and a definition: `status` (active/archived), `source` (user, agent, or sector-default), first/last-seen period, and `rank` (primary/secondary). KPIs can appear, be reprioritized, or be retired as the company shifts focus, without data loss — an archived KPI keeps its historical series and is shown as discontinued. The experience is company-oriented: each company has its own profile assembled from this layer.
3. **Facts (values)** — reference the *definition*, never the relevance profile. Changing what is relevant therefore never orphans or deletes a value, and the agent may persist an extracted fact before the user has curated its KPI into the profile.

All three layers are surfaces the agent operates on under the autopilot trust ladder: propose a definition, propose a relevance entry, and propose a fact value — each confirmed by the user before it is trusted.

### Per-company KPI relevance profile

KPI relevance is company- and sector-specific: revenue is meaningless for a pre-revenue company, and applying every canonical key everywhere produces noise. Each tracked company therefore has a **relevance profile** — the subset of canonical keys that matter for it plus its custom KPIs, optionally ranked primary/secondary. The profile is assembled from three signals:

1. what the company self-declares — "Selected KPIs" sections, management commentary, and segment tables in its reports and presentations, which the agent extracts and proposes;
2. the sector default pack;
3. user curation (confirm, add, drop, reorder).

The agent may both propose values for known KPIs and propose new custom KPI definitions it discovers in a report; new definitions are confirmed by the user before they become part of the profile.

### Fact attributes and dimensions

A financial fact value is decimal-exact (stored as text, parsed as a decimal — not binary float) so confirmation against the source and cross-checks against external data are exact. A fact is identified and qualified by:

- `metric_key` (canonical or custom), `period`, and `company`
- `statement_basis`: consolidated or standalone (default consolidated)
- `attribution`: total, owners-of-parent, or non-controlling-interest — material for acquisitive groups (cyber_Folks net profit is 41.2M but only 22.9M is attributable to shareholders; EPS uses the parent figure)
- `value_kind` + `unit`: a numeric kind (monetary, percentage, ratio, count, physical, duration) plus an explicit typed unit — currency (PLN/EUR/USD/DKK/KRW), mass (t, t/day), area (m²), energy (TWh, GW), count (stores, units, beds, clients), per-unit ratio (PLN per lot, $ per lb), or duration (months of runway, years for WALT, $/day). Unit is a typed field, not a fixed enum.
- `currency`: per fact, and may differ from the company's main reporting currency — ASEE reports in EUR, Text's operating KPIs are in USD while its financials are PLN, Scanway and Newag carry EUR backlog; foreign companies span USD, EUR, DKK, and KRW (in trillions).
- `reporting_standard`: the accounting framework — IFRS, US-GAAP, or a local GAAP (K-IFRS for SK Hynix). Usually a company-level attribute, recorded so cross-standard comparisons can flag reduced comparability (US-GAAP has no standard EBITDA; IFRS and US-GAAP differ on leases, R&D capitalization, and REIT measures).
- `variant`: the flavor of the metric — reported, adjusted/non-IFRS or non-GAAP (Asseco and Vercom report adjusted EBIT/EBITDA; US tech reports large GAAP vs non-GAAP gaps), constant-currency / CER (LPP, Microsoft, and Novo Nordisk report at constant exchange rates), continuing versus discontinued operations (Creotech after its spin-off), net-of-cancellations (developer unit sales), or inventory-valuation basis (LIFO / clean CCS, as refiners like Orlen report EBITDA LIFO). The same metric_key, period, basis, and attribution can carry more than one variant.
- `measure_window`: flow (during the period), point-in-time (at a date, e.g. backlog, ERC, land bank, store count), trailing (e.g. ARPU LTM), or cumulative (e.g. lifetime units sold).
- `data_quality` and supersession: a value is estimated/preliminary or reported-final. Estimates are routinely published in standalone releases before the final report (Grupa Kęty every quarter; many retailers, developers, and banks). The estimate and the later final value for the same metric/period/dimensions are both retained; the final supersedes the estimate for display while the estimate history is preserved.
- value stored in base units, with the as-reported form and scale (e.g. "245 253 tys. zł") kept in provenance so the display matches the source document.
- signed values are allowed — loss-making and pre-revenue companies (Creotech, Scanway, DataWalk) report negative EBITDA/net result, and the relevance profile simply drops metrics that are not meaningful (e.g. revenue multiples for a pre-revenue company).
- a segment label is a compatible future extension; v1 stores the top-level total and represents segment-specific metrics (hotel RevPAR, DOOH screens) as custom KPIs. Holdings whose segments have entirely different KPI vocabularies (GK Immobile, Digital Network) are handled this way.

### Period model

- Granularity includes monthly (Inter Cars publishes monthly sales), quarterly, half-year, nine-month, and annual; calendar quarters are not assumed.
- Fiscal calendars may be offset — Text runs April–March, LPP historically February–January, Synektik an offset year — so a company carries a fiscal-year-end offset and periods store fiscal labels independent of calendar dates.
- Discrete-period and year-to-date figures coexist for the same metric (the Polish ESPI convention) and are stored distinctly via `measure_window`.
- Forward-looking pipeline metrics (order backlog at Budimex/Newag/Elektrotim/Asseco, ERC at Kruk, land bank and units-in-offer at developers) are point-in-time facts; company guidance/targets are a separate later concept, not a fact.

### Open-core boundary

- AI fundamentals features (extraction and any later AI assistance over fundamentals) are part of the open core and free to use with a user-supplied provider API key. They are not gated behind a license token.
- The named future paid areas remain managed AI (provider access without the user supplying a key), cloud sync and backup, and official signed installers. These are recorded as direction only; no pricing, packaging, or entitlement detail belongs in public docs.
- This does not change the entitlement module or [ADR 0017](0017-license-gate.md). The fundamentals milestones add no new gated entitlements.

## Consequences

- Implementation milestones can proceed against a decision-complete domain: financial periods, financial facts (with statement-basis, attribution, value-type, measure-window, and data-quality dimensions), KPI definitions across canonical packs plus custom, and per-company relevance profiles.
- The financial-sector pack lets PZU-style insurers be modeled now; brokerage/exchange names rely on the universal pack plus custom KPIs until a richer sector pack is justified.
- Because relevance is per-company and report-derived, the fundamentals view shows what matters for each company rather than a fixed grid; the agent's ability to propose both values and new KPI definitions is what makes this scale and is a prerequisite for the autonomous pipeline.
- Operating KPIs typically live in narrative and presentation documents rather than the IFRS statement tables, so report-document persistence and extraction must cover those documents, not only the financial statements.
- The model generalizes to foreign markets (US, EU, and others) with no structural change: per-fact currency and reporting standard, fiscal-year offsets, the constant-currency variant, statement-type packs (incl. REIT), and derived ratios (book-to-bill, reserve-replacement) all carry over. The v0.34 schema reserves the seams — `reporting_standard` and the REIT statement type are added now even though v1 is GPW-first. Deferred to a future multi-market milestone (north-star, not v1): FX normalization to a display currency for cross-company comparison, locale-aware and multilingual extraction, and a company identity/listing model (exchange, ISIN, dual listings/ADRs). None requires a later migration of the fact model.
- The price/market boundary is explicit, so AI extraction, charts, and any external fundamentals source (for example a future EODHD study) stay on the report-derived side of the line or require a deliberate ADR to cross it.
- Charts render report-derived fundamentals only; reusing the chart primitives for price series would be a scope change requiring a new decision.
- Keeping AI fundamentals free preserves the local-first, BYO-key posture and draws the paid line at managed infrastructure rather than at core research capability.