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

### Implement keyboard shortcuts

Intent: add discoverable keyboard shortcuts for repeated v1 workflows after core screens are stable.

Acceptance criteria:

- Shortcut map covers common Inbox actions first.
- Shortcuts are documented in Settings or Help/About.
- Every shortcut action remains available through visible UI controls.
- Shortcuts do not fire while typing in inputs, note editors, forms, or transcript selection.
- Windows-native and browser editing shortcut conflicts are avoided.

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

### Implement company notebooks

Intent: add per-company notebook support with notes that can be created manually or from feed items.

Acceptance criteria:

- Each company has a notebook view.
- Notes support title, body, tags, provenance, kind, claim status, event date, and review period.
- Feed item detail view can create a note draft linked to that feed item.

Docs/contracts touched: product spec, contracts.

Test expectations: Rust notebook storage tests and UI workflow tests.

### Implement Gemini YouTube transcription spike

Intent: validate Gemini as the first provider only for YouTube press conference transcription and transcript-to-note workflows.

Acceptance criteria:

- User can submit a YouTube URL for a selected company.
- App creates a transcript job using the Gemini provider.
- Returned transcript segments can be reviewed.
- User can save selected segments as notebook notes with provenance.
- Settings disclose free-tier limits and provider privacy terms.

Docs/contracts touched: architecture, product spec, contracts, source/AI policy ADR.

Test expectations: provider contract tests with fixtures and transcript-to-note workflow tests.

### Implement GPW ESPI/EBI adapter spike

Intent: validate the first official GPW source path.

Acceptance criteria:

- Adapter can fetch and normalize recent public GPW ESPI/EBI report listings.
- Adapter stores source URL, timestamps, language, company match, and attribution.
- Adapter primarily matches companies by ISIN.
- Rate limit and source policy are documented.
- Detail-page fetching is tested separately from listing ingestion.

Docs/contracts touched: contracts, architecture, source-specific ADR if scraping is required.

Test expectations: adapter unit tests with fixtures.

## Ready

Milestone 2 is closed. Start Milestone 3 work on a separate branch after merging the Milestone 2 branch to `master`.

## In Progress

### Replace fixture inbox with stored feed items

Intent: move the Inbox from hard-coded fixture rows to feed items stored in SQLite.

Acceptance criteria:

- Feed items are read from local storage.
- Inbox watchlist, company, source, type, unread, and saved filters apply to stored feed items.
- Inbox search applies to stored feed items by company, title, source, type, and summary.
- Active feed filters can be cleared in one action, including from the empty state.
- Selecting a feed item updates the detail pane with source URL, timestamps, attribution, and summary.
- Source URL remains directly actionable from the detail pane.
- Read/unread and saved/unsaved changes persist in SQLite.
- Empty states distinguish no tracked companies from no matching items.
- Fixture data is only used in tests or development seeding.

Current slice:

- SQLite-backed feed item read model exists.
- Development fixture feed rows are seeded only when the local feed is empty.
- Read/unread and saved/unsaved state updates round-trip through Tauri/Rust and persist in SQLite.
- Topbar source refresh is a disabled placeholder until real source adapter refresh jobs exist.
- DB status pill reloads local SQLite-backed app state as a small utility action.
- Sources screen lists configured SQLite-backed source adapters and their status.
- Feed detail pane is scoped to Inbox instead of appearing on unrelated screens.
- Native select options use explicit theme colors so dark-mode dropdowns remain readable.
- Inbox feed/detail split can be resized with a drag handle between panels.

Docs/contracts touched: product spec, contracts, data model.

Test expectations: Rust feed storage tests and UI filter workflow tests.

## Review

No cards.

## Done

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

### Add fixture-backed company lookup

Intent: make company creation less manual by filling ticker, name, and ISIN from an exchange-scoped lookup.

Acceptance criteria:

- User can request lookup from the Companies form.
- Exact ticker or ISIN lookup fills missing company fields when a fixture match exists.
- Name lookup can find a fixture match when the entered name is specific enough.
- Manual company entry remains possible.
- Lookup source is clearly fixture/local for now and replaceable by a future registry adapter.

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
- Claim review date/quarter behavior is selected.
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
