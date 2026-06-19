# ADR 0035: Two-Layer AI Architecture and the Local Interpretative Layer (Design)

Status: Accepted

This ADR captures the **design** for splitting Brawler's AI surface into two layers and introducing a local, on-device **interpretative layer** (semantic lookup) alongside the existing generative provider boundary. It records the abstraction constraints required for replaceability, upgradeability, and full reversibility. Sequencing is decided (static foundation `v0.39.0`, embedding model `v0.45.0`); the runtime/model/vector-store defaults were **confirmed at v0.45.0 planning** — see [Amendment (v0.45.0): confirmed runtime defaults](#amendment-v0450-confirmed-runtime-defaults) — and the per-capability eval validates the encoder choice. Extends [ADR 0028](0028-multi-provider-ai-boundary.md) (multi-provider generative boundary) and relates to [ADR 0032](0032-search-and-backup-boundaries.md) (FTS5 search) and [ADR 0034](0034-espi-event-classification.md) (first consumer).

## Context

Multiple current and upcoming milestones independently need *interpretative lookup* over local content — "what is this," "what is this similar to," "what does this match":

- ESPI classification (`v0.40.0`) — classify a filing into a typed category.
- Story clustering (`v0.46.0`) — group near-duplicate multi-source coverage.
- Management claims (`v0.42.0`) and the autonomous pipeline (`v0.49.0`) — match claims to facts, evidence to questions.
- Global search (`v0.38.0`, [ADR 0032](0032-search-and-backup-boundaries.md)) is keyword-only (FTS5) today and would benefit from semantic (hybrid) retrieval.

Today the only "AI" boundary is the generative, BYO-key, remote provider boundary ([ADR 0028](0028-multi-provider-ai-boundary.md)). Routing every interpretative lookup through a remote generative model is slow, costs per call, requires an API key, and sends content off-device — at odds with the local-first and privacy principles. A small **on-device encoder** (text → vector) can serve interpretative lookup locally: fast, free, offline, private.

Two owner requirements shape this design:

1. **Replaceable / upgradeable / maintainable.** It must be easy to swap one model for another, and to swap the engine behind it, without touching the features that use it.
2. **Reversible to "static."** If an embedded model does not prove good enough, it must be possible to remove the model — and the whole "AI inside" idea — and fall back to a non-ML/deterministic implementation, without ripping out consumers or losing data.

Both requirements are satisfied by the same move: **consumers bind to capabilities, not to models.**

## Decision (proposed)

### 1. Two AI layers

- **Interpretative layer** (new) — on-device semantic lookup: embeddings and light classifiers. Always-on, free, offline, private. Powers classification, similarity, matching, and semantic/hybrid search.
- **Generative / reasoning layer** (existing, [ADR 0028](0028-multi-provider-ai-boundary.md)) — summarize, extract, assess, with citations. Provider-neutral, BYO-key, remote by default; an optional local generative model may be added later behind the same boundary.

Heavy reasoning (Polish financial summarization, KPI extraction with citations) stays in the generative layer; a small local model is not relied on for it.

### 2. Capability boundary (consumers depend on tasks, not models)

Consumers depend on small task-level capability contracts, never on "embeddings" or "a model." Initial capabilities (added only as real consumers appear — see Scope):

- `Classifier` — `classify(text, taxonomy) -> { category, confidence }`
- `SimilarityProvider` — `most_similar(item, candidates, k)` / `score(a, b)`
- `Matcher` — `match(query, candidates) -> ranked`
- `SemanticSearch` — `search(query, scope) -> ranked`

### 3. Implementation strategies, with a static baseline that ships first

Each capability has interchangeable implementations. The **static/deterministic implementation is the shipped baseline**; the model-backed implementation is an *optional enhancement layered behind the same interface*:

| Capability | Static baseline | Model implementation |
| --- | --- | --- |
| `Classifier` | rules over source category/title | embedding nearest-prototype |
| `SimilarityProvider` | lexical (BM25 over existing FTS5, trigram/Jaccard) | cosine over embeddings |
| `Matcher` | keyword/fuzzy overlap | embedding similarity |
| `SemanticSearch` | FTS5 keyword (today) | hybrid keyword + vector |

Removing the model is therefore not surgery: it is switching a capability's active implementation back to the static one it was layered on. Consumers are unaffected because they only ever knew the capability.

### 4. Engine boundary and model versioning (below the model implementation)

Beneath the model-backed implementations sits a separate, lower-level boundary for the inference engine and model itself:

- `Embedder` — `embed(texts) -> vectors`, with `model_id` and `dim` metadata. Implementations: local (`candle` / `fastembed`-class, pure-Rust preferred) and optionally remote embedding APIs.
- Every stored vector records its `model_id` and `dim`. Changing the model re-runs the embed job and rebuilds the index; vectors from a different `model_id` are never mixed.

This is the second swap point: the capability boundary gives reversibility; the engine boundary gives model upgradeability.

### 5. The interpretative layer produces only disposable, derived artifacts

Hard rule: the interpretative layer may only ever produce **derived/cache** data, never a source of truth. Vectors are an index computed from canonical data (filings, notes, claims, facts), stored in a dedicated vector store (`sqlite-vec`-class, co-located with the existing SQLite database so it inherits the backup/WAL posture, [ADR 0032](0032-search-and-backup-boundaries.md)).

Consequences of the rule:
- Changing the model → rebuild the index; no data migration.
- Disabling/removing the model entirely → stop populating vectors, switch capabilities to static, drop the vector table. **Zero canonical data loss.**

### 6. Selection via registry + configuration

Active implementation per capability is chosen through a registry/factory + settings, the same pattern [ADR 0028](0028-multi-provider-ai-boundary.md) mandates for generative providers. Defaults are conservative (static); the model is opt-in. The interpretative layer reuses the credential/config/provider-selection boundary rather than inventing a parallel one.

### 7. Eval harness decides keep/drop, per capability

The model is kept only where it measurably beats the static baseline. Each capability ships with a small deterministic eval (sample-backed) so the model-vs-static decision is data-driven, not assumed. A capability where the model does not win stays static.

### 8. Runtime/model defaults (confirmed at v0.45.0 planning)

