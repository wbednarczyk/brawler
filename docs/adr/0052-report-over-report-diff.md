# ADR 0052: Report-over-report diff — pure-Rust statement diff, narrative MD&A deferred

Status: **Accepted** (v0.47.0). Scope narrowed to structured financial
statements after a real-data spike; the narrative management-report (MD&A) diff
and the AI delta summary are **deferred** to a later milestone. Recorded so the
extraction approach and its real-data evidence are not re-litigated.

## Context

The `v0.47.0` milestone (epic `db6be22`) sets out to show what actually changed
between consecutive periodic reports — section-level comparison plus a cited
delta summary — so the investor stops rereading 80 unchanged pages. Stored
report documents already exist ([ADR 0036](0036-report-document-storage-and-backfill.md)):
full files are persisted locally for periodic/financial reports, the diff
targets. No PDF **text extraction** existed in the codebase before this milestone —
KPI extraction sends the *native* PDF to a multimodal provider and never extracts
text ([ADR 0027](0027-company-fundamentals-scope.md)). A deterministic
section-level diff needs reproducible extracted text, so text extraction is this
milestone's central new capability and its main risk.

Per the project's **real-data-validation-precedes-implementation** rule
([ADR 0045](0045-guardrail-harvest-loop.md), [docs/testing.md](../testing.md)) —
the same rule that killed cross-source clustering in
[ADR 0051](0051-story-clustering-across-sources.md) — the extraction/alignment
approach was prototyped and measured against the maintainer's real watchlist
report PDFs **before** committing to a design.

## Evidence (real data)

A throwaway pure-Rust spike (`pdf-extract` for text, `similar` for diffing,
heuristic section detection) was run over **11 real GPW periodic-report PDFs
across 6 issuers** (CyberFolks full consecutive series Q3'25→Q1'26 as
consolidated / standalone / management-report; plus LPP, MODIVO, ABE, Creotech
Instruments, Creotech Quantum).

**Extraction quality (pure Rust):**

| Metric | Result across 11 PDFs / 6 issuers |
| --- | --- |
| Extraction verdict | **GOOD** on all 11 |
| Alphabetic-character ratio | 0.80–0.89 (clean text, not garbage) |
| Polish diacritics | intact on every file |
| Edge cases | a report `file(1)` saw as "0 pages" and a deflate-encoded report both extracted cleanly |

**Deterministic section diff (heading + positional alignment):**

| Pair | Heading alignment | Outcome |
| --- | --- | --- |
| Report vs **itself** (self-diff) | **100% identical, 0 delta** | deterministic invariant holds |
| Consolidated statements (SSF) Q3'25→Q1'26 | **85%** | clean per-section deltas |
| Standalone statements (JSF) Q3'25→Q1'26 | **~92%** | clean per-section deltas |
| Narrative management report (MD&A / *raport kwartalny*) | **4%** | exact-heading alignment collapses |

Why each behaves as it does:

- **Pure-Rust extraction is reliable** across the issuer-format spread, including
  signed and deflate-encoded PDFs. No AI is needed to obtain the text.
- **Structured financial statements diff well.** Their section headings
  (*skonsolidowane sprawozdanie z sytuacji finansowej*, *…z zysków i strat*, …)
  are stable and templated, so heading-keyed alignment reaches 85–92% with
  meaningful per-section line deltas.
- **The narrative management report does not.** Its headings drift quarter to
  quarter and its tables pollute heading detection, so exact-string alignment
  collapses to 4%. This is exactly where a deterministic line diff is also least
  meaningful — "what changed" there is interpretation, not line edits.
- **A naive exact-heading matcher is not deterministic.** Duplicate heading keys
  cross-matched the wrong instance, so a report diffed against itself was *not*
  empty until alignment was changed to consume each target at most once
  (positional identity). The deterministic self-diff = empty invariant is a hard
  test gate, not an afterthought.

## Evidence (market-wide — whole GPW + NewConnect)

