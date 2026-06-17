# ADR 0041: Deterministic Valuation Engine (Proposed)

Status: Proposed (draft — planning)

This ADR captures the **design intent** for a deterministic valuation engine: a pure-computation
domain slice that turns confirmed fundamentals into inspectable, scenario-based fair-value estimates.
It is the foundation of the "decision-making augmentor" direction — the app should provide not only
sourced facts but computed valuation, as **decision support**, to help a user judge a company.

It builds on:

- [ADR 0027](0027-company-fundamentals-scope.md) — fundamentals scope and the `financial_facts` model
  (decimal-exact TEXT + `rust_decimal`, fact dimensions, provenance) the engine consumes.
- [ADR 0039](0039-ports-and-adapters-posture.md) — package-by-feature internals; **storage is not a
  port** (SQLite is the single source of truth), so valuation tables are added via append-only forward
  migrations, not behind a repository trait.
- [ADR 0028](0028-multi-provider-ai-boundary.md) / [ADR 0035](0035-two-layer-ai-and-local-interpretative-layer.md)
  — the determinism boundary: valuation math is deterministic Rust; AI is used only upstream (fact
  extraction) and for optional narrative synthesis, **never to compute the number**.

## Context

Brawler already builds the substrate for valuation: AI KPI extraction with mandatory confirmation,
the three-layer KPI model, cross-company comparison (`v0.53.0`), and a versioned quality scorecard
(`v0.44.0`). What does not exist is a layer that **values** a company from those facts: scenario price
targets, an FCF sanity cross-check, and peer-relative context.

Constraints specific to Brawler's universe:

- **Thin-market reality (GPW-first).** Most GPW sectors have few, illiquid listed comparables, so
  peer-median multiples are unreliable. The engine must not depend on deep peer sets to produce a
  defensible estimate.
- **Local-first, deterministic, testable.** Valuation must be reproducible and unit/snapshot-tested;
  no cloud, no AI in the arithmetic.

## Decision (intended)

1. Add a pure-Rust `valuation` domain slice consuming confirmed `financial_facts`. No external calls;
   deterministic; fully testable.
2. **Methods, market-aware default:**
   - **DCF / owner-earnings** as the default for thin markets (GPW) — peer-independent, with explicit,
     inspectable assumptions.
   - **Multiple-based bear/base/bull scenarios** (e.g. EV/EBITDA) where peer sets are deep enough.
   - **FCF cross-check**: normalize maintenance vs growth capex → EV/normalized-FCF sanity multiple.
   - **Peer-relative multiple** with an explicit thin-flag (reuses `v0.53.0` comparison + materialized
     sector sets); surfaced as context, not a sole basis, when N is small.
3. **Outputs:** bear/base/bull fair-value per share, upside vs current price (and vs analyst consensus
   later, behind a separate adapter/ADR), and a **what-if / sensitivity** capability (instant local
   recompute; rank inputs by impact on fair value).
4. **Persistence:** store valuation results as **append-only `valuation_runs`** (with provenance reused
   from the fundamentals model: `provider_id`/`model_id` where AI-assisted inputs are involved,
   `data_as_of`, confirmation/coverage state) to enable what-changed diffs and the thesis/journal layer.
5. **Framing:** all output is **decision support** — scores, ranges, assumptions, and "what to watch."
   This ADR introduces no prescriptive (buy/sell/hold) output; that boundary is owned by
   [ADR 0042](0042-advisory-verdict-port-and-open-core-boundary.md).

## Consequences

- New append-only migration(s) for valuation tables; no change to the storage posture.
- Enables the scoring valuation-dimension (ADR 0042), the thesis workbench (ADR 0043), and the
  watchlist screener.
- A future SEC EDGAR XBRL fundamentals adapter (US coverage) and an analyst-consensus adapter feed this
  engine; both are separate, later ADRs and do not block it.

## Open questions

- Exact DCF assumption surface and how user overrides are captured/provenance-stamped.
- Valuation-run retention/versioning detail vs the backup/retention policy.
- Final milestone sequencing (pulled forward — see roadmap).
