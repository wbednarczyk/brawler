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

### Refine feed item metadata bar readability

Intent: make the feed item top metadata line easier to scan across official reports, public media, and future transcript items.

Acceptance criteria:

- Inbox and company feed rows separate company, item type, source, and timestamp into a clearer visual hierarchy.
- Long source names, localized labels, and compact widths remain readable without crowding the title.
- Timestamp display follows the app-wide human-readable timestamp standard.
- Saved and unread indicators do not compete with the title or metadata.
- The design works for official report rows, public media rows, and items with missing optional metadata.

Docs/contracts touched: UI information architecture and product spec if this becomes a formal cross-screen row pattern.

Test expectations: focused UI workflow or component coverage for representative feed rows after the layout pass.

### Refine Sources grouping and status hierarchy

Intent: make Sources scale beyond a flat diagnostics list as official reports, calendars, media, registry, and private research adapters accumulate.

Acceptance criteria:

- Sources are grouped by purpose: official reports, official calendar/events, public media/news, company registry, private/authenticated research, and disabled/review candidates.
- Enabled sources appear before disabled sources inside each group.
- Disabled placeholders are collapsed by default or visually de-emphasized.
- Source rows remain compact and expandable for details.
- Source health/status is visually separated from source configuration.
- Per-source refresh actions remain clear; group-level refresh can be considered later.

Docs/contracts touched: UI information architecture, product spec if grouping becomes a formal UI standard.

Test expectations: UI workflow/component coverage for grouped Sources once implemented.

### Modularize large UI, storage, and test files

Intent: reduce architecture debt from early milestone scaffolding so future features remain readable, testable, and extensible.

Acceptance criteria:

- `src/App.tsx` is split into screen-level or domain-level modules such as Inbox, Companies, Notebooks, Events, Transcripts, Settings, and shared UI helpers.
- `src-tauri/src/storage.rs` is split into domain modules such as migrations, settings, companies, feed, notebooks, events, transcripts, source adapter state, and registry operations.
- Large tests are split by workflow/domain where possible without losing shared mock setup.
- Public contracts and Tauri command behavior remain unchanged during extraction.
- Refactors are done in small slices near active feature work, not as a risky all-at-once rewrite.

Docs/contracts touched: architecture, project practices, kanban.

Test expectations: normal local check set after each extraction slice.

Design reference: [Modularization Design](modularization-design.md).

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

### Implement v1 friend-test license gate

Intent: prevent casual redistribution of v1 friend-test builds without introducing hosted accounts, telemetry, billing, or activation infrastructure.

Acceptance criteria:

- A licensing ADR records the v1 friend-test posture, threat model, and offline validation approach before implementation.
- App can validate an offline signed license key using public verification material embedded in the app.
- Private signing material and key-generation workflow stay outside the repository and build outputs.
- First-run flow or Settings lets the user enter, inspect, replace, and clear a license.
- Normal app use is gated when no valid license exists.
- Expired, invalid, tampered, wrong-version, and missing-license states are clear and recoverable.
- License validation does not require cloud accounts, telemetry, hosted activation, or billing infrastructure.
- Logs, settings export, diagnostics, and tests do not leak private signing material or full license secrets.
- Packaged v1 friend-test artifacts enforce the license gate before distribution.

Docs/contracts touched: licensing ADR, project practices, product spec, UI information architecture, contracts, release docs.

Test expectations: Rust license validation tests and UI workflow tests for entry, invalid states, expiry, and gated app access.

## Ready

## In Progress

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

Delivered:

- Bankier Gielda RSS is implemented as the first public media/news adapter and stores matched tracked-company items in the Inbox.
- Use Bankier per-company komunikaty as the active v1 official-report adapter for tracked GPW companies.
- Resolve and cache Bankier instrument slugs/tag IDs in `company_source_ids`.
- Fetch one serialized public JSON listing page per tracked company, then fetch article pages only when local detail body text is missing.
- Keep `gpw-espi-ebi` registered but disabled until a later reliability pass proves it should be re-enabled.
- Avoid browser impersonation; use the neutral app user agent and rely on low volume, cached identifiers, and cached details.
- Portal Analiz is documented by ADR 0014 and visible only as a disabled late-v1 authenticated research placeholder.
- Bankier Firma RSS and Bankier Wiadomosci RSS are visible only as disabled reviewed public RSS candidates until matching quality is proven.
- Source statuses distinguish official reports, public RSS, public JSON, authenticated placeholders, disabled sources, and manual/local registry data.
- Refresh commands reject disabled placeholders without network access.
- Feed cleanup is visible in Settings with current status, retention window, interval, protected saved items, and current-session last cleanup result.
- Final local checks passed.
- Version bumped to `0.8.0`.

