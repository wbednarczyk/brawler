-- ADR 0082 amendment (2026-07-14): the Twelve Data fallback is removed — the
-- live smoke proved GPW time_series is paid-plan-only (free tier covers
-- metadata, not quotes), so a free key could never serve the fallback and the
-- adapter was dead weight (owner decision). Forward repair of the 0071 seed
-- (shipped migrations are immutable): drop the seeded adapter row and any of
-- its runtime state. Idempotent and self-healing (DELETEs of absent rows are
-- no-ops).

DELETE FROM source_adapter_state WHERE source_adapter_id = 'twelvedata-eod';
DELETE FROM source_adapters WHERE id = 'twelvedata-eod';
