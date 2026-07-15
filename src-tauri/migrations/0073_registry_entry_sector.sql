-- v0.53 sector persistence (ADR 0067 Decision 3, T3).
-- The registry refresh caches the directory-sourced sector here; the tracked
-- `companies.sector` (added in 0071) is propagated from this cache on refresh
-- and on company creation, but a `sector_source='manual'` value always wins.
-- Append-only, immutable once applied (data-model rules).

PRAGMA foreign_keys = ON;

ALTER TABLE company_registry_entries ADD COLUMN sector TEXT;