Docs/contracts touched: source strategy, contracts, data model if source identifier behavior changes.

Test expectations: parser/fetcher test-sample tests, source adapter contract tests, storage dedupe tests, and source-status UI label coverage.

## Review

No cards.

## Done

### M10.13 Close M10

Intent: finish milestone documentation and versioning only after the packaged app workflow works end to end.

Acceptance criteria:

- A Windows packaged app run can create a transcript job from a real YouTube URL using real Gemini credentials configured through the app or an approved local runtime path.
- The app stores returned transcript segments and shows the completed transcript in the Transcripts view.
- User can select transcript material and save it as an editable Markdown note.
- Roadmap M10 is marked completed.
- Kanban M10 cards are moved to Done.
- Version is bumped to `0.10.0` in all required files.
- Required automated checks pass.
- Live Gemini transcript generation has been manually smoke-tested with a real supported YouTube URL.

Delivered:

- Removed the temporary keyring diagnostic UI and command before closure.
- Updated transcript contracts and product docs to match auto-start, Retry, editable transcript descriptions, searchable transcript text, and optional company binding.
- Recorded successful live Gemini smoke validation for `gemini-2.5-flash`.
- Recorded successful packaged Windows app validation with real Gemini transcription after selecting `gemini-3.5-flash`.
- Version bumped to `0.10.0` in package, Rust, lock, Tauri config, and Rust health-test files.
- Final automated checks passed.

Docs/contracts touched: roadmap, kanban, contracts, product spec.

Test expectations: normal local check set plus documented live Gemini smoke evidence and manual Windows packaged-app validation.

### M10.12 Add live Gemini smoke path

Intent: make the real-provider exit criterion repeatable without adding live Gemini to default checks.

Acceptance criteria:

- A documented manual or opt-in smoke procedure runs one real YouTube transcription through `provider_gemini`.
- The smoke path records whether credentials are configured and whether transcript segments were created.
- The smoke path is excluded from default CI and normal local checks.
- M10 cannot be closed until this smoke check has passed at least once on the feature branch.

Delivered:

- Added ignored Rust smoke test `live_gemini_transcribes_youtube_url`.
- Added `make smoke-gemini-transcript`.
- Smoke command requires `GEMINI_API_KEY` and `BRAWLER_GEMINI_SMOKE_YOUTUBE_URL`.
- Optional `BRAWLER_GEMINI_SMOKE_MODEL` can validate alternatives.
- Smoke output records the model and transcript segment count.
- Default Rust tests ignore the live smoke test.
- Added [Live Smoke Tests](live-smoke-tests.md) with command, expected result, failure interpretation, and M10 closure rule.

Docs/contracts touched: engineering workflow, live smoke docs, kanban.

Test expectations: normal automated checks remain mock/sample based; live smoke is manual/opt-in.

### M10.11 Implement live Gemini transcript generation

Intent: make M10 exit with working transcript generation against the real Gemini API, not only offline sample output.

Acceptance criteria:

- `provider_gemini` sends supported public YouTube URLs to Gemini from the Rust side.
- Gemini transcription model is selectable in Settings and defaults to the cheapest/fastest configured model expected to support YouTube/video transcription.
- Gemini output is requested as transcript-like structured segments with text, language, and timestamp data when available.
- Provider success stores immutable transcript segments and marks the job completed.
- Provider failure maps missing credentials, quota/limit, provider, network, invalid URL, URL rejection, and parse errors to existing job failure fields with useful cause text when the provider returns it.
- The offline `test_sample` provider remains available for automated tests and development, but cannot satisfy M10 closure.
- No Gemini API key or full transcript body is written to logs.

