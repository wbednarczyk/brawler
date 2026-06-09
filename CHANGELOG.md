# Changelog

All notable project changes are recorded here.

Historical entries through `0.24.1` were curated from `docs/kanban-archive.md` because the early Git history predates the Conventional Commits policy. Future entries are generated with `git-cliff` from Conventional Commits and may be edited for clarity before release.

## 0.24.1

### Fixed

- Hardened company lookup and company creation so NewConnect and future company-directory sources work through shared registry behavior.
- Generalized company-directory bootstrap, stale checks, media matching, and source-listing matching beyond hard-coded GPW/NewConnect assumptions.

### Tests

- Added regression coverage for future-exchange lookup/create, Companies UI add, watchlists, notebooks, manual events, import/export, media matching, and source-listing matching.

## 0.24.0

### Added

- Added the research/evidence read-model boundary for future Research workspace features.
- Added durable research review checkpoints and typed evidence links.
- Added backend-owned research timeline read models assembled from canonical feed, notebook, event, transcript, and AI-analysis domains.

### Changed

- Recorded the large-file responsibility audit and deferred stored timeline projections until performance or review semantics require them.

## 0.23.0

### Added

- Added opt-in Playwright browser UI smoke tests for layout regressions that jsdom cannot catch.
- Added deterministic browser-smoke data and Make/npm commands for installing and running Chromium-only UI smoke tests.

### Tests

- Covered fixed app chrome, internal scroll regions, Companies list height, Notebooks pane scrolling, Sources compact rows, Watchlists scrolling, and basic navigation.

## 0.22.0

### Added

- Reworked Sources into a normal-user trust and control surface with required, optional, and developer visibility tiers.
- Added optional source enablement/disablement with protection for required and developer-only sources.
- Added NewConnect company-directory support and kept GPW/NewConnect directory lists separated while preserving shared lookup/cache behavior.

### Changed

- Moved unimplemented source candidates and implementation details out of normal UI into Developer Diagnostics and docs.
- Moved company-directory refreshes to the async source-refresh task boundary.
- Added deterministic exchange colors for GPW, NewConnect, and future market prefixes.

## 0.21.0

### Added

- Added portable-only Windows executable candidate packaging.
- Added executable-adjacent data-directory mode for portable app runs.
- Added WSL/native Windows packaging helpers and package smoke documentation.

### Changed

- Built release executables as GUI-subsystem Windows apps so they run without a terminal window.

## 0.20.0

### Added

- Added JSON import/export for research data: companies, watchlists, memberships, and notebook entries.
- Added YAML import/export for allowlisted non-secret settings.
- Added import preview, transactional apply, merge semantics, and file picker behavior.

## 0.19.0

### Added

- Added a dedicated Watchlists panel for creating, renaming, deleting, selecting, and managing watchlist memberships.
- Added backend watchlist rename/delete lifecycle commands.

### Changed

- Removed watchlist mutation controls from Companies while preserving membership context and watchlist filters.
- Added layout and normal-user-copy regression guardrails.

## 0.18.0

### Changed

- Polished Notebooks, Inbox, Sources, Settings, Companies, shell/sidebar, topbar, scrolling, selected rows, and architecture-copy visibility.
- Added shared ticker rendering, app themes, watchlist filters, locale coverage, and focused workflow tests.

## 0.17.0

### Added

- Added the local author/friend-test license gate.
- Added extensible license parsing, verification, entitlement policy, OS keychain storage, redacted metadata, typed commands, UI gate/settings flows, owner tooling, and license operations docs.

## 0.16.0

### Added

- Added local metrics with typed samples, runtime counters, collector registry, on-demand snapshots, and Developer Diagnostics presentation.

### Changed

- Kept collector and presentation/export boundaries ready for future Prometheus, OpenTelemetry, or file adapters without adding remote exposure.

## 0.15.0

### Added

- Added local JSON Lines runtime logging, log directory initialization, configurable rotation, redaction, Settings controls, Developer Diagnostics log viewer, and typed commands.

## 0.14.0

### Added

- Added local Developer mode diagnostics with persisted mode, diagnostics storage, redaction, retention, typed commands, Diagnostics UI, and first AI/source/credential diagnostic producers.

## 0.13.0

### Added

