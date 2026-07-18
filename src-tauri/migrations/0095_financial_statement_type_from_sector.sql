-- Financial `statement_type` from registry sector (ADR 0083 Decision 4
-- amendment, 2026-07-17 — T3 gate finding).
--
-- The health scores (Piotroski F / Altman Z″) exclude financial-statement
-- companies via `companies.statement_type` (Decision 4). The T3 real-data gate
-- found EVERY tracked company still at the `'industrial'` column DEFAULT —
-- banks, insurers and brokers included — so the `NotApplicable` gate never
-- fired and a backfilled bank would emit a misleading Z″ headline. This maps the
-- registry sector strings of *unambiguous* financial issuers to their sector
-- `statement_type` (the banking / insurance / specialty_finance sector KPI
-- packs, migration 0034).
--
-- Forward-only, idempotent, self-healing (data-model migration rules):
--   * Only rows still holding the `'industrial'` DEFAULT are rewritten — a
--     manually-set `statement_type` is authoritative and never overwritten.
--   * Conservative allow-list: only sectors that unambiguously mean a financial
--     issuer (commercial banks, insurers, exchanges & brokerage houses). Any
--     other sector — including the borderline debt collectors ('Wierzytelności')
--     that BiznesRadar still scores, and investment holdings
--     ('Działalność Inwestycyjna') — stays untouched.
--     FOLLOW-UP (2026-07-18): the debt-collector question was resolved by owner
--     decision — migration 0098 maps 'Wierzytelności' → specialty_finance.
--     Investment holdings ('Działalność Inwestycyjna') remain unmapped.
--   * Re-running is a no-op: a mapped row is no longer `'industrial'`.

UPDATE companies SET statement_type = 'banking'
 WHERE statement_type = 'industrial' AND sector = 'banki komercyjne';

UPDATE companies SET statement_type = 'insurance'
 WHERE statement_type = 'industrial' AND sector = 'firmy ubezpieczeniowe';

UPDATE companies SET statement_type = 'specialty_finance'
 WHERE statement_type = 'industrial' AND sector = 'giełdy i biura maklerskie';