Delivered:

- Added real `provider_gemini` HTTP execution through the Rust backend using the Gemini `generateContent` endpoint.
- Added direct YouTube URL request construction using Gemini file data.
- Added structured JSON transcript prompt and parser that stores immutable transcript segments.
- Added provider error mapping for missing credentials, limits, network failures, invalid URLs, URL/request rejection, provider errors, and parse failures.
- Changed the user-facing transcript run action from `Run sample` to real Gemini execution; created jobs now auto-start when credentials are configured, and failed/queued job action is labeled `Retry`.
- Changed backend default run mode to `provider_gemini`; `test_sample` remains available only when explicitly requested by tests/development.
- Added selectable Gemini transcription model in Settings.
- Default model is `gemini-2.5-flash`, the cheapest configured candidate that passed M10.12 live smoke validation.
- Added configurable Gemini transcription timeout in Settings. Default is `300` seconds, with `45`, `90`, `180`, `300`, and `600` seconds accepted.
- Added Settings action linking to Google AI Studio API-key creation.
- Removed the temporary in-app Gemini keyring diagnostic after Windows Credential Manager validation; normal runtime behavior is credential status, save, replace, clear, and provider execution.
- Added migration `0019_youtube_transcription_model.sql`.
- Tests cover mocked Gemini response parsing, invalid URL rejection, HTTP error cause extraction, missing credentials, Settings model selection, and Gemini run invocation.
- Live smoke validation passed for `gemini-2.5-flash` with 27 transcript segments from a real YouTube URL.
- Packaged Windows app validation produced a successful real Gemini transcript after selecting `gemini-3.5-flash`.

Docs/contracts touched: contracts, architecture, product spec, UI information architecture, kanban.

Test expectations: Rust provider mapping tests using mocked/sample Gemini responses; no default test requires live Gemini.

### M10.10 Implement Gemini credential settings

Intent: let the app use real Gemini credentials without exposing secrets to React or storing secrets in SQLite/YAML.

Acceptance criteria:

- Settings shows whether YouTube transcription credentials are configured.
- User can save, replace, and clear the Gemini API key for YouTube transcription.
- Runtime secret storage uses the OS keychain.
- `.env` or environment-variable fallback is allowed only for local development and tests.
- Frontend never receives the API key value.
- Missing credentials produce a clear recoverable state before live transcription is attempted.

Delivered:

- Added a reusable Rust credential boundary with provider, purpose, secret kind, storage, status, and development-environment fallback metadata.
- Added Gemini YouTube transcription API-key status, save/replace, clear, and read paths.
- Runtime credential storage uses the OS keychain through the `keyring` crate.
- `GEMINI_API_KEY` is accepted only as a development/test fallback and is reported separately from OS-keychain storage.
- Added typed Tauri commands for Gemini transcription credential status, save, and clear.
- Settings now shows credential status, storage, secret kind, and save/clear controls without exposing the secret value.
- Docs now record the broader credential model for future API keys, username/password credentials, session tokens, and other secret forms.
- Tests cover non-secret credential metadata, empty secret rejection, and Settings save/clear UI command behavior.

Docs/contracts touched: contracts, project practices, architecture, UI information architecture, kanban.

Test expectations: Rust command/storage tests with mock-safe credential behavior; UI tests for configured/not-configured states.

### M10.9 Polish transcript workflow reliability and UX

Intent: make the first video-to-notebook workflow coherent enough for repeated manual testing.

Acceptance criteria:

- Validation errors for invalid/missing YouTube URLs are immediate and clear.
- Duplicate job behavior for the same company and URL is defined and implemented.
- Long transcripts remain navigable and do not make the UI sluggish.
- Job errors and provider limit states are visible without losing saved work.
- Buttons, icons, labels, and expandable patterns match current app conventions.

Delivered:

