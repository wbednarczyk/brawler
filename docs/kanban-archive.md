# Kanban Archive

Historical completed cards moved out of the active Kanban board to keep agent context smaller. Active work remains in [Kanban](kanban.md).

## Done

### M29: Research questions and evidence links

Intent: add durable company-scoped research questions and typed evidence-link workflows inside the Research workspace.

Delivered:

- Added durable research questions stored outside notebooks.
- Added company-scoped question creation inside the Research screen.
- Added question status workflow for open, answered, closed, and reopen states.
- Represented research questions as timeline evidence items.
- Added selected-question evidence linking and unlinking from visible Research rows.
- Added backend validation for question-to-evidence links.
- Added research import/export support for questions and evidence links.
- Added regression coverage for question creation, linking, import/export, and question-row navigation.
- Updated docs, contracts, data model, UI architecture, and ADR checkpoint notes.
- Bumped app version to `0.29.0`.

ADR checkpoint: Updated [ADR 0022](adr/0022-research-evidence-read-model-boundary.md); no new ADR required because the work implements the accepted research/evidence boundary.

Validation:

- `make check` passed.
- `rtk npm test -- ResearchScreen.test.tsx --run` passed.
- `rtk npm run typecheck` passed.
- `git diff --check` passed.

### M28: Cross-platform release artifacts

Intent: publish practical public release binaries from standard Linux infrastructure while keeping Radicle as the canonical forge.

Delivered:

- Added Linux `amd64` release artifact targets for `.deb`, `.rpm`, and `.AppImage`.
- Added a Windows `x64` portable zip artifact target built from Linux/WSL with `cargo-xwin`.
- Added a tag-triggered GitHub Release workflow on `ubuntu-latest` that publishes release artifacts for `v*` tags.
- Kept Radicle as the canonical source forge while using GitHub Releases as the public binary mirror.
- Set Linux release-build runtime data to `~/.brawler`.
- Kept Windows portable runtime data beside `brawler.exe` in `data/`.
- Documented the release artifact ADR, packaging smoke checks, Makefile targets, and future macOS/native package extension points.
- Cleaned active Radboard titles so epics use `epic: <title>` and active tasks use meaningful action-oriented names without milestone numbering prefixes.
- Bumped app version to `0.28.0`.

ADR checkpoint: Added [ADR 0024](adr/0024-cross-platform-release-artifacts.md).

Validation:

- `make package-release-artifacts` passed.
- `make check` passed.
- `make release-check` passed.
- Artifact inspection passed for `.deb`, `.rpm`, `.AppImage`, and Windows portable zip contents.

Deferred:

- native Pacman package generation
- Windows installer packaging
- code signing
- package repositories
- macOS and macOS arm64 packaging
- paid or larger CI runners

### M25: Company evidence timeline and release workflow

Intent: deliver the first visible Research workspace slice and make release/version workflow predictable for future milestones.

Delivered:

- Added a top-level Research navigation entry and company-scoped Research screen.
- Added backend timeline results with evidence items plus summary counts for total evidence, changed-since-review evidence, and last-reviewed timestamp.
- Added backend-owned evidence type filtering and changed-only filtering.
- Added company-level `Mark reviewed` checkpoint workflow.
- Rendered compact evidence rows with product-language labels, translated AI/event labels, changed state, attribution, and owning-domain/source actions.
- Fixed Research feed-item navigation so it opens the selected Inbox item without hiding it through the wrong company filter.
- Added Research workflow regression tests for navigation, filtering, raw identifier leaks, and feed-item opening.
- Added backend research tests for timeline summary, review state, evidence filters, and changed-only behavior.
- Added SemVer and Conventional Commit release workflow docs, local commit-message hook validation, git-cliff changelog configuration, and release validation commands.
- Curated the retroactive changelog baseline through `0.24.1`.
- Bumped app version to `0.25.0`.

ADR checkpoint: no new ADR required for M25 closure; M25 implements accepted Research/read-model decisions already captured in docs and the release workflow decisions captured in release workflow docs.

Validation:

- `rtk npm run typecheck` passed.
- `rtk npm test` passed, 110 tests.
- `rtk npm run build` passed.
- `make rust-check` passed.
- `rtk git diff --check` passed.
- Manual Research UI checks passed, including feed-item navigation from Research into Inbox.

### Patch 0.24.1: Multi-registry company directory hardening

Intent: make NewConnect and future company-directory sources work through shared company lookup, company creation, and company-owned workflows without hard-coded GPW/NC assumptions.

Delivered:

- Fixed company lookup and Companies UI add flow so NewConnect companies can be added from lookup results.
- Generalized company-directory bootstrap and stale checks to all enabled `company_registry` adapters.
- Generalized public-media matching to all tracked companies.
- Added an exchange-aware source-listing matching helper for future report/event adapters.
- Added regression coverage for future exchange lookup/create, Companies UI add, watchlists, notebooks, manual events, import/export, media matching, and source-listing matching.
- Documented the checklist for adding the next company-directory source.
- Bumped app version to `0.24.1`.

ADR checkpoint: no new ADR required; this hardens the existing source-directory and modularity decisions.

### M24: Modularization readiness and research workspace architecture

Intent: refactor the boundaries required before research-workspace feature implementation, without adding visible research-workspace features or unrelated broad cleanup.

Delivered:

- Added [ADR 0022](adr/0022-research-evidence-read-model-boundary.md) for the durable research/evidence read-model boundary.
- Added focused frontend research API/type modules: `src/api/research.ts` and `src/api/researchTypes.ts`.
- Added Rust research command and storage modules: `src-tauri/src/commands/research.rs` and `src-tauri/src/storage/research.rs`.
- Added migration `0030_research_evidence_boundary.sql` for durable review checkpoints and typed evidence links.
- Added backend-owned research evidence/timeline read models assembled from canonical domain tables.
- Kept full stored timeline/evidence projections deferred behind the research API until performance or review semantics require them.
- Added typed commands for research evidence, company/watchlist timelines, review checkpoints, and evidence links.
- Added storage tests for evidence aggregation, review-state updates, watchlist review state, idempotent links, and missing-reference validation.
- Recorded the large-file responsibility audit with split/defer/no-action decisions.
- Updated product spec, contracts, data model, UI information architecture, modularization design, AI analysis framework, roadmap, and Kanban.
- Added the post-M24 research implementation sequence as M25-M29 backlog cards.
- Bumped app version to `0.24.0`.

ADR checkpoint: Added [ADR 0022](adr/0022-research-evidence-read-model-boundary.md).

Validation:

- User manually signed off M24 closure.
- `rtk npm run typecheck` passed.
- `rtk npm test -- --run` passed.
- `rtk npm run build` passed.
- `rtk cargo fmt --check` passed.
- `rtk cargo clippy --all-targets -- -D warnings` passed.
- `rtk cargo test` passed.
- `rtk git diff --check` passed.

### M23: Browser UI regression testing assessment and first Playwright slice

Intent: add a small opt-in Playwright browser UI regression smoke path focused on layout problems that Vitest/jsdom cannot reliably detect.

Delivered:

- Added an opt-in Playwright browser UI smoke suite with Chromium-only first coverage.
- Added deterministic browser-smoke data so UI smoke does not use live sources or the user's app data.
- Added `npm run test:browser:install`, `npm run test:browser`, `make ui-smoke-install`, and `make ui-smoke`.
- Added regression coverage for fixed app chrome, absence of global app scrolling, Companies list scrolling/height, Notebooks pane scrolling, Sources compact rows, Watchlists scrolling, and basic navigation.
- Added [ADR 0021](adr/0021-browser-ui-regression-testing.md) for the browser UI regression testing boundary.
- Documented setup, commands, failure-only artifacts, and the WSL/Vite-preview versus native-Windows testing split.
- Kept Playwright opt-in; default `make check` and default frontend tests remain Vitest/jsdom based.
- Bumped app version to `0.23.0`.

ADR checkpoint: Added [ADR 0021](adr/0021-browser-ui-regression-testing.md).

Validation:

- User manually signed off M23 closure.
- `rtk npm run typecheck` passed.
- `rtk npm test -- --run` passed.
- `rtk npm run build` passed.
- `rtk npm run test:browser` passed.
- `rtk cargo fmt --check` passed.
- `rtk cargo clippy --all-targets -- -D warnings` passed.
- `rtk cargo test` passed.

### M22: Sources trust, control, and company-directory extensibility

Intent: make Sources useful as a normal-user trust/control surface while preserving developer/author visibility for source candidates and implementation detail.

Delivered:

- Reworked normal Sources to show only implemented required/optional sources with normal-user status depth.
- Moved unimplemented source candidates and review-only details out of normal Sources and into Developer Diagnostics plus docs.
- Added source visibility tiers to the source read model: required, optional, and developer.
- Added typed optional-source enable/disable support with required-source and developer-candidate protection.
- Made batch/manual/scheduled source refresh respect persisted optional source enablement.
- Moved company-directory refresh commands to the async source-refresh task boundary so long live refreshes do not block the app UI.
- Default source classification:
  - required: GPW company directory / registry support, NewConnect company directory support.
  - optional, default enabled: Bankier Company Komunikaty, Bankier Giełda RSS, GPW market events RSS, Bankier Kalendarium.
  - candidate/developer-only: GPW ESPI/EBI, Portal Analiz, Bankier Firma RSS, Bankier Wiadomości RSS, Strefa report calendar, Money calendar.
- Reframed GPW registry read-model and normal UI copy as GPW company directory / lookup support.
- Implemented the NewConnect company-directory source from the official NewConnect company list and exposed it as a required normal source.
- Kept GPW and NewConnect directory company lists separated in each source detail panel while preserving shared lookup/cache behavior.
- Added a deterministic exchange-color strategy so `GPW:`, `NC:`, and future market prefixes stay visually distinct.
- Derived simple source health for normal UI: healthy, needs attention, not refreshed yet, and off.
- Removed unmatched source-item diagnostics from normal Sources.
- Added source-candidate study documentation covering current and future source candidates before candidate promotion.
- Added company-directory architecture documentation for NewConnect and later company-directory sources.
- Added ADR 0020 for source visibility tiers, Developer-mode candidate visibility, optional enablement, and company-directory boundaries.
- Updated product, UI IA, source strategy, contracts, and data model docs.
- Added/updated regression tests for normal UI hiding candidates, Developer mode candidate visibility, optional source enablement persistence, required-source protection, forbidden normal-user implementation terms, Sources workflows, ticker exchange colors, and Sources layout/scroll/selected-row styling.
- Bumped app version to `0.22.0`.

ADR checkpoint: Added [ADR 0020](adr/0020-sources-visibility-and-directory-boundaries.md).

Validation:

- User manually signed off M22 closure.
- `rtk npm run typecheck` passed.
- `rtk npm test -- --run` passed.
- `rtk npm run build` passed.
- `rtk cargo fmt --check` passed.
- `rtk cargo clippy --all-targets -- -D warnings` passed.
- `rtk cargo test` passed.
- `rtk git diff --check` passed.

### M21: Portable Windows executable candidate

Delivered:

- Added ADR 0019 for portable-only Windows candidate packaging and executable-adjacent data policy.
- Added portable Windows data-directory mode and hardened WSL/native Windows packaging helpers.
- Added GUI-subsystem release executable behavior, README quickstart, package smoke checklist, and documented deferred release automation/installer scope.
- Bumped app version to `0.21.0`.

Validation:

- User manually signed off M21 closure.
- `make package-windows-from-linux` produced `brawler-0.21.0-windows-x64-portable.exe`.
- Packaged artifact was verified as a Windows GUI executable and created executable-adjacent `data/` storage.
- Required closure checks passed before version bump.

### M20: Import and export companies, watchlists, notebooks, and settings

Delivered:

- Added JSON research-data export/import for companies, watchlists, memberships, and notebook entries.
- Added YAML settings export/import for allowlisted non-secret preferences.
- Added import preview, transactional apply behavior, merge semantics, file picker filters, and ADR 0018.
- Bumped app version to `0.20.0`.

Validation:

- User manually reviewed and signed off M20.
- Frontend typecheck/test/build passed.
- Rust fmt, clippy, and tests passed.

### M19: Dedicated watchlist management

Delivered:

- Added backend watchlist rename/delete lifecycle commands and a dedicated Watchlists panel.
- Removed watchlist mutation controls from Companies while preserving membership context and cross-view filtering.
- Added UI regression guardrails for fixed chrome, scroll regions, and normal-user copy.
- Bumped app version to `0.19.0`.

Validation:

- User manually reviewed and signed off M19.
- Frontend typecheck/test/build passed.
- Rust fmt, clippy, and tests passed.

### M18: Implement V1 application polish

Delivered:

- Repaired Notebooks, Inbox, Sources, Settings, Companies, shell/sidebar, topbar, scrolling, selected-row, and architecture-copy polish.
- Added shared ticker rendering, app themes, watchlist filters, locale coverage, docs/contracts updates, and focused workflow tests.
- Bumped app version to `0.18.0`.

Validation:

- User manually reviewed and signed off M18.
- Frontend typecheck/test/build passed.
- Rust fmt, clippy, and tests passed.

### M17: Implement local entitlement gate

Delivered:

- Added ADR 0017 for the local entitlement gate.
- Added extensible license parsing, verification, entitlement policy, OS keychain storage, redacted metadata, typed commands, UI gate/settings flows, owner tooling, and license operations docs.
- Bumped app version to `0.17.0`.

Validation:

- Manual UI license gate and token-generation testing passed by user.
- Frontend typecheck/test/build passed.
- Rust licensing tests, fmt, clippy, and full tests passed.

### M16: Implement local metrics exposure

Delivered:

- Added a dedicated local metrics module with typed samples, runtime counters, collector registry, on-demand snapshots, and Developer Diagnostics presentation.
- Kept collector and presentation/export boundaries ready for future Prometheus/OpenTelemetry/file adapters without adding remote exposure.
- Bumped app version to `0.16.0`.

