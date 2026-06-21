# ADR 0051: Story clustering across sources — evaluated and not shipped

Status: **Rejected** (heuristic/local-embedding approach); cross-source story
clustering **deferred** to a future LLM-judge design. Recorded so the evaluation
and its real-data evidence are not re-litigated.

## Context

The `v0.46.0` milestone (epic `7cba98b`) proposed collapsing near-duplicate
cross-source coverage of the same event for the same company — an official
ESPI/EBI filing plus media articles about it — into one **story**, so the Inbox
reads as a briefing rather than a firehose. The plan (as approved) was
heuristic-first: block candidates by company + day, score by canonical-URL /
title similarity, persist first-class clusters, rank members by an explicit
source `authority`, and render one briefing row with the official source on top.
Embedding similarity (the `SimilarityProvider` capability shipped in `v0.45.0`,
[ADR 0035](0035-two-layer-ai-and-local-interpretative-layer.md)) was deferred
"until the heuristic proved insufficient."

The full feature was implemented and then **validated against a real production
database** (49 tracked companies, 401 feed items over ~2 weeks, the two live
Polish sources: `bankier-company-komunikaty` official + `bankier-market-rss`
media). That validation is why this ADR records a rejection rather than an
acceptance.

## Evidence (real data, hand-labeled ground truth)

From 17 same-company/same-day cross-source buckets, hand-labeling produced 15–16
genuinely same-event official×media pairs among 62 candidate pairs. Measured
precision/recall of every approach:

| Approach | Best operating point | Precision | Recall |
| --- | --- | --- | --- |
| Title Jaccard (the shipped heuristic) | t=0.30 | 1.00 | **0.20** |
| IDF-weighted token overlap | t=0.15 | **1.00** | 0.27 |
| Embedding cosine (e5-small) | t=0.86 | 0.52 | 0.87 |
| Embedding + lexical verify | t=0.86 | 0.86 | 0.40 |
| Reciprocal-best-match on embeddings | floor=0.86 | **0.79** | **0.73** |

Why each fails:

- **Lexical (Jaccard / IDF).** Official titles are templated ("*Dywidenda za 2025
  rok*"); media is editorial ("*Prawie dwa miliardy zysku, ale dywidenda
  skromniejsza*"). They share the event but almost no words — median cross-source
  title similarity **0.12**. IDF weighting roughly doubles recall (0.20→0.40) but
  cannot recover heavy paraphrase.
- **Embeddings (e5-small).** The cosine range is compressed on homogeneous Polish
  financial text: same-event pairs sit at ~0.90, but *unrelated different-company*
  items already sit at median **0.89**. No global threshold separates them. The
  embedder correctly applies the e5 `query:` prefix — this is the small model's
  inherent property, not a bug.
- **Best local result** is reciprocal-best-match on embeddings: **precision 0.79,
  recall 0.73** — i.e. ~1 in 5 merged stories would be wrong, and ~1 in 4 real
  duplicates missed. For a feature whose entire value is "trust this is one
  story," a visible 20% false-merge rate is not trustworthy. Even the human
  ground-truth labeling was fuzzy (an official "*uchwały ZWZ*" filing bundles many
  decisions while media covers one), confirming "same event" needs judgment, not a
  threshold.

There **is** one reliable-but-narrow local operating point — IDF-weighted overlap
at t≈0.15 gives precision 1.00 at recall ~0.27 (merges only obvious shared-entity
cases, zero false merges) — but it catches only ~1 in 4 duplicates and is not
worth a milestone on its own.

## Decision

1. **Do not ship automatic cross-source story clustering.** Neither the lexical
   heuristic nor a local-embedding threshold reaches a trustworthy precision at
   useful recall. Shipping semi-working clustering would mislead — a wrong merge
   (e.g. a dividend filing fused under one headline with an unrelated board
   appointment) is highly visible and damaging.
2. **Scope reassessment: cross-source dedup is a nice-to-have, not core.** At real
   scale the feed is ~0.6 items per company per day (not a firehose); genuine
   cross-source duplication is ~15 pairs in two weeks across 49 companies; exact /
   same-source dedup already exists (`dedupe_key`, `duplicate_signature`, the
   GPW↔Bankier title suppression); and existing source/type/unread filters tame
   noise. The app's value is downstream of the feed (fundamentals/KPI extraction,
   AI analysis, quality frameworks, valuation, claims, report-season, notebooks).
3. **The reliable path, if revisited, is an LLM judge** — a model reading both
   titles (ideally bodies) and answering "same event?" This is a semantic judgment
   task small embeddings/lexical methods cannot do. It belongs behind the existing
   provider-neutral AI boundary ([ADR 0028](0028-multi-provider-ai-boundary.md) /
   [ADR 0016](0016-provider-neutral-ai-analysis-framework.md)) as its own future
   milestone, and conflicts with the local-first/no-cost-by-default posture unless
   gated behind a user-enabled provider. The natural shape is **embeddings propose,
   the LLM (or the user) disposes** — including a soft "related coverage" affordance
   the user judges, rather than an automatic merge.
4. **No code, schema, or `authority` field is retained.** The implementation
   (clustering module, the `story_clusters`/`story_cluster_members` migration, the
   `cluster_block_key` column, the read-model annotations, the `SourceAuthority`
   descriptor field, and the Inbox story rendering) is reverted — it was speculative
   generality with no shipped consumer. Canonical-source ranking is reintroduced
   with whatever eventually consumes it. The reverted migration was never applied to
   any released database.
5. **Re-point the `v0.45.0` embedding model.** It was justified by being clustering's
   "first high-value consumer" ([ADR 0035](0035-two-layer-ai-and-local-interpretative-layer.md)
   sequencing). With clustering dropped its real payoff is **ranking/retrieval**,
   which is forgiving of the precision wall that killed clustering: `SemanticSearch`
   for the command palette (`v0.48.0`) and RAG evidence retrieval for the AI
   milestones (`v0.47.0` report diff, `v0.49.0` autonomous pipeline, `v0.50.0`
   qualitative quality frameworks). ADR 0035 made the model reversible-to-static for
   exactly this kind of reassessment; that option remains open if no consumer lands.

## Consequences

- `v0.46.0` as specified is not delivered. The roadmap re-points the embedding
  model's home to `v0.48.0` semantic search; the epic and its sub-issues are closed
  as "evaluated, not shipped" with this ADR as the rationale.
- `story_key` (shipped in `v0.45.1` Architecture v2) remains; it is harmless and was
  not the failure point. No migration is added or removed from released history.
- The throwaway validation harness lived only on the feature branch against a
  gitignored DB copy; nothing touched a real database.

## Alternatives considered

All four matching approaches above were implemented and measured against real
labeled data — that measurement *is* the alternatives analysis. An LLM-judge
approach was not built (out of scope for a local-first milestone) but is the
recorded forward path. Shipping the narrow zero-false-merge IDF variant was
considered and rejected as not worth a milestone for ~27% recall.
