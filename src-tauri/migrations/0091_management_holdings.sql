-- v0.57 Company Health — management-holdings substrate (ADR 0083 Decision 6,
-- card 9730f5f; data-model § Company Health). Parsed from the mandatory
-- periodic-report section "Zestawienie stanu posiadania akcji … przez osoby
-- zarządzające i nadzorujące". Shape refined 2026-07-17 from the 15-document /
-- 67-person-row hand-labeled ground truth. Two concerns, both forward-only /
-- idempotent / self-healing (data-model migration rules):
--
--   1. `management_holdings` — the parsed by-person holdings substrate.
--      Deterministic id from (report_document_id, person_normalized): one row
--      per person per document, so a re-parse upserts in place and re-ingest
--      never duplicates. Nullable role/shares/indirect/prior — the section
--      variability (glyph-blanked digits, prose-zero aggregates, before/after
--      columns) is encoded honestly, never guessed. A `shares = '0'` row is a
--      REAL zero holding (zero skin-in-the-game is signal); NULL shares means the
--      figure was present but not deterministically recoverable (e.g.
--      glyph-mangled); '-'/'nd.' (person left the organ) produce NO row.
--
--   2. `management_holdings_residual` — the once-per-document parking marker
--      (mirrors `ownership_extraction_residual`) for a document whose holdings
--      section the DETERMINISTIC parser could not turn into person rows
--      (glyph-mangled text layer, image table, or a missing/unresolved section).
--      NO holdings row is written — never guess. Cleared by the extraction job
--      the moment a (later) parser version succeeds, so the two never coexist.

PRAGMA foreign_keys = ON;

-- 1. Parsed by-person management holdings (data-model § Company Health).
CREATE TABLE IF NOT EXISTS management_holdings (
    -- Deterministic: mgmthld_<slug(report_document_id)>_<slug(person_normalized)>.
    -- A document-level zero aggregate (no named persons) uses a reserved
    -- person_normalized sentinel so the aggregate picture is one stable row.
    id TEXT PRIMARY KEY,
    company_id TEXT NOT NULL REFERENCES companies(id) ON DELETE CASCADE,
    report_document_id TEXT NOT NULL REFERENCES report_documents(id) ON DELETE CASCADE,
    person_name_raw TEXT NOT NULL,
    -- Uppercase normalize_holder_name form — the founder-stamping join key.
    person_normalized TEXT NOT NULL,
    -- Nullable: from organ keywords / inline role cell / in-table organ subheader;
    -- NULL when the organ was not disambiguated (aggregate-only statements).
    role TEXT CHECK(role IN ('management', 'supervisory')),
    -- Decimal-exact TEXT, nullable. '0' is a real zero-holding row; NULL = stated
    -- but unreadable (glyph-blanked). Never fabricated.
    shares TEXT,
    -- The holding vehicle when the section states "pośrednio poprzez …" / a family
    -- foundation — the founder-badge join bridge (founders hold via vehicles, so a
    -- person-name-only join misses them). Normalized form is the stake-join key.
    indirect_via_raw TEXT,
    indirect_via_normalized TEXT,
    -- The earlier column of a before/after or Nabycie/Zbycie/zmiana table, if any.
    prior_shares TEXT,
    prior_as_of TEXT,
    -- Explicit "na dzień <date>" on the section/caption when present, else the
    -- report period_end_date (document-period resolution reused from ownership).
    as_of TEXT NOT NULL,
    -- Document-level aggregate zero statement ("osoby zarządzające i nadzorujące
    -- nie posiadają akcji") with no named person: one sentinel row, shares='0'.
    is_zero_aggregate INTEGER NOT NULL DEFAULT 0,
    created_at TEXT NOT NULL DEFAULT (strftime('%Y-%m-%dT%H:%M:%fZ', 'now')),
    updated_at TEXT NOT NULL DEFAULT (strftime('%Y-%m-%dT%H:%M:%fZ', 'now'))
);

CREATE INDEX IF NOT EXISTS idx_management_holdings_company
    ON management_holdings(company_id);
CREATE INDEX IF NOT EXISTS idx_management_holdings_person
    ON management_holdings(company_id, person_normalized);
CREATE INDEX IF NOT EXISTS idx_management_holdings_vehicle
    ON management_holdings(company_id, indirect_via_normalized);

-- 2. Once-per-document parking marker for holdings sections the deterministic
-- parser could not read (glyph-mangled / image table / missing section).
CREATE TABLE IF NOT EXISTS management_holdings_residual (
    -- One residual per stored document (idempotent upsert key).
    report_document_id TEXT PRIMARY KEY
        REFERENCES report_documents(id) ON DELETE CASCADE,
    company_id TEXT NOT NULL REFERENCES companies(id) ON DELETE CASCADE,
    -- The deterministic parser's terminal non-Parsed state.
    parse_state TEXT NOT NULL CHECK(parse_state IN (
        'section_missing', 'table_unparsable', 'glyph_encoded'
    )),
    -- The disclosure date resolved for the document, if any — carried so the
    -- AI/OCR write can reuse it.
    detected_as_of TEXT,
    -- The holdings heading line that anchored the (failed) parse, verbatim.
    matched_heading TEXT,
    created_at TEXT NOT NULL DEFAULT (strftime('%Y-%m-%dT%H:%M:%fZ', 'now')),
    updated_at TEXT NOT NULL DEFAULT (strftime('%Y-%m-%dT%H:%M:%fZ', 'now'))
);

CREATE INDEX IF NOT EXISTS idx_management_holdings_residual_company
    ON management_holdings_residual(company_id);
