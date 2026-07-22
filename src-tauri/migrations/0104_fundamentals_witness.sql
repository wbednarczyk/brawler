-- BiznesRadar fundamentals WITNESS (v0.59.0, ADR 0085, realizing ADR 0061
-- decision 4). The aggregator page is the pipeline's LAST layer after ADR 0084
-- removed the AI residual: it corroborates a primary (PDF) parse, never sources
-- or overwrites a fact.
--
-- Two things live here:
--   1. `fundamentals_witness_pages` — the politeness CACHE. ADR 0085 decision 3
--      caps the witness at ONE page fetch per tracked company per day; without a
--      durable record of "we already asked today" a re-run (autopilot retry,
--      manual re-extract, backfill sweep) would refetch per report document.
--      One row per company, upserted in place — this is a cache, not history.
--      A FAILED/uncovered attempt is cached too, so a dead slug or a flaky host
--      is not hammered once per document either.
--   2. The `biznesradar-fundamenty` catalog row, so the Sources screen shows the
--      witness with real freshness (`last_success_at`, DoD §C).
--
-- Forward-only, idempotent and self-healing (data-model migration rules).

PRAGMA foreign_keys = ON;

CREATE TABLE IF NOT EXISTS fundamentals_witness_pages (
    company_id TEXT PRIMARY KEY REFERENCES companies(id) ON DELETE CASCADE,
    -- The canonical page URL the attempt used (ticker appended; BiznesRadar
    -- 301-redirects it to the canonical slug, e.g. CDPROJEKT -> CD-PROJEKT).
    page_url TEXT NOT NULL,
    -- The fetched page body. NULL for every non-`ok` status: a failed or
    -- uncovered attempt caches the DECISION, never a body we do not have.
    html TEXT,
    -- Why the cached row says what it says. `no_coverage` (the generic landing /
    -- unresolvable slug) and `fetch_failed` are BOTH normal degraded states —
    -- they mean "witness unavailable", never "witness agreed" (ADR 0085 dec. 5).
    status TEXT NOT NULL CHECK(status IN ('ok', 'no_coverage', 'fetch_failed')),
    fetched_at TEXT NOT NULL DEFAULT (strftime('%Y-%m-%dT%H:%M:%fZ', 'now'))
);

-- Cadence sweep read path: "which companies are outside the window?".
CREATE INDEX IF NOT EXISTS idx_fundamentals_witness_pages_fetched
    ON fundamentals_witness_pages(fetched_at);

-- Catalog row for the witness adapter. source_type / fetch_mode / markets must
-- match the registry descriptor (drift-guard `registry_matches_seeded_catalog`).
-- Role (`witness`) is code-side in the registry, never a DB column.
INSERT INTO source_adapters (
    id, display_name, source_type, fetch_mode, enabled, default_poll_interval_seconds
) VALUES (
    'biznesradar-fundamenty', 'BiznesRadar Wyniki Finansowe', 'fundamentals', 'public_page', 1, 86400
)
ON CONFLICT(id) DO UPDATE SET
    display_name = excluded.display_name,
    source_type = excluded.source_type,
    fetch_mode = excluded.fetch_mode,
    default_poll_interval_seconds = excluded.default_poll_interval_seconds,
    updated_at = strftime('%Y-%m-%dT%H:%M:%fZ', 'now');

INSERT OR IGNORE INTO source_adapter_markets (source_adapter_id, market)
VALUES ('biznesradar-fundamenty', 'GPW');