- Missing URL keeps the create button disabled.
- Invalid non-YouTube URLs show immediate, specific validation feedback.
- Duplicate transcript job creation is defined: same URL and same company scope returns the existing job.
- Unlinked and company-linked transcript jobs remain separate duplicate scopes.
- Transcripts UI deduplicates job rows defensively by job ID after create/list refresh.
- Failed transcript jobs show expanded provider diagnostics with `errorCode` and stored error text.
- Retry action remains visible for failed jobs.
- Transcript jobs can be deleted from the Transcripts list, with stored segments removed through cascade delete.
- Transcript segment lists remain constrained in a scrollable review area.
- Tests cover URL validation, duplicate create behavior, provider error display, transcript delete behavior, and duplicate URL/company-scope storage behavior.

Docs/contracts touched: contracts, kanban.

Test expectations: focused UI tests for validation/error states and storage tests for duplicate behavior.

### M10.8 Create editable notebook note from selected transcript segments

Intent: complete the transcript-to-note loop with editable Markdown notes linked back to transcript origin.

Acceptance criteria:

- User can create a note draft from selected transcript segments.
- Note creation is blocked until the transcript job has a resolved company.
- Draft title/body/tags/kind/follow-up fields are editable before saving.
- Save creates a normal notebook entry for the company.
- Saved note origin links include transcript job, selected segment IDs, original YouTube URL, provider, and timestamp ranges when available.
- The original transcript segment text remains immutable.

Delivered:

- Added `create_note_from_transcript_selection` as a Tauri command and storage operation.
- Backend validation rejects unlinked transcript jobs for company notebook note creation, non-completed jobs, empty segment selections, and unknown segment IDs.
- Transcript-derived notes are saved as normal Markdown notebook entries.
- Each selected segment is stored as a `transcript_segment` origin with segment ID, video URL, and job/provider/timestamp context.
- Transcripts UI can open an editable note draft from selected segments and save it.
- Unlinked transcripts remain viewable, and expanded transcript jobs provide optional company linking before company notebook note creation.
- UI workflow tests cover segment selection to saved note.
- Rust storage tests cover successful origin creation and unlinked-transcript rejection for company notebook note creation.

Docs/contracts touched: contracts, data model, product spec, UI flows, kanban.

Test expectations: Rust storage tests for note origin links and UI workflow tests for segment selection to saved note.

### M10.7 Add transcript segment review UI

Intent: let the user inspect returned transcript segments without editing source transcript text.

Acceptance criteria:

- Completed jobs show transcript segments in chronological order.
- Segment text is read-only.
- Timestamp ranges are shown when available.
- User can select one or more whole segments for note creation.
- Selection remains usable for long transcript lists.

Delivered:

- Transcript job rows now use the app-wide expandable-row behavior.
- Completed jobs load and display transcript segments inline.
- Segment text is read-only and timestamp ranges are shown in a compact `m:ss-m:ss` format.
- Users can select multiple whole segments for the upcoming note-creation workflow.
- Long segment lists are constrained in a scrollable review area.
- UI workflow tests cover completed-job segment display and multi-segment selection.

Docs/contracts touched: UI information architecture, kanban.

Test expectations: UI workflow tests for completed job segment display and multi-segment selection.

### M10.6 Implement provider runner with test-sample fallback

Intent: connect the job workflow to a provider abstraction while keeping default checks offline.

Acceptance criteria:

- A provider interface exists for YouTube transcript extraction.
- Gemini implementation is isolated behind that interface.
- Provider output can return company recognition candidates when available.
- Automated tests use test samples/mocks and never require live Gemini credentials.
- Live provider execution runs when the user creates a configured job or retries a queued/failed job.
- Provider errors are stored on the job and displayed in UI.

Delivered:

- Added a transcript provider interface with provider output and provider error mapping.
- Added an offline test-sample transcript provider that produces immutable transcript segment drafts.
- Added an isolated unconfigured Gemini provider implementation that fails with `provider_not_configured` until credential wiring is implemented.
- Added `run_video_transcript_job` command with explicit `providerMode`.
- Running a queued job stores sample transcript segments and marks the job completed.
- Provider failures are persisted on the transcript job with `status = failed`, `errorCode`, and user-readable error.
- The early sample run action was replaced by live Gemini execution in later M10 slices; `test_sample` remains an internal test/development provider only.
- Tests cover provider contracts and UI runner command behavior without live credentials or network.

Docs/contracts touched: contracts, kanban.

Test expectations: provider contract tests with test samples and job failure tests.

