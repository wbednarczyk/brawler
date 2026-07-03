# ADR 0050: Architecture v2 — domain stores, pluggable source pipeline, entity resolution, durable jobs, frontend decomposition, vector scaling

Status: Accepted

> **Update (2026-07-03):** the `SourceAdapter` port gains a behavioral contract — a
> `Fetcher` trait with polymorphic dispatch from `source_refresh`, replacing per-source
> imperative branching — in [ADR 0069](0069-source-reliability-and-disclosure-signals.md).

> **Update (post-v0.45.1):** this ADR cites **story clustering (`v0.46`)** as a
> motivating consumer of the entity-resolution `story_key` and the embedding
> `SimilarityProvider`. That milestone was subsequently implemented, evaluated
> against real data, and **dropped** — no local method reached trustworthy
> precision at useful recall ([ADR 0051](0051-story-clustering-across-sources.md)).
> The Architecture v2 foundations here are unaffected and stand as delivered; the
> `story_key` remains a cross-source identity primitive, and the embedding model is
> re-pointed at ranking/retrieval consumers (semantic search `v0.48`, RAG for the AI
> milestones). The two carried-forward consumers named below — the **Rust-side
> scheduler** (decision 5, lands with the autonomous pipeline `v0.49`) and **ANN
> activation** (decision 6, near `v0.53`) — are still open as described.

## Context

A whole-application architecture review (the QA-architect pass that followed the
test-architecture work, [ADR 0048](0048-test-architecture-sample-data-broad-clickable-coverage-and-layered-parallelism.md)
/ [ADR 0049](0049-test-architecture-v2-data-transform-correctness.md)) found the
app cleanly layered and genuinely spec-driven, but carrying **two concentration
points** and **one missing realization** that the data-heavy roadmap (story
clustering `v0.46`, autonomous pipeline `v0.49`, cross-company compare `v0.53`)
will stress:

- **`AppState` is a 207-method storage facade** (`storage/mod.rs`, ~1991 lines) —
  the single chokepoint every command funnels through. It is the sanctioned
  facade ([Modularization Design](../modularization-design.md)), but at 207
  methods it is the merge-conflict and comprehension bottleneck.
- **Source adapters have no shared interface in code**, although
  [architecture.md](../architecture.md) and [ADR 0039](0039-ports-and-adapters-posture.md)
  already *declare* "source adapters" a port that "should return normalized
  records through a common interface." Eight adapters are wired bespoke through
  `storage/sources.rs` (~1078 lines) and `jobs/source_refresh.rs` (~27.5k) — the
  port is declared but unrealized.
- **Ingestion/unification logic is scattered** (per-adapter `normalize_*`,
  matching in `feed_matching`, dedup in adapters, event derivation in a job) —
  there is no single pipeline, which is exactly what "many sources → one unified
  set" needs.
- **The frontend mirrors the backend chokepoint**: `AppStateRoot.tsx` (~1967
  lines, ~146 hooks) plus god-hooks (`useResearchController` ~694,
  `useAppViewModel` ~592).
- **Jobs are fire-and-forget** `spawn_blocking` tasks with no durable queue; a
  crash mid-job loses the work. [architecture.md](../architecture.md) already
  notes Rust-side scheduling as deferred hardening for the `v0.49` pipeline.
- **Similarity is a brute-force cosine scan** (no ANN) — documented as fine at
  watchlist scale, with the swap boundary already reserved.

Critically, several of these are **realizations of already-documented intent**,
not new architecture: the source-adapter port (ADR 0039), the Rust-side
scheduler (architecture.md "Source Refresh Scheduling"), and the ANN swap
(architecture.md "Extensibility Boundaries") are all pre-declared. This ADR turns
that intent into a planned, sequenced program and records the decisions taken
during planning. **The test-architecture-v2 harness (ADR 0049) is the safety net
that makes these refactors safe** — the dual-execution mock-fidelity contract,
golden snapshots, transform invariants, and the e2e pipeline test all fail if a
refactor changes observable behavior. Hardening tests first, then refactoring, is
the deliberate sequence.

