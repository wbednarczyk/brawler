-- BiznesRadar-PRIMARY fundamentals: per-(company, page_kind) page cache
-- (v0.59.0, ADR 0086 decision 2, plan TOR C slice C2).
--
-- ADR 0085's witness fetched ONE page per company (the income statement,
-- `raporty-finansowe-rachunek-zyskow-i-strat`), cached one row per company in
-- `fundamentals_witness_pages` (migration 0104, PK = company_id). ADR 0086
-- promotes the aggregator to PRIMARY and fetches THREE robots-allowed pages per
-- tracked company per day — income, balance, cash flow — so the cache key
-- becomes (company_id, page_kind).
--
-- Append-only, per the data-model migration rules: the old
-- `fundamentals_witness_pages` table is KEPT (never dropped — a migration never
-- deletes user data), its existing rows are COPIED here as the `income` page
-- kind, and the storage layer's reads/writes move to this table (tolerant of a
-- missing row: a cache miss simply means "fetch is due"). Re-running is a no-op
-- (CREATE IF NOT EXISTS + INSERT OR IGNORE), self-healing.
--
-- Also seeds the `inventories` canonical KPI definition so the aggregator's
-- balance-sheet `Zapasy` row (dictionary entry added with this slice) resolves to
-- a catalog metric and its facts actually persist — the same convention the other
-- balance-sheet inputs follow (migration 0089, INSERT OR IGNORE on a stable id).

PRAGMA foreign_keys = ON;

CREATE TABLE IF NOT EXISTS fundamentals_aggregator_pages (
    company_id TEXT NOT NULL REFERENCES companies(id) ON DELETE CASCADE,
    -- Which of the three robots-allowed report pages this row caches. The three
    -- kinds map to the BiznesRadar paths `raporty-finansowe-rachunek-zyskow-i-strat`
    -- (income), `-bilans` (balance), `-przeplywy-pieniezne` (cashflow).
    page_kind TEXT NOT NULL CHECK(page_kind IN ('income', 'balance', 'cashflow')),
    -- The canonical page URL the attempt used (ticker appended; BiznesRadar
    -- 301-redirects it to the canonical slug, e.g. CDPROJEKT -> CD-PROJEKT).
    page_url TEXT NOT NULL,
    -- The fetched page body. NULL for every non-`ok` status: a failed or uncovered
    -- attempt caches the DECISION, never a body we do not have.
    html TEXT,
    -- Why the cached row says what it says. `no_coverage` (the generic landing /
    -- unresolvable slug) and `fetch_failed` are BOTH normal degraded states.
    status TEXT NOT NULL CHECK(status IN ('ok', 'no_coverage', 'fetch_failed')),
    fetched_at TEXT NOT NULL DEFAULT (strftime('%Y-%m-%dT%H:%M:%fZ', 'now')),
    -- One row per (company, page kind), upserted in place — a cache, not history.
    PRIMARY KEY (company_id, page_kind)
);

-- Cadence sweep read path: "which (company, page) pairs are outside the window?".
CREATE INDEX IF NOT EXISTS idx_fundamentals_aggregator_pages_fetched
    ON fundamentals_aggregator_pages(fetched_at);

-- Carry the existing single-page witness cache forward as the `income` page kind.
-- INSERT OR IGNORE so a re-run (or a live DB where the copy already happened)
-- never duplicates or overwrites a freshly-fetched income row.
INSERT OR IGNORE INTO fundamentals_aggregator_pages
    (company_id, page_kind, page_url, html, status, fetched_at)
SELECT company_id, 'income', page_url, html, status, fetched_at
FROM fundamentals_witness_pages;

-- Inventory (`Zapasy`) as a canonical reported balance-sheet metric, so the
-- aggregator-sourced facts for it resolve to a definition and persist.
INSERT OR IGNORE INTO kpi_definitions
    (id, scope, metric_key, label, value_kind, unit, computation, formula) VALUES
('kpidef_inventories', 'canonical', 'inventories',
 'Inventories', 'monetary', NULL, 'reported', NULL);
