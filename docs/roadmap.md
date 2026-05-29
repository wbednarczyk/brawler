# Roadmap

This roadmap turns the current product and architecture plan into implementation milestones. It is intentionally milestone-based instead of date-based.

See also [Project Brief](project-brief.md), [UI Information Architecture](ui-information-architecture.md), [Data Model](data-model.md), [Source Strategy](source-strategy.md), [Project Practices](project-practices.md), and [Kanban](kanban.md).

## Roadmap Principles

- Build from local-first foundations toward source ingestion and AI.
- Keep every milestone demoable.
- Prefer fixture-backed workflows before external integrations.
- Do not introduce cloud services in v1.
- Keep contracts and docs updated with each milestone.
- Make local build/test commands the primary interface; GitHub Actions mirrors them.
- Use Nix from the first scaffold for reproducible WSL2 Ubuntu 24.04 development.
- Keep GitHub Actions feedback fast and secret-free by default.
- Minimize GitHub Actions usage while the repo is private: no larger runners, no default macOS CI, no scheduled jobs, and manual packaging until needed.
- Prefer lean, behavior-focused tests over broad brittle suites.
- Keep secrets in the OS keychain and non-secret settings in SQLite.
- Use SemVer-style `0.x.y` versions from the first scaffold.

## Milestone 0: Planning Baseline

Goal: make v1 implementation decision-complete enough to scaffold.

Included:

- project brief
- ADRs for local-first, stack, SQLite, source/AI policy, notebooks/transcripts, and theming
- UX flows
- UI information architecture
- data model
- source strategy
- contracts
- Kanban board

Exit criteria:

- docs are linked from the project brief
- v1 scope and deferred scope are explicit
- first implementation cards are Ready

Status: mostly complete.

## Milestone 1: Desktop Shell And Theme

Goal: create the app skeleton with the chosen stack and visual foundation.

Included:

- Tauri + React + TypeScript scaffold
- `flake.nix` and `flake.lock`
- `nix develop` development shell
- optional `.envrc` for direnv users
- GitHub Actions CI skeleton
- documented local build/test commands
- Rust command boundary
- `health` command
- app shell layout
- dark/light/system theme setting
- dark default
- `night-neon` design tokens
- local settings persistence
- OS keychain integration decision documented for future provider secrets

Exit criteria:

- app starts locally
- app build/test commands work inside `nix develop`
- Makefile exposes thin WSL/Nix wrappers for install, check, test, build, and preview commands
- Windows hands-on sanity testing is documented and supported by a PowerShell helper script
- WSL can trigger the experimental `package-windows-from-linux` flow during the Windows cross-build spike
- UI renders shell with primary navigation
- dark theme is active on first run
- theme can be changed and persists
- basic smoke test exists
- default CI runs without secrets
- default CI uses the same commands as local development or thin wrappers
- default CI validates Nix setup if it remains fast enough
- default CI uses standard Linux runners and avoids heavy packaging jobs
- native Windows click-testing can be run on demand from a Windows checkout or worktree
- packaged Windows app sanity testing direction is named `make package-windows-from-linux`

## Milestone 2: Local Domain And Storage Foundation

Goal: implement the local persistence layer and core domain modules without external source dependencies.

Included:

- SQLite migration runner
- initial schema from `docs/data-model.md`
- Rust storage layer
- companies
- watchlists
- settings
- YAML settings import/export/bootstrap contract
- seed or fixture feed items
- Tauri commands for companies, watchlists, settings, and fixture feed

Exit criteria:

- clean database can be created by migration
- company can be added by exchange-qualified ticker
- watchlist can be created and assigned companies
- fixture feed can be shown in Inbox
- migration tests exist
- migration check runs in CI

## Milestone 3: Inbox And Company Workspace

Goal: make the primary non-AI research workflow usable with local/fixture data.

Included:

- Inbox filters
- feed detail pane
- read/unread and saved states
- company workspace with tabs or segmented views
- company Feed tab
- Source Status screen shell

