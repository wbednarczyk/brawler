# 0083 — Company Health: Piotroski F / Altman Z″ Scores, Insider Sentiment, Red-Flags Panel

Status: Accepted (2026-07-17, owner decisions at milestone planning)

Epic: `318d025` · milestone `v0.57.0` · extends [ADR 0046](0046-quality-frameworks-quantitative.md), [ADR 0061](0061-deterministic-fundamentals-data-gathering.md), [ADR 0068](0068-attention-routing-and-morning-briefing.md), [ADR 0072](0072-ownership-structure.md)

## Context

The app should raise "something smells here" per company on its own. Settled science provides published health formulas (Piotroski F-score 2000, Altman Z-score family 1968/1995, Beneish M-score 1999) — we implement them as-published and cite them, never inventing our own composite. Exploration facts driving the design:

- The ESEF tier extracts 13 IFRS concepts; F+Z additionally need `current_assets`, `current_liabilities`, `retained_earnings`, `long_term_debt`. Beneish needs receivables/SG&A/D&A on top — the weakest coverage.
- The ADR 0046 scorecard emits per-criterion verdicts; it has **no composite-sum primitive**, and its expression grammar has no indicator functions or year-over-year deltas. F/Z are composite scores with per-component explanations.
- Insider filings are category-only signals (`insider_transaction`) — no who/role/direction/volume/price. Body-parsing precedent exists (`fundamentals::ownership::espi_notification`).
- Fund exits are promised as a read-only derivation over `ownership_stakes` ([data-model § Ownership](../data-model.md), interface note); derived non-filing events already flow to alerts via the KNF pattern (feed item + confirmed signal, ADR 0069).
- Bankier kalendarium already lands expected `periodic_report` dates in `company_events`; no expected-vs-actual comparison exists.

## Decisions

### 1. Scope: F + Z″ now, Beneish M deferred

Piotroski F and Altman Z ship in v0.57. **Beneish M is deferred** (owner decision): it requires three additional extraction concepts (receivables, SG&A, D&A) that are often note-level with uncertain ESEF coverage, so most companies would land in `insufficient_data` anyway. Deferral is recorded on the roadmap; the score engine is shaped so M slots in as a third family later.

### 2. Scores are a deterministic Rust computer, not DSL formulas

A new `fundamentals/health` module computes both scores from the existing `MetricsContext` (confirmed facts only, annual FY periods). Rationale: the criterion DSL lacks indicator logic (`1 if ΔROA > 0`), prior-period references, and structured per-component output; forcing F/Z into it would bloat the grammar for one consumer. The computer returns **typed per-component results** — 9 F signals, 4 Z″ inputs — each with its measured values, so the UI explains every point.

**Scoped amendment of ADR 0046's "no built-in synthetic resolvers"**: the score scalars `piotroski_f` and `altman_z` are injected into metric resolution the same way `QuoteFacts` market scalars are (v0.53 precedent), so scorecard criteria can reference them (`piotroski_f >= 7`). The open-catalog rule stands for everything else; health scores are the second (and only other) injection point.

Scores are **computed as-of a period** (pure function over that period's + the prior period's facts), never persisted as facts — recomputing history is free and deterministic, which gives deterioration detection without new storage.

### 3. Strict completeness — full inputs or no headline number

A headline score renders **only when every input is present** (published-formula comparability; cross-checkable against third-party published values). Anything less is an explicit `insufficient_data` state carrying the per-component breakdown: which components computed, which inputs are missing. Never a silently rescaled or partial headline (ADR 0061 "never silently wrong"). Thin history is explicit: F requires the prior FY period; a company with one period shows `insufficient_data (no prior period)`.

### 4. Altman variant: Z″ (emerging markets), financials excluded

One variant for all scored companies: **Z″ = 6.56·X1 + 3.26·X2 + 6.72·X3 + 1.05·X4** (X1 working capital/TA, X2 retained earnings/TA, X3 EBIT/TA, X4 book equity/total liabilities), bands safe > 2.6, grey 1.1–2.6, distress < 1.1. Chosen because GPW is an emerging market per Altman's own adaptation, and X4 uses book equity — avoiding the weak `shares_outstanding` coverage that market-cap-based original Z would inherit. Banks/insurers and other financial-statement companies are **excluded** (Altman does not apply): exclusion keys off the existing `statement_type`/sector-pack discriminator, shown as an explicit `not_applicable` state, never a blank. The variant label ("Altman Z″, EM") is always visible beside the number.