These were proposed for confirmation; the choices made at v0.45.0 planning are recorded in [Amendment (v0.45.0)](#amendment-v0450-confirmed-runtime-defaults). In brief:

- Inference engine: **`candle`** (pure-Rust) — chosen over a `fastembed`/`ort` (onnxruntime C++) path to keep the cross-build-from-Linux packaging path free of native dependencies.
- Encoder: **`intfloat/multilingual-e5-small`** (384-dim) as the default candidate, evaluated on Polish; the eval harness (§7) confirms the final encoder. Weights distributed as an **optional runtime download**, not bundled into the default installer.
- Vector store: a **pure-Rust brute-force cosine** scan over an f32 `BLOB` column — chosen over a `sqlite-vec`-class C extension because the single-user corpus is small (thousands of vectors) and brute-force is sub-millisecond there, keeping the layer pure-Rust and cross-buildable. `sqlite-vec` remains a future swap behind the same vector-store boundary if scale ever demands ANN.

## Scope boundary

- In scope (this design): the two-layer split, the capability and engine boundaries, the static baselines, the disposable derived-index rule, registry/config selection, and the eval policy. The capability surface grows only with real consumers: `Classifier` (`v0.40.0`), `SimilarityProvider` (`v0.46.0` clustering), `Matcher`/`SemanticSearch` (claims, hybrid search).
- Out of scope (deferred): a local **generative** LLM (remains a possible future adapter behind the [ADR 0028](0028-multi-provider-ai-boundary.md) boundary, not part of the interpretative layer); speculative capabilities with no near-term caller.

## Consequences

- The open desktop core becomes meaningfully "smart" with **no API key** (local-first dividend), while quality-critical reasoning stays in the BYO-key/managed generative layer — reinforcing the open-core split (the managed/autonomous frontier stays the paid tier).
- Reversibility is structural: because consumers bind to capabilities and the model produces only a disposable index, "remove the AI-inside idea" is a configuration switch plus dropping a derived table, with no consumer rewrite and no data loss.
- New surface to build: capability contracts + registry, the engine/`Embedder` boundary, the vector store + embed/re-embed job, static baselines, and per-capability evals. Net-new runtime dependencies (a Rust inference crate, a vector-store extension) — conservative-dependency review required at implementation.
- Near-term consumers that would otherwise each reinvent similarity/matching get a shared substrate; classification (`v0.40.0`) ships on the static `Classifier` (rules) regardless, so the layer does not block it.

## Sequencing (decided)

The interpretative layer is split into two milestones:

- `v0.39.0` — **static foundation**: capability contracts + registry/config selection + static baselines + eval harness. No embedding model or vector store (the static baselines need neither). Built before its consumers; epic `8e94b2f`. Its first consumer is ESPI classification (`v0.40.0`), which binds to the `Classifier` capability.
- `v0.45.0` — **embedding model**: the on-device `candle` embedder + pure-Rust brute-force vector store, enabling the model-backed implementation per capability only where the eval beats static. This milestone wires **`SimilarityProvider`** only (its first high-value consumer, story clustering, is the next milestone); model `Classifier`/`Matcher`/`SemanticSearch` are deferred to their consumers per the just-in-time rule above. Sequenced immediately before story clustering (`v0.46.0`); epic `64980da`. Confirmed runtime defaults below.

## Open decisions for owner confirmation

- **Runtime/model defaults** (§8) — **confirmed at v0.45.0 planning** (see amendment below); the encoder is further validated by the eval harness.
- **Hybrid search timing** — when semantic retrieval augments the existing FTS5 search ([ADR 0032](0032-search-and-backup-boundaries.md)) versus remaining keyword-only. Still open — deferred to the `SemanticSearch` consumer, not part of `v0.45.0`.

## Amendment (v0.45.0): confirmed runtime defaults

The §8 defaults left open for confirmation were decided at v0.45.0 planning. The guiding constraint was to keep the **entire** interpretative layer pure-Rust and free of native dependencies, so the Linux→Windows cross-build packaging path (`make package-windows-from-linux`) stays simple — the same reason §8 already preferred a pure-Rust inference path.

1. **Inference engine — `candle` (pure-Rust).** Chosen over `fastembed`-rs / `ort`, which wrap onnxruntime (C++) and would add a native dependency to the cross-build. `candle` runs BERT-family encoders on CPU with no C/C++ toolchain.
2. **Encoder — `intfloat/multilingual-e5-small`** (384-dim, ~118 MB) as the default candidate: multilingual with strong Polish coverage and desktop-friendly size. `bge-m3`-class models were rejected for v0.45.0 as too heavy (≫500M params, slow CPU embedding) for a local similarity index. The eval harness (§7) confirms or revises the final encoder; the `Embedder` boundary makes a later swap a config change, never a consumer rewrite.
3. **Weights — optional runtime download.** Not bundled in the installer. On first opt-in into the `embedding` strategy, weights (safetensors + `tokenizer.json`) download into the app data directory and are checksum-verified before activation; the capability falls back to the static (lexical) baseline until they are present. This keeps the installer lean and the model fully optional and reversible.
4. **Vector store — pure-Rust brute-force cosine** over an f32 `BLOB` column in a `content_embeddings` table co-located with the main SQLite database (inheriting WAL + backup posture, [ADR 0032](0032-search-and-backup-boundaries.md)). Chosen over the §8-proposed `sqlite-vec` C extension: the single-user, watchlist-scoped corpus is thousands of vectors, where a full cosine scan is sub-millisecond, so ANN indexing is unnecessary complexity and a native dependency we avoid. `sqlite-vec` remains a clean future swap behind the same vector-store boundary if a corpus ever outgrows brute-force. The table records `model_id` and `dim` per the §4 versioning rule and is a disposable derived index per §5 (drop = zero canonical data loss).
5. **Capability scope — `SimilarityProvider` only this milestone.** Per the just-in-time rule (§Scope boundary), the embedding model is wired behind `SimilarityProvider` (the substrate story clustering, `v0.46.0`, will consume) plus its model-vs-lexical eval. Model `Classifier`, `Matcher`, and hybrid `SemanticSearch` are deferred until their consumers arrive.
6. **Eval execution.** The similarity eval runs against the **real cached model** (downloaded once into the local/CI model cache), not committed precomputed vectors. To keep default `make check` offline and fast, the model-backed eval is an **opt-in/periodic** suite (the `make check-epic` tier, alongside `knip`/Playwright) and **skips gracefully** when the model cache is absent — it never triggers a download from the per-change gate. See [engineering-workflow.md](../engineering-workflow.md).

Dependency note (conservative-dependency review, per [AGENTS.md](../../AGENTS.md)): v0.45.0 adds `candle-core`, `candle-nn`, `candle-transformers`, `tokenizers`, and `hf-hub` (download). These are net-new and non-trivial in size; they are justified by the local-first dividend (on-device semantic similarity with no API key, no per-call cost, no data leaving the device) and are confined behind the `Embedder` boundary so they are removable with the model.

7. **The candle engine is gated behind an off-by-default `embedding-model` cargo feature.** These crates are heavy to compile, so making them unconditional would slow every `make check`/CI build for an opt-in capability. They are therefore `optional` dependencies enabled by the `embedding-model` feature: the default build compiles none of them (the lockfile still resolves them), keeping the per-change gate fast and offline; release/dev builds and the periodic model eval build with `--features embedding-model`. With the feature off, `weights_state` reports `unsupported`, the registry's embedding strategy is `Unavailable`, and the static lexical baseline serves similarity — so the capability degrades cleanly rather than failing. This realizes the §3 "removing the model is switching a strategy" reversibility at the build level too. The shipped desktop app enables the feature.