### M10.5 Build transcript job UI shell

Intent: add the first user-visible workflow for submitting a YouTube URL for a selected company and seeing job status.

Acceptance criteria:

- User can open a transcript workflow from Notebooks or a company workspace.
- User enters a YouTube link in a field labeled `URL`.
- User may optionally provide a ticker/company before transcription.
- If no company is provided, the job remains visible as an unlinked transcript.
- Company selection through the cached company lookup becomes mandatory only before saving selected transcript segments into a company notebook.
- Job status is visible and refreshable.
- Empty, queued, running, completed, and failed states are readable and compact.

Delivered:

- Transcripts screen now has a `URL`-first job creation form.
- Company/ticker is optional before job creation and uses local tracked-company suggestions.
- Invalid non-YouTube URLs are rejected before command execution.
- Created jobs call `create_video_transcript_job` with nullable company ID and configured provider ID.
- Existing jobs are refreshable and rendered as compact status rows.
- Queued/completed/failed states have visible status indicators.
- Unresolved company jobs remain visible and ready for the later company-resolution/provider slices.

Docs/contracts touched: kanban.

Test expectations: UI workflow test for opening the workflow, submitting a URL with a mocked command, and seeing status.

### M10.4 Add Gemini settings and provider disclosure UI

Intent: make the provider configuration explicit and privacy-visible before any live provider call.

Acceptance criteria:

- Settings exposes Gemini YouTube transcription configuration status.
- Provider disclosure explains that YouTube URL/video content is sent to Gemini when the user starts a transcript job.
- Secret handling follows the OS keychain contract; `.env` remains development/test-only.
- Missing provider configuration is a clear recoverable state.
- General AI analysis remains provider-neutral and separate from this setting.

Delivered:

- Settings shows Gemini as the selected YouTube transcription provider using the canonical `provider_gemini` ID.
- Settings shows YouTube transcription credentials as `Not configured` until OS-keychain secret storage is implemented.
- Settings discloses that starting a transcript job sends the YouTube URL and video content to Gemini.
- General AI provider remains separate and unconfigured.
- Migration `0018_youtube_transcription_provider_id.sql` upgrades older local settings from `gemini` to `provider_gemini`.
- Storage validates future YouTube transcription provider updates against accepted provider IDs.

Docs/contracts touched: contracts, kanban.

Test expectations: UI/settings workflow tests for configured and missing-provider states using mocks.

### M10.3 Add transcript command boundary and frontend types

Intent: expose transcript storage through typed Tauri commands before building UI.

Acceptance criteria:

- `create_video_transcript_job` command exists with typed input/output.
- Command input requires `URL`/YouTube source URL and allows nullable company ID.
- `list_video_transcript_jobs` or equivalent job listing command exists for company-scoped UI.
- `list_transcript_segments` command exists for a job.
- Frontend TypeScript types match the command contracts.
- Command tests/mocks cover success and validation errors.

Delivered:

- Tauri commands now expose transcript job creation, transcript job listing, transcript segment listing, and transcript job company resolution.
- Frontend has typed transcript job and segment read models.
- Transcripts placeholder loads local transcript jobs through `list_video_transcript_jobs` and displays command-backed job count/status.
- UI command mocks cover transcript job list/create, segment list, and company resolution.

Docs/contracts touched: kanban.

Test expectations: Rust command/storage coverage and UI command mock coverage.

### M10.2 Add transcript storage foundation

Intent: create migration-managed local storage for transcript jobs and immutable transcript segments.

Acceptance criteria:

- SQLite migration creates transcript job and segment tables.
- Jobs preserve company, provider, YouTube URL, status, timestamps, and error text.
- Segments preserve company, job, optional timestamp range, optional speaker, language, text, and creation time.
- Segment text is not updated through normal storage APIs after creation.
- Storage can create/list jobs and create/list segments.

Delivered:

- Migration `0017_transcript_storage_foundation.sql` upgrades transcript tables to the accepted M10 contract.
- Transcript jobs now support nullable company identity, company-resolution status, recognition candidates, source label, status, error code, timestamps, and error text.
- Transcript segments now support nullable company identity and remain linked to their parent job.
- Storage methods can create/list transcript jobs and create/list transcript segments.
- Segment text is protected by a SQLite trigger so source transcript text cannot be updated in place.
- Rust tests cover job creation/listing, segment creation/listing, company inheritance, migration count, and segment immutability.

