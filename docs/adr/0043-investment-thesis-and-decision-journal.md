# ADR 0043: Investment Thesis Workbench & Decision Journal (Proposed)

Status: Proposed (draft — planning)

This ADR captures the **design intent** for persisting an investment thesis as a first-class entity and
for a decision journal — the memory and accountability layer of the decision-making augmentor.

It builds on:

- [ADR 0041](0041-deterministic-valuation-engine.md) — valuation runs the thesis links to.
- [ADR 0042](0042-advisory-verdict-port-and-open-core-boundary.md) — the scorecard/verdict the thesis
  records (decision-support in open-core).
- [ADR 0022](0022-research-evidence-read-model-boundary.md) and [ADR 0040](0040-management-claims-tracker.md)
  — the evidence-link graph and the first-class-entity + AI-extract-with-confirmation patterns this reuses.
- [ADR 0039](0039-ports-and-adapters-posture.md) — storage is not a port; new tables via append-only
  forward migrations.

## Context

After computing a valuation and a scorecard, the user needs to **record a view and act on it**:
a thesis (what I believe and why, with scenarios and what would change my mind) and a decision
(did I buy/pass, at what price, and how did it turn out). Today nothing persists a thesis as an entity;
"trade journal" and "portfolio position tracking" are explicitly listed as **Not in V1** — this ADR
reschedules a scoped decision journal into the valuation & decision arc.

## Decision (intended)

1. **Thesis entity** (append-only/versioned), provenance-stamped (`provider_id`/`model_id` where
   AI-assisted, `data_as_of`), holding: decision-support verdict + score reference, scenario forecasts
   (bear/base/bull), a **variant** field (where the view differs from consensus and why), an
   **inversion** field (what would break the thesis), and **disclosed gaps / coverage state** (a
   confidence meter generalizing the "what do we actually know" signal).
2. **Linkage:** link a thesis to its valuation run(s) and supporting evidence via the existing
   evidence-link graph, with an orphan check before linking (no dangling references).
3. **Decision journal:** record the user's decision (buy/pass/watch), price/context, rationale, and a
   later outcome review — the loop that lets the user learn from their own calls. Scoped to journaling,
   not portfolio/position accounting (which remains out of scope).
4. **Freshness hook:** designed so the living-thesis milestone can mark a thesis stale and trigger
   re-scoring when new feed items, reports, signals, or events arrive (see roadmap `v0.57.0`).

## Consequences

- New append-only migration(s) for thesis, forecast, and decision-journal tables; reuses evidence-link
  and provenance models; no change to the storage posture.
- Provides the persisted context the watchlist screener and the living-thesis differentiator build on.

## Open questions

- Thesis versioning granularity vs valuation-run versioning, and how what-changed diffs are computed.
- Exact decision-journal fields and how outcome review is prompted without drifting into portfolio
  accounting.
- Import/export coverage for theses and journal entries (ties to `v0.52.0`).
