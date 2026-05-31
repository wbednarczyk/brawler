# Brawler Project Brief

## Document Map

- [Architecture](architecture.md)
- [Product Spec](product-spec.md)
- [UI Flows](ui-flows.md)
- [UI Information Architecture](ui-information-architecture.md)
- [Data Model](data-model.md)
- [Source Strategy](source-strategy.md)
- [Roadmap](roadmap.md)
- [Engineering Workflow](engineering-workflow.md)
- [Project Practices](project-practices.md)
- [Contracts](contracts.md)
- [Kanban](kanban.md)
- [ADR 0001: Local-First Desktop Application](adr/0001-local-first-desktop.md)
- [ADR 0002: Tauri, React, and Rust Core](adr/0002-tauri-react-rust-core.md)
- [ADR 0003: SQLite Local Storage](adr/0003-sqlite-local-storage.md)
- [ADR 0004: Source and AI Policy](adr/0004-source-and-ai-policy.md)
- [ADR 0005: Company Notebooks and Transcripts](adr/0005-company-notebooks-and-transcripts.md)
- [ADR 0006: Theme and Visual Direction](adr/0006-theme-and-visual-direction.md)
- [ADR 0007: GitHub Build and Lean Testing](adr/0007-github-build-and-lean-testing.md)
- [ADR 0008: License and Project Governance](adr/0008-license-and-project-governance.md)
- [ADR 0009: Local Config, Secrets, Data, and Observability](adr/0009-local-config-secrets-data-and-observability.md)
- [ADR 0010: Security, Dependencies, and AI Policy](adr/0010-security-dependencies-and-ai-policy.md)
- [ADR 0011: Export, Backup, Versioning, and Releases](adr/0011-export-backup-versioning-and-releases.md)
- [ADR 0012: Nix Development Environment](adr/0012-nix-development-environment.md)

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

Brawler is all rights reserved for now. The exact future license and monetization model must be decided before public release, accepting external contributions, or publishing release artifacts.
