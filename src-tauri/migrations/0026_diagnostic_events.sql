CREATE TABLE diagnostic_events (
    id TEXT PRIMARY KEY NOT NULL DEFAULT ('diagnostic_event_' || lower(hex(randomblob(16)))),
    occurred_at TEXT NOT NULL DEFAULT (strftime('%Y-%m-%dT%H:%M:%fZ', 'now')),
    module TEXT NOT NULL,
    scope_type TEXT,
    scope_id TEXT,
    stage TEXT NOT NULL,
    severity TEXT NOT NULL,
    message TEXT NOT NULL,
    metadata_json TEXT NOT NULL DEFAULT '{}',
    created_at TEXT NOT NULL DEFAULT (strftime('%Y-%m-%dT%H:%M:%fZ', 'now'))
);

CREATE INDEX diagnostic_events_occurred_at_idx
ON diagnostic_events(occurred_at);

CREATE INDEX diagnostic_events_module_occurred_at_idx
ON diagnostic_events(module, occurred_at);

CREATE INDEX diagnostic_events_severity_occurred_at_idx
ON diagnostic_events(severity, occurred_at);

CREATE INDEX diagnostic_events_scope_occurred_at_idx
ON diagnostic_events(scope_type, scope_id, occurred_at);
