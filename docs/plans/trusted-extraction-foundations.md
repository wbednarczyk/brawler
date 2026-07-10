# Trusted extraction foundations — backfill → classification → history sweep → vision tier → coverage map

Epic: execution card `25cd300`, cross-cutting under `971aff6` (ADR 0061 — this plan delivers its tier-4/last-resort layer plus the automation the owner's trust verdict demands) · Design: [ADR 0077](../adr/0077-trusted-extraction-foundations.md). Lands ahead of `v0.51.0`; no milestone renumbering. Owner approved the design 2026-07-08.

Goal (owner's acceptance journey, verbatim): *add LPP → click Backfill → result reports are automatically processed, data extracted and visible in the panel.* Trust comes from cross-validation plus **measured** recall/precision on real reports — never from faith in the extractor. AI is a fallback behind determinism, **free tiers only** (owner decision: paid APIs are out; candidates = Gemini free tier + Mistral OCR free tier, chosen by a numeric spike).

Known gaps this plan closes (explored 2026-07-08):

- **Gap 1:** `run_backfill` (`jobs/backfill.rs:42`) ends at fetching attachments — nothing triggers extraction.
- **Gap 2:** the only autopilot trigger is source refresh, and `run_detection_sweep` deliberately takes only the newest document per type (`jobs/autopilot.rs:708`) — history is skipped by design. Closed by the new history sweep, **not** by widening refresh-sweep.
- **Trust asymmetry:** the AI-confirm path stores facts with `validation_status='none'` (`storage/kpi_extraction.rs:435`, comment at `:509-513`) while deterministic facts pass full `validate` (balance sheet, FCF, cash-flow tie, cross-check).
- **Pacing bug:** `run_kpi_extraction_job` swallows every failure into `Ok(failed)`, so the queue's 2..64s retry backoff (`jobs/queue.rs:51`) never engages on 429.
- Idempotency per (company, document) already exists (`create_run_if_absent`) — batching over history is safe.

## Pre-decided shapes (from ADR 0077 — STOP-AND-ASK before deviating)

- **Taxonomy** `doc_kind`: `periodic_ssf | periodic_jsf | auditor_opinion | presentation | governance | other` (NULL → unclassified). `classify_statement` is reimplemented **on top of** `classify_doc_kind` so the two can't drift; existing `classify_statement` tests stay green. Canonical report per period: ssf > jsf, newest revision (pure function).
- **Coverage is a computed read model, not a table** (v1): per company — periods from reports (`derive_report_period`) ∪ periods with facts; row: has_report, has_facts (+validation), review_state, skipped_budget. Small cardinality; `spawn_blocking`.
- **History sweep** is a new durable job: triggered at the end of `run_backfill` (gap 1) and by a manual command/button. Selector: canonical periodic reports whose period has no accepted facts. Shared `enqueue_extraction_run(...)` extracted from detection sweep; **refresh-sweep stays newest-per-type unchanged**. Table `history_sweeps` (status, counters, AI budget).
- **Tier-4 (AI/vision) lives in `jobs/structured_extraction.rs`, NEVER in pure `pipeline.rs`.** Trigger: determinism ends Flagged|Empty. AI output passes the **same `validate` + `validate_tier`**; validated → facts `source_tier='ai'` with a real `validation_status`; unvalidated → proposals (existing confirm mechanism — no new workbench). **`validation_status='none'` is banned on every AI path**, including the v0.36 confirm-path repair.
- **Provider is chosen by spike numbers, not preference.** Free-only candidates: (a) Mistral OCR → markdown → deterministic table parser (pure Rust post-OCR), (b) Mistral OCR → markdown → text-LLM, (c) Gemini native-doc. The spike also measures real Mistral free-tier limits (account console — unpublished) and 429 behavior. If none clears the bar → tier-4 is descoped, the sweep runs deterministically, and the coverage map honestly shows the residue (an acceptable epic outcome).
- **Cost v1 = call counter** (`complete_document` returns no usage; changing the trait is out of scope). Settings: `history_sweep_ai_call_limit` (default 30, 0 = off), `backfill_years` (default 3, range 1–10; replaces the constant — the coverage map exposes the 3-years-vs-`cagr(revenue,3)`-needs-4-FY gap). Exhausted budget → `skipped_budget` visible in the map, never a silent skip.
- **Pacing fix:** ProviderLimit/transient → `Err` (queue backoff 2..64s engages); terminal errors → fast exhaustion with a clear message.
- **PDF rasterization is FORBIDDEN in v1** (pure-Rust cross-build constraint; both candidates accept documents natively).

## Phases

```
F0 Spike: ground truth + provider verdict ── GATE G0 (owner) ──┐
F1 Taxonomy → F2 Coverage map → F3 History sweep               │
T5.1 429 pacing fix (independent — can land immediately)       │
F4 Vision tier-4 + provider ◄──────────────────────────────────┘
F5 Counter + budget + UI → F3b enable tier-4 in sweeps
F6 Closure: LPP live-drive + recall/precision numbers + docs + retro
```

Lane A (F1→F2→F3: classify.rs, backfill.rs, new job, UI) can run parallel to F0/Lane B (providers/*, structured_extraction.rs, kpi_extraction.rs). Every task: red test first. T5.1 + F0 start now; F1+ starts after v0.50 closure.

## Tasks

### F0 — Spike: ground truth + provider verdict [M] — GATE G0

**T0.1 Ground truth + metrics harness (this is the durable recall-ratchet, G-3).**
Scope: hand-label 2–3 CBF quarterlies (double-pass: agent proposes values from the PDFs, owner verifies) → `private/realdata/t7-cbf/ground_truth/*.json` (gitignored). One JSON per (document, period): `{ document_file, company: "CBF", fiscal_year, period_type, facts: [{ metric_key, value, unit, statement, page }] }` — metric keys from the KPI catalog only. New realdata test `src-tauri/src/storage/tests/extraction_metrics.rs` (pinned to the corpus like `t7_cbf_corpus.rs`): runs the deterministic pipeline over the labeled documents, computes recall (labeled facts found) and precision (found facts correct vs label), prints a per-document table, asserts against a floor that starts at the measured baseline. Make target `realdata-extraction-metrics` beside `realdata-extraction-check` (same `REALDATA_DIR` guard, skips cleanly when `private/realdata` is absent). Docs: testing.md gains the ground-truth/metrics section in the same change.
Tests-that-redden: the metrics test itself (fails if corpus present and recall/precision drop below the pinned floor).
Acceptance: baseline numbers printed and recorded in ADR 0077's evidence section; owner has verified the labels (second pass).

**T0.2 Provider prototypes (throwaway, gitignored `private/realdata/spikes/`).**
Scope: 3 shapes — (a) Mistral OCR → markdown → deterministic table parser, (b) Mistral OCR → markdown → text-LLM, (c) Gemini native-doc — over the same labeled files, measured on the **residue of determinism** (that is tier-4's actual job): recall/precision on residue, calls per document, real free-tier limits and 429 behavior (owner creates the free Mistral account and provides the key), latency. Scripts stay out of the app; no repo code changes beyond notes.
Acceptance: a numbers table (per shape) ready for G0.

**G0 (owner gate).** Choose a shape or descope tier-4. Proposed bar: post-validation precision ≥ 0.98, recall on residue ≥ 0.80, free tier practically bearable. Entering F4 without a numeric verdict = STOP.

### F1 — Document taxonomy [M]

**T1.1** ADR 0077 ratified sections + `DocKind` + `classify_doc_kind` in `fundamentals/extraction/classify.rs`; contract test over a committed corpus of labeled GPW titles (`src-tauri/testdata/doc_titles_labeled.json`) — **G-2**; `classify_statement` reimplemented on `classify_doc_kind`, its tests untouched and green.
**T1.2** Migration `0061_report_document_kind.sql` (nullable `doc_kind` + index); idempotent `reclassify_report_documents` command (classification is Rust code, not a SQL backfill).
**T1.3** Canonical-report-per-period (pure function + tests: ssf beats jsf, newest revision wins).
**T1.4** `doc_kind` badge + filter in `CompanyReportDocumentsPanel` (ui primitives; DTO + mock + fidelity + contracts.md in the same change).

### F2 — Coverage map [M]

**T2.1** `get_fundamentals_coverage(companyId)` — read model + command + DTO + mock/fidelity; a Rust test per cell state (report/no-report × facts/validated/proposals/skipped_budget).
**T2.2** Coverage panel (mockup-first: approved mockup in `docs/mockups/` before JSX): grid period × {report, facts, review}; click → document list / review queue. `FundamentalsPanel.buildFactMatrix` stays (it answers a different question).

### F3 — History sweep [M]

**T3.1** Shared `enqueue_extraction_run` + `history_sweep_candidates` selector (canonical periodic reports, period without accepted facts); selector + idempotency tests.
**T3.2** Durable job `history_sweep` + migration `0062_history_sweeps.sql`; chained from `run_backfill`; manual command + button; trust ladder unchanged (off never auto-runs).
**T3.3** `backfill_years` setting + visible sweep progress (docs/runs/AI used-vs-limit) + explicit reporting of `MAX_BACKFILL_PAGES` truncation.

### F4 — Vision tier-4 [L] — G0 DECIDED 2026-07-08: **hybrid** (ADR 0077 evidence section)

Shape: Mistral OCR + per-company **OCR-extraction profile**. The text-LLM runs once per company to *bootstrap* the profile (label map, value-column layout, **scale** — mln/tys./full PLN is a first-class profile field; spike evidence: fabrications come from the LLM reading numbers, never from mapping labels); every document then goes through the deterministic markdown-table parser driven by that profile. Profile bootstraps and low-confidence output land as proposals; everything passes `validate`/`validate_tier`. Gemini rejected (measured: 20-req/window quota, 503 storms).

**T4.1** Mistral provider (`providers/analysis/mistral.rs` modeled on `anthropic.rs`: OCR endpoint + chat, 429 → ProviderLimit, keychain, catalog entry, key form; OCR v4 tables arrive separately in `pages[].tables` and must be stitched into the markdown) + the pure markdown-table parser with proptest + insta + the profile schema (versioned, per company — instantiates ADR 0061's tier-3 profile philosophy at the OCR tier; exact storage shape decided at F4 kickoff against the existing extraction-profile substrate). Scale guardrail: magnitude cross-check vs prior period catches a 1000× mis-scale.
**T4.2** `AiCapability::VisionExtraction` (Document class) — full ceremony + roundtrip tests (**G-5**; the hard-fail on Document-capability-without-Native-support stays).
**T4.3** Tier-4 hook in `jobs/structured_extraction.rs`: Flagged|Empty + budget available → call → parse → `validate`/`validate_tier` → facts with a real status, or proposals. Guardrail test **G-1**: an AI fact with `validation_status='none'` reddens; a payload violating the balance-sheet identity lands as a proposal, never a fact.
**T4.4** Confirm-path fix: confirming a proposal runs `validate` before persisting (end of `'none'`).
**T4.5** Manual AI-extract (v0.36) rewired through the same tier-4 function (one implementation; knip/deadcode clean).

### F5 — Cost / budget / pacing [S]

**T5.1 429 pacing fix (independent — lands immediately; repairs a live bug).**
Scope: `run_kpi_extraction_job` (`jobs/kpi_extraction.rs:19-75`) must stop swallowing transient failures. `extract` already returns `(error_code, message)` where provider failures map through `AnalysisProviderError::code()`. Split by class: transient (`ProviderLimit`, `ProviderUnavailable`, `NetworkError` codes) → return `Err(...)` from the job runner so `jobs/queue.rs` schedules the 2..64s backoff retry — **without** marking the domain job terminally failed while attempts remain; terminal (bad config, `non_pdf_document`, parse/`ProviderError`, missing provider) → current fast-fail path (`mark_kpi_extraction_job_failed` + `Ok(failed)`). Verify both call sites stay coherent: the queue handler and `stage_extract` (`jobs/autopilot.rs:346`). Record queue-visible status so the UI does not show a silent stall.
Tests-that-redden (write first): a queued kpi-extraction job whose provider returns `ProviderLimit` is retried with backoff (attempts increment, job not terminally failed until `max_attempts`); a terminal error still fast-fails with its error code. Extend `jobs/queue.rs`-style tests or the kpi_extraction test module — follow the existing scripted-provider pattern from `providers/analysis/pool.rs` tests.
Docs: contracts.md / data-model.md only if a visible status shape changes; otherwise none.
Acceptance: `make check` green under Nix; the two new behaviors covered; no change to retry semantics of other job kinds.

**T5.2** `ai_calls_used` counter + limit enforcement → `skipped_budget` (**G-4**: a sweep never exceeds the limit and never silently drops).
**T5.3** UI: AI used/limit in sweep progress, settings entries, `skipped_budget` in the coverage map.
**F3b** Enable tier-4 inside sweeps only when T4.3 + T5.2 are green.

### F6 — Closure [M]

**T6.1** Acceptance journey LPP live-drive (`tests/live/backfill-extraction.spec.ts`): add LPP → Backfill → sweep → facts in panel + coverage map + badges + counter. **Real LPP data is the completion evidence.**
**T6.2** Numbers: recall/precision before/after on the CBF corpus recorded in ADR 0077; ratchet re-pinned deliberately; corpus + double-extraction anchor green.
**T6.3** Docs (contracts de-`planned`-tagged), wiki/, retro (app + dev loop), owner sign-off.

## Docs / ADR / migrations

- **ADR 0077** (single ADR — the trust invariant binds both halves; spike verdict lands as a dated evidence section, ADR 0052 pattern). Amends 0061/0055/0060/0036.
- contracts.md: coverage command, `run_history_sweep`, `reclassify_report_documents`, `doc_kind` in DTOs, sweep progress event, capability, settings. data-model.md: `doc_kind`, `history_sweeps`, settings keys, provenance note. product-spec.md: taxonomy, coverage map, backfill→sweep automation, budget. testing.md: ground truth, metrics target, ratchet, `'none'` guard. ui-flows/IA + ux-journeys (LPP journey). roadmap.md: epic entry.
- Migrations: 0061 (`doc_kind` nullable + index), 0062 (`history_sweeps`) — append-only, nothing breaking; settings are generic KV with safe defaults.

## Guardrails (harvested as part of this epic)

G-1 AI never `validation_status='none'` · G-2 taxonomy contract test · G-3 recall/precision ratchet on real data · G-4 budget-stop · G-5 capability roundtrip + Document⇒Native · G-6 (rule + checklist) PDF rasterization ban.

## Tripwires (STOP-AND-ASK)

PDF rasterization · AI facts with `'none'` or any loosening of `validate` · entering F4 without a numeric G0 verdict · changing newest-per-type in refresh-sweep · IO in `pipeline.rs` · a new review workbench · editing a shipped migration.

## Epic Definition of Done

1. LPP journey green end-to-end on the real Windows app (live-drive).
2. Recall/precision on the CBF corpus recorded in ADR 0077, ratchet pinned; `realdata-extraction-check` + double-extraction anchor green.
3. `make check` green under Nix; docs-drift green; knip clean (every command reachable in the UI).
4. Retro before sign-off; guardrails G-1…G-6 active.
