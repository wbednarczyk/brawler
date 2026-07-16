-- Aggregator ownership reset (parser-defect repair, 2026-07-16).
--
-- Defect class: the first BiznesRadar "Akcjonariat" parser collected only `<td>`
-- cells and walked BOTH `table.qTableFull` on the page. That ingested (a) each
-- table's `<th>razem</th>` summary row — whose first `<td>` "93.22 %" became a
-- holder name and whose second "12 867 479" (a share COUNT) became a capital % —
-- and (b) the entire "Pozostali akcjonariusze" sub-5% fund-statement table. Result:
-- garbage `aggregator` stakes on every tracked company (donut showed 13 720 265,3%).
--
-- The fixed parser ingests ONLY the "Główni akcjonariusze" table, skips `<th>`
-- (header/summary) rows, rejects percentage-named or >100% rows, and drops a basis
-- whose disclosed capital sums > 102%. This migration clears every previously
-- ingested aggregator basis so the next daily/manual refresh rewrites clean bases
-- under the fixed parser. Only the `aggregator` source is touched — report and ESPI
-- stakes are the depth/provenance record and are never aggregator-derived.
--
-- Forward-only, idempotent, self-healing (data-model migration rules): a re-run is
-- a no-op once no aggregator rows remain.

DELETE FROM ownership_stakes WHERE source = 'aggregator';
