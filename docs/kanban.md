# Kanban

Active work only. Completed-card history lives in [Kanban Archive](kanban-archive.md) to keep day-to-day agent context small.

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

### M12.6 Add shortcut framework and discoverability shell

Intent: create the shared shortcut infrastructure before adding screen-specific commands.

Acceptance criteria:

- Shared keyboard shortcut hook or manager exists under the appropriate shared/app boundary.
- Shortcut registration supports app-level and screen-level commands.
- Shortcuts do not fire while focus is in inputs, textareas, note editors, transcript selection controls, or other editable fields.
- Windows/browser editing shortcut conflicts are avoided.
- Settings or Help/About exposes a discoverable shortcut reference.
- Every listed shortcut maps to a visible UI action.

Docs/contracts touched: UI information architecture if shortcut reference placement changes.

Test expectations: unit/component tests for shortcut suppression, registration, and discoverability rendering.

### M12.7 Add Inbox keyboard shortcuts

Intent: make the daily Inbox review workflow efficient from the keyboard.

Acceptance criteria:

- Inbox shortcuts cover feed navigation, read/unread, save/unsave, opening source, search focus, and refresh.
- Existing mouse and visible-button workflows remain unchanged.
- Shortcuts respect current filters and selected item behavior.
- Shortcuts are suppressed while typing in search or other editable controls.
- Shortcut reference includes the Inbox shortcuts.

Docs/contracts touched: product spec or UI flows only if behavior changes beyond the existing M12 scope.

Test expectations: workflow tests for representative Inbox shortcuts and suppression while typing.

### M12.8 Add Company and notebook workflow shortcuts

Intent: add shortcuts where they reduce repeated company research and note editing work.

Acceptance criteria:

- Company workspace shortcuts improve navigation without replacing visible controls.
- Notebook shortcuts include `Ctrl+E` to open the editor for the selected note or claim.
- Notebook shortcuts include `Ctrl+S` to save the item currently being edited.
- `Ctrl+S` does not conflict with browser save behavior inside the app workflow.
- Shortcuts are suppressed or scoped correctly inside Markdown/note editing where normal text entry should win.
- Shortcut reference includes Company and Notebook shortcuts.

Docs/contracts touched: product spec or UI flows only if shortcut behavior needs more detail.

Test expectations: workflow tests for edit/save shortcuts and editable-field suppression.

### M12.9 Close M12 workflow polish

Intent: verify M12 end to end and close documentation/versioning once locale and shortcut workflows are stable.

Acceptance criteria:

- English and Polish locale workflows pass focused UI tests.
- Critical shortcut workflows pass focused UI tests.
- App build/typecheck/test commands pass.
- Roadmap records M12 completion status.
- Kanban M12 cards move out of active context.
- Version is bumped if this milestone closure is treated as a release boundary.

Docs/contracts touched: roadmap, kanban, product spec/contracts only if final behavior differs from planned scope.

Test expectations: normal local frontend and Rust checks appropriate to touched code.

## In Progress

## Review

### M12.5 Localize implemented screen copy

Intent: apply the locale boundary to static copy in the implemented screens without translating source/user content.

Acceptance criteria:

- Inbox, Companies, Notebooks, Events, Sources, Transcripts, and Settings use localized app-owned labels, buttons, empty states, and status text where they are visible in normal workflows.
- Shared components receive localized strings from callers or shared locale helpers rather than hard-coding English.
- Source-provided text, company names, ticker symbols, URLs, source attribution, transcript text, notebook titles/bodies, and fetched article/report text remain unchanged.
- Tests are updated only where they assert app-owned UI copy.

Delivered:

- Added a shared locale context and hook so implemented screens can consume the active locale without prop drilling.
- Added English and Polish resource coverage for implemented screen headings, descriptions, primary actions, filters, empty states, status text, form labels, accessible labels, and Settings labels/actions.
- Localized app-owned copy in Inbox, Companies, Notebooks, Events, Sources, Transcripts, the app shell, and implemented Settings sections.
- Localized app-owned controller messages that surface as validation errors, status feedback, watchlist feedback, and confirmation dialogs.
- Kept source-provided data, company names, ticker symbols, URLs, transcript text, and notebook/user content untranslated.
- Updated the app workflow harness to preserve mocked settings changes like the SQLite command response.
- Extended locale and Settings workflow tests to verify Polish screen copy, localized Settings controls, and localized screen actions.
- Ran the full frontend workflow suite after broad copy rewiring.

