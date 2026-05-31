# Roadmap

This roadmap turns the current product and architecture plan into implementation milestones. It is intentionally milestone-based instead of date-based.

See also [Project Brief](project-brief.md), [UI Information Architecture](ui-information-architecture.md), [Data Model](data-model.md), [Source Strategy](source-strategy.md), [Project Practices](project-practices.md), and [Kanban](kanban.md).

## Roadmap Principles

- Build from local-first foundations toward source ingestion and AI.
- Keep every milestone demoable.
- Prefer test-sample-backed workflows before external integrations.
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

Status: complete.

Notes:

- YAML settings import/export/bootstrap is contract-accepted and implementation-deferred to later export/import/backup work.
- Some early Milestone 3 UI pieces were built during Milestone 2, but Milestone 2 closure is based on local storage, settings, sample feed, and command-boundary completion.

Included:

- SQLite migration runner
- initial schema from `docs/data-model.md`
- Rust storage layer
- companies
- watchlists
- settings
- YAML settings import/export/bootstrap contract, with implementation deferred to later export/import/backup work
- early seed or sample feed items for development only, later removed from target runtime initialization in M7
- Tauri commands for companies, watchlists, settings, and feed reads

Exit criteria:

- clean database can be created by migration
- company can be added by exchange-qualified ticker
- watchlist can be created and assigned companies
- feed reads can be shown in Inbox using development/test sample data before real ingestion exists
- migration tests exist
- migration check runs in CI
- runtime settings can be read and updated through Tauri commands
- theme persistence uses SQLite as the runtime source of truth
- YAML settings import/export/bootstrap is explicitly deferred with a follow-up card
- source poll interval editability is tracked as a Settings follow-up, not as GPW adapter closure

## Milestone 3: Inbox And Company Workspace

Goal: make the primary non-AI research workflow usable with local sample data.

Status: complete.

Notes:

- Milestone 3 originally used SQLite-backed local feed items and development seed data. M7 removes sample data from target runtime database initialization; sample data remains test-only.
- Real source ingestion remains deferred to Milestone 5.
- Notebook, Claims, and Transcripts workspace tabs are present as placeholders and move into their dedicated later milestones.
- Manual source refresh remains a disabled placeholder until source adapter jobs exist.

Included:

- Inbox filters
- feed detail pane
- read/unread and saved states
- company workspace with tabs or segmented views
- company Feed tab
- Source Status screen shell

Exit criteria:

- user can review sample feed items
- user can filter by company/watchlist/type/read/saved
- user can open a company page
- user can see source status placeholder
- workflow tests cover daily inbox review

## Milestone 4: Notebooks And Claims

Goal: make durable company research notes useful before external ingestion is complete.

Status: complete in `0.4.0`.

Included:

- Markdown note editor
- notebook list/detail
- create note manually
- create note from feed item
- note origin
- note tags
- claim status
- follow-up quarter and follow-up date
- Claims tab
- cross-company Notebooks screen

Exit criteria:

- note can be created from a feed item and links back to it
- claim note can be followed up and status changed
- notes are searchable or filterable enough for v1 workflows
- storage and UI workflow tests exist

## Milestone 5: GPW ESPI/EBI Listing Adapter

Goal: ingest real official GPW report listings conservatively.

Status: completed.

Included:

- `gpw-espi-ebi` adapter
- listing-level fetch from public GPW report page
- normalization to feed items
- initial ISIN-based company matching, later superseded by ticker-first matching with ISIN fallback
- dedupe
- adapter state
- manual refresh
- in-app scheduled refresh while the desktop UI is open
- source status details
- test-sample-based adapter tests

Exit criteria:

- adapter fetches recent listings
- matched listings appear in Inbox/company Feed
- unmatched items are tracked or diagnosable
- source status shows last success/error
- detail-page fetching remains separate unless explicitly accepted

Completion notes:

- The first GPW ESPI/EBI path fetches the public listing page, parses recent listings, normalizes feed items, deduplicates by source key, and matches tracked companies by ISIN.
- Manual refresh and in-app scheduled refresh are available while the desktop UI is open.
- Sources diagnostics show last attempt, last trigger, last success/error, last result counts, next scheduled refresh, source URL, rate-limit policy, and unmatched listing diagnostics.
- Automated coverage uses test samples/injected fetchers so default checks do not require live GPW availability.
- Poll interval editability remains tracked as a Settings follow-up and is not required for M5 closure.