Exit criteria:

- user can review fixture feed items
- user can filter by company/watchlist/type/read/saved
- user can open a company page
- user can see source status placeholder
- workflow tests cover daily inbox review

## Milestone 4: Notebooks And Claims

Goal: make durable company research notes useful before external ingestion is complete.

Included:

- Markdown note editor
- notebook list/detail
- create note manually
- create note from feed item
- note provenance
- note tags
- claim status
- review quarter and review date
- Claims tab
- cross-company Notebooks screen

Exit criteria:

- note can be created from a feed item and links back to it
- claim note can be reviewed and status changed
- notes are searchable or filterable enough for v1 workflows
- storage and UI workflow tests exist

## Milestone 5: GPW ESPI/EBI Listing Adapter

Goal: ingest real official GPW report listings conservatively.

Included:

- `gpw-espi-ebi` adapter
- listing-level fetch from public GPW report page
- normalization to feed items
- ISIN-based company matching
- dedupe
- adapter state
- manual refresh
- source status details
- fixture-based adapter tests

Exit criteria:

- adapter fetches recent listings
- matched listings appear in Inbox/company Feed
- unmatched items are tracked or diagnosable
- source status shows last success/error
- detail-page fetching remains separate unless explicitly accepted

## Milestone 6: GPW Detail Fetch Spike

Goal: decide whether fetching GPW report detail pages is stable and useful.

Included:

- sample detail-page fixtures
- parsing spike for report body and attachments
- source policy check
- rate-limit behavior
- decision whether to promote detail fetching into normal adapter behavior

Exit criteria:

- ADR or source-strategy update records the decision
- parser tests exist if accepted
- if rejected, listing-level ingestion remains the supported path

## Milestone 7: YouTube Transcription To Notes

Goal: validate the first video-to-notebook workflow.

Included:

- Gemini provider configuration for YouTube transcription only
- provider disclosure in settings
- transcript job creation
- transcript segment storage
- transcript review UI
- create note from selected transcript segments
- provenance to segment and YouTube URL

Exit criteria:

- user can submit YouTube URL for a company
- transcript job status is visible
- user can save selected transcript material as an editable Markdown note
- transcript source text remains immutable
- provider tests use fixtures/mocks

## Milestone 8: General AI Analysis Contract Spike

Goal: validate provider-neutral AI analysis without choosing a default provider.

Included:

- provider interface
- prompt/result contract for summaries, tags, significance, reasoning, and source references
- fixture-backed AI result flow
- UI display in feed detail
- no buy/sell/hold recommendation guardrails

Exit criteria:

- app can display analysis results from fixtures or a configured provider
- no general provider is hard-coded as preferred
- AI results preserve source references
- tests cover contract mapping

## Milestone 9: V1 Packaging Candidate

Goal: produce the first personal-use Windows build candidate.

Included:

- Windows packaging
- GitHub Actions packaging workflow
- local database location decision
- app version/about screen
- basic backup/export consideration
- smoke test checklist
- README quickstart
- import/restore and full local backup

Exit criteria:

- app can be installed or run on Windows
- existing local data survives restart
- primary workflows pass smoke testing
- packaging workflow can be run from GitHub
- packaging workflow is manually triggered unless release automation is explicitly approved
- known limitations are documented

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

## Future: Cloud Backup And Sync

Cloud backup/sync is not part of core v1 implementation. It is a future roadmap area that requires a separate design discussion and ADR covering identity, encryption, sync conflicts, storage provider, monetization, and cost.

## Not In V1

- portfolio position tracking
- trade journal
- billing/licensing UI
- cloud sync
- team collaboration
- hosted ingestion jobs
- paid source integrations
- mobile app

## Next Ready Candidates

Recommended next Ready cards:

- Scaffold desktop application
- Define UI design system tokens
- Design initial SQLite migrations

Do not start GPW ingestion before the local shell, storage, and fixture feed workflow exist.