This is organized as a single umbrella **Architecture v2** epic with the six
items below as children (cross-cutting, non-product, like the test epics; lands
ahead of `v0.46`).

## Decisions

### 1. Split the `AppState` facade into concrete domain stores (NOT a repository port)

The single 207-method `AppState` is split into **focused, domain-grouped concrete
structs** — `CompanyStore`, `FeedStore`, `ResearchStore`, `FundamentalsStore`,
`SourcesStore`, … — each owning a pool handle and exposing only its domain's
operations. Commands depend on the **specific store** their domain needs; a thin
composition root (`AppState`, or Tauri-managed state holding the stores) wires
them. The domain `storage/*` modules (already cohesive) are unchanged; this is a
structural split of the *facade*, done incrementally (strangler: establish the
pattern on company/watchlist/feed first, migrate the rest domain-by-domain,
shrink `AppState` to a composition root).

**This is explicitly NOT the `Repository` port [ADR 0039](0039-ports-and-adapters-posture.md)
declined.** The stores are concrete and SQLite-coupled; no swap abstraction, no
domain-vs-row trait, no second backend is introduced. ADR 0039's "storage is
intentionally not a repository port" stands — the storage-port trigger (a real
second backend / sync engine) has not fired. The decomposition is about
dissolving a god-object, not abstracting the database.

### 2. Realize the `SourceAdapter` port — a trait + registry

The source-adapter port already declared in [ADR 0039](0039-ports-and-adapters-posture.md)
and [architecture.md](../architecture.md) is **realized in code**: a
`SourceAdapter` trait (`fetch → parse → Vec<NormalizedSourceItem>`, plus the
declared metadata — source type, rate limits, supported markets, allowed fetch
mode) and a **registry** the refresh path iterates. Adapters are migrated behind
it **one at a time** (strangler), collapsing the bespoke per-adapter wiring in
`storage/sources.rs` and `jobs/source_refresh.rs`. A new source then becomes one
module implementing one trait — not new branches in two large files.

### 3. One generic ingestion pipeline + an entity-resolution module

The scattered ingestion logic is unified into **one pipeline** the registry
drives for every adapter: `parse → normalize → resolve-entity → dedup → upsert →
derive-events`. The new **entity-resolution module** owns canonical identity —
resolving a parsed item's company (by ISIN / qualified ticker / name signal) to a
single canonical company id, and (for `v0.46`) a canonical **story key** for
cross-source clustering — extracted from the per-adapter `normalize_*` and
`feed_matching` code it replaces. This is the direct enabler for story clustering
(`v0.46`) and cross-company comparison (`v0.53`). The transforms here plug into
the **invariant harness from [ADR 0049](0049-test-architecture-v2-data-transform-correctness.md)**
(idempotent, order-independent, deterministic, associative) and the e2e pipeline
test generalizes to cover the unified path.

### 4. Decompose the frontend state root into feature-scoped state

`AppStateRoot.tsx` and the god-hooks are decomposed into **feature-scoped state**
— a context (or lightweight store) per domain and per-screen controllers — so a
screen subscribes only to the state it uses. This lands **before `v0.48`** (feed
triage mode + command palette), which adds cross-cutting interaction state the
current single root cannot absorb cleanly. Stays within the primitive-first UI
contract ([ADR 0037](0037-ui-component-framework-and-authoring-contract.md)); no
new state library is adopted unless a child task justifies one against the
conservative-dependency posture.

### 5. Durable SQLite-backed job queue + Rust-side scheduler

The fire-and-forget `spawn_blocking` model is replaced by a **persisted job
queue**: a `jobs` table (status, attempts, payload, timestamps) in the existing
SQLite database, a worker loop that **claims / executes / retries / resumes**, and
the Rust-side scheduler enqueuing onto it — realizing the deferred hardening in
[architecture.md](../architecture.md) "Source Refresh Scheduling." This is
**local-first with no heavy new dependency** (it builds on the existing
SQLite/`r2d2`/migrations stack), survives crashes, and gives retry/backpressure/
observability. It lands **before `v0.49`** (autonomous report pipeline), which
requires durable, resumable work. The existing 13 jobs migrate onto it
incrementally. The local-first boundary holds: work runs only while the app is
open (background/closed execution remains the managed-AI frontier, out of scope).