To prove extraction robustness rather than assume it, the extraction step was run
against the **entire market**: a harness reusing the *shipped* `bankier_company`
resolver harvested the latest periodic report for all **770 companies** (413 GPW +
357 NewConnect), yielding **613 real reports** (599 PDF + 14 ESEF/iXBRL `.xhtml`).
Outcomes:

| Outcome | Count | Share | Meaning |
| --- | --- | --- | --- |
| GOOD | 548 | 89.4% | clean, sectionable text, Polish diacritics intact (PDF **and** xhtml) |
| NO_TEXT_LAYER | 64 | 10.4% | correctly flagged not-diffable (58 scanned/image PDFs — mostly small NewConnect; 6 mis-selected non-statement xhtml) |
| PANIC | 1 | 0.16% | `pdf-extract` crashed internally (lib.rs:1490) — must be caught |
| silent garbage | 0 | 0% | no report produced wrong text passed off as good |

Three findings that change the design — none visible on the watchlist sample:

- **ESEF/iXBRL is a mandatory second format, not an edge case.** Under the EU ESEF
  mandate, larger issuers file the statement as iXBRL `.xhtml` with **no PDF** (CD
  Projekt, DataWalk, Gobarto, …); their pages carry only chrome PDFs. A PDF-only
  extractor silently misses them. The xhtml path extracts *cleaner* than PDF
  (HTML structure), once the hidden inline-XBRL header/`display:none` facts are
  stripped.
- **`pdf-extract` panics on some valid PDFs (~0.16%).** A panic on the UI-offloaded
  job would crash it. Extraction must run inside `catch_unwind` and treat a panic
  as `extraction_failed` (flagged, not-diffable) — never propagate.
- **No-text-layer is a real ~10% class** (scanned reports from small issuers).
  The reliable signal is **text density (chars/page)**, not alpha-ratio — number-
  dense statements (JSW, CFSA) are low-alpha but fully extractable. Scanned reports
  are flagged not-diffable; **OCR stays out of scope**.

## Decision

**v0.47.0 ships a pure-Rust, deterministic, structured-financial-statement diff —
no AI, fully local/offline/free.**

1. **Extraction — deterministic Rust, no AI, dual-format, panic-safe.** A pure-Rust
   step extracts text and detects sections from stored periodic **financial-statement**
   documents (consolidated SSF, standalone JSF), in **both** formats found across the
   market: **PDF** (`pdf-extract`) and **ESEF/iXBRL `.xhtml`** (HTML parse, stripping
   the inline-XBRL header/`display:none` facts). The diff is fully reproducible over
   that text. Required robustness, proven necessary by the market-wide run:
   - extraction runs inside `catch_unwind`; a `pdf-extract` panic becomes an
     `extraction_failed` state (flagged, not-diffable) — it never crashes the job;
   - a report whose text density (chars/page) is below threshold is classified
     `no_text_layer` (scanned/image) and surfaces an explicit "can't diff — no
     extractable text" state; **OCR is out of scope**;
   - the extractor must not emit `pdf-extract`'s glyph/ligature logging to
     std{out,err} (it floods output) — route or silence it.
2. **Alignment — heading + lexical baseline, embedding enhancer.** Sections align
   by heading + lexical similarity, always-on, with **positional consumption** so
   duplicate headings never cross-match. When the user has enabled the embedding
   model ([ADR 0035](0035-two-layer-ai-and-local-interpretative-layer.md)),
   semantic similarity refines ambiguous matches. The feature is **never blocked**
   on the optional model download — it degrades to the lexical baseline, matching
   the model's reversible-to-static posture. Exact-string heading matching alone
   is explicitly insufficient (it is why the MD&A collapses).
3. **Persistence — persist sections, diff on-demand.** Extracted sections are
   written once to a derived, rebuildable `report_document_sections` table
   (content-hash keyed; dropping it only forces re-extraction). The diff itself is
   an **on-demand backend read model** — no stored diff projection. There is no AI
   summary in this milestone, so no summary cache.
