# ADR 0069: Source Reliability and Disclosure Signals

Status: Accepted — **decision 2's attention-routing amendment amended 2026-08-06 by [ADR 0097](0097-toasts-are-action-feedback-only.md)**: an `espi_only` event surfaces via the Today stream + sidebar badge (no toast), and its Review opens the missed report through the stored witness URL

The official-report core of the product currently rides on a single active channel (Bankier per-company pages; the GPW ESPI/EBI adapter is registered but disabled since migration 0011), and the `SourceAdapter` boundary is a metadata catalog without a behavioral contract — adding a source means bespoke fetch code wired imperatively into `jobs/source_refresh.rs`. This ADR fixes both and extends the disclosure-signal taxonomy with two investor-relevant categories.

## Context

- A 2026-07-03 audit: `SourceAdapter` (registry.rs) exposes only descriptor metadata — no polymorphic `fetch()`; `source_refresh.rs` (~960 lines) branches per source. This contradicts the "US/EU adapters later without changing the core feed model" promise and inflates every new-source cost (ownership, analyst recommendations, KNF, market data all add adapters).
- Bankier as the only official-report channel is a single point of failure; source-strategy carried an open question on reconciling Bankier/Parkiet ESPI items with canonical GPW report IDs.
- The typed-signal taxonomy (ADR 0034) lacks auditor-opinion/going-concern and major-holdings categories, both formulaic disclosures well suited to the deterministic rule classifier.

## Decision

1. **`Fetcher` trait beside the descriptor** (the registry stays a const metadata catalog): the trait is the refresh behavior — `refresh(&self, state, ctx) -> Result<RefreshOutcome, _>` implemented per adapter; `source_refresh.rs` dispatches polymorphically and each adapter keeps its internal item types. Amends the ADR 0050 "SourceAdapter port realized" note — the port gains behavior. Existing adapters migrate strangler-style; new adapters (KNF, market data, analyst recs, ownership) implement the trait from day one. *Amended 2026-07-15 (owner-approved): the original `fetch() -> Vec<RawItem>` wording assumed a unified pre-pipeline item type that has never existed — adapters produce per-source item structs and the existing dispatch (`RefreshBehavior`) is already registry-driven. The trait therefore sits at the refresh level; a shared item shape is deliberately **not** introduced now. Marker: when a third disclosure-type source lands (ownership v0.56 / analyst recs v0.58), evaluate a common normalized item shape for the `disclosure` category, evidence-first.*
2. **ESPI/EBI as a second witness**: re-enable the GPW ESPI/EBI channel in a reconciliation role — match items to Bankier-sourced reports by (company, disclosure date, report type/number); agreement is recorded per pair (`matched | bankier_only | espi_only`). Bankier remains the primary ingestion path; witness items never enter the feed (no dual ingestion), so deduplication is impossible by construction. The witness closes the SPOF and answers the open source-strategy question. Promotion of ESPI to co-primary is a later, evidence-gated decision. *Amended 2026-07-15 (owner decision): mismatch surfacing must reach where the owner actually looks — an `espi_only` result for a tracked company raises an attention event through the ADR 0068 routing (toast + morning briefing line, with the missed report previewable); developer diagnostics keep the full reconciliation ledger but are not the primary outlet.*
3. **KNF short-selling registry adapter**: the public national register of net short positions becomes a `disclosure`-type adapter — per-company short-position entries (holder, size, date) stored with history and surfaced as a typed signal (`short_position_change`), a company-workspace readout, **and a dedicated dashboard panel** (owner decision 2026-07-15: short-position register for tracked companies with history, mockup-first per ui-authoring). Official public source; conservative daily polling; standard attribution. *Probe 2026-07-15: the register exposes a stable public JSON endpoint (current + historical methods) — no HTML scraping needed; details on card `6204cd0`.*
4. **Auditor-opinion / going-concern signal**: a new deterministic rule-classifier category over filing titles/categories/body (qualified opinion, disclaimer, going-concern emphasis), with the existing opt-in confirm-before-apply AI fallback (ADR 0034 semantics). High-signal: feeds the red-flags panel (v0.57).

## Consequences

- New-source marginal cost drops to "implement one trait + descriptor" — the extensibility story matches the architecture promise.
- `source_refresh.rs` shrinks toward pure orchestration.
- Two new signal categories join feed badges/filters, digests, and (later) alert rules (ADR 0068) and the red-flags panel.
- Reconciliation telemetry gives the first measured answer on Bankier completeness vs the official channel.
