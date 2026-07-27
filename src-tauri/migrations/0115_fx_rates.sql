-- v0.61 FX substrate (ADR 0089 decision 2).
-- Append-only, immutable once applied (data-model rules).

PRAGMA foreign_keys = ON;

-- NBP Table-A mid rates. Append-only; corrections upsert by (currency, date),
-- never a wholesale history rewrite — the exact idiom `daily_quotes` uses.
-- Full available history is backfilled on first need (comparison PLN conversion:
-- flow KPIs at the period-average mid, stock KPIs at the last mid <= period end;
-- ratios/percentages never converted). `mid_rate` is decimal-exact TEXT (never a
-- float) so conversion stays exact. Reads of a missing (currency, date) yield a
-- typed per-cell flag at the read model, never a silent PLN guess.
CREATE TABLE IF NOT EXISTS fx_rates (
    currency TEXT NOT NULL,              -- ISO 4217 code (EUR, USD, GBP, CHF, ...)
    date TEXT NOT NULL,                  -- NBP effectiveDate, ISO YYYY-MM-DD
    mid_rate TEXT NOT NULL,              -- decimal-exact mid (PLN per 1 unit)
    source_adapter_id TEXT,              -- provenance: which adapter supplied it
    fetched_at TEXT NOT NULL DEFAULT (strftime('%Y-%m-%dT%H:%M:%fZ', 'now')),
    PRIMARY KEY (currency, date)         -- unique (currency, date); serves the range scans
);

-- NBP FX adapter (ADR 0089 dec. 2). An INTERNAL substrate source (visibility
-- 'developer'), engaged programmatically by the `fx_daily_pull` durable-queue
-- job — NOT part of the source-refresh sweep and never enqueued by the source
-- scheduler (a developer-visibility adapter is excluded from the non-developer
-- adapter list the scheduler iterates). Its row exists for provenance +
-- last_success_at health stamping (record_source_outcome). Seeded on the shared
-- market-data lane politeness posture; keyless, official public API.
INSERT INTO source_adapters (
    id, display_name, source_type, fetch_mode, enabled, default_poll_interval_seconds
) VALUES (
    'nbp-fx', 'NBP — kursy średnie (Tabela A)', 'fx_rates', 'public_json', 1, 86400
)
ON CONFLICT(id) DO UPDATE SET
    display_name = excluded.display_name,
    source_type = excluded.source_type,
    fetch_mode = excluded.fetch_mode,
    default_poll_interval_seconds = excluded.default_poll_interval_seconds,
    updated_at = strftime('%Y-%m-%dT%H:%M:%fZ', 'now');
