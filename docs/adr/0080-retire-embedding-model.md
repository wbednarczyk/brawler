# ADR 0080: Retire the Local Embedding Model and Dead Subsystems

Status: Accepted (2026-07-11, owner sign-off)

Exercises the reversibility [ADR 0035](0035-two-layer-ai-and-local-interpretative-layer.md) designed in ("the vector index is disposable so the model is reversible to static"). Amends [ADR 0050](0050-architecture-v2-domain-stores-source-pipeline-durable-jobs.md) (AV6 ANN activation and the story-key persist path retired), [ADR 0051](0051-story-clustering-across-sources.md) (the "harmless" `story_key` column is removed now that no consumer will arrive), and [ADR 0053](0053-dockview-layout-pilot.md)/[ADR 0054](0054-mode-based-thesis-centric-shell.md) (the unvalidated OS pop-out path is dropped; in-app floating stays). Execution detail: [docs/plans/v0.52-judgment-capture.md](../plans/v0.52-judgment-capture.md) stream R.

## Context

Owner decision (2026-07-11): remove the small local language model from the app. The evidence record supports it — the `e5-small` embedding model (`v0.45.0`, ADR 0035) was adopted per capability **only where a per-capability eval beats the static baseline, and none ever did**: story clustering was evaluated on real data and dropped (ADR 0051 — best local precision 0.79/recall 0.73), report-diff section alignment shipped on heading + positional matching without a similarity call (verified: `report_diff/` has no `SimilarityProvider` usage), and `SemanticSearch` stayed a deferred follow-on that never landed. A three-agent audit (2026-07-11) found the only reachable consumer of the whole similarity surface is the developer-gated Diagnostics "find similar" box; the vector scaffold (`instant-distance` ANN index, embeddings store, `content_embedding` job) ships in every build as a documented no-op under the default `static` strategy.

The same audit surfaced adjacent write-only/dead code: the `feed_items.story_key` column is computed and persisted on **every feed ingest** and never read outside tests (its consumer, clustering, was dropped); `rename_cockpit_layout` is a complete command/storage/api path with zero callers; the dockview OS-window pop-out path was never validated on native Windows and silently degrades. Separately, the audit found the **AI claim-extraction pipeline** (commands/job/tables) is built but UI-unwired — the owner decided it stays **parked, not removed** (a future UI candidate; the manual management-claims path is live and separate).

## Decision

1. **Remove the embedding model and its feature.** Cargo feature `embedding-model` and the optional `candle-core`/`candle-nn`/`candle-transformers`/`tokenizers` dependencies are deleted, together with the model-download/eval harness and its make targets.
2. **Remove the always-compiled vector scaffold.** `interpretation/{embedder, model, candle_embedder, vector, vector_index, embedding_similarity, eval}`, the `instant-distance` dependency, `storage/embeddings.rs`, the `content_embedding` job + queue lane, the embedding commands (`download_embedding_model`, `rebuild_embedding_index`, `find_similar_content`, `set_similarity_strategy`, `get_embedding_model_status`) with `api/interpretation.ts`, and the `EmbeddingSettings`/`EmbeddingModelSection` UI. **Kept:** `interpretation/rule_classifier.rs` (live consumer: `storage/signals.rs`); `lexical_similarity` is kept only if a live consumer remains after `find_similar_content` goes, else removed with the rest. `wiki/embedding-model.md` is deleted; the contracts.md "Interpretation — Embedding Model" section is removed in the same change (docs-drift enforces).
3. **Remove the write-only story-key path.** `entity_resolution.rs`, the `derive_story_key` ingest stage, and the `sources.rs` write sites go; a forward migration drops `feed_items.story_key` + its index. This closes ADR 0050's AV6/story-key forward carry and tightens ADR 0051's "harmless leftover" to "removed".
4. **Forward, self-healing migrations only** (data-model rules): one migration drops `content_embeddings` and purges queued `content_embedding` jobs; one drops the `story_key` column + index. `similarity_strategy` reads tolerate the legacy `embedding` value by mapping it to `static`. Both migrations are tested for idempotence/self-heal against a real pre-removal snapshot (embeddings + story_key data present).
5. **Small dead plumbing goes:** `rename_cockpit_layout` (command, storage fn, api wrapper, DTO) and the OS pop-out path in `DockLayout.tsx` (`popOutOrFloat`/`addPopoutGroup`, its button, `Alt+P`) together with the `core:webview:allow-create-webview-window` capability. In-app floating groups stay untouched.
6. **Claim-extraction pipeline: parked, untouched** (owner decision 2026-07-11). The AI report→claim-proposal pipeline (commands/job/storage/migration 0046) remains in the codebase awaiting a future UI slice; revisit at the NS1 conversational slice. Touching it in stream R is a tripwire.
7. **Guardrail harvest (ADR 0045):** the orphaned `api/claimExtraction.ts` wrapper slipped past `knip` because `knip.json` excludes `exports`/`types` — R1 adds a scoped unused-exports check for `src/api/**` (precise; must not flag legitimate code) so a UI-unwired command surface reddens the gate instead of hiding.

## Consequences

- Shipped binary loses the candle inference stack, tokenizers, and `instant-distance`; the Windows cross-build tree shrinks (verified via `make package-windows-from-linux` in M5, which runs after R1).
- Old databases keep working: destructive changes ship as forward, idempotent, snapshot-tested migrations; legacy `similarity_strategy='embedding'` reads as `static`.
- Re-introduction cost is honest: git history preserves the implementation; any future semantic-search/RAG capability re-enters through a fresh eval-gated ADR (the ADR 0035 bar stands) rather than by resurrecting this code.
- The `v0.48` SemanticSearch deferred follow-on and ADR 0050's AV6 (ANN activation) are formally retired; roadmap annotated.
- Drift recorded while auditing (informational, no action in this ADR): dockview is in practice the **default** cockpit shell, not the "opt-in advanced layout" ADR 0054 describes — annotated in ADR 0054.

## Open questions

- None. The only judgment call in R1 (keep vs drop `lexical_similarity`) resolves mechanically by whether a live consumer remains.