Docs/contracts touched: kanban.

Test expectations:

- `rtk npm typecheck` passed.
- `rtk npm test -- --run src/shared/locale/locale.test.ts src/screens/Settings/SettingsScreen.test.tsx` passed, 6 tests.
- `rtk npm test -- --run` passed, 68 tests.

### M12.4 Wire locale setting through Settings and app shell

Intent: let the user switch app language from Settings and apply it across the app shell.

Acceptance criteria:

- Settings shows a locale selector with English and Polish.
- English remains the first-run default.
- Changing locale persists to SQLite settings.
- App shell navigation, topbar labels, status labels, and shared empty/status primitives use localized app-owned copy.
- Locale changes apply without requiring an app restart unless a documented technical constraint appears.

Delivered:

- Added app-level `locale` state in `AppStateRoot`.
- Refreshed locale from SQLite-backed settings alongside theme.
- Added `updateLocale` to the settings controller.
- Added a Settings language selector with English and Polish options.
- Localized Settings title/description/application-settings label and Appearance labels.
- Localized app shell navigation labels, topbar search labels/placeholders, AI mode pill, source refresh labels, database status labels, theme selector labels, and health pending copy.
- Kept broader screen copy for M12.5.
- Updated the Settings workflow test to switch to Polish and verify Settings heading, navigation, and search placeholder update without restart.

Docs/contracts touched: kanban.

Test expectations:

- `rtk npm typecheck` passed.
- `rtk npm test -- --run src/screens/Settings/SettingsScreen.test.tsx` passed, 1 test.

### M12.3 Add extensible frontend locale resources

Intent: create the frontend i18n boundary used by app-owned UI copy.

Acceptance criteria:

- Shared locale resources exist for English and Polish.
- Locale lookup has a typed key structure or equivalent guard against ad hoc string lookup.
- Missing translation behavior is deterministic and does not crash the app.
- Resource structure can add future languages without rewriting screens.
- App-owned formatting labels use the selected locale where practical.
- Source-provided text, company names, ticker symbols, URLs, source attribution, transcript text, and notebook bodies are not translated.

Delivered:

- Added `src/shared/locale/index.ts` with supported locales, default locale, typed locale keys, normalization, locale display names, and translator helpers.
- Added English and Polish app-owned UI resources for app shell, navigation, Settings appearance labels, theme labels, and locale labels.
- Added deterministic fallback behavior: missing keys fall back to English and then to the key itself.
- Kept the module independent from Tauri/settings APIs so screens can consume locale helpers without crossing ownership boundaries.
- Added `src/shared/locale/locale.test.ts` for default locale, supported locales, normalization, English/Polish lookup, missing-key behavior, and localized locale names.
- Updated [Modularization Design](modularization-design.md) to list `src/shared/locale/`.

Docs/contracts touched: kanban, modularization design.

Test expectations:

- `rtk npm test -- --run src/shared/locale/locale.test.ts` passed, 5 tests.
- `rtk npm typecheck` passed.

### M12.2 Add locale setting contract and persistence

Intent: persist the selected app locale in the same SQLite-backed settings boundary as other runtime settings.

Acceptance criteria:

- Settings storage includes a `locale` value with default `en`.
- Initial supported locale values are `en` and `pl`.
- Invalid locale values are rejected or normalized through the typed settings boundary.
- Frontend settings API/types expose locale without changing unrelated settings behavior.
- Existing settings migrations/defaults keep older local databases working.

Delivered:

- Added migration `0022_app_locale.sql` to seed `locale = en`.
- Added `locale` to Rust `UserSettings` and optional `SettingsUpdate`.
- Validated locale updates against the initial supported values `en` and `pl`.
- Added storage tests for default locale, persisted locale updates, and invalid locale rejection.
- Updated schema/database-status tests for the new migration and settings row count.
- Added frontend `AppLocale` and `locale` settings DTO support.
- Updated frontend test harness settings mocks to carry locale.

