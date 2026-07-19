# ADR 0061: 100%-Deterministic Fundamentals Data Gathering — Structured-First Pipeline + Validation + Company Profiles

Status: Accepted (2026-07-01)

Supersedes the "Gemini-Pro-for-documents" premise of [ADR 0060](0060-ai-capability-routing-and-openai-compatible-provider.md)
for the KPI-extraction path (ADR 0060's per-capability routing + OpenAI-compatible provider survive for
text/qualitative tasks and the AI fallback pool). Amends [ADR 0055](0055-autonomous-report-pipeline-trust-ladder.md)
(the autopilot extract stage) and [ADR 0036](0036-report-document-storage-and-backfill.md) (document fetch).
Relates to [ADR 0028](0028-multi-provider-ai-boundary.md), [ADR 0052](0052-report-over-report-diff.md)
(pure-Rust extraction is reliable), [ADR 0027](0027-company-fundamentals-scope.md), and
[ADR 0049](0049-test-architecture-v2-data-transform-correctness.md).

Cross-cutting fundamentals-reliability epic — **no `milestone:` label**.

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
   - **2a. ESPI cover-note "WYBRANE DANE FINANSOWE" (`EspiCoverNote`)** — *Status: planned (spike-adopted
     2026-07-19, card `76a4636`)*. The same mandated table, taken not from a fetched attachment but from
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
3. **Deterministic PDF parser** (interim full statements) — a curated **Polish label → `metric_key`**
   dictionary + a value parser (current-period column selection, unit multiplier tys/mln, sign =
   parentheses, note-ref/dot-leader stripping).
4. **AI over extracted text** (never native PDF) — last resort for the residual tail only, through the
   provider pool (decision 5).

### 2. The "good" gate — objective validation (no human, no guessing)
A parsed value is auto-accepted only if it passes **all**: (a) **accounting identities** (Assets =
Liabilities + Equity; subtotals sum; cash-flow ties: opening + Δ = closing); (b) **comparative-period
cross-check** (the filing's prior-period column equals the fact stored a year earlier); (c) **structure
match** to the company profile (decision 3); (d) **completeness** for the company's KPI profile. Failure →
the HTML witness (decision 4), then a user notification — never a silent emit. Every fact carries its
**source tier + validation status + primary-source citation**.

### 3. Per-company extraction profile + drift → learning loop
A **versioned per-company/template profile** records which labels → which KPIs, sections, unit convention,
column layout (+ a template hash). On each new report, drift vs the profile (a) signals distrust → runs
the HTML witness, and (b) fires a **"structure changed" notification with a clean label diff** (no
gibberish). When drift + the fallback confirm the new values, the **profile is updated** → the next period
is automatically "good" again. A company/template is **bootstrapped once** (the only place the owner
glances at start); thereafter it is zero-touch and self-validating; drift → re-bootstrap.

### 4. HTML aggregator as second witness + fallback
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
emission).

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