*Amendment (2026-07-17, T3 gate findings):* (a) **F5 leverage input = total non-current liabilities** (`total_liabilities − current_liabilities`, both full-coverage facts), replacing `long_term_debt` (`ifrs-full:LongtermBorrowings`, ~6/15 real coverage — the binding constraint the T1 probe flagged). Matches the cross-check source's published F5 basis (BiznesRadar "Spadek zadłużenia", confirmed from real ratios) and stays within the formula's practice envelope; the component detail labels the basis. `long_term_debt` remains extracted for future use. (b) Cross-check bridge confirmed exactly: BiznesRadar's EM-Score = **3.25 + our Z″** (their published "wyraz wolny" row); our safe > 2.6 ⇔ their > 5.85. (c) **`statement_type` financial classification is a default-on precondition**: the real DB has every company `'industrial'` (banks/insurers included), so the `NotApplicable` gate never fires — a forward, idempotent migration maps the registry sector strings of obvious financial issuers (banki/ubezpieczenia/rynek kapitałowy…) to their sector `statement_type` **only where the column still holds the default**; manual values stay authoritative. (d) Health facts require a **re-extraction backfill** over already-stored ESEF packages (stored facts predate the concept-map extension); headless backfill entry + T9 live pass on the owner's app.

### 5. Extraction extension

ESEF concept map (+ structured-xHTML tier where present) gains `current_assets`, `current_liabilities`, `retained_earnings`, `long_term_debt`, seeded as reported `kpi_definitions` rows plus derived `working_capital` and `current_ratio`. Same validation pipeline (ADR 0061); a real-DB coverage probe reports per-company concept coverage before the score UI defaults on.

### 6. Insider substrate: two extraction targets, ground truth first

- **`insider_transactions`** — parsed from MAR art. 19 notification bodies (already classified `insider_transaction`): person, role (management/supervisory), buy/sell, volume, price, transaction date, provenance to the feed item. Backfilled over stored filings.
- **`management_holdings`** — parsed from the mandatory periodic-report section "Zestawienie stanu posiadania akcji … przez osoby zarządzające i nadzorujące": person, role, shares, as_of, provenance to the report document (card `9730f5f`).
- Both join `ownership_stakes` by the canonical holder identity (v0.56 `HolderIdentityMap` path) to stamp `founder_insider` and a skin-in-the-game badge in the Ownership section. v0.57 adds no ownership tables (interface note honored) — these are new sibling tables.
- Per [testing.md](../testing.md): a hand-labeled ground-truth set from the maintainer's real DB **precedes** parser commitment for both targets.

*Amendment (2026-07-17, ground-truth findings — 22 filings / 30 transactions labeled):* (a) the seeded `insider_transaction` rule patterns matched **0/22** real filings (too-narrow phrase forms) — the patterns are corrected as data + a reclassification backfill + a real-title corpus test, closing the "seed never validated against a real corpus" class; (b) the Bankier body is only the ESPI **cover note**: person/role/direction parse from it, volume/price/tx-date live in the attached notification PDF for ~90% of transactions — body parsing ships in T4 with NULLs for what the body omits (never guessed), and a follow-on **T4b** fetches + deterministically parses the attachment PDFs (ADR 0061 PDF tier; official disclosure documents, policy-clean); (c) shape refinements: nullable `role`/`direction`, `instrument` discriminator (subscription warrants are common), `related_pdmr` anchoring for closely-associated vehicles. Until T4b lands, the sentiment aggregate (Decision 7) is **count-based net direction with volume-where-known**, labeled honestly.

### 7. Insider sentiment: computed read model

Timeline of parsed transactions + rolling net bought−sold over **90-day and 12-month windows**, with a **minimum of 2 transactions** before any aggregate is labeled sentiment (below that: transactions listed, no aggregate). Computed at read time (short-positions-view precedent), no stored projection. Decision support: counts, volumes, and who — never "bullish/bearish" advice language.

### 8. Red flags: derived at read, raised as signals, acknowledgeable

Flag taxonomy (fixed, per-type severity — a static map, not user-configurable in v1):

