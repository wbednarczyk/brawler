# ADR 0033: Feed Retention Policy (Design)

Status: Proposed

This ADR captures the **design** for feed retention (issue `c45fb1e`, milestone `v0.38.0`). Implementation is deferred to a future milestone; the default values below are proposed and require owner confirmation before implementation.

## Context

Feed items accumulate indefinitely as sources are polled. Today the only cleanup is `prune_old_feed_items` (`src/storage/feed.rs`), which deletes **unsaved** feed items older than a fixed 30-day window (`feedPruneRetentionDays`), scheduled daily from the frontend. This is a single global window with "saved" as the only protection, so high-volume media sources and low-volume official filings are treated identically, and items that are referenced elsewhere (evidence, notes, AI analysis) are protected only if they were also explicitly saved.

The local data store should not grow without bound, but no durable, decision-relevant content should be silently lost.

## Decision (proposed)

1. **Per-source-category retention.** Retention windows are set per source category rather than one global window, because value-density differs by category. Proposed defaults:
   - Official company filings (ESPI/EBI, registry, calendar): **retained indefinitely** (never auto-pruned).
   - Official secondary / company news: **180 days**.
   - General market media (RSS): **30 days**.
   - Research/aggregator media: **90 days**.

   Categories map to the existing source-adapter purpose grouping in the registry, so a new adapter inherits its category's window.

2. **Protected items are never pruned**, regardless of age or category. An item is protected when it is any of:
   - saved by the user;
   - linked as research evidence;
   - the origin of a notebook entry;
   - the subject of a stored AI analysis;
   - part of a clustered story whose representative is protected (future, once clustering exists in `v0.44.0`).

3. **User-adjustable settings.** A Settings → Data retention section exposes the per-category windows (from a bounded set of options, with an "indefinite" choice) and clamps/validates like other settings. Defaults apply when unset.

4. **Transparent cleanup.** Cleanup reports what it removed (count per category) through the existing prune result surface, and the schedule/last-run is visible. Cleanup never deletes protected items; it deletes only unprotected items past their category window.

5. **Database size / item-count inspection.** Surface stored feed-item counts (total and per category) and on-disk database size so the user can see growth and the effect of retention. The metrics layer already computes database byte size (`storage/metrics.rs`); this extends it with feed counts.

## Consequences

- Prevents unbounded growth while preserving official filings and any item that carries durable, cross-referenced value.
- Per-category windows and protection rules are additive over the current single-window prune; the existing 30-day media default is preserved as the general-media category default.
- Requires a retention-settings data model + commands, a category resolver, an extended prune query that joins protection sources (evidence, notebooks, AI analysis), and inspection read models — scoped to a future implementation milestone.
- Open decisions for owner confirmation: the exact default windows per category, whether "indefinite" is allowed for media categories, and whether retention runs on the same daily schedule as today.
- Related: scheduling ownership is being revisited (see [architecture.md](../architecture.md) "Source Refresh Scheduling"); retention cleanup should ride the same scheduler decision.
