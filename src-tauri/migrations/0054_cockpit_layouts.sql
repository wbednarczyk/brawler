-- Research cockpit layouts (ADR 0053, Research cockpit epic): named, user-saved
-- panel arrangements for the dockview docking shell. `panels_json` is the
-- app-owned source of truth (which panels are open + the linked selection);
-- `layout_json` is the opaque dockview geometry (api.toJSON(), may be NULL);
-- `dockview_version` lets a future geometry-format change be migrated or safely
-- discarded on restore. Append-only, idempotent, self-healing.

CREATE TABLE IF NOT EXISTS cockpit_layouts (
    id TEXT PRIMARY KEY,
    name TEXT NOT NULL UNIQUE,
    ordinal INTEGER NOT NULL DEFAULT 0,
    panels_json TEXT NOT NULL,
    layout_json TEXT,
    dockview_version TEXT,
    created_at TEXT NOT NULL DEFAULT (strftime('%Y-%m-%dT%H:%M:%fZ', 'now')),
    updated_at TEXT NOT NULL DEFAULT (strftime('%Y-%m-%dT%H:%M:%fZ', 'now'))
);

CREATE UNIQUE INDEX IF NOT EXISTS idx_cockpit_layouts_name
    ON cockpit_layouts (name);

CREATE INDEX IF NOT EXISTS idx_cockpit_layouts_ordinal
    ON cockpit_layouts (ordinal);
