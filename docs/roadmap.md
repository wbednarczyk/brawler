# Roadmap

This roadmap turns the current product and architecture plan into implementation milestones. It is intentionally milestone-based instead of date-based.

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
- Source poll interval editability was delivered later in M8 Settings work and was not required for M5 closure.

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

Status: completed in `0.8.0`.

Goal: ingest company-related news, articles, analysis, and private research sources after official GPW report ingestion is stable.

Included:

- selected public Polish media/news sources where usage is allowed
- RSS or public-feed adapters when available
- Bankier/Parkiet ESPI/EBI RSS adapters as secondary sources if they are useful for cross-checking or missed-item diagnostics
- Bankier Company Komunikaty is currently the active v1 GPW official-report source; `gpw-espi-ebi` remains disabled until a later reliability pass proves it should be re-enabled.
- Bankier market/news RSS adapter candidate after source review
- Investing.com Poland RSS adapter candidate after feed URL and ticker-matching review
- Stooq-style ticker news pages when technically and policy-wise acceptable
- XTB market news and analysis pages if a source review accepts them
- ISBnews or similar providers only after access, paywall, licensing, and attribution rules are understood
- StockWatch/BiznesRadar research candidates only after scraping/paywall/terms review
- Portal Analiz private-account adapter as a v1 source candidate governed by [ADR 0014](adr/0014-portal-analiz-authenticated-source-policy.md) before implementation

Deferred/enrichment candidates:

- Stooq CSV quote/history endpoints for price context around reports and news.
- Notoria or other structured fundamentals providers as contact-first/commercial options, not scraping assumptions.
- Issuer investor-relations pages for selected companies, especially where they expose RSS, calendars, English reports, or presentations.
- per-source policy notes, rate limits, attribution labels, and source status
- company matching for article/news content using ticker, company name, aliases, and source-specific IDs
- duplicate handling across reports, PAP-derived copies, media rewrites, and syndicated content
- test-sample-backed parser/fetcher tests for every accepted source
- feed pruning configuration in Settings or Sources, including last run and whether scheduled cleanup is enabled; Settings currently exposes cleanup status, retention window, interval, and protected saved items

Exit criteria:

- at least one non-GPW-report article/news source can ingest matched company items into the Inbox
- source adapters clearly distinguish official reports, public media, analysis, and authenticated private research
- Portal Analiz has a specific ADR covering authentication, local credential storage, user-account access posture, rate limits, and implementation boundaries before any scraper is built
- private/account-based sources use OS keychain secrets and never export credentials
- source status makes it clear whether a source is public, RSS-like, authenticated, paywalled, or manually configured
- tests do not require live external services, credentials, or paid accounts

## Milestone 9: Company Events Calendar

Status: completed in `0.9.0`.

Goal: show a cross-watchlist calendar of company events, with upcoming events as the default focus and historical dates available for context.

Included:

- Events screen or panel with a current-week default view
- working-day columns in the default week view
- previous/next/current week navigation
- report publication dates
- dividend-related dates when available
- company meetings, conference calls, and other investor-calendar events when available from accepted sources
- company and watchlist filters
- secondary list view with upcoming, historical, combined, and custom date ranges
- due-soon grouping
- historical timeline/search mode
- source URL, attribution, and fetched timestamp for sourced events
- manual event entry if official sources are incomplete
- test-sample-backed event data before broad source coverage exists

Exit criteria:

- user can see current-week dated events for companies in their watchlists by default
- user can switch to the list view for upcoming events, historical events, combined date ranges, or custom date ranges
- events can be filtered by watchlist, company, event type, and date range
- event rows show date, company, event type, source, and status
- sourced events retain origin/source attribution
- manual events are clearly distinguishable from sourced events
- storage and UI workflow tests exist

## Milestone 10: YouTube Transcription To Notes

Goal: validate the first video-to-notebook workflow with real Gemini-backed YouTube transcript generation.

Status: completed in `0.10.0`.

Included:

- Gemini provider configuration for YouTube transcription only, including OS-keychain credential storage
- provider disclosure in settings
- transcript job creation
- transcript segment storage
- transcript review UI
- create note from selected transcript segments
- origin to segment and YouTube URL
- live `provider_gemini` execution against the real Gemini API for supported public YouTube URLs
- offline test-sample provider for automated tests and development only

Exit criteria:

- user can submit a YouTube URL with or without a company
- a configured real Gemini API key can generate transcript segments from a real supported YouTube URL
- transcript job status is visible
- user can save selected transcript material as an editable Markdown note
- unlinked transcripts remain visible and can be linked to a company later when notebook note creation is needed
- transcript source text remains immutable
- Gemini use, preview/provider limitations, and privacy implications are disclosed before use
- missing credentials, provider limits, network errors, and provider errors are visible as recoverable job failures
- at least one manual live Gemini smoke check passes before M10 is closed
- default automated tests and CI use test samples/mocks and do not require Gemini credentials or live external services

## Milestone 11: Modularization

Status: completed as part of the `0.12.0` branch history.

Goal: complete the broad modularization pass and turn modularity into a continuous development rule.

Included:

- frontend API, app, screen, shared, style, and test module boundaries
- Rust command, storage, provider, job, and app-state module boundaries
- shared frontend primitives adopted where they preserve behavior and class semantics
- storage tests split by domain
- completed-card Kanban history moved out of active context
- modularization design converted from extraction plan to ongoing architecture guide

Exit criteria:

- original monolithic frontend and Rust files no longer carry mixed responsibilities
- remaining large files are intentional state roots, facades, composition points, or cohesive domain views
- future feature work has a documented modularity checklist
- active documentation context is optimized for future agents

## Milestone 12: Keyboard Shortcuts, Locale, And Workflow Polish

Status: completed in `0.12.0`.

Goal: make repeated desktop use faster and add an extensible app-locale framework while keeping English as the default.