Validation:

- Focused metrics tests, frontend typecheck/test/build, Rust fmt, clippy, and full tests passed.

### Documentation Context Optimization

Delivered:

- Added Kanban Archive and moved completed-card history out of active Kanban.
- Updated AGENTS, Project Brief, and canonical docs routing so future agents load less unrelated context.

Validation:

- Confirmed active Kanban routes completed-card history to Kanban Archive and that product/contract/security/testing requirements were unchanged.

### M15: Implement local logs framework

Delivered:

- Added local JSON Lines runtime logging, log directory initialization, configurable rotation, redaction, Settings controls, Developer Diagnostics log viewer, and typed commands.
- Updated ADR 0015 and related docs.
- Bumped app version to `0.15.0`.

Validation:

- Focused storage/observability/logging/job tests, frontend typecheck/test/build, Rust fmt, clippy, full tests, and diff check passed.

### M14: Implement developer mode diagnostics framework

Delivered:

- Added ADR 0015 for local observability and Developer mode policy plus ADR 0016 for provider-neutral AI analysis diagnostics.
- Added persisted Developer mode, diagnostics storage/redaction/retention, typed commands, developer-only Diagnostics UI, and first AI/source/credential diagnostic producers.
- Bumped app version to `0.14.0`.

Validation:

- Focused diagnostics/source-adapter tests, frontend typecheck/test/build, Rust fmt, clippy, and full tests passed.

### M13.8 Close M13

Intent: verify M13 end to end and close documentation/versioning once the provider-neutral AI analysis workflow is stable.

Acceptance criteria:

- Source-grounded analysis works through deterministic test samples and the live Gemini path.
- Settings configuration and feed-detail workflows pass focused UI tests.
- Storage, job, provider, command, and frontend tests pass.
- Roadmap records M13 completion status.
- Kanban M13 cards move out of active context.
- Version is bumped to `0.13.0`.

Delivered:

- Added provider-neutral AI analysis architecture, contracts, storage, settings, async job runtime, typed commands, and frontend API.
- Added deterministic `test_sample` analysis provider for secret-free automated tests.
- Added Gemini as the first live general-analysis provider behind the provider-neutral boundary.
- Added Settings configuration for general AI provider/model/timeout and provider disclosure.
- Added feed-detail AI analysis UI in Inbox and company feed details with prompt presets, custom questions, async state, retry, result metadata, reasoning, tags, and source references.
- Added opt-in live Gemini feed-item analysis smoke path with documented env vars and `make smoke-gemini-analysis`.
- Recorded successful live Gemini analysis smoke evidence: provider `provider_gemini`, model `gemini-3.5-flash`, `job_status=succeeded`, 1 source reference.
- Added future V1 Milestone 14 for a module-neutral developer mode diagnostics framework.
- Moved M13 active cards out of the active Kanban context.
- Version bumped to `0.13.0` in package, Rust, lock, Tauri config, and Rust health-test files.

Docs/contracts touched: roadmap, kanban, kanban archive, contracts, data model, project brief, architecture, engineering workflow, live smoke tests, AI analysis framework, version files.

Test expectations:

- `rtk npm run typecheck` passed.
- `rtk npm test -- --run` passed, 77 tests.
- `rtk npm run build` passed.
- `rtk cargo fmt --check` passed from `src-tauri/`.
- `rtk cargo clippy --all-targets -- -D warnings` passed from `src-tauri/`.
- `rtk cargo test` passed from `src-tauri/`, 102 tests, 3 ignored.
- `make smoke-gemini-analysis` passed with local `.env.local` credentials.

### M12.9 Close M12 workflow polish

Intent: verify M12 end to end and close documentation/versioning once locale and shortcut workflows are stable.

Acceptance criteria:

- English and Polish locale workflows pass focused UI tests.
- Critical shortcut workflows pass focused UI tests.
- Project practices record that future feature work must evaluate whether new or changed user actions should be shortcut actions.
- App build/typecheck/test commands pass.
- Roadmap records M12 completion status.
- Kanban M12 cards move out of active context.
- Version is bumped if this milestone closure is treated as a release boundary.

Delivered:

- Closed M12 locale work with English as the first-run default and Polish as the first additional app locale.
- Closed M12 shortcut work with configurable app, Inbox, Company, and notebook shortcut actions, plus Settings discoverability, persistence, reset, disable, and conflict warnings.
- Preserved visible UI controls for shortcut actions and scoped shortcuts to avoid text-entry and browser/WebView conflicts.
- Recorded the continuous development rule that future feature work must check whether new or changed user actions should be shortcut actions.
- Moved M12 active cards out of the active Kanban context.
- Version bumped to `0.12.0` in package, Rust, lock, Tauri config, and Rust health-test files.

Docs/contracts touched: roadmap, kanban, kanban archive, version files.

Test expectations:

- `rtk npm run typecheck` passed.
- `rtk npm test -- --run` passed, 76 tests.
- `rtk npm run build` passed.
- `rtk cargo fmt --check` passed.
- `rtk cargo clippy --all-targets -- -D warnings` passed.
- `rtk cargo test` passed, 92 tests, 2 ignored.

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
- Added [Testing](testing.md#live-smoke-tests) with command, expected result, failure interpretation, and M10 closure rule.

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

### Modularization 13AF: Convert modularization design into ongoing guide

Intent: close the modularization effort by recording findings and making modularity a continuous development rule.

Acceptance criteria:

- `docs/modularization-design.md` states the broad M13 modularization effort is complete.
- The document records findings from the modularization pass.
- Current frontend, Rust, style, and test structures reflect the repo after M13.
- The document includes a continuous development checklist for future feature work.
- Project practices require non-trivial work to consider the modularization guide.
- The old extraction-order and M10-specific near-term recommendation no longer read like open work.

Delivered:

- Reframed `docs/modularization-design.md` as a post-M13 operating guide.
- Added findings, accepted composition points, current structure, future extraction triggers, and standing modularity rules.
- Updated `docs/project-practices.md` to reference the modularization guide before non-trivial implementation work.

Docs/contracts touched: modularization design, project practices, kanban.

Test expectations: docs-only change; no automated test required.

Design reference: [Modularization Design](modularization-design.md).

### Modularization 13AE: Adopt shared frontend primitives in screens

Intent: make the documented shared component structure active in existing screen code instead of only available for future work.

Acceptance criteria:

- Generic primary, secondary, icon, danger, action, ghost, and minimal button uses go through `Button` where behavior and CSS classes are preserved.
- Simple/structured screen empty states use `EmptyState` where markup is equivalent.
- Generic membership/status chips use `StatusPill` where markup is equivalent.
- Domain-specific segmented controls, row selectors, field-clear buttons, collapsible headers, registry suggestion rows, and anchor links remain native or domain-specific.
- The modularization design documents the adoption rule for future feature work.

Delivered:

- Extended `Button` to preserve existing `action-button` and `ghost-button` variants.
- Rewired existing screens and extracted screen components to use `Button`, `EmptyState`, and `StatusPill` for equivalent generic controls.
- Left native controls in place where the component would obscure domain-specific semantics.
- Added the shared-module adoption rule to the modularization design.

Docs/contracts touched: kanban, modularization design.

Test expectations: normal frontend check set passes.

Design reference: [Modularization Design](modularization-design.md).

### Modularization 13AD: Complete remaining documented frontend targets

Intent: close the remaining modularization-design target files without changing application behavior.

Acceptance criteria:

- `src/app/App.tsx` is a small app entry wrapper, with the stateful composition root moved out.
- Notebook entry detail editing lives in `NotebookEntryEditor.tsx`.
- Transcript segment review/search lives in `TranscriptSegmentReview.tsx`.
- Transcript note draft editing lives in `TranscriptNoteDraft.tsx`.
- A dedicated `TranscriptsScreen.test.tsx` exists.
- Test helper facade files exist for render helpers, Tauri mocks, and shared test data.
- Documented shared component, hook, and formatting target files exist as reusable primitives or aliases.

Delivered:

- Added `src/app/AppStateRoot.tsx` and reduced `src/app/App.tsx` to a small wrapper.
- Added `src/screens/Notebooks/NotebookEntryEditor.tsx`.
- Added `src/screens/Transcripts/TranscriptSegmentReview.tsx`.
- Added `src/screens/Transcripts/TranscriptNoteDraft.tsx`.
- Added `src/screens/Transcripts/TranscriptsScreen.test.tsx`.
- Added `src/test/renderApp.tsx`, `src/test/mockTauri.ts`, and `src/test/testData.ts`.
- Added shared component, hook, and formatting target files under `src/shared/`.

Docs/contracts touched: kanban, modularization design.

Test expectations: normal frontend check set passes.

Design reference: [Modularization Design](modularization-design.md).

### Modularization 13AC: Extract Inbox detail pane

Intent: reduce `InboxScreen.tsx` by moving selected feed item detail rendering and detail actions into a focused component.

Acceptance criteria:

- Feed detail aside, selected-item metadata, source links, attachments, and detail action buttons live in `InboxDetailPane.tsx`.
- `InboxScreen.tsx` keeps inbox filters, list rendering, empty states, and detail-pane resize behavior.
- Existing read/save/company/note/source detail actions remain unchanged.

Delivered:

- Added `src/screens/Inbox/InboxDetailPane.tsx`.
- Replaced inline detail aside markup in `InboxScreen.tsx`.
- Reduced `InboxScreen.tsx` to roughly 342 lines.

Docs/contracts touched: kanban, modularization design.

Test expectations: normal frontend check set passes.

Design reference: [Modularization Design](modularization-design.md).

### Modularization 13AB: Extract Events week and list views

Intent: reduce `EventsScreen.tsx` by separating event list and week calendar rendering from filter/composer orchestration.

Acceptance criteria:

- List event rows and selected event details live in `EventListView.tsx`.
- Week calendar day/weekend rendering and selected week-card details live in `WeekEventsView.tsx`.
- `EventsScreen.tsx` keeps page header, refresh controls, filters, week navigation, and manual event composer.
- Existing list/week selection, keyboard activation, source links, filters, and error states remain unchanged.

Delivered:

- Added `src/screens/Events/EventListView.tsx`.
- Added `src/screens/Events/WeekEventsView.tsx`.
- Replaced inline event row/card render functions in `EventsScreen.tsx`.
- Reduced `EventsScreen.tsx` to roughly 422 lines.

Docs/contracts touched: kanban, modularization design.

Test expectations: normal frontend check set passes.

Design reference: [Modularization Design](modularization-design.md).

### Modularization 13AA: Extract transcript job row and detail panel

Intent: reduce `TranscriptsScreen.tsx` by moving repeated transcript job row/detail rendering into a focused component.

Acceptance criteria:

- Transcript job row, expanded detail panel, company link panel, segment search/review list, and notebook note draft live in `TranscriptJobRow.tsx`.
- `TranscriptsScreen.tsx` keeps page header, runtime strip, job composer, empty/error state, and job list composition.
- Existing transcript job retry/delete, description edit, company link, segment selection, and note draft workflows remain unchanged.

Delivered:

- Added `src/screens/Transcripts/TranscriptJobRow.tsx`.
- Replaced inline transcript job map body in `TranscriptsScreen.tsx`.
- Reduced `TranscriptsScreen.tsx` to roughly 156 lines.

Docs/contracts touched: kanban, modularization design.

Test expectations: normal frontend check set passes.

Design reference: [Modularization Design](modularization-design.md).

### Modularization 13Z: Extract Companies workspace

Intent: reduce `CompaniesScreen.tsx` by separating selected-company workspace tabs from company/watchlist list orchestration.

Acceptance criteria:

- Selected company workspace header, tabs, feed detail, notebook, claims, transcripts placeholder, and metadata panel live in `CompanyWorkspace.tsx`.
- `CompaniesScreen.tsx` keeps watchlists, company form, search, company rows, watchlist assignment, and top-level errors.
- Existing company feed, notebook, claims, metadata, inbox, and note workflows remain unchanged.

Delivered:

- Added `src/screens/Companies/CompanyWorkspace.tsx`.
- Replaced inline selected-company workspace markup in `CompaniesScreen.tsx`.
- Reduced `CompaniesScreen.tsx` to roughly 537 lines.

Docs/contracts touched: kanban, modularization design.

Test expectations: normal frontend check set passes.

Design reference: [Modularization Design](modularization-design.md).

### Modularization 13Y: Split storage tests by domain

Intent: make the extracted storage test suite easier to navigate without adding production indirection.

Acceptance criteria:

- Storage tests live under `src-tauri/src/storage/tests/` by domain.
- Shared test samples/helpers live in a common test helper module.
- Test names, test behavior, runtime storage modules, and public storage API behavior remain unchanged.
- The split does not introduce tiny production modules or change schema/migration behavior.

Delivered:

- Converted `src-tauri/src/storage/tests.rs` into `src-tauri/src/storage/tests/mod.rs`.
- Added domain test modules for schema, companies, events, transcripts, feed/source ingestion, notebooks, source registry, settings, and watchlists.
- Added `src-tauri/src/storage/tests/common.rs` for shared storage test samples.
- Kept `storage/mod.rs` as a 547-line runtime facade plus test module declaration.

Docs/contracts touched: kanban.

Test expectations: normal Rust check set passes.

Design reference: [Modularization Design](modularization-design.md).

### Modularization 13X: Split storage tests from facade

Intent: reduce `src-tauri/src/storage/mod.rs` without fragmenting the runtime `AppState` facade.

Acceptance criteria:

- Storage tests live in `src-tauri/src/storage/tests.rs`.
- `src-tauri/src/storage/mod.rs` keeps the runtime facade, module exports, shared storage helpers, and a small test module declaration.
- Storage test behavior, test names, and public storage API behavior remain unchanged.

Delivered:

- Moved the large inline `#[cfg(test)] mod tests` block from `storage/mod.rs` to `storage/tests.rs`.
- Moved test-only imports and constants into `storage/tests.rs`.
- Reduced `storage/mod.rs` from roughly 3,310 lines to roughly 547 lines.

Docs/contracts touched: kanban.

Test expectations: normal Rust check set passes.

Design reference: [Modularization Design](modularization-design.md).

### Modularization 13W: Extract transcript runtime and job composer

Intent: reduce `TranscriptsScreen.tsx` by moving runtime status and new-job form responsibilities into focused components.

Acceptance criteria:

- Transcript runtime provider/credential/timeout summary lives in `TranscriptRuntimeStrip.tsx`.
- New transcript job form and company suggestions live in `TranscriptJobComposer.tsx`.
- `TranscriptsScreen.tsx` keeps transcript job list/detail composition and existing workflow props.
- Existing URL validation, company suggestion selection, create-job behavior, and runtime summary copy remain unchanged.

Delivered:

- Added `src/screens/Transcripts/TranscriptRuntimeStrip.tsx`.
- Added `src/screens/Transcripts/TranscriptJobComposer.tsx`.
- Replaced inline runtime strip and job composer markup in `TranscriptsScreen.tsx`.
- Reduced `TranscriptsScreen.tsx` from roughly 620 lines to roughly 527 lines.

Docs/contracts touched: kanban.

Test expectations: normal frontend check set passes.

Design reference: [Modularization Design](modularization-design.md).

### Modularization 13V: Extract Sources adapter row

Intent: align Sources screen ownership with the modularization design target by moving adapter row and detail-panel UI out of `SourcesScreen.tsx`.

Acceptance criteria:

- Source adapter row, expanded details, registry panel, and unmatched diagnostics panel live in `SourceAdapterRow.tsx`.
- `SourcesScreen.tsx` remains responsible for the page header, refresh summary, list composition, and top-level errors.
- Existing source refresh, registry refresh, unmatched diagnostics, source page link, keyboard activation, and registry add behavior remain unchanged.

Delivered:

- Added `src/screens/Sources/SourceAdapterRow.tsx`.
- Replaced inline source adapter row/detail markup in `SourcesScreen.tsx` with `SourceAdapterRow` composition.
- Reduced `SourcesScreen.tsx` from roughly 379 lines to roughly 134 lines.

Docs/contracts touched: kanban.

Test expectations: normal frontend check set passes.

Design reference: [Modularization Design](modularization-design.md).

### Modularization 13U: Split Settings screen sections

Intent: align Settings screen ownership with the modularization design target by moving section-specific UI out of `SettingsScreen.tsx`.

Acceptance criteria:

- Appearance, source/feed cleanup/import-export, AI provider, and credential sections live in separate Settings screen components.
- `SettingsScreen.tsx` remains responsible for page layout and composing the section components.
- Existing Settings props, UI copy, credential workflow, theme workflow, and model/timeout workflow remain unchanged.

Delivered:

- Added `AppearanceSettings.tsx`, `SourceSettings.tsx`, `AiSettings.tsx`, and `CredentialSettings.tsx`.
- Replaced inline section markup in `SettingsScreen.tsx` with section component composition.
- Reduced `SettingsScreen.tsx` from roughly 283 lines to roughly 84 lines.

Docs/contracts touched: kanban.

Test expectations: normal frontend check set passes.

Design reference: [Modularization Design](modularization-design.md).

### Modularization 13T: Align Rust job scheduler module

Intent: make the Rust jobs module names match the modularization design target.

Acceptance criteria:

- Blocking background job execution helper lives in `src-tauri/src/jobs/scheduler.rs`.
- Command modules call `jobs::scheduler::run_blocking_task`.
- Feed cleanup, source refresh, and transcript runner command behavior remains unchanged.

Delivered:

- Renamed `src-tauri/src/jobs/tasks.rs` to `src-tauri/src/jobs/scheduler.rs`.
- Updated job module exports and command call sites.

Docs/contracts touched: kanban.

Test expectations: normal Rust check set passes.

Design reference: [Modularization Design](modularization-design.md).

### Modularization 13S: Move credentials under providers

Intent: align credential storage/runtime code with the provider boundary in the modularization design.

Acceptance criteria:

- Credential descriptor, status, OS keychain, and development-environment fallback code live in `src-tauri/src/providers/credentials.rs`.
- Credential commands keep their public command names and payloads.
- Transcript runner reads Gemini credentials through the provider credential module.
- Secret handling, OS keychain behavior, and live keyring smoke test behavior remain unchanged.

Delivered:

- Moved root Rust credential implementation to `src-tauri/src/providers/credentials.rs`.
- Exported the credential provider module from `src-tauri/src/providers/mod.rs`.
- Updated credential commands and transcript runner imports to use `providers::credentials`.
- Removed the root `credentials` module from `src-tauri/src/lib.rs`.

Docs/contracts touched: kanban.

Test expectations: normal Rust check set passes.

Design reference: [Modularization Design](modularization-design.md).

### Modularization 13R: Extract Rust storage registry module

Intent: align storage ownership with the modularization design by separating source adapter registry/catalog state from source ingestion storage.

Acceptance criteria:

- Source adapter catalog listing, GPW company registry listing, source adapter attempt records, and source adapter error records live in `src-tauri/src/storage/registry.rs`.
- `src-tauri/src/storage/sources.rs` continues to own source ingestion and ingestion helper behavior.
- `AppState` command-facing method names and public Tauri command contracts remain unchanged.
- No schema, migration, source policy, or local-first behavior changes are introduced.

Delivered:

- Added `src-tauri/src/storage/registry.rs`.
- Moved source adapter catalog/registry listing and adapter attempt/error storage functions out of `sources.rs`.
- Updated the storage facade to delegate those methods to `registry`.
- Reduced `src-tauri/src/storage/sources.rs` from roughly 1,517 lines to roughly 1,262 lines.

Docs/contracts touched: kanban.

Test expectations: normal Rust check set passes.

Design reference: [Modularization Design](modularization-design.md).

### Modularization 13Q: Align health command module naming

Intent: make the Rust command module names match the modularization design target without changing command behavior.

Acceptance criteria:

- Health and database status commands live in `src-tauri/src/commands/health.rs`.
- Command registration and tests refer to `commands::health`.
- Tauri command names, payloads, and responses remain unchanged.

Delivered:

- Renamed the command module from `commands/system.rs` to `commands/health.rs`.
- Updated command module exports, Tauri handler registration, and the health unit test reference.

Docs/contracts touched: kanban.

Test expectations: normal Rust check set passes.

Design reference: [Modularization Design](modularization-design.md).

### Modularization 13P: Extract app view model

Intent: move pure derived app data, filters, selected records, status summaries, scheduler keys, and dirty flags out of the app composition component.

Acceptance criteria:

- Derived app data lives in an app-owned view-model hook instead of inline in `src/app/App.tsx`.
- Inbox, company, notebook, event, source, registry, transcript suggestion, scheduler, and shell status derived values preserve existing filtering and fallback behavior.
- `App.tsx` keeps state ownership, controller composition, lifecycle composition, and screen prop wiring.
- Public API modules, Tauri command contracts, and user-visible behavior remain unchanged.

Delivered:

- Added `src/app/useAppViewModel.ts` for pure app-level derived data.
- Replaced inline `useMemo` view-data blocks in `src/app/App.tsx` with a single view-model hook call.
- Reduced `src/app/App.tsx` from roughly 1,460 lines to roughly 1,136 lines.

Docs/contracts touched: kanban.

Test expectations: normal frontend check set passes.

Design reference: [Modularization Design](modularization-design.md).

### Modularization 13O: Extract app lifecycle effects

Intent: move bootstrap refreshes, scheduler timers, theme application, and selection synchronization out of the app composition component.

Acceptance criteria:

- App lifecycle effects live in an app-owned hook instead of inline in `src/app/App.tsx`.
- Initial data refresh, theme application, selected feed item fallback, source adapter refs, feed pruning, source refresh scheduling, registry refresh scheduling, event refreshes, and notebook edit-form synchronization preserve existing behavior.
- `App.tsx` keeps state ownership and controller composition while passing dependencies into the lifecycle hook.
- Public Tauri command contracts, API modules, source refresh intervals, and user-visible behavior remain unchanged.

Delivered:

- Added `src/app/useAppLifecycleEffects.ts` for app bootstrap, timer, selection, and edit-form effects.
- Replaced inline lifecycle `useEffect` blocks in `src/app/App.tsx` with a single hook call.
- Reduced `src/app/App.tsx` from roughly 1,625 lines to roughly 1,460 lines.

Docs/contracts touched: kanban.

Test expectations: normal frontend check set passes.

Design reference: [Modularization Design](modularization-design.md).

### Modularization 13N: Add Rust app-state boundary module

Intent: align the Rust application state surface with the target modular structure without changing storage behavior.

Acceptance criteria:

- `src-tauri/src/app_state.rs` exists as the app-state boundary named by the modularization design.
- Tauri setup, command handlers, and backend jobs refer to `app_state::AppState` instead of reaching through `storage::AppState`.
- Storage remains the owner of SQLite implementation details and data contracts for this slice.
- Public Tauri command names, payloads, storage method behavior, and job behavior remain unchanged.

Delivered:

- Added `src-tauri/src/app_state.rs` as the command-facing app-state boundary.
- Updated Tauri setup, command modules, and job modules to use `app_state::AppState`.
- Kept storage data types and facade method implementations under `src-tauri/src/storage/`.
- Preserved public command contracts and local-first storage behavior.

Docs/contracts touched: kanban.

Test expectations: normal Rust check set passes.

Design reference: [Modularization Design](modularization-design.md).

### Modularization 13M: Move app composition under `src/app`

Intent: align the frontend app composition file with the target structure in the modularization design.

Acceptance criteria:

- The real app composition component lives at `src/app/App.tsx`.
- Root `src/App.tsx` remains only as a compatibility re-export for existing imports.
- Import paths are updated without changing runtime behavior, screen props, or tests.
- App shell, controllers, screen composition, and global state ownership remain unchanged.

Delivered:

- Moved the composition component from `src/App.tsx` to `src/app/App.tsx`.
- Replaced root `src/App.tsx` with a one-line re-export.
- Updated import paths for API, app, screen, and shared modules after the move.

Docs/contracts touched: kanban.

Test expectations: normal frontend and Rust check set passes.

Design reference: [Modularization Design](modularization-design.md).

### Modularization 13L: Extract app data refresh controller

Intent: move app bootstrap refreshes, database-backed refresh, source/reference refreshes, credential/settings refreshes, and feed cleanup actions out of `src/App.tsx`.

Acceptance criteria:

- Health, database status, companies, watchlists, feed, sources, registry entries, settings, and Gemini credential refresh functions live in an app-owned data controller.
- Database-backed refresh and delete-unsaved-feed workflows preserve existing state transitions, confirmation copy, and post-refresh behavior.
- Feed prune workflow preserves current retention setting and silent failure behavior.
- `App.tsx` keeps data state ownership and passes dependencies into the controller without changing screen props.
- Public Tauri command contracts and API module boundaries remain unchanged.

Delivered:

- Added `src/app/useAppDataController.ts` for app data refresh and cleanup actions.
- Replaced inline refresh/delete/prune functions in `src/App.tsx` with controller composition.
- Moved direct frontend API imports for data refresh out of `src/App.tsx`.
- Reduced `src/App.tsx` from roughly 1,805 lines to roughly 1,625 lines.

Docs/contracts touched: kanban.

Test expectations: normal frontend and Rust check set passes.

Design reference: [Modularization Design](modularization-design.md).

### Modularization 13K: Extract workspace navigation and detail-pane controllers

Intent: move company workspace navigation, notebook origin actions, company feed row navigation, and detail-pane resizing out of `src/App.tsx`.

Acceptance criteria:

- Company workspace open/focus and keyboard row navigation live in an app-owned workspace navigation controller.
- Notebook origin rendering and origin-to-Inbox navigation preserve existing labels, buttons, links, filters, and selected feed behavior.
- Company feed item expand/keyboard navigation preserve existing behavior.
- Detail-pane pointer and keyboard resize behavior lives in a focused app hook and preserves existing min/max/responsive constraints.
- `App.tsx` keeps state ownership and passes dependencies into the controllers without changing screen props.

Delivered:

- Added `src/app/useWorkspaceNavigationController.tsx` for workspace navigation, notebook origin rendering, and company feed row navigation.
- Added `src/app/useDetailPaneResize.ts` for Inbox detail-pane resize handlers.
- Replaced inline workspace/origin/resize helpers in `src/App.tsx` with controller composition.
- Reduced `src/App.tsx` from roughly 1,966 lines to roughly 1,805 lines.

Docs/contracts touched: kanban.

Test expectations: normal frontend and Rust check set passes.

Design reference: [Modularization Design](modularization-design.md).

### Modularization 13: Close modularization pass

Intent: verify that the app reached the target modular shape without behavior loss or undocumented contract drift.

Acceptance criteria:

- `src/App.tsx`, `src/App.test.tsx`, `src/styles.css`, `src-tauri/src/lib.rs`, and Rust storage files are no longer architecture debt by the standards in [Modularization Design](modularization-design.md).
- Frontend API modules, screens, shared helpers, Rust commands, storage modules, providers, source adapters, and jobs follow the documented ownership rules.
- Public Tauri command contracts remain unchanged unless separately documented and accepted.
- No local-first, secret-handling, source-policy, or AI-policy requirement was weakened during extraction.
- Final `git diff --stat` for each completed slice stayed reviewable or was split when it became too broad.
- The normal local check set passes after the final slice.

Delivered:

- Split frontend API command calls into `src/api/`.
- Split frontend screens and screen tests under `src/screens/`, with shared workflow test harness in `src/test/`.
- Split shared frontend formatting, components, and app-level controllers under `src/shared/` and `src/app/`.
- Split CSS into `src/styles/` modules with `src/styles.css` as an import entrypoint.
- Split Rust command handlers, app-state boundary, providers, jobs, and storage modules under `src-tauri/src/`.
- Reduced key architecture-debt files: `src/app/App.tsx` is roughly 1,136 lines, root `src/App.tsx` is a one-line re-export, `src/App.test.tsx` is 46 lines, `src/styles.css` is 17 lines, `src-tauri/src/lib.rs` is 89 lines, and `src-tauri/src/storage/mod.rs` is the storage facade/tests at roughly 3,309 lines.
- Preserved public Tauri command contracts, local-first behavior, secret handling, source policy, and AI decision-support boundaries.
- Final normal local check set passed.

Docs/contracts touched: modularization design, kanban.

Test expectations: normal local check set passed.

Design reference: [Modularization Design](modularization-design.md).

### Modularization 13J: Extract source display controller

Intent: move Sources-screen display helpers and source row interaction handlers out of `src/App.tsx`.

Acceptance criteria:

- Source adapter expand/select, unmatched item toggle, registry list toggle, source status opening, and source row keyboard activation live in an app-owned source display controller.
- Source scheduler, trigger, and next-refresh labels use shared formatting and preserve existing text.
- `App.tsx` keeps source state ownership and passes dependencies into the controller without changing screen props.
- Source status behavior, selected adapter behavior, and refresh-inspection behavior remain unchanged.
- Public Tauri command contracts and API module boundaries remain unchanged.

Delivered:

- Added `src/app/useSourceDisplayController.ts` for source display handlers and formatting helpers.
- Replaced inline source display helpers and duplicate `formatPollInterval` implementation in `src/App.tsx`.
- Reduced `src/App.tsx` from roughly 2,093 lines to roughly 1,966 lines.
- Left final M13 close-out pending for one remaining audit of app bootstrap, detail-pane helpers, origin rendering, and screen composition.

Docs/contracts touched: kanban.

Test expectations: normal frontend and Rust check set passes.

Design reference: [Modularization Design](modularization-design.md).

### Modularization 13I: Extract transcript workflow controller

Intent: move transcript job, segment, link, run/delete, description, and transcript-to-notebook workflows out of `src/App.tsx`.

Acceptance criteria:

- Transcript job listing, segment loading, job selection, segment selection, create/run/delete, company linking, and description update live in an app-owned transcript controller.
- Transcript note draft and transcript-selection notebook creation preserve validation copy, note body/tag mapping, selected segments clearing, and notebook refresh behavior.
- `App.tsx` keeps transcript and notebook state ownership and passes dependencies into the controller without changing screen props.
- Gemini credential gating, provider selection, confirmation copy, and error handling remain unchanged.
- Public Tauri command contracts and API module boundaries remain unchanged.

Delivered:

- Added `src/app/useTranscriptController.ts` for transcript workflow handlers.
- Replaced inline transcript handlers in `src/App.tsx` with controller composition.
- Reduced `src/App.tsx` from roughly 2,465 lines to roughly 2,093 lines.
- Left final M13 close-out pending because `src/App.tsx` still owns refresh bootstrapping, detail-pane/source display helpers, origin rendering, and screen composition.

Docs/contracts touched: kanban.

Test expectations: normal frontend and Rust check set passes.

Design reference: [Modularization Design](modularization-design.md).

### Modularization 13H: Extract company event workflow controller

Intent: move company event refresh, filter clearing, composer setup, and manual event creation out of `src/App.tsx`.

Acceptance criteria:

- Event list refresh and event filter mapping live in an app-owned event controller.
- Manual event composer setup and create workflow preserve existing default company/date behavior, validation copy, and source metadata.
- `App.tsx` keeps event state ownership and passes dependencies into the controller without changing screen props.
- Existing week/list filtering, selected event retention, error handling, and user-visible copy remain unchanged.
- Public Tauri command contracts and API module boundaries remain unchanged.

Delivered:

- Added `src/app/useCompanyEventsController.ts` for company event refresh and mutation handlers.
- Replaced inline event workflow handlers in `src/App.tsx` with controller composition.
- Reduced `src/App.tsx` from roughly 2,520 lines to roughly 2,465 lines.
- Left final M13 close-out pending because `src/App.tsx` still owns transcript, detail-pane, source display, and origin-rendering workflow handlers.

Docs/contracts touched: kanban.

Test expectations: normal frontend and Rust check set passes.

Design reference: [Modularization Design](modularization-design.md).

### Modularization 13G: Extract feed and Inbox interaction controller

Intent: move feed item state mutations and Inbox interaction handlers out of `src/App.tsx`.

Acceptance criteria:

- Feed read/saved updates, selected-item updates, visible mark-read, keyboard row selection, and Inbox filter clearing live in an app-owned feed controller.
- Feed item navigation into company workspace and company feed-item inspection preserve existing selection and section behavior.
- `App.tsx` keeps feed/filter state ownership and passes dependencies into the controller without changing screen props.
- Existing command calls, error handling, keyboard behavior, filter reset values, and user-visible copy remain unchanged.
- Public Tauri command contracts and API module boundaries remain unchanged.

Delivered:

- Added `src/app/useFeedController.ts` for feed item and Inbox interaction handlers.
- Replaced inline feed/Inbox handlers in `src/App.tsx` with controller composition.
- Reduced `src/App.tsx` from roughly 2,607 lines to roughly 2,520 lines.
- Left final M13 close-out pending because `src/App.tsx` still owns transcript, event, detail-pane, and origin-rendering workflow handlers.

Docs/contracts touched: kanban.

Test expectations: normal frontend and Rust check set passes.

Design reference: [Modularization Design](modularization-design.md).

### Modularization 13F: Extract company and watchlist controller

Intent: move company form, GPW registry lookup, local company mutations, and watchlist assignment handlers out of `src/App.tsx`.

Acceptance criteria:

- Company form update/clear, registry lookup, registry selection, company create/delete, and registry add workflows live in an app-owned company controller.
- Watchlist create, company assign/remove, assignment draft state, and transient row feedback live in the same controller because they operate on company rows.
- `App.tsx` keeps company/watchlist state ownership and passes dependencies into the controller without changing screen props.
- Existing confirmation copy, lookup status copy, assignment feedback copy, and refresh behavior remain unchanged.
- Public Tauri command contracts and API module boundaries remain unchanged.

Delivered:

- Added `src/app/useCompanyController.ts` for company and watchlist workflow handlers.
- Replaced inline company/watchlist handlers in `src/App.tsx` with controller composition.
- Reduced `src/App.tsx` from roughly 2,815 lines to roughly 2,607 lines.
- Left final M13 close-out pending because `src/App.tsx` still owns feed, transcript, event, and detail-pane workflow handlers.

Docs/contracts touched: kanban.

Test expectations: normal frontend and Rust check set passes.

Design reference: [Modularization Design](modularization-design.md).

### Modularization 13E: Extract notebook workflow controller

Intent: move Companies-screen and Notebooks-screen notebook workflow handlers out of `src/App.tsx`.

Acceptance criteria:

- Notebook refresh, manual note create/save/cancel, claim status update, and Notebooks-screen create/save/cancel flows live in an app-owned notebook controller.
- Feed-item-to-note draft creation stays behaviorally identical, including tags, origin metadata, selected notebook company, and navigation to Notebooks.
- `App.tsx` keeps notebook state ownership and passes dependencies into the controller without changing screen props.
- Existing notebook origin handling, markdown form data, follow-up fields, and claim status behavior remain unchanged.
- Public Tauri command contracts and API module boundaries remain unchanged.

Delivered:

- Added `src/app/useNotebookController.ts` for notebook refresh and workflow handlers.
- Replaced inline notebook handlers in `src/App.tsx` with controller composition.
- Kept origin rendering in `App.tsx` for this slice because it still coordinates Inbox navigation and external URL actions.
- Reduced `src/App.tsx` from roughly 3,117 lines to roughly 2,815 lines.
- Left final M13 close-out pending because `src/App.tsx` still owns company/watchlist, feed, transcript, and event workflow handlers.

Docs/contracts touched: kanban.

Test expectations: normal frontend and Rust check set passes.

Design reference: [Modularization Design](modularization-design.md).

### Modularization 13D: Extract settings and credentials controller

Intent: move settings mutations and Gemini credential mutations out of `src/App.tsx`.

Acceptance criteria:

- Theme, source poll interval, YouTube transcription model, and YouTube transcription timeout updates live in an app-owned settings controller.
- Gemini API key save/clear workflows live in the same controller because they are Settings-screen credential actions.
- `App.tsx` keeps settings and credential state ownership while composing controller callbacks.
- Settings screen props, UI copy, command names, secret handling, and OS keychain behavior remain unchanged.
- Public Tauri command contracts and API module boundaries remain unchanged.

Delivered:

- Added `src/app/useSettingsController.ts` for Settings-screen mutation handlers.
- Replaced inline settings and credential handlers in `src/App.tsx` with controller composition.
- Reduced `src/App.tsx` from roughly 3,184 lines to roughly 3,117 lines.
- Left final M13 close-out pending because `src/App.tsx` still owns broad screen state orchestration and workflow handlers.

Docs/contracts touched: kanban.

Test expectations: normal frontend and Rust check set passes.

Design reference: [Modularization Design](modularization-design.md).

### Modularization 13C: Extract source refresh orchestration

Intent: move source refresh, per-source refresh, event-source refresh, registry refresh, and scheduled refresh execution out of `src/App.tsx`.

Acceptance criteria:

- Source refresh API calls and post-refresh view synchronization live in an app-owned controller module.
- `App.tsx` keeps the existing state ownership and passes dependencies into the controller without changing screen props.
- Manual source refresh, single-source refresh, event calendar refresh, scheduled source refresh, and GPW registry refresh preserve existing behavior.
- Source refresh in-flight guards, failure counters, success/error states, and selected source adapter updates remain unchanged.
- Public Tauri command contracts, source adapter behavior, scheduler intervals, and user-visible copy remain unchanged.

Delivered:

- Added `src/app/useSourceRefreshController.ts` to own source refresh orchestration and shared refresh-result handling.
- Replaced the in-place source refresh functions in `src/App.tsx` with controller composition.
- Kept feed pruning and timer setup in `App.tsx` for this slice while moving scheduled refresh execution into the controller.
- Reduced `src/App.tsx` from roughly 3,488 lines to roughly 3,184 lines.
- Left final M13 close-out pending because `src/App.tsx` still owns broad screen state orchestration and workflow handlers.

Docs/contracts touched: kanban.

Test expectations: normal frontend and Rust check set passes.

Design reference: [Modularization Design](modularization-design.md).

### Modularization 13B: Extract frontend app shell and app helpers

Intent: reduce `src/App.tsx` frontend architecture debt by moving shell UI and pure app-level helpers into owned modules.

Acceptance criteria:

- Sidebar, topbar, navigation, search, health/status pills, source refresh action, and theme selector live in an app shell component.
- Pure app-level helpers for theme resolution, notebook forms, event forms, transcript job forms, and source scheduler constants live under `src/app/`.
- Existing screen rendering, state ownership, command calls, scheduler behavior, source refresh behavior, and UI copy remain unchanged.
- Public Tauri command contracts and API module boundaries remain unchanged.
- `src/App.tsx` is smaller and closer to state orchestration only, while final M13 close-out remains pending.

Delivered:

- Added `src/app/AppShell.tsx` for the app frame, navigation, topbar actions, health display, source status pill, database status pill, source refresh button, and theme control.
- Added focused app helper modules for theme, notebooks, events, transcript jobs, and source scheduling.
- Replaced duplicated shell/helper code in `src/App.tsx` with imports and `AppShell` composition.
- Reduced `src/App.tsx` from roughly 3,756 lines to roughly 3,488 lines.
- Left final M13 close-out pending because `src/App.tsx` still owns broad state orchestration and screen prop wiring.

Docs/contracts touched: kanban.

Test expectations: normal frontend and Rust check set passes.

Design reference: [Modularization Design](modularization-design.md).

### Modularization 13A: Finish Rust storage module split

Intent: close the remaining Rust storage architecture debt before the final modularization audit.

Acceptance criteria:

- `src-tauri/src/storage/mod.rs` is reduced to the storage facade, shared helpers, and existing storage tests.
- Domain-specific SQLite behavior lives in storage modules for companies, watchlists, feed, notebooks, events, sources, registry/source state, migrations, settings, transcripts, and shared types.
- `AppState` keeps the existing public storage API and command-facing method names.
- No schema or migration SQL change is introduced during the split.
- Public Tauri command contracts remain unchanged.
- Source adapter policy, local-first behavior, secret handling, and AI decision-support boundaries remain unchanged.

Delivered:

- Added storage modules for shared types, migrations, companies, watchlists, feed, notebooks, events, and sources/source state.
- Kept `storage/mod.rs` as the `AppState` facade with connection locking and public method delegation.
- Preserved the existing storage tests in `storage/mod.rs`.
- Reduced `storage/mod.rs` from roughly 7,165 lines to roughly 3,309 lines.
- Left final M13 close-out pending because `src/App.tsx` remains a frontend architecture-debt outlier.

Docs/contracts touched: kanban.

Test expectations: normal Rust and frontend check set passes.

Design reference: [Modularization Design](modularization-design.md).

### Modularization 12: Split CSS by screen and shared controls

Intent: make styling ownership match the extracted frontend modules after component and screen boundaries are stable.

Acceptance criteria:

- Shared tokens, layout, controls, rows, forms, and table styles move under `src/styles/`.
- Screen-specific styles move under `src/styles/screens/`.
- CSS is split after, not before, the matching screen/component extraction.
- Visual behavior, responsive layout, density, and theme behavior remain unchanged unless an explicit UI polish card says otherwise.
- Class names remain understandable and scoped to screen/shared ownership.
- The app no longer relies on one large mixed-responsibility `src/styles.css` file.

Delivered:

- Converted `src/styles.css` into an ordered import entrypoint.
- Added shared style modules for tokens, shell, layout, controls, rows, membership chips, notebook shared styles, utilities, and responsive rules under `src/styles/`.
- Added screen-owned style modules under `src/styles/screens/` for Events, Notebooks, Companies, company workspace, Transcripts, Sources, Settings, and Inbox.
- Preserved existing selectors, cascade order, responsive behavior, and generated CSS bundle size during the split.

Docs/contracts touched: kanban; UI information architecture only if visual behavior changes.

Test expectations: frontend build/tests pass; run a focused desktop visual sanity check when the CSS split is broad.

Design reference: [Modularization Design](modularization-design.md).

### Modularization 11: Split frontend and Rust tests by domain

Intent: reduce broad test files and shared mock setup debt after module boundaries are stable.

Acceptance criteria:

- Shared frontend test helpers live under `src/test/`.
- Screen workflow tests move near the screens they verify.
- Broad `src/App.test.tsx` coverage is split into domain-focused tests without losing critical workflows.
- Rust test support builders move into `src-tauri/src/test_support.rs` or domain-specific test support modules when shared broadly.
- Tests keep using mocks, injected fetchers, and test samples for default checks.
- Mock/sample success is not presented as live feature completion evidence.

Delivered:

- Added `src/test/appWorkflowHarness.tsx` for shared app workflow rendering, Tauri command mocks, sample data, mutable mock state, and shared test utilities.
- Reduced `src/App.test.tsx` to app shell/navigation status smoke coverage.
- Moved Events, Inbox, Sources, Settings, Companies, and notebook/transcript workflow coverage into test files beside the matching screen modules.
- Preserved all 62 frontend workflow tests and existing mock/sample command behavior.
- Did not add Rust shared test support in this slice because the current broad debt was frontend-only.

Docs/contracts touched: kanban; project practices only if testing posture changes.

Test expectations: normal frontend and Rust test suites pass after each moved test group.

Design reference: [Modularization Design](modularization-design.md).

### Modularization 10: Extract Rust jobs and orchestration modules

Intent: separate scheduler, source refresh orchestration, transcript runner orchestration, and maintenance cleanup from command registration and storage details.

Acceptance criteria:

- Job/orchestration modules live under `src-tauri/src/jobs/`.
- Source refresh orchestration combines adapters, storage, dedupe, source state, and scheduler behavior without living in `lib.rs`.
- Transcript runner orchestration combines transcript jobs, providers, credentials, and storage without living in command handlers.
- Feed cleanup orchestration remains separate from source refresh.
- Scheduler overlap/backoff behavior and command-triggered manual refresh behavior remain unchanged.
- No source adapter policy is weakened during extraction.

Delivered:

- Added `src-tauri/src/jobs/` with job modules for source refresh, transcript runner orchestration, feed cleanup, and blocking task execution.
- Moved source refresh orchestration out of `commands/sources.rs` while preserving command DTOs, Tauri command names, manual/scheduler trigger handling, disabled adapter errors, and registry refresh behavior.
- Moved transcript job provider selection, credential lookup, status transitions, and segment creation out of `commands/transcripts.rs`.
- Moved feed prune/delete cleanup execution out of `commands/feed.rs`.
- Updated company lookup bootstrap behavior to call the source refresh job module instead of a command module.

Docs/contracts touched: kanban; architecture only if orchestration boundaries change.

Test expectations: scheduler, refresh, cleanup, and transcript runner tests pass.

Design reference: [Modularization Design](modularization-design.md).

### Modularization 9: Extract Rust storage modules

Intent: split `src-tauri/src/storage.rs` into domain-focused SQLite modules while preserving schema, migrations, row mapping, and public storage behavior.

Acceptance criteria:

- Storage modules live under `src-tauri/src/storage/`.
- Migrations, settings, companies, watchlists, feed, notebooks, events, sources, transcripts, registry, and shared storage types are separated into cohesive modules.
- Shared connection setup, migration runner entrypoint, and storage facade remain clear in `storage/mod.rs`.
- No database schema change is introduced during pure module extraction.
- Existing migration tests, storage tests, source dedupe tests, transcript tests, and notebook tests keep the same behavior.
- Large storage tests move close to the module they verify when practical.

Delivered:

- Converted `src-tauri/src/storage.rs` into `src-tauri/src/storage/mod.rs` so storage can grow as a module tree while preserving `crate::storage` imports.
- Added `src-tauri/src/storage/error.rs` for `StorageError` and `StorageResult`.
- Added `src-tauri/src/storage/settings.rs` for settings DTOs, settings read/update behavior, and settings validation helpers.
- Added `src-tauri/src/storage/transcripts.rs` for transcript DTOs, transcript job/segment CRUD, transcript job status transitions, row mappers, IDs, and transcript-selection note creation.
- Kept remaining storage domains in `storage/mod.rs` for follow-up storage slices to avoid a high-risk all-domain split in one step.
- No schema or migration SQL changes were introduced.

Docs/contracts touched: kanban; architecture only if the target module map changes.

Test expectations: Rust formatting, clippy, migration tests, storage tests, and relevant source/transcript/notebook tests pass.

Design reference: [Modularization Design](modularization-design.md).

### Modularization 8: Extract Rust command modules

Intent: make `src-tauri/src/lib.rs` mostly app setup and command registration by moving Tauri command handlers into domain command modules.

Acceptance criteria:

- Command modules live under `src-tauri/src/commands/`.
- Start with transcripts, settings, and credentials, then continue through companies, watchlists, feed, notebooks, events, and sources.
- Command handlers stay thin: validate/parse command input, call storage/domain/provider/job code, and map errors.
- Tauri command names and permissions remain unchanged.
- App state management remains explicit and typed.
- Command registration remains easy to audit from `lib.rs` or `commands/mod.rs`.

Delivered:

- Added `src-tauri/src/commands/` with domain command modules for system, companies, watchlists, feed, notebooks, events, transcripts, sources, settings, and credentials.
- Moved command input structs into their domain modules.
- Moved the transcript runner command into `commands/transcripts.rs` while preserving provider selection, credential access, segment creation, and job status mapping.
- Moved current source refresh command orchestration into `commands/sources.rs` as a temporary boundary until M10 extracts jobs/orchestration.
- Kept `src-tauri/src/lib.rs` focused on app setup, managed state, and explicit command registration paths.
- Preserved existing Tauri command function names and storage/provider behavior.

Docs/contracts touched: kanban; contracts only if command behavior mismatch is discovered.

Test expectations: Rust command and integration tests pass; Tauri command availability smoke tests still pass.

Design reference: [Modularization Design](modularization-design.md).

### Modularization 7: Extract Rust transcript providers

Intent: separate real Gemini transcript execution and offline test-sample behavior from Tauri command orchestration.

Acceptance criteria:

- Transcript providers move under `src-tauri/src/providers/transcripts/`.
- Gemini provider HTTP request building, response parsing, model handling, timeout handling, and provider error mapping are independently testable.
- Offline `test_sample` transcript behavior remains available for automated tests and development only.
- Provider code continues to receive secrets only from the Rust credential boundary.
- Live provider behavior, default provider mode, and transcript job status mapping remain unchanged.

Delivered:

- Added `src-tauri/src/providers/mod.rs` and `src-tauri/src/providers/transcripts/` as the Rust transcript provider module tree.
- Moved shared transcript provider contracts into `providers/transcripts/types.rs`.
- Moved the offline `test_sample` provider into `providers/transcripts/test_sample.rs`.
- Moved Gemini request construction, injected client boundary, response parsing, timeout handling, URL validation, HTTP error mapping, and provider tests into `providers/transcripts/gemini.rs`.
- Updated `src-tauri/src/lib.rs` to use `providers::transcripts` while keeping transcript job orchestration and Tauri command behavior unchanged.
- Removed the old top-level `src-tauri/src/transcript_providers.rs` module.

Docs/contracts touched: kanban; architecture only if the target module map changes.

Test expectations: Rust provider tests pass with mocked/sample Gemini responses; no default test requires live Gemini credentials.

Design reference: [Modularization Design](modularization-design.md).

### Modularization 6: Extract shared frontend helpers and app shell types

Intent: turn repeated UI helpers, formatting logic, hooks, navigation types, and app-shell state types into small stable modules after screen extraction exposes real reuse.

Acceptance criteria:

- App shell code lives under `src/app/` with navigation and shell-level types separated from screen modules.
- Shared components move under `src/shared/components/` only when they are generic and reused by at least two screens.
- Shared formatting helpers move under `src/shared/formatting/`.
- Shared hooks move under `src/shared/hooks/` only when they are not domain-specific.
- Domain-specific helpers remain with their screen instead of becoming broad shared utilities.
- No UI behavior changes are introduced by helper extraction.

Delivered:

- Added `src/app/navigation.ts`, `src/app/appTypes.ts`, and `src/app/layout.ts` for app shell navigation, state types, and layout constants.
- Added shared date/time and label formatting modules under `src/shared/formatting/`.
- Added reusable notebook components under `src/shared/components/` for markdown note bodies, date fields, and quarter fields.
- Added shared notebook and event form types under `src/shared/types/` and updated screen contracts to reuse them.
- Moved source-specific pure label helpers into `src/screens/Sources/sourceHelpers.ts` while keeping scheduler/next-poll formatting in `src/App.tsx` because it depends on app state.
- Replaced the corresponding helper/type definitions in `src/App.tsx` with imports without changing UI behavior.

Docs/contracts touched: kanban.

Test expectations: frontend typecheck and existing UI tests pass.

Design reference: [Modularization Design](modularization-design.md).

### Modularization 5: Extract Notebooks, Events, and Sources screens

Intent: complete frontend screen ownership by moving remaining domain screens out of `src/App.tsx`.

Acceptance criteria:

- `NotebooksScreen` and notebook editor/detail components live under `src/screens/Notebooks/`.
- `EventsScreen`, week view, and event list view live under `src/screens/Events/`.
- `SourcesScreen`, source adapter rows, grouped source status, unmatched diagnostics, and per-source refresh controls live under `src/screens/Sources/`.
- Notebook filtering/editing/origin links, event filtering/manual creation, and source refresh/status workflows remain unchanged.
- Screen modules call only their domain API modules and shared helpers.
- `src/App.tsx` becomes a shell: navigation, active section, topbar, global status, and screen composition.

Delivered:

- Added `src/screens/Notebooks/NotebooksScreen.tsx` and `src/screens/Notebooks/notebookTypes.ts` for the standalone Notebooks workspace presentation.
- Added `src/screens/Events/EventsScreen.tsx` and `src/screens/Events/eventTypes.ts` for event filters, manual event composer, week cards, and list rows.
- Added `src/screens/Sources/SourcesScreen.tsx` and `src/screens/Sources/sourceTypes.ts` for adapter rows, detail panels, registry diagnostics, unmatched items, and refresh controls.
- Replaced the remaining inline Notebooks, Events, and Sources render blocks in `src/App.tsx` with screen composition.
- Kept state, scheduling, and command handlers in `src/App.tsx` to preserve current behavior for this extraction slice.

Docs/contracts touched: kanban; UI information architecture only if screen behavior changes.

Test expectations: existing notebook, events, sources, source-refresh, and cleanup visibility tests pass; move tests near extracted screens when stable.

Design reference: [Modularization Design](modularization-design.md).

### Modularization 1: Extract frontend API boundary

Intent: make React presentation code stop owning Tauri command invocation details before moving screens out of `src/App.tsx`.

Acceptance criteria:

- `invoke` calls move out of `src/App.tsx` into domain API modules under `src/api/`.
- API modules cover companies, watchlists, feed, notebooks, events, sources, transcripts, settings, and credentials.
- TypeScript DTOs used by Tauri commands move with the API boundary or into clearly named domain type files.
- Public Tauri command names, inputs, outputs, and UI behavior remain unchanged.
- `src/App.tsx` imports typed API helpers instead of calling `invoke` directly.
- No screen extraction or behavior change is bundled into this slice unless a tiny compatibility adjustment is unavoidable.

Delivered:

- Added `src/api/tauri.ts` as the only frontend module that imports Tauri `invoke`.
- Added domain API modules for companies, credentials, events, feed, notebooks, settings, sources, system health/database status, transcripts, and watchlists.
- Moved frontend Tauri DTO types into `src/api/types.ts`.
- Updated `src/App.tsx` to use typed domain API helpers instead of direct `invoke` calls.
- Preserved command names and argument shapes, including no-argument command call shape.
- No screen extraction or UI behavior change was included.

Docs/contracts touched: kanban; contracts only if an existing command contract mismatch is discovered.

Test expectations: frontend typecheck and existing UI tests pass.

Design reference: [Modularization Design](modularization-design.md).

### Modularization 4: Extract Inbox and Companies screens

Intent: remove feed review, company management, company workspace, and watchlist workflow responsibility from the app shell.

Acceptance criteria:

- Inbox presentation and local UI state move under `src/screens/Inbox/`.
- Companies and company workspace presentation move under `src/screens/Companies/`.
- Company workspace tabs for Feed, Notebook, Claims, Transcripts, and Metadata are owned by the Companies screen area or clearly shared modules.
- Feed filtering, unread/saved state, source detail opening, company navigation, registry-assisted company creation, and watchlist membership behavior remain unchanged.
- Shared row/detail helpers are extracted only when they are reused by at least two domains.
- `src/App.tsx` no longer contains feed-row or company-workspace rendering logic.

Delivered:

- Added `src/screens/Inbox/InboxScreen.tsx`.
- Added `src/screens/Inbox/inboxTypes.ts`.
- Moved Inbox filters, review summary, feed list rows, empty states, resize separator, and feed detail pane out of `src/App.tsx`.
- Added `src/screens/Companies/CompaniesScreen.tsx`.
- Added `src/screens/Companies/companyTypes.ts`.
- Moved watchlists, company creation/lookup, registry suggestions, tracked-company list, company row actions, and company workspace tabs out of `src/App.tsx`.
- Preserved company workspace Feed, Notebook, Claims, Transcripts placeholder, and Metadata tabs.
- Kept workflow state, command handlers, notebook date/quarter controls, Markdown rendering, and origin rendering in `src/App.tsx` for this slice.
- Preserved feed filtering, unread/saved state, detail pane behavior, source links, company navigation, registry-assisted company creation, watchlist assignment/removal, company feed actions, notebook editing, claims status, and metadata display.

Docs/contracts touched: kanban; UI information architecture only if screen behavior changes.

Test expectations: existing Inbox, company workspace, registry lookup, and watchlist UI tests pass; move tests near extracted screens when stable.

Design reference: [Modularization Design](modularization-design.md).

### Modularization 3: Extract Transcripts screen

Intent: move the M10 transcript workflow out of `src/App.tsx` without weakening live Gemini behavior, note creation, or company-linking behavior.

Acceptance criteria:

- `TranscriptsScreen` lives under `src/screens/Transcripts/`.
- Transcript job form, job rows, segment review, company resolution, transcript selection, and note draft behavior are separated into cohesive screen-local components.
- Transcript-specific types live near the Transcripts screen or the transcripts API module.
- The app shell owns navigation and global reload/status wiring only.
- Completed, failed, queued, retry, delete, rename, company-link, segment selection, and note-save workflows behave exactly as before.
- The `provider_gemini` and `test_sample` contract distinction remains unchanged.

Delivered:

- Added `src/screens/Transcripts/TranscriptsScreen.tsx`.
- Added `src/screens/Transcripts/transcriptTypes.ts` for screen props and transcript-local form types.
- Added `src/screens/Transcripts/transcriptHelpers.tsx` for transcript URL validation, segment timestamp formatting, segment search matching, and highlighted search rendering.
- Moved the Transcripts render tree out of `src/App.tsx`.
- Kept transcript command/state ownership in `src/App.tsx` for this slice.
- Preserved transcript job creation, optional company selection, refresh, retry, delete, rename, company linking, segment search/selection, and note draft workflows.
- Left shared notebook date/quarter controls in `src/App.tsx` and passed them into the Transcripts screen for this slice.

Docs/contracts touched: kanban; contracts/product docs only if a behavior mismatch is discovered.

Test expectations: existing transcript UI tests pass; add or move focused transcript workflow tests when extraction makes that practical.

Design reference: [Modularization Design](modularization-design.md).

### Modularization 2: Extract Settings screen

Intent: move the settings, credential, model, timeout, source cleanup, and theme workflow out of the app shell as the first screen extraction.

Acceptance criteria:

- `SettingsScreen` lives under `src/screens/Settings/`.
- Settings-specific subcomponents are extracted for appearance, sources/cleanup, AI model/timeout, and credential settings where the existing UI responsibility is separable.
- Settings-specific types live near the Settings screen or the settings API module.
- `src/app/App.tsx` renders Settings through a screen module and keeps only shell-level state and callbacks.
- Credential secret values remain inaccessible to React; only non-secret credential status is passed through the screen.
- User-visible Settings behavior and copy remain unchanged unless docs/contracts are updated in the same slice.

Delivered:

- Added `src/screens/Settings/SettingsScreen.tsx`.
- Added `src/screens/Settings/settingsTypes.ts` for the Settings screen props.
- Moved the Settings render tree out of `src/App.tsx`.
- Kept global settings state, scheduler-facing settings behavior, and credential command handlers in `src/App.tsx`.
- Passed only non-secret credential status and local input draft state to the Settings screen.
- Preserved visible Settings copy, labels, controls, and command behavior.

Docs/contracts touched: kanban; UI information architecture or contracts only if behavior changes.

Test expectations: existing Settings workflow tests pass; add or move focused Settings tests when extraction makes that practical.

Design reference: [Modularization Design](modularization-design.md).


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


### Implement keyboard shortcuts (superseded by M12)

This older backlog card was superseded by the active [Milestone 12](kanban.md) Ready card, which combines keyboard shortcuts with locale and workflow polish.

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

## Archived Investigation And Study Notes (moved 2026-07-02)

Doc-slimming pass under [ADR 0063](adr/0063-claude-native-context-architecture.md) (Lean-docs layering policy). These blocks are pure execution chronicle — milestone-by-milestone narration of already-delivered work, or resolved investigation logs — moved here verbatim from their source doc, which now carries a pointer to this section instead. Current behavior for any feature named below is documented in its canonical spec (contracts.md / data-model.md / ui-flows.md / ui-information-architecture.md / the cited ADRs), not here.

### From docs/product-spec.md — "Future Experience Directions" (Research Workspace)

A future product-differentiation direction is to make the app a personal research memory system for public companies, not only a feed reader. The goal is to help the user answer what changed, why it matters, what management said before, and what should be checked next.

Candidate capabilities:

- company change timeline combining feed items, official reports, media items, notes, transcripts, claims, calendar events, and future AI outputs
- "what changed since last review" views for a company, watchlist, or time window
- expanded management claim tracking with source, expected period, follow-up date, related future events, and status history
- source-grounded company or watchlist research brief with cited evidence and no buy/sell/hold recommendations
- open research questions or threads per company, linked to notes, claims, events, and source items
- watchlist review mode that guides the user company-by-company through unread items, upcoming events, unresolved claims, and open questions
- event-aware reminders such as unresolved claims tied to an upcoming reporting period
- source quality and trust signals that distinguish official reports, company publications, media articles, opinion, paid research, and other source types
- daily or weekly personal research digest generated from the user's local watchlists and source items
- evidence linking between related source items, notes, claims, transcripts, questions, AI briefs, and events

This direction should not be implemented as ten separate isolated screens. Before implementation, a dedicated planning milestone should define a shared research evidence model, timeline/read-model boundary, review-session model, evidence-linking model, AI brief-building boundary, storage implications, and import/export impact. Existing notebooks, claims, feed items, transcripts, and future events should plug into this model through stable contracts instead of direct cross-screen coupling.

M24 accepts that research-workspace features should be built on a dedicated research/evidence boundary. Existing feed items, notebook entries, claims, transcript segments, events, AI analysis results, watchlists, and sources remain canonical in their owning domains. Research views consume backend-owned evidence and timeline read models rather than independently combining unrelated screen APIs. Durable research-owned state starts with review checkpoints and typed evidence links; full stored timeline projections wait until performance or review semantics require them.

M31 adds event-aware research reminders and on-demand personal research digests. Reminders are research-owned review pressure derived from open claims, upcoming events, and open research questions, plus explicit manual reminders. Digests are immutable, cited AI snapshots generated from backend-collected changed evidence and open reminders for the selected company or watchlist.

M25 adds the first visible Research screen as a top-level navigation item. The first slice is company-scoped: the user selects a tracked company and sees a newest-first evidence timeline combining feed items, notebook entries, claims, events, transcript segments, and AI analysis through the backend research read model. The screen header shows the selected company, last reviewed state, total visible evidence, and changed-since-review count. Evidence type filters and a changed-only filter are sent to the backend; the UI renders the returned timeline result instead of recomputing review counts.

The M25 review action is a single company-level `Mark reviewed` action. It stores a research review checkpoint for the selected company and refreshes the timeline so changed-since-review state is updated. Evidence rows should use product-language labels such as report, note, claim, event, transcript, or AI analysis instead of raw implementation identifiers. Where practical, an evidence row should offer an owning-domain action such as opening the source URL, opening the Inbox item, or moving to the relevant app area.

M26 adds watchlist review mode inside the same Research screen. The user can switch between company and watchlist review, select a watchlist, and work through member companies using a compact company queue with backend-owned changed evidence counts. Marking a watchlist reviewed updates only the watchlist checkpoint by default. The UI exposes an explicit option to also mark current member companies reviewed, but the cascade behavior is owned by the backend command.

M29 adds company-scoped research questions inside the Research screen. A question is a durable research-owned item with title, optional context, and `open`, `answered`, or `closed` status. The user can select a company, create questions for that company, select a question, delete a question, and link visible evidence rows to it through typed evidence links. Questions are shown in the company evidence timeline and imported/exported with research data. Watchlist-scoped questions remain a backend-compatible extension point, but normal UI creates company questions only until a dedicated watchlist-question workflow is designed.

AI research briefs are separate research entities with citations and provider/model/prompt provenance. They are not ordinary notebook entries, though a future workflow may let the user create a note from a brief or selected excerpt.

M30 adds on-demand AI research briefs for company and watchlist research scopes. Brief generation uses backend-owned evidence collection, prompt/context building, provider execution, citation mapping, rendering, and persistence boundaries. Generated briefs are immutable snapshots with visible citations and provenance. They are imported/exported with research data. Briefs must not contain buy/sell/hold recommendations, and creating a notebook note from a brief is not automatic.

The management claims tracker (`v0.42.0`, [ADR 0040](adr/0040-management-claims-tracker.md)) delivers the "expanded management claim tracking" and "unresolved claims tied to an upcoming reporting period" capabilities above. Claims become a first-class entity with a due period, quantitative target, source evidence, and a user-set verdict. AI extraction proposes claims from report documents and transcripts for mandatory confirmation. A due-period derivation job resurfaces an open claim into a "claims to verify" review queue when the due period's report arrives, attaching the matching confirmed financial fact for quantitative claims; the user resolves the verdict against evidence. Claims plug into the existing research evidence boundary (timeline, evidence links, reminders, digests) and remain owner-durable in import/export. There are no automated verdicts.

### From docs/product-spec.md — "Future Experience Directions" (Company Fundamentals)

Milestones v0.34.0–v0.37.0 add a fundamentals view for tracked companies: key financial figures pulled from quarterly and annual reports, tracked per reporting period, and charted over time. The intent is to cut the time an investor spends re-reading reports to find the same handful of numbers each quarter.

User-facing behavior:

- a fundamentals panel in the company workspace presenting figures as a KPI-row × reporting-period-column matrix, where every value links back to the report it came from; a fact's detail shows its period, value, source, and status
- figures are shown the way the report printed them (e.g. `319,7 mln PLN`, `8,63 PLN`) using the as-reported figure captured at extraction, falling back to a locale-aware formatted base value for manually entered numbers; KPI names are localized
- a fixed set of standard figures (revenue, operating profit, net profit, EBITDA, EPS, gross/operating/net margin, net debt, cash) plus the ability to define custom figures for a company that the standard set does not cover, such as subscribers, stores, or order backlog; the add-fact KPI picker supports inline search
- AI assistance that reads a stored report and proposes figures, reviewed in a centered modal where the user confirms, edits, or rejects each value, with bulk "confirm all known" and "accept all suggestions" actions; the modal auto-closes when nothing is left to review and no AI-proposed number is stored as a confirmed figure without review
- the user can also enter and correct figures manually against a report
- inline sparkline trends per KPI plus a larger per-KPI trend chart over comparable periods (built with in-house SVG primitives, no charting dependency), and (in v0.53.0) side-by-side comparison of the same figure across companies
- the panel and the review modal remain usable in tall, narrow windows (e.g. a quarter of an ultrawide monitor), stacking or shrinking rather than clipping

**Trust ladder for automated extraction ([ADR 0061](adr/0061-deterministic-fundamentals-data-gathering.md)).** For a company opted into the autonomous report pipeline, deterministic structured extraction (ESEF/iXBRL, or a PDF whose reporting period is derivable) runs *before* AI, in both `assist` and `autopilot` mode. A figure that clears the objective validation gate (accounting identities, comparative cross-check, structure match) auto-confirms outright — no unreviewed grace period — in either mode; a figure that is uncontradicted but not fully proven still follows the per-company mode (auto-committed `auto_unreviewed` in autopilot, left `pending` for review in assist). A detected layout/structure change never silently blocks extraction — it falls through to the AI proposal path — but is always surfaced as a "structure changed" signal, never dropped: the fact detail shows its source tier (ESEF/tagged, structured HTML, PDF, aggregator, or **AI** — the AI-confirmed tier the pool falls through to) and validation status badges, and a drifted fact's detail shows the clean label diff (new/missing report lines, reporting-unit change); the same "structure changed" signal and label diff surface on the autopilot run's notification card on Today when a run's extraction carried it, so the user sees it without opening the company.

Scope boundary: this covers report-derived fundamentals only. Price and volume charts, technical indicators, valuation tooling that needs live prices, and market dashboards stay out of scope, as recorded in [ADR 0027](adr/0027-company-fundamentals-scope.md). AI fundamentals features are part of the open core and free to use with a user-supplied provider API key.

### From docs/product-spec.md — "Future Experience Directions" (Report Documents And History Backfill)

Milestone v0.41.0 persists the actual report files and removes the cold-start problem when tracking a company ([ADR 0036](adr/0036-report-document-storage-and-backfill.md)).

User-facing behavior:

- official ESPI/EBI report attachments are stored locally as report documents with durable attribution and linked from the company's evidence/timeline; periodic/financial reports keep the full file (so AI extraction and diff have a real document to cite), while routine filings keep only the link and title
- the existing escape hatch — pasting a report PDF URL, or resolving one from the company's IR page — still stores the full file and flows through the same downstream path
- an explicit **"Backfill history"** action on a tracked company (on track, or from the company workspace) fetches roughly the last 3 years of periodic reports and ESPI/EBI filings, so the company's research timeline is populated with prior years instead of starting at "now"; items appear with their original publication dates
- backfill shows progress and diagnostics while it runs, can be cancelled, and is safe to re-run (no duplicates); it never runs automatically and only runs while the app is open
- dividend and general-meeting filings that state a future date produce a **proposed** calendar event the user confirms before it appears on the calendar; a date is never guessed onto the calendar

Scope boundary: backfill covers official report sources only (not media), does not backfill historical calendar entries (the calendar focuses on upcoming events), and does no per-company PDF parsing or ESEF/iXBRL parsing.

### From docs/product-spec.md — "Future Experience Directions" (Report-Over-Report Diff)

Milestone v0.47.0 lets the investor see what actually changed between two consecutive periodic **financial statements** instead of rereading the whole filing ([ADR 0052](adr/0052-report-over-report-diff.md)).

User-facing behavior:

- from a company's report documents (and on new periodic-report arrival), the investor can open a **section-by-section diff** of a financial statement against the previous same-type filing (consolidated SSF vs the prior SSF, standalone JSF vs the prior JSF)
- each section is shown as unchanged, changed, or only-in-one-report; changed sections show the textual differences, with both reports cited so the investor can jump to the source
- the diff is **deterministic and fully local** — pure-Rust text extraction, no AI and no network; the same two reports always produce the same diff, and a report compared against itself shows no changes
- a report whose PDF has no extractable text layer (scanned) shows an explicit "can't diff — no extractable text" state rather than a misleading empty diff

Scope boundary: v0.47.0 diffs the **structured financial statements only**. The narrative management report (MD&A) diff and an AI-written "what changed / new risks / tone shift" delta summary are deferred to a later milestone — a real-data spike showed narrative section headings drift too much for trustworthy deterministic alignment ([ADR 0052](adr/0052-report-over-report-diff.md)). Financial-table *value* reconciliation stays with KPI extraction, not the diff. No cross-company diffing.

### From docs/data-model.md — Research Evidence Boundary (M24/M25 milestone narration)

M24 uses a hybrid model governed by [ADR 0022](adr/0022-research-evidence-read-model-boundary.md) (bullets retained live in data-model.md; framing sentence only, moved here):

- M25 uses one company-level review checkpoint per company for the first visible Research screen.
- Timeline summary counts and changed-only filtering are derived read-model behavior; no stored timeline projection is added for M25.

### From docs/source-strategy.md — "M6 Source Investigation Results"

The current source ranking for GPW report bodies is:

1. GPW detail page body extraction remains the primary path. The listing page links to `komunikat?geru_id=...` detail pages, and sampled detail pages include the official PAP-rendered report content under `.report-data`. The markup is noisy, but the report body and sections are present without login.
2. PAP Biznes is the strongest official-adjacent fallback candidate because GPW detail pages embed PAP report markup and rewrite some report links to `https://espiebi.pap.pl/espi/pl/reports/view/...`. PAP also exposes public ESPI/EBI pages on `biznes.pap.pl`. PAP terms and direct URL patterns still need a separate review before implementation.
3. Bankier and Parkiet RSS are technically useful as secondary cross-check, diagnostics, and emergency fallback signals. They are not original GPW/PAP source paths, so they should not become the canonical official-report source without a later ADR.
4. Stooq is useful for general company and market news, but current investigation did not find a dedicated ESPI/EBI report-body path comparable to GPW or Bankier. Treat it as a later media/news candidate, not a primary report-body source.

Do not weaken the v1 requirement because a source is inconvenient. If GPW parsing becomes unreliable, the next step is to harden the parser or evaluate PAP/Bankier/issuer alternatives, then explicitly discuss the trade-off before changing the roadmap.

**Superseded (2026-07-02):** this ranking predates the M8 decision to run Bankier Company Komunikaty as the active v1 official-report source and disable `gpw-espi-ebi` pending a tracked-company-scoped fetch path (see the live "GPW ESPI/EBI Adapter" and "Media/RSS Sources" sections of source-strategy.md). PAP Biznes was never pursued; BiznesRadar + Bankier "wyniki finansowe" became the accepted aggregator/fallback tier instead ([ADR 0061](adr/0061-deterministic-fundamentals-data-gathering.md)).

### From docs/source-strategy.md — "Source Candidate Study"

M22 includes a study task for all currently registered or documented source candidates. A candidate must not become visible in normal UI or user-enableable until its study records an accepted source path, usage posture, attribution, matching strategy, rate limit, tests, and implementation scope.

Current candidates to study:

- GPW ESPI/EBI official reports reliability path.
- Portal Analiz authenticated private research.
- Bankier Firma RSS.
- Bankier Wiadomości RSS.
- Strefa Inwestorów report calendar.
- Money calendar/report-date pages.
- Bankier ESPI RSS and Parkiet ESPI/EBI RSS as secondary official-report cross-checks.
- PAP Biznes ESPI/EBI fallback path.
- Investing.com Poland RSS categories.
- Stooq ticker news and price/context endpoints.
- XTB market news and analysis pages.
- BiznesRadar and StockWatch public or authenticated research candidates.
- Future official/public company and filing sources such as SEC EDGAR, Nasdaq RSS, and major European exchange feeds.

Study output should classify each candidate as one of:

- accepted for implementation,
- keep as documented future candidate,
- developer-only diagnostic/cross-check candidate,
- rejected for policy, reliability, matching quality, licensing, cost, or UX reasons.

Normal Sources must not show candidates that are not implemented. Developer mode and owner/developer docs may expose candidate status and study notes.

Initial M22 candidate disposition:

- GPW ESPI/EBI official reports: keep as developer-only reliability candidate; next action is tracked-company-scoped fetch reliability research before runtime promotion.
- Portal Analiz authenticated private research: keep as developer-only authenticated-source candidate governed by ADR 0014; next action is account-scoped usage-path research before any implementation.
- Bankier Firma RSS: keep as documented future candidate; next action is matching-quality review against tracked GPW companies.
- Bankier Wiadomości RSS: keep as documented future candidate; next action is noise/backfill and matching-quality review before considering ingestion.
- Strefa Inwestorów report calendar: keep as documented future candidate; next action is source-specific parsing and attribution review.
- Money calendar/report-date pages: keep as documented future candidate; next action is source-specific parsing and matching review.
- Bankier ESPI RSS and Parkiet ESPI/EBI RSS: keep as developer-only diagnostic/cross-check candidates; next action is reconciliation research against canonical official-report items.
- PAP Biznes ESPI/EBI fallback path: keep as documented future official-adjacent fallback candidate; next action is policy/terms and URL-pattern review.
- Investing.com Poland RSS categories: keep as documented future public-media candidate; next action is exact feed discovery and ticker/company matching review.
- Stooq ticker news and price/context endpoints: keep as documented future media/price-context candidate; next action is separate source-type decision for news versus price context.
- XTB market news and analysis pages: keep as documented future media/analysis candidate; next action is source policy and attribution review.
- BiznesRadar and StockWatch: keep as documented future research/context candidates; next action is public/authenticated access and scraping-policy review.
- SEC EDGAR, Nasdaq RSS, and major European exchange feeds: keep as future official/public market expansion candidates after GPW/NewConnect directory support is stable.

### From docs/source-strategy.md — "Source Research Notes"

Current checked references:

- GPW ESPI/EBI report listing: `https://www.gpw.pl/komunikaty`
- GPW public page shows report listings under "Raporty Spółek ESPI/EBI" with timestamps, report type, ESPI/EBI label, company name/ISIN, report title, and detail links.
- GPW also describes processed information products available in CSV or XLS under its information services area; these are future options, not v1 assumptions.
- No documented public GPW ESPI/EBI API has been accepted for v1 yet. M6 found an internal GPW AJAX listing endpoint used by the public page; this is cleaner than full-page listing parsing, but still must be treated as a public-page implementation detail rather than a contracted API.
- PAP Biznes exposes ESPI/EBI-related public pages, including `https://biznes.pap.pl/espi` and `https://biznes.pap.pl/ebi/company`, and describes ESPI/EBI communications in its business service offer. Treat PAP as a source candidate only after policy and terms review; do not implement PAP scraping by default.
- Bankier exposes ESPI/EBI listings and article pages, including a JSON listing endpoint used by its UI. Treat Bankier as a secondary cross-check/fallback candidate, not the primary v1 official source.
- Bankier per-company komunikaty pages canonicalize short GPW tickers to Bankier instrument slugs such as `CDR -> CDPROJEKT` and `PKN -> PKNORLEN`, expose `data-tag-id` values, and feed the public article listing JSON endpoint with `tags_ids`, `pub_id=3,379`, `sort=time_utc desc`, and article fields.
- Bankier exposes `https://www.bankier.pl/rss/espi.xml`, which is a convenient secondary ESPI feed. It should not replace GPW canonical ingestion.
- Bankier's RSS directory also lists general market/news channels, including Giełda and Wiadomości dnia categories. Treat these as media/news candidates, not official-report sources.
- Parkiet exposes `https://www.parkiet.com/rss/7111-komunikaty`, which is a convenient secondary ESPI/EBI feed. It should not replace GPW canonical ingestion.
- Stooq exposes ticker news pages and PAP-sourced market news, but current investigation did not reveal a dedicated ESPI/EBI report-body source path suitable for v1 report ingestion.
- Stooq also exposes CSV quote/history endpoints used by community tooling. Treat this as a later price-context candidate, not as a news or report-body source.
- Investing.com Poland exposes an RSS directory with news and analysis categories, including company-news-style categories. Exact feed URLs and GPW ticker matching quality need source review.
- `wegar-2/pyespiebipapapi` is a community scraping wrapper for PAP ESPI/EBI pages and can inform PAP fallback research, but it is not an official PAP API and should not be depended on directly in the Rust/Tauri app.
- XTB market news and analysis pages are candidate media/analysis sources, not official-report sources, and need source review before ingestion.
- Portal Analiz is desired for v1 as an authenticated private research source using the user's own paid account and is governed by ADR 0014 before implementation.

### From docs/source-strategy.md — "Open Source Questions" (resolved/superseded items)

- Should the GPW listing fetcher switch from full-page GET to the observed `ajaxindex.php` listing fragment POST? — **Resolved**: yes, the live `gpw-espi-ebi` adapter uses the AJAX fragment endpoint for listings (see "GPW ESPI/EBI Adapter").
- Do GPW detail pages expose stable report body structure and attachments across enough report types? — **Resolved**: yes, accepted per [ADR 0013](adr/0013-gpw-detail-fetching-policy.md) (see "Detail Fetching").
- Can PAP detail URLs be mapped directly from GPW/PAP identifiers without relying on search? — **Superseded**: PAP was never pursued as a source; BiznesRadar + Bankier became the accepted aggregator/fallback tier ([ADR 0061](adr/0061-deterministic-fundamentals-data-gathering.md)).
- Is PAP Biznes usable under acceptable terms for local personal polling, or is it a commercial/protected source unsuitable for v1 ingestion? — **Superseded**, same reason.
- Which Bankier RSS channels have enough company-specific signal to justify a v1 media adapter? — **Substantially resolved**: `bankier-market-rss` (Giełda) was accepted and implemented first; `bankier-firma-rss`/`bankier-wiadomosci-rss` remain disabled reviewed placeholders pending matching-quality proof (see "Media/RSS Sources").
- Are StockWatch/BiznesRadar technically and legally acceptable as scraping targets, or should they stay manual/open-in-browser references? — **Resolved**: BiznesRadar accepted as the primary aggregator witness (Bankier "wyniki finansowe" fallback), StockWatch not chosen ([ADR 0061](adr/0061-deterministic-fundamentals-data-gathering.md)).
- Which Polish media/RSS sources should be considered after GPW official reports? — **Resolved/covered live**: candidate list and the M8 acceptance ranking are in "Media/RSS Sources".
- Which public Polish article/news source should be implemented first after GPW details: Stooq, XTB, Bankier, or another source? — **Resolved**: Bankier Giełda RSS, implemented first (M8).

### From docs/ui-information-architecture.md — "Company Workspace (retired)" framing and Milestone 3/4 interaction narration

> **Superseded by [ADR 0057](adr/0057-composable-views-and-curated-dashboard.md).** The click-through tabbed company workspace is retired. The single-company deep-dive is now the **curated cockpit dashboard**: a seeded `cockpit_layout` scoped to the company that opens with a calm default panel set (Fundamentals, Feed, Claims, Quality, Report documents, Notebook) and stays composable (add/remove/move panels, then `Save dashboard` to persist per company). Each tabbed section below is now a **company-scoped cockpit panel** (Feed → `companyFeed`, Notebook → `companyNotebook`, Fundamentals/Claims/Quality/Report documents/diff as their panels); Metadata folded into the Companies library; Transcripts remains a future-milestone placeholder. The descriptions below are retained as the behavioral spec for those panels.

The section was headed "Tabs or segmented views" (Feed, Notebook, Claims, Transcripts, Fundamentals, Metadata) — now cockpit panels; the behavioral spec for each panel moved live into "Company Cockpit Dashboard Panels".

Retired Milestone-3 interaction narration (Feed tab):

Milestone 3 starts with inline source/origin detail inside the Company Feed tab. The feed row itself is the click target instead of a separate inspect button. Clicking a feed row, or pressing Enter/Space on a focused feed row, expands the detail directly under that row; repeating the action collapses it. Up and Down arrows move through company feed rows while preserving expansion state: collapsed feed details stay collapsed, and already-open detail moves to the focused feed row. Company feed rows use the same unread dot and read/unread typography as Inbox feed rows.

Retired Milestone-4 interaction narration (Notebook tab):

Milestone 4 begins with a company-scoped Notebook tab that lists durable notes and provides a compact manual Markdown note form. Feed-item note drafts are added after the base manual-note path is stable.

Retired Milestone chronicle (Claims tab):

Milestone v0.42.0 makes claims a first-class entity (existing claim notes are migrated).

### From docs/ui-information-architecture.md — "Future Research Workspace" (delivered M25/M26/M29/M31 chronicle)

Purpose: provide source-grounded company and watchlist review surfaces once the research/evidence boundary exists.

Candidate regions:

- company or watchlist selector
- evidence timeline
- changed-since-review summary
- open claims and research questions
- upcoming events and reminder pressure
- AI research brief panel with citations
- related evidence links

Rules:

- Do not add visible Research Workspace placeholders before the feature is implemented.
- M25 implements Research as a real top-level screen, scoped to a selected tracked company.
- M25 Research shows a screen header with selected company context, last-reviewed state, total evidence count, changed-since-review count, evidence type filters, changed-only filter, and a `Mark reviewed` action.
- M25 Research evidence rows are compact timeline rows with product-language type/trust labels, occurred timestamp, title, summary when available, changed-since-review state, and owning-domain/source actions where practical.
- M31 Research shows open reminders and generated digest snapshots as real company/watchlist review tools. The screen receives reminder and digest read models from backend commands; it must not assemble digest inputs in React.
- M26 Research adds a Company/Watchlist mode switch in the same screen, not a separate screen.
- Watchlist mode uses a left-side member-company review queue and a right-side evidence timeline for the selected member company.
- Watchlist review defaults to updating the watchlist checkpoint only. The optional cascade to member-company checkpoints must be an explicit user action and backend command input.
- M29 Research adds a company-scoped Questions panel in the same Research screen. The user creates, selects, answers, closes, reopens, and links questions to evidence without leaving the research context.
- The selected research question controls link actions on evidence rows. Evidence rows may show a compact link action only when a selected question can link to that evidence item.
- Future research views consume backend-owned evidence and timeline read models.
- Research views should not independently call Inbox, Notebooks, Events, Transcripts, Sources, and AI APIs to assemble timelines.
- Research summary counts, changed-since-review state, and timeline filtering semantics belong to backend read models. The UI displays the result and captures user intent.
- Review actions update research-owned review checkpoints.
- Evidence-link interactions use typed research links while preserving existing notebook origin links.
- AI brief UI must show citations and provenance enough for source-grounded review, without presenting buy/sell/hold recommendations.

### From docs/ai-analysis-framework.md

Retired 2026-07-03 (ADR 0065 evidence-based retire policy: the doc was never loaded into agent context across 20 days of session history, stale since 2026-06-19, superseded by later ADRs). Full M13/M30-era design record, kept verbatim as chronicle; current behavior is [ADR 0028](adr/0028-multi-provider-ai-boundary.md) (multi-provider boundary), [ADR 0035](adr/0035-two-layer-ai-and-local-interpretative-layer.md) (two-layer AI), [ADR 0060](adr/0060-ai-capability-routing-and-openai-compatible-provider.md) (capability routing), [ADR 0061](adr/0061-deterministic-fundamentals-data-gathering.md) (deterministic fundamentals), and [Architecture](architecture.md).

**M13 Goal.** M13 added source-grounded feed-item analysis using Gemini as the first live provider while keeping the runtime extensible for OpenAI, Anthropic, and other future providers (since delivered — [ADR 0028](adr/0028-multi-provider-ai-boundary.md)). The app analyzes local source material as decision support only; analysis must cite source material and must not be presented as buy/sell/hold advice.

**Accepted architecture decisions (M13).**

- Provider-neutral interface: app, command, storage, and UI code use an `AiAnalysisProvider`-style contract; Gemini was an adapter behind that boundary, not the domain model.
- Async job model: AI analysis runs through an asynchronous job so the UI is not blocked while a provider request is in progress (generalized to all AI features by ADR 0028 decision 3).
- Explicit user action: analysis starts from a visible user action, not automatically during ingestion or feed-detail open.
- Settings surface: general AI analysis provider/model configuration and provider disclosure before feed-item analysis was broadly used.
- Credential boundary: general analysis credentials stayed under the reusable provider credential boundary, purpose-aware (the same Gemini API key could satisfy multiple Gemini purposes when configured). **Superseded by ADR 0028 decision 4**: purpose is no longer part of credential identity — one `CredentialDescriptor` per provider only.
- Initial source context: M13 analyzed feed-item title, summary/body text, source URL, attribution, company context, and language when available; attachments and transcript-derived analysis were later slices (since delivered — document input, ADR 0028 decision 6).
- Prompt/version provenance: persisted results store provider ID, model, prompt version, source references, and created timestamp (still true, now documented live in [data-model.md](data-model.md) and [contracts.md](contracts.md)).
- Recommendation enforcement deferral: M13 prompt policy said not to generate buy/sell/hold or portfolio advice; automated post-generation recommendation-language detection was deferred as a post-v1 task.

**Non-goals (M13, since superseded by delivered scope).** No automatic batch analysis during source ingestion; no portfolio-aware or personalized advice; no opinionated mode; no hosted services, telemetry, remote logs, or cloud queues; no attachment ingestion or transcript-segment analysis in the first M13 slice (delivered later); no post-generation recommendation-language validator in M13; no research-brief generation in M13 (delivered as M30, see below).

**Rust/Frontend ownership (M13 module boundary, since restructured).** M13 expected `src-tauri/src/providers/analysis/` (provider-neutral types, Gemini adapter, deterministic test provider), `jobs/ai_analysis.rs`, `storage/ai_analysis.rs`, `commands/ai_analysis.rs`, `storage/settings.rs`, `providers/credentials.rs`; and frontend `src/api/aiAnalysis.ts`, `AppStateRoot.tsx`/a focused controller, `src/screens/Inbox/`, `src/screens/Companies/`, `src/screens/Settings/`, `src/shared/locale/`. ADR 0028 restructured the Rust provider layer into `providers/analysis/{types,registry,transport,document,gemini,anthropic,openai}.rs`; current ownership is documented live in [modularization-design.md](modularization-design.md).

**UI/UX direction (M13 ideation).** M13 considered an AI research-panel pattern inspired by AI-powered finance products (e.g. Google Finance), adapted to Brawler's local-first workflow: suggested prompts (source-grounded presets like "summarize what changed", "extract risks", "identify management claims", "explain why this may matter"), a custom question field, a short local follow-up thread per feed item, visible async progress, cited results, decision-support framing copy, and an optional animated accent border respecting reduced-motion. The implementation was directed not to copy another product's visual design directly.

**Runtime flow (M13).** User opens a feed item and selects an analysis action/prompt/question → frontend calls a typed command to create/reuse an analysis job → Rust loads feed context and settings → Rust resolves provider credentials without exposing secrets to React → the async job calls the provider and maps output into the neutral result contract → storage persists job status and, on success, result fields plus source references → UI polls/refreshes through typed commands and renders queued/running/succeeded/failed → user can retry a failed job.

**M30 Research Brief Boundary.** Company/watchlist research briefs reuse the provider-neutral AI posture without turning feed-item analysis into a catch-all module. Expected ownership: evidence collection belongs to the research/evidence domain; prompt/context building belongs to a research brief builder; provider execution uses the provider/job boundary; citation mapping links generated claims back to research evidence; rendering converts the stored brief into UI-facing read models; persistence stores brief provenance, rendered content, citations, provider ID, model, prompt version, and generation timestamps. Briefs are dedicated entities, not notebook entries by default, though may be converted through an explicit user action later. M30 accepted decisions: generation is explicit/on-demand only; both company and watchlist brief scopes are in scope; the initial provider configuration reused the general analysis provider/model settings; evidence collection used a backend-owned default collector for the selected scope; provider output was structured into sections with citation IDs then rendered by the backend; briefs are immutable snapshots (regeneration creates a new brief); citations store evidence references and short labels/snippets only, not full copied source bodies; briefs/citations are durable research data included in research import/export; creating notebook notes from briefs stayed out of scope for M30 and must never happen automatically. This boundary is delivered and now documented live in [modularization-design.md](modularization-design.md) § "Research/Evidence Boundary Ownership" (AI brief posture).

**Initial commands (M13, names since evolved — see [contracts.md](contracts.md) for the current AI analysis/brief/digest command surface).** `start_ai_analysis(input)`: create or reuse an analysis job for a feed item, start async processing, return current job/result state. `list_ai_analysis(input)`: return current analysis jobs/results for one feed item. `retry_ai_analysis(jobId)`: rerun a failed analysis job with current settings.

**Settings (M13, since superseded by ADR 0060 per-capability `capability_providers` routing).** `generalAnalysisProvider` (initially `provider_gemini` or unset), `generalAnalysisModel`, `generalAnalysisTimeoutSeconds`, `aiAnalysisMode` (`source_grounded`). Settings had to disclose that feed-item source text/metadata are sent to the configured provider when the user starts analysis.

**Test strategy (M13).** Default checks must not require secrets or live external services. Required M13 test areas: storage migration/persistence for jobs/results/source references; settings defaults/updates/invalid-value handling; provider-neutral request/result mapping; deterministic test-provider job flow; Gemini adapter mapping with mocked HTTP; UI workflow for explicit analysis start/loading/success/failure/retry; Settings workflow for general AI analysis configuration. Live Gemini analysis smoke testing was to be opt-in and documented if it became part of M13 closure evidence.

### From docs/modularization-design.md — "Historical Pain Points"

The M13 modularization pass was started because these files had become mixed-responsibility architecture debt: `src/App.tsx` (app shell, routing, data loading, screen rendering, workflow logic, formatting helpers, and mutations mixed in one file); `src/App.test.tsx` (many unrelated workflows sharing one mock setup and one large test file); `src/styles.css` (app-wide and screen-specific styling mixed); `src-tauri/src/storage.rs` (migrations, data types, settings, source state, companies, watchlists, feed, notebooks, events, transcripts, and tests mixed in one module); `src-tauri/src/lib.rs` (Tauri command registration, command handlers, refresh orchestration, transcript runner orchestration, and source adapter dispatch mixed). All resolved by the M13 pass; current structure is documented live in modularization-design.md.

### From docs/modularization-design.md — "Findings"

Broad extraction is useful when a file mixes multiple architectural layers; it is less useful when a file is a cohesive domain view or facade. `AppStateRoot.tsx` is intentionally large because it coordinates app state — splitting it should wait for a feature-driven state-domain boundary, not a line-count target; the AI-analysis domain (per-feed-item jobs/error/in-flight maps, poll timers, start/retry commands, load+poll effects) was extracted to `src/app/useAiAnalysisController.ts` on exactly that basis, a cohesive self-contained boundary. The signals and fundamentals domains were intentionally left in place because their state fed `useAppViewModel` derivations; extracting them cleanly needed a coordinated controller/view-model boundary, not an in-place lift. Shared frontend primitives are useful only when they preserve existing class semantics and accessibility: `Button`, `EmptyState`, and `StatusPill` were adopted for generic controls; segmented controls, row selectors, field clear buttons, collapsible headers, suggestion rows, and anchor links stayed native/domain-specific on purpose (rule retained live in modularization-design.md § Frontend Ownership Rules). Screen tests became easier to reason about after screen extraction, but the shared app workflow harness remained useful for integration-style UI flows. Rust command modules should stay thin — most complexity belongs in storage, provider, source adapter, or job modules (rule retained live in modularization-design.md § Rust Ownership Rules). Storage facades are acceptable when they keep the public boundary clear and delegate domain behavior to focused modules. CSS extraction worked best after screen/component extraction stabilized.

### From docs/modularization-design.md — "Completion Note" (narrative)

The broad M13 modularization effort is complete. This document is treated as a standing architecture checklist, not a backlog of remaining extraction tasks. (Live extraction-trigger checklist and known-acceptable-large-files list retained in modularization-design.md § Future Extraction Triggers.)

### From docs/modularization-design.md — "M24 Large-File Responsibility Audit"

M24 reviewed the large/coordinating files in light of the research-workspace roadmap; the rule remained responsibility-first (line count is a signal to inspect, not a reason to split by itself). Required M24 extractions completed: `src/api/types.ts` (research/evidence DTOs moved to `src/api/researchTypes.ts`); `src/api/research.ts` added for research command calls; `src-tauri/src/storage/mod.rs` kept as the storage facade with research behavior extracted to `src-tauri/src/storage/research.rs`; `src-tauri/src/commands/research.rs` added; `src-tauri/src/storage/tests/research.rs` added for research boundary tests. Deferred until feature pressure (since resolved by later milestones): `AppStateRoot.tsx` (no split for M24, no visible research UI/state yet — M25 later added a focused research controller/view-model); `appWorkflowHarness.tsx` (split deferred until research UI workflow tests made setup harder); `storage/import_export.rs` (M24 research-owned durable state deferred from M20 export files); `CompanyWorkspace.tsx`/`NotebooksScreen.tsx` (M25/M27 later extracted research-facing timeline/review/evidence-link UI). No action taken in M24 on `lib.rs`/`commands/mod.rs` (remained registration facades), `storage/sources.rs`/`storage/transcripts.rs`/source adapters/source tests (outside M24 scope), and `storage/metrics.rs` (unrelated cohesive observability domain). Live forward-looking rule retained in modularization-design.md: if a future feature needs stored timeline/evidence projections, add the projection implementation behind the existing research/evidence API rather than moving aggregation into screens or broadening the storage facade.

### From docs/modularization-design.md — "Research Workspace Readiness Check"

Before implementing the (then-future) research-workspace direction, M24 ran a focused modularization readiness check to decide whether existing boundaries could support a shared research evidence model, timeline/read model, review workflow, research questions, claim expansion, source-grounded AI briefs, digests, reminders, and evidence links. Areas inspected: frontend app state/view-model ownership (whether `AppStateRoot` should compose a new research controller); frontend API DTO ownership (whether research evidence/timeline/review types should live outside the generic `src/api/types.ts`); screen ownership between Companies, Notebooks, Events, Inbox, Sources, and a future Research/Review surface; Rust command ownership for research evidence, review state, links, questions, reminders, and generated briefs; storage ownership for cross-domain evidence/read models versus existing feed/notebook/event/transcript/AI-analysis tables; import/export, backup, retention, and migration impact for new research-workspace entities; AI brief boundaries for evidence collection, prompt/building, provider execution, citation mapping, rendering, and persistence; test ownership for evidence aggregation, review state, linking integrity, citation grounding, and real-browser workflow/layout coverage. Output was a refactor-before-feature decision list (required before implementation / foldable into the first feature slice / defer until concrete pressure / no action needed). The resulting accepted research/evidence boundary (ADR 0022) is retained live in modularization-design.md § "Research/Evidence Boundary Ownership" — the readiness-check process itself was a one-time M24 planning exercise, now superseded by the delivered Research screen (M25/M26/M29/M31, see [ui-information-architecture.md](ui-information-architecture.md) and [ADR 0022](adr/0022-research-evidence-read-model-boundary.md)).
