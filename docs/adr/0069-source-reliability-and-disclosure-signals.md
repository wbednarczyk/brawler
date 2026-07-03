# ADR 0069: Source Reliability and Disclosure Signals

Status: Accepted

The official-report core of the product currently rides on a single active channel (Bankier per-company pages; the GPW ESPI/EBI adapter is registered but disabled since migration 0011), and the `SourceAdapter` boundary is a metadata catalog without a behavioral contract — adding a source means bespoke fetch code wired imperatively into `jobs/source_refresh.rs`. This ADR fixes both and extends the disclosure-signal taxonomy with two investor-relevant categories.

## Context

- A 2026-07-03 audit: `SourceAdapter` (registry.rs) exposes only descriptor metadata — no polymorphic `fetch()`; `source_refresh.rs` (~960 lines) branches per source. This contradicts the "US/EU adapters later without changing the core feed model" promise and inflates every new-source cost (ownership, analyst recommendations, KNF, market data all add adapters).
- Bankier as the only official-report channel is a single point of failure; source-strategy carried an open question on reconciling Bankier/Parkiet ESPI items with canonical GPW report IDs.
- The typed-signal taxonomy (ADR 0034) lacks auditor-opinion/going-concern and major-holdings categories, both formulaic disclosures well suited to the deterministic rule classifier.

## Decision

1. **`Fetcher` trait beside the descriptor** (the registry stays a const metadata catalog): `fetch(&self, ctx) -> Result<Vec<RawItem>, _>` implemented per adapter; `source_refresh.rs` dispatches polymorphically. Amends the ADR 0050 "SourceAdapter port realized" note — the port gains behavior. Existing adapters migrate strangler-style; new adapters (KNF, market data, analyst recs, ownership) implement the trait from day one.
2. **ESPI/EBI as a second witness**: re-enable the GPW ESPI/EBI channel in a reconciliation role — match items to Bankier-sourced reports by (company, disclosure date, report type/number); agreement is recorded, mismatches and misses surface in developer diagnostics and a source-health signal. Bankier remains the primary ingestion path; the witness closes the SPOF and answers the open source-strategy question. Promotion of ESPI to co-primary is a later, evidence-gated decision.
3. **KNF short-selling registry adapter**: the public national register of net short positions becomes a `disclosure`-type adapter — per-company short-position entries (holder, size, date) stored with history and surfaced as a typed signal (`short_position_change`) plus a company-workspace readout. Official public source; conservative daily polling; standard attribution.
4. **Auditor-opinion / going-concern signal**: a new deterministic rule-classifier category over filing titles/categories/body (qualified opinion, disclaimer, going-concern emphasis), with the existing opt-in confirm-before-apply AI fallback (ADR 0034 semantics). High-signal: feeds the red-flags panel (v0.57).

## Consequences

- New-source marginal cost drops to "implement one trait + descriptor" — the extensibility story matches the architecture promise.
- `source_refresh.rs` shrinks toward pure orchestration.
- Two new signal categories join feed badges/filters, digests, and (later) alert rules (ADR 0068) and the red-flags panel.
- Reconciliation telemetry gives the first measured answer on Bankier completeness vs the official channel.
