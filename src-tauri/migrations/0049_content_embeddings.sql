-- Content embeddings: the interpretative AI layer's disposable vector index
-- (ADR 0035, v0.45.0). Each row is a derived embedding of a canonical content
-- row (a feed item, etc.), NEVER a source of truth. Dropping this table loses
-- zero canonical data; it only forces a re-embed.
--
-- Append-only, idempotent, self-healing: CREATE TABLE IF NOT EXISTS +
-- INSERT OR IGNORE so it converges on every database regardless of prior state.

CREATE TABLE IF NOT EXISTS content_embeddings (
    -- Canonical content kind, aligned with the unified search content types
    -- (ADR 0032): feed_item, company, notebook_entry, ...
    content_type TEXT NOT NULL,
    -- Opaque id of the canonical row.
    content_id TEXT NOT NULL,
    -- The embedding model that produced this vector (e.g.
    -- intfloat/multilingual-e5-small). Vectors from different model_ids are
    -- never mixed in a similarity computation.
    model_id TEXT NOT NULL,
    -- Vector dimensionality (e.g. 384).
    dim INTEGER NOT NULL,
    -- The embedding as a little-endian f32 BLOB (dim * 4 bytes).
    vector BLOB NOT NULL,
    -- Hash of the embedded text, so re-embedding can skip unchanged content.
    content_hash TEXT NOT NULL,
    created_at TEXT NOT NULL DEFAULT (strftime('%Y-%m-%dT%H:%M:%fZ', 'now')),
    PRIMARY KEY (content_type, content_id, model_id)
);

-- Candidate scans are scoped by (model_id, content_type); index that access path.
CREATE INDEX IF NOT EXISTS idx_content_embeddings_model_type
    ON content_embeddings (model_id, content_type);

-- The active SimilarityProvider strategy. Defaults to the deterministic static
-- (lexical) baseline; switched to 'embedding' only once the model is ready
-- (ADR 0035 section 6). Read with a safe default so a missing row never crashes
-- startup.
INSERT OR IGNORE INTO settings (key, value, value_type) VALUES
    ('similarity_strategy', 'static', 'string');
