CREATE TABLE feed_item_attachments (
    id TEXT PRIMARY KEY,
    feed_item_id TEXT NOT NULL REFERENCES feed_items(id) ON DELETE CASCADE,
    label TEXT NOT NULL,
    url TEXT NOT NULL,
    position INTEGER NOT NULL,
    created_at TEXT NOT NULL DEFAULT (strftime('%Y-%m-%dT%H:%M:%fZ', 'now')),
    UNIQUE(feed_item_id, url)
);

CREATE INDEX idx_feed_item_attachments_feed_item_id ON feed_item_attachments(feed_item_id);