Docs/contracts touched: kanban, contracts, data model.

Test expectations:

- `rtk cargo fmt --check` from `src-tauri` passed.
- `rtk cargo test settings` from `src-tauri` passed, 7 tests.
- `rtk cargo test schema` from `src-tauri` passed, 3 tests.
- `rtk npm typecheck` passed.
- `rtk npm test -- --run src/screens/Settings/SettingsScreen.test.tsx` passed, 1 test.

### M12.1 Design locale and shortcut implementation plan

Intent: turn the M12 roadmap scope into concrete implementation boundaries before editing app behavior.

Acceptance criteria:

- Locale ownership is assigned across settings storage, frontend app state, shared locale resources, and screen rendering.
- Shortcut ownership is assigned across shared hooks, app shell, screen-local handlers, and tests.
- English and Polish are confirmed as the initial supported locales.
- The locale design supports adding future languages through resources/configuration instead of per-screen rewrites.
- Shortcut conflict rules are explicit for inputs, textareas, note editors, transcript selection, and browser/Windows-native shortcuts.
- The plan identifies which screens need localized static copy in M12 and which copy can remain source/user-provided.

Delivered:

- Confirmed the existing settings ownership path: `src-tauri/src/storage/settings.rs`, `src-tauri/src/commands/settings.rs`, `src/api/settings.ts`, `src/api/types.ts`, `src/app/useAppDataController.ts`, `src/app/useSettingsController.ts`, and `src/screens/Settings/*`.
- Assigned locale resources and typed lookup helpers to a new focused shared locale module, such as `src/shared/locale/`.
- Assigned app-shell locale wiring to `src/app/AppStateRoot.tsx`, `src/app/AppShell.tsx`, and existing app settings controllers.
- Assigned screen localization to each owning screen while keeping source/user content out of translation.
- Confirmed English and Polish as the initial supported locales, with future locales added through resources/configuration.
- Assigned shared shortcut infrastructure to `src/shared/hooks/` or `src/app/` depending on whether the code is generic registration logic or app-level command wiring.
- Kept `useKeyboardListNavigation` as a local row/list navigation helper, not the general shortcut framework.
- Assigned screen-specific shortcut actions and tests to the owning screen modules.
- Documented shortcut suppression for inputs, textareas, note editors, transcript selection controls, editable controls, and Windows/browser editing conflicts.
- Updated [Modularization Design](modularization-design.md) with M12 locale and shortcut ownership rules.

Docs/contracts touched: kanban, modularization design.

Test expectations: docs-only planning card; no automated test required.

### Documentation Context Optimization

Intent: reduce the amount of documentation context future agents need to load while preserving all canonical project value.

Acceptance criteria:

- Active Kanban context contains active work and a pointer to completed-card history.
- Completed-card history remains available in a separate archive.
- Agent required reading is task-scoped instead of blanket-loading every canonical document.
- The project brief provides a clear routing map for which docs to read by task type.
- Cross-document "see also" lines point back to the project brief and only the most relevant local references.
- No product, contract, source-policy, security, testing, or modularization requirement is weakened.

Delivered:

- Added [Kanban Archive](kanban-archive.md) and moved completed-card history there.
- Kept active Backlog, Ready, In Progress, Review, and a Done archive pointer in [Kanban](kanban.md).
- Updated [AGENTS.md](../AGENTS.md) with task-scoped required reading.
- Reworked the [Project Brief](project-brief.md) document map into a task-oriented docs router.
- Shortened broad cross-document reference lines across the canonical docs.
- Updated architecture wording to treat [Modularization Design](modularization-design.md) as the current ownership guide, not a future extraction-order plan.

Docs/contracts touched: agent contract, project brief, kanban, kanban archive, architecture, canonical documentation headers.

Test expectations: docs-only change; link and stale-reference checks are sufficient.

## Done

Completed-card history has moved to [Kanban Archive](kanban-archive.md) so the active board stays small for day-to-day agent context.
