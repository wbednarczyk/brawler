# Roadmap

This roadmap turns the current product and architecture plan into implementation milestones. It is intentionally milestone-based instead of date-based. It is **forward-looking**: it covers the active and upcoming milestones plus unscheduled future work. Delivered milestone history lives in [CHANGELOG.md](../CHANGELOG.md) (authoritative per-version release notes) and [Kanban Archive](kanban-archive.md) (completed-card detail); live epic/task status lives in Radicle/Radboard (see [Radicle/Radboard Tracking](kanban.md)).

Use [Project Brief](project-brief.md) for the full documentation map. Related references: [Project Practices](project-practices.md), [Radicle/Radboard Tracking](kanban.md), [Product Spec](product-spec.md), and [Source Strategy](source-strategy.md).

## Roadmap Principles

- Build from local-first foundations toward source ingestion and AI.
- Keep every milestone demoable.
- Milestones must close on real working behavior against the real local runtime, real source, real API, or real agent named by the milestone.
- Use test samples, mocks, and seed data only as intermediate development steps and in deterministic automated tests; do not treat them as completion evidence unless a milestone explicitly says it is a mock/sample-only spike.
- Do not introduce cloud services in v1.
- Keep contracts and docs updated with each milestone.
- Make local build/test commands the primary interface; GitHub Actions mirrors them.
- Use Nix from the first scaffold for reproducible WSL2 Ubuntu 24.04 development.
- Keep GitHub Actions feedback fast and secret-free by default.
- Minimize GitHub Actions usage while the repo is private: no larger runners, no default macOS CI, no scheduled jobs, and manual packaging until needed.
- Prefer lean, behavior-focused tests over broad brittle suites.
- Keep secrets in the OS keychain and non-secret settings in SQLite.
- Use SemVer-style `0.x.y` versions from the first scaffold.
- The local entitlement module supports optional and future gated entitlements, but normal open-core desktop use does not require a license key.

## Delivered

Milestones through `v0.38.0` are shipped. This roadmap does not restate completed work: the authoritative, per-version release history is [CHANGELOG.md](../CHANGELOG.md), and completed-card detail (through ~`v0.24.x`) is in [Kanban Archive](kanban-archive.md). The recent delivered arc:

- Through `v0.31.0` — local-first foundations, GPW ESPI/EBI and Polish media/research sources, the company workspace, notebooks and claims, YouTube transcription, the general AI analysis framework, developer-mode/diagnostics, watchlist management, import/export, portable Windows packaging, modularization, and the research workspace (questions, evidence links, briefs, reminders, digest).
- `v0.35.0` — Multi-provider AI (Claude + OpenAI alongside Gemini, all free with a user-supplied key).
- `v0.34.0`, `v0.36.0`–`v0.37.0` — Company Fundamentals: financial facts data model, AI KPI extraction with mandatory confirmation, and the fundamentals panel with KPI charts. Scope and the KPI taxonomy are fixed in [ADR 0027](adr/0027-company-fundamentals-scope.md).
- `v0.38.0` — Search and data safety hardening: unified FTS5 global search across all stored content (companies, watchlists, feed items, notes, transcript segments, events, briefs, digests), automatic rotating local database backups with restore, pre-migration snapshots, and a WAL connection pool for concurrent background jobs. Boundaries in [ADR 0032](adr/0032-search-and-backup-boundaries.md); feed-retention policy designed in [ADR 0033](adr/0033-feed-retention-policy.md) for a later milestone.
- `v0.39.0` — Interpretative AI layer (static foundation): the two-layer AI architecture and the on-device interpretative layer as capability contracts (`Classifier`, `SimilarityProvider`, `Matcher`, `SemanticSearch`) with registry selection, deterministic static baselines (rule classifier, lexical similarity), and a per-capability eval harness — no embedding model or vector store yet. Boundary and reversibility rules in [ADR 0035](adr/0035-two-layer-ai-and-local-interpretative-layer.md).
- `v0.40.0` — Typed ESPI event classification: official ESPI/EBI filings classified into typed `company_signals` (insider transactions, dividends, profit warnings, significant contracts, own-share transactions, guidance changes, general meetings) as the first consumer of the `Classifier` capability, with a deterministic rule classifier at ingestion plus an opt-in AI fallback that requires confirmation, and feed badge/filter, research-timeline, digest, and reminder surfacing. Calendar-event derivation for dated categories was deferred to `v0.41.0`. Design in [ADR 0034](adr/0034-espi-event-classification.md).
- `v0.41.0` — Report document ingestion & history backfill: ESPI/EBI report attachments persisted as stored report documents (full file for periodic reports, metadata + URL for other filings), an explicit on-track ~3-year history backfill of reports and filings, and derived dividend/general-meeting calendar events (deterministic-first body date parse with an opt-in AI fallback, always confirm-before-create). All target the active Bankier official-report path. Design in [ADR 0036](adr/0036-report-document-storage-and-backfill.md).
- `v0.42.0` — Management claims tracker: management claims promoted to a first-class entity with a due period and a user-set verdict; AI claim extraction (mandatory confirmation) from report documents and transcripts; a due-period derivation surfaces an open claim into a "claims to verify" review queue when the due-period report arrives, with KPI-backed verification for quantitative claims; claims flow into the research timeline, reminders/digests, and import/export. Design in [ADR 0040](adr/0040-management-claims-tracker.md).

