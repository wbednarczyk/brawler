-- Guardrail G1 catch (review follow-through, 2026-07-22): the WDF cover-note
-- mapper emits 16 issuer-row metric keys that were NEVER seeded into the
-- catalog, so every matching cover-note row silently dropped at the defensive
-- `NoDefinition` skip since the tier shipped. Same class migration 0111 fixed
-- for the two parent-attributable keys; this seeds the REST of the mapper's
-- static vocabulary. The new `every_emittable_metric_key_has_a_canonical_definition`
-- source-scan test reddens if a future mapper key lands without its seed.
--
-- Labels mirror the issuer's own WDF row phrasing (these keys ARE verbatim
-- cover-note row identities, not IFRS concepts); INSERT OR IGNORE on stable ids,
-- canonical scope, monetary/reported — the 0110/0111 convention.

INSERT OR IGNORE INTO kpi_definitions
    (id, scope, metric_key, label, value_kind, unit, computation, formula) VALUES
('kpidef_wdf_calkowite_dochody_netto', 'canonical', 'wdf_calkowite_dochody_netto',
 'Całkowite dochody netto', 'monetary', NULL, 'reported', NULL),
('kpidef_wdf_calkowite_dochody_netto_parent', 'canonical',
 'wdf_calkowite_dochody_netto_przypadajace_na_akcjonariuszy_jednostki_dominujacej',
 'Całkowite dochody netto przypadające na akcjonariuszy jednostki dominującej', 'monetary', NULL, 'reported', NULL),
('kpidef_wdf_calkowity_dochod', 'canonical', 'wdf_calkowity_dochod',
 'Całkowity dochód', 'monetary', NULL, 'reported', NULL),
('kpidef_wdf_calkowity_dochod_netto', 'canonical', 'wdf_calkowity_dochod_netto',
 'Całkowity dochód netto', 'monetary', NULL, 'reported', NULL),
('kpidef_wdf_calkowity_dochod_parent', 'canonical',
 'wdf_calkowity_dochod_przypadajacy_akcjonariuszom_jednostki_dominujacej',
 'Całkowity dochód przypadający akcjonariuszom jednostki dominującej', 'monetary', NULL, 'reported', NULL),
('kpidef_wdf_calkowity_dochod_strata_netto', 'canonical', 'wdf_calkowity_dochod_strata_netto',
 'Całkowity dochód (strata) netto', 'monetary', NULL, 'reported', NULL),
('kpidef_wdf_calkowity_dochod_strata_netto_parent', 'canonical',
 'wdf_calkowity_dochod_strata_netto_akcjonariuszy_jednostki_dominujacej',
 'Całkowity dochód (strata) netto akcjonariuszy jednostki dominującej', 'monetary', NULL, 'reported', NULL),
('kpidef_wdf_laczne_calkowite_dochody', 'canonical', 'wdf_laczne_calkowite_dochody',
 'Łączne całkowite dochody', 'monetary', NULL, 'reported', NULL),
('kpidef_wdf_net_cash_change', 'canonical', 'wdf_net_cash_change',
 'Zmiana netto stanu środków pieniężnych', 'monetary', NULL, 'reported', NULL),
('kpidef_wdf_noncurrent_assets', 'canonical', 'wdf_noncurrent_assets',
 'Aktywa trwałe', 'monetary', NULL, 'reported', NULL),
('kpidef_wdf_noncurrent_liabilities', 'canonical', 'wdf_noncurrent_liabilities',
 'Zobowiązania długoterminowe', 'monetary', NULL, 'reported', NULL),
('kpidef_wdf_pretax_profit', 'canonical', 'wdf_pretax_profit',
 'Zysk (strata) przed opodatkowaniem', 'monetary', NULL, 'reported', NULL),
('kpidef_wdf_share_capital', 'canonical', 'wdf_share_capital',
 'Kapitał zakładowy', 'monetary', NULL, 'reported', NULL),
('kpidef_wdf_zysk_netto_przed_odpisami', 'canonical',
 'wdf_zysk_netto_przed_odpisami_aktualizujacymi_netto',
 'Zysk netto przed odpisami aktualizującymi netto', 'monetary', NULL, 'reported', NULL),
('kpidef_wdf_zysk_strata_brutto_ze_sprzedazy', 'canonical', 'wdf_zysk_strata_brutto_ze_sprzedazy',
 'Zysk (strata) brutto ze sprzedaży', 'monetary', NULL, 'reported', NULL),
('kpidef_wdf_net_profit_nci', 'canonical',
 'wdf_zysk_strata_netto_przypadajacy_na_udzialy_niekontrolujace',
 'Zysk (strata) netto przypadający na udziały niekontrolujące', 'monetary', NULL, 'reported', NULL),
('kpidef_net_profit_discontinued', 'canonical', 'net_profit_discontinued',
 'Zysk (strata) netto z działalności zaniechanej', 'monetary', NULL, 'reported', NULL);
