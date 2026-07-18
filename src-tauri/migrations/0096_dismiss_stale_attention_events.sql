-- Dismiss the pre-existing wall of stale, unseen attention events (v0.57.0 fix
-- wave 2, ADR 0068 amendment "historical ingest never impersonates the present").
--
-- Root cause repaired: a history backfill re-ingesting years of filings fired an
-- attention event per historical signal because (a) there was no freshness gate
-- and (b) the per-rule daily throttle keyed on the evidence's DOMAIN date, so
-- distinct old dates never coalesced. The owner saw ~19 unseen persistent toasts
-- covering the sidebar. The forward code fix adds a 14-day freshness gate and
-- moves `fired_at` to the wall-clock firing time so the throttle coalesces per
-- wall-clock day; this migration clears the backlog those old rules already wrote
-- so the toast wall disappears on update WITHOUT touching genuinely fresh events.
--
-- Forward-only, idempotent, self-healing (data-model migration rules): the
-- predicate only ever matches a still-unseen, still-undismissed event whose
-- `fired_at` day is more than 30 days before wall-clock now; re-running is a
-- no-op, and an event that is already seen, already dismissed, or fresh is never
-- altered.

UPDATE attention_events
SET dismissed = 1,
    seen = 1
WHERE dismissed = 0
  AND seen = 0
  AND substr(fired_at, 1, 10) < strftime('%Y-%m-%d', 'now', '-30 days');
