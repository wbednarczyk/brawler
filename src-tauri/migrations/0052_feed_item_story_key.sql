-- Canonical cross-source story key on feed items (Architecture v2 / ADR 0050,
-- the v0.46 story-clustering enabler). The ingestion pipeline derives this key
-- from the matched companies + publication day + a slug of the normalized title
-- (see `entity_resolution::story_key`), so items reported by different sources
-- about the same event share a key and can be clustered.
--
-- Nullable + derived: a feed item with no matched company or too-short title has
-- no key, and the column can always be recomputed from canonical data. Reads must
-- tolerate NULL. Run once by the versioned migration runner.

ALTER TABLE feed_items ADD COLUMN story_key TEXT;

CREATE INDEX IF NOT EXISTS idx_feed_items_story_key ON feed_items (story_key);
