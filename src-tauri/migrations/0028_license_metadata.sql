CREATE TABLE license_metadata (
    id INTEGER PRIMARY KEY CHECK (id = 1),
    status TEXT NOT NULL,
    reason TEXT,
    license_id TEXT,
    holder TEXT,
    channel TEXT,
    edition TEXT,
    features_json TEXT NOT NULL DEFAULT '[]',
    issued_at TEXT,
    expires_at TEXT,
    app_version_range TEXT,
    key_id TEXT,
    checked_at TEXT NOT NULL DEFAULT (strftime('%Y-%m-%dT%H:%M:%fZ', 'now')),
    updated_at TEXT NOT NULL DEFAULT (strftime('%Y-%m-%dT%H:%M:%fZ', 'now'))
);

CREATE INDEX idx_license_metadata_status ON license_metadata(status);