Included:

- app-wide shortcut map
- discoverable shortcut reference in Settings or Help/About
- configurable shortcut bindings for all defined shortcut actions
- Inbox shortcuts for navigation, read/unread, save/unsave, opening source, search focus, and refresh
- Company/notebook shortcuts where they reduce repeated work, including `Ctrl+E` to open the editor for the selected note or claim and `Ctrl+S` to save the item currently being edited
- conflict checks with native Windows/browser text-editing shortcuts
- extensible locale setting in Settings
- English default locale
- Polish locale as the first additional language
- locale resource structure that can add future languages without rewriting screens
- localized static UI copy for the main app shell and implemented screens
- localized formatting for app-owned labels where applicable, without changing stored source text, company names, ticker symbols, URLs, or provider/source attribution
- tests for critical shortcut workflows

Exit criteria:

- common daily inbox actions can be performed from the keyboard
- shortcuts are visible/discoverable in the app
- shortcuts can be configured, disabled, and reset without changing code
- every shortcut action remains available through visible UI controls
- text inputs and editors do not accidentally trigger global shortcuts
- user can switch between English and Polish from Settings
- English remains the first-run default
- language choice persists in SQLite settings
- the locale implementation can add future supported locales through locale resources/configuration instead of per-screen rewrites
- source-provided text remains in its original language
- workflow tests cover the most important shortcuts

Completion notes:

- Added an extensible SQLite-backed locale setting with English default and Polish as the first additional language.
- Added shared locale resources and wired app-owned static copy across the implemented app shell and screens without translating source, company, transcript, notebook, URL, or attribution content.
- Added a configurable shortcut framework with Settings discoverability, per-action enablement, reset, persistence, and conflict warnings.
- Added app, Inbox, Company, and notebook workflow shortcuts while preserving visible UI controls.
- Recorded the ongoing development rule that future feature work must evaluate whether changed user actions should be shortcut actions.
- Verified frontend typecheck, full frontend tests, frontend build, Rust format, Rust clippy, and Rust tests during milestone closure.

## Milestone 13: General AI Analysis Framework

Status: completed in `0.13.0`.

Goal: add source-grounded AI analysis using the existing Gemini implementation first while keeping the framework extensible for ChatGPT, Claude, and other future providers.

Included:

- provider-neutral AI analysis interface
- Gemini-backed general analysis implementation as the first live provider path
- provider/model/credential boundaries that can support OpenAI, Anthropic, and other future providers without rewiring the UI
- asynchronous AI analysis job model so provider calls do not block the UI
- prompt/result contract for summaries, tags, significance, reasoning, and source references
- test-sample-backed and mocked AI result flow
- UI display in feed detail
- Settings configuration for general AI analysis provider, model, timeout, and provider disclosure
- source-grounded prompt policy that excludes buy/sell/hold and portfolio advice; automated post-generation recommendation-language enforcement is deferred after v1

Exit criteria:

- app can display source-grounded AI analysis results from Gemini or deterministic test samples
- Gemini is the first implementation, but the contract does not hard-code Gemini as the only future provider
- provider credentials remain under the reusable OS-keychain credential boundary
- analysis runs as an async job with visible queued/running/succeeded/failed states
- analysis starts from an explicit visible user action
- Settings exposes general AI analysis configuration and provider disclosure
- AI results preserve source references
- tests cover storage, job flow, contract mapping, provider mapping, and UI display
- default automated tests do not require live external services or secrets

## Milestone 14: Developer Mode And Diagnostics Framework

Status: completed in `0.14.0`.

Goal: add an extensible local developer mode that lets trusted users inspect what the app is doing across modules without adding telemetry or exposing secrets.

Included:

- Developer mode setting, default off, enabled only through an intentional local developer mechanism such as an environment variable or local dev configuration
- hidden runtime author unlock for enabling Developer mode after the app is already running when a local author passphrase is configured
- Diagnostics panel may show Developer mode status and a disable action only after Developer mode is already active
- dedicated developer-only diagnostics panel with module-scoped timelines
- typed diagnostic event contract shared by app modules
- local-only SQLite diagnostic storage with retention rules, initially latest 1,000 events or 7 days, whichever trims first
- module registry so AI analysis, other external-AI workflows, sources, scheduler, credentials, storage, transcripts, shortcuts, locale, licensing, packaging checks, and future modules can report events through one framework
- external-AI workflows as the first rich diagnostic producers, starting with AI analysis job lifecycle, provider resolution, credential check result, request sent, response received, parse/result storage, and failure stage
- lightweight baseline producers for non-AI modules where useful, without turning M14 into a full observability rewrite
- privacy and secret redaction rules for diagnostic payloads
- controls for clearing diagnostics and copying a redacted diagnostic summary
- UI affordance that makes developer mode clearly separate from normal user-facing decision-support UI
- structured event fields kept compatible with future observability adapters where cheap, without adding OpenTelemetry or remote reporting overhead in M14

Exit criteria:

- Developer mode can be enabled through an intentional local developer mechanism and disabled from the app once active
- diagnostics are not visible in normal mode
- diagnostic events are not recorded while Developer mode is disabled
- modules can report typed diagnostic events without each module inventing its own debug UI
- diagnostic events include timestamp, module, scope/entity ID, stage, severity (`debug`, `info`, `warning`, `error`), message, and redacted metadata
- AI analysis and future external-AI workflows report enough staged progress to explain queued/running/failure states without exposing API keys, raw full prompts, full source bodies, or raw provider responses by default
- default retention prevents unbounded local database growth
- tests cover developer-mode gating, settings persistence, event recording/redaction, bounded retention, and developer-only UI visibility
- no diagnostic event leaves the local machine

Non-goals:

- remote telemetry, crash reporting, or hosted observability
- OpenTelemetry implementation or exporter setup
- streaming token-level Gemini progress
- storing full provider prompts/responses by default
- raw JSON/file export of diagnostic events
- replacing normal user-facing status/error UI

## Milestone 15: Local Logs Framework

Goal: add conservative local runtime logs that complement developer diagnostics without introducing telemetry or leaking private material.

Status: completed in `0.15.0`.

Included:

- local log files under the app data directory
- Rust `log` facade with local JSON Lines file backend
- append-only runtime logging for startup, command failures, source/provider failures, storage errors, and important background job transitions
- shared redaction rules with diagnostics
- configurable log rotation, defaulting to five files of five MiB each
- `info` default log level with developer override through Settings and environment
- clear distinction between normal user-facing status, developer diagnostics, and runtime logs
- Settings exposes always-visible local log configuration
- Diagnostics panel exposes a Developer-mode full in-app log viewer, log status, redacted copy action, and open-logs-folder action
- no broad filesystem permissions or arbitrary log-path browsing from React

Exit criteria:

- app writes bounded local logs in the app data/logs directory
- logs rotate and do not grow without limit
- logs do not include API keys, full prompts, full source bodies, full transcript text, raw provider responses, license private material, or full license secrets by default
- log level and rotation limits can be raised intentionally for development without changing production defaults
- Settings can configure local log level and rotation limits
- Diagnostics can inspect current logs when Developer mode is active
- tests cover redaction and rotation behavior where practical
- docs describe how local logs differ from diagnostics and user-facing errors

Non-goals:

- remote log shipping
- hosted crash reporting
- OpenTelemetry exporter setup
- exposing raw logs to normal users
- replacing structured diagnostics with free-form logs

## Milestone 16: Local Metrics Exposure

Goal: expose modest local operational metrics that help understand app health and performance without product analytics, telemetry, or hosted observability.

Status: completed in `0.16.0`.

Included:

- local metrics model for counters, gauges, and durations where useful
- static collector registry, typed internal samples, runtime counters, and a presentation/export adapter boundary for future Prometheus or other local metrics integrations
- metrics view or Diagnostics-panel tab visible only when Developer mode is active
- source refresh duration and item counts per adapter
- source refresh failure and scheduler skipped/running counters
- AI/external-provider job duration, timeout count, failure count, and provider/model labels that do not leak prompts or source bodies
- transcript job duration and failure count
- credential check outcome counts without secret values
- SQLite database size and local row-count summaries for high-growth tables
- feed item counts by source/type and diagnostic event counts by module/severity
- cleanup deleted-item count and duration when retention cleanup runs
- metric names and labels shaped so a future OpenTelemetry adapter could map them cheaply, without adding OpenTelemetry code unless the implementation cost remains low

Exit criteria:

- trusted users can inspect local operational metrics in Developer mode
- metrics are local-only and do not leave the machine
- metric labels avoid high-cardinality or private values such as full URLs, titles, prompts, note bodies, transcript text, or company-specific secrets
- metrics help diagnose source refresh, scheduler, AI provider, transcript, storage, cleanup, and diagnostics health
- tests cover core aggregation and privacy-safe labeling
- docs explain that metrics are operational health data, not user analytics

Non-goals:

- user behavior analytics
- click tracking, screen dwell time, portfolio behavior, or investment-behavior tracking
- remote metrics export
- mandatory OpenTelemetry dependency
- high-cardinality metrics over source URLs, titles, prompts, company names, or note text

## Milestone 17: V1 Friend-Test License Gate

Goal: add a lightweight local entitlement module. Public-opening work later changed the entitlement policy so normal open-core desktop use no longer requires a license key.

Status: completed in `0.17.0`.

Included:

- ADR for the local entitlement posture and threat model
- offline signed entitlement validation
- embedded public verification material
- license entry and status UI in first-run flow or Settings
- local storage for accepted license state without storing private signing material
- app access gate when no valid license exists, superseded by the later open-core policy that keeps normal desktop use available without a license key
- clear expired, invalid, tampered, and unsupported-version states
- release-owner workflow and local automation for generating entitlement material outside the app repository

Exit criteria:

- entitlement validation works offline; normal open-core desktop use is no longer blocked by missing license state after public-opening work
- license verification works offline and does not require cloud accounts, telemetry, hosted activation, or billing infrastructure
- the app embeds only public verification material; private signing material remains outside the repo and build outputs
- tests cover valid, missing, expired, tampered, unsupported-channel, and unsupported-version entitlement states
- logs, settings export, and diagnostics do not leak license private signing material or full license secrets
- user-facing copy makes license status understandable without implying investment advice, account sync, or cloud activation

## Milestone 18: V1 Application Polish

Status: completed in `0.18.0`.

Goal: tighten the daily-use UX/UI and nearby application structure before producing a packaging candidate.

Included:

- notebook workspace layout and scrolling repair
- Inbox feed row metadata and detail-pane readability polish
- safer placement for destructive feed cleanup actions
- Sources grouping and source-health hierarchy polish
- Settings subnavigation for growing settings areas
- watchlist filtering and temporary membership visibility polish before the dedicated M19 watchlist workflow
- reusable field clear controls across typed inputs
- keyboard focus, selected-row, and expanded-row consistency pass
- removal of normal user-facing implementation/architecture wording such as SQLite, database-engine labels, Tauri, adapters, modules, collectors, or similar plumbing outside Developer Diagnostics and owner/developer documentation
- theme framework for separate brightness mode and accent palette selection
- `midnight-horizon` palette based on the project owner's reference-image colors
- focused locale coverage for changed UI
- light extraction of repeated async/loading/error or scheduler patterns where touched
- focused workflow tests and manual smoke checklist for polished screens

Exit criteria:

- Notebooks can be used with long company lists, note lists, and note bodies without broken formatting or lost scrolling
- Inbox rows and details are faster to scan across official reports and public media items
- destructive feed cleanup is visually separated from routine review controls
- Sources is grouped by source purpose and surfaces source health clearly
- Settings remains navigable as settings areas grow
- watchlist memberships remain visible enough for scanning, with full management explicitly moved to M19
- normal user-facing UI uses product language rather than implementation architecture language; technical details remain limited to Developer Diagnostics and owner/developer documentation
- common text/search/URL inputs expose consistent clear controls where useful
- theme mode and accent palette are separate settings, with at least `night-neon` and `midnight-horizon` palettes supported
- keyboard and accessible row states remain consistent across polished screens
- existing local-first, licensing, source, AI, logging, metrics, and settings behavior remains intact
- focused automated tests and a manual smoke checklist cover the polished workflows

## Milestone 19: Dedicated Watchlist Management

Status: completed in `0.19.0`.

Goal: make watchlists a complete, coherent management workflow in a dedicated Watchlists menu panel.

Included:

- a dedicated Watchlists panel/view in the left menu
- watchlist create controls
- watchlist rename controls that preserve the stable internal watchlist id
- watchlist delete controls with confirmation
- selected-watchlist company membership management
- add already-tracked companies to the selected watchlist from a searchable company picker/list
- remove companies from the selected watchlist from the same panel
- visible membership summaries in the Companies list and company workspace without create/delete/add/remove controls there
- watchlist filters retained in Inbox, Events/Calendar, Companies, and Notebooks
- UX wording that explains watchlists as user-owned company groups, not storage entities
- modular backend boundaries that allow future features such as watchlist-based alerts to attach later, without implementing alert placeholders or unused alert fields in M19

Exit criteria:

- user can create, rename, and delete watchlists from the dedicated Watchlists menu panel
- user can add and remove companies from a watchlist without opening each company workspace
- adding companies to a watchlist is limited to companies already tracked in the company list
- renaming a watchlist does not change its stable watchlist id
- Companies list rows show which watchlists each company belongs to, but expose no watchlist mutation controls
- Company workspace shows current memberships as context only, with no watchlist mutation controls
- existing watchlist filters continue to work across Inbox, Events/Calendar, Companies, and Notebooks
- deleting an active filtered watchlist resets affected view filters to `All`
- focused workflow tests cover the dedicated Watchlists panel, rename/delete behavior, filter reset, and Companies membership-display regression
- docs and contracts describe the new ownership of watchlist management

## Milestone 20: Import And Export

Goal: let the user move core local configuration and company-group data in and out of the app without exposing secrets.

Status: completed in `0.20.0`.

Included:

- export companies list
- import companies list
- export watchlists and watchlist memberships
- import watchlists and watchlist memberships
- export notebook entries with tags, claim fields, follow-up fields, and origin metadata
- import notebook entries for existing companies or companies included in the same import
- export non-secret settings
- import non-secret settings
- validation and preview of imported data before it is applied
- conflict handling for existing companies, watchlists, and settings
- explicit exclusion of API keys, license tokens, private signing material, logs, diagnostics, metrics, feed items, and transcript records unless a later backup milestone expands scope
- structured import/export adapters so future backup, sync, or alternate file formats can be added without rewriting the feature
- Settings entry points for import/export actions using product language rather than implementation details
- owner/developer documentation for the supported file format and manual recovery expectations

Exit criteria:

- user can export companies and watchlists to a documented structured file
- user can import companies and watchlists from a supported file and see clear validation errors for unsupported or malformed content
- imported companies and watchlists merge predictably with existing local data without duplicating existing companies
- user can export non-secret settings to a documented structured file
- user can import supported non-secret settings and reject unsupported, invalid, or secret-bearing settings
- import/export code is organized behind explicit format, validation, domain-apply, and UI workflow boundaries
- contracts, data model, product spec, and UI information architecture describe the supported import/export scope
- automated tests cover file validation, secret exclusion, merge behavior, and settings round-trips
- manual smoke testing covers export, clean-profile import, duplicate import, invalid file handling, and settings import

## Milestone 21: Portable Windows Executable Candidate

Status: completed in `0.21.0`.

Goal: produce the first personal-use portable Windows executable candidate.

Included:

- portable Windows executable packaging
- self-sufficient executable packaging posture, with required runtime libraries bundled or otherwise delivered with the candidate artifact
- local WSL-to-Windows packaging command hardening
- native Windows packaging fallback hardening where needed
- portable data directory mode that stores the portable app's data next to the executable
- clear artifact naming with app version and Windows target
- packaged-app entitlement status validation
- packaged-app local data persistence validation
- smoke test checklist
- README quickstart for running the portable executable
- known limitations for the portable candidate

Exit criteria:

- app can be run on Windows from the portable candidate artifact
- the candidate artifact includes the runtime pieces needed by a normal Windows 10/11 machine, or documents any unavoidable prerequisite explicitly
- portable app data is stored in the same folder tree as the executable
- portable app data survives restart and moving the portable app folder when OS-keychain secrets are re-entered as needed
- packaged app keeps normal open-core use available without a license key and still validates optional entitlement tokens offline
- primary workflows pass smoke testing
- portable package creation can be run from the documented local command path
- known limitations are documented

Deferred from the original M21 scope:

- installer packaging
- GitHub Actions packaging workflow
- app version/about screen
- richer license status surface beyond the existing Settings UI
- full import/restore and local backup beyond the M20 import/export feature
- release automation, tags, changelog, and hosted artifacts

## Milestone 22: Sources Trust, Control, And Company Directory Extensibility

Status: completed in `0.22.0`.

Goal: make Sources a normal-user trust and control surface, move unimplemented source candidates to Developer mode/docs, implement the NewConnect company directory source, and keep the company-directory model ready for later markets.

Included:

