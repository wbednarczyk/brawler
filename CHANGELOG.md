# Changelog

## v0.65.7 - 2026-08-13


### Features

- **fundamentals**: queue integration for KPI ingest runs (#364) (#379)

## v0.65.6 - 2026-08-13


### Bug Fixes

- **fundamentals**: commit rebinds manifest to live run context; ordinal-ordered ledger (#362) (#377)


### Features

- **fundamentals**: idempotent manifest replay and typed commit concurrency (#363) (#378)

## v0.65.5 - 2026-08-12


### Features

- **fundamentals**: atomic manifest commit — one transaction per report (#362) (#376)

## v0.65.4 - 2026-08-11


### Features

- **fundamentals**: deterministic manifest validation with immutable attempts (#361) (#375)

## v0.65.3 - 2026-08-11


### Features

- **fundamentals**: closed ingest-run state machine with typed transition refusals (#360) (#374)

## v0.65.2 - 2026-08-11


### Features

- **fundamentals**: staged observations, commit receipts and live byte-retention guard (#359) (#373)

## v0.65.1 - 2026-08-10


### Features

- **fundamentals**: durable KPI ingest runs — migration 0137 + lease-based store (#358) (#372)

## v0.65.0 - 2026-08-10


### Features

- **compare**: remove the Compare screen + drop ~5,200 audited dead lines (#370)

## v0.64.0 - 2026-08-09


### Features

- **attention**: retire system-event toasts — ambient attention lives in Today (#330) (#349)

## v0.63.14 - 2026-08-06


### Features

- **feed**: remove the per-channel mass-delete feature (#329) (#347)

## v0.63.13 - 2026-08-06


### Bug Fixes

- **mcp**: enforce citation integrity on agent verdicts (#343) (#346)

## v0.63.11 - 2026-08-05


### Bug Fixes

- **ci**: mutation audit always shards — an unsharded full sweep dies at the 180-min cap (#340)


### Features

- **ci**: bench audit base-vs-head + live-drive hints and pr-live-cycle (#336, #337) (#342)

## v0.63.10 - 2026-08-05


### Features

- **ci**: gate architecture v2 — the PR is the only gate (ADR 0096) (#339)

## v0.63.9 - 2026-08-05


### Features

- **fundamentals**: retire PDF fact extraction (ADR 0095) + first measured ground-truth baseline for the structured tiers (#182) (#333)

## v0.63.7 - 2026-08-04


### Bug Fixes

- **insider**: make attachment merge batching-independent via durable unit provenance (#318) (#322)

## v0.63.6 - 2026-08-03


### Features

- **ownership**: mark threshold crossings on the stakes-over-time chart, and unstick two visual baselines (#145, #314) (#320)

## v0.63.4 - 2026-08-02


### Bug Fixes

- **a11y**: a cockpit panel is a group, not a landmark — plus the two harness cards reality had already closed (#142, #158, #120) (#315)

## v0.63.3 - 2026-08-02


### Bug Fixes

- **deps**: take over the three stuck dependabot majors — reqwest 0.13, schemars 1.2, and a refused @types/node (#313)

## v0.63.2 - 2026-08-02


### Bug Fixes

- **fundamentals**: good-first-issue batch — WDF cover-note KPI seeds, honest bank denominator, GPW listing titles, live-spec contention, dependency-scan gate (#312)

## v0.63.1 - 2026-08-01


### Features

- **ui**: fact detail in a Modal + fundamentals matrix grouped by statement (kpi_definitions.statement_group) (#310)

## v0.63.0 - 2026-08-01


### Features

- **mcp**: agent acquisition path — preliminary-results ingestion with provenance, agent trust tier, batch fact writes, security-gated capture (epic #285) (#306)

## v0.62.10 - 2026-07-31


### Features

- **fundamentals**: banking & insurance extraction — bank vocabulary for all three tiers, WDF bank-table honesty + repair, startup doc-kind self-heal (epic #277) (#283)

## v0.62.9 - 2026-07-31


### Bug Fixes

- **fundamentals**: discovered-tasks batch — mutants false-red, doc_kind marker system + title precedence, kpi_relevance layers 2-3 (ADR 0092 Accepted), span-arithmetic TTM (#276)

## v0.62.8 - 2026-07-30


### Bug Fixes

- **fundamentals**: data trust — measured repairs + guards for every bad-data class: container truth, association trust, currency, witness corroboration, honest TTM, kpi_relevance lifecycle (epic #229) (#268)

## v0.62.2 - 2026-07-30


### Bug Fixes

- **fundamentals**: extraction-honesty residue — 0119 repair, typed no-facts reasons, tests/ typecheck gate (#255)

## v0.62.1 - 2026-07-30


### Bug Fixes

- **release**: tag pushes over SSH with the deploy key — a workflow-touching merge racing the build made the App tag push look like a workflow edit (v0.62.0) (#246)

- **release**: republish computes prev — an empty prev degenerated the cliff range to a bare tag, so recovery could never regenerate notes or heal the stamp (#247)

- **mcp**: structuredContent object envelope + create_company parity; mutants kill-tests (#252)

## v0.62.0 - 2026-07-29


### Bug Fixes

- **release**: dev-shell banner to stderr, empty-notes fallback, Conventional PR-title gate, v0.61.6 changelog backfill (#242)


### Features

- **testing**: failure-path & real-state test layer — chaos seam, job-failure visibility, honesty ratchets, shape corpus (epic #40) (#239)

## v0.61.6 - 2026-07-29

*(Backfilled by hand: the squash-merge title of #227 was not Conventional, so git-cliff filtered the release empty — the PR-title gate added after this prevents the class.)*

### Bug Fixes

- **tests**: cargo-mutants sandbox resolves scenario data via `BRAWLER_SCENARIOS_DIR` + a guard test banning cross-tree `include_str!` (#110) (#227)

- **fundamentals**: capex displays as an outflow in the numeric fallback; as-reported values never rewritten (#156) (#227)

### Features

- **autopilot**: transient stage failures retry up to 3 attempts before the run fails (#159) (#227)

- **fundamentals**: financial facts carry an optional annotation (migration 0117) editable from the fact row (#189) (#227)

- **reports**: per-document extraction status chip (has data / flagged / empty) in the documents view (#155) (#227)

## v0.61.5 - 2026-07-28


### Bug Fixes

- **release**: stamp pushes via release-stamp deploy key + pre-tag preflight; backfill v0.61.2–v0.61.4 stamps (#221)

- **release**: preflight probes deploy-key capability over SSH — the ruleset API check false-negatives under GITHUB_TOKEN (#226)

- **fundamentals**: on-demand BiznesRadar pull shares the queue's per-adapter serialization (no double-fetch race) (#225)


### Features

- **app**: saved cockpit views rename in place (#89) + export write via typed command, webview fs permission dropped (#106) (#222)

## v0.61.4 - 2026-07-28


### Bug Fixes

- **api**: low-hanging batch 2 — orphaned src/api exports cleared + export-level gate, extraction headless-only doc, live-suite calibration, repoctx-first hook (#220)

## v0.61.3 - 2026-07-28


### Bug Fixes

- **release**: repo always shows the released version (post-tag bot stamp) + disk-guard vhdx-drive autodetect (#218)

- **ui**: low-hanging batch — alert category names, verdict busy-guard, orphan fallback copy, chip overflow hardening, MCP count SSoT, region-scoped live locators (#219)

## v0.61.2 - 2026-07-28


### Bug Fixes

- **ci**: changelog step uses the release range, not post-tag --unreleased (+ v0.61.1 backfill) (#214)


### Performance

- **ci**: browser suite in 4 Playwright shards (#213)

## v0.61.1 - 2026-07-28


### Bug Fixes

- **ci**: devshell image uses --impure profile install for nixos/nix registry

- **ci**: devshell image resolves base-profile priority conflict (--priority 4)

- **ci**: release tag step sets git identity (empty ident broke the first real release) (#212)


### CI

- label events re-run only the release-label check (#210)


### Features

- **ci**: GitHub-native CI, continuous release, and issue-tracking migration (ADR 0090)

## v0.61.0 - 2026-07-27

The valuation arc opens: per-company fundamentals become **relative analysis**.
A new **Porównaj (Compare)** mode lines companies up side by side, and a
deterministic **comparative valuation (level 1)** says where a company stands
against its sector peers — sourced, evidence-linked, and honest about thin
GPW peer sets. Two epics (ADR 0089).

### Added

- **Porównaj mode** — its own sidebar entry under Dashboard. Pick a set of
  companies (multi-select, watchlist quick-pick, or the sector-peers helper)
  and the comparison computes reactively — no submit button. Two views:
  - **Profil (default)**: every available canonical KPI vertically ×
    companies at one selected period, with a **Różnica** column for a pair
    (multiple for values, p.p. for ratios, "—" where incomparable).
  - **Trend**: one KPI across periods with a multi-series overlay chart
    (up to 4 colored series), sharing the chips' slot colors.
  Every value links to its source evidence; gaps render as typed chips
  ("brak danych za okres", "brak kursu FX"), never silently.
- **Comparison read model** (`get_kpi_comparison`): aligned period axes
  across companies, decimal-exact values, PLN conversion with a labeled
  basis, and server-side QoQ/YoY deltas with a documented rule for
  sign-changes (a typed "delta undefined" instead of a misleading percent).
- **NBP FX substrate**: daily mid rates (Table A) in an append-only
  `fx_rates` store with full-history backfill; flow KPIs convert at the
  period-average mid, stock KPIs at the last mid on or before period end;
  a missing rate is an explicit per-cell flag, never a silent guess.
- **Fundamentals "Pozycje × okresy" table** in the company Dashboard:
  line items × recent periods with inline QoQ/YoY deltas (the v0.47 promise),
  driven by the same read model.
- **Sector percentiles** (`get_sector_percentiles`): where the company's
  level-0 ratios sit among its tracked sector peers (mid-rank inclusive
  percentiles), always showing the peer count and a thin-flag under 4 peers.
- **Comparative valuation L1**: implied fair-value ranges per multiple
  (P/E, EV/EBITDA with a real net-debt bridge, P/BV) from peer P25/median/P75,
  drawn as a **football field** with the current-price marker; a deterministic
  **confidence grade (A–D)** built from four inspectable components (data
  completeness, peer depth, method convergence, validation states); runs
  recorded append-only in `valuation_runs` (only when inputs actually change).
  Decision support only — ranges and facts, never a cheap/expensive verdict.
- **4 new MCP tools**: `get_kpi_comparison`, `get_sector_percentiles`,
  `list_valuation_runs` (read tier) and `compute_comparative_valuation`
  (act tier, gated by the writes toggle) — the surface grows to 100 tools.
- New `RangeBarChart` UI primitive (the football field), gallery + a11y covered.
- User guide: `wiki/cross-company-comparison.md`; J6 journey E2E with budget.

### Fixed

- **MCP-written qualitative verdicts now actually render** in the Quality
  panel — the write tool shipped in v0.60 but the panel never displayed the
  results; found by the closure audit, fixed with a standing guard test.
- KPI names in the Compare picker rendered raw English in the Polish UI
  ("Current assets") — all KPI display names now route through
  `localizedKpiLabel`, with a rule + tests guarding the class.
- Diagnostics source-reconciliation rows: the long "Pominięte przez główne"
  status chip painted over the ticker column (a Polish-only overflow) —
  fixed with a content-sized status track and a bounding-box guardrail spec.
- NULL-currency ratio/percentage cells no longer carry a spurious
  `currency_unknown` flag (ratios have no currency by design).

## v0.60.0 - 2026-07-27

Two epics land together: **the Today home is re-invented as a real triage
surface**, and **the MCP port grows from 4 tools to a UI-parity surface of 96**
— the first full milestone of the BYOA (bring-your-own-agent) direction.

### Added

- **Today attention home v2** (ADR 0087). The Today/Pulse screen is now a
  grouped, deduplicated, severity-ranked attention stream with a typed
  three-level severity (`urgent` / `notable` / `routine`) mapped in exactly one
  backend place — a new trigger without a classification reddens a gate. Four
  summary tiles (Urgent first) sit above an expandable morning-briefing bar.
- **Severity aging and systemic-cause aggregates.** Stale urgent items demote
  after 72 h instead of shouting forever; the same root cause firing across
  many companies collapses into one cross-company aggregate row with a group
  **Dismiss all**.
- **Attention Archive view** — dismissed/seen items remain reachable instead of
  vanishing.
- **Evidence-specific rows**: each attention row carries copy specific to its
  evidence, with fire-time title snapshots (migration 0114) so a later rename
  never rewrites history; filenames render as metadata, not headlines. An
  in-app severity legend and an alert-origin indicator explain why a row is
  where it is.
- **MCP surface v2** (ADR 0088): a registry-driven, UI-parity tool surface over
  the typed command layer — **96 tools (41 read + 55 act)** with
  schemars-generated schemas and a frozen `tools/list` snapshot; every IPC
  command must be classified (`read` / `act` / `excluded`) or the gate reddens.
- **Provenance-mandatory writes behind an off-by-default switch.** Agent writes
  require `mcpWritesEnabled` (Settings → MCP; the setting itself is on the
  denylist, so an agent cannot switch it on). Every write family enforces its
  provenance rules at call time; job triggers are allowed, deletes/undo/settings
  stay UI-only.
- **Unclassified-filings triage pair** (`list_unclassified_filings` +
  `classify_filing`) so an agent can work through official reports the
  deterministic classifier left unclassified, plus `set_qualitative_verdicts`
  to close the post-AI-retirement verdict path.
- **Agent onboarding docs**: a wiki MCP agent guide + connection how-to, and the
  repo-side `brawler-mcp` skill, both held to the live catalog by a drift gate.
  An MCP dogfooding ritual joins the closure checklist.

### Changed

- **Toasts are pointers now.** The attention stream is the system of record:
  only urgent toasts persist, the stack never blocks interaction, and
  seen/dismiss state stays in sync with the stream.
- **The briefing seam is cut on the token pattern**: the backend stores only
  typed codes and source data (no composed English prose in the database), the
  frontend translates — briefing items are now fully bilingual and
  restyle-safe.
- **Config-state banner and per-category error bars** on Today — a
  misconfigured or failing source shows up as its own labeled state instead of
  a silently quiet screen.
- Journey J1 ("morning triage at 10 new items") now takes **6 interactions
  against a budget of ≤15**, enforced by the journey E2E budget ratchet.

## v0.59.0 - 2026-07-22

The biggest philosophical release since the first scaffold: **the in-app AI
analysis layer is gone, and financial data became a full automaton.** Brawler is
now a deterministic research substrate — numbers simply arrive, honestly
labeled, with nothing waiting for your confirmation — and intelligence comes
from **your own agent** over the local MCP port (BYOA: bring your own agent).

### Changed

- **The in-app AI analysis layer is retired** (ADR 0084). All eleven routed AI
  capabilities — feed analysis, research briefs/digests, KPI and claim
  extraction, signal/event/ownership classification fallbacks, qualitative
  assessment, briefing narration — plus the provider routing, failover pools,
  provider adapters and their API keys are removed. The only AI left in the app
  is **YouTube transcription** (Gemini, data acquisition). Saved outputs from
  the AI era remain readable as your data. The morning briefing is a fully
  deterministic composed list.
- **Core financial figures now arrive automatically, review-free** (ADR 0086).
  BiznesRadar is the **primary source for core KPIs**: once a day Brawler
  politely reads three public report pages per tracked company (income
  statement, balance sheet, cash flow) and ingests **every period column** they
  carry — a newly tracked company gets its whole reported history on day one.
  Issuer filings (annual ESEF, interim ESPI "Wybrane dane finansowe" cover
  tables, structured/positional xHTML) always outrank the aggregator and take
  over its slots; **your manual edits are untouchable**. Facts land
  `confirmed` immediately — the pending/ratification workflow is gone; origin
  is a provenance label with a citation down to the source row, never a to-do.
- **PDFs are for humans now.** The deterministic PDF fact-extraction arm is
  deleted (every issuer's layout was a separate fight and a source of silent
  scale errors). PDF text is still read for reporting-period detection and
  insider/ownership attachments, and a PDF report's Today card honestly says
  "core figures arrive from the aggregator source" instead of framing the
  by-design gap as a failure.
- **Cross-source witnessing is reversed.** Where the issuer's filing (or your
  manual entry) holds a figure and the aggregator disagrees, an informational
  disagreement entry with a plain-language diff is recorded on the Coverage
  panel's Flagged periods — nothing is blocked or overwritten. Empty/zero
  aggregator cells are never treated as data.

### Added

- **`rebuild_fundamentals`** headless command: repopulates all fundamentals
  from sources (aggregator pull + full ESEF re-extraction + WDF cover-note
  re-scan) with a per-tier verdict — used for the one-off v0.59 data rebuild.
- **Mapping guardrails** that make metric-semantics errors impossible to ship
  silently: a source-scan gate proving every emittable metric key has a catalog
  definition; a golden "source vocabulary contract" pinning every row of the
  real BiznesRadar pages as mapped-or-explicitly-skipped; and a cross-company
  "mapping suspect" alarm when one metric disagrees at many companies at once.
  Both hard gates caught real bugs on their first run.
- **Parent-attributable metrics** (equity and net profit attributable to the
  parent's shareholders, discontinued-operations result, inventories) as
  first-class catalog KPIs with Polish/English labels — BiznesRadar's
  parent-equity row no longer masquerades as group equity.

### Fixed

- 22 verified code-review findings across the new pipeline, including:
  BiznesRadar's parent-equity row silently understating group `total_equity`
  (the root cause of a cross-check failure that discarded a whole ESEF
  filing); 16 cover-note metric keys whose facts silently vanished for lack of
  catalog definitions; glued quarter headers (`3Q2024`) collapsing to H1;
  aggregator-sourced priors vetoing issuer filings in the comparative
  cross-check; a contradicted ESEF set surfacing as a silent "empty" instead
  of a flagged, reviewable outcome; ~9,000 per-fact write transactions per
  daily pull batched down to ~150.
- The company KPI catalog now always includes the shared canonical definitions
  (the fact matrix no longer invents placeholder English labels or loses
  per-share formatting), with concise Polish labels for the new metrics.

## v0.58.0 - 2026-07-19

Analyst recommendations: Brawler now tracks **what sell-side firms say about
your companies** — strictly as attributed, quoted third-party opinions, never
as the app's own advice.

### Added

- **Analyst recommendations panel** (opt-in, from the cockpit panel palette):
  each entry shows the rating **verbatim** ("kupuj", "akumuluj", "trzymaj",
  "redukuj", "sprzedaj") with an upgrade/downgrade/new/reiterated marker versus
  the same firm's prior entry, the target price with % vs the current close,
  the analyst + issuing firm, the publication date, and a link to the broker's
  PDF report. A summary strip shows the latest target (always with its firm and
  date), local-history depth, and the last change.
- **Append-only local history**: the free BiznesRadar page only carries the few
  most recent recommendations, so Brawler accumulates its own revision history
  from the day tracking starts — the panel footer says this honestly. Daily,
  polite, robots-clean ingestion for the whole watchlist (first real run:
  93 recommendations across 41 companies).
- **`recommendation_change` signal**: every new or changed recommendation lands
  in the feed with a badge, reaches Today and the morning briefing, and can
  drive alert rules like any other signal category.
- **"Vs target" readout** beside the price context — target price and distance
  from the current close, always naming the firm and date beneath the number,
  with a jump into the full panel. A bare, unattributed number never appears.
- **ESPI "Wybrane dane finansowe" tier-0 verdict** (research spike, adopted):
  the mandatory cover table of every ESPI periodic report — already flowing
  through the feed — was measured on a 347-value hand-labeled corpus at
  **100% recall and precision with zero false values**, using the form's own
  PLN↔EUR columns as a built-in checksum. Adopted as a planned extraction tier
  (`EspiCoverNote`, ADR 0061 amendment); implementation tracked separately.

### Changed

- **Automatic feed cleanup is disabled** (owner decision): the previous 30-day
  auto-prune silently deleted old feed items — including periodic reports used
  for research. Nothing is deleted on a timer anymore; cleanup is a manual
  "Clean up feed now" action in Settings → Sources, and the settings section
  now reports the true state. A redesigned, safe retention mechanism is tracked
  as a backlog card.

### Fixed

- Holder-name canonicalization is now idempotent for Unicode characters whose
  uppercase form decomposes (found by its own property test; counterexample
  pinned).
- Adapter-count test assertions are derived from the source registry instead of
  hand-counted constants (adding a source no longer requires bumping scattered
  numbers).

## v0.57.0 - 2026-07-18

Company health: the app now **raises "something smells here" on its own** for
every company you track. The Quality panel gains **health scores** — Piotroski F
(0–9) and Altman Z″ EM (safe/grey/distress) — computed deterministically from
confirmed facts with the published formulas cited, an expandable per-component
breakdown, and honest states: a full score or "insufficient data" listing
exactly what's missing (never a rescaled number), "not applicable" for banks,
insurers and brokers. Scores were validated against BiznesRadar's published
values on the maintainer's real portfolio (their EM-Score = 3.25 + our Z″,
matched to ±0.12). A new **Warning signals** cockpit panel collects auditor red
flags, late periodic reports, fund exits (including an ESPI filing crossing
below the 5% disclosure threshold), health-score deteriorations and
short-selling spikes — each with a severity, an evidence link, an acknowledge
flow, and full alert-rule wiring. The Ownership section gains **Insiders**:
parsed MAR art. 19 transactions (who/role/direction, volumes and prices filled
from the attached notification PDFs), management/supervisory holdings parsed
from periodic reports, rolling 90-day/12-month net buy-sell, and a
**skin-in-the-game badge** on shareholders tied to the board — including stakes
held via family foundations and holding vehicles. Guides:
[wiki/quality-frameworks.md](wiki/quality-frameworks.md),
[wiki/red-flags.md](wiki/red-flags.md), [wiki/ownership.md](wiki/ownership.md).

### Added

- Health scores (Piotroski F + Altman Z″ EM) in the Quality panel, usable in
  scorecard criteria (`piotroski_f` / `altman_z`); 4 new extracted
  balance-sheet concepts + a health-facts re-extraction backfill.
- Warning signals (red flags) panel in the default cockpit set with acknowledge
  → history semantics; derived flags raise typed signals through the existing
  alert rules (report delay, fund exit, score deterioration).
- Insider substrate: MAR art. 19 cover-note parser + attachment-PDF tier,
  management-holdings section parser (both ground-truth-validated on the real
  database), insider sentiment view, founder/insider stamping with the
  indirect-via vehicle bridge.
- Ownership OCR for unreadable shareholder tables (vision tier), always
  confirm-before-apply in the Review queue; xhtml residuals OCR their PDF
  siblings.

### Changed

- **Report history now backfills automatically** for every automated company —
  adding a company pulls its reports, facts and sectors with zero clicks
  (previously manual per company; 33 of 50 tracked companies had no coverage).
- Alert hygiene: historical ingest never impersonates the present (14-day
  freshness gate, wall-clock firing times), persistent toasts are capped at 3
  with a "+N more" summary and no longer cover the navigation; attention toasts
  show tickers and localized labels.
- Financial-statement companies (PKO, PEO, PZU, XTB, GPW, KRU) are classified
  via their registry sector so health scores honestly report "not applicable".

### Fixed

- ESPI insider-transaction classifier seeds matched 0/22 real filings —
  corrected patterns with a real-title corpus test; a startup catch-up
  reclassifies stored filings.
- Unit-scale detection: a narrative "mln zł" mention no longer overrides a
  statement's "w tys. zł" declaration (this had silently mis-scaled two stored
  CD PROJEKT facts ×1000 — repaired by a guarded migration; zero divergences
  across all companies after).
- Skin-in-the-game corroboration, management-holdings junk-row gate, and the
  KRU section parse — all found and fixed during live verification on the real
  app.

## v0.56.0 - 2026-07-16

Ownership structure: the app now knows **who owns every company you track and
how that changes over time** — gathered automatically, with visible sources.
The Basic info panel gains an **Ownership section**: a donut of the current
structure by holder type (founder/OFE/TFI/State Treasury/treasury shares…),
the top holders' stakes over time on one chart with a dashed free-float line,
and the **derived free float** (100% − disclosed stakes, with an honest
uncertainty note). Mid-milestone the source roles were pivoted on real-data
evidence (ADR 0072 amended): **BiznesRadar covers the whole watchlist daily**,
while periodic reports and ESPI threshold filings add depth, provenance, and
freshness where they exist — the newest full picture wins, and the sources
audit each other. Decision support only, all local. Guide:
[Ownership](wiki/ownership.md).

### Added

- **`ownership_stakes` with history** — append-only snapshots per (source,
  as-of, holder) with **capital % and votes % kept separate** (the
  preferred-share gap is itself a signal); deterministic ids make re-ingest
  idempotent, and re-ingest never wipes a classification (COALESCE upsert
  pinned by test).
- **BiznesRadar "Akcjonariat" as the automatic breadth source** — a daily
  polite fetch per tracked GPW company writes the "Główni akcjonariusze"
  table as a full-picture `aggregator` basis (single as-of from the page's
  "Data aktualizacji"); plausibility gates (summary rows skipped, >100%
  rejected, implausible sums written as diagnostics instead of data) mean the
  automaton can never write garbage stakes.
- **Report extraction** — the mandatory "shareholders ≥5%" table of stored
  periodic reports parsed deterministically on the autopilot lane (90.2%
  recall / 100% precision on a hand-labeled real-data ground truth);
  unreadable documents queue in a residual list for the future OCR/AI path
  (always confirm-gated).
- **ESPI major-holdings signal** — art. 69 threshold notifications classified
  as a new typed signal; a confirmed notification updates stakes with a
  conservative parse that never guesses (ambiguity → diagnostics, not data).
- **Holder-type classification** — a seeded dictionary of Polish TFI/OFE/state
  entities + name heuristics classify most holders; the residual goes to AI
  **as proposals you confirm or reject**; a manual re-type always wins.
  Cosmetic name variants merge by holder identity ("NN PTE" =
  "Nationale-Nederlanden PTE S.A.").
- **Two chart primitives** — `DonutChart` and `MultiLineChart` (shared scale,
  dataviz-validated palette, dashed neutral series, SVG tooltip), reusable
  beyond ownership.
- New wiki guide: [Ownership](wiki/ownership.md).

### Changed

- **Witness direction reversed** (ADR 0072 amendment): the disclosed
  reports/ESPI state now audits the BiznesRadar table; divergences above the
  disclosure threshold land in Diagnostics, same as before.
- The Ownership header labels its basis source honestly — "periodic report",
  "ESPI filing", "BiznesRadar", or "manual entry" (previously hardcoded).
- Current state reads by **disclosure basis**: holders who drop below the 5%
  threshold vanish from the current view (they never file "0%") but stay in
  history; the newest full picture across reports and BiznesRadar wins, with
  ESPI/manual overlays on top.

### Fixed

- **Aggregator parser defect caught live in owner dogfooding**: the page's
  "razem" summary rows were ingested as holders (share counts as percentages
  — a 13,720,265% donut) and the sub-5% fund-statement table was swallowed
  wholesale, on every tracked company. Fixed with table anchoring + row/basis
  plausibility gates, a repair migration that resets aggregator rows, a real
  regression sample, and a live spec asserting every rendered percentage
  stays ≤ 100.
- Dockview cockpit no longer buries the Basic info tab under the last-added
  panel on default layouts.

## v0.55.0 - 2026-07-15

Source reliability & disclosure signals: the feed is now **harder to fool and
harder to starve**. The official GPW ESPI/EBI channel runs as a **witness**
auditing what the primary source delivers — if it misses an official report on
one of your companies, you get a toast and a morning-briefing line. Two new
high-signal disclosure categories land in the feed and alert rules: **short
positions from the KNF register** (with a dedicated dashboard panel) and
**auditor red flags** (qualified opinion / disclaimer / going-concern). Under
the hood, every source now plugs into one `Fetcher` socket — adding a future
source is one self-contained file. Decision support only, all local. Guide:
[Source reliability & disclosure signals](wiki/source-reliability-and-disclosure-signals.md).

### Added

- **ESPI/EBI second witness** — the official GPW channel is re-enabled in a
  *witness* role (a "Świadek" badge in Sources): after each refresh it
  reconciles the official disclosure list against Bankier-sourced reports
  (exact ESPI report number first, company+date fallback) and persists a pair
  ledger (`matched | bankier_only | espi_only`). A report the primary channel
  missed on a tracked company raises a **system attention event** — persistent
  toast + Today row + morning-briefing line, no rule setup needed, deduplicated
  in the database. The witness never ingests feed items (no duplicates, by
  construction and by test). Full ledger: Diagnostics → "Uzgadnianie źródeł".
  First measured completeness run on real data: 15 witness items, 5 matched,
  0 missed by Bankier.
- **KNF short-selling register** — a new disclosure source over the register's
  stable public JSON endpoint (no scraping). Net short positions ≥ 0.5% are
  mirrored per company with an append-only change history; every change
  (entered / increased / decreased / exited) becomes a feed item plus a
  **"Short position" signal** that participates in alert rules. New cockpit
  panel **"Krótka sprzedaż (KNF)"** (palette): current holders, 30-day change,
  full history, register-refresh timestamp, and a calm empty state.
- **Auditor-opinion signal** — filings whose titles carry auditor red flags
  ("opinia z zastrzeżeniem", "odmowa wyrażenia opinii", "kontynuacja
  działalności" emphasis…) classify into a dedicated danger-badged category,
  alert-rule-capable; validated against the maintainer's real database with
  zero false positives. Feeds the v0.57 red-flags panel.

### Changed

- **SourceAdapter port gains behavior** (ADR 0069, amended): a `Fetcher` trait
  at the refresh level; all seven active adapters migrated strangler-style,
  per-source dispatch branching retired (`Fetcher | Disabled` only), full-
  refresh sweep membership pinned by test. New sources implement one trait in
  one file. Last Twelve Data residue removed.
- Sources screen: adapter rows expose the new `role` (primary / witness).

### Fixed

- A refresh path that succeeded without recording its run outcome showed a
  source as "never refreshed" forever (caught live on the witness) — outcome
  recording is now mandatory-by-checklist with a regression test.
- Negative-zero percent values ("-0,00%") normalized in fixed-precision
  formatting.
- Reconciliation no longer flags Bankier reports on the witness listing's
  boundary date (latest-N truncation artifact), and both new sources reject an
  empty upstream snapshot instead of misreading it as a mass exit.

## v0.54.0 - 2026-07-15

Attention routing: the app now **tells you what deserves a look** instead of
waiting to be scanned. You set **alert rules** ("notify me when…"), fired
alerts surface as **persistent in-app toasts** and a Today attention list, and
a **morning briefing** opens the day with "what changed in my companies + what
needs doing" — every item linked back to its evidence. Decision support only:
facts and links, never buy/sell advice, all local. Guide:
[Attention & briefing](wiki/attention-and-briefing.md).

### Added

- **Alerts** — a new Library screen (left sidebar, `Ctrl+9`, also in the `⌘K`
  palette). Build a rule from preset chips: a **signal category** (profit
  warning, insider transactions), **autopilot finished a report**, the price
  **entering your range**, or a **52-week low** — scoped to one company or a
  whole watchlist. A live plain-language preview restates the rule before you
  add it ("Notify me when a company on My GPW publishes a profit warning.");
  rules toggle on/off and delete with undo.
- **Attention events** — rules are evaluated automatically as data arrives
  (signal classification, autopilot completion, the post-session price pull).
  Each fired alert is stored once per piece of evidence (no re-fires on
  re-ingest, per-rule daily throttle) and links straight to what raised it.
- **Persistent alert toasts** — fired alerts pop up as toasts that stay until
  dismissed, with a click-through to the evidence (the signal, the run, the
  quote). The toast primitive gained this persistent variant alongside the
  existing auto-dismissing one; alert toasts can't be pushed out by ordinary
  feedback toasts and announce assertively to screen readers.
- **Today attention list** — open (undismissed) alerts appear as a fifth
  category in the Today stream, grouped by company, with mark-seen on review
  and dismiss.
- **Morning briefing** — a Today card composing "what changed since the last
  briefing": new signals, autopilot results, claims due for verification,
  upcoming report dates, fired alerts — deterministically ordered by the
  domain date. With an AI provider configured (the briefing has its **own
  provider routing** in Settings), a short cited narrative is added — a
  narrative citing anything outside the composed list is rejected, never
  stored. **Without any provider the briefing still renders as a structured
  list** — never blocked. Generate on demand or let the once-a-day auto
  refresh do it.

### Changed

- **Consistent action feedback** — manual source refresh, research import, and
  digest/brief generation now confirm with a transient toast instead of
  inline-only status; form validation and persistent statuses stay inline by
  design (the boundary is now a documented authoring rule).
- The J1 "morning review" journey starts with reading the briefing at the top
  of Today.

### Fixed

- Creating an alert rule after deleting another could crash with a database
  UNIQUE-constraint error (caught live on the owner's data): rule ids are now
  content-derived, and creating a rule identical to an existing one is a clear
  "already exists" message instead of a crash or a silent duplicate.
- WCAG AA color contrast for accent-colored small text (toast actions, active
  chips, preview highlights) in both light palettes.

## v0.53.0 - 2026-07-15

Market data foundation: track a company and Brawler now pulls its **daily
prices** automatically, turns them into a **price context** beside the reported
fundamentals — where the price sits in its year, what it's worth, the level-0
valuation ratios — and classifies its **sector** on its own. Everything is
automatic (no manual import), local, and decision support only. This release
also lands the **dashboard redesign**: one company-scoped Dashboard with saved
presets. Guide: [Price context](wiki/price-context.md).

### Added

- **Automatic daily prices.** Tracking a GPW company backfills its full
  end-of-day price history from its market debut, and a post-session daily pull
  appends each new session's bar — zero clicks, zero import. Prices come from
  Yahoo Finance (`<ticker>.WA`, PLN); a brief outage records a source-health
  note and self-heals on the next pull.
- **Price context** (leads the Fundamentals panel): latest close and day
  change, the **52-week range** with your distance from each end, **market cap**,
  and a **candlestick chart** of the session history with a readable
  round-number price scale and the covered date range.
- **Level-0 valuation ratios**, computed from the latest close × your confirmed
  facts: **P/E, P/BV, EV/EBITDA, dividend yield, FCF yield**, and **price vs
  52-week range (percentile)** (shown once there are ≥20 sessions, so it's
  context not noise). Ratios compute from **whichever inputs exist** — P/E, for
  example, tries market cap ÷ net profit, then price ÷ diluted EPS, then ÷ basic
  EPS — and only stay empty (`—`) when nothing resolves, never a guess.
- **Sector classification.** Companies are classified automatically from the
  GPW/NewConnect directory, with a **manual override** that a directory refresh
  never overwrites. The override field suggests matching sectors as you type
  (not a wall of every value).
- **Basic info panel.** A read-only company card — name, ticker, ISIN, sector
  (with a provenance chip), and the latest recorded shares outstanding with its
  period. Editable fields (sector, IR reports URL) are hidden behind a single
  **Edit** toggle instead of a button on every field.
- **Dashboard redesign.** Opening a company lands one **company-scoped
  Dashboard** with follow presets (panels track the view company); the
  Companies screen became the **Library**. Plus a UX-quality-loop pilot:
  interaction contracts, visual contact sheets, and live UX checkpoints.

### Changed

- The Fundamentals panel is reordered: **price context first, financial facts
  next, everything else after**. Sector and the IR reports URL moved out of it
  into the new Basic info panel.

### Fixed

- Price-history writes no longer race concurrent readers into a "database is
  locked" failure: all write transactions now begin immediately (guarded by a
  lint so it can't regress).

### Removed

- The Twelve Data secondary quote provider was implemented and then removed the
  same day: a live check proved GPW prices need a paid plan there, so the
  free-tier fallback premise was false. A free degraded fallback is planned for
  a later release; Yahoo covers all of GPW today.

## v0.52.0 - 2026-07-12

Judgment capture: start recording your own investment judgment — decisions and
pre-report expectations — before the history is lost, and let the app replay it
back as a **factual mirror, never a grade**. Plus the first slice of the
"talk to your research" north star: a **read-only MCP server** that lets an AI
assistant (Claude Code, Claude Desktop, …) read your local research over
localhost. And a slimming pass: the local embedding model, which never beat the
simpler static baseline, is retired. Local-first, decision support only, as
always. Guides: [Decision journal](wiki/decision-journal.md),
[MCP server](wiki/mcp-server.md).

### Added

- **Decision journal.** A per-company, append-only log of your decisions —
  buy, pass, keep watching, or a sell note — each with a decision date, a
  Markdown rationale, and optional **evidence links** to the reports, notes,
  claims, or assessments you actually relied on. Entries are **immutable by
  the database itself** (no edit, no delete); a change of mind is a follow-up
  entry that supersedes the old one, and the chain stays visible. Available as
  a cockpit panel per company and as **Journal (all companies)** from the
  command palette.
- **Pre-report expectations.** On a company's Report Season card, write down
  your stance and expected figures before the report. Expectations stay
  editable until the period's facts are confirmed — then they **freeze exactly
  as written**, and the card shows **expected vs. actual** side by side, with
  room for a resolution note on what you learned.
- **MCP server (read-only).** Settings → MCP server: enable a small,
  localhost-only (`127.0.0.1`) connector with a bearer token stored in the OS
  keychain and shown exactly once. Four read-only tools — company dossier,
  research search, claims due for verification, quality assessments — plus
  ready-to-paste client snippets and a bundled stdio adapter
  (`brawler-mcp-stdio.exe`) for clients that prefer pipes. Off by default;
  no tool can write anything.
- **Styled date picker.** A new app-wide date field replaces the raw native
  date input (first used in the notebook and the journal).

### Changed

- **Add-panel palette rebuilt** (owner dogfooding): the cockpit's `+ Add
  panel` surface now lists **generic panel types** that bind to the current
  view company — instead of one entry per tracked company per panel kind
  (296 entries → 16). Per-company "Switch view company" entries are gone too;
  the header selector is the single way to retarget a view. The palette
  search field now wears the app's design system instead of a bare native
  input.
- **New commands report errors as a typed code + message envelope** (first
  delivery of the error-contract migration), so failures surface precisely
  instead of as raw strings.
- **AI prose follows the app language** in the remaining surfaces (feed
  analysis, research briefs/digests), and untouched template frameworks
  re-localize their criterion labels on startup after a locale switch.

### Fixed

- Inbox no longer shows 0 items on "Everything" while current feed rows exist.
- Quality panel formats measured values (no more raw
  `0.00986053636687284…`).
- Journal evidence links validate against the right table — the first real
  entry with a decision-entry evidence link failed with "invalid research
  value"; a structural test now walks every allowed evidence type.
- Cockpit accessibility debt: 4 latent WCAG violations fixed; compact
  in-pane panel headers no longer duplicate the dock-tab title.
- Two flaky tests stabilized (TodayScreen date-order dependence,
  SettingsScreen theme-persistence timeout).

### Removed

- **Local embedding model retired.** The optional e5-small model, its vector
  index and similarity scaffold, and the write-only story-key ingest path are
  gone — the model never beat the simpler static baseline it was meant to
  improve on. The app is **57 crates and ~3.3 MB lighter**; old databases
  open cleanly (forward migrations clean up stored embeddings), and nothing
  you can see changes: rule-based signal classification and management-claim
  tracking are untouched.

## v0.51.0 - 2026-07-10

Trusted extraction: turn "add a company, see its fundamentals" into a measured,
one-click flow. A new **Coverage map** shows what has data and what is missing
per reporting period; **Backfill history** fetches past reports and extracts
them in one action; extraction now reads most interim reports **deterministically**,
with a free-tier OCR fallback held to a spend budget you control; everything
that can't be read cleanly lands in a **Review queue** instead of failing
silently. Local-first, decision support only, as always. Guide:
[Fundamentals coverage](wiki/fundamentals-coverage.md).

### Added

- **Coverage map (per company).** A new **Coverage** panel on the company
  dashboard: one row per reporting period (newest first) showing the canonical
  report, how many figures are recorded (validated vs. still-to-review), and a
  **to-review** count that opens the review queue. Gaps are never hidden — a
  period with a report but no data reads "not processed → Extract", a
  metadata-only filing reads "link-only — no stored file", and a period a report
  couldn't be found for says so.
- **One-click history backfill + sweep.** The Coverage footer's **Backfill
  history** fetches a company's past reports and then automatically extracts
  them — no per-document clicking. **Extract missing periods** runs the
  extraction alone over reports already stored. A live status line follows the
  work (backfilling → extracting N/M → done), and a company with automation off
  says so rather than doing nothing silently.
- **Deterministic interim extraction.** Quarterly and half-year reports that
  ship as web-page (XHTML) renderings now extract **automatically and
  deterministically**, without AI — including a new positional reader for
  reports whose layout only makes sense by column position.
- **Free-tier OCR fallback with a per-company profile.** When a report can't be
  read deterministically, an optional **Mistral OCR** tier reads it. The first
  read for a company lands as proposals you **confirm**; confirming teaches
  Brawler that company's layout so later reports read straight through.
- **Review queue.** A **Review queue** panel lists every figure awaiting your
  confirmation, grouped by period, each tagged with where it came from (OCR
  bootstrap, a flagged deterministic parse, or an older AI read) next to its
  source snippet. **Confirm** records the value (and confirms the OCR layout on
  the first one); **Reject** discards it. Reachable from the Coverage map's
  to-review cell or the panel picker.
- **AI spend budget for sweeps.** A new **Settings → AI** control caps how many
  OCR/AI calls a single history sweep may spend (presets 0/10/30/100, default
  30; **0 = no limit**). The Coverage footer shows the latest sweep's spend
  ("AI: 2/30"). The budget is snapshotted per sweep, so a change only affects
  future runs, and a sweep that hits the cap says "Skipped — AI budget" rather
  than dropping a period quietly.
- **Backfill depth setting.** Choose how many years of history a backfill
  reaches back (default 3, up to 10).

### Changed

- **Honest failure reporting.** A backfill on an unsupported market fails fast
  with a clear message instead of a silent no-op; OCR/AI fallbacks that degrade
  now leave a trail under **Diagnostics → Logs**; and a report that genuinely
  can't be extracted is reported as such, never as an empty success.
- **One validation regime.** Deterministic and AI-read figures now pass through
  the same validation, and the confirm step validates on confirm — the old
  unvalidated "none" state is gone.

### Fixed

- **Data repairs on upgrade.** Existing installs self-heal on launch: report
  documents are (re)classified into the new taxonomy, canonical-report links are
  repaired, and legacy annual periods are normalized to full-year — so the
  Coverage map is correct without re-importing anything.

## v0.50.0 — 2026-07-08

Quality frameworks learn to judge what a formula can't: **qualitative criteria**
(moat, pricing power, capital allocation…) assessed by an AI agent **from the
evidence already stored in your app**, every verdict cited so you can check it
yourself. Around it, a **systemic UX overhaul**: a normative design system with
enforcement at every layer, a redesigned **Today** attention stream, a global
**Ctrl+K** command palette, undo instead of "are you sure?", and an
accessibility pass across every screen. Decision support only, local-first, as
always. Guides: [Quality frameworks](wiki/quality-frameworks.md),
[Research workspace](wiki/research-workspace.md).

### Added

- **Qualitative criteria in quality frameworks.** Add a criterion that carries
  *assessment guidance* instead of a formula; click **Assess** and an AI agent
  reads only your app-held evidence for that company — claims, notes, typed
  signals, transcripts, stored report documents — and returns a verdict
  (pass / partial / fail / **insufficient evidence**) with citations and a
  written rationale, in the app's language. Never a buy/sell/hold. The shipped
  Kroeze-style template gains six qualitative criteria (bilingual, upgraded in
  place on existing installs). Assessments re-run on demand, re-run
  automatically when Autopilot processes a new report, and a **changed verdict**
  is surfaced in your research digest.
- **Today, redesigned as a single attention stream.** One prioritized list —
  autopilot runs, claims to verify, fresh disclosures, upcoming reports — with
  counter tiles that filter the stream and full j/k keyboard flow. Measured at
  6 interactions for the morning review, against a documented budget of 15.
- **Global command palette (Ctrl+K)** — jump anywhere, run any command, without
  the mouse; all shortcuts listed under Settings → Keyboard shortcuts.
- **Undo instead of confirmation dialogs.** Where a deletion can be restored
  faithfully, it now shows a toast with **Undo**; only genuinely irreversible
  actions keep an inline confirm. Focus lands sensibly after a row disappears.
- **Per-document "Extract data"** on report documents, with an honest
  four-state result toast (new / already recorded / divergent / no definition),
  plus a job-status surface so a failed assessment is never a silent nothing.

### Changed

- **One design system, enforced.** Spacing and type scales as tokens with a
  stylelint gate, semantic tone colors consistent across the dark and light
  palettes, one formatting layer for every date and number in the app (measured
  criterion values now render as "28,6%", not a 28-digit decimal), and panel
  **density contracts** so every cockpit panel stays usable at S/M/L pane
  widths — each rule backed by a test that reddens on regression.
- **Accessibility debt paid down**: automated WCAG A/AA checks now run over
  every screen in both themes (jsdom + real browser), with zero excluded
  screens; a live contrast bug in the light theme was found and fixed by the
  new gate itself.
- **Cockpit views follow a view-level company context** (panels can follow the
  view's company or pin their own), and **Compare is hidden until its data
  arrives in v0.53** rather than shipping as a dead tab.
- AI assessment output (and future AI text surfaces) now answers **in the
  app's language** (Polish/English), keeping source quotes verbatim.

### Fixed

- Six defect classes found by real-company validation on the maintainer's own
  data, each closed with a durable regression gate: silent assessment-job
  failures, no reachable extraction path for stored documents, a capability
  missing from AI provider routing, dishonest extraction toasts and a stale
  sibling panel, annual ESEF report packages (`.xbri`) yielding zero facts, and
  a crash when re-extracting a document whose facts already existed
  (re-observation policy: same value is idempotent, divergent values are
  surfaced — never silently overwritten).
- The mutation-testing memory jail no longer aborts the whole suite when a
  single runaway mutant is OOM-killed (`OOMPolicy=continue`).

## v0.49.0 — 2026-07-03

Brawler learns to work while you don't: **Autopilot** turns a tracked company's
new periodic report into one reviewed-or-reversible notification with no manual
steps, and the research cockpit becomes **composable** — build your own
multi-panel views and land on a curated per-company dashboard. Decision support
only, as always: Brawler tells you *what changed* and *what to verify*, never
buy/sell/hold, and everything stays local. Guides:
[Autopilot](wiki/autopilot.md), [Cockpit views](wiki/cockpit-views.md),
[Per-company settings](wiki/company-settings.md).

### Added

- **Autopilot — the autonomous report pipeline.** When a company you track
  publishes a new report, Brawler detects it, fetches it, reads out the
  figures, diffs it against the previous filing, cross-references your open
  claims, and surfaces the result as a single card on **Today** — with a
  **Review** button, an honest "couldn't finish" state when a run fails, and a
  two-step **Undo** that reverts exactly the facts that one run produced
  ("Reverted N facts"). Opt-in **per company** via a trust ladder: **Off**
  (today's manual flow), **Assist** (figures extracted automatically but
  nothing saved until you confirm), **Autopilot** (figures saved automatically,
  flagged as not-yet-reviewed, fully cited, undoable). The refresh schedule now
  runs in the Rust backend; Autopilot works while the app is open. See the
  [Autopilot guide](wiki/autopilot.md).
- **Structured-first KPI extraction, with provenance on every fact.** Before
  asking an AI, Brawler now reads figures **deterministically** from the
  report's own machine-readable data — official ESEF/iXBRL filings first, then
  PDFs it can map with confidence, witnessed against an aggregator where
  available — behind a validation gate (accounting identities plus a
  prior-period cross-check), so no figure lands without a verdict. A clean
  structured read auto-confirms in both Assist and Autopilot, because it's a
  direct read, not a guess; AI extraction remains the fallback and follows your
  chosen mode. Every fact shows **Source** and **Validation** chips in
  Fundamentals, and when a report's structure shifts from what's on file (new
  or missing line items, a different unit), the Today card shows the drift
  instead of silently absorbing wrong numbers.
- **Composable cockpit views.** A **"+ New view"** in the sidebar builds your
  own dashboard: name it, pick a grid visually (2×2 / 2×3 / 3×3 presets, or a
  custom size with linked sliders/inputs and a live preview), then click each
  empty cell to choose its panel — feed, inspector, fundamentals, claims,
  quality, report comparison, documents, notebook, and the library screens.
  Saved views live in the sidebar as their own entries: open with a click,
  delete from a hover action, persist rearrangements with **Save layout**. See
  the [Cockpit views guide](wiki/cockpit-views.md).
- **Company dashboard.** Opening a company now lands on a curated multi-panel
  dashboard — fundamentals, feed, claims, quality, report documents, notebook
  already in place — replacing the old tabbed workspace. It's a view like any
  other: rearrange freely and **Save dashboard** to keep each company's own
  arrangement. Pinned sidebar companies, feed items, and Today **Review**
  buttons all land on the same dashboard.
- **Per-company settings, including bulk manage.** Quick-edit a single
  company's autopilot mode and IR reports-page URL from its Fundamentals
  panel, or use **Companies → Manage settings** to change many at once: pick
  companies on the left (individually, select-all, or a whole watchlist),
  set grouped values on the right. Companies that don't share a value show as
  **mixed** — nothing is overwritten until you explicitly choose. The surface
  stacks in narrow windows instead of overflowing. See the
  [Per-company settings guide](wiki/company-settings.md).
- **AI provider pools per capability + an OpenAI-compatible provider.** Route
  each AI capability to its own ordered list of (provider, model) pairs with
  automatic failover and a cooldown for unavailable members; document-reading
  capabilities can only route to document-capable providers. A new generic
  **OpenAI-compatible** provider (base URL + model, key in the OS keychain)
  opens the door to self-hosted and alternative endpoints, with presets in the
  wiki.
- **Fair background work.** Background jobs now run in isolated lanes
  (sources / autopilot / AI / indexing), so a large watchlist refresh can no
  longer starve an autopilot run; each source refreshes one company at a time
  as resumable chunks, only one refresh per source runs at once, and AI calls
  respect a per-provider concurrency limit. Tunable under **Settings →
  Background work**.

### Changed

- The blank **Cockpit** sidebar entry is retired — saved views, **New view**,
  and company dashboards are the ways into the cockpit now.
- **Companies** is now a library and settings surface; clicking a company goes
  straight to its dashboard.
- The legacy cockpit layout toolbar is slimmed down to the **Preset** select.
- Autopilot notifications are **localized and honest**: the Today card
  composes its sentence in your language from the run's actual counts instead
  of a canned English summary.
- Dockview panel headers are visibly slimmer, so panels read as lightweight
  building blocks rather than heavy windows.

### Fixed

- **Stuck autopilot runs.** A re-created or retried job whose ID matched an
  already-finished row was silently ignored, leaving the run stuck forever;
  such jobs now re-arm and complete (the same fix revived per-job retries and
  the startup content-indexing refresh, both silently dead after their first
  success).
- **Crashes on Polish report text.** UTF-8 byte-slicing panics in date
  scanning and report parsing are fixed with character-boundary handling, and
  all similar slicing sites were swept.
- A transient AI-provider outage during extraction was recorded as a
  *succeeded* run with zero figures — permanently blocking re-detection. It
  now fails properly and self-heals, so the next refresh re-runs it.
- Autopilot picked "the newest report" by database insertion order, so a
  history backfill could make it fire on a years-old report; it now ranks by
  the report's actual disclosure date.
- The test-sample AI provider can no longer auto-confirm figures in autopilot
  mode — sample KPIs are never committed as real facts.
- Free-text settings fields (provider base URL, model name) were impossible to
  type into — each keystroke was validated and reverted; they now edit a local
  draft and save on blur.
- Narrow windows: the Today autopilot run card no longer overflows (its
  metadata and actions wrap).

### Under the hood (no user-facing change)

- One mandatory test gate: `make check` now runs every deterministic suite
  hard-fail (frontend, Rust, dead-export scan, contract-drift guard, the full
  browser suite) with a self-enforcing anti-rot meta-guard, wired into the
  pre-commit hook ([ADR 0062](docs/adr/0062-mandatory-test-gate-and-test-driven-loop.md)).
- Docs architecture rework + spec–code drift gates: the agent contract was
  consolidated and deduplicated, and `make check` now fails on drift between
  docs and reality — commands vs. code, screens vs. navigation, settings keys
  vs. schema ([ADR 0063](docs/adr/0063-claude-native-context-architecture.md),
  [ADR 0065](docs/adr/0065-spec-code-drift-gates.md)).
- Planning landed for the investor week calendar
  ([ADR 0058](docs/adr/0058-investor-week-calendar.md), scheduled later) and
  the AI-quality provider-routing epic
  ([ADR 0060](docs/adr/0060-ai-capability-routing-and-openai-compatible-provider.md),
  delivered above as amended); autopilot/queue/extraction design records:
  [ADR 0055](docs/adr/0055-autonomous-report-pipeline-trust-ladder.md),
  [ADR 0056](docs/adr/0056-per-company-settings-surface.md),
  [ADR 0057](docs/adr/0057-composable-views-and-curated-dashboard.md),
  [ADR 0059](docs/adr/0059-worker-pools-and-queue-fairness.md),
  [ADR 0061](docs/adr/0061-deterministic-fundamentals-data-gathering.md).
- Dev tooling: a `make live-up` live-drive cycle (rebuild + relaunch with CDP
  for on-real-app verification); test-coverage floors raised after backfilling
  behavior tests for the new v0.49 surfaces.

---


## v0.48.0 - 2026-06-24

A new home and a calmer, mode-based workspace. Brawler now opens on **Today** —
an at-a-glance "what needs my attention" briefing — and is organized around the
jobs you actually do (triage, deep-dive on one company, focused reading/writing)
rather than one freeform grid. Decision support only: nothing here tells you to
buy or sell. Direction and the UX research behind it:
[ADR 0054](docs/adr/0054-mode-based-thesis-centric-shell.md). See the
[research workspace guide](wiki/research-workspace.md).

### Added

- **Today/Pulse — the new home.** An attention digest, not a wall of feed items:
  *what changed* (the freshest report disclosures), *to verify* (management
  claims due/overdue for your pinned companies), *upcoming reports*, a
  watchlist-level conviction overview, and a compact recent-activity peek. Each
  item has a **Review** action that jumps you to the right place.
- **Left-sidebar navigation spine.** A persistent sidebar grouped into Modes,
  your **pinned/favorite companies** (pin from a company's header for one-click
  access), a Library (Inbox, Watchlists, Transcripts, Sources), and Utilities —
  replacing the old top navigation bar.
- **Company workspace — Advanced layout.** The tabbed company workspace stays the
  default; an **Advanced layout** button opens the dockview research cockpit
  scoped to that company — multiple resizable, dockable panels (feed, inspector,
  claims, report comparison, fundamentals, …) with task presets, named saved
  layouts, and pop-out-to-window.
- **Focus modes.** A full-screen, distraction-free surface (Esc to exit) for
  deep **reading** a long report-over-report comparison and long-form **writing**
  of notebook notes.

### Changed

- **Brawler opens on Today** instead of the feed; the full feed lives in the
  **Inbox** (in the sidebar Library).
- The cockpit feed now matches the real Inbox (only unread items stand out), the
  feed **inspector** was rebuilt to read cleanly, and the linked claims/report
  panels name the company they're showing.

### Notes & scope

- Per-company **conviction status** and the **Compare** cross-company KPI table
  are present as placeholders — they fill in with the valuation, quality, and
  thesis work on the roadmap, and stay evidence-linked decision support (never a
  rating). The remaining keyboard feed-triage actions are a follow-on.
- Your data — watchlists, notes, claims, and settings — stays local on your
  machine.

## v0.47.0 - 2026-06-22

Report comparison: see what changed between a company's two most recent financial
statements of the same kind, section by section — fully local, deterministic, and
offline (no AI, no API key). Decision support only: it shows what moved, never
whether to buy or sell. Design and real-data validation:
[ADR 0052](docs/adr/0052-report-over-report-diff.md).

### Added

- **Report comparison (report-over-report diff).** Open a company →
  *Fundamentals* to find the **Report comparison** panel. Brawler lines up two
  consecutive same-type financial statements — consolidated (*skonsolidowane* /
  SSF) or standalone (*jednostkowe* / JSF), never mixing the two — and tags each
  section as **changed**, **added**, **removed**, or **unchanged**, with a count
  of how many sections aligned between the filings. It reads both regular report
  **PDFs** and the newer **ESEF/iXBRL** filings (e.g. CD Projekt), and runs fully
  offline. See the [Report comparison guide](wiki/report-comparison.md).
- **Honest "can't compare" states.** A scanned/image report with no real text
  layer is flagged plainly ("no extractable text") instead of producing a
  misleading diff; a report that's only been detected (not yet downloaded) can be
  fetched and extracted on demand from the panel.

### Notes & scope

- Scope was narrowed to the **structured financial statements** after a
  market-wide real-data spike (770 GPW + NewConnect companies → 613 reports;
  89.4% extracted cleanly, with no case of wrong text passed off as good). The
  **narrative management-report (MD&A) diff** and an **AI "what changed" summary**
  are deferred to a later release — across issuers their headings don't line up
  reliably enough yet. Rationale and evidence in
  [ADR 0052](docs/adr/0052-report-over-report-diff.md).

### Under the hood (no user-facing change)

- A panic-safe PDF/ESEF text-extraction layer with per-document extraction state
  (migration 0053), a deterministic section diff with a self-diff-is-empty
  invariant, statement classification guarded by a golden snapshot, and four new
  typed commands. Correctness guardrails were harvested into the canonical docs:
  a grid-overflow layout rule, an inner scroll-container overflow check, a
  new-IPC-command test rule, and a third-party-parser panic-safety rule.

## v0.45.1 - 2026-06-20

A foundations release: mostly internal architecture and test-harness work that
makes the upcoming data-heavy features (story clustering, the autonomous report
pipeline, cross-company comparison) safe to build — plus a navigation change and
bug fixes you'll notice. Schema changes are automatic and idempotent; nothing
user-facing was removed.

### Changed

- **Primary navigation moved to a top bar** ([ADR 0047](docs/adr/0047-top-navigation-bar.md)) — the main screens are now reached from a top navigation bar.

### Fixed

- Four open bugs resolved: Inbox/Watchlist layout, ROIC/ROCE computation, and KPI-extraction guards.

### Under the hood (no user-facing change)

- **Architecture v2** ([ADR 0050](docs/adr/0050-architecture-v2-domain-stores-source-pipeline-durable-jobs.md)): the storage layer is split into focused per-domain stores; source adapters declare themselves through a single registry; ingestion runs through one shared pipeline that records a cross-source *story key* (the basis for upcoming story clustering); the frontend reads per-screen view-model contexts instead of a prop-drilled god-component; and background work runs on a new **durable, crash-resumable SQLite job queue**. A pure-Rust approximate-nearest-neighbour index is in place behind the similarity boundary for future scale.
- **Test architecture** ([ADR 0048](docs/adr/0048-test-architecture-sample-data-broad-clickable-coverage-and-layered-parallelism.md) / [ADR 0049](docs/adr/0049-test-architecture-v2-data-transform-correctness.md)): a canonical sample-data factory, layered parallel test execution, property/invariant + golden tests, parser fuzzing, a mock-runtime fidelity contract, migration-safety coverage, and broad clickable browser journeys — a large expansion of automated coverage.

## v0.45.0 - 2026-06-19

On-device semantic similarity: an optional, local embedding model that matches
your feed by *meaning*, not just keywords — the foundation for upcoming
cross-source story clustering. Decision support only, fully local, no API key.
Design: [ADR 0035](docs/adr/0035-two-layer-ai-and-local-interpretative-layer.md).

### Added

- **On-device embedding model (optional, local, no API key).** Under
  **Settings → AI → Semantic similarity** you can download a small multilingual
  model (`intfloat/multilingual-e5-small`, ~450 MB, one-time) that runs entirely
  on your machine — offline after the download, with nothing leaving your computer
  — and switch similarity matching from keywords to the model. On a Polish
  ESPI/EBI test it surfaced the right related filing first ~83% of the time versus
  ~42% for keyword matching: Polish word endings break plain keyword matching,
  while the model matches on meaning. It is fully optional and reversible — the
  similarity index is disposable and you can switch back to keywords at any time.
- **Developer-mode similarity check.** Diagnostics → *Similarity check* ranks your
  feed items by similarity to a chosen one and shows which method produced the
  ranking — a way to see the model at work ahead of the story-clustering feature.

### Changed

- Packaged Windows and Linux builds now ship the on-device similarity engine; the
  model weights themselves remain an optional, on-demand download.

## v0.44.1 - 2026-06-18

A follow-up to the quality frameworks feature.

### Added

- **Drill into evaluation history.** Each past run in a framework's evaluation
  history now expands in place to reveal its full per-criterion detail — every
  criterion's verdict and the measured value as it stood when that run was made
  (from the immutable snapshot, so it shows the figures at the time, not the
  current ones). History goes from a bare log to a record you can inspect.

## v0.44.0 - 2026-06-17

Score a company against your own quantitative quality checklists, built from its
reported fundamentals — no AI, fully deterministic, decision support only.
Design: [ADR 0046](docs/adr/0046-quality-frameworks-quantitative.md).

### Added

- **Quality** tab in the company workspace. Build *quality frameworks* — named
  checklists of criteria written in a small expression language (e.g.
  `roic >= 15%`, `net_debt_to_ebitda < 2.5 AND fcf > 0`, `cagr(revenue, 5) > 10%`)
  — entirely from the UI. Pick metrics from a dropdown instead of recalling key
  names, with live validation as you type.
- **Scorecard.** *Evaluate* scores the company against a framework over its latest
  confirmed financial period: each criterion gets a pass / partial / fail /
  no-data verdict with the measured value shown next to it. Runs are saved as
  immutable snapshots, and the **evaluation history** (each run prunable) reflects
  what the figures were at the time, even after newer numbers arrive.
- **Kroeze-style quality template** ships with the app — a general quality
  checklist (durable returns, healthy margins, conservative leverage, cash
  generation, growth). Every framework, templates included, is editable in place,
  clonable, and (for templates) resettable to its shipped defaults.
- **A broader computed-metric library** out of the box — liquidity
  (`current_ratio`, `quick_ratio`), leverage/coverage (`debt_to_equity`,
  `interest_coverage`), `payout_ratio`, `fcf_margin`, and more — plus a new global
  `user` scope so custom metrics can be defined and referenced by criteria.
- **A new `wiki/`** with user-facing guides, including a full
  [DSL reference](wiki/dsl-reference.md) for writing criteria.
- Frameworks, their criteria, and any referenced custom metrics are included in
  the data export/import bundle, so a framework you build travels with your data.

### Fixed

- The shared `Modal` no longer steals focus back to the dialog on each render, so
  typing in a field inside a modal (e.g. naming a new framework) keeps focus.

### Internal

- New `fundamentals/` domain core: a hand-written expression engine (lexer, Pratt
  parser, evaluator) shared by metric formulas and criteria, a deterministic
  shared derived-metrics service (reused by the planned comparison/valuation
  work), and the rule engine. Migration `0048_quality_frameworks.sql`.

## v0.43.0 - 2026-06-17

The Report-Season Cockpit prepares you for earnings season in one place: a
time-driven view, next to the Inbox, of upcoming report dates across your
watchlists, each with a pre-report card of what to check before the company
reports. Design: [ADR 0044](docs/adr/0044-report-season-cockpit.md).

### Added

- **Report Season** screen (in the sidebar, next to Inbox). Lists upcoming
  report dates across your watchlists, ordered by date, with a past-reports
  section and a stale-calendar indicator when the calendar data is out of date.
  Scope it to a single watchlist or view all tracked companies.
- **Pre-report card** per upcoming report, composed from your existing data:
  open research questions, unresolved management claims (bucketed due / overdue
  / upcoming), the last reported period's confirmed KPIs, and recent evidence —
  with one-click drill-in to the company workspace and its claims.
- **Prepare → process workflow.** Mark a company *prepared* once you have
  reviewed its card, and *processed* once you have handled the published report.
  The state persists per report occurrence and survives calendar refreshes.

### Changed

- Tickers in the cockpit use the shared exchange-colored `TickerLabel`, matching
  the rest of the app.

### Internal

- **Guardrail-harvest feedback loop** ([ADR 0045](docs/adr/0045-guardrail-harvest-loop.md)):
  flagged defects are converted into a precise automated gate or a documented
  rule in the same change, so a class of mistake is closed rather than repeated.
  Adds a tiered Definition-of-Done checklist, a `make check-epic` target that
  runs all suites (gate + knip + Playwright smoke) at epic closure, and
  screen-scaffold + domain-component authoring guidance.
- Report-season read models are backend-owned with no stored projection and no
  duplicated domain logic; the only new schema is `report_preparations`
  (migration 0047, idempotent).

## v0.42.0 - 2026-06-16

The Management Claims Tracker turns management promises into first-class,
trackable claims that resurface for verification when the relevant report
arrives. Design: [ADR 0040](docs/adr/0040-management-claims-tracker.md).

### Added

- **Management claims tracker** on the company Claims tab. Track a management
  promise with a due period (e.g. FY2026 Q4) and a user-set verdict — pending,
  delivered, partially delivered, missed, or revised. Add and resolve claims
  in place.
- **Due-period review queue** ("claims to verify"). When the report for a
  claim's due period arrives, the claim automatically resurfaces, bucketed as
  due / overdue / upcoming. For a quantitative claim, the matching confirmed
  financial figure is shown next to it so you can resolve the verdict against
  the evidence.
- **AI claim extraction** from report documents and earnings-call transcripts.
  The model proposes forward-looking management claims — each with a suggested
  due period, an optional quantitative target, and a verbatim source snippet —
  for your review. Nothing is created without explicit confirmation, and
  rejected proposals are remembered so they are not re-proposed.
- Claims participate in the rest of the research workspace — the research
  timeline, reminders, and digests — and are included in research data
  export/import.

### Changed

- Claims are now a first-class entity instead of a flavor of notebook note.
  Existing claim notes are migrated automatically on first launch, preserving
  each claim's identity, status, due period, and existing evidence links and
  reminders.


## v0.41.2 - 2026-06-16

Internal quality hardening; no user-facing behavior change.

### Changed

- Encoded more of the project's practices as automated stop-gates: enforce the
  shared `ErrorText` primitive, add primitive and accessibility tests and a
  primitive gallery, and add dead-code, lint, and stylelint checks.
- Aligned the toolchain to Node 22 (Vite 8 / Vitest 4) so local and CI builds
  use one version, and extracted the AI-analysis controller out of the app
  root for clearer module boundaries.


## v0.41.1 - 2026-06-16



### Features

- adopt and enforce the primitive-first component framework


## v0.41.0 - 2026-06-16



### Features

- report document ingestion, history backfill, and derived events (v0.41.0)


## v0.40.0 - 2026-06-15



### Features

- typed ESPI/EBI event classification


## v0.39.0 - 2026-06-15



### Features

- add interpretative AI layer static foundation


## v0.38.0 - 2026-06-14



### Features

- global search and database improvements


## v0.37.0 - 2026-06-14



### Features

- v0.37 panel, charts, as-reported formatting, and review modal

- fundamentals panel UI, KPI extraction flow, detail-rail refactor, and assertion-driven UI test harness


## v0.36.0 - 2026-06-13



### Bug Fixes

- normalize report MIME type and surface extraction errors

- make KPI extraction thorough and lift output token cap

- reset KPI extraction panel per feed item

- diagnose extraction completeness and tighten period detection

- exclude derived KPIs from extraction and improve panel clarity



### Features

- add KPI extraction contracts and prompt boundary

- add native PDF document input to the Claude adapter

- build AI KPI extraction job with confirm/reject staging

- add per-company IR reports-page URL field

- add AI-assisted IR-page report resolver

- add frontend API for KPI extraction and IR resolution

- add KPI extraction review UI and IR reports-page field


## v0.35.0 - 2026-06-13



### Changed

- migrate AI analysis and transcription layer to async

- add provider registries replacing duplicated dispatch

- one key per provider, generic provider_id-keyed commands



### Features

- per-provider model registry and multi-provider defaults (migration 0036)

- catalog-driven AI provider and model selection

- shared analysis prompts/parsing and Claude (Anthropic) adapter

- OpenAI (ChatGPT) analysis adapter

- document-input abstraction with Gemini native and capability flags

- per-provider API key entry for Claude and OpenAI


## v0.34.0 - 2026-06-13



### Features

- add financial facts schema (migration 0034)

- add financials storage, commands, and DTOs

- persist report documents and capture from URLs

- add manual KPI entry/edit workflow


## v0.33.0 - 2026-06-12



### Changed

- modularize import export backend

- split workspace screen panels

- split screen styling

- extract feed company matching boundary

- extract shared AI response helpers

- extract parsing helpers

- split translation resources

- modularize app workflow harness


## v0.32.0 - 2026-06-12



### Changed

- introduce shared UI primitives

- expand shared UI primitive foundation


## v0.31.1 - 2026-06-12



### Bug Fixes

- polish settings navigation and AI options


## v0.31.0 - 2026-06-12



### Features

- add event-aware reminders and AI digest workspace


## v0.30.0 - 2026-06-11



### Features

- add AI briefs and improve research question workflow


## v0.29.0 - 2026-06-11



### Features

- add questions and evidence links


## v0.28.8 - 2026-06-11



### Bug Fixes

- avoid weak substring matches for media companies


## v0.28.7 - 2026-06-11



### Bug Fixes

- isolate Nix wrappers from inherited library paths for local windows build

- prevent Inbox layout collapse on narrow desktop windows


## v0.28.6 - 2026-06-11



### Bug Fixes

- repair WSL Linux startup and artifact collection


## v0.28.5 - 2026-06-10



### Bug Fixes

- publish changelog entries as GitHub release notes


## v0.28.4 - 2026-06-10



### Bug Fixes

- install minimal AppImage prerequisites


## v0.28.3 - 2026-06-10



### Bug Fixes

- install AppImage runtime tools


## v0.28.1 - 2026-06-10



### Bug Fixes

- allow AppImage packaging without FUSE


## v0.28.0 - 2026-06-10



### Features

- add cross-platform release artifacts


## v0.27.0 - 2026-06-10



### Changed

- prepare repository for open-core publication


## v0.26.0 - 2026-06-10

### Added

- Added watchlist review mode to the Research screen with a Company/Watchlist mode switch.
- Added backend-owned watchlist evidence summaries with member-company review queue counts.
- Added an explicit cascade option when marking a watchlist reviewed so member companies are only marked reviewed by user choice.

### Fixed

- Added the missing notebook note deletion action.

### Tests

- Added regression coverage for watchlist review mode, review cascade behavior, and notebook note deletion.


All notable project changes are recorded here.

Historical entries through `0.24.1` were curated from `docs/kanban-archive.md` because the early Git history predates the Conventional Commits policy. Future entries are generated with `git-cliff` from Conventional Commits and may be edited for clarity before release.

## v0.25.0 - 2026-06-09

### Added

- Added the first visible Research workspace screen with company-scoped evidence timelines.
- Added backend Research timeline summaries with total evidence, changed-since-review counts, and last-reviewed timestamps.
- Added backend-owned Research filters for evidence type and changed-only views.
- Added company-level Research review checkpoints with a `Mark reviewed` workflow.
- Added SemVer, Conventional Commit, git-cliff changelog, and release validation workflow documentation and scripts.

### Changed

- Rendered Research evidence rows with product-language labels, compact density, and owning-domain/source actions.
- Reworked the future Research roadmap so watchlist review, questions, briefs, reminders, and optional stored projections build on the completed company timeline.

### Fixed

- Fixed Research feed-item navigation so opening evidence lands in Inbox with the selected item still visible.
- Hid raw internal event/provider identifiers from normal Research UI, including event codes and AI provider ids.

### Tests

- Added regression coverage for Research navigation, backend filtering, review summaries, feed-item opening, and normal-user copy leaks.
- Added release workflow guardrails for commit message validation, version synchronization, and changelog generation.

## v0.24.1

### Fixed

- Hardened company lookup and company creation so NewConnect and future company-directory sources work through shared registry behavior.
- Generalized company-directory bootstrap, stale checks, media matching, and source-listing matching beyond hard-coded GPW/NewConnect assumptions.

### Tests

- Added regression coverage for future-exchange lookup/create, Companies UI add, watchlists, notebooks, manual events, import/export, media matching, and source-listing matching.

## v0.24.0

### Added

- Added the research/evidence read-model boundary for future Research workspace features.
- Added durable research review checkpoints and typed evidence links.
- Added backend-owned research timeline read models assembled from canonical feed, notebook, event, transcript, and AI-analysis domains.

### Changed

- Recorded the large-file responsibility audit and deferred stored timeline projections until performance or review semantics require them.

## v0.23.0

### Added

- Added opt-in Playwright browser UI smoke tests for layout regressions that jsdom cannot catch.
- Added deterministic browser-smoke data and Make/npm commands for installing and running Chromium-only UI smoke tests.

### Tests

- Covered fixed app chrome, internal scroll regions, Companies list height, Notebooks pane scrolling, Sources compact rows, Watchlists scrolling, and basic navigation.

## v0.22.0

### Added

- Reworked Sources into a normal-user trust and control surface with required, optional, and developer visibility tiers.
- Added optional source enablement/disablement with protection for required and developer-only sources.
- Added NewConnect company-directory support and kept GPW/NewConnect directory lists separated while preserving shared lookup/cache behavior.

### Changed

- Moved unimplemented source candidates and implementation details out of normal UI into Developer Diagnostics and docs.
- Moved company-directory refreshes to the async source-refresh task boundary.
- Added deterministic exchange colors for GPW, NewConnect, and future market prefixes.

## v0.21.0

### Added

- Added portable-only Windows executable candidate packaging.
- Added executable-adjacent data-directory mode for portable app runs.
- Added WSL/native Windows packaging helpers and package smoke documentation.

### Changed

- Built release executables as GUI-subsystem Windows apps so they run without a terminal window.

## v0.20.0

### Added

- Added JSON import/export for research data: companies, watchlists, memberships, and notebook entries.
- Added YAML import/export for allowlisted non-secret settings.
- Added import preview, transactional apply, merge semantics, and file picker behavior.

## v0.19.0

### Added

- Added a dedicated Watchlists panel for creating, renaming, deleting, selecting, and managing watchlist memberships.
- Added backend watchlist rename/delete lifecycle commands.

### Changed

- Removed watchlist mutation controls from Companies while preserving membership context and watchlist filters.
- Added layout and normal-user-copy regression guardrails.

## v0.18.0

### Changed

- Polished Notebooks, Inbox, Sources, Settings, Companies, shell/sidebar, topbar, scrolling, selected rows, and architecture-copy visibility.
- Added shared ticker rendering, app themes, watchlist filters, locale coverage, and focused workflow tests.

## v0.17.0

### Added

- Added the local author/friend-test license gate.
- Added extensible license parsing, verification, entitlement policy, OS keychain storage, redacted metadata, typed commands, UI gate/settings flows, owner tooling, and license operations docs.

## v0.16.0

### Added

- Added local metrics with typed samples, runtime counters, collector registry, on-demand snapshots, and Developer Diagnostics presentation.

### Changed

- Kept collector and presentation/export boundaries ready for future Prometheus, OpenTelemetry, or file adapters without adding remote exposure.

## v0.15.0

### Added

- Added local JSON Lines runtime logging, log directory initialization, configurable rotation, redaction, Settings controls, Developer Diagnostics log viewer, and typed commands.

## v0.14.0

### Added

- Added local Developer mode diagnostics with persisted mode, diagnostics storage, redaction, retention, typed commands, Diagnostics UI, and first AI/source/credential diagnostic producers.

## v0.13.0

### Added

- Added provider-neutral AI analysis architecture, contracts, storage, settings, async job runtime, typed commands, and frontend API.
- Added deterministic test-sample analysis provider and Gemini as the first live general-analysis provider.
- Added feed-detail AI analysis UI with prompt presets, custom questions, async state, retry, metadata, reasoning, tags, and source references.
- Added opt-in live Gemini feed-item analysis smoke path.

## v0.12.0

### Added

- Added English/Polish locale workflow with English as the first-run default.
- Added configurable app, Inbox, Company, and notebook shortcut actions with Settings discoverability, persistence, reset, disable, and conflict warnings.

### Changed

- Recorded the standing rule that new or changed repeated user actions should be evaluated for shortcut support.

## v0.11.0

### Changed

- Completed the broad modularization pass by extracting frontend API boundaries and major screen modules out of the app shell.
- Preserved existing Inbox, Companies, Transcripts, Settings, and workflow behavior while reducing large-file responsibility.

## v0.10.0

### Added

- Added the YouTube-to-transcript-to-notebook workflow backed by Gemini.
- Added transcript job storage, immutable transcript segments, typed transcript commands, URL-first transcript UI, segment review, segment selection, and editable note creation from selected transcript material.
- Added Gemini credential settings, model selection, timeout settings, provider disclosure, OS-keychain credential storage, and opt-in live smoke tests.

### Changed

- Promoted real Gemini execution for transcript jobs while keeping test-sample providers for automated tests and development.

## v0.9.0

### Added

- Added company events storage, typed commands, Events navigation, upcoming/week/list views, filters, manual event creation, and source-backed event ingestion.
- Added GPW Market Events RSS and Bankier Kalendarium ingestion for tracked company events.

## v0.8.0

### Added

- Added Bankier Gielda RSS as the first public media/news adapter.
- Added Bankier per-company komunikaty as the active v1 official-report adapter for tracked GPW companies.
- Added company source identifiers, source status distinctions, disabled reviewed candidates, and feed cleanup controls.

### Changed

- Kept GPW ESPI/EBI registered but disabled until a later reliability pass.
- Documented Portal Analiz as a late-v1 authenticated research candidate.

## v0.7.0

### Added

- Added the GPW company registry cache for lookup, autocomplete, registry refresh, and ticker-first source matching.
- Added Sources registry detail, cached-company search, tracked/untracked state, and add actions.

### Changed

- Removed target-runtime sample registry/feed seed data.
- Added slow in-app stale-cache registry refresh behavior.

## v0.6.0

### Added

- Added GPW detail-page fetching for matched official report bodies and attachments.
- Added parser tests, detail usability warnings, detail fetch counters, attachment storage, and source status detail warnings.

### Changed

- Recorded GPW detail fetching as the primary in-app official report body path, with Bankier/Parkiet/PAP as fallback or cross-check candidates.

## v0.5.0

### Added

- Added the first GPW ESPI/EBI listing adapter, normalized listing parser, source ingestion, manual refresh, scheduler behavior, source status, unmatched diagnostics, and source policy visibility.

## v0.4.0

### Added

- Added durable company notebooks, Markdown notes, note editing, tags, note kinds, claim status, follow-up fields, note origins, feed-to-note drafts, and claim views.
- Added the cross-company Notebooks screen and claim-oriented filtering.

## v0.3.0

### Added

- Added the Inbox and Company Workspace using local persisted feed items.
- Added Inbox filters, read/saved state, source details, empty states, company workspace navigation, company feed details, source status, and topbar refresh/status wiring.

## v0.2.0

### Added

- Added local SQLite storage foundations for companies, watchlists, settings, feed items, source records, notebooks, transcripts, jobs, and settings.
- Added settings storage commands, Settings screen basics, test-sample-backed company lookup, basic watchlists, and migration tests.

## v0.1.0

### Added

- Added the Tauri, React, TypeScript, Rust, Nix, and Makefile desktop shell foundation.
- Added app shell layout, dark/light theme selection, initial visual tokens, health command, local build/test commands, Windows sanity helpers, and CI skeleton.

## v0.0.0

### Added

- Added the spec-driven planning baseline: project brief, product scope, architecture, ADRs, UI flows, information architecture, contracts, data model, source strategy, roadmap, and agent contract.