### 6. Swap the brute-force cosine scan for an ANN index behind the existing port

The brute-force cosine over `content_embeddings` is swappable for an **ANN-backed
index behind the existing interpretation capability port**
([ADR 0035](0035-two-layer-ai-and-local-interpretative-layer.md)). The swap
boundary is the `VectorIndex` trait (`interpretation/vector_index.rs`):
`BruteForceVectorIndex` (the O(N) scan, T4-gated) is the default and
`AnnVectorIndex` is the approximate alternative.

**Candidate decision:** `sqlite-vec` is **rejected** — it is a C/native SQLite
extension and the shipped engine must stay pure-Rust to keep the `cargo-xwin`
Linux→Windows cross-build working. The pure-Rust **`instant-distance`** (HNSW)
was chosen; its dependency tree (libc/getrandom/parking_lot/rand bindings, no C
compilation) cross-compiles to `x86_64-pc-windows-msvc` — **validated** with
cargo-xwin. The vector index stays disposable/derived (rebuildable, reversible).

**Status:** the `AnnVectorIndex` implementation is **added now** (the dependency
+ the impl behind the trait, with brute-force-parity tests), superseding the
original "scheduled, not immediate" framing for the *engine*. Because building an
HNSW over candidates supplied fresh per call costs more than one linear scan, the
ANN's **production activation** is the persisted `content_embeddings` path (where
the index is large and reused) — that wiring lands when corpus scale justifies it
(near `v0.53`). The **T4 behavioral scale gate**
([ADR 0049](0049-test-architecture-v2-data-transform-correctness.md)) continues to
guard the linear-scan contract, and any ANN activation must preserve top-k
correctness against it.

## Consequences

- **One umbrella "Architecture v2" epic, six children** (Decisions 1–6),
  cross-cutting and non-product (no `milestone:vX.Y.0`), sequenced so each lands
  ahead of the feature that needs it: the source pipeline + entity resolution
  (2, 3) before `v0.46`; the frontend decomposition (4) before `v0.48`; the
  durable queue (5) before `v0.49`; the ANN swap (6) near `v0.53`. The facade
  split (1) is foundational and runs early/incrementally alongside the rest.
- **Refactor safety rests on ADR 0049.** Each child is a behavior-preserving
  refactor guarded by the dual-execution contract, golden snapshots, transform
  invariants, and the e2e pipeline test — extend those (e.g. add corpus journeys,
  route the pipeline test through the new path) as part of each child, not after.
- **Doc updates in the same planning change:** [architecture.md](../architecture.md)
  (source pipeline + entity resolution under Extensibility Boundaries; the durable
  queue under Source Refresh Scheduling; the domain-store split under Local
  Storage), [modularization-design.md](../modularization-design.md) (domain stores
  in the Rust ownership rules + large-file audit), and [roadmap.md](../roadmap.md)
  (the Architecture v2 epic in the Foundational section). Radicle/Radboard tracks
  the work; this ADR + the docs record the decisions.
- **Relationship to existing ADRs:** this *realizes* the source-adapter port and
  reaffirms the storage non-port stance of [ADR 0039](0039-ports-and-adapters-posture.md);
  extends the interpretation port of [ADR 0035](0035-two-layer-ai-and-local-interpretative-layer.md)
  (ANN swap); builds on the storage/pool/search boundaries of [ADR 0032](0032-search-and-backup-boundaries.md);
  and is the structural counterpart to the test-architecture ADRs 0048/0049.
- **Risk:** these are large, behavior-sensitive refactors. Mitigated by (a) the
  ADR 0049 harness as the safety net, (b) strangler/incremental migration for
  Decisions 1, 2, 5 (never a big-bang rewrite), and (c) keeping each child
  reviewable and tied to the feature milestone that justifies it. The facade
  split must not regress into a repository port (Decision 1); the durable queue
  must not pull a heavy dependency (Decision 5); the ANN swap must preserve the
  T4 contract (Decision 6).