- normal Sources view showing only implemented sources with normal-user detail depth
- Developer-mode/docs visibility for unimplemented candidates, review-only source details, and source diagnostics that are not normal-user actions
- source visibility tiers for required, optional, and candidate/developer-only sources
- source enable/disable support for optional implemented sources
- protection against disabling required source support from normal UI
- GPW registry reframed as company directory / lookup support
- simple source health statuses suitable for normal users
- removal of unmatched source-item diagnostics from normal UI unless turned into clear user actions
- source-candidate study covering current candidates before any candidate is promoted into normal UI
- NewConnect company-directory source implementation using the official NewConnect company list
- company-directory architecture review for later company-directory sources
- assessment of whether the current company registry, source ID, and database model can support multiple company-directory sources without a larger refactor
- source copy cleanup so normal UI does not expose implementation or architecture wording
- focused tests for source visibility, enable/disable behavior, required-source protection, Developer-mode candidate visibility, copy guardrails, and Sources layout/scroll behavior

Initial source classification:

- required: GPW company directory / registry support, NewConnect company directory support
- optional, default enabled: Bankier Company Komunikaty, Bankier Giełda RSS, GPW market events RSS, Bankier Kalendarium
- candidate/developer-only: GPW ESPI/EBI, Portal Analiz, Bankier Firma RSS, Bankier Wiadomości RSS, Strefa report calendar, Money calendar

Exit criteria:

- normal users see implemented sources only, with clear health and actions
- optional implemented sources can be enabled and disabled, and the choice persists
- required source support cannot be disabled from normal UI
- Developer mode or owner docs expose source candidates and technical review details without showing placeholders in normal UI
- GPW company directory support is understandable as company lookup/matching support
- NewConnect company-directory source is implemented and uses the same directory/cache boundary as GPW
- source-candidate study records the next action for every current candidate
- docs/contracts describe the source visibility tiers and source enablement boundary
- automated tests cover the normal/developer visibility split, enable/disable behavior, copy guardrails, and layout/scroll regressions

## Milestone 23: Browser UI Regression Testing Assessment

Status: completed in `0.23.0`.

Goal: add a small opt-in Playwright browser UI regression smoke path focused on layout problems that Vitest/jsdom cannot reliably detect.

Included:

- compare current Vitest/jsdom workflow tests, CSS contract tests, manual smoke checks, and real-browser automation coverage
- add Playwright with a Chromium-only first configuration
- target the Vite preview app first, not the real Tauri desktop runtime
- keep the browser smoke suite opt-in at first, outside default `make check` and default CI
- use DOM/layout assertions as pass/fail evidence
- retain screenshots and traces only on failure
- use two desktop viewports: compact desktop and normal desktop
- use deterministic frontend test data instead of live sources or the user's local runtime database
- keep Playwright tests under `tests/browser/`
- add a small first-test slice focused on regressions that have already happened: scrolling boundaries, fixed app chrome, Sources bar sizing, notebook panel layout, Companies list height, Watchlists member scrolling, and dense list rendering
- add a tiny navigation smoke check across the main screens
- document setup, commands, troubleshooting, artifact handling, and the WSL/Windows split
- update project practices and engineering workflow for when browser automation should be run

Not in scope:

- broad end-to-end coverage of all product workflows
- live external source/API tests
- replacing existing Vitest component/workflow tests
- real Tauri desktop automation
- Windows file dialog, keychain, taskbar, portable executable, or WebView2 validation
- screenshot comparison tests as pass/fail evidence
- adding the browser smoke suite to default CI before it proves stable

Accepted architecture decisions:

- Test target: Vite preview app in Chromium first; real Tauri desktop automation remains deferred.
- Execution path: opt-in local smoke command first; promotion to default checks requires later stability evidence.
- Evidence model: DOM/layout assertions, with screenshots/traces on failure only.
- Runtime ownership: WSL/Nix owns automated browser layout smoke; native Windows remains the runtime for hands-on desktop behavior and packaging smoke.
- Artifact policy: retain screenshots/traces only on failure.
- Data strategy: deterministic frontend test harness.
- First slice: regression-only layout checks plus a tiny navigation smoke.

Exit criteria:

- ADR 0021 records the browser UI regression testing boundary
- Playwright scope is implemented as approved
- local commands and CI posture are documented
- opt-in browser smoke command exists and is not part of default `make check`
- first smoke tests cover shell fixed chrome, no global app scrollbar, Companies list scroll/height, Notebooks pane scrolling, Sources compact rows, Watchlists member scrolling, and basic navigation
- existing Vitest/jsdom tests remain the fast default workflow suite
- validation evidence includes existing frontend checks plus the new opt-in browser smoke

## Milestone 24: Modularization Readiness And Research Workspace Architecture

Status: completed in `0.24.0`.

Goal: refactor the boundaries required for the next product-differentiation layer, then plan the research-workspace architecture before user-facing implementation so future company timelines, review workflows, research questions, claim tracking, AI briefs, digests, source trust signals, reminders, and evidence links are built on one coherent model.

Context:

Brawler should grow toward a personal research memory system for public companies, not a generic market dashboard. The product value should come from traceable source-grounded research: what changed, why it matters, what management said before, and what the user should check next.

Included:

- audit current frontend, Rust, storage, source, AI, import/export, and test boundaries against the research-workspace roadmap
- map cross-domain pressure points for feed items, notes, claims, transcripts, events, AI outputs, watchlists, sources, retention, import/export, and future backup/sync
- assess current large/coordinating files by responsibility rather than line count, including `AppStateRoot`, frontend API DTOs, storage facade, import/export storage, company workspace, notebooks, source rows/adapters, and shared test harnesses
- complete the extractions that must happen before research-workspace feature work, while leaving unrelated cleanup to future feature slices
- define and execute a refactor sequence for approved research-workspace readiness work, keeping pure extraction separate from user-facing behavior changes
- group the research-workspace candidates into cohesive implementation milestones
- design a shared research evidence model for feed items, official reports, media items, notes, claims, transcripts, events, questions, reminders, AI briefs, and digests
- decide timeline/read-model ownership for aggregating evidence across domains
- define company review and watchlist review semantics, including last-reviewed state and changed-since-review behavior
- define evidence-linking semantics across notes, claims, source items, events, questions, transcripts, AI briefs, and digests
- plan AI brief generation as pluggable evidence collector, prompt/builder, provider, renderer, and storage boundaries
- define source quality/trust signal vocabulary suitable for normal UI
- assess storage, import/export, backup, retention, migration, and test impact
- decide which existing modules are extensible enough and which boundaries need refactoring before feature implementation
- capture the durable research/evidence boundary in ADR 0022