## Active And Upcoming Milestones

The next milestone is `v0.43.0`. This is the forward plan (milestone intent only; live epic/task status and IDs are in Radicle/Radboard, see [kanban.md](kanban.md)):

- `v0.43.0` — **Report-season cockpit**: upcoming report dates with pre-report cards built from questions, claims, KPIs, and evidence.
- `v0.44.0` — **Cross-company KPI comparison**: side-by-side tables and multi-series trend charts.
- `v0.45.0` — **Quality frameworks (quantitative checks)**: a rule engine evaluates user frameworks against the fundamentals facts and produces a versioned scorecard; ships clonable templates including a Kroeze-style quality template. Depends only on facts (`v0.37.0`); resequenceable.
- `v0.46.0` — **Interpretative AI layer — embedding model**: add the on-device embedding model + vector store (`sqlite-vec`-class, a disposable derived index) behind the existing capability boundary, and enable the model-backed implementation per capability only where the eval shows it beats the static baseline. Sequenced immediately before its first high-value consumer (story clustering). Design in [ADR 0035](adr/0035-two-layer-ai-and-local-interpretative-layer.md).
- `v0.47.0` — **Story clustering across sources**: cluster near-duplicate multi-source coverage into single stories with the official source ranked first; first model consumer of the `SimilarityProvider` capability.
- `v0.48.0` — **Report-over-report diff**: diff consecutive periodic reports section by section with a cited AI delta summary.
- `v0.49.0` — **Feed triage mode and command palette**: keyboard feed triage and a global command palette over search, navigation, and actions.
- `v0.50.0` — **Autonomous report pipeline** (North Star, detailed below): detect publication, auto-fetch, auto-extract, and notify with cross-references, behind a per-company trust ladder.
- `v0.51.0` — **Quality frameworks (qualitative assessment)**: agent-assessed criteria (moat, pricing power, recurring revenue, capital allocation) with citations, composed into the scorecard and re-evaluated by autopilot.
- `v0.52.0` — **Re-invent the notebook panel**.
- `v0.53.0` — **Import/export v2**: unified data bundle and per-feature coverage, including the financial facts + KPI definitions export/import deferred from `v0.37.0`.

### Valuation & decision arc (decision-making augmentor)

These milestones turn the fundamentals substrate into computed valuation and a decision workflow, so the app provides sourced facts **and** computed analysis as decision support. Version numbers below are provisional; the deterministic valuation engine is **pulled forward** (resequenceable, depends only on facts) to land alongside/after the quality-scorecard block (`v0.45.0`), ahead of the clustering/diff/triage milestones — final numbering set at planning approval.

- `v0.54.0` — **Deterministic valuation engine**: a pure-Rust, deterministic valuation slice over confirmed `financial_facts` — DCF/owner-earnings (default for thin GPW peer sets), multiple-based bear/base/bull scenarios where peer sets are deep, an FCF normalization cross-check, peer-relative multiples with a thin-flag, and what-if/sensitivity. Outputs scenario fair values and upside, decision-support framed. Design in [ADR 0041](adr/0041-deterministic-valuation-engine.md).
- `v0.55.0` — **Valuation-aware scoring + advisory-verdict seam**: extend the `v0.45.0` versioned scorecard with a valuation dimension and a scenario/upside readout, and define the open-core `AdvisoryVerdictProvider` port with an **empty default** (decision-support only; no prescriptive output in the open-core build). Design and the open-core boundary in [ADR 0042](adr/0042-advisory-verdict-port-and-open-core-boundary.md).
- `v0.56.0` — **Investment thesis workbench + decision journal**: persisted, provenance-stamped theses (verdict as decision support, scenario forecasts, variant/inversion/disclosed-gaps, valuation↔thesis link with orphan check) and a decision journal (recorded buy/pass + rationale + outcome). Reschedules the previously "Not in V1" trade journal. Design in [ADR 0043](adr/0043-investment-thesis-and-decision-journal.md).
- `v0.57.0` — **Living thesis (newsfeed-as-input)**: link feed items, report documents, signals, and events to theses for staleness state, what-changed diffs, catalyst-aware refresh, and re-score triggers — the differentiator that keeps a thesis fresh from the live feed.
- `v0.58.0` — **Watchlist screener / leaderboard**: batch-run the valuation engine and scorecard across the watchlist into a ranked board with deltas since the last run; extends the report-season cockpit (`v0.43.0`) and cross-company comparison (`v0.44.0`).

