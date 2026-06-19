# ADR 0046: Quality Frameworks — Quantitative Checks, the Metric Expression Engine, and the Open Metric Catalog (Design)

Status: Accepted

This ADR captures the **design** for quality frameworks — quantitative checks (epic `287d0b4`, milestone `v0.44.0`): a user-owned *framework* is a named set of investing criteria that are testable against the fundamentals facts with **no AI** (e.g. `roic >= 15%`, `net_debt_to_ebitda < 2.5`, `fcf > 0`, `cagr(revenue, 5) > 10%`). A deterministic engine evaluates them exactly against confirmed facts and records the measured value, producing a **versioned scorecard** per company. It ships clonable app templates including a Kroeze-style quality framework. It records the decisions made during milestone planning so contracts, data model, product spec, and UI flows are decision-complete before implementation.

It builds on:

- [ADR 0027](0027-company-fundamentals-scope.md) — the `financial_periods` / `financial_facts` / `kpi_definitions` model the criteria evaluate against. `kpi_definitions.formula` already stores derived-metric formulas as expression strings (`free_cash_flow = operating_cash_flow - capex`, `roic = nopat / invested_capital`, `net_debt_to_ebitda = net_debt / ebitda_ttm`), but **nothing evaluated them** before this milestone.
- [ADR 0039](0039-ports-and-adapters-posture.md) — hexagonal at external seams, package-by-feature inside the core. This milestone is mostly **domain core** in a new `fundamentals/` slice and deliberately introduces **no new ports** (Decision 7).
- [ADR 0040](0040-management-claims-tracker.md) — the job→proposal→confirm and quantitative-target (`target_metric_key`/`target_comparator`/`target_value_numeric`) patterns this design mirrors for criterion shape and confirmed-facts-only reads.
- [ADR 0038](0038-enforcement-as-guardrails.md) and [ADR 0045](0045-guardrail-harvest-loop.md) — enforcement-as-guardrails and the guardrail-harvest loop (Decision 8).
- It is the quantitative half; qualitative/agent-assessed criteria are the next epic ([roadmap](../roadmap.md) `v0.50.0`). The scorecard is the future input to the `AdvisoryVerdictProvider` port ([ADR 0042](0042-advisory-verdict-port-and-open-core-boundary.md), `v0.55.0`), which is **out of scope here**.

## Context

The fundamentals substrate (facts, the KPI catalog, periods) exists but is **inert** — facts are stored and displayed, never *evaluated*. This is the first milestone that turns facts into computed judgement. Doing so surfaces a gap and an opportunity:

1. The data model specifies derived metrics (ROIC, net-debt/EBITDA, FCF, margins) as "computed at read time from confirmed facts," and `kpi_definitions.formula` stores the formulas, but there is **no code that evaluates them** — `storage/financials.rs` is facts CRUD only.
2. Quality-framework criteria are *mostly comparisons over derived values*. So the criterion engine and the derived-metrics layer are the **same expression engine**: arithmetic over a metric environment to compute a metric value, the same engine plus comparators and boolean logic to render a pass/fail verdict.
3. The derived-metrics layer is needed again by cross-company comparison (`v0.53.0`) and the deterministic valuation engine (`v0.54.0`). Building it once, reusably, is the architecturally correct response.

## Decisions

### 1. One expression engine, two consumers

A single deterministic expression engine lives in `src-tauri/src/fundamentals/expr/` (lexer + hand-written Pratt parser → typed `Expr` AST + evaluator). **No new parser crate** — the grammar is small, and a hand-rolled Pratt parser keeps it local, deterministic, and fully testable. The grammar:

