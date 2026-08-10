-- Durable KPI ingest runs (ADR 0098 decisions 2, 6, 8; epic #352, card #358).
-- One row = one (report document, company, extraction profile): the external
-- agent's worklist entry (dec. 8, model A) and lease/heartbeat holder. The
-- closed 11-state lifecycle (dec. 6) is enforced here only by CHECK; the typed
-- transition table lands in #360. Staged observations + commit receipts
-- (dec. 3/5) are #359; `report_documents.published_at` is #354.

PRAGMA foreign_keys = ON;

CREATE TABLE IF NOT EXISTS kpi_ingest_runs (
    -- Opaque append-history handle: kpiing_{sha256(report_document_id, company_id,
    -- profile_version, now_nanos)[:16 bytes hex]}. NOT deterministic on the
    -- (document, company, profile) triple alone -- a document can legally get a
    -- new run after its content changes under the same URL (ADR 0098 dec. 2/3;
    -- content identity lives in source_content_hash below, not in this id).
    id TEXT PRIMARY KEY,
    -- Row-audit protection only (ON DELETE RESTRICT): a document referenced by
    -- any durable run, in any state, cannot be deleted. Byte-retention downgrade
    -- to metadata_only stays a separate contract (#359, data-model.md).
    report_document_id TEXT NOT NULL REFERENCES report_documents(id) ON DELETE RESTRICT,
    -- Deleting a company takes its runs with it, like every other company-owned row.
    company_id TEXT NOT NULL REFERENCES companies(id) ON DELETE CASCADE,
    -- May not exist yet at discovery time; a period delete never destroys audit.
    period_id TEXT REFERENCES financial_periods(id) ON DELETE SET NULL,
    -- Immutable snapshot SHA-256 of the content the run actually read, set once
    -- by record_source_capture. report_documents.content_hash is NOT a
    -- substitute -- it is overwritten on re-fetch (report_documents.rs).
    source_content_hash TEXT,
    scope TEXT CHECK (scope IS NULL OR scope IN ('standalone', 'consolidated')),
    data_quality TEXT CHECK (data_quality IS NULL OR data_quality IN ('final', 'preliminary', 'estimated')),
    -- Extraction-profile version; part of the run's identity triple (below) --
    -- required from creation, unlike instruction_version.
    profile_version TEXT NOT NULL,
    -- Agent-instruction version. Nullable in discovery; #360 invariant: must be
    -- non-NULL before the run may transition to 'extracting' (ADR 0098 dec. 2).
    instruction_version TEXT,
    -- Closed lifecycle, ADR 0098 dec. 6.
    status TEXT NOT NULL DEFAULT 'discovered' CHECK (status IN (
        'discovered', 'source_captured', 'extracting', 'staged', 'validation_failed',
        'ready_to_commit', 'committing', 'complete', 'partial', 'failed', 'cancelled'
    )),
    manifest_hash TEXT,
    manifest_revision INTEGER NOT NULL DEFAULT 0 CHECK (manifest_revision >= 0),
    -- Incremented once per successful claim/reclaim (never per heartbeat/release).
    attempt_count INTEGER NOT NULL DEFAULT 0 CHECK (attempt_count >= 0),
    -- Lease (model A, dec. 8): all three NULL (unleased) or all three set
    -- (leased) -- never a half-filled/orphaned heartbeat.
    lease_holder TEXT,
    lease_expires_at TEXT,
    last_heartbeat_at TEXT,
    -- Versioned completeness denominator (dec. 4); filled by #362, column exists now.
    expected_kpis_json TEXT,
    missing_reasons_json TEXT,
    progress_json TEXT,
    cost_json TEXT,          -- diagnostic (tokens/cost) -- never part of the trust verdict.
    last_error TEXT,
    created_at TEXT NOT NULL DEFAULT (strftime('%Y-%m-%dT%H:%M:%fZ', 'now')),
    updated_at TEXT NOT NULL DEFAULT (strftime('%Y-%m-%dT%H:%M:%fZ', 'now')),
    -- Table-level constraints must trail every column def (SQLite grammar).
    CHECK (
        (lease_holder IS NULL AND lease_expires_at IS NULL AND last_heartbeat_at IS NULL)
        OR (lease_holder IS NOT NULL AND lease_expires_at IS NOT NULL AND last_heartbeat_at IS NOT NULL)
    )
);

-- create_run_if_absent's idempotency gate: at most ONE non-terminal run per
-- (document, company, profile) triple. Terminal history is exempt, so a
-- content change under the same URL can start a fresh run for the same triple.
CREATE UNIQUE INDEX IF NOT EXISTS idx_kpi_ingest_runs_active
    ON kpi_ingest_runs (report_document_id, company_id, profile_version)
    WHERE status NOT IN ('complete', 'partial', 'failed', 'cancelled');

-- claim_next's candidate scan: claimable status + lease expiry.
CREATE INDEX IF NOT EXISTS idx_kpi_ingest_runs_claimable
    ON kpi_ingest_runs (status, lease_expires_at);

-- list_runs(companyId) listing; created_at is a stable tie-break only, never a
-- domain "newest" selector (data-model.md conventions).
CREATE INDEX IF NOT EXISTS idx_kpi_ingest_runs_company
    ON kpi_ingest_runs (company_id, created_at DESC);
