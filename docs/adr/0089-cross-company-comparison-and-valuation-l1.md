# ADR 0089: Cross-company comparison + comparative valuation level 1

Status: Accepted (2026-07-27, owner sign-off at v0.61 planning — plan + storyboard approved in chat)

Deciders: maintainer. Area: fundamentals, valuation, frontend, sources.

The `v0.61.0` milestone ADR required by the roadmap's valuation arc. Realizes the comparison
milestone deferred since ADR 0052 (single-company numeric table note) and the level-1 slice of
[ADR 0041](0041-deterministic-valuation-engine.md) (triangulation layering: level 0 shipped with
[ADR 0067](0067-market-data-foundation.md) in v0.53; the DCF engine stays v0.62). Builds the peer-set
derivation ADR 0067 decision 3 explicitly deferred here. Approved storyboard (normative UI scope):
`docs/mockups/v061-compare-storyboard.html`.

## Context

The app has per-company fundamentals (`financial_facts`, three-layer KPI model, ADR 0027), EOD
quotes + `companies.sector` + level-0 market ratios (ADR 0067/0082), coverage and health read models
(ADR 0077/0083), and a Compare shell placeholder (nav slot reserved since v0.48, U-Rc). What is
missing is everything relative: no comparison read model, no QoQ/YoY deltas anywhere, no peer sets,
no FX layer (facts store native currency), no valuation module. The owner wants relative analysis as
decision support: the same canonical KPI across chosen companies, and where the company stands
against sector peers — with every number still evidence-linked.

Owner decisions at planning (2026-07-27): two epics (comparison / valuation L1); explicit company
selection with a sector-peers helper (not automatic-sector-only, not watchlist-only); the
single-company periods×deltas table lands in the company Dashboard's Fundamentals panel as the N=1
case of the same read model; a real FX layer now (not a flag-only fallback); `valuation_runs`
persisted from v0.61.

## Decision

1. **Comparison read model** — a Rust read model (new typed commands, `get_kpi_comparison` family):
   input = company ids + canonical `metric_key`s + granularity (annual/quarterly); output = an
   aligned period axis derived from `financial_periods` (year + period-type keys), per-cell
   decimal-exact value (base units, native currency + PLN-converted), the fact id + provenance
   `validation_status` (evidence link), and **QoQ/YoY deltas computed server-side** (percentage for
   monetary values, p.p. for ratio/percentage KPIs). N=1 serves the Fundamentals periods×deltas
   table — one read model, two surfaces. Runs off the UI thread (`spawn_blocking`); reads existing
   facts, never re-parses report tables (re-parsing was rejected in the v0.47 scope note — it
   produces unconfirmed numbers and duplicates KPI extraction).
2. **FX substrate (NBP)** — a new append-only `fx_rates` table (`currency, date, mid_rate`, unique
   `(currency, date)`) fed by an **NBP Table-A mid-rate adapter** (official public API, policy-clean,
   keyless; chosen over ECB reference rates because NBP is PLN-based — the app's comparison
   currency — and is the official Polish source). Full-history backfill on first need, then a daily
   pull job on the durable queue sharing the market-data lane (deliberate lane assignment per
   ADR 0059 — both are small, latency-tolerant external pulls). *(Amended 2026-07-29, #159: the
   synchronous first-need entry point `ensure_fx_backfilled` was removed unused — the daily job
   already backfills full history for any needed currency with no stored rows, so a new currency's
   history lands on the next daily run; a true first-need hook re-enters deliberately with the
   first non-PLN adapter.)* Conversion rule, deterministic and
   labeled in the cell's provenance: **flow** KPIs (`measure_window = flow`) convert at the
   period-average mid; **stock** KPIs at the last mid ≤ period end. Ratios/percentages are never
   converted. A missing rate or NULL fact currency renders an explicit per-cell flag — never a
   silent PLN guess. (Rejected: flag-only/no-FX v1 — the owner chose real conversion now.)
3. **Peer sets + sector percentiles** — sector peers derived at read time from `companies.sector`
   among **tracked** companies (no new tables); percentiles over level-0 ratios and selected
   canonical KPIs computed only from validated/confirmed data. **Thin-flag when N < 4** (GPW-honest);
   the peer count N is always displayed; a company without a sector is excluded with an explicit
   reason. Percentiles are computed independently of the user's on-screen selection.
4. **Comparative valuation level 1** — a new pure-Rust `valuation` domain slice (the ADR 0041 home;
   DCF joins it in v0.62): implied fair value per multiple (P/E, EV/EBITDA, P/BV × peer median),
   method-convergence spread, and a football-field range readout, plus a deterministic
   **confidence grade** composed from data completeness (fundamentals coverage), peer depth,
   method convergence, and provenance validation states — each component inspectable. All output is
   decision support: ranges, percentiles, and facts; no cheap/expensive or buy/sell/hold language
   (ADR 0042 boundary).
5. **Persistence** — `valuation_runs`, append-only from v0.61 (`company_id, method, inputs_json,
   outputs` low/base/high per share, `data_as_of`, `confidence_grade`, provenance, timestamps),
   schema designed against the v0.62 DCF needs so DCF adds rows, not columns. (Rejected:
   compute-only until v0.62 — the owner chose early history for what-changed diffs.)
6. **UI** — the real **Porównaj** mode: its own left-menu entry directly under Dashboard (restoring
   the reserved `navigation.ts` slot); company multi-select ("zestaw spółek" in UI copy — never
   "kohort") + sector-peers helper + watchlist quick-pick; side-by-side evidence-linked table
   (unbounded companies); `MultiLineChart` overlay (chart capped at the existing
   `MULTI_LINE_MAX_SERIES = 4` colored series; the table is not capped); valuation section
   (football field, percentile chips with N, confidence badge). The Fundamentals panel gains the
   periods×deltas table (N=1). Normative scope per approved storyboard frames (entry, primed,
   loading, success, error/partial, thin-set recovery, narrow pane, Fundamentals surface).
7. **MCP parity** (ADR 0088 registry rule) — every new command is classified in the registry;
   comparison + valuation reads join the read tier; the frozen tools/list snapshot is updated.
8. **Stale-reference cleanup in-milestone** — the v0.61-MCP-writes comments
   (`QualityPanel.tsx`, `qualityFrameworks.ts`, `quality_frameworks.rs`, `mcp/registry.rs:250` +
   snapshot) are stale since ADR 0088 shipped `set_qualitative_verdicts`/`set_claim_verdict` in
   v0.60; the ui-information-architecture "hidden until v0.53" Compare note and the ADR 0041
   layering-note version numbers are refreshed; epic b7a54ba's AV6 comment is marked stale
   (AV6 closed; embeddings retired by ADR 0080).

## Consequences

- Two append-only migrations (`fx_rates`, `valuation_runs`); no storage-posture change (ADR 0039).
- New external source: NBP (official/public, keyless) — recorded here as the source decision;
  witnesses are not needed for central-bank reference data.
- The v0.62 DCF engine lands into an existing `valuation` slice and an existing runs table.
- Real-data pre-validation (owner DB) gates the read-model approach before implementation
  (testing.md mandate): fact/sector coverage, peer depth, currency spread, hand-labeled
  period-alignment ground truth, NBP history depth.
- Compare becomes the third live mode in the sidebar; the v0.64 command center and v0.65 thesis
  arc consume the comparison + valuation read models.
