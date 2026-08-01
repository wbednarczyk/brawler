-- Issue #309 (found during the #307 classification review, 2026-08-01): three
-- more keys the WDF cover-note mapper emits were never seeded into the catalog,
-- so `record_structured_fact` dropped every matching row at its defensive
-- `NoDefinition` skip — the same silent class migrations 0111 and 0112 fixed for
-- the parent-attributable and the plain issuer rows.
--
-- Why they survived 0112's sweep: the `every_emittable_metric_key_has_a_canonical_definition`
-- source scan matched only the `Some("literal")` return shape and was blind to
-- `Some(if cond { "a" } else { "b" })`, which is exactly how `classify` returns
-- the rozwodniona/basic and the przed-odpisami/plain pairs. The gate now scans
-- the whole `classify` body (shape-independent), so the blind spot cannot
-- re-open. All three rows are verbatim WDF cover-note row identities evidenced
-- on the real Pekao PSr H1/2026 note (rows XXI, XXII), which is why the labels
-- mirror the issuer's own phrasing rather than an IFRS concept name — the
-- 0110/0111/0112 convention: INSERT OR IGNORE on stable bare canonical ids.
--
-- `origin`/`statement_group` (columns 0129/0130 added after 0112) are set
-- explicitly: this IS an app-owned seed, and both per-share rows are reported
-- per-share accounting lines while the EBITDA variant is an income-statement
-- line beside its already-seeded sibling `wdf_zysk_netto_przed_odpisami_aktualizujacymi_netto`.
-- Both per-share keys are also point-in-time stocks (equity per share at a date),
-- so they join `STOCK_METRIC_KEYS` (`fundamentals/metrics.rs`) — a `_ttm` window
-- over a balance-derived per-share figure is a category error, and until now the
-- question was moot because the key never resolved to a definition at all.

INSERT OR IGNORE INTO kpi_definitions
    (id, scope, metric_key, label, value_kind, unit, computation, formula, origin, statement_group) VALUES
('kpidef_wdf_book_value_per_share', 'canonical', 'wdf_book_value_per_share',
 'Wartość księgowa na jedną akcję', 'monetary', 'per_share', 'reported', NULL, 'seed', 'per_share'),
('kpidef_wdf_rozwodniona_wartosc_ksiegowa_na_jedna_akcje', 'canonical',
 'wdf_rozwodniona_wartosc_ksiegowa_na_jedna_akcje',
 'Rozwodniona wartość księgowa na jedną akcję', 'monetary', 'per_share', 'reported', NULL, 'seed', 'per_share'),
('kpidef_wdf_ebitda_przed_odpisami_aktualizujacymi_netto', 'canonical',
 'wdf_ebitda_przed_odpisami_aktualizujacymi_netto',
 'EBITDA przed odpisami aktualizującymi netto', 'monetary', NULL, 'reported', NULL, 'seed', 'income');
