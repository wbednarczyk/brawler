CREATE TABLE research_review_checkpoints (
    id TEXT PRIMARY KEY,
    scope_type TEXT NOT NULL CHECK(scope_type IN ('company', 'watchlist')),
    scope_id TEXT NOT NULL,
    reviewed_at TEXT NOT NULL,
    created_at TEXT NOT NULL DEFAULT (strftime('%Y-%m-%dT%H:%M:%fZ', 'now')),
    updated_at TEXT NOT NULL DEFAULT (strftime('%Y-%m-%dT%H:%M:%fZ', 'now')),
    UNIQUE(scope_type, scope_id)
);

CREATE INDEX idx_research_review_checkpoints_scope
    ON research_review_checkpoints(scope_type, scope_id);

CREATE TABLE evidence_links (
    id TEXT PRIMARY KEY,
    from_type TEXT NOT NULL,
    from_id TEXT NOT NULL,
    to_type TEXT NOT NULL,
    to_id TEXT NOT NULL,
    relation_type TEXT NOT NULL CHECK(relation_type IN (
        'originates_from',
        'cites',
        'supports',
        'contradicts',
        'updates',
        'follows_up',
        'answers',
        'related'
    )),
    created_at TEXT NOT NULL DEFAULT (strftime('%Y-%m-%dT%H:%M:%fZ', 'now')),
    UNIQUE(from_type, from_id, to_type, to_id, relation_type)
);

CREATE INDEX idx_evidence_links_from ON evidence_links(from_type, from_id);
CREATE INDEX idx_evidence_links_to ON evidence_links(to_type, to_id);
