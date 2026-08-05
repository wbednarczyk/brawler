# ADR 0095: Retire the html_positional extraction tier

Status: Accepted (2026-08-05, owner decision in chat after the #182 measurement; scope widened same day: FULL removal executes immediately in the #182 PR — all pdf-tier facts deleted incl. legacy `pdf/api` rows, parser/routing/harness removed, no write path may produce tier `pdf` again. Boundary: report_diff's PDF *text* reading (ADR 0052, a reader not an acquirer) and the WDF cover-note tier stay. The ladder is ESEF → WDF → BiznesRadar-primary, 100% deterministic acquisition.)

Deciders: maintainer. Area: fundamentals, extraction, data trust.

## Context

ADR 0086 retired deterministic PDF fact extraction and promoted the BiznesRadar aggregator
to primary for core KPIs, leaving a ladder of ESEF → structured xHTML/positional → WDF
cover note → BiznesRadar. The positional tier (`extraction_method='html_positional'`,
persisted as `source_tier='pdf'`) survived as a layout-heuristic reader of pdf2htmlEX-style
xHTML renderings — spiritually the last remnant of PDF layout scraping.

The first ground-truth measurement of the structured tiers (epic #40 / #182, 2026-08-05 —
corpus of 32 real filings, 1035 machine-verified labels) put numbers on it:

| | positional | ESEF (kept) |
|---|---|---|
| facts in the production DB | 118 (8 companies) | 885 (45 companies) |
| measured precision / recall (value-only) | 92% / 35% (CDR-dominated: 12/17 corpus docs) | 100% / 73% |
| measured precision / recall (**currency-aware**, final) | **5/90 (5.6%) / 5/252 (2.0%)** — 78 "matches" store no currency at all | **142/143 (99.3%)** / 72.4% (one KGH row tagged AED) |
| defects found in one measurement pass | 3 (footnote-list column bug, Dino year shift, basis mis-stamping) | 0 wrong values |

The tier's marginal value is near zero: its headline interim figures are covered by the
measured WDF cover-note tier (347/347 on its own labeled corpus, zero-fetch) and by the
BiznesRadar primary (quarterly series); only 4 of its 118 facts ever earned witness
corroboration. Its layout heuristics are the app's most defect-dense surface per fact
produced, and its fragility classes (column picking, note-reference stripping, basis
stamping) are open-ended — every new issuer layout is a new way to be wrong.

## Decision

1. **Retire `html_positional`.** The extraction ladder becomes **ESEF → WDF cover note →
   BiznesRadar-primary** (agent tier per ADR 0093 unchanged). No new positional facts are
   produced; the parser, its routing arm, and its per-document plumbing are removed.
2. **ESEF stays, explicitly.** It is the only fully policy-clean, issuer-authoritative
   structured source (EU-mandated iXBRL), measured at 100% precision; its recall gap is a
   concept-map widening (#327), not a fragility problem. It also hedges the BiznesRadar
   dependency (ToS-gray, changeable) with an independent primary-quality source.
3. **Stored positional facts are deleted by a forward migration** (owner decision
   2026-08-05, superseding the initially considered demote-below-aggregator: demotion
   still displays a measured-5.6%-precision, currency-less value wherever BiznesRadar does
   not cover the slot — "never silently wrong" forbids that). These are derived,
   machine-extracted facts of a retired tier, not user data; the migration itself is the
   audit record. Deleting frees the slots honestly: the history sweep sees the periods as
   uncovered again and BR/ESEF/WDF re-fill them under their own provenance on the next
   refresh/sweep — including the 4 witness-corroborated values (the corroboration proves
   the aggregator holds them). The execution slice must verify referencing surfaces
   (evidence links, coverage map, read models) tolerate the deletion, and keep legacy
   read/MCP mappings for the `pdf` tier *value* only as long as historical snapshots
   require (raw `sourceTier` is an exposed surface).
4. **Measurement v2 (#331) narrows to ESEF-only**, which dissolves its hardest audit
   blockers (CDR concentration, PL/EN twin estimand) by construction.

## Consequences

- Execution is a dedicated slice (tracked card): remove the parser + routing + F6b harness
  (its 0.99 floors describe a retiring tier), update data-model.md ladder prose,
  contracts/coverage surfaces that name the tier, and the #182 diagnostic scorer's
  positional arm (kept only as a stored-state auditor of the retained facts, or dropped).
- The #182 corpus keeps its positional ground truth as historical evidence; no new
  positional labeling.
- Companies whose interims are xhtml-only with no text-layer WDF and no BR coverage lose a
  (rare) acquisition path; the coverage map surfaces such gaps honestly, and the agent tier
  (ADR 0093) remains the manual-assist route.
- ADR 0086's ladder description is amended by this ADR; ADR 0052's report_diff text
  extraction (reading, not fact extraction) is untouched.
