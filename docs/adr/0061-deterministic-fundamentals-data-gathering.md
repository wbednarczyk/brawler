# ADR 0061: 100%-Deterministic Fundamentals Data Gathering — Structured-First Pipeline + Validation + Company Profiles

Status: Accepted (2026-07-01); decisions 1/3 (tier ladder, deterministic PDF parser) superseded by
[ADR 0086](0086-aggregator-primary-fundamentals.md) (2026-07-21): the PDF fact arm is retired,
BiznesRadar is aggregator-primary for core KPIs, facts are review-free. ESEF/WDF decisions stand.

Supersedes the "Gemini-Pro-for-documents" premise of [ADR 0060](0060-ai-capability-routing-and-openai-compatible-provider.md)
for the KPI-extraction path (ADR 0060's per-capability routing + OpenAI-compatible provider survive for
text/qualitative tasks and the AI fallback pool). Amends [ADR 0055](0055-autonomous-report-pipeline-trust-ladder.md)
(the autopilot extract stage) and [ADR 0036](0036-report-document-storage-and-backfill.md) (document fetch).
Relates to [ADR 0028](0028-multi-provider-ai-boundary.md), [ADR 0052](0052-report-over-report-diff.md)
(pure-Rust extraction is reliable), [ADR 0027](0027-company-fundamentals-scope.md), and
[ADR 0049](0049-test-architecture-v2-data-transform-correctness.md).

Cross-cutting fundamentals-reliability epic — carried **no `milestone:` label** until 2026-07-19, when the owner slotted the remaining slices into **`v0.59.0`** (alongside the Today-view reinvention; [roadmap](../roadmap.md)).

## Context

The v0.49 autopilot extracts KPIs by sending the report **PDF natively to Gemini**, whose free tier
returns more 502/503 than success. The owner's requirement is **100% deterministic, 100% automatic** KPI
gathering — **no human in the loop as the steady state** (reviewing a few extractions is acceptable only
to bootstrap).

Two hard truths shape the design:

- **100% *provable* determinism is not achievable from raw PDFs** — they have no schema, layouts vary
  without bound, so no parser is provably correct for all future filings. It *is* achievable from
  **structured/tagged data** (XBRL/iXBRL, structured xHTML, a data provider's API). So the source of
  truth must be structure; PDF becomes a validated backup, not the authority.
- **Real-data spike (this session, owner's DB):** pure-Rust text extraction on the real CBF 2026 Q1 SSF
  PDF gave clean text (diacritics intact, consistent with ADR 0052's 613-filing validation), and a naive
  Polish label dictionary matched **15/17 core KPIs with values on the first try** (the 2 misses were
  dictionary gaps, not extraction failures) → PDF recall ~90% for the core set: a strong backup, not the
  authority. Coverage across the 50 tracked companies: **264 structured (xHTML/iXBRL) + 6 XBRL documents
  are already referenced** (discovered via bankier-komunikaty attachments) but almost all **unfetched**;
  of companies with ingested reports, ~8 have a structured doc and ~30 are **PDF-only**. So structured
  data is real and grows over time, but PDF + a validation layer + an aggregator witness are essential,
  not optional.

## Decision

A **layered, self-validating pipeline**. A KPI is taken from the highest available tier; every emitted
fact is either validated or flagged — **never silently wrong**.

### 1. Source tiers (best provenance/determinism first)
1. **ESEF / iXBRL** (annual, regulated-market GPW). Parse the Inline-XBRL package: `ix:nonFraction` facts
   with context (period), unit, `scale`/`sign`/`decimals`; map **IFRS concept → `metric_key`** (a stable
   map, unlike free text). Exact, ~100% for tagged facts. The fetch path (ADR 0036) must retrieve the
   already-discovered structured packages (today it fetches PDFs and skips them).
2. **Structured xHTML "wybrane dane finansowe"** (some interims) — deterministic HTML-table parse via the
   existing xHTML seam.
   - **2a. ESPI cover-note "WYBRANE DANE FINANSOWE" (`EspiCoverNote`)** — *Status: implemented (spike-adopted
     2026-07-19, shipped 2026-07-20, card `76a4636`)*. The same mandated table, taken not from a fetched attachment but from
     the plain-text body of the periodic-report komunikat **already ingested** by the Bankier primary —
     zero-fetch, available the day of publication, instant breadth for companies with no fetched
     documents. Slots **below** `StructuredXhtml` (issuer cover-note figures are untagged, so they never
     outrank iXBRL/xHTML) and above `Pdf`. Deterministic row grammar (roman-numeral or custom-label rows;
     tys/mln unit headers; minus and parenthesis signs) with the **PLN↔EUR cross-check as the emit gate**:
     a concatenated digit run is split only when the form's own FX-footnote rate confirms exactly one
     split — abstain otherwise. Extraction runs **at ingest time** and persists via
     `fundamentals_provenance` with a feed-item citation — mandatory, because feed retention can delete
     the carrier text (measured 2026-07-19: a prune removed 448/451 WDF bodies). Known limits: headline
     lines only (no health-score depth — ESEF/PDF stay necessary); `Liczba akcji` scale depends on a
     per-row `(w tys.)` annotation and is treated conservatively. Spike evidence: 15-doc hand-labeled
     corpus, 347/347 recall and precision, 0 false values (`private/realdata/spikes/espi-wdf/RESULTS.md`).
     **As shipped** (2026-07-20): the ingest hook is a post-commit step of `ingest_bankier_company_items`
     (feed ingestion is the stronger guarantee — an extraction failure never rolls it back); only
     `is_periodic_report_item` komunikaty are parsed; the period comes from the shared
     `period_from_title_url` derivation and an underivable period **abstains**; facts run the same
     `validate_parsed_set` gate as every tier and land `auto_unreviewed`; on an occupied slot an
     `esef`/`structured_xhtml` (or provenance-less/manual) fact is left untouched while a `pdf`/
     `html_aggregator` fact is upgraded in place. **Owner decisions at review (2026-07-21):** (a) the
     in-place **upgrade stands** rather than routing the disagreement to the flagged-review queue —
     the higher tier carries the issuer's own figure, and holding a correct number back for manual
     clicking would contradict the epic's "100% automatic" requirement; the upgrade is therefore
     **recorded in full** (metric, previous value, previous tier, new value) as a `tier_upgrade`
     diagnostic, because a lower tier disagreeing with the issuer is the primary drift signal for
     decision 3's learning loop — a bare counter would discard that evidence; (b) `auto_unreviewed`
     **stays for this milestone** despite the 347/347 corpus result, and is revisited once the
     real-data recall/precision harness supplies measured numbers rather than a judgment call.
     Persistence/observability contract: [data-model.md](../data-model.md).
3. **Deterministic PDF parser** (interim full statements) — a curated **Polish label → `metric_key`**
   dictionary + a value parser (current-period column selection, unit multiplier tys/mln, sign =
   parentheses, note-ref/dot-leader stripping).
   **Retired 2026-07-21 ([ADR 0086](0086-aggregator-primary-fundamentals.md) dec. 1):** no production
   path parses financial FACTS out of PDFs anymore (`parse_pdf_text*` deleted). The ladder now ends at
   BiznesRadar-primary — ESEF → structured xHTML/positional → EspiCoverNote (WDF) → `html_aggregator`.
   PDF text survives only for document period derivation and insider/ownership parsing.
4. ~~**AI over extracted text** (never native PDF) — last resort for the residual tail only, through the
   provider pool (decision 5).~~ **Struck 2026-07-20 by [ADR 0084](0084-retire-in-app-ai-layer.md):** the AI
   residual tier and the tier-4 OCR realization ([ADR 0077](0077-trusted-extraction-foundations.md)) are
   removed. The pipeline ends at the **HTML aggregator witness** (decision 4); a document no deterministic
   tier parses is **flagged with a notification**, never guessed and never silently absent. Decision 5 (AI
   provider pool) is likewise removed. Honest-100% scope now reads: automatic on the deterministically
   covered set, gaps explicit and measured (the `v0.59.0` real-data recall/precision harness).

**Period derivation is upstream of every tier** (amendment 2026-07-21, card `fc692da`). A document whose
reporting period cannot be derived never reaches *any* tier — no parser, no new tier and no OCR can help
it — and the `v0.59.0` A4 sweep measured that as the single largest gap: **1 144 of 1 544** eligible stored
documents, **83 of them (31.7% of the kind) `periodic_ssf`** — real financial statements invisible purely
for want of a period parse. The derivation (`report_diff::classify`) therefore has **one grammar over two
carriers**:

1. **title/URL** — patterns derived from the maintainer's real issuer forms, not from imagination: calendar
   period-end dates (`30.06.2025`, `2025_09_30`), `QSr/PSr N`, glued `1Q2026`/`Q125`/`2023q3`/`1HY2025`,
   roman and word quarters across any separator (`III_kwartal`, `IIIQ`, `pierwszy kwartał`), `HY`/`H1`,
   month counts (`3M`/`6M`/`9M`/`12M`), `PSSF`/`PSF`, `SA-Q`/`SA-P`/`SA-R`, `rok obrotowy`, `za <rok> r.`,
   and Polish/English month-end phrases (`30 czerwca 2025`, `30 June 2025`).
2. **the document's own cover page** — when the title/URL name nothing and the document is a periodic
   statement (a bare `SSF.pdf` is a real, recurring attachment shape), the first ~1 500 characters of the
   extracted text are run through the **same** grammar. Restricted to periodic statements on purpose: the
   documents that legitimately have no period are ~3 000 of the 3 790 stored files, and a text extraction
   each to confirm an expected `None` would make every sweep an overnight run.

Both carriers keep decision 1's abstention contract, and the widening **adds** one abstention: a stated
reporting window that does not open on 1 January (a non-December fiscal year — Synektik reports 1 Oct –
30 Sep) abstains outright, because the calendar mapping this derivation assumes would be wrong there.
Genuine ambiguity — two glued readings, or two calendar period ends the text does not order as a
current-vs-prior pair (separated by "oraz", or listed ascending) — abstains as well. The **one**
comparative shape that is NOT ambiguous is resolved rather than abstained: a balance-sheet header pairing
the current period end with the prior one, both calendar boundaries, current listed first and therefore
later (`31.03.2025 31.12.2024` → Q1 2025; annual `31.12.2024 31.12.2023` → FY 2024) — the document names
the period explicitly there, and abstaining lost real statements. A **feed-title** carrier adds two
title-only rules: the Polish `z dnia <date>` publication idiom is stripped (it is never the reporting
period), and an explicit period marker wins over a lone bare date (a bare feed-title date is usually the
publication date). An undecidable period stays `no_period_derived`: nothing is persisted, and "I don't
know" is never widened into a guess.

### 2. The "good" gate — objective validation (no human, no guessing)
A parsed value is auto-accepted only if it passes **all**: (a) **accounting identities** (Assets =
Liabilities + Equity; subtotals sum; cash-flow ties: opening + Δ = closing); (b) **comparative-period
cross-check** (the filing's prior-period column equals the fact stored a year earlier); (c) **structure
match** to the company profile (decision 3); (d) **completeness** for the company's KPI profile. Failure →
the HTML witness (decision 4), then a user notification — never a silent emit. Every fact carries its
**source tier + validation status + primary-source citation**.

### 3. Per-company extraction profile + drift → learning loop
**Retired 2026-07-21 ([ADR 0086](0086-aggregator-primary-fundamentals.md) dec. 1):** the per-company
`ExtractionProfile` (bootstrap/merge/drift) is deleted with the PDF fact arm — no profile machinery
ships. Provenance is now durable labels (`source_tier` + `extraction_method` + citation), not a learned
per-company profile; structure-drift survives only as a flagged `reason_code` for the structured tiers.

A **versioned per-company/template profile** records which labels → which KPIs, sections, unit convention,
column layout (+ a template hash). On each new report, drift vs the profile (a) signals distrust → runs
the HTML witness, and (b) fires a **"structure changed" notification with a clean label diff** (no
gibberish). When drift + the fallback confirm the new values, the **profile is updated** → the next period
is automatically "good" again. A company/template is **bootstrapped once** (the only place the owner
glances at start); thereafter it is zero-touch and self-validating; drift → re-bootstrap.

### 4. HTML aggregator as second witness + fallback
**Amended — BiznesRadar promoted witness → PRIMARY 2026-07-21 ([ADR 0086](0086-aggregator-primary-fundamentals.md)
dec. 2/4, building on [ADR 0085](0085-biznesradar-fundamentals-witness.md)):** the aggregator is now the
**primary** source for core KPIs (daily pull of three raporty-finansowe pages per company, every period
column, `source_tier='html_aggregator'`), sitting **below** issuer tiers per slot. Where an issuer tier
(ESEF/WDF) holds a slot, a divergent aggregator value records an informational `witness_disagreement` and
never overwrites it; an empty/zero aggregator cell is never written.

Where an aggregator (**BiznesRadar / Bankier "wyniki finansowe" / StockWatch** — **not Stooq**, which is
price data) covers a company, cross-check its structured tables **routinely** (not only on failure):
agreement PDF↔aggregator ⇒ ~100% confidence. The primary filing is the source of truth, the aggregator a
witness; on disagreement → notify with a diff, never silently trust the aggregator.

**Chosen sources (2026-07-01): BiznesRadar primary + Bankier "wyniki finansowe" fallback**, both read through
one **source-generic** HTML-table adapter (rows = metrics, columns = periods; Polish labels reuse the tier-2
dictionary and number rules), so the concrete host is configuration, not code. BiznesRadar leads on GPW +
NewConnect fundamentals coverage; Bankier is the fallback because its fetch/politeness infra already exists
(`bankier_*` adapters). **The adapter (parser) ships now; binding a live BiznesRadar fetch is gated on a
source-specific scraping ADR** (ToS review, politeness/rate-limit, cache) per the source-strategy rule
(prefer official/RSS; fragile scraping needs source-specific ADR approval) — it is not wired to a live host
silently.

### 5. AI provider pool (availability failover)
The AI tier (last resort, and the text/qualitative tasks of ADR 0060) runs through an **ordered
`(provider, model)` pool per capability**: send to the first; **fail over on 5xx/429/timeout/connection
error** to the next, until success or exhaustion; a just-failed provider enters a short **cooldown**
(skipped first). Builds on ADR 0059's per-provider concurrency gate (the pool takes the next provider and
*its* semaphore) and ADR 0060's routing (one provider → a pool). This improves **availability, not
correctness** — pool output still passes the decision-2 validation before landing; failover never triggers
on a valid 200 with bad content. Adding any OpenAI-compatible model (GLM/Zhipu, Groq, OpenRouter) is
configuration, not code.

## Consequences

- Gemini (and any single AI provider) leaves the critical path; fundamentals no longer depend on a flaky
  free tier. The number that matters comes from tagged data or a validated deterministic parse.
- **Honest 100% scope:** 100% automatic on the covered/bootstrapped set (ESEF, aggregator-covered, or
  once-bootstrapped templates); a NewConnect micro-cap with no aggregator coverage needs a one-time profile
  bootstrap. The set grows toward full with each company. **"Never silently wrong" holds always** via the
  validation layer.
- New assets: an IFRS-concept→`metric_key` map, a Polish label dictionary, and per-company profiles (none
  exist today — `kpi_definitions` carry English labels only).
- Reuses `extract_report`/`split_sections`/`extract_xhtml` ([report_diff/extraction.rs](../../src-tauri/src/report_diff/extraction.rs)), `report_document_sections`, `financial_facts`, and the trust ladder.

## Guardrail (ADR 0045)

No KPI fact is emitted without a validation status: a value that fails the identities/comparative/structure
checks and cannot be corroborated by a witness is **flagged, never silently stored**. A deterministic
`insta` golden + property tests pin each parser; an **end-to-end pipeline test** (sample bytes,
`run_until_idle`, no network) pins the tier ordering + gate + fallback + drift loop (closing the ADR 0049
"no e2e ingestion pipeline test" gap); a `#[ignore]` real-data harness measures recall/precision on the
owner's filings before any default flip.

## Status notes

Accepted 2026-07-01. Direction and every design fork co-decided with the maintainer across an extended
planning session: structured-first (ESEF mandated in-milestone), the objective "good" gate, per-company
profiles + drift/learning loop (maintainer's idea), HTML witness (not Stooq), the AI provider pool, and the
honest 100% scope (bootstrap-once, never-silently-wrong). Delivered in slices S0–S6 (S0 = this ADR + the
coverage spike). The v0.49 extract silent-failure fix and the ADR 0059 queue-fairness work already shipped;
this epic builds the reliable data-gathering spine on top.

Decision 5 (the AI provider pool) was implemented 2026-07-02 together with the accepted-and-amended
[ADR 0060](0060-ai-capability-routing-and-openai-compatible-provider.md) (per-capability ordered-list
routing + the generic OpenAI-compatible provider).

Decision 2's gate checks (b) comparative-period cross-check and (d) completeness are now live in the
structured-extraction path (2026-07-02, closing the gap where `validate` was always called with
`comparatives`/`stored_prior` hard-wired to `None`): `run_structured_extraction` derives the prior
fiscal year/period end, reads the already-stored prior period back via
`storage::financials::stored_fact_set` (bridging `definition_id` → `metric_key`), and derives
`expected_keys` from the company's `active`+`primary`-ranked `kpi_relevance` rows. Each tier
(ESEF/PDF/HTML) reads the prior-period column out of its own freshly-extracted facts and
cross-checks it; a mismatch drives the existing Failed→Flagged/witness escalation, never a new
acceptance state. The PDF parser (tier 2) gained `parse_pdf_text_with_comparatives`, reading the
report's second value column (the prior-period comparative Polish statements print alongside the
current figure) as additional facts stamped with the prior period end — distinguishable purely by
period end, so they never pollute the emitted current-period set. Completeness is report-only
(`ValidationReport.completeness`, never flips `Status`); the pipeline downgrades an otherwise-`Accepted`
set to `AcceptedUnreviewed` only when it hits **zero** of the expected primary KPIs (never blocks
emission). **The check was inert in production until `v0.59.0`** (card `fb93944`): its denominator comes
from `kpi_relevance`, and that table held **zero rows**, so `expected_primary_metric_keys` returned `None`
for every company and completeness had nothing to measure. Owner decision 2026-07-21: seed a common IFRS
**core set** — revenue, operating profit, net profit, total assets, total equity — as a starting
denominator (migration `0106`, contract in [data-model.md](../data-model.md)), while the durable
per-sector/per-company selection is studied separately (card `3569d99`). The seed never touches a curated
row.

Decision 1b (the fetch path retrieving already-discovered structured packages) landed 2026-07-02 for
**direct `.xhtml` links** discovered on the Bankier komunikaty attachment path: a structured attachment is
now always a fetch candidate (migration 0058 also flips already-registered, never-fetched `.xhtml`
attachments from `metadata_only` to `pending`), the corresponding `.xades` signature stays `metadata_only`,
and autopilot detection prefers the structured document on a disclosure-date tie against a PDF sibling.
**ZIP/ESEF package attachments (the multi-file taxonomy bundle) remain out of scope** — only single-file
`.xhtml` links are handled; unpacking a ZIP package is a separate follow-up.

**Amendment (2026-07-19) — `EspiCoverNote` tier adopted from the espi-wdf spike** (v0.58 spike card
`bdda6cf`, owner decision in-session). The mandated ESPI "WYBRANE DANE FINANSOWE" cover table, already
present as plain text in ingested Bankier periodic-report komunikaty, becomes a planned tier `2a`
(decision 1 above): zero-fetch, publication-day breadth, strictly below `StructuredXhtml` in trust.
Measured on a 15-document hand-labeled corpus from the owner DB (347 facts, pass-2 verified): recall
and precision 347/347, zero false values; 33 ambiguous digit-run splits all resolved by the form's own
PLN↔EUR rate, which becomes the emit gate (abstain over guess). Implementation is a separate card
(`76a4636`, parent epic scope): 1:1 Rust port of the measured parser pinned to the same ground truth,
ingest-time hook in the Bankier adapter, provenance citation of the feed item. The ingest-time
requirement is load-bearing: the same session measured the app's (since-disabled) automatic feed prune
deleting 448 of 451 WDF carrier bodies — lazily reading old feed text is not a viable route.

**Amendment (2026-07-21) — unit scale is READ, never guessed** (owner rule, dogfooding on the real DB).
The binding rule, in the owner's words: *"in every report, for every company, somewhere it is written what
unit the report is in; if nothing is written, assume no multiplier; needing to check/guess a multiplier
means the reader did not read the report/PDF accurately enough."* This makes the **document's own unit
declaration the only source of scale**, and reshapes `detect_unit_scale` (PDF tier) and the WDF header scan
(tier 2a) accordingly:

- **English declarations are first-class DECLARATION-tier votes**, alongside the Polish forms — at least
  `in thousands` / `thousands of` / `PLN '000` / `PLN thousand(s)`, and the millions counterparts
  (`in millions` / `millions of` / `PLN million`). *Real failure:* Benefit Systems H1 2025 filed an English
  translation SSF declaring "All amounts are expressed in thousands of Polish złoty"; the Polish-only
  detector saw no declaration, fell to the bare-caption tie, and a stray narrative "95 mln zł" flipped the
  whole filing to Millions → **every value stored ×1000 too big**. English declarations must win the same
  way a Polish declaration does. Precedence structure is unchanged: **declaration > bare caption**.
- **Silent default = no multiplier (raw złoty), NOT thousands.** With no declaration and no scale caption
  anywhere, the scale is `Ones` (×1). This flips the historical silent default (`Thousands`). *Real failure:*
  Digital Network Q1 2025 (`WYBRANE SKONSOLIDOWANE DANE FINANSOWE`, bare `PLN PLN EUR EUR` header, no unit
  declared) had its raw-złoty figures scaled up (revenue 15 395 950,73 PLN stored orders of magnitude too
  big). The cost of this rule: a genuinely-in-thousands document that declares nothing is now read raw — but
  per the rule such a document does not exist (the declaration is always written), and reading it raw is the
  honest failure mode versus a silent ×1000 guess.
- **Groszy corroboration.** An aggregate financial-statement line whose values carry a 2-digit comma decimal
  (groszy) is evidence of raw-złoty denomination — thousands/millions figures are whole. It **only breaks
  the silent case** (toward `Ones`, which is already the default); a **declaration always wins** over a groszy
  signal (a declared-thousands document showing groszy on aggregate lines is treated as thousands, the
  contradiction noted but not acted on). Groszy is therefore a documented corroborating invariant, not a
  production control-flow branch — encoding it as a branch that changes nothing would be misleading.
- **Quarantine stays a safety net, not a scale source.** The history-plausibility quarantine still catches a
  mis-scaled magnitude downstream, but scale itself is now resolved by *reading the declaration*, never by
  plausibility-guessing a multiplier — the two roles are kept distinct.
- **Profile scale demoted to informational** (after the live bypass incident, 2026-07-21 22:15). The
  per-company `ExtractionProfile.unit_scale` (§3) previously *overrode* the document's declaration at parse
  time — so a profile bootstrapped in the broken-detector era (Millions for thousands-declared filers: DVL,
  NWG, BFT…) silently bypassed the whole "read the declaration" rule at runtime. *Real failure:* a live
  re-extraction of Develia's Q1 2026 QSr (declares `w tys.` ×10, zero `w mln`) re-created revenue at
  892 109 000 000 (×1000 too big), while the same reader with `profile = None` produced the correct
  892 109 000. Resolution: **scale is now always `detect_unit_scale(document)`; the profile's `unit_scale`
  has no scaling authority** — it is retained only for serialization compat, drift reporting, and merge
  refresh (kept authoritative for the **label map**). A detected-scale ≠ profile-scale difference is **not**
  distrust-worthy: it no longer trips the drift gate (`Drift::is_drift()` is `false` for a scale-only diff),
  is recorded in the `DriftReport` as informational, and `merge_confirmed` refreshes the stale profile scale
  from the document on the next confirmed merge — so the facts EMIT at the correct scale and the profile
  self-heals. **Label-set drift semantics are unchanged** (a renamed/added/removed line still distrusts the
  parse, flags, and surfaces "structure changed").

**Candidate cross-check invariant (not yet an engine):** for a cover table carrying EPS, share count and
parent net profit, `EPS × shares ≈ net profit attributable to the parent` holds only at the correct scale
(Digital Network Q1 2025: 1,12 × 4 165 685 ≈ 4 665 567 ≈ 4 682 950,64 — the identity holds *only* raw,
proving no multiplier). Recorded here as a future plausibility/validation check, not built in this change.

Regression guards pinned: the KRUK/CDR/car declaration-counting tests, the A6 bare-caption tie-break
(caption `(tys. zł)` + narrative `mln zł` → Thousands), and the WDF 347/347 corpus (all 15 docs declare
their unit, so the silent-default flip does not touch them).
