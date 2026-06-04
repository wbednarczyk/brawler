# Brawler Project Brief

## Document Map

Use this map to load only the docs needed for the current task.

Core orientation:

- [Project Practices](project-practices.md): standing rules for feature completion, secrets, dependencies, security, AI, testing, releases, and modularity.
- [Architecture](architecture.md): stack, runtime ownership, storage posture, command boundaries, and extensibility boundaries.
- [Modularization Design](modularization-design.md): current module structure and the checklist for keeping future work modular.

Product and UX:

- [Product Spec](product-spec.md): user-facing v1 behavior and deferred scope.
- [UI Flows](ui-flows.md): task flows and interaction sequences.
- [UI Information Architecture](ui-information-architecture.md): screens, navigation, layout, and information hierarchy.

Contracts and data:

- [Contracts](contracts.md): stable command payloads and UI-facing read models.
- [Data Model](data-model.md): local entity model, origin model, migrations, and deferred data areas.
- [Source Strategy](source-strategy.md): source adapter policy, accepted source paths, source candidates, and rate-limit posture.
- [AI Analysis Framework](ai-analysis-framework.md): M13 provider-neutral AI analysis design, async job model, settings, and implementation boundaries.

Planning and workflow:

- [Roadmap](roadmap.md): milestone intent and exit criteria.
- [Kanban](kanban.md): active work only.
- [Kanban Archive](kanban-archive.md): completed-card history.
- [Engineering Workflow](engineering-workflow.md): local commands, Nix, WSL/Windows split, CI, quality gates, and packaging posture.
- [Live Smoke Tests](live-smoke-tests.md): opt-in real-provider validation procedures.

Decision records:

- [ADRs](adr/): accepted architecture, source, policy, workflow, and governance decisions. Read only ADRs relevant to the current task.

## Product Intent

Brawler is a temporary codename for a personal investor newsfeed application. The first user is an individual investor who follows public companies across multiple markets and wants one place to review important company-specific information.

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

## Monetization Direction

The current GitHub repository is private. The future monetization model is undecided. Open core plus paid convenience features remains one candidate, but other approaches are still possible.

Brawler is all rights reserved for now. The exact future license and monetization model must be decided before public release, accepting external contributions, or publishing public release artifacts.

V1 friend-testing distribution is allowed only after the functional v1 work is complete and a local license-key gate exists. The intended friend-test gate should prevent casual redistribution without requiring cloud accounts, telemetry, hosted activation, or a billing system.
