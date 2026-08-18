-- Layer 1 raw tagged-fact capture (ADR 0100 decisions 1, 8, 9; epic #398).
-- Every tagged fact in an ESEF/iXBRL instance is written 1:1, unconditionally
-- -- no naming decision, no crosswalk lookup, no interpretation. A concept
-- with no crosswalk entry is a ROW, not a diagnostic that evaporates
-- (decision 1). This is storage only: nothing populates these tables yet --
-- the parser/pipeline wiring is a later slice.
--
-- Disposable derived index, same doctrine as `report_document_sections`
-- (migration 0053): every row is computed from a report document's stored
-- bytes, never a source of truth. Dropping these tables loses zero canonical
-- data, it only forces re-extraction -- which is exactly why the extraction
-- record joins the report-bytes retention protection (see
-- `report_documents.rs` `PROTECTION_EXISTS_CLAUSES`): a raw-only package
-- whose bytes were pruned could never be rebuilt.
--
-- Append-only, idempotent, self-healing (CREATE TABLE IF NOT EXISTS) so it
-- converges on every database regardless of prior state.

-- One row per extracted report document: the outcome of running the (future)
-- tagged-fact extractor over its stored bytes. Freshness is
-- `(source_content_hash, extractor_version)`, NOT `extractor_version` alone
-- (decision 8) -- recapture replaces a document's bytes without changing its
-- id, the same reason migration 0140 added `content_hash` to the
-- derived-period cache.
CREATE TABLE IF NOT EXISTS report_tagged_fact_extractions (
    report_document_id TEXT PRIMARY KEY
        REFERENCES report_documents(id) ON DELETE CASCADE,
    -- sha256 of the source document bytes this extraction ran over. Re-run
    -- skips a document whose hash + extractor_version are unchanged.
    source_content_hash TEXT,
    -- Extraction-heuristic version; a bump invalidates and rebuilds affected rows.
    extractor_version INTEGER NOT NULL,
    -- Free-text outcome token (the extractor slice owns the vocabulary --
    -- e.g. 'extracted' | 'no_instance' | 'extraction_failed' -- mirroring
    -- report_document_extractions.extraction_state, which is likewise
    -- undocumented at the DB level: the CHECK belongs to the writer that
    -- knows the real state space, not to storage that has none yet).
    state TEXT NOT NULL,
    -- Ship-gate counters (ADR 0100 decision 9: "encountered = stored", not
    -- "successfully normalized"). encountered_count is every occurrence the
    -- extractor saw; stored_count is every row actually written (includes
    -- parse_status != 'ok' rows -- decision 9 forbids a silent drop);
    -- dimensional_count is the is_dimensional=1 subset (not yet projectable,
    -- ADR 0100 consequences).
    encountered_count INTEGER NOT NULL DEFAULT 0,
    stored_count INTEGER NOT NULL DEFAULT 0,
    dimensional_count INTEGER NOT NULL DEFAULT 0,
    extracted_at TEXT NOT NULL DEFAULT (strftime('%Y-%m-%dT%H:%M:%fZ', 'now'))
);

-- One row per tagged fact OCCURRENCE, written 1:1 from a filing (decision 1).
-- A package legitimately contains several instances (`package_entry_path`
-- distinguishes them) and a concept is legitimately tagged 2-3x at an
-- identical (concept, context) within one instance -- `fact_identity`
-- distinguishes THOSE. Every supported occurrence gets a row even when
-- normalization fails (decision 9): `value_numeric` is nullable, `parse_status`
-- is typed, never a silent drop.
CREATE TABLE IF NOT EXISTS report_tagged_facts (
    -- rtf_{32 hex} -- the `generate_observation_id` id policy
    -- (storage/kpi_ingest_staging.rs): collision-safe, non-deterministic.
    id TEXT PRIMARY KEY,
    report_document_id TEXT NOT NULL
        REFERENCES report_documents(id) ON DELETE CASCADE,
    company_id TEXT NOT NULL
        REFERENCES companies(id) ON DELETE CASCADE,
    -- Path of the instance inside the package; '' for a non-package document.
    package_entry_path TEXT NOT NULL,
    -- 'xml_id:<the ix fact's id attribute>' where present (measured: 3439/3439
    -- sampled facts carry a unique id), else a deterministic source locator.
    -- NOT NULL on purpose (decision spec): a nullable column would let SQLite
    -- admit many NULLs under the unique index below.
    fact_identity TEXT NOT NULL,
    identity_kind TEXT NOT NULL CHECK (identity_kind IN ('xml_id', 'occurrence')),
    -- The EXPANDED namespace URI, never the QName prefix -- a prefix is a
    -- document-local alias (decision 1).
    concept_namespace_uri TEXT NOT NULL,
    concept_local_name TEXT NOT NULL,
    context_ref TEXT NOT NULL,
    period_type TEXT NOT NULL CHECK (period_type IN ('instant', 'duration')),
    period_start TEXT,
    period_end TEXT NOT NULL,
    unit_ref TEXT,
    unit_measure TEXT,
    -- Exactly as printed.
    value_raw TEXT NOT NULL,
    -- Decimal-exact TEXT (the valuation_runs/kpi_staged_observations convention).
    value_numeric TEXT,
    scale INTEGER,
    sign TEXT,
    decimals TEXT,
    is_dimensional INTEGER NOT NULL DEFAULT 0 CHECK (is_dimensional IN (0, 1)),
    dimensions_json TEXT,
    -- decision 9: every supported occurrence gets a row even when
    -- normalization fails.
    parse_status TEXT NOT NULL
        CHECK (parse_status IN ('ok', 'unparsed_value', 'unresolved_context', 'unsupported_unit')),
    parse_error TEXT,
    created_at TEXT NOT NULL DEFAULT (strftime('%Y-%m-%dT%H:%M:%fZ', 'now')),
    updated_at TEXT NOT NULL DEFAULT (strftime('%Y-%m-%dT%H:%M:%fZ', 'now')),
    -- A concept is legitimately tagged 2-3x at an identical (concept, context)
    -- -- distinguished by fact_identity, not collapsed here (decision 1/4).
    -- Two instances in one package (differing package_entry_path) never collide.
    UNIQUE (report_document_id, package_entry_path, fact_identity)
);

CREATE INDEX IF NOT EXISTS idx_report_tagged_facts_document
    ON report_tagged_facts (report_document_id);
CREATE INDEX IF NOT EXISTS idx_report_tagged_facts_concept
    ON report_tagged_facts (concept_namespace_uri, concept_local_name);

-- Fact <-> presentation-role relation (decision 3): a concept participates in
-- several roles (income statement, comprehensive income, cash-flow
-- reconciliation, ...), so a scalar column on report_tagged_facts could not
-- tell those occurrences apart. One row per (fact, role); the pair is the
-- natural primary key, no surrogate id needed for a pure junction table (the
-- `report_document_sections` composite-PK idiom, migration 0053).
CREATE TABLE IF NOT EXISTS report_tagged_fact_roles (
    fact_id TEXT NOT NULL
        REFERENCES report_tagged_facts(id) ON DELETE CASCADE,
    role_uri TEXT NOT NULL,
    -- The classified family (decision 3): an unrecognised role classifies as
    -- 'other' explicitly, never by guess.
    role_kind TEXT NOT NULL
        CHECK (role_kind IN ('balance', 'income', 'comprehensive_income', 'cash_flow', 'equity_changes', 'other')),
    PRIMARY KEY (fact_id, role_uri)
);
