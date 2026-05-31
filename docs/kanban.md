# Kanban

## Backlog

### Design feed retention policy

Intent: prevent the local SQLite database from growing indefinitely with low-value feed items.

Acceptance criteria:

- Default retention periods are defined per source category.
- User-adjustable retention settings are documented.
- Saved items, note-linked items, AI-analyzed items, and explicitly preserved items are protected from routine cleanup.
- Cleanup behavior is transparent in Settings or Sources before it can delete data.
- Database size and item counts can be inspected by the user.

Docs/contracts touched: product spec, data model, contracts, settings docs.

Test expectations: future retention policy unit tests and migration/storage tests.

### Implement YAML settings import/export/bootstrap

Intent: implement the accepted YAML settings contract during the later export/import/backup work.

Acceptance criteria:

- Non-secret settings export to YAML.
- YAML import validates known setting keys and value types before writing to SQLite.
- YAML bootstrap can initialize non-secret settings for a new local database.
- Provider secrets are never exported.

Docs/contracts touched: contracts, product spec, project practices.

Test expectations: YAML round-trip, validation, and secret-exclusion tests.

### Make source poll interval editable in Settings

Intent: let the user tune source polling cadence without changing code.

Acceptance criteria:

- Settings screen exposes a compact poll interval control.
- Accepted values are validated before writing to SQLite.
- The in-app source scheduler immediately uses the updated interval.
- The Sources screen continues to show the active scheduler cadence.

Docs/contracts touched: contracts, product spec, settings docs.

Test expectations: settings command validation and UI workflow test for updating poll interval.

### Refine watchlist membership UX

Intent: replace the early company-row assign/remove controls with a more intuitive watchlist membership workflow.

Acceptance criteria:

- User can see current watchlist membership for a company at a glance.
- Adding/removing memberships does not require tedious repeated row-level actions.
- Mutating actions provide quick visual confirmation.
- Workflow remains responsive and efficient for many companies and many watchlists.

Docs/contracts touched: UI information architecture, product spec if behavior changes.

Test expectations: UI workflow tests for membership add/remove and confirmation states.

### Add field-level clear controls across typed inputs

Intent: make repeated desktop data entry and filtering faster by giving text-like fields a consistent inline clear affordance.

Acceptance criteria:

- Text, search, URL, and optional metadata inputs expose a compact inline clear control when they have a value.
- Required fields only expose clear controls when clearing does not create confusing validation feedback, or the validation state remains clear and local.
- Controls use consistent icon-only styling and accessible labels.
- Clearing one field must not trigger stale lookup/autocomplete side effects.
- Native browser search clear controls are avoided when the app renders its own clear control.
- Existing manual typing, autocomplete, lookup, and form-submit workflows continue to work.

Docs/contracts touched: product spec or UI information architecture if this becomes a cross-screen UI standard.

Test expectations: focused UI workflow tests for representative forms and filters.

### Implement keyboard shortcuts

Intent: add discoverable keyboard shortcuts for repeated v1 workflows after core screens are stable.

Acceptance criteria:

- Shortcut map covers common Inbox actions first.
- Shortcuts are documented in Settings or Help/About.
- Every shortcut action remains available through visible UI controls.
- Shortcuts do not fire while typing in inputs, note editors, forms, or transcript selection.
- Windows-native and browser editing shortcut conflicts are avoided.
- Notebook shortcuts include `Ctrl+E` to open the editor for the selected item and `Ctrl+S` to save the currently edited item.

Docs/contracts touched: product spec, UI information architecture, roadmap.

Test expectations: workflow tests for critical shortcuts.

### Explore terminal interface

Intent: record and later evaluate a terminal/TUI version of Brawler for keyboard-first investor research.

Acceptance criteria:

- TUI scope is designed after desktop v1 foundations are stable.
- Design is loosely inspired by `k9s` density and navigation ergonomics.
- Theme uses terminal-safe variants of the night-neon palette.
- Optional synthwave-style background music is opt-in only.
- TUI reuses the core domain and storage contracts.

Docs/contracts touched: product spec, roadmap, architecture if accepted.

Test expectations: future TUI command/navigation tests if implemented.

### Explore mobile clients and sync

Intent: record and later evaluate mobile versions with cross-device sync.

Acceptance criteria:

