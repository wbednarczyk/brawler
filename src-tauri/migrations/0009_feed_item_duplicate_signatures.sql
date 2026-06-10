PRAGMA foreign_keys = ON;

ALTER TABLE feed_items
ADD COLUMN duplicate_signature TEXT;

CREATE INDEX idx_feed_items_duplicate_signature
ON feed_items(duplicate_signature)
WHERE duplicate_signature IS NOT NULL;