Deferred follow-ons (separate ADRs when scheduled): a **SEC EDGAR XBRL fundamentals adapter** for US coverage (new `financial_data` source type; open-core, emits no advice), and an **analyst-consensus adapter** (mostly paid/restricted; behind a flag + ADR) to feed "vs consensus" context.

Sequencing notes: the interpretative AI layer is split into a static foundation (`v0.39.0`, no model) and an embedding-model milestone (`v0.46.0`, lands before story clustering, its first model consumer); the model-backed path is adopted per capability only where a per-capability eval beats the static baseline, and the vector index is disposable so the model is reversible to static — see [ADR 0035](adr/0035-two-layer-ai-and-local-interpretative-layer.md). The quality-frameworks milestones (`v0.45.0` quantitative, `v0.51.0` qualitative) depend only on the fundamentals facts and are resequenceable. The fundamentals schema was validated against ~37 GPW companies across sectors; findings (statement-type packs, generalized unit model, fact variants, period model) are recorded in [ADR 0027](adr/0027-company-fundamentals-scope.md). The valuation & decision arc (`v0.54.0`+) also depends only on facts and is GPW-first/DCF-lean for thin-market reliability; the deterministic valuation engine is pulled forward to land near the quality-scorecard block. The arc stays open-core and decision-support only: any prescriptive (buy/sell/hold) advisory output is supplied by an out-of-band adapter behind the `AdvisoryVerdictProvider` port and is absent from the open-core build, preserving the `AGENTS.md` decision-support rule and the planned recommendation-guardrail enforcement — see [ADR 0042](adr/0042-advisory-verdict-port-and-open-core-boundary.md).

## North Star: Autonomous Report Pipeline (v0.50.0)

The fundamentals, extraction, diff, claims, and cockpit milestones are building blocks toward one experience: a tracked company publishes a periodic report, and the app detects it, fetches it, extracts the figures, summarizes what changed, cross-references the result against open claims, research questions, and evidence, and surfaces a single notification — with no manual steps.

This is deliberately sequenced last (v0.50.0) because it composes everything before it. It introduces a trust ladder rather than changing the confirmation guarantee: confirm-before-commit stays the default, the user opts a specific company into auto-confirm, and auto-committed facts carry a distinct unreviewed provenance state so they stay flagged, reversible, and cited. The financial_facts confirmation model in v0.34.0 is designed so this state is an additive value, not a later migration.

Boundary: fetching and analyzing while the app is closed crosses into a hosted/scheduled service and belongs to the managed-AI paid frontier, not the open core. Autopilot runs while the app is open.

## Future: Release Packaging And Distribution Hardening

Goal: harden distribution after the first public Linux and Windows release artifacts are proven.

Candidate scope:

- Windows installer packaging
- app version/about screen
- optional tag-driven release candidate workflow
- native Pacman packaging if AppImage is not enough for Arch users
- code signing
- package repositories

Not scheduled.

## Future: Full Backup And Restore

Goal: design and implement full local backup and restore beyond the scoped M20 import/export feature.

Candidate scope:

- full app data backup format
- restore safety while the app is running
- clear exclusions for secrets, license tokens, logs, diagnostics, metrics, and private signing material
- compatibility strategy across app versions
- manual recovery documentation

Not scheduled.

## Future: Windows Taskbar Unread Indicator

Goal: make unread Inbox activity visible from the Windows taskbar without opening the app.

Candidate scope:

- small dot-style taskbar indicator when unread feed items exist
- clear the indicator when unread count returns to zero
- route unread state through a small desktop taskbar indicator boundary
- Windows adapter for the real taskbar integration
- no-op adapter for non-Windows and unsupported runtimes
- native Windows packaged-app smoke test covering indicator appearance and clearing

Deferred:

- numeric unread badges
- source-failure, license, or background-job taskbar indicators
- taskbar behavior on non-Windows platforms

Architecture notes:

- The Inbox should publish unread activity state; it should not own Windows taskbar API calls directly.
- The desktop boundary should allow later platform adapters or richer attention states without changing Inbox behavior.
- If Tauri does not expose a suitable Windows taskbar overlay/badge API, a Windows-specific native adapter should be isolated behind the same boundary.

Not scheduled.

## Future Exploration: Terminal Interface

Goal: explore a keyboard-first terminal version after the desktop v1 foundations are stable.

Intent:

- provide a dense TUI experience loosely inspired by `k9s`
- reuse the same local domain/storage contracts as the desktop app
- use the night-neon visual identity in terminal-safe colors
- support fast feed, watchlist, company, and notebook navigation
- make optional synthwave-style background music an explicit opt-in experiment

Not in scope for v1.

## Future Exploration: Mobile And Sync

Goal: explore mobile clients and cross-device sync after local-first desktop workflows are proven.

Intent:

- provide access to watchlists, inbox, notes, claims, and transcripts on mobile devices
- preserve offline-first behavior where practical
- design sync, encryption, conflict resolution, account model, and privacy guarantees before implementation

Not in scope for v1. Cloud backup/sync remains a separate design discussion.

## Future Study: Google Finance Source Value

Goal: determine whether Google Finance can legally and reliably improve the investor workflow before adding it to source implementation scope.

Questions:

- What user value would Google Finance add beyond existing official reports, public/RSS media sources, company registry data, and future AI analysis?
- Is there an official, documented, and permitted access path suitable for a desktop app, or would use depend on fragile/restricted scraping?
- Which data would be useful if permitted: price snapshots, market news, related companies, financial summaries, watchlist enrichment, or company identity matching?
- Does Google Finance coverage improve GPW support enough to justify adapter complexity, or is it more useful for later US/EU market expansion?
- How would attribution, refresh cadence, rate limits, data freshness, and source diagnostics appear in the existing source adapter model?
- Are there better official/public alternatives for the same data with clearer usage terms?

Decision criteria:

- Do not implement unless usage terms and access path are acceptable for local-first desktop use.
- Prefer source-adapter integration only if the data can be fetched through a stable, allowed mechanism and represented with durable attribution.
- If useful only for manual links or user-opened research, treat it as an external-link affordance rather than ingestion.
- If the study recommends implementation, record the source policy and adapter design in Source Strategy or a source-specific ADR before coding.

Not in scope for v1 unless the study identifies a low-risk, permitted, high-value path.

## Future: AI Recommendation Guardrail Enforcement

Goal: add automated post-generation validation that detects and rejects AI output containing buy/sell/hold, portfolio allocation, or similarly actionable recommendation language.

M13 keeps the source-grounded prompt policy and UI positioning as decision support, but hard output enforcement is explicitly deferred until after v1.

Not in scope for v1.

## Future Exploration: Agent / MCP Surface

Goal: expose Brawler's local domain (fundamentals facts, KPIs, valuation runs, scorecards, theses, signals) to an agent through a **local MCP server**, so deep, conversational analysis can be driven over the same data the GUI uses — without making Brawler agent-native or leaving local-first.

Intent:

- Treat the MCP server as a second **inbound adapter** over the existing domain core (alongside the UI ↔ Rust typed-command seam), reusing the same typed contracts — not a new business logic path. See [ADR 0039](adr/0039-ports-and-adapters-posture.md).
- Keep it **local-only** (stdio/loopback); no cloud, no data leaving the machine; strict typed tools mirroring the existing command surface (no arbitrary shell or broad filesystem access).
- Keep the **engine deterministic**: the agent orchestrates and synthesizes; valuation/scoring remain deterministic Rust ([ADR 0041](adr/0041-deterministic-valuation-engine.md)). Agent-as-core-engine is a deliberate non-goal (it would break determinism, testability, cost, and offline operation).
- Inherit the open-core/gating boundary: decision-support tools are open-core; any prescriptive advisory tool is gated by the same `AdvisoryVerdictProvider` seam ([ADR 0042](adr/0042-advisory-verdict-port-and-open-core-boundary.md)).

Sequencing: this only has value once the valuation & decision arc domain exists, so it is a follow-on to that arc, not part of it. Record the surface design and tool contracts in an ADR before implementation.

Not scheduled.

## Future: Cloud Backup And Sync

Cloud backup/sync is not part of core v1 implementation. It is a future roadmap area that requires a separate design discussion and ADR covering identity, encryption, sync conflicts, storage provider, monetization, and cost.

## Not In V1

- portfolio position tracking
- trade journal
- billing/payment infrastructure
- hosted license activation
