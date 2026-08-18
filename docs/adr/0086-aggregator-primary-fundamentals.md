# ADR 0086: Aggregator-primary fundamentals, PDF-fact retirement, review-free facts

Status: Accepted (2026-07-21, owner decisions at Track C planning); decisions 2 and 7 superseded on completion by [ADR 0098](0098-mcp-native-kpi-acquisition-lifecycle.md) (BiznesRadar: primary → witness/complement per slot; agent: additive → agent-first); **amended for tagged filings by [ADR 0100](0100-two-layer-tagged-fact-capture-and-ifrs-vocabulary.md)** (epic #398): where an ESEF/iXBRL package exists, full capture reads ~400 tagged facts per report against the aggregator's ~20–30, so the issuer package is the **breadth source** for the periods it covers and the aggregator corroborates. The precedence ladder is unchanged — `esef` already outranks `html_aggregator`; only the stated posture moves.

Deciders: maintainer. Area: fundamentals, sources, architecture.

## Context

v0.59 fixed the deterministic PDF fact reader three times over (unit declarations, poisoned
per-company profiles, a retry storm) and a full-database audit still found 152 mis-scaled values
across 11 issuers — including companies never suspected before. The structural lesson: **every
issuer's PDF is different, and each newly tracked company reopens the fight.** Meanwhile the two
spec-driven formats — ESEF (annual, iXBRL-tagged) and the regulator-mandated WDF cover-note table
(interim, measured 347/347 on the owner's corpus) — are issuer-independent and stable, and the
BiznesRadar aggregator publishes machine-consumable statements for the whole GPW.

Separately, the owner rejected the ratification workflow: manually reviewing and confirming facts
"kills the app's usability". The app must be an automaton: data simply arrives, honestly labeled;
editing is an option (manual now, MCP write-tools later), never a duty. The codebase is also to be
slimmed down — the PDF fact arm, its profile machinery and its review flows are the bloat.

## Decisions

1. **The PDF fact-extraction arm is retired.** No production code path parses financial FACTS out
   of PDF statements anymore: `parse_pdf_text*`, the pipeline tier-2 arm, the per-company
   `ExtractionProfile` (bootstrap/merge/drift) and their tests are deleted (clean cut, restorable
   via git tag `pre-track-c`). PDF text reading SURVIVES only for: document period derivation
   (registry grouping, cached per ADR 0061/0109) and insider/ownership attachment parsing (separate
   domain). The in-app report viewer originally planned alongside this cut was **deferred out of
   v0.59 to a future milestone** (owner, 2026-07-22 — first mockup rejected; stored PDFs remain on
   disk, viewable externally until then). Shared numeric/label helpers (`detect_unit_scale`,
   `parse_amount`, `normalize_label`, `match_dictionary_label`, `is_per_share`, `UnitScale`) move
   to a common extraction module — they serve the surviving html/positional tiers.
2. **BiznesRadar is promoted from witness to PRIMARY source for core KPIs** (pattern: ownership,
   ADR 0072 — role is code-side, snapshots labeled by tier, daily cadence). The adapter fetches
   THREE robots-allowed pages per tracked company per day (`raporty-finansowe-rachunek-zyskow-i-strat`,
   `-bilans`, `-przeplywy-pieniezne`), caches them per (company, page_kind), and writes facts for
   EVERY period column the pages carry: `source_tier='html_aggregator'`, citation carrying the page
   URL + row label. Attribution stays visible; no redistribution (sui-generis posture as ADR 0072).
3. **The extraction ladder becomes**: ESEF → StructuredXhtml/positional → EspiCoverNote (WDF) →
   **BiznesRadar-primary**. Precedence per slot: `manual` > `esef` > `espi_cover_note` >
   positional > `html_aggregator`; a higher tier re-observes/overwrites a lower tier's slot, the
   aggregator only ever overwrites itself, and MANUAL facts are untouchable by every automatic path
   (divergence is logged, never applied).
4. **Reversed witnessing.** Where an issuer tier (ESEF/WDF) holds a slot, a disagreeing aggregator
   value records an informational `witness_disagreement` outcome — it never blocks or overwrites
   the issuer value. An empty/zero aggregator cell against a non-zero issuer value is a scrape
   artifact, never evidence (the BFT zero-guard, ADR 0085 amendment).

   **Amendment (2026-07-22, code-review of the BR-primary pull).** Two clarifications, both made
   concrete in `jobs::aggregator_fundamentals_pull`:
   - **"Issuer tier" is the full issuer-produced taxonomy, not a hand-listed subset.** The
     positional `pdf` tier (the issuer's own filing read deterministically, ADR 0077) counts as an
     issuer tier for reversed witnessing, alongside ESEF / structured xHTML / WDF cover-note. The
     only NON-issuer tier is the aggregator's own `html_aggregator`. This is enforced by
     `SourceTier::is_issuer` (`= !HtmlAggregator`); both the pull's issuer check and the storage
     `aggregator_owns_slot` predicate parse the enum instead of string-matching literals, so a new
     tier cannot be silently omitted.
   - **A MANUAL-held slot's divergence also records the informational outcome** (decision 3's
     "divergence is logged, never applied", made concrete). The manual value is never touched, but
     the aggregator's disagreement with the user's own entry is recorded as the same review-free,
     never-blocking `witness_disagreement` (tier `manual`) plus a structured log line — the user
     must be able to learn of the conflict, not have it silently dropped.
   - **The outcome `detail_json` is the canonical gate shape** the WDF witness seam writes
     (`{failedIdentities, failedCrossChecks, witnessDisagreements:[{metricKey, detail:{expected,
     actual, residual, …}}]}`, convention `expected` = aggregator, `actual` = the held filing/manual
     value), so the Coverage "Flagged periods" panel renders it as investor language rather than raw
     JSON keys. The prior flat `{aggregatorValue, issuerValue, …}` object leaked raw keys into the
     UI and is retired.
5. **Facts are review-free.** No fact ever lands `pending` or waits for ratification;
   `confirmation_state` is frozen (column kept for compatibility, every writer stamps `confirmed`,
   existing rows unified). Truth about origin lives in `source_tier` + `extraction_method` +
   citation, surfaced as labels — never as a to-do for the user. Flagged periods remain an
   informational surface with a manual retry. Editing paths: manual add/edit/delete of KPI values
   stays; MCP write-tools (planned, NS1 arc) become the agent-assisted option — both are OPTIONS,
   not workflow. This amends ADR 0055 (trust ladder = provenance labels, not workflow).
6. **One-off data rebuild (owner-approved, 2026-07-21).** All existing facts (manual included) are
   wiped by a MANUAL one-time SQL cleanup on the live DB (with a file backup first) — deliberately
   NOT a migration: migrations remain schema-only and never delete user data. Repopulation runs
   through a `rebuild fundamentals` command: BR-primary pull (all companies × all period columns)
   + full ESEF re-extraction + WDF re-scan of stored cover-note carriers.
7. **Agent + MCP stays additive.** Nondeterministic extraction (an agent reading a PDF) may only
   enter through the MCP write path with mandatory provenance, subject to the same validation
   gates — it is an option on top of the automaton, never a tier inside it.

## Consequences

- New-company onboarding stops being a parser fight: core KPIs arrive automatically from BR from
  day one; annual detail from ESEF; interim core corroborated by WDF.
- Detail lines absent from BR/ESEF/WDF (issuer-specific subtotals) are simply absent — by design;
  reading the filing (externally until the deferred in-app viewer ships) + manual/MCP entry cover
  the long tail.
- The recall harness re-baselines against the new ladder (coverage = ESEF+WDF+BR).
- ADR statuses: **supersedes ADR 0061 decisions 1/3** (ladder + PDF parser section); **amends
  ADR 0085** (witness → primary; zero-guard), **ADR 0055** (review-free), **ADR 0084** (the
  "gaps refill from PDF" note expires). ADR 0077's positional sub-tier survives unchanged.

## Rejected options

- **Paid fundamentals APIs (EODHD/FMP/Notoria):** weak or paid GPW coverage, aggregator numbers
  without filing citations, policy cost (no paid APIs) — dominated by BR, which is already
  integrated and free.
- **Bankier as the aggregator:** no machine-consumable fundamentals surface in our reach and an
  AI-hostile robots posture (same verdict as ownership, ADR 0072).
- **Keeping the PDF arm frozen-but-present:** dead code rots, confuses agents and contradicts the
  de-bloat goal; git history is the archive.
