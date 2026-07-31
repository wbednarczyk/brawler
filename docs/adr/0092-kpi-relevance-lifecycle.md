# ADR 0092: kpi_relevance lifecycle — layered expectations, no self-referential gate

Status: Accepted (2026-07-31, layers 2–3 implementation PR — issues #273 / #274; proposed 2026-07-30 as the epic #229 T7 study, issue #81)

Deciders: maintainer. Area: fundamentals, data trust.

## Context

`kpi_relevance` is the denominator of fundamentals honesty: `expected_primary_metric_keys`
feeds the ADR 0061 decision 2(d) completeness gate (an extraction that hits none of a
company's expected primary KPIs downgrades to `AcceptedUnreviewed`) and the A4 recall
measurement. The v0.59 A4 harness proved the table empty in production — the gate was
inert and recall unmeasurable. Migration 0106 seeded a five-key IFRS core (revenue,
operating_profit, net_profit, total_assets, total_equity) as a stopgap; the epic #229
audit then found the hole regrows (companies created after 0106 had zero rows — fixed by
the create-time seed shipped with T7). This ADR answers issue #81: how is `kpi_relevance`
populated **durably**?

Substrate that already exists: `kpi_definitions` sector packs (banking 12 keys, insurance
7, specialty_finance 4, reit) seeded but with **no runtime selection layer**;
`companies.statement_type` plus the conservative registry-sector → statement_type bridge
(migrations 0095/0098); the manual-wins precedent (`sector_source`, holder types);
`kpi_relevance.source` free-text with the conventional vocabulary `core | sector |
derived | user`.

## Decision: four layers, strict ownership, and a gate that never eats its own tail

1. **Core floor (`source='core'`)** — the five universal IFRS keys, seeded at company
   creation (T7) and healed by migration for existing gaps. Automation never removes or
   reranks core rows. This floor alone makes the completeness gate live and recall
   measurable for every company.

2. **Statement-pack additions (`source='sector'`)** — rows derived from
   `companies.statement_type` over the existing `scope='sector'` definition packs, seeded
   at creation and by a healing migration, **conservative subset only**: keys that are
   genuinely universal within the statement type (a bank's net interest income — not
   every pack key some insurer might report). A `statement_type` change re-seeds
   additively (INSERT OR IGNORE); it never deletes.

3. **Derived observations (`source='derived'`) enrich but NEVER gate.** A background pass
   may mark keys the company consistently reports (issuer-tier facts in ≥3 of the last 4
   periods) — powering the company-characteristic KPI surface (issue #149) and coverage
   display. **Derived rows are excluded from `expected_primary_metric_keys`.** Rationale
   (the reason plain "derive it from history" is rejected as the gate source): the
   completeness gate compares extraction output against expectations; deriving the
   expectations from extraction output makes a systematic extraction hole (a parser that
   never yields equity) silently erase the very expectation that would have caught it.
   The gate's denominator must come only from layers independent of extraction: core,
   statement-pack, user.

4. **User/MCP curation (`source='user'`) always wins** — existing create/update/delete
   commands; automation treats user rows as untouchable (INSERT OR IGNORE everywhere,
   manual-wins precedent). Ownership model per issue #81(d): automatic with user
   override.

Non-decisions kept out: per-company automatic *primary* selection beyond the layers above
(no evidence it beats statement packs); expanding the all-or-nothing gate to partial
thresholds (interim reports legitimately carry fewer keys — `present == 0` remains the
only downgrade trigger); quality-framework coupling (criteria are user judgment, relevance
is expectation — a future UX may *suggest* relevance from criteria, never auto-write it).

## Consequences

- Epic #229 shipped the core-floor lifecycle (create-time seed + healing migration) and
  this ADR. **All four layers are now live** (2026-07-31, issues #273 / #274): layer 2 as
  `seed_statement_pack_kpi_relevance` + migration `0126`, layer 3 as
  `refresh_derived_kpi_relevance`, both converging per company on the daily aggregator
  pull; layer 4 was already there. The no-self-referential-gate rule is enforced
  structurally — `expected_primary_metric_keys` filters `source != 'derived'` rather than
  relying on the `secondary` rank layer 3 happens to write, and a guard test hand-upgrades
  a derived row to `primary` to prove it still cannot gate.
- Two findings from implementing layer 2, both recorded rather than papered over:
  - The conservative subsets are **banking** (`net_interest_income`,
    `net_fee_commission_income`, `total_loans`, `total_deposits`), **insurance**
    (`gross_insurance_revenue` — the IFRS 17 top line), **reit** (`ffo`), and
    **specialty_finance: nothing**. Migration 0095 maps exchanges and brokerage houses
    (GPW, XTB) onto the same `statement_type` as debt collectors (KRU), and the pack
    (`recoveries`, `erc`, `cash_ebitda`, `portfolio_purchases`) is debt-collector
    vocabulary — no key is universal across that mix. Splitting the type is a separate
    decision, not one this ADR forces.
  - `statement_type` had **no runtime write path at all** — migrations 0095/0098 only,
    which made them one-shot exactly like 0106 was. `create_company` now runs the same
    registry-sector bridge in-transaction, so a bank tracked today is classified today
    (and stops receiving a meaningless Altman Z″ under the ADR 0083 D4 gate). Layer 2's
    convergence on the daily pull is what makes any later reclassification additive,
    since there is no setter to hang it off.
- A4 recall becomes measurable with an honest, extraction-independent denominator; the
  chicken-and-egg of newly tracked companies (#81 b) is dissolved by layers 1–2 being
  available at creation time, before any extraction runs.
- The class "a documented gate is inert because its data never arrives" gains its
  guardrail: relevance provisioning is part of company creation, not a one-shot
  migration.
