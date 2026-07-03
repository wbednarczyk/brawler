-- Structured-first fundamentals extraction (ADR 0061): per-fact provenance
-- (which tier produced it + its validation verdict + any drift diff) and the
-- per-company extraction profile the deterministic PDF parser learns.
--
-- Append-only, idempotent, self-healing: CREATE TABLE IF NOT EXISTS so it
-- converges on every database regardless of prior state. financial_facts is
-- NOT altered -- provenance is modeled as a separate concern in its own table,
-- joined by fact id, so the central facts contract and its mock-fidelity
-- corpus are untouched. Reads of a missing provenance row tolerate absence
-- (legacy facts predate the pipeline -> treated as unknown tier/unvalidated).

-- One row per financial fact the structured pipeline produced.
CREATE TABLE IF NOT EXISTS financial_fact_provenance (
    fact_id TEXT PRIMARY KEY,
    -- esef | structured_xhtml | pdf | html_aggregator | ai_text
    source_tier TEXT NOT NULL,
    -- passed | witness_confirmed | unreviewed | flagged | none
    validation_status TEXT NOT NULL DEFAULT 'none',
    -- Clean label diff (serialized DriftReport) when a drift accompanied the fact.
    drift_json TEXT,
    -- The source concept/label this value was read from (primary citation).
    citation TEXT,
    created_at TEXT NOT NULL DEFAULT (strftime('%Y-%m-%dT%H:%M:%fZ', 'now')),
    updated_at TEXT NOT NULL DEFAULT (strftime('%Y-%m-%dT%H:%M:%fZ', 'now'))
);

CREATE INDEX IF NOT EXISTS idx_fact_provenance_status
    ON financial_fact_provenance (validation_status);

-- The confirmed, versioned PDF extraction layout for one company (ADR 0061 S3).
-- A company with no row has never been bootstrapped; the parser falls back to
-- the default dictionary and cannot yet detect drift (safe default).
CREATE TABLE IF NOT EXISTS company_extraction_profile (
    company_id TEXT PRIMARY KEY,
    template_hash TEXT NOT NULL,
    -- Serialized UnitScale (Ones | Thousands | Millions).
    unit_scale TEXT NOT NULL DEFAULT 'Thousands',
    -- Serialized ExtractionProfile (label_map, version, ...).
    profile_json TEXT NOT NULL,
    version INTEGER NOT NULL DEFAULT 1,
    updated_at TEXT NOT NULL DEFAULT (strftime('%Y-%m-%dT%H:%M:%fZ', 'now'))
);
