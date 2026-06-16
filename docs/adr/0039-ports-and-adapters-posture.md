# ADR 0039: Ports and Adapters posture — hexagonal at the seams, package-by-feature inside

## Status

Accepted.

## Context

Brawler's module organization was never explicitly mapped onto the Ports and Adapters (Hexagonal) metapattern, even though it uses that pattern in several places. The question came up directly: how does Brawler's modularization compare to hexagonal architecture, and should the project adopt it more strictly — for example by putting a repository port in front of SQLite storage?

The metapattern isolates a business-logic core from external dependencies through ports (interfaces the core defines) and adapters (translation layers to concrete external systems), so the externals become replaceable. It explicitly notes the cost — indirection, boilerplate, hindered optimization — and that it is a poor fit for small components or where there is no real second implementation.

Brawler already applies this pattern where it pays off, but the decision of *where* it applies (and, just as importantly, *where it deliberately does not*) lived only in implicit structure and scattered ADRs. Without recording it, a future agent could either prematurely abstract a single-implementation seam or fail to add a port where one is genuinely warranted. This ADR makes the posture explicit. It records existing decisions; it does not mandate a refactor.

Related: [ADR 0016](0016-provider-neutral-ai-analysis-framework.md) (provider-neutral AI), [ADR 0028](0028-multi-provider-ai-boundary.md) (multi-provider boundary), [ADR 0035](0035-two-layer-ai-and-local-interpretative-layer.md) (interpretative capability contracts), [ADR 0032](0032-search-and-backup-boundaries.md), [ADR 0018](0018-import-export-boundaries.md), [ADR 0017](0017-license-gate.md), and [modularization-design.md](../modularization-design.md).

## Decision

**Brawler is hexagonal (Ports and Adapters) at its external seams and package-by-feature inside the Rust domain core.** The pattern is applied where the metapattern's payoff is real — a seam with more than one plausible implementation, an untrusted/replaceable external dependency, or a vendor-lock-in risk — and consciously declined where it is not.

1. **Ports exist at the external seams.** The following are ports with interchangeable adapters, and new work must bind to the port, never to a concrete implementation:
   - **Source adapters** — normalized records behind a common interface ([source-strategy.md](../source-strategy.md)).
   - **AI providers** — provider/model/credential-neutral interfaces ([ADR 0016](0016-provider-neutral-ai-analysis-framework.md), [ADR 0028](0028-multi-provider-ai-boundary.md)).
   - **Interpretative capabilities** — `Classifier`, `SimilarityProvider`, `Matcher`, `SemanticSearch` capability contracts with a static baseline adapter and optional embedding adapter ([ADR 0035](0035-two-layer-ai-and-local-interpretative-layer.md)). This is the project's reference implementation of the metapattern.
   - **Credentials** — a reusable secret-kind boundary, not provider-specific storage.
   - **Search, backups/restore, the connection pool** ([ADR 0032](0032-search-and-backup-boundaries.md)) and **import/export format adapters** ([ADR 0018](0018-import-export-boundaries.md)) — requested through typed boundaries, never by touching files or internals directly.
   - **Licensing** — parser/verifier/entitlement-policy adapters ([ADR 0017](0017-license-gate.md)).
   - **The UI ↔ Rust seam** — the React UI is a driving adapter that reaches the domain only through typed Tauri commands and events; it never holds secrets, SQL, or filesystem access.

2. **Inside the core, organize by domain (vertical slice), then by layer.** The metapattern does not prescribe the core's internal structure; Brawler fills that gap with package-by-feature (`companies/`, `feed/`, `notebooks/`, …) as defined in [modularization-design.md](../modularization-design.md), not with onion-style concentric layers.

3. **Storage is a deliberately domain-coupled persistence layer, not an abstract repository port.** SQLite is the single runtime source of truth for a local-first app ([architecture.md](../architecture.md)). Domain code in `storage/*` may know it is talking to SQLite-shaped rows. We do **not** introduce a `Repository` trait / domain-vs-row type split, because that seam has exactly one adapter and the indirection would be paid on every read for no current benefit — which is precisely the "poor fit, no second implementation" case the metapattern warns against, and the "avoid premature complexity not tied to a real planned extension" rule in [AGENTS.md](../../AGENTS.md).

4. **Do not add a port without a real or concretely planned second implementation.** A port whose population is permanently one adapter is premature complexity. Conversely, when a seam gains a genuine second implementation or replaceability requirement, add the port as part of that feature slice rather than hard-coding the second path.

## Storage-port trigger

Introduce repository ports in front of storage **if and when** any of the following becomes real planned scope, recorded as its own ADR at that time:

- a second storage backend (non-SQLite) or an embedded alternative,
- a sync / replication / multi-device engine that needs a storage-neutral domain,
- a non-SQLite durable import/sync target that the domain must write through.

Until a trigger fires, new features bind to the existing storage facade and domain storage modules.

## Consequences

- The implicit architecture becomes explicit and guard-railed: feature planning can state which seam a change touches and whether it is a port (extend the adapter set) or core (add to the domain slice).
- Agents have a recorded test for when to add an abstraction, preventing both premature repository-style ports and missed ports at genuinely replaceable seams.
- No code changes follow from this ADR; it documents and aligns existing decisions. A storage-port refactor remains deferred behind the trigger above.
- This complements [ADR 0038](0038-enforcement-as-guardrails.md): the doc records the posture; existing boundary tests (provider/source/credential/interpretative contracts) are the gates that keep the ported seams from being bypassed.
