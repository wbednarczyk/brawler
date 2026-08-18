-- Regression fix (ADR 0100 decision 3, epic #398): a bare iXBRL instance (not
-- inside a ZIP package) carries no `*_pre.xml` presentation linkbase, so
-- EVERY one of its facts would fail the strict primary-statement role filter
-- unconditionally -- the document would silently project zero facts, a real
-- (if small) regression measured on the maintainer's DB: 13 facts from one
-- bare-instance document out of 493 ESEF facts total. The projection now
-- falls back to the pre-epic dimensionless + crosswalk-resolved selection for
-- exactly this evidence-free case; this counter is that fallback's visible,
-- never-silent record: `0` for a document that carried linkbase evidence, the
-- number of affected facts otherwise.
--
-- `TAGGED_FACT_EXTRACTOR_VERSION` bumps 1 -> 2 in the same change, so every
-- existing generation rebuilds and reports this counter instead of a stale 0.
ALTER TABLE report_tagged_fact_extractions
    ADD COLUMN no_linkbase_fallback_count INTEGER NOT NULL DEFAULT 0;
