-- Debt-collector (`Wierzytelności`) `statement_type` → `specialty_finance`
-- (ADR 0083 Decision 4 amendment, 2026-07-18 — owner decision).
--
-- Migration 0095 mapped the *unambiguous* financial-issuer sectors (banks,
-- insurers, exchanges & brokerage houses) off the `'industrial'` default so the
-- health scores' `NotApplicable` gate fires, but conservatively LEFT the
-- borderline debt collectors ('Wierzytelności' — KRU/KRUK) untouched pending an
-- owner call. The owner has now decided: a debt collector's balance sheet is a
-- financial-institution balance sheet (receivables portfolio, not working
-- capital), so Altman Z″ / Piotroski F do not apply — map it to
-- `specialty_finance` (the same sector KPI pack as brokers, migration 0034).
-- Investment holdings ('Działalność Inwestycyjna' — GKI) STAY unmapped: still an
-- open owner decision, deliberately not swept here.
--
-- Append-only forward migration (0095 is already applied on the live DB; the
-- data-model rule forbids editing a shipped migration). Same idempotent,
-- self-healing semantics as 0095:
--   * Only rows still holding the `'industrial'` DEFAULT are rewritten — a
--     manually-set `statement_type` is authoritative and never overwritten.
--   * Re-running is a no-op: a mapped row is no longer `'industrial'`.
--   * `sector` is matched exactly as stored in the registry ('Wierzytelności').

UPDATE companies SET statement_type = 'specialty_finance'
 WHERE statement_type = 'industrial' AND sector = 'Wierzytelności';