Docs/contracts touched: kanban.

Test expectations: Rust migration/storage tests for job lifecycle, segment insertion/listing, and immutability.

### M10.1 Finalize transcript contracts and data model

Intent: make the M10 storage and command surface explicit before implementation.

Acceptance criteria:

- `docs/contracts.md` defines transcript job, transcript segment, and transcript-to-note command payloads with the fields needed for v1.
- `docs/data-model.md` defines `transcript_jobs`, `transcript_segments`, and note-origin links to transcript segments.
- Provider selection is documented as Gemini-only for YouTube transcription, not a default for general AI analysis.
- Transcript source text immutability is documented.
- Transcript jobs support URL-first creation with optional company/ticker.
- Company resolution statuses cover provided, recognized, unresolved, and needs-user-selection states.
- Error/status values are named before UI work starts.

Delivered:

- Transcript job and segment contracts now allow URL-first jobs with optional company identity.
- Create-job and resolve-company command payloads are documented.
- Job processing statuses, company-resolution statuses, and provider error codes are documented.
- Data model documents nullable transcript company identity, company-resolution fields, immutable segments, and transcript note-origin requirements.

Docs/contracts touched: contracts, data model, product spec, UI flows, UI information architecture, kanban.

Test expectations: none beyond docs review for this slice.

### M9.7 Close M9

Intent: finish milestone documentation and versioning after implementation is complete.

Acceptance criteria:

- Roadmap M9 is marked completed.
- Kanban M9 cards are moved to Done.
- Version is bumped to `0.9.0` in all required files.
- Required automated checks pass.

Delivered:

- Roadmap M9 is marked completed.
- M9 implementation cards are in Done.
- Version bumped to `0.9.0` in package, Rust, lock, and Tauri config files.

Docs/contracts touched: roadmap, kanban.

Test expectations: normal local check set.

### M9.1 Implement event storage contract and SQLite foundation

Intent: define the canonical local event record and create the SQLite storage foundation for M9.

Acceptance criteria:

- `docs/contracts.md` defines the Company Event read model and create input.
- `docs/data-model.md` defines `company_events`.
- A migration creates the `company_events` table and indexes.
- Rust storage can create and list events.
- Event records preserve source URL, attribution, fetched timestamp, and manual/source distinction.
- Storage tests cover create/list behavior and source-event dedupe.

Delivered:

- `company_events` SQLite table and indexes were added.
- Rust storage can create and list company events.
- Tauri command boundary exists for listing and creating events.
- Storage tests cover manual event creation/listing and sourced-event dedupe.

Docs/contracts touched: contracts, data model, kanban.

Test expectations: Rust storage and migration tests.

### M9.1a Implement GPW Market Events RSS adapter foundation

Intent: use the official GPW market-events RSS feed as the first real event source for M9.

Acceptance criteria:

- `gpw-market-events-rss` source adapter is registered as an enabled official calendar RSS source.
- Parser extracts event date, market section, event label, instrument type, ticker, title, source URL, and stable source key.
- Storage ingestion creates `company_events` only for tracked companies matched by exact ticker.
- Unmatched RSS items are counted as diagnostics and do not create visible events.
- Repeated refreshes dedupe by source adapter and source event key.
- Tests use RSS test samples and do not require live GPW access.

Delivered:

- `gpw-market-events-rss` is registered as an enabled official-calendar RSS adapter.
- Parser converts GPW market-events RSS items into event candidates.
- Refresh path ingests GPW market events into `company_events` for tracked exact-ticker matches.
- Repeated ingestion dedupes by source event key.

Docs/contracts touched: source strategy, contracts, data model, kanban.

Test expectations: parser and storage ingestion tests.

### M9.2 Add event command boundary and frontend types

Intent: expose the event storage model through typed Tauri commands and frontend TypeScript types before building UI.

Acceptance criteria:

- Frontend can list company events through a typed command.
- Frontend can create a manual event through a typed command.
- Command names and payloads match `docs/contracts.md`.
- No external source fetching is introduced in this slice.

