-- Remove the write-only story-key path (ADR 0080 decision 3, closing ADR 0050's
-- AV6/story-key forward carry and tightening ADR 0051's "harmless leftover" to
-- "removed"). `feed_items.story_key` was computed and persisted on every ingest
-- and never read outside tests — its consumer, story clustering, was dropped
-- (ADR 0051). The column is nullable + derived, so dropping it loses zero
-- canonical data.
--
-- The index must go first: SQLite refuses DROP COLUMN while an index references
-- the column. ALTER TABLE ... DROP COLUMN needs SQLite >= 3.35; the bundled
-- rusqlite build ships 3.46. Idempotence is runner-guarded (versioned, applied
-- once); the column deterministically exists here because migration 0052 always
-- runs first on the same database.

DROP INDEX IF EXISTS idx_feed_items_story_key;

ALTER TABLE feed_items DROP COLUMN story_key;