| flag | source | severity |
|---|---|---|
| `auditor_red_flag` (qualified/disclaimer/negative/going-concern) | existing `auditor_opinion` signal | high |
| `report_delay` | expected `company_events.periodic_report` date passed (3-day grace) with no official report ingested | high |
| `fund_exit` | read-only derivation over `ownership_stakes` bases (holder present in previous full-picture basis, absent from newest; or an ESPI `major_holdings_change` crossing below the 5% disclosure threshold — a mere above-threshold decrease raises nothing, refined at T7) | medium |
| `score_deterioration` | F drop ≥ 2 vs prior FY, or Z″ band downgrade | medium |
| `short_spike` | KNF `delta_30d_pp` above threshold | medium |

- The **panel state is a computed read model** (`red_flags_view(company_id)`): active flags with severity, evidence links, and acknowledged history.
- **Raising** a new flag follows the KNF pattern: one synthetic `feed_items` row + one `confirmed` `company_signals` row (new empty-pattern seed categories `report_delay`, `fund_exit`, `score_deterioration`; `auditor_opinion` and `short_position_change` already exist) — so existing `signal_category` alert rules fire with zero new alert plumbing (ADR 0068 additive rule). Detection runs inline at the producing seams (ownership ingest, score recompute on new confirmed facts, calendar/report reconciliation on refresh), idempotent by deterministic flag id.
- **Acknowledge** is per flag instance: `red_flag_acks(flag_id, company_id, acked_at)` — an acked flag leaves the active list, stays in history, and never re-raises for the same evidence.

### 9. UI homes

Scores render in the **Quality area** beside the scorecard (component breakdowns expandable, formula citation visible). The **red-flags panel** is a new company-scoped cockpit panel type `redFlags` in the default panel set, with a calm explicit "no active flags" state. Insider sentiment + skin-in-the-game badge live with the **Ownership ("Akcjonariat") section**. Decision-support framing throughout; no composite conviction rating is derived from these (ADR 0042/0054 guardrail — the deferred conviction rollup stays deferred).

### 10. Real-data acceptance gate

Before the score UI defaults on: computed F and Z″ cross-checked against third-party published values (BiznesRadar publishes both for GPW) on a hand-collected sample of maintainer-tracked companies — validation-only use, no new runtime source (source policy unchanged, ADR 0082 posture). Tolerance and discrepancy triage are recorded in the plan doc.

### 11. Ownership OCR residuals ride along

Card `5171372` (parent epic `7e0b7c1`) executes in this milestone: the ADR 0077 tier-4 vision/OCR pipeline runs over `ownership_extraction_residual` documents; results land as **confirm-before-apply** proposals in the review queue (ADR 0072 decision 2a semantics), residuals clear on confirm. No new decisions — this closes the v0.56 promise.

## Rejected

- **Beneish M now** — input coverage too weak to be honest (see Decision 1).
- **F/Z as DSL catalog formulas** — grammar lacks indicators/prior-period refs; per-component explanation doesn't fit criterion output.
- **Partial headline scores ("5 pts of 6/9 available")** — breaks comparability with the literature and between companies; the breakdown conveys partial information without a misleading number.
- **Original Z / Z′ per-sector variant switching** — needs a manufacturer classification we don't have (`companies.sector` is free text); Z″ is the published EM answer and drops the market-cap dependency.
- **A stored `red_flags` table** — active flags are derivable; storing them invites drift. Only acks (user state) and raise-events (signals, for alerting/dedup) persist.
- **A new alert trigger type for derived conditions** — the KNF feed-item+signal pattern already routes derived events through existing rules.

## Consequences

- Migration(s): `insider_transactions`, `management_holdings`, `red_flag_acks`, new `signal_categories` + `kpi_definitions` seed rows — append-only, forward-only.
- New IPC commands (contracts.md § Company Health): `get_company_health(companyId)`, `get_insider_overview(companyId)`, `red_flags_view` equivalents + `acknowledge_red_flag`; all join the mock-fidelity corpus.
- Two new parsers join the real-data ground-truth loop; score engine ships proptest invariants (determinism, totality/no-panic on partial inputs) + insta goldens.
- `piotroski_f`/`altman_z` become referenceable in scorecard criteria; ADR 0046's resolver note is amended by Decision 2.
- Beneish M and any conviction rollup remain explicitly out of scope.
