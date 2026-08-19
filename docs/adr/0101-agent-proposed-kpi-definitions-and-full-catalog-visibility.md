# ADR 0101: Agent-proposed KPI definitions and full-catalog visibility

Status: Proposed (2026-08-18, owner decision at epic #399 planning; sol r1 folded, local skeptic re-verify SHIP-WITH-FIXES). Amends [ADR 0099](0099-acquisition-mcp-surface-mechanics.md) decision 3 (allowlist widened 9→10) and resolves its tension with [ADR 0093](0093-agent-acquisition-tier-and-preliminary-lifecycle.md) decision 4. Implementation: epic #399.

Deciders: maintainer. Area: fundamentals, MCP port, data trust.

## Context

Epic #399 (agent-path full capture for untagged documents — preliminary/interim GPW and NewConnect filings the agent reads directly, mapping into the vocabulary [ADR 0100](0100-two-layer-tagged-fact-capture-and-ifrs-vocabulary.md) establishes) surfaced a proven contradiction between two accepted ADRs:

- ADR 0093 decision 4 says agents **may mint** KPI definitions for issuer-characteristic metrics the catalog lacks (company-scoped, `origin='agent'`).
- ADR 0099 decision 3 freezes the acquisition allowlist at exactly **nine** workflow tools — no `create_kpi_definition`, no minting path at all on the scope that actually runs ingestion.

An agent doing full capture on an untagged document routinely meets a disclosed number with no catalog entry; without a minting path it is one of "map, propose, or exclude" down to two options, contradicting the epic's own DoD. The owner's resolution: a **tenth acquisition tool**, `propose_kpi_definition`, carrying a **hard anti-duplicate guard with no fuzzy matching** — `create_kpi_definition`'s raw `INSERT` stays reachable only from Full scope and its existing callers.

Facts verified on the tree (master 0011143) that bound the design:

- `build_catalog` (`kpi_ingest_context.rs:414-442`) today returns only `resolved_expected` plus company-origin rows tagged `origin=="agent"`; the ~373 canonical, non-expected definitions are dropped from every acquisition-scope read.
- The canonical catalog holds **373 canonical definitions** (403 seeds total: migration 0143 +87, 0148 +203 from corpus harvest, 0149 repair) on a freshly migrated in-memory database.
- A full `CatalogEntryDto` per canonical row costs ≈85 KiB serialized — too expensive to ship whole; a compact `{metricKey, label}` projection costs ≈35 KiB, ~6 pages at the existing `CATALOG_PAGE_MAX=64` under `RESPONSE_BUDGET_BYTES=262144`.
- A synonym mechanism already exists: `fundamentals/kpi_aliases.rs` ([ADR 0100](0100-two-layer-tagged-fact-capture-and-ifrs-vocabulary.md) decision 12) — one curated, evidence-proven, one-sided entry (`inventory` → `inventories`) already consumed inside `resolve_kpi_definition`, i.e. already on the catalog resolution path a new tool would need.
- The acquisition scope's `tools/list` gate is ≤16 KiB; measured today (compact) at **9 228 B** — roughly 7 KiB of headroom for a tenth tool's schema.

## Decisions

### 1. `propose_kpi_definition` is the tenth acquisition tool (amends ADR 0099 dec. 3)

Widening the allowlist is, per ADR 0099 decision 3 itself, "a deliberate ADR change" — this is that change. The freeze's stated rationale covered only the read side (the context read model already covers catalog, comparison facts and document bytes); it never reasoned about a write gap on a scope that has no minting path at all. `propose_kpi_definition` joins the nine existing acquisition tools at its own contract position.

### 2. Agent minting is exclusively through this tool; `create_kpi_definition` stays Full-scope

This resolves the ADR 0093 dec. 4 vs ADR 0099 dec. 3 tension directly: ADR 0093's "agents may mint" is honored, but only through a tool built for it. `create_kpi_definition` remains outside the acquisition allowlist and untouched for its existing Full-scope callers (see decision 3) — there is exactly one minting door on the acquisition scope, and it is this one.

### 3. A new narrow `get_or_create_kpi_definition` helper, used only by propose

`create_kpi_definition` (`financials.rs:447-517`) is a bare `INSERT`; a duplicate raises a raw `SQLITE_CONSTRAINT` on `(metric_key, scope, company, sector)`, and it has several existing callers (`commands/financials.rs:13`, `commands/tagged_fact_promotion.rs:364`, `mcp/acts.rs:435`) whose duplicate-handling contract a global semantic change would silently break. Rather than touch it, propose gets its own transactional helper: on an **exact key match**, it returns the existing definition typed (`created:false`) instead of erroring. `create_kpi_definition` itself stays strict-INSERT for every caller it already has.

### 4. The guard is exact-key match plus the curated synonym redirect — no fuzzy matching

Three checks, in order (pinned during S4 implementation — the real corpus forced the ordering): (1) an exact `metric_key` match on the run company's OWN minted rows returns that definition (`created: false`; a repeat proposal never suddenly redirects); (2) `kpi_aliases::resolve(key)` — the same table ADR 0100 decision 12 established — is consulted, and a hit returns a typed `synonym_redirect {canonicalKey, definitionId}` instead of minting; the redirect outranks shared-canon reuse because an alias source is a deprecated key whose zero-fact canonical row may still exist (`inventory` → `inventories`), and reusing it would resurrect the fragmentation the alias retires; (3) an exact `metric_key` match in the shared canon returns the CANONICAL definition (`created: false`) — proposing a key the canon already carries must never mint a company-scoped shadow whose duplicate key would fragment the catalog. This extends ADR 0100 decision 12's table without touching its invariants (one-sided, evidence-proven, no alias chains). One list serves two roles: `propose_kpi_definition` refuses duplicates through it, `resolve_kpi_definition` redirects reads through it — the same data, consulted consistently. No fuzzy/similarity matching and no new dependency: the owner's resolution was explicit that the anti-duplicate guard stays hard-edged.

### 5. Synonym curation stays owner-reviewed; harvested in the same PR

Near-misses surfaced by real proposal traffic (sandbox dogfooding, §G) are curated into `kpi_aliases.rs` by the owner, harvested in the same PR that surfaces them (guardrail-harvest loop) — never auto-added. The one-sidedness rule (an alias source must hold zero facts) stays enforced by ADR 0100's existing gates; this ADR adds no new gate, it adds entries to the table those gates already police.

### 6. Proposed definitions are company-scoped, `origin=agent`, and never enter the denominator

Reaffirms ADR 0093 decision 4 and ADR 0098 decision 4: a proposed definition is `scope='company'` for the run's own company, `kpi_definitions.origin='agent'`, and is an extra — never a completeness-denominator entry. The expected-KPI snapshot stamped on a run at creation is unaffected by anything an agent proposes mid-run.

### 7. The catalog widens to the full canon plus agent-company rows; user-origin stays excluded

`build_catalog` returns every canonical definition, not only `resolved_expected` — the ~373 canonical rows currently invisible to the acquisition scope become visible, so an agent can check "does this already exist" before proposing. User-origin rows stay excluded exactly as today (the existing test at `:1455` holds): the acquisition scope reads the shared vocabulary and its own company's agent-minted extras, never another scope's user-entered rows. Doctrine: **page the catalog to the end of its cursor before proposing anything** — a proposal made without having seen the full canon risks exactly the duplication this ADR exists to prevent.

### 8. Plausibility gets an explicit `notRequested` state

Widening the catalog must not silently widen what plausibility costs or implies. A canonical definition visible only because decision 7 exposed the full catalog, but not a member of the run's expected set, is `notRequested` — plausibility continues to be built only from `expected` (cost unchanged), and the absence of a plausibility entry is never allowed to be misread as "no history exists" for a metric nobody asked this run to observe.

### 9. The ≤16 KiB scoped `tools/list` gate stands; headroom is measured

The gate is not relaxed for the tenth tool. Baseline before this change: **9 228 B** (compact catalog schema, measured by sol on the current tree) — roughly 7 KiB of headroom absorbs one additional tool schema. The gate is re-measured after `propose_kpi_definition` lands (epic #399 slice S4) rather than assumed to hold.

### 10. MCP-only coverage: fidelity-corpus exempt, registry/storage tests plus dogfooding

Like the other nine acquisition workflow tools, `propose_kpi_definition` has no Tauri command twin and is therefore exempt from the dual-execution mock-fidelity corpus (`testing.md` § dual-execution scope exemption, established by ADR 0099's Consequences). Its coverage is registry/storage integration tests (`registry::call` against real storage) plus the MCP dogfooding ritual (`testing.md:1144`) as a closure step.

### 11. Proposing requires a live lease

`propose_kpi_definition` is lease-bound exactly like `stage_kpi_observations`: it validates the caller holds the run's live lease (holder + not-expired) before touching storage. A lapsed or taken-over lease refuses with the same typed codes (`run_lease_expired`/`run_taken_over`) the other write tools already use — no separate authorization story for the new tool.

## Consequences

- The Full-scope frozen tool count and the acquisition allowlist both move: `FROZEN_EXPOSED_TOOL_COUNT` and `KPI_ACQUISITION_TOOLS` grow by one, `tools/list` insta snapshots (Full and scoped) are regenerated, and every prose reference to "the nine workflow tools" (registry, server tests, `contracts.md`, the wiki MCP guide, the acquisition skill, docs-drift's error string) becomes ten — tracked as the epic's S4 blast-radius checklist, not repeated here.
- `docs/contracts.md` § KPI acquisition workflow tools gains `propose_kpi_definition`'s wire shape and joins the read/act inventory the registry already owns as the single source of truth for exact counts.
- The compact catalog projection (decision 7) means `list_kpi_definitions` (Full-scope, full metadata) stays the tool an agent would need for anything beyond a reuse-or-propose decision; the acquisition scope's compact `{metricKey, label}` rows are deliberately not a substitute for it.
- The epic's DoD — every disclosed number mapped, proposed, or excluded with a reason, never silently absent — gets its "proposed" leg from this ADR; "excluded with a reason" is [ADR 0102](0102-full-capture-staging-contract-excluded-observations-and-chunked-drafts.md).