Candidate capabilities to organize:

- company change timeline
- "what changed since last review" views
- expanded management claim tracker
- source-grounded AI research briefs
- open research questions or threads
- watchlist review mode
- event-aware reminders
- source quality and trust signals
- personal research digest
- evidence graph or related-items linking

Not in scope:

- implementing the research workspace features
- broad line-count-driven cleanup without a research-workspace reason
- schema migrations or behavior changes unless the approved architecture requires a preparatory refactor milestone
- adding AI-generated briefs before evidence and citation boundaries are clear
- adding generic portfolio tracking, trading signals, technical charts, or market dashboard features

Accepted architecture decisions:

- Refactor timing: M24 refactors everything required for research-workspace readiness, but does not perform unrelated line-count cleanup.
- Frontend domain ownership: research-workspace aggregation gets a dedicated frontend domain/API/controller boundary.
- Backend domain ownership: Rust gets a dedicated research/evidence command and domain boundary while canonical domain storage remains owned by existing modules.
- DTO ownership: research-workspace API types move into focused modules instead of expanding `src/api/types.ts`.
- Evidence model: hybrid model with canonical domain tables as source of truth, typed evidence read models and links for cross-domain behavior, and stored projections deferred until needed.
- Timeline ownership: backend read models own timeline aggregation first; stored projections remain a later implementation detail behind the same boundary.
- Review state: layered model using company/watchlist checkpoints, existing item-level state, and future evidence-level state only where needed.
- Linking model: typed evidence links alongside existing origin references.
- AI output persistence: AI research briefs are dedicated entities with provenance and citations, not ordinary notebook entries.
- Reminder model: decide during M24 whether first implementation needs explicit reminder entities or can derive reminders from claims/events until pressure appears.
- Source trust vocabulary: start with fixed app-owned taxonomy suitable for normal UI; user-editable labels can be added later if there is product pressure.

Exit criteria:

- modularization readiness findings are recorded with required/refactor-later/no-action recommendations
- required research-workspace readiness refactors are completed before feature implementation starts
- recommended architecture is recorded with options, tradeoffs, and accepted decisions
- user explicitly approves any remaining architecture decisions before implementation milestones are created
- roadmap and Radicle/Radboard contain the resulting implementation sequence
- docs/contracts/data model/UI architecture are updated enough that future agents can implement without rediscovering the model
- ADR checkpoint is complete

## Patch 0.24.1: Multi-Registry Company Directory Hardening

Status: completed in `0.24.1`.

Goal: make the company-directory, lookup, company-add, and company-owned workflow paths work for NewConnect and any later company registry source without GPW/NC-specific assumptions.

Included:

- company lookup searches all active company-directory entries, with selected exchange used only as a duplicate-ticker preference
- company creation from lookup works for NewConnect and future exchange-qualified tickers
- company-directory bootstrap and stale checks apply to all enabled `company_registry` source adapters
- media matching considers all tracked companies instead of only GPW companies
- source-listing matching has a reusable exchange-aware registry lookup helper for future report/event adapters
- regression tests cover future exchange behavior for lookup, company creation, watchlists, notebooks, manual events, import/export, media matching, source-listing matching, and Companies UI add flow
- source strategy documents the checklist for adding the next company-directory source

ADR checkpoint: no new ADR required; this hardens the M22/M24 source and modularity decisions without changing the durable architecture.

## Milestone 25: Company Evidence Timeline And Release Workflow

Goal: deliver the first visible Research workspace slice and put the release/version workflow on rails.

Status: completed in `0.25.0`.

Included:

- top-level Research navigation entry and screen
- company-scoped evidence timeline assembled through the backend research read model
- timeline summary with total evidence, changed-since-review count, and last-reviewed state
- backend-owned evidence type filters and changed-only filtering
- company-level `Mark reviewed` checkpoint workflow
- evidence rows with product-language labels, source/trust context, compact density, and owning-domain/source actions
- regression coverage for Research navigation, backend filtering, review state, and normal-user copy leaks
- SemVer and Conventional Commit release workflow documentation
- local commit-message hook and validator
- `git-cliff` changelog configuration and release validation commands
- retroactive changelog baseline through `0.24.1`

Exit criteria:

- Research opens as a real user-facing screen from the left navigation
- Research timeline uses backend-owned aggregation and filtering, not frontend cross-domain assembly
- feed evidence opens the selected Inbox item without hiding it through wrong filters
- normal Research UI does not expose raw identifiers such as event codes or provider ids
- version files are synchronized at `0.25.0`
- changelog includes the `0.25.0` release entry
- release workflow checks pass

ADR checkpoint: no new ADR required for the final closure pass. M25 implements accepted Research/read-model decisions already captured in docs and the release workflow decisions captured in release workflow docs.

## Milestone 26: Watchlist Research Review

Goal: make Research useful for watchlist-level review while preserving backend-owned evidence aggregation and review semantics.

Status: completed in `0.26.0`.

Included:

- Company/Watchlist mode switch inside the existing Research screen
- watchlist-scoped evidence timelines assembled through the backend research read model
- member-company review queue for the selected watchlist
- backend-owned per-company watchlist summary counts
- watchlist-level `Mark reviewed` checkpoint workflow
- explicit option to cascade a watchlist review checkpoint to current member companies
- note deletion from the selected notebook entry detail toolbar
- regression coverage for watchlist review mode, explicit cascade behavior, and notebook note deletion

Exit criteria:

- Research can switch between company and watchlist review without adding a separate top-level screen
- watchlist mode shows the selected watchlist's member companies and evidence for the selected member company
- watchlist timeline filtering and changed-since-review counts come from the backend research boundary
- marking a watchlist reviewed does not mark member companies reviewed unless the explicit cascade option is selected
- the cascade option is sent to the backend command and applied there, not inferred in the frontend
- a selected notebook entry can be deleted through the UI with confirmation
- automated tests cover the new watchlist review workflow and notebook delete workflow
- docs and contracts describe watchlist review semantics

## Milestone 27: Public Opening Preparation

Goal: prepare the current repository tree for public open-core publication while keeping owner-only operational context private.

Status: completed in `0.27.0`.

Included:

- MPL-2.0 project license and package metadata
- public README, maintainer, contribution, support, security, and code-of-conduct files
- dependency-license audit summary for npm and Cargo dependencies
- public/private documentation split ADR and public docs cleanup
- private archive preservation for removed licensing and naming documents
- Brawler confirmed as the official application name
- local entitlement module changed so normal open-core desktop use does not require a license token
- public-safe license generation defaults with owner-specific key paths moved to environment overrides

Exit criteria:

- current source tree is suitable for public publication after Git history sanitization
- public docs avoid owner-only strategy, private key paths, raw token operations, and personal seed-host detail
- no committed public-facing docs are required to depend on the private sibling repository
- ignored local files such as `.env.local`, `private/`, `data/`, build outputs, and target directories remain excluded from the clean public baseline
- open-core license posture is explicit and consistent across public repo metadata
- automated release and application checks pass

## Milestone 28: Cross-Platform Release Artifacts

Goal: publish practical public release binaries from standard Linux infrastructure while keeping Radicle as the canonical forge.

Status: completed in `0.28.0`.

Included:

- Linux `amd64` release packages: `.deb`, `.rpm`, and `.AppImage`
- zipped Windows `x64` portable executable built from Linux with `cargo-xwin`
- GitHub Release artifact workflow triggered by pushed `v*` tags on standard `ubuntu-latest`
- GitHub Releases as the public binary mirror
- Radicle retained as canonical source, issue, and patch forge
- Linux release-build data location set to `~/.brawler`
- Windows portable release-build data location kept as `data/` next to the executable
- release artifact naming policy for Linux and Windows assets
- packaging smoke-test documentation for Linux packages, Linux AppImage, and Windows portable zip
- Radboard cleanup so active epic titles use `epic: <title>` without numeric prefixes

Deferred:

- native Pacman package generation
- Windows installer packaging
- code signing
- package repositories
- macOS and macOS arm64 packaging
- paid or larger CI runners

Exit criteria:

- `make package-linux-amd64` builds and collects versioned `.deb`, `.rpm`, and `.AppImage` artifacts
- `make package-windows-portable-zip` builds a versioned Windows portable zip from Linux
- GitHub Actions release workflow uses only `ubuntu-latest` and uploads all release artifacts to the matching GitHub Release for `v*` tags
- Linux release builds use `~/.brawler` for app data
- Windows portable zip contains only `brawler.exe` and a portable readme
- smoke-test docs cover expected install/run and data-location checks
- ADR 0024 records the release artifact policy
- automated validation and available packaging smoke checks pass

## Milestone 29: Research Questions And Evidence Links

Goal: add durable company-scoped research questions and typed evidence-link workflows inside the Research workspace.

Status: completed in `0.29.0`.

Included:

- durable research questions stored outside notebooks
- company-scoped question creation in the Research screen
- question status workflow with open, answered, closed, and reopen states
- research questions represented as timeline evidence items
- selected-question evidence linking and unlinking from visible Research rows
- backend validation for question-to-evidence links
- research import/export coverage for questions and evidence links
- regression coverage for question creation, linking, import/export, and question-row navigation
- docs, contracts, data model, UI architecture, and ADR checkpoint updates

Exit criteria:

- Research questions can be created for the selected company
- questions appear in the Research evidence timeline
- selecting a question enables linking visible evidence rows to that question
- question-row navigation stays inside Research and does not disturb the evidence timeline
- linked evidence can be reviewed and removed from the selected question
- research import/export includes questions and evidence links
- automated validation passes

## Milestone 30: AI Research Briefs

Goal: generate source-grounded company and watchlist research briefs inside the Research workspace while preserving the backend-owned research/evidence boundary.

Status: completed in `0.30.0`.

Planned scope:

- company and watchlist brief generation from backend-collected research evidence
- dedicated brief, brief-job, and citation persistence
- immutable brief snapshots; regeneration creates a new brief instead of overwriting prior research
- provider-neutral AI execution through the existing AI analysis provider boundary
- backend prompt/context builder with citation-keyed evidence input
- citation mapper that links generated claims back to research evidence items
- backend renderer/read model for UI display
- compact Research UI for generating, viewing, and reviewing brief citations/provenance
- research import/export coverage for briefs and citations
- collector, provider-job, citation-grounding, storage, and UI workflow tests

Architecture decisions:

- Brief generation is explicit and on-demand only.
- Company and watchlist briefs are both in scope.
- Briefs are dedicated research entities, not notebook entries.
- The initial provider configuration reuses existing general AI provider/model settings.
- Evidence collection uses a backend-owned default collector for the selected scope, not frontend-assembled evidence.
- Provider output should be structured into sections with citation IDs, then rendered by the backend.
- Citation storage keeps evidence references and short citation labels/snippets, not full copied source text.
- Creating notebook notes from briefs remains out of scope for this milestone; no automatic note creation is allowed.
- Briefs and citations are durable user-owned research data and are included in research import/export.