- **metric references** — bare keys (`roic`, `revenue`, `operating_cash_flow`).
- **aggregation suffixes** already present in seeded formulas — `_ttm`, `_avg`. These (and the `ttm`/`avg`/`cagr`/`trend` window functions) are the engine's only *built-in* resolution, implemented in `metrics.rs` over the period series; everything else resolves through the catalog.
- **intermediates that formulas reference** are resolved **entirely through the catalog**, not through hidden code: each is either a reported `kpi_definitions` fact (`net_debt`, `ebitda`, `total_equity`, `cash`, …) or a **seeded derived `kpi_definitions` row** with its own formula over reported inputs. There are **no built-in synthetic-key resolvers** — a key that is neither a fact, a derived row, nor a suffix/window of one is `Unavailable`. Concretely, `invested_capital = total_equity + net_debt` and `capital_employed = total_equity + net_debt + cash` are seeded derived rows (migration `0050`). **ROIC is defined pre-tax** — `roic = operating_profit / invested_capital` — because no income-tax fact is extracted; NOPAT (and any guessed effective tax rate) is deliberately **not** used. This corrects an earlier overstatement that "the small remainder are built-in resolvers"; that path was never implemented and left ROIC/ROCE uncomputable (issue `674cb5a`). The `every_canonical_derived_metric_is_computable_from_a_representative_fact_set` gate keeps every canonical derived metric resolvable from a full fact set.
- **arithmetic** — `+ - * /`, parentheses.
- **functions** — `cagr(metric, n)`, `ttm(metric)`, `avg(metric, n)`, `trend(metric, n)` (window-parameterized period math).
- **criteria-only extension** — comparators (`>= <= > < == ~=`), boolean `AND OR NOT`, percent literals (`15%`).

The **arithmetic subset** evaluates `kpi_definitions.formula` to produce metric values (the metrics service). The **full grammar** evaluates a framework criterion to a verdict. One grammar, one source of truth (this ADR + contracts), rendered for users in `wiki/dsl-reference.md`.

### 2. The shared derived-metrics service is a catalog-driven core module, not a port

`src-tauri/src/fundamentals/metrics.rs` builds an **in-memory, never-persisted** `ComputedMetrics` table (metric_key → value + unit + kind + provenance of the facts/periods used) for one company at one period, by evaluating `kpi_definitions.formula` topologically (derived-on-derived: `fcf_conversion → free_cash_flow → capex`). Reported metrics pass through from confirmed facts.

It is a **shared core module with a clean public API**, reused by frameworks now and by comparison (`v0.53.0`) and valuation (`v0.54.0`) later. Per [ADR 0039](0039-ports-and-adapters-posture.md) rule #4 it is **not** abstracted behind a trait/port: it has multiple consumers but exactly one implementation, so a port would be premature complexity. A `Missing` input yields an `Unavailable` metric, never an error.

### 3. The metric catalog is open; criteria reference data, not code

There is **no hardcoded metric list**. The metrics service computes from whatever `kpi_definitions` rows exist, across **all** scopes. This milestone:

- **Tops up the canonical library** so popular computed metrics ship out of the box independent of any framework — liquidity (`current_ratio`, `quick_ratio`), leverage/coverage (`debt_to_equity`, `interest_coverage`), `payout_ratio`, `fcf_margin`, `fcf_yield`, `asset_turnover`, and similar gaps over the already-broad `0034` catalog. Growth rates stay **DSL functions** (`cagr(revenue, 5)`), not fixed rows.
- **Adds a global `user` scope** to `kpi_definitions` (alongside `canonical` / `sector` / `company`) for user-defined custom metrics, so a framework can reference a metric the user defines (e.g. `rule_of_40 = revenue_growth + fcf_margin`). This reuses the existing `create_kpi_definition` command and `formula` field; the metrics service evaluates it with no change. **This milestone ships the data path + scope; the custom-metric *authoring UI* is deferred** (engine-extensible now, UI a later fast-follow). Adding the scope value is handled by a guarded table rebuild in the migration if the existing `scope` CHECK constraint is restrictive.

Extensibility is thus achieved through **data** (catalog rows), which is the ADR-0039-correct way to get pluggability without a new port.

### 4. Criteria are a free-text DSL; the framework owns them

