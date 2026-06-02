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
