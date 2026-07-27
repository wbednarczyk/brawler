-- v0.61 comparative-valuation L1 persistence (ADR 0089 decisions 4-5).
-- Append-only, immutable once applied (data-model rules). The DCF engine (v0.62,
-- ADR 0041) writes NEW `method` values into this same table -- it adds rows, not
-- columns -- so the schema is designed against that need now.

PRAGMA foreign_keys = ON;

-- One row = one persisted comparative-valuation run for one (company, method).
-- Appended ONLY when the input signature (the canonical `inputs_json`) differs
-- from that (company, method)'s latest stored run -- never a row per render.
-- `fair_low/base/high` are per-share, decimal-exact TEXT (never a float).
-- `data_as_of` is the DOMAIN as-of date of the inputs (the price/fundamentals
-- snapshot) and is the newest-run ordering key -- ordering never keys on
-- `created_at` (the guardrail: wall-clock insert order can diverge from the data
-- date, e.g. a late backfill).
CREATE TABLE IF NOT EXISTS valuation_runs (
    id TEXT PRIMARY KEY,                    -- collision-safe deterministic id
    company_id TEXT NOT NULL,
    method TEXT NOT NULL,                   -- pe_multiple | ev_ebitda_multiple | pbv_multiple (DCF adds more)
    inputs_json TEXT NOT NULL,              -- canonical serialized inputs = the signature
    fair_low TEXT,                          -- per-share fair value, decimal-exact TEXT (NULL on a typed absence)
    fair_base TEXT,
    fair_high TEXT,
    data_as_of TEXT NOT NULL,               -- domain as-of date, ISO YYYY-MM-DD (newest-run ordering key)
    confidence_grade TEXT NOT NULL,         -- A | B | C | D (the composite letter)
    created_at TEXT NOT NULL DEFAULT (strftime('%Y-%m-%dT%H:%M:%fZ', 'now'))
);

-- list_valuation_runs(companyId) newest-first: ordered by the DOMAIN date, then
-- created_at as a stable tie-break within the same as-of date.
CREATE INDEX IF NOT EXISTS idx_valuation_runs_company_as_of
    ON valuation_runs (company_id, data_as_of DESC, created_at DESC);

-- latest-run-per-method signature lookup (the append gate).
CREATE INDEX IF NOT EXISTS idx_valuation_runs_company_method_as_of
    ON valuation_runs (company_id, method, data_as_of DESC, created_at DESC);
