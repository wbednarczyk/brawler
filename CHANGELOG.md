# Changelog

## v - 2026-06-16



### Features

- management claims tracker (v0.42.0)


## v - 2026-06-16



### Changed

- enforce ErrorText, add primitive/a11y tests, extract AI-analysis controller


## v0.41.1 - 2026-06-16



### Features

- adopt and enforce the primitive-first component framework


## v0.41.0 - 2026-06-16



### Features

- report document ingestion, history backfill, and derived events (v0.41.0)


## v0.40.0 - 2026-06-15



### Features

- typed ESPI/EBI event classification


## v0.39.0 - 2026-06-15



### Features

- add interpretative AI layer static foundation


## v0.38.0 - 2026-06-14



### Features

- global search and database improvements


## v0.37.0 - 2026-06-14



### Features

- v0.37 panel, charts, as-reported formatting, and review modal

- fundamentals panel UI, KPI extraction flow, detail-rail refactor, and assertion-driven UI test harness


## v0.36.0 - 2026-06-13



### Bug Fixes

- normalize report MIME type and surface extraction errors

- make KPI extraction thorough and lift output token cap

- reset KPI extraction panel per feed item

- diagnose extraction completeness and tighten period detection

- exclude derived KPIs from extraction and improve panel clarity



### Features

- add KPI extraction contracts and prompt boundary

- add native PDF document input to the Claude adapter

- build AI KPI extraction job with confirm/reject staging

- add per-company IR reports-page URL field

- add AI-assisted IR-page report resolver

- add frontend API for KPI extraction and IR resolution

- add KPI extraction review UI and IR reports-page field


## v0.35.0 - 2026-06-13



### Changed

- migrate AI analysis and transcription layer to async

- add provider registries replacing duplicated dispatch

- one key per provider, generic provider_id-keyed commands



### Features

- per-provider model registry and multi-provider defaults (migration 0036)

- catalog-driven AI provider and model selection

- shared analysis prompts/parsing and Claude (Anthropic) adapter

- OpenAI (ChatGPT) analysis adapter

- document-input abstraction with Gemini native and capability flags

- per-provider API key entry for Claude and OpenAI


## v0.34.0 - 2026-06-13



### Features

- add financial facts schema (migration 0034)

- add financials storage, commands, and DTOs

- persist report documents and capture from URLs

- add manual KPI entry/edit workflow


## v0.33.0 - 2026-06-12



### Changed

- modularize import export backend

- split workspace screen panels

- split screen styling

- extract feed company matching boundary

- extract shared AI response helpers

- extract parsing helpers

- split translation resources

- modularize app workflow harness


## v0.32.0 - 2026-06-12



### Changed

- introduce shared UI primitives

- expand shared UI primitive foundation


## v0.31.1 - 2026-06-12



### Bug Fixes

- polish settings navigation and AI options


## v0.31.0 - 2026-06-12



### Features

- add event-aware reminders and AI digest workspace


## v0.30.0 - 2026-06-11



### Features

- add AI briefs and improve research question workflow


## v0.29.0 - 2026-06-11



### Features

- add questions and evidence links


## v0.28.8 - 2026-06-11



### Bug Fixes

- avoid weak substring matches for media companies


## v0.28.7 - 2026-06-11



### Bug Fixes

- isolate Nix wrappers from inherited library paths for local windows build

- prevent Inbox layout collapse on narrow desktop windows


## v0.28.6 - 2026-06-11



### Bug Fixes

- repair WSL Linux startup and artifact collection


## v0.28.5 - 2026-06-10



### Bug Fixes

- publish changelog entries as GitHub release notes


## v0.28.4 - 2026-06-10



### Bug Fixes

- install minimal AppImage prerequisites


## v0.28.3 - 2026-06-10



### Bug Fixes

- install AppImage runtime tools


## v0.28.1 - 2026-06-10



### Bug Fixes

- allow AppImage packaging without FUSE


## v0.28.0 - 2026-06-10



### Features

- add cross-platform release artifacts


## v0.27.0 - 2026-06-10



### Changed

- prepare repository for open-core publication


## v0.26.0 - 2026-06-10

### Added

- Added watchlist review mode to the Research screen with a Company/Watchlist mode switch.
- Added backend-owned watchlist evidence summaries with member-company review queue counts.
- Added an explicit cascade option when marking a watchlist reviewed so member companies are only marked reviewed by user choice.

### Fixed

- Added the missing notebook note deletion action.

### Tests

- Added regression coverage for watchlist review mode, review cascade behavior, and notebook note deletion.


All notable project changes are recorded here.

