# Brawler Project Brief

## Document Map

Use this map to load only the docs needed for the current task.

Core orientation:

- [Architecture](architecture.md): stack, runtime ownership, storage posture, command boundaries, and extensibility boundaries.
- [Modularization Design](modularization-design.md): current module structure and the checklist for keeping future work modular.

Product and UX:

- [Product Spec](product-spec.md): user-facing v1 behavior and deferred scope.
- [UI Flows](ui-flows.md): task flows and interaction sequences.
- [UI Information Architecture](ui-information-architecture.md): screens, navigation, layout, and information hierarchy.
- [UI Authoring Guide](ui-authoring.md): how to build UI — the `src/ui` primitive catalog and the primitive-first authoring contract (read before writing any frontend UI).

Contracts and data:

- [Contracts](contracts.md): stable command payloads and UI-facing read models.
- [Data Model](data-model.md): local entity model, origin model, migrations, and deferred data areas.
- [Source Strategy](source-strategy.md): source adapter policy, accepted source paths, source candidates, and rate-limit posture.
- [AI Analysis Framework](ai-analysis-framework.md): M13 provider-neutral AI analysis design, async job model, settings, and implementation boundaries.
- [Fundamentals Scope (ADR 0027)](adr/0027-company-fundamentals-scope.md): in-scope report-derived KPIs versus the excluded price/market boundary, the fixed KPI taxonomy plus custom per-company KPIs, and the open-core posture for fundamentals AI.

Planning and workflow:

- [Roadmap](roadmap.md): milestone intent and exit criteria.
- [Radicle/Radboard Tracking](kanban.md): pointer to active Radicle issue tracking and board labels.
- [Kanban Archive](kanban-archive.md): completed-card history.
- [Engineering Workflow](engineering-workflow.md): local commands, Nix, WSL/Windows split, CI, the Definition of Done (handover gate), standing operating rules, and packaging posture.
- [Testing](testing.md): testing strategy and layers, and the browser/manual/live/packaging smoke procedures.
- [Release Workflow](release-workflow.md): SemVer policy, Conventional Commits, local commit hooks, git-cliff changelog generation, and retroactive tag policy.
- [Public/Private Documentation Split](adr/0023-public-private-documentation-split.md): public docs versus owner-only operational context.
- [Dependency License Audit](dependency-licenses.md): current dependency-license posture for public-opening work.

Decision records:

- [ADRs](adr/): accepted architecture, source, policy, workflow, and governance decisions. Read only ADRs relevant to the current task.

## Product Intent

Brawler is a personal investor newsfeed application. The first user is an individual investor who follows public companies across multiple markets and wants one place to review important company-specific information.

The first production direction is a local-first Windows desktop app that can later compile for other operating systems and architectures. The app should be built with monetization optionality, but the first version should stay useful for personal use without cloud infrastructure.

## V1 Goal

Build an investor workspace for company news, official reports, and ticker-specific notes:

- maintain multiple watchlists of companies
- pull GPW-focused official reports and selected public/RSS news sources
- show a chronological feed with filters, unread state, source attribution, and company grouping
- maintain a notebook for each ticker
- create notes directly from feed items, transcripts, and selected AI-suggested excerpts
- track management claims or promises across future quarters
- run local ingestion while the desktop app is open
- use provider-neutral AI contracts for summaries, tags, significance labels, video transcription, and note extraction

V1 is not a portfolio tracker, trading tool, or investment recommendation engine.

## Target Markets

V1 prioritizes excellent GPW support. Later adapters should support US and European markets without changing the core feed model.

Initial source priorities:

- GPW ESPI/EBI official reports
- selected Polish public/RSS media sources where usage is allowed
- future adapter candidates: SEC EDGAR APIs, Nasdaq RSS feeds, major European exchange sources

## Product Principles

- Local-first by default.
- Source attribution must be visible and durable.
- Ticker-based UI should stay simple, but storage must avoid ticker collisions.
- Notes must preserve origin so future review can trace a claim back to a report, article, or transcript.
- AI must explain and cite source material, not provide buy/sell recommendations.
- Extensibility is a product architecture principle across the application. Modules should expose clear logical boundaries for future implementations, adapters, providers, collectors, renderers, storage backends, or integration surfaces when a real extension path is plausible.
- Dark theme is the default UI mode, with a user-selectable light theme.
- GitHub Actions should provide fast build and test feedback from the first scaffold.
- GitHub Actions usage should be conservative to avoid paid usage at the start.
- Nix is the day-1 development environment, with WSL2 Ubuntu 24.04 as the primary local dev layer.
- Tests should be lean, behavior-focused, and test-sample-backed for external integrations.
- Secrets live in the OS keychain; YAML config is import/export/bootstrap only.
- The app uses strict Tauri permissions and typed command boundaries from day 1.
- Versioning starts with SemVer-style `0.x.y` releases.
- The architecture should stay modular enough for frequent iteration.

## Open-Core Direction

Brawler uses an open-core posture. The desktop core is open source under the Mozilla Public License 2.0 and should stay useful without payment. Future hosted services, premium integrations, official distribution infrastructure, gated features, or support may be licensed separately.

Detailed owner-only strategy and publication operations belong in the private sibling repository `../brawler-private` when it is available locally. Public docs should avoid personal infrastructure details and speculative monetization experiments.

The local entitlement module remains useful for future gated features and official entitlements, but the open desktop core does not depend on a license token for normal use.