Exit criteria:

- A user can generate a company-scoped brief from the Research screen.
- A user can generate a watchlist-scoped brief from the Research screen.
- Generated briefs persist with provider ID, model, prompt version, evidence collector version, renderer version, timestamps, status, and citations.
- Brief output is source-grounded and shows citations in normal UI.
- Brief output does not present buy/sell/hold recommendations.
- Brief generation remains non-blocking and status updates without manual refresh.
- Regenerating a brief creates a new snapshot.
- Research import/export includes briefs and citations.
- Automated validation passes.

## Future: Research Workspace Implementation Sequence

M25 delivered the first company-scoped Research screen, M26 delivered watchlist review mode, M29 delivered research questions plus evidence links, M30 delivered AI research briefs, and M31 delivered event-aware reminders plus personal research digest generation.

Status: event-aware reminders and personal research digest completed in `0.31.0`.

The recommended follow-up sequence after M31 is:

1. Stored timeline/evidence projections only if needed.
   - If live read-model aggregation becomes too slow or review semantics require snapshots, add stored projections behind the existing research API.
   - Projection rows must be rebuildable or have explicit import/export and backup policy.

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

## Milestone: Multi-provider AI (v0.35.0)

Status: completed in `0.35.0`.

Goal: add Claude (Anthropic) and OpenAI (ChatGPT) as AI providers alongside Gemini, all free with a user-supplied key, before AI KPI extraction is built on the provider boundary. Inserted ahead of extraction so the report-document input path is designed against more than one provider rather than retrofitted later.

Scope: a provider registry/factory replacing the hardcoded dispatch, per-provider keychain credentials and model selection, Claude and OpenAI analysis adapters implementing the existing provider trait, and a document-input abstraction on the trait (with Gemini retrofitted). Managed/hosted AI remains a future paid tier; this milestone keeps every provider free with the user's own key.

Delivered: async provider/transcription boundary; provider registries; one-key-per-provider keychain credentials with a generic command surface; per-provider model registry (migration 0036) and catalog-driven selection UI; Claude and OpenAI adapters over a shared prompt/parse layer; document-input abstraction (Gemini native proven, capability-flagged). The local PDF text-extraction dependency and the extraction job itself remain deferred to v0.36.0.

## Milestone: Company Fundamentals (v0.34.0, v0.36.0–v0.37.0)

Goal: turn report numbers into a structured, source-linked fundamentals view per company, so the investor stops re-reading reports to find the same figures each quarter. Scope and the KPI taxonomy are fixed in [ADR 0027](adr/0027-company-fundamentals-scope.md).

Note: the AI extraction and panel/charts work shifted up one minor version after a dedicated [Multi-provider AI](#milestone-multi-provider-ai-v0350) milestone was inserted at v0.35.0, so the report-document input path is designed against multiple providers before extraction is built.

v0.34.0 — Financial facts foundation:

Status: completed in `0.34.0`.

- ADR and KPI taxonomy (this milestone's first task)
- data model for financial periods, financial facts, and KPI definitions (canonical plus custom per-company), with provenance and confirmation state
- financial facts storage, commands, and a focused frontend DTO module
- report document persistence with user-supplied PDF URLs and URL evidence capture
- manual KPI entry and edit workflow

Deferred to v0.39.0: ESPI/EBI attachment ingestion and on-track company history backfill, which depend on the reusable feed company-matching boundary and live GPW verification. User-supplied URL capture plus manual entry provide the same fundamentals capability in the meantime.

v0.36.0 — AI KPI extraction with confirmation:

- provider-neutral extraction contracts with per-fact source citations and prompt-version provenance
- report-document input for the Gemini adapter plus a deterministic test provider
- an extraction job that persists proposed facts as pending confirmation
- a review UI where the user confirms, edits, or rejects each proposed fact before it is committed

v0.37.0 — Fundamentals panel and KPI charts:

- hand-rolled SVG chart primitives in the shared UI layer, no new runtime dependency
- a fundamentals panel in the company workspace with per-period figures and click-through to source evidence
- custom per-company KPI management and KPI trend charts
- export/import of financial facts and KPI definitions

Cross-company KPI comparison follows later in v0.42.0.

Exit criteria:

- a real GPW company's periodic report can be turned into confirmed, source-linked financial facts end to end
- no AI-proposed figure is stored as confirmed without explicit user review
- figures render per period and over time with every value traceable to its report
- the scope stays report-derived; no price, volume, technical, or market-dashboard features are introduced

## North Star: Autonomous Report Pipeline (v0.47.0)

The fundamentals, extraction, diff, claims, and cockpit milestones are building blocks toward one experience: a tracked company publishes a periodic report, and the app detects it, fetches it, extracts the figures, summarizes what changed, cross-references the result against open claims, research questions, and evidence, and surfaces a single notification — with no manual steps.

This is deliberately sequenced last (v0.47.0) because it composes everything before it. It introduces a trust ladder rather than changing the confirmation guarantee: confirm-before-commit stays the default, the user opts a specific company into auto-confirm, and auto-committed facts carry a distinct unreviewed provenance state so they stay flagged, reversible, and cited. The financial_facts confirmation model in v0.34.0 is designed so this state is an additive value, not a later migration.

Boundary: fetching and analyzing while the app is closed crosses into a hosted/scheduled service and belongs to the managed-AI paid frontier, not the open core. Autopilot runs while the app is open.

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
- cloud sync
- team collaboration
- hosted ingestion jobs
- commercial paid data APIs that require redistribution/licensing or product-level billing
- mobile app
- price/volume series, technical-analysis indicators, valuation tooling requiring live prices, and market dashboards (see [ADR 0027](adr/0027-company-fundamentals-scope.md))