Historical entries through `0.24.1` were curated from `docs/kanban-archive.md` because the early Git history predates the Conventional Commits policy. Future entries are generated with `git-cliff` from Conventional Commits and may be edited for clarity before release.

## v0.25.0 - 2026-06-09

### Added

- Added the first visible Research workspace screen with company-scoped evidence timelines.
- Added backend Research timeline summaries with total evidence, changed-since-review counts, and last-reviewed timestamps.
- Added backend-owned Research filters for evidence type and changed-only views.
- Added company-level Research review checkpoints with a `Mark reviewed` workflow.
- Added SemVer, Conventional Commit, git-cliff changelog, and release validation workflow documentation and scripts.

### Changed

- Rendered Research evidence rows with product-language labels, compact density, and owning-domain/source actions.
- Reworked the future Research roadmap so watchlist review, questions, briefs, reminders, and optional stored projections build on the completed company timeline.

### Fixed

- Fixed Research feed-item navigation so opening evidence lands in Inbox with the selected item still visible.
- Hid raw internal event/provider identifiers from normal Research UI, including event codes and AI provider ids.

### Tests

- Added regression coverage for Research navigation, backend filtering, review summaries, feed-item opening, and normal-user copy leaks.
- Added release workflow guardrails for commit message validation, version synchronization, and changelog generation.

## v0.24.1

### Fixed

- Hardened company lookup and company creation so NewConnect and future company-directory sources work through shared registry behavior.
- Generalized company-directory bootstrap, stale checks, media matching, and source-listing matching beyond hard-coded GPW/NewConnect assumptions.

### Tests

- Added regression coverage for future-exchange lookup/create, Companies UI add, watchlists, notebooks, manual events, import/export, media matching, and source-listing matching.

## v0.24.0

### Added

- Added the research/evidence read-model boundary for future Research workspace features.
- Added durable research review checkpoints and typed evidence links.
- Added backend-owned research timeline read models assembled from canonical feed, notebook, event, transcript, and AI-analysis domains.

### Changed

- Recorded the large-file responsibility audit and deferred stored timeline projections until performance or review semantics require them.

## v0.23.0

### Added

- Added opt-in Playwright browser UI smoke tests for layout regressions that jsdom cannot catch.
- Added deterministic browser-smoke data and Make/npm commands for installing and running Chromium-only UI smoke tests.

### Tests

- Covered fixed app chrome, internal scroll regions, Companies list height, Notebooks pane scrolling, Sources compact rows, Watchlists scrolling, and basic navigation.

## v0.22.0

### Added

- Reworked Sources into a normal-user trust and control surface with required, optional, and developer visibility tiers.
- Added optional source enablement/disablement with protection for required and developer-only sources.
- Added NewConnect company-directory support and kept GPW/NewConnect directory lists separated while preserving shared lookup/cache behavior.

### Changed

- Moved unimplemented source candidates and implementation details out of normal UI into Developer Diagnostics and docs.
- Moved company-directory refreshes to the async source-refresh task boundary.
- Added deterministic exchange colors for GPW, NewConnect, and future market prefixes.

## v0.21.0

### Added

- Added portable-only Windows executable candidate packaging.
- Added executable-adjacent data-directory mode for portable app runs.
- Added WSL/native Windows packaging helpers and package smoke documentation.

### Changed

- Built release executables as GUI-subsystem Windows apps so they run without a terminal window.

## v0.20.0

### Added

- Added JSON import/export for research data: companies, watchlists, memberships, and notebook entries.
- Added YAML import/export for allowlisted non-secret settings.
- Added import preview, transactional apply, merge semantics, and file picker behavior.

## v0.19.0

### Added

- Added a dedicated Watchlists panel for creating, renaming, deleting, selecting, and managing watchlist memberships.
- Added backend watchlist rename/delete lifecycle commands.

### Changed

- Removed watchlist mutation controls from Companies while preserving membership context and watchlist filters.
- Added layout and normal-user-copy regression guardrails.

## v0.18.0

### Changed

- Polished Notebooks, Inbox, Sources, Settings, Companies, shell/sidebar, topbar, scrolling, selected rows, and architecture-copy visibility.
- Added shared ticker rendering, app themes, watchlist filters, locale coverage, and focused workflow tests.

## v0.17.0

### Added

- Added the local author/friend-test license gate.
- Added extensible license parsing, verification, entitlement policy, OS keychain storage, redacted metadata, typed commands, UI gate/settings flows, owner tooling, and license operations docs.

## v0.16.0

### Added

- Added local metrics with typed samples, runtime counters, collector registry, on-demand snapshots, and Developer Diagnostics presentation.

### Changed

- Kept collector and presentation/export boundaries ready for future Prometheus, OpenTelemetry, or file adapters without adding remote exposure.