## Milestone 6: GPW Detail Fetch Spike

Status: completed in `0.6.0`.

Goal: implement a reliable path for reading official GPW report bodies inside the app.

Included:

- detail-page test samples
- parsing spike for report body and attachments
- source policy check
- rate-limit behavior
- decision and implementation path for normal detail-body ingestion

Exit criteria:

- ADR or source-strategy update records the required body-ingestion policy
- parser tests exist
- matched GPW feed items can store and expose report body text from an accepted source path
- if GPW detail parsing is rejected for a specific item, listing-level ingestion remains a temporary fallback and source diagnostics expose the body-fetch failure

Completion notes:

- [ADR 0013](adr/0013-gpw-detail-fetching-policy.md) makes in-app official report body access required for v1 GPW support.
- GPW detail fetching is the primary implementation path under strict constraints.
- Bankier/Parkiet ESPI/EBI RSS feeds may be used as secondary cross-check or fallback signals, but not as replacements for canonical GPW official-source ingestion while GPW remains acceptable.
- Normal ingestion fetches detail bodies for matched GPW items under conservative limits.
- Parsed GPW detail attachments are stored and shown in feed details.
- Source diagnostics expose detail fetch counters and the latest detail warning.
- If GPW detail parsing proves insufficient, fallback body-source investigation is part of M6 rather than a reason to drop the feature.

## Milestone 7: GPW Company Registry Cache

Status: completed in `0.7.0`.

Goal: make company management and source matching reliable by caching GPW ticker/ISIN/company metadata locally.

Included:

- source review for a reliable GPW company list
- SQLite-backed registry cache
- manual registry refresh
- slow scheduled registry refresh, initially daily or weekly
- lookup/autocomplete from the local registry cache
- ticker-first source matching with exact ISIN fallback
- registry freshness and last-error visibility in Sources or Settings

Exit criteria:

- user can add GPW companies from registry search without manually typing all fields
- ticker-managed companies match through registry metadata when a report listing exposes ISIN but the user's company record lacks ISIN
- issuer/company name alone is not used for silent automatic feed matching
- hard-coded lookup test samples are clearly replaced or reduced to tests
- tests cover registry parsing, storage, lookup, and feed matching
- target runtime databases are not seeded from test samples or hard-coded company metadata

Completion notes:

- GPW company metadata is cached in SQLite from the public GPW company list and survives app restarts.
- Sources exposes manual registry refresh, registry freshness, last-error/cache result status, refresh policy, and a searchable cached-company list.
- The desktop UI schedules a slow stale-cache registry refresh check using the registry adapter interval without refreshing immediately on startup.
- Company creation can autocomplete from cached GPW ticker, name, or ISIN input and fill registry metadata without silently overwriting existing companies.
- Tracked companies are searchable in Companies.
- GPW feed matching uses ticker-first matching with exact ISIN-to-registry fallback; issuer/company name alone is not used as a silent match key.
- Target runtime database initialization no longer seeds feed rows or registry rows from sample data.
- Test coverage covers registry parsing, storage, lookup, source matching, stale-refresh behavior, and UI registry autocomplete/search workflows.

## Milestone 8: Polish Media And Research Sources

Goal: ingest company-related news, articles, analysis, and private research sources after official GPW report ingestion is stable.

Included:

- selected public Polish media/news sources where usage is allowed
- RSS or public-feed adapters when available
- Bankier/Parkiet ESPI/EBI RSS adapters as secondary sources if they are useful for cross-checking or missed-item diagnostics
- Bankier market/news RSS adapter candidate after source review
- Investing.com Poland RSS adapter candidate after feed URL and ticker-matching review
- Stooq-style ticker news pages when technically and policy-wise acceptable
- XTB market news and analysis pages if a source review accepts them
- ISBnews or similar providers only after access, paywall, licensing, and attribution rules are understood
- StockWatch/BiznesRadar research candidates only after scraping/paywall/terms review
- Portal Analiz private-account adapter as a v1 source, subject to a dedicated source ADR before implementation

Deferred/enrichment candidates:

- Stooq CSV quote/history endpoints for price context around reports and news.
- Notoria or other structured fundamentals providers as contact-first/commercial options, not scraping assumptions.
- Issuer investor-relations pages for selected companies, especially where they expose RSS, calendars, English reports, or presentations.
- per-source policy notes, rate limits, attribution labels, and source status
- company matching for article/news content using ticker, company name, aliases, and source-specific IDs
- duplicate handling across reports, PAP-derived copies, media rewrites, and syndicated content
- test-sample-backed parser/fetcher tests for every accepted source

Exit criteria:

- at least one non-GPW-report article/news source can ingest matched company items into the Inbox
- source adapters clearly distinguish official reports, public media, analysis, and authenticated private research
- Portal Analiz has a specific ADR covering authentication, local credential storage, user-account scraping posture, rate limits, and implementation boundaries before any scraper is built
- private/account-based sources use OS keychain secrets and never export credentials
- source status makes it clear whether a source is public, RSS-like, authenticated, paywalled, or manually configured
- tests do not require live external services, credentials, or paid accounts

## Milestone 9: Company Events Calendar

Goal: show a cross-watchlist calendar of company events, with upcoming events as the default focus and historical dates available for context.

Included:

- Events screen or panel
- report publication dates
- dividend-related dates when available
- company meetings, conference calls, and other investor-calendar events when available from accepted sources
- company and watchlist filters
- upcoming and historical date ranges
- due-soon grouping
- historical timeline/search mode
- source URL, attribution, and fetched timestamp for sourced events
- manual event entry or correction if official sources are incomplete
- test-sample-backed event data before broad source coverage exists

Exit criteria:

- user can see upcoming dated events for companies in their watchlists by default
- user can switch to historical events or a combined date range
- events can be filtered by watchlist, company, event type, and date range
- event rows show date, company, event type, source, and status
- sourced events retain origin/source attribution
- manual events are clearly distinguishable from sourced events
- storage and UI workflow tests exist

## Milestone 10: YouTube Transcription To Notes

Goal: validate the first video-to-notebook workflow.

Included:

- Gemini provider configuration for YouTube transcription only
- provider disclosure in settings
- transcript job creation
- transcript segment storage
- transcript review UI
- create note from selected transcript segments
- origin to segment and YouTube URL

Exit criteria:

- user can submit YouTube URL for a company
- transcript job status is visible
- user can save selected transcript material as an editable Markdown note
- transcript source text remains immutable
- provider tests use test samples/mocks

## Milestone 11: General AI Analysis Contract Spike

Goal: validate provider-neutral AI analysis without choosing a default provider.

Included:

- provider interface
- prompt/result contract for summaries, tags, significance, reasoning, and source references
- test-sample-backed AI result flow
- UI display in feed detail
- no buy/sell/hold recommendation guardrails

Exit criteria:

- app can display analysis results from sample data or a configured provider
- no general provider is hard-coded as preferred
- AI results preserve source references
- tests cover contract mapping

## Milestone 12: Keyboard Shortcuts And Workflow Polish

Goal: make repeated desktop use faster without making shortcuts the only way to operate the app.

Included:

- app-wide shortcut map
- discoverable shortcut reference in Settings or Help/About
- Inbox shortcuts for navigation, read/unread, save/unsave, opening source, search focus, and refresh
- Company/notebook shortcuts where they reduce repeated work, including `Ctrl+E` to open the editor for the selected note or claim and `Ctrl+S` to save the item currently being edited
- conflict checks with native Windows/browser text-editing shortcuts
- tests for critical shortcut workflows

Exit criteria:

- common daily inbox actions can be performed from the keyboard
- shortcuts are visible/discoverable in the app
- every shortcut action remains available through visible UI controls
- text inputs and editors do not accidentally trigger global shortcuts
- workflow tests cover the most important shortcuts

## Milestone 13: V1 Packaging Candidate

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
- commercial paid data APIs that require redistribution/licensing or product-level billing
- mobile app

## Next Ready Candidates

Recommended next Ready cards:

- GPW detail fetch spike
- Polish media and research source strategy/card breakdown
- Events workspace data model and first screen
- Source poll interval editability in Settings

Do not start GPW detail fetching before the M6 source-policy check confirms that detail-page structure and terms are acceptable.