A framework is a named, user-owned set of criteria. Each criterion stores its **expression as DSL text** (the epic's "rule expression grammar") plus a cached parsed AST. Compound logic (`AND`/`OR`) is expressible in one criterion. The engine records the **measured value** of the leading metric for display, and a per-criterion verdict: `pass | partial | fail | unavailable`. `partial` is an optional near-threshold band on a criterion; `unavailable` is produced when a referenced metric cannot be computed (missing fact) — distinct from `fail`.

### 5. Evaluation is a manual run over the latest period, persisted as an immutable snapshot

`evaluate_framework(company, framework)` is an explicit user action that assesses the **latest available period/TTM**. It builds `ComputedMetrics` in memory, evaluates each criterion, then persists **only** an immutable `framework_evaluations` row (pinning the framework version and period) plus per-criterion `criterion_results` rows (verdict, measured value, threshold snapshot, and the specific facts/periods each criterion used). The scorecard is the latest run; history is queryable. The measured value is **pinned to the run** and does not change when underlying facts later change (a final supersedes an estimate) — which is exactly why we snapshot rather than recompute on read, mirroring the `financial_facts` provenance model. `ComputedMetrics` itself is never stored. Auto-on-new-facts / autopilot is deferred to `v0.49.0`/`v0.50.0`.

### 6. Frameworks are editable in place; origin is provenance, not a lock; templates reset from a Rust constant

`quality_frameworks.origin` is `app_template | user` — a **provenance label, not an edit permission**. **Every** framework and its criteria are editable and deletable in place regardless of origin, mirroring the `kpi_definitions.scope` `canonical`/`company` convention. **Clone** is a separate convenience that duplicates any framework into a new `user` variant with `cloned_from` set.

App-template updates **never overwrite** an edited framework. The seed migration uses `INSERT OR IGNORE` on a stable id, so re-runs never clobber user edits. An `app_template`-origin framework instead offers an explicit **Reset to template defaults** action (`reset_framework_to_template`) that re-derives its criteria from the shipped definition. The Kroeze (and any future) template definition therefore lives as a **Rust constant** that is the single source for both the migration seed and the reset path, so the two cannot drift. (Versioned re-seed / "template updated" state is deliberately deferred — `INSERT OR IGNORE` + explicit reset is the v1 contract.)

### 7. Architecture posture: domain core, no new ports (ADR 0039)

This milestone is package-by-feature domain core in `fundamentals/` and introduces **no new ports**. It binds to the ports it already crosses: the UI↔Rust typed-command seam (all access via typed Tauri commands/events; the React UI holds no SQL/secrets), the import/export format-adapter boundary ([ADR 0018](0018-import-export-boundaries.md)) for the frameworks bundle section, and the domain-coupled storage facade (no `Repository` trait, per [ADR 0039](0039-ports-and-adapters-posture.md) §3). The scorecard output is kept a clean typed value so the future `AdvisoryVerdictProvider` port ([ADR 0042](0042-advisory-verdict-port-and-open-core-boundary.md)) can consume it without rework.

### 8. Enforcement (per ADR 0038 / ADR 0045)

- Migration `0048_quality_frameworks.sql` is append-only, idempotent, and self-healing; the seed is `INSERT OR IGNORE`; the `scope` CHECK rebuild preserves existing rows; reads of the new tables/columns tolerate a missing row.
- **Grammar-drift gate:** a test asserts every seeded app-template criterion expression **and** every `kpi_definitions.formula` parses and validates with the one engine — so a formula or template can never ship unparseable.
- **Missing-input gate:** a test asserts an absent metric yields `unavailable`, never a panic or error.
- **Decision-support gate:** criterion expressions evaluate to measurements and verdicts only; they cannot encode buy/sell/hold output. Enforced as a documented rule in contracts + a review-checklist item (a free-text DSL cannot be cleanly machine-checked for advice, so per ADR 0045 a doc rule is chosen over a noisy automated gate). This preserves the `AGENTS.md` decision-support rule.
- Verdict, origin, and scope values are validated at the storage boundary, matching the existing `company_events`/`kpi_definitions` validation pattern.
- en/pl translation parity and the translation/pluralization/a11y guards cover the new screen copy.

## Consequences

- The missing derived-metrics computation layer comes into existence as a **reusable** module, unblocking `v0.53.0` and `v0.54.0` rather than being built rule-engine-locally and duplicated later.
- One expression grammar serves both metric formulas and criteria, with a single canonical definition (this ADR + contracts) and a user-facing rendering (`wiki/dsl-reference.md`) kept in lockstep.
- The metric catalog becomes user-extensible through data; the only deferred piece is the authoring UI, called out explicitly so the seam is not mistaken for missing scope.
- A new top-level `wiki/` directory is introduced for end-user how-to docs, distinct from the canonical `docs/` specs; the DSL grammar's single source of truth remains the ADR/contract.
- The milestone adds one new domain slice (engine + metrics service + frameworks), one migration with four tables plus catalog top-up and a new KPI scope, a set of typed commands, one new company-detail tab, and three wiki pages. No new external source, no AI, no new port.