4. **Scope — structured statements only.** The diff covers consecutive same-type
   financial statements. The **narrative management-report (MD&A) diff** and the
   **AI delta summary** are deferred (see below). Financial-table *value*
   reconciliation remains owned by KPI extraction ([ADR 0027](0027-company-fundamentals-scope.md)),
   not this diff. Cross-company diff stays out of scope.
5. **Surfacing.** The statement diff is reachable from the company workspace and
   on new periodic-report arrival, primitive-first per
   [ADR 0037](0037-ui-component-framework-and-authoring-contract.md).

### Why narrow, rather than ship the full epic

The epic's original exit criterion included a cited AI **delta summary**. The
real-data spike shows the summary's primary target — the narrative MD&A — is
exactly where deterministic alignment fails (4%), and an AI summary over the
*statement* deltas overlaps KPI extraction's table territory while adding little
("revenue line moved" is already visible in the diff). Narrowing to statements
keeps a high, measurable quality bar (85–92%, deterministic self-diff = empty),
ships entirely local with no AI-provider dependency, and defers the genuinely
AI-shaped narrative work to a milestone that can also strengthen section
detection. This is the synthesis of the maintainer's directive ("if pure Rust is
reliable, we can skip AI") with the evidence (pure Rust is reliable *for
statements*).

## Deferred (follow-on milestone, not dropped)

- **Narrative management-report (MD&A) diff** — needs stronger section detection
  (numbering + known-label + layout heuristics; font-size cues are lost in plain
  text) and embedding-backed alignment before it is trustworthy.
- **AI delta summary with dual citations** — the narrative "new risks / tone
  shift / changed segment commentary" interpretation, behind the AI provider
  boundary ([ADR 0028](0028-multi-provider-ai-boundary.md)), optional and
  re-runnable. It belongs with the MD&A work where it has a real target.

These are recorded on the epic and its child issues, not closed silently.

## Consequences

- New derived table `report_document_sections` (append-only, idempotent,
  self-healing migration; rebuildable — losing it forces re-extraction). Each
  document carries an extraction state: `extracted` | `no_text_layer` |
  `extraction_failed` (panic-caught). Both `pdf` and `xhtml` source formats are
  extracted; only `extracted` documents are diffable.
- New runtime dependency: a pure-Rust PDF text-extraction crate (`pdf-extract`),
  validated to build under Nix before adoption, **wrapped in `catch_unwind`** (it
  panics on ~0.16% of real market PDFs). Its glyph/ligature fallback logging must
  be silenced/routed, not emitted to std{out,err}. ESEF/iXBRL `.xhtml` is parsed
  with the existing `scraper` HTML stack — no new dependency.
- New typed read-model commands for extracted sections and the on-demand diff
  (see [contracts.md](../contracts.md)); no AI command this milestone.
- Heavy work (extraction over a multi-page PDF, the cosine scan when the embedding
  enhancer is on) is offloaded off the UI thread per the standing async rule
  ([AGENTS.md](../../AGENTS.md)).
- Test gates: a golden extraction snapshot per issuer format (PDF and xhtml); a
  **panic-safety** test (the known-panicking PDF class yields `extraction_failed`,
  not a crash); a **no-text-layer** test (a scanned report yields `no_text_layer`);
  the **deterministic self-diff = empty** invariant; an alignment eval over the real
  consecutive pairs with a precision floor; idempotence/order-stability property
  tests on the diff ([ADR 0049](0049-test-architecture-v2-data-transform-correctness.md)).
  The market-wide corpus (613 reports) is the extraction-robustness reference set,
  kept under `private/` (out of CI).

## Bugs surfaced by the spike (filed, guardrail-harvest)

- **Report-document backfill title↔URL misattribution** — some stored CyberFolks
  rows point at *Vercom* / *Shoper* attachment URLs (wrong company's bytes under
  the right company's title). Data-integrity bug in the backfill attachment
  resolution.
- **Watchlist report coverage gap** — 10/18 watchlist companies have zero stored
  report documents; most others are `metadata_only` or mislabeled. Backfill ran
  richly for only one company.
