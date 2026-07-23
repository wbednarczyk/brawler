# ADR 0084: Retire the In-App AI Analysis Layer — Intelligence via MCP

Status: Accepted (2026-07-20, owner sign-off)

Follows the retirement pattern of [ADR 0080](0080-retire-embedding-model.md). **Supersedes** [ADR 0060](0060-ai-capability-routing-and-openai-compatible-provider.md) (per-capability routing + OpenAI-compatible provider — the routed layer itself is removed; epic `e1c3fac` closes) and [ADR 0077](0077-trusted-extraction-foundations.md) (tier-4 OCR extraction). **Amends** [ADR 0028](0028-multi-provider-ai-boundary.md) (the multi-provider analysis boundary is dissolved; the transcript-provider boundary survives), [ADR 0059](0059-worker-pools-and-queue-fairness.md) (AI lanes/per-provider semaphores go; non-AI lanes unchanged), [ADR 0061](0061-deterministic-fundamentals-data-gathering.md) (the "AI over extracted text" tier is struck — the pipeline ends at the aggregator witness), [ADR 0068](0068-attention-routing-and-morning-briefing.md) (the briefing is deterministic-list only), [ADR 0075](0075-qualitative-assessment-frameworks.md) (qualitative criteria verdicts become manual now, agent-written via MCP later), and **reverses [ADR 0080](0080-retire-embedding-model.md) decision 6** (the parked AI claim-extraction pipeline is now removed, owner decision 2026-07-20; the manual claims path stays). Execution detail: [docs/plans/v0.59-ai-retirement-deterministic-fundamentals.md](../plans/v0.59-ai-retirement-deterministic-fundamentals.md).

## Context

Owner conclusion (2026-07-19/20, planning session): **why keep an in-app AI layer when the user can talk to their research through MCP?** In-app AI capabilities run hardcoded prompts the user cannot change, against free-tier models whose quotas and availability routinely fail. The evidence record agrees:

- The embedding model never beat the static baseline in any per-capability eval and was retired (ADR 0080); story clustering lost to a heuristic (ADR 0051); ADR 0061 exists because free-tier Gemini returned more 5xx than success on the KPI path.
- Dogfooding 2026-07-19 (owner's real DB): the attention stream showed a wall of "KPI extraction unavailable (no AI provider configured)" rows. **Factual correction recorded at owner's insistence:** the Mistral provider *was* configured — its free quota was exhausted. The app collapsed a quota failure into a wrong "not configured" diagnosis (`compose_summary`, `jobs/autopilot.rs:691`), so even the failure reporting of the AI layer was misleading. Either way the measured daily value of the layer was ~zero.
- An external agent over MCP inverts every weakness: frontier model (user's own subscription), user-owned prompts, full context via typed tools, instant iteration. This is an architectural gap, not an execution gap — no amount of in-app prompt tuning closes it.

A full capability inventory (2026-07-20, three-agent audit) found 11 routed `AiCapability` variants (`providers/analysis/capabilities.rs`) plus the out-of-enum transcript generator. Verdicts below. The deterministic substrate the app actually runs on — ESPI rule classifier, `signal_dates`, the structured-extraction tiers, the composed briefing list — does not depend on any of them.

## Decision

1. **Remove the analysis-provider layer entirely**: `providers/analysis/` (capabilities, routing, pool, cooldown gate, prompts, registry, the `ocr_document` trait hook) and all analysis adapters — Gemini-analysis, Claude, OpenAI, generic OpenAI-compatible, Mistral (OCR-only in practice). No adapter is kept "for later": they implement the analysis trait, not transcription, and sunk cost is not an argument. **Reversibility: tag `v0.58.0`** is the last release with the live layer; re-introduction of any in-app inference goes through a fresh eval-gated ADR that beats the deterministic baseline on real data (the ADR 0080 bar), never through resurrecting this code by default.
2. **Per-capability disposition** (input read → fate):

   | Capability | Reads | Fate |
   |---|---|---|
   | `vision_extraction` (tier-4 OCR: KPI, ownership OCR, mgmt holdings) | report PDFs | **removed** (decision 4) |
   | `kpi_extraction` (legacy enum variant) | — | **removed** (dead: no production call site) |
   | `claim_extraction` | report PDF / transcript text | **removed**; manual `create_management_claim` stays (reverses ADR 0080 §6) |
   | `feed_analysis` | feed-item text | **removed** |
   | `research_brief` / `research_digest` | transcript segments + notes | **removed** |
   | `morning_briefing` (narrative phrasing) | typed composed items | **removed**; deterministic `gather_sources` + `compose_briefing` list stays as the only briefing |
   | `event_date` / `signal_classification` (AI fallbacks, default OFF) | filing title+body | **removed**; deterministic rule classifier + `signal_dates` stay; residual unknowns land in an explicit unclassified bucket (future MCP triage tool, `v0.60.0`) |
   | `qualitative_assessment` | report excerpts + transcripts + typed | **removed**; criteria verdicts are manual now, agent-written with provenance via MCP write-tools later (`v0.60.0`) |
   | `ownership_holder_classification` | holder name (typed) | **removed** with the layer; holder types stay user-editable |
3. **Transcripts stay** — `VideoTranscriptProvider` trait + the Gemini implementation. Transcription is data acquisition (speech→text), not interpretation; it is the only Gemini/API-key use left. The trait remains the pluggability boundary for future engines (e.g. local Whisper).
4. **Tier-4 OCR removed from the fundamentals pipeline** (executes with the ADR 0061 completion work): `fundamentals/extraction/ocr/*`, `run_tier4_extraction`/`tier4_with_provider`/`Tier4Gate` + per-sweep budget, the OCR paths of ownership/management-holdings extraction, `SourceTier::AiText`, the dead `KpiExtraction` variant. The pipeline is ESEF → xHTML → EspiCoverNote → PDF → aggregator witness; a document no deterministic tier parses is **flagged with a notification, never silently absent and never guessed**. A future *local deterministic* OCR tier (pure-Rust `ocrs`) is an open option gated on the recall/precision harness showing scanned documents materially hurt coverage.
5. **Clean cut — the AI artifacts are removed, not orphaned** (owner decision, revised 2026-07-20 later the same day; supersedes this decision's original "stored outputs remain readable / no table drops" text, which is preserved in git history). Rationale: backward compatibility with a retired subsystem buys nothing long-term and leaves zombie tables, zombie read surfaces, and coverage numbers inflated by a source that no longer exists. Capabilities worth keeping survive **as manual capabilities**; what goes is AI *in the pipeline* and everything it left behind. The boundary was set against a measurement of the owner's live database (2026-07-20), not by table name — names mislead here:

   **Removed** (forward, idempotent, snapshot-tested migrations, per data-model rules): `ai_analysis_*` (results/jobs/tags/source_references), `ai_research_brief*` + `ai_research_digest*` (+ citations/jobs), `claim_extraction_*`, `kpi_extraction_jobs` + `kpi_extraction_proposals` (confirmed proposals already materialized into `financial_facts` — only the staging ledger and 218 unreviewed OCR/AI-origin proposals go), `ownership_ocr_proposal*`, `ownership_holder_type_proposals`, `company_ocr_extraction_profile`; the `narrative_markdown` / `narrative_provider_id` / `narrative_model` columns on `morning_briefings`; the settings rows `ai_analysis_mode`, `ai_workers`, `ai_provider_concurrency`, `capability_providers`, `general_analysis_provider`, `espi_ai_fallback_enabled`, `history_sweep_ai_call_limit`; and the **26 `financial_facts` whose provenance is `source_tier='ai'`** together with those provenance rows — after the cut no tier could reproduce or validate them, and leaving them would inflate the deterministic-coverage measurement (A4 harness). The resulting gaps refill from ESEF/PDF/EspiCoverNote on the next sweep or stand as honest flagged gaps.

   **Kept — measured as NOT AI despite their names**: `criterion_results` (184 rows, **all `source='engine'`** — deterministic DSL evaluations per ADR 0046; the AI-assessor path never wrote to the owner's database), `company_extraction_profile` (deterministic PDF profiles, ADR 0061 decision 3), `morning_briefings`/`morning_briefing_items` (deterministic composition), `management_claims` (manual path), `financial_facts` from deterministic tiers, `youtube_transcription_provider` settings.

   **Consequently the read-only stored-output surfaces are removed too** — there is nothing left to display. All AI read commands (`list_ai_analysis`, `list_research_briefs`, `list_research_digests`, `get_qualitative_assessment`, `list_pending_kpi_proposals`, the KPI/ownership-OCR confirm/reject pairs) go with their tables.

   A separate forward migration purges queued jobs of the removed kinds so the durable queue cannot wedge on unknown work (`0101_purge_retired_ai_job_kinds.sql`, non-destructive: only `pending`/`running` rows of retired kinds). Destructive migrations are covered by the app's pre-migration snapshot + rotating backups (`v0.38.0`, ADR 0032) and are snapshot-tested for idempotence and self-heal, following the ADR 0080 decision-4 precedent.
6. **Honest failure reporting replaces guessed diagnoses**: `compose_summary` (autopilot) stops phrasing causes in English prose and emits typed reason codes (`quota_exhausted` / `provider_not_configured` / `provider_error` / `no_deterministic_tier`), rendered by the frontend through the translation layer. This closes the misdiagnosis class the dogfooding screenshot exposed (backend-composed user-visible strings are the `v0.60.0` Today-redesign seam; the reason codes land now so the data is right).

   **Completion (2026-07-21):** the migration is finished — **every** fragment `compose_summary` emits is now a typed token, not just the unavailable branch (`kpi_confirmed:<c>:<p>`, `kpi_pending:<p>`, `kpi_extraction_unavailable:<code>`, `report_diff_available`, `claims_to_verify:<n>`, `research_questions:<n>`, `expectations_to_review`, `report_processed`; joined with `"; "`). A fifth reason code, **`witness_fallback`** (`KpiUnavailableReason::WitnessFallback`), was added end-to-end — previously a witness-fallback gap (aggregator sourced the period, no issuer tier could read the filing; ADR 0085 / C1) collapsed into `no_deterministic_tier`, so the run's notification lied about its cause. The frontend `renderAutopilotSummaryTokens` translates the token stream (en + pl) and passes any non-token (legacy English-prose) summary through **verbatim** (tolerant read for existing DBs). Counts are declined on the frontend via `pluralNoun`, never baked into the token. Format contract: [contracts.md](../contracts.md) § Autonomous Report Pipeline.
7. **Credentials**: Claude/OpenAI/Mistral keychain handling and settings surfaces are removed; the Gemini key remains solely for transcripts. Existing OS-keychain entries are not actively deleted (outside app scope, harmless).
8. **Product identity is rewritten deliberately** (project-brief + CLAUDE.md Product Intent): Brawler is a **deterministic research substrate with an MCP port — the user brings their own agent (BYOA)**. "AI decision support" as an in-app feature family is retired; decision support arrives through deterministic computation plus the user's agent over MCP. Open-core consequence: the app runs fully featured with **zero API keys** (transcripts optional).

## Consequences

- The shipped binary loses four HTTP adapters, prompt corpus, routing/pool/gate plumbing, and every AI settings surface; onboarding needs no keys; no feature degrades on a third-party quota again.
- Feature losses are explicit and accepted: no claim proposals, no feed-analysis panel, no AI briefing prose, no qualitative auto-verdicts, no OCR coverage for scanned-only documents (now honest flagged gaps, measured by the `v0.59.0` recall/precision harness). Per the revised decision 5, **previously stored AI outputs are deleted rather than archived** — a deliberate, owner-approved one-way cut (5 research briefs, 2 digests, 2 holder-type proposals, 16 OCR profiles, 354 KPI staging proposals, 26 AI-sourced facts in the owner's live database; 0 feed analyses and 0 claim proposals existed). Their replacements arrive as MCP read/write tools (`v0.60.0` MCP surface v2) where an external agent does the same jobs with a better model and the user's own prompts, writing back with mandatory provenance.
- NS1 is promoted from north star to product spine: roadmap re-sequenced (`v0.60.0` Today reinvention, `v0.61.0` MCP surface v2, valuation arc +2 — MCP surface v2 folded INTO `v0.60.0` and the arc shifted back one at the 2026-07-22 v0.60 planning).
- ADR 0060's routing and ADR 0077's OCR foundations are formally retired; ADR 0061's honest-100% scope now reads "deterministic tiers + witness, gaps flagged" with no AI residual.
- Queue lanes shrink (ADR 0059 amendment); worker fairness rules for non-AI lanes unchanged.
- Migration 0046 (claim-extraction tables) and all AI-era data remain valid history under append-only rules.

## Open questions

- ~~Local `ocrs` OCR tier: decide only after the `v0.59.0` real-data harness quantifies the scanned-document gap (decision 4).~~
  **RESOLVED — no-go (2026-07-21), on measurement.** A dedicated evaluation spike classified the owner's
  entire fetched periodic-report population (n=723; 430 PDFs) by text-layer presence, using **two
  independent extractors** (`pypdf` and the app's own `pdf-extract`, 430/430 agreement, zero disputes —
  ruling out "our extractor is weak" as the cause). Result: **94.9% have a text layer, 4.4% are non-PDF
  containers mislabeled `.pdf` (card `eb71488`), and 3 documents — 0.7% — are genuine scans.** Of those
  three, two are Energa filings mis-associated to another company (card `c7e354d`) and one is a 4-page
  auditor review opinion carrying none of the line items we extract. **Documents a local OCR tier would
  actually recover: zero to one.** The orchestrator independently re-sampled (random n=60: 57 text-layer,
  1 scan, 2 mislabeled) and confirms the finding. No OCR engine is adopted; no dependency added; the
  pure-Rust cross-build constraint is therefore not challenged.
  **Where the real headroom is:** the same spike showed a crude regex finds label-and-number co-location
  in 37–65% of documents against the pipeline's 23.2% conversion — so the raw material is present in the
  text layer for far more documents than we convert. The bottleneck is the **label dictionary and table
  parser**, downstream of text acquisition, plus period derivation upstream of it (card `fc692da`).
  **Scope caveat:** this conclusion is scoped to the owner's current watchlist. `data-model.md` estimates
  a ~10% no-text-layer class market-wide, concentrated in small NewConnect issuers; if the watchlist
  expands materially in that direction, the classification is cheap to re-run and the answer could change.
- Exact read-only surfaces for legacy stored outputs (which panels keep a viewer vs. archive-only): resolved mechanically during execution by "a view exists where users already had one; nothing new is built".