## v0.15.0

### Added

- Added local JSON Lines runtime logging, log directory initialization, configurable rotation, redaction, Settings controls, Developer Diagnostics log viewer, and typed commands.

## v0.14.0

### Added

- Added local Developer mode diagnostics with persisted mode, diagnostics storage, redaction, retention, typed commands, Diagnostics UI, and first AI/source/credential diagnostic producers.

## v0.13.0

### Added

- Added provider-neutral AI analysis architecture, contracts, storage, settings, async job runtime, typed commands, and frontend API.
- Added deterministic test-sample analysis provider and Gemini as the first live general-analysis provider.
- Added feed-detail AI analysis UI with prompt presets, custom questions, async state, retry, metadata, reasoning, tags, and source references.
- Added opt-in live Gemini feed-item analysis smoke path.

## v0.12.0

### Added

- Added English/Polish locale workflow with English as the first-run default.
- Added configurable app, Inbox, Company, and notebook shortcut actions with Settings discoverability, persistence, reset, disable, and conflict warnings.

### Changed

- Recorded the standing rule that new or changed repeated user actions should be evaluated for shortcut support.

## v0.11.0

### Changed

- Completed the broad modularization pass by extracting frontend API boundaries and major screen modules out of the app shell.
- Preserved existing Inbox, Companies, Transcripts, Settings, and workflow behavior while reducing large-file responsibility.

## v0.10.0

### Added

- Added the YouTube-to-transcript-to-notebook workflow backed by Gemini.
- Added transcript job storage, immutable transcript segments, typed transcript commands, URL-first transcript UI, segment review, segment selection, and editable note creation from selected transcript material.
- Added Gemini credential settings, model selection, timeout settings, provider disclosure, OS-keychain credential storage, and opt-in live smoke tests.

### Changed

- Promoted real Gemini execution for transcript jobs while keeping test-sample providers for automated tests and development.

## v0.9.0

### Added

- Added company events storage, typed commands, Events navigation, upcoming/week/list views, filters, manual event creation, and source-backed event ingestion.
- Added GPW Market Events RSS and Bankier Kalendarium ingestion for tracked company events.

## v0.8.0

### Added

- Added Bankier Gielda RSS as the first public media/news adapter.
- Added Bankier per-company komunikaty as the active v1 official-report adapter for tracked GPW companies.
- Added company source identifiers, source status distinctions, disabled reviewed candidates, and feed cleanup controls.

### Changed

- Kept GPW ESPI/EBI registered but disabled until a later reliability pass.
- Documented Portal Analiz as a late-v1 authenticated research candidate.

## v0.7.0

### Added

- Added the GPW company registry cache for lookup, autocomplete, registry refresh, and ticker-first source matching.
- Added Sources registry detail, cached-company search, tracked/untracked state, and add actions.

### Changed

- Removed target-runtime sample registry/feed seed data.
- Added slow in-app stale-cache registry refresh behavior.

## v0.6.0

### Added

- Added GPW detail-page fetching for matched official report bodies and attachments.
- Added parser tests, detail usability warnings, detail fetch counters, attachment storage, and source status detail warnings.

### Changed

- Recorded GPW detail fetching as the primary in-app official report body path, with Bankier/Parkiet/PAP as fallback or cross-check candidates.

## v0.5.0

### Added

- Added the first GPW ESPI/EBI listing adapter, normalized listing parser, source ingestion, manual refresh, scheduler behavior, source status, unmatched diagnostics, and source policy visibility.

## v0.4.0

### Added

- Added durable company notebooks, Markdown notes, note editing, tags, note kinds, claim status, follow-up fields, note origins, feed-to-note drafts, and claim views.
- Added the cross-company Notebooks screen and claim-oriented filtering.

## v0.3.0

### Added

- Added the Inbox and Company Workspace using local persisted feed items.
- Added Inbox filters, read/saved state, source details, empty states, company workspace navigation, company feed details, source status, and topbar refresh/status wiring.

## v0.2.0

### Added

- Added local SQLite storage foundations for companies, watchlists, settings, feed items, source records, notebooks, transcripts, jobs, and settings.
- Added settings storage commands, Settings screen basics, test-sample-backed company lookup, basic watchlists, and migration tests.

## v0.1.0

### Added

- Added the Tauri, React, TypeScript, Rust, Nix, and Makefile desktop shell foundation.
- Added app shell layout, dark/light theme selection, initial visual tokens, health command, local build/test commands, Windows sanity helpers, and CI skeleton.

## v0.0.0

### Added

- Added the spec-driven planning baseline: project brief, product scope, architecture, ADRs, UI flows, information architecture, contracts, data model, source strategy, roadmap, and agent contract.
