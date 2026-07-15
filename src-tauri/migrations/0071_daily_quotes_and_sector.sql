-- v0.53 Market data foundation (ADR 0067 / ADR 0082).
-- Append-only, immutable once applied (data-model rules).

PRAGMA foreign_keys = ON;

-- EOD price series. Append-only; corrections upsert by (company_id, date),
-- never a wholesale history rewrite. Full available history is ingested from
-- day one (52-week stats, own-history percentiles, future backtests).
CREATE TABLE daily_quotes (
    company_id TEXT NOT NULL REFERENCES companies(id) ON DELETE CASCADE,
    date TEXT NOT NULL,                 -- session date, ISO YYYY-MM-DD
    open REAL NOT NULL,
    high REAL NOT NULL,
    low REAL NOT NULL,
    close REAL NOT NULL,
    volume INTEGER NOT NULL,
    source_adapter_id TEXT,             -- provenance: which provider supplied the bar
    fetched_at TEXT NOT NULL DEFAULT (strftime('%Y-%m-%dT%H:%M:%fZ', 'now')),
    PRIMARY KEY (company_id, date)      -- unique (company_id, date); serves the 52w/percentile range scans
);

-- Company sector classification (ADR 0067 decision 3). Populated from the
-- GPW/NewConnect directory (sector_source='registry') with a manual override
-- (sector_source='manual') that a registry refresh must never clobber. Reads
-- tolerate a missing value with a safe default (both columns nullable).
ALTER TABLE companies ADD COLUMN sector TEXT;
ALTER TABLE companies ADD COLUMN sector_source TEXT;  -- 'registry' | 'manual' | NULL

-- market_data source adapters (ADR 0082). Primary yahoo-eod (keyless);
-- twelvedata-eod is the official fallback, registered but not independently
-- scheduled (engaged programmatically by the primary pull on 429/999/unmapped).
INSERT INTO source_adapters (
    id, display_name, source_type, fetch_mode, enabled, default_poll_interval_seconds
) VALUES (
    'yahoo-eod', 'Yahoo Finance EOD Quotes', 'market_data', 'public_json', 1, 86400
)
ON CONFLICT(id) DO UPDATE SET
    display_name = excluded.display_name,
    source_type = excluded.source_type,
    fetch_mode = excluded.fetch_mode,
    default_poll_interval_seconds = excluded.default_poll_interval_seconds,
    updated_at = strftime('%Y-%m-%dT%H:%M:%fZ', 'now');

INSERT INTO source_adapters (
    id, display_name, source_type, fetch_mode, enabled, default_poll_interval_seconds
) VALUES (
    'twelvedata-eod', 'Twelve Data EOD Quotes', 'market_data', 'public_json', 0, 86400
)
ON CONFLICT(id) DO UPDATE SET
    display_name = excluded.display_name,
    source_type = excluded.source_type,
    fetch_mode = excluded.fetch_mode,
    default_poll_interval_seconds = excluded.default_poll_interval_seconds,
    updated_at = strftime('%Y-%m-%dT%H:%M:%fZ', 'now');

INSERT OR IGNORE INTO source_adapter_markets (source_adapter_id, market) VALUES
    ('yahoo-eod', 'GPW'),
    ('twelvedata-eod', 'GPW');