- Sync ownership, hosting, encryption, conflict resolution, and privacy model are designed before implementation.
- Mobile scope is defined separately from desktop parity.
- Offline-first expectations are documented.
- Monetization implications are captured before launch.

Docs/contracts touched: product spec, roadmap, architecture, future sync ADR.

Test expectations: future sync contract tests, conflict-resolution tests, and mobile workflow tests if implemented.

### Implement Gemini YouTube transcription spike

Intent: validate Gemini as the first provider only for YouTube press conference transcription and transcript-to-note workflows.

Acceptance criteria:

- User can submit a YouTube URL for a selected company.
- App creates a transcript job using the Gemini provider.
- Returned transcript segments can be reviewed.
- User can save selected segments as notebook notes with origin.
- Settings disclose free-tier limits and provider privacy terms.

Docs/contracts touched: architecture, product spec, contracts, source/AI policy ADR.

Test expectations: provider contract tests with test samples and transcript-to-note workflow tests.

### Implement company events calendar

Intent: show dated company events for companies in the user's watchlists, with upcoming events as the default focus and historical dates available for context.

Acceptance criteria:

- Events screen or panel lists company events across watchlists.
- Upcoming events are the default view.
- Historical events or a combined date range can be selected.
- Report publication dates and dividend-related dates are supported first when source data is available.
- Watchlist, company, event-type, due-soon, and date-range filters are available.
- Event rows show date, company ticker, event type, source/manual marker, and status.
- Event details show source URL, attribution, fetched timestamp, event date/time, and origin/source type.
- Manual events can be represented distinctly from sourced events.
- User corrections do not destroy the original sourced event record.

Docs/contracts touched: roadmap, product spec, UI information architecture, data model, contracts.

Test expectations: storage tests for event records and UI workflow tests for filtering and expanded event details.

### Implement Polish media and research sources

Intent: extend the Inbox beyond official reports so it also collects company-related news, articles, analysis, and private research for tracked companies.

Acceptance criteria:

- Candidate public sources are reviewed and ranked before implementation.
- At least one public article/news/analysis source is implemented as a source adapter.
- Source type distinguishes public media, analysis, and official reports.
- Company matching supports ticker, company name, aliases, and source-specific IDs where appropriate.
- Dedupe handles syndicated or copied content across sources.
- Portal Analiz is tracked as a v1 authenticated private source but is not implemented until a dedicated ADR approves the source policy, credentials/session handling, and rate limits.
- Authenticated sources store secrets only in the OS keychain and tests use test samples/mocks.
- Sources screen shows public/RSS/paywalled/authenticated source status clearly.

Docs/contracts touched: roadmap, product spec, source strategy, contracts, future Portal Analiz ADR.

Test expectations: parser/fetcher test-sample tests, source adapter contract tests, storage dedupe tests, and UI source-status workflow tests.

## Ready

No cards.

## In Progress

No cards.

## Review

No cards.

## Done

### Complete Milestone 7: GPW Company Registry Cache

Intent: replace annoying manual GPW company metadata management with a cached local registry used for lookup, autocomplete, and ticker-first source matching.

Acceptance criteria:

- A reliable GPW company metadata source is selected and documented.
- Registry records include exchange, ticker, exchange-qualified ticker, company name, ISIN, aliases when available, source URL/source name, fetched timestamp, and registry freshness.
- Registry data is stored in SQLite and survives app restarts.
- Registry refresh runs manually and on a slow cadence, initially daily or weekly.
- Company creation can search/autocomplete from the local registry cache.
- Existing manually created companies are preserved and not overwritten silently.
- GPW source matching uses ticker first, then exact ISIN fallback; issuer/company name alone is not a silent match key.
- Sources or Settings show registry last refresh and last error.

Delivered:

- SQLite `company_registry_entries` cache exists.
- GPW registry source adapter fetches the complete public GPW company list from `https://www.gpw.pl/spolki?offset=0&limit=500`.
- Parser tests use a test-sample-backed GPW company-list fragment so default checks stay offline.
- Target runtime databases no longer seed registry rows or feed rows from sample data.
- Company lookup reads SQLite registry entries and may bootstrap the live GPW registry only when the runtime cache is empty.
- GPW feed matching resolves ISIN to ticker through the registry and then matches tracked companies by ticker.
- Sources view shows the registry adapter, freshness/error status, and manual registry refresh.
- Sources registry detail exposes a searchable cached-company list with tracked/untracked state and add actions for untracked companies.
- Companies form shows cached GPW registry suggestions from ticker, company-name, or ISIN input and can fill the creation form from a selected suggestion.
- Companies view has tracked-company search so registry-assisted additions remain easy to find and inspect.
- The desktop UI schedules a slow in-app stale-cache registry refresh check using the registry adapter interval, currently one day, without refreshing immediately on startup.
- Final local checks passed.
- Version bumped to `0.7.0`.

