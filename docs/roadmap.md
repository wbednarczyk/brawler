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

## Active And Upcoming Milestones

The next milestone is `v0.39.0`. This is the forward plan (milestone intent only; live epic/task status and IDs are in Radicle/Radboard, see [kanban.md](kanban.md)):

- `v0.39.0` — **Typed ESPI event classification**: classify ESPI/EBI filings into typed company events (insider transactions, dividends, profit warnings, contracts, buybacks). Also delivers the ESPI/EBI attachment ingestion and on-track company history backfill deferred from `v0.34.0`.
- `v0.40.0` — **Management claims tracker**: track management claims from reports and transcripts with due periods, verdicts, and KPI-backed verification.
- `v0.41.0` — **Report-season cockpit**: upcoming report dates with pre-report cards built from questions, claims, KPIs, and evidence.
- `v0.42.0` — **Cross-company KPI comparison**: side-by-side tables and multi-series trend charts.
- `v0.43.0` — **Quality frameworks (quantitative checks)**: a rule engine evaluates user frameworks against the fundamentals facts and produces a versioned scorecard; ships clonable templates including a Kroeze-style quality template. Depends only on facts (`v0.37.0`); resequenceable.
- `v0.44.0` — **Story clustering across sources**: cluster near-duplicate multi-source coverage into single stories with the official source ranked first.
- `v0.45.0` — **Report-over-report diff**: diff consecutive periodic reports section by section with a cited AI delta summary.
- `v0.46.0` — **Feed triage mode and command palette**: keyboard feed triage and a global command palette over search, navigation, and actions.
- `v0.47.0` — **Autonomous report pipeline** (North Star, detailed below): detect publication, auto-fetch, auto-extract, and notify with cross-references, behind a per-company trust ladder.
- `v0.48.0` — **Quality frameworks (qualitative assessment)**: agent-assessed criteria (moat, pricing power, recurring revenue, capital allocation) with citations, composed into the scorecard and re-evaluated by autopilot.
- `v0.49.0` — **Re-invent the notebook panel**.
- `v0.50.0` — **Import/export v2**: unified data bundle and per-feature coverage, including the financial facts + KPI definitions export/import deferred from `v0.37.0`.

Sequencing notes: the quality-frameworks milestones (`v0.43.0` quantitative, `v0.48.0` qualitative) depend only on the fundamentals facts and are resequenceable. The fundamentals schema was validated against ~37 GPW companies across sectors; findings (statement-type packs, generalized unit model, fact variants, period model) are recorded in [ADR 0027](adr/0027-company-fundamentals-scope.md).

## North Star: Autonomous Report Pipeline (v0.47.0)

The fundamentals, extraction, diff, claims, and cockpit milestones are building blocks toward one experience: a tracked company publishes a periodic report, and the app detects it, fetches it, extracts the figures, summarizes what changed, cross-references the result against open claims, research questions, and evidence, and surfaces a single notification — with no manual steps.

This is deliberately sequenced last (v0.47.0) because it composes everything before it. It introduces a trust ladder rather than changing the confirmation guarantee: confirm-before-commit stays the default, the user opts a specific company into auto-confirm, and auto-committed facts carry a distinct unreviewed provenance state so they stay flagged, reversible, and cited. The financial_facts confirmation model in v0.34.0 is designed so this state is an additive value, not a later migration.

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

## Future: Cloud Backup And Sync

Cloud backup/sync is not part of core v1 implementation. It is a future roadmap area that requires a separate design discussion and ADR covering identity, encryption, sync conflicts, storage provider, monetization, and cost.

## Not In V1

- portfolio position tracking
- trade journal
- billing/payment infrastructure
- hosted license activation