- Added provider-neutral AI analysis architecture, contracts, storage, settings, async job runtime, typed commands, and frontend API.
- Added deterministic test-sample analysis provider and Gemini as the first live general-analysis provider.
- Added feed-detail AI analysis UI with prompt presets, custom questions, async state, retry, metadata, reasoning, tags, and source references.
- Added opt-in live Gemini feed-item analysis smoke path.

## 0.12.0

### Added

- Added English/Polish locale workflow with English as the first-run default.
- Added configurable app, Inbox, Company, and notebook shortcut actions with Settings discoverability, persistence, reset, disable, and conflict warnings.

### Changed

- Recorded the standing rule that new or changed repeated user actions should be evaluated for shortcut support.

## 0.11.0

### Changed

- Completed the broad modularization pass by extracting frontend API boundaries and major screen modules out of the app shell.
- Preserved existing Inbox, Companies, Transcripts, Settings, and workflow behavior while reducing large-file responsibility.

## 0.10.0

### Added

- Added the YouTube-to-transcript-to-notebook workflow backed by Gemini.
- Added transcript job storage, immutable transcript segments, typed transcript commands, URL-first transcript UI, segment review, segment selection, and editable note creation from selected transcript material.
- Added Gemini credential settings, model selection, timeout settings, provider disclosure, OS-keychain credential storage, and opt-in live smoke tests.

### Changed

- Promoted real Gemini execution for transcript jobs while keeping test-sample providers for automated tests and development.

## 0.9.0

### Added

- Added company events storage, typed commands, Events navigation, upcoming/week/list views, filters, manual event creation, and source-backed event ingestion.
- Added GPW Market Events RSS and Bankier Kalendarium ingestion for tracked company events.

## 0.8.0

### Added

- Added Bankier Gielda RSS as the first public media/news adapter.
- Added Bankier per-company komunikaty as the active v1 official-report adapter for tracked GPW companies.
- Added company source identifiers, source status distinctions, disabled reviewed candidates, and feed cleanup controls.

### Changed

- Kept GPW ESPI/EBI registered but disabled until a later reliability pass.
- Documented Portal Analiz as a late-v1 authenticated research candidate.

## 0.7.0

### Added

- Added the GPW company registry cache for lookup, autocomplete, registry refresh, and ticker-first source matching.
- Added Sources registry detail, cached-company search, tracked/untracked state, and add actions.

### Changed

- Removed target-runtime sample registry/feed seed data.
- Added slow in-app stale-cache registry refresh behavior.

## 0.6.0

### Added

- Added GPW detail-page fetching for matched official report bodies and attachments.
- Added parser tests, detail usability warnings, detail fetch counters, attachment storage, and source status detail warnings.

### Changed

- Recorded GPW detail fetching as the primary in-app official report body path, with Bankier/Parkiet/PAP as fallback or cross-check candidates.

## 0.5.0

### Added

- Added the first GPW ESPI/EBI listing adapter, normalized listing parser, source ingestion, manual refresh, scheduler behavior, source status, unmatched diagnostics, and source policy visibility.

## 0.4.0

### Added

- Added durable company notebooks, Markdown notes, note editing, tags, note kinds, claim status, follow-up fields, note origins, feed-to-note drafts, and claim views.
- Added the cross-company Notebooks screen and claim-oriented filtering.

## 0.3.0

### Added

- Added the Inbox and Company Workspace using local persisted feed items.
- Added Inbox filters, read/saved state, source details, empty states, company workspace navigation, company feed details, source status, and topbar refresh/status wiring.

## 0.2.0

### Added

- Added local SQLite storage foundations for companies, watchlists, settings, feed items, source records, notebooks, transcripts, jobs, and settings.
- Added settings storage commands, Settings screen basics, test-sample-backed company lookup, basic watchlists, and migration tests.

## 0.1.0

### Added

- Added the Tauri, React, TypeScript, Rust, Nix, and Makefile desktop shell foundation.
- Added app shell layout, dark/light theme selection, initial visual tokens, health command, local build/test commands, Windows sanity helpers, and CI skeleton.

## 0.0.0

### Added

- Added the spec-driven planning baseline: project brief, product scope, architecture, ADRs, UI flows, information architecture, contracts, data model, source strategy, roadmap, and agent contract.