Docs/contracts touched: source strategy, product spec, contracts, data model, architecture.

Test expectations: registry parser/fetcher test-sample tests, migration tests, lookup tests, source matching tests, and UI autocomplete workflow tests.

### Complete Milestone 6: GPW Detail Fetch Spike

Intent: implement a reliable path for reading official GPW ESPI/EBI report bodies inside the app.

Acceptance criteria:

- GPW detail-page test samples exist.
- Parser spike extracts useful report body or excerpt text when technically stable.
- Parser spike extracts visible attachment links when present.
- Detail-page source policy and rate-limit behavior are documented.
- Matched GPW feed items can store and expose official report body text from an accepted source path.
- If GPW detail pages are not reliable enough for some items, fallback body-source investigation is required.
- Secondary Bankier/Parkiet RSS is documented as cross-check/fallback signal, not canonical replacement.

Delivered so far:

- Test-sample-backed detail parser tests exist.
- Parser assumptions were compared against real GPW detail-page structure before promotion.
- GPW was compared with PAP, Bankier, Parkiet, and Stooq before changing any report-body requirement.
- Injectable detail-page fetch boundary exists with test-sample-backed fetch-and-parse coverage.
- No-attachment detail pages are covered, and later English/entity/signature sections do not leak into the main report body.
- Detail usability evaluation emits warnings for missing title or missing/very short body text.
- Aggregate spike report flags rejected samples for parser hardening or fallback-source investigation.
- Conservative detail fetch policy defaults are documented and implemented for matched-item ingestion.
- ADR 0013 records that in-app official report body access is required and GPW detail fetching is the primary implementation path.
- Source strategy records that GPW remains the primary path, the GPW AJAX listing endpoint is the live listing transport, PAP/Bankier/Parkiet are fallback or cross-check candidates, and Stooq is not a primary ESPI/EBI report-body candidate.
- Bankier/Parkiet RSS are documented as diagnostics/fallback visibility signals, not GPW official report replacements.
- Additional source candidates discovered during M6 research are recorded: Bankier market/news RSS, Investing.com Poland RSS, Stooq price CSV, BiznesRadar/StockWatch analysis pages, Notoria commercial data, and issuer IR pages.
- Accepted detail-body fetching is wired into normal matched-item ingestion.
- Detail fetch counters and last detail warning are exposed in refresh results and source adapter status.
- Parsed GPW detail attachments are stored as feed item attachment links and shown in feed details.
- Source adapter status text describes the current M6 listing/detail behavior.
- Final local checks passed.
- Version bumped to `0.6.0`.

Docs/contracts touched: source strategy, contracts, roadmap if the decision changes scope.

Test expectations: Rust parser tests with local test samples; no live GPW network dependency.

### Complete Milestone 5: GPW ESPI/EBI Listing Adapter

Intent: validate the first official GPW source path.

Acceptance criteria:

- Adapter can fetch and normalize recent public GPW ESPI/EBI report listings.
- Adapter stores source URL, timestamps, language, company match, and attribution.
- Adapter primarily matches companies by ISIN.
- Rate limit and source policy are documented.
- Detail-page fetching remains separate from listing ingestion.

Delivered:

- Rust `gpw-espi-ebi` adapter module parses listing HTML test samples into normalized report listings.
- Parser extracts publication timestamp, report type, ESPI/EBI system, report number, company name, ISIN, title, detail URL, fetched timestamp, and dedupe key.
- Rust storage ingests normalized GPW listings into `feed_items`, upserts by adapter/dedupe key, matches companies by ISIN, and stores unmatched items without showing them in normal feed views.
- `refresh_sources` Tauri command performs an explicit manual fetch of the GPW ESPI/EBI public-page listing fragment and ingests parsed listings.
- `refresh_sources` returns detail-body counters for attempted, stored, and failed GPW detail fetches.
- Source adapter status exposes the last GPW detail warning when body fetching or parsing fails.
- Accepted GPW detail attachments are stored as feed item attachment links and shown in feed details.
- Topbar, Sources screen, and no-feed empty-state source refresh controls trigger manual source refresh and reload feed/source status.
- Desktop runtime schedules in-app source refreshes while the UI is open, using the configured poll interval and skipping overlapping runs.
- Scheduled source refreshes back off after repeated refresh failures while preserving manual refresh.
- Sources screen shows the expected next scheduled in-app source poll.
- Sources screen shows whether the last source refresh attempt was manual or scheduler-triggered.
- Sources screen exposes recent unmatched source diagnostics so a successful fetch with no company matches is understandable.
- Manual refresh records a persisted last-attempt timestamp before fetching.
- Successful refreshes with zero parsed listings persist a success timestamp and zero-count result instead of looking like no-op failures.
- Successful refreshes persist last fetched/created/matched/unmatched counts for adapter diagnostics.
- Failed manual refreshes persist adapter `last_error_at` and `last_error` so source status remains diagnosable after transient UI errors.
- Topbar refresh control exposes the latest refresh failure state until a later successful refresh attempt.
- Sources screen shows GPW source URL, rate-limit policy, and source-policy note.
- Tests use bundled test samples/injected fetchers; source status and scheduler behavior have UI coverage.

Docs/contracts touched: contracts, architecture, source strategy.

Test expectations: adapter unit tests with test samples and storage ingestion tests for matched/unmatched listings.

### Complete Milestone 4: Notebooks And Claims

Intent: add durable per-company Markdown notes that can later be created manually, from feed items, and from transcript selections.

Acceptance criteria:

- Notebook entries can be created for a company.
- Notebook entries list by company in the company workspace.
- Notebook entries can be edited after creation without losing origin.
- Notes support Markdown body, tags, kind, claim status, event date, follow-up quarter, follow-up date, and origin.
- Feed item detail view can create a note draft linked to that feed item.
- Cross-company Notebooks screen lists notes, supports company-first navigation, and includes enough filtering/search for v1.
- Claims tab lists claim notes and allows status update.

Current slice:

- Rust storage exposes typed `create_notebook_entry` and `list_notebook_entries` commands against the existing SQLite notebook tables.
- Notebook entry creation supports Markdown body, tags, note kind, claim status, event date, follow-up quarter, follow-up date, and origin.
- Company workspace Notebook tab lists notebook entries for the selected company.
- Company workspace Notebook tab can create manual Markdown notes with tags, kind, optional claim status, event date, follow-up quarter, follow-up date, and manual origin.
- Company workspace Notebook tab uses a compact selectable list, an on-demand creation form, and an editable selected-note detail pane.
- Notebook and claim rows omit raw Markdown body previews; full Markdown is shown only in expanded/read detail.
- Rust storage exposes typed `update_notebook_entry` for editable note fields while preserving origin.
- Main Notebooks screen replaces the placeholder with company-first navigation and company-scoped note rows that expand in place, open read-first, and can switch in place to edit mode.
- Main Notebooks screen can create a manual Markdown note for the selected company with the same core fields as the company workspace form.
- Main Notebooks inline edit supports the same core note metadata fields, including event date and exact follow-up date.
- Main Notebooks company navigator shows note count plus actionable open-claim and follow-up scheduled counts when present.
- Inbox and company feed detail actions can open an editable note draft in the main Notebooks pane with `feed_item` origin.
- Note detail surfaces render origin links as compact actions, including opening feed-item origins back in the Inbox and opening URL-backed sources externally.
- Origin links are immutable through normal note edits and covered by storage tests.
- UI-facing feed items are scoped to tracked companies, so feed-to-note starts from a matched company and attaches the draft automatically.
- Company workspace Claims tab lists claim-like notebook entries, expands claim details in place, and supports claim status update through the notebook update contract.
- Main Notebooks screen filters selected-company notes by kind, claim status, tag, and follow-up scheduling presence while global search continues to match note content.
- Notebook read mode renders common Markdown while edit mode keeps the raw Markdown body.

Docs/contracts touched: roadmap, kanban, contracts, product spec, UI information architecture, data model.

Test expectations: Rust notebook storage tests and UI workflow tests as UI surfaces are implemented.

### Complete Milestone 3: Inbox And Company Workspace