Delivered:

- `list_company_events` and `create_company_event` are exposed through Tauri.
- Frontend `CompanyEvent` types and command calls are wired into the app.
- Rust storage tests and UI command mocks cover the boundary.

Docs/contracts touched: contracts.

Test expectations: Rust command/storage tests and UI command mocks.

### M9.3 Build Events screen shell with upcoming default view

Intent: add the first visible Events screen using local event data.

Acceptance criteria:

- Primary navigation includes Events.
- Events screen shows upcoming events by default.
- Empty state is clear when no events exist.
- Event rows show date, company ticker, event type, source/manual marker, and status.
- Rows use the app-wide expandable-detail pattern.

Delivered:

- Events is a primary navigation section.
- Event rows/cards use the app-wide click-to-expand pattern.
- Event details show company, type, status, source, attribution, fetched timestamp, and source link where available.

Docs/contracts touched: product spec, roadmap, kanban.

Test expectations: UI workflow test for opening Events and viewing event rows.

### M9.4 Add event filtering and history mode

Intent: make the Events screen useful across many watchlists and companies.

Acceptance criteria:

- User can switch between upcoming, historical, and combined date ranges.
- Events opens in a current-week view by default, with working-day columns and previous/next/current week controls.
- The existing broader date-range workflow remains available as a secondary list view.
- User can filter by watchlist, company, event type, and status.
- Due-soon grouping or highlighting is visible for upcoming events.
- Filters remain compact and consistent with Inbox/Notebooks behavior.

Delivered:

- Events defaults to a working-day week view with previous, next, and current-week navigation.
- List view provides upcoming, historical, all, and custom date-range modes.
- Watchlist, company, event type, and status filters are implemented.
- Today/soon/past event highlighting is visible in week and list rows.

Docs/contracts touched: product spec, roadmap, kanban.

Test expectations: UI workflow tests for filters and event highlighting.

### M9.5 Add manual event creation and source update workflow

Intent: allow the user to add missing events while sourced data changes are handled by source refresh.

Acceptance criteria:

- User can create manual events for a selected company.
- Manual events are visually distinct from sourced events.
- Source-keyed event refresh updates the existing sourced row when the source changes the event.
- Event editor uses the existing compact date picker behavior.

Delivered:

- Manual event creation is implemented in the Events screen.
- Manual events are visibly marked and styled separately from sourced events.
- Source-keyed upsert updates existing sourced events instead of creating correction rows.
- Sourced event correction UI was removed; source refresh owns source-backed updates.

Docs/contracts touched: contracts, data model, product spec, kanban.

Test expectations: storage tests for source-keyed updates and UI workflow tests for manual events.

### M9.6 Add active sourced event ingestion

Intent: populate events from accepted real-world source adapters after the storage and UI workflow are stable.

Acceptance criteria:

- GPW Market Events RSS is implemented as an official calendar event source.
- Bankier Kalendarium HTML is implemented as an active public calendar source for broader company-calendar coverage.
- Hidden/empty Bankier calendar RSS endpoints are not treated as reliable until direct checks prove stable populated content.
- Report publication dates can create sourced events where the source data supports it.
- Dividend-related dates are added only from accepted reliable sources.
- Source URL, attribution, fetched timestamp, and source event key are preserved.
- Source-keyed upsert prevents duplicate sourced events on repeated refreshes and updates changed sourced event fields.

Delivered:

- GPW Market Events RSS is active and creates sourced events for tracked exact-ticker matches.
- Bankier Kalendarium is active and creates public-calendar sourced events for tracked exact-ticker matches.
- Events week navigation fetches Bankier dated calendar pages on demand in the background after showing cached local data.
- Strefa report calendar and Money calendar remain disabled public-calendar candidates.
- Hidden/empty Bankier calendar RSS endpoints remain rejected/unproven in source strategy.
- Source URL, attribution, fetched timestamp, and source event key are preserved for accepted event sources.
- Source-keyed upsert behavior has regression tests.

Docs/contracts touched: source strategy, contracts, data model, kanban.

Test expectations: adapter/storage tests using test samples only.

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