Intent: make the primary non-AI research workflow usable with local sample data.

Acceptance criteria:

- Feed items are read from local SQLite storage.
- Inbox filters cover watchlist, company, source, type, unread, saved, and search.
- Feed item detail shows source URL, timestamps, attribution, and summary.
- Source URLs are directly actionable.
- Read/unread and saved/unsaved state persists through the Tauri/Rust command boundary.
- Empty states distinguish no companies, no stored feed items, and no matching filters.
- Company workspace opens from company rows and matching Inbox items.
- Company workspace includes Feed, Notebook, Claims, Transcripts, and Metadata tabs.
- Company Feed tab shows company-scoped feed items, inline details, read/save actions, and open-in-Inbox behavior.
- Sources screen shows local source adapter status, and the topbar source status opens the most relevant adapter.
- Top toolbar remains visible while workspace content scrolls.
- Daily review workflow has automated frontend coverage.

Notes:

- Sample feed rows were early development seed data and are no longer inserted into target runtime databases as of the M7 registry-cache closure work.
- Real source ingestion and manual refresh jobs are deferred to later source milestones.
- Notebook, Claims, and Transcripts tabs remain intentional placeholders until their roadmap milestones.

Docs/contracts touched: roadmap, kanban, product spec, UI information architecture, contracts, data model.

Test expectations: `make check` before commit and Windows package sanity by the project owner.

### Finish Milestone 2 before Milestone 3

Intent: stop milestone drift and close Local Domain And Storage Foundation before continuing Inbox/Company Workspace work.

Acceptance criteria:

- Milestone 2 exit criteria are checked against implementation.
- Settings commands and Settings screen basics are complete.
- YAML settings boundary is explicitly deferred with a follow-up card.
- Milestone 2 cards are moved to Done or Review.
- Milestone 3 work resumes only after Milestone 2 closure.

Docs/contracts touched: roadmap, kanban, contracts, product spec.

Test expectations: `make check` plus Windows package sanity after milestone closure.

### Implement settings storage commands and Settings screen

Intent: finish Milestone 2 settings scope by moving runtime settings from frontend-only state/localStorage into SQLite-backed Tauri commands.

Acceptance criteria:

- Rust exposes typed `get_settings` and `update_settings` commands.
- Settings screen reads current SQLite settings.
- Theme setting is stored in SQLite and remains dark by default on first run.
- Theme changes persist through app restart.
- Settings values remain non-secret only.
- Frontend no longer treats localStorage as the runtime source of truth for theme.

Docs/contracts touched: contracts, product spec, data model.

Test expectations: Rust settings storage tests and UI workflow test for settings/theme persistence.

### Decide Milestone 2 YAML settings boundary

Intent: decide whether YAML import/export/bootstrap is implemented in Milestone 2 or explicitly deferred to a later export/backup slice.

Acceptance criteria:

- Decision is documented in roadmap and contracts.
- Milestone 2 marks YAML as contract-only.
- Later implementation card exists.

Docs/contracts touched: roadmap, contracts, product spec, project practices.

Test expectations: none because implementation is explicitly deferred.

### Design initial SQLite migrations

Intent: create migration-managed local storage for companies, watchlists, feed items, source records, notebook entries, transcript jobs, transcript segments, jobs, and settings.

Acceptance criteria:

- Migration runner exists.
- Initial schema represents contracts in `docs/contracts.md` and the entity list in `docs/data-model.md`.
- Migration tests cover clean database creation.
- Migration check is suitable for GitHub Actions.

Docs/contracts touched: contracts, architecture.

Test expectations: migration tests.

### Add test-sample-backed company lookup

Intent: make company creation less manual by filling ticker, name, and ISIN from an exchange-scoped lookup.

Acceptance criteria:

- User can request lookup from the Companies form.
- Exact ticker or ISIN lookup fills missing company fields when a test sample match exists.
- Name lookup can find a test sample match when the entered name is specific enough.
- Manual company entry remains possible.
- Lookup source is clearly sample/local for now and replaceable by a future registry adapter.

Docs/contracts touched: product spec.

Test expectations: Rust lookup tests and UI workflow test for lookup-backed form fill.

### Implement basic watchlists

Intent: let the user create local watchlists and assign companies to them before feed filtering exists.

Acceptance criteria:

- User can create a watchlist.
- User can assign a company to a watchlist.
- User can remove a company from a watchlist without deleting the company.
- Company rows show current watchlist membership.
- Watchlist list shows company counts.
- Assigning the same company twice is harmless.
- Watchlists persist in SQLite.

Docs/contracts touched: product spec, contracts.

Test expectations: Rust watchlist storage tests and UI workflow tests.

### Scaffold desktop application

Intent: create the Tauri + React + TypeScript desktop shell with Rust domain modules.

Acceptance criteria:

- `flake.nix` and committed `flake.lock` provide the development environment.
- `nix develop` works on WSL2 Ubuntu 24.04.
- App build/test commands run inside `nix develop`.
- Makefile targets run automated build/test commands through `nix develop`.
- Windows hands-on sanity testing is supported by a documented PowerShell helper script.
- The experimental Windows-from-Linux packaged app sanity target is named `make package-windows-from-linux`.
- Tauri app starts on the development machine.
- React UI renders a basic investor inbox shell.
- UI supports dark and light theme selection with dark as the default.
- Initial visual tokens implement the night-neon blue, pink, and purple palette.
- Rust command `health` returns app status.
- Local build/test commands are documented.
- GitHub Actions CI skeleton runs frontend and Rust checks without secrets.
- GitHub Actions uses the same commands as local development or thin wrappers.
- GitHub Actions validates the Nix setup if it remains fast enough.
- Default CI uses standard Linux runners only and avoids larger runners, scheduled jobs, and packaging builds.
- WSL is documented as the automated test/build environment, while Windows is documented as the native hands-on GUI test environment.
- `make package-windows-from-linux` builds, copies, and launches a portable Windows `.exe` from WSL/Linux.

Docs/contracts touched: contracts, architecture.

Test expectations: Nix shell check, desktop smoke test, Rust command test, initial CI check, and Windows-from-Linux package check.

### Bootstrap docs and agent contract

Intent: create the spec-driven foundation for the project.

Acceptance criteria:

- Required docs exist under `docs/`.
- ADRs capture local-first, stack, storage, and source/AI decisions.
- `AGENTS.md` defines repo-level agent rules.
- Git repository is initialized.

Docs/contracts touched: all initial docs.

Test expectations: verify planned files exist and docs link to each other.

### Resolve open UX questions

Intent: make the first implementation plan decision-complete from the user workflow inward.

Acceptance criteria:

- Open questions in `docs/ui-flows.md` are answered or converted into ADRs.
- Company workspace navigation pattern is selected.
- Note editing format is selected.
- Claim follow-up date/quarter behavior is selected.
- Transcript editability rules are selected.
- Source status placement is selected.

Docs/contracts touched: UI flows, product spec, contracts, ADRs if needed.

Test expectations: none.

### Finalize screen-level information architecture

Intent: make v1 screens concrete enough to scaffold the UI without inventing navigation during implementation.

Acceptance criteria:

- App shell regions are defined.
- Inbox, Companies, Company Workspace, Notebooks, Transcripts, Sources, and Settings screens are specified.
- Each screen lists purpose, core regions, and primary actions.
- Deferred UI is explicitly listed.

Docs/contracts touched: UI flows, UI information architecture, product spec.

Test expectations: none.

### Finalize first data model

Intent: map UX screens and contracts to the initial local SQLite model.

Acceptance criteria:

- Core entities are documented.
- Entity relationships are documented.
- First migration scope is listed.
- Deferred data areas are explicit.

Docs/contracts touched: data model, contracts, architecture.

Test expectations: none.

### Finalize source strategy

Intent: define how v1 source adapters should be selected, fetched, normalized, and monitored.

Acceptance criteria:

- GPW ESPI/EBI source strategy is documented.
- Company matching and dedupe approach are documented.
- Source status UI requirements are documented.
- Open source questions are captured.

Docs/contracts touched: source strategy, contracts, data model, architecture.

Test expectations: none.

### Decide day-1 project practices

Intent: record project operating rules before implementation scaffolding.

Acceptance criteria:

- License posture is documented.
- Secrets, local config, data location, and logging policy are documented.
- Dependency, security, and AI policy are documented.
- Export/backup, GitHub workflow, and versioning policy are documented.
- Relevant ADRs exist.

Docs/contracts touched: project practices, ADRs, project brief, architecture, contracts, data model, product spec, roadmap, engineering workflow, agent contract.

Test expectations: none.
