# Kanban

## Backlog

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

### Scaffold desktop application

Intent: create the Tauri + React + TypeScript desktop shell with Rust domain modules.

Acceptance criteria:

- `flake.nix` and committed `flake.lock` provide the development environment.
- `nix develop` works on WSL2 Ubuntu 24.04.
- App build/test commands run inside `nix develop`.
- Tauri app starts on the development machine.
- React UI renders a basic investor inbox shell.
- UI supports dark and light theme selection with dark as the default.
- Initial visual tokens implement the night-neon blue, pink, and purple palette.
- Rust command `health` returns app status.
- Tauri events can notify the UI about job/feed updates.
- Local build/test commands are documented.
- GitHub Actions CI skeleton runs frontend and Rust checks without secrets.
- GitHub Actions uses the same commands as local development or thin wrappers.
- GitHub Actions validates the Nix setup if it remains fast enough.
- Default CI uses standard Linux runners only and avoids larger runners, scheduled jobs, and packaging builds.

Docs/contracts touched: architecture, contracts.

Test expectations: Nix shell check, desktop smoke test, Rust command test, and initial CI check.

### Define UI design system tokens

Intent: define theme tokens for the dark-default and light theme UI.

Acceptance criteria:

- Theme setting is persisted through the settings contract.
- Dark theme is the first-run default.
- Night-neon palette tokens cover background, surface, text, border, primary accent, secondary accent, focus, warning, success, and danger.
- Light theme uses the same accent identity with accessible light surfaces.

Docs/contracts touched: product spec, contracts, theme ADR.

Test expectations: UI token tests or visual smoke coverage once UI exists.

### Design initial SQLite migrations

Intent: create migration-managed local storage for companies, watchlists, feed items, source records, notebook entries, transcript jobs, transcript segments, jobs, and settings.

Acceptance criteria:

- Migration runner exists.
- Initial schema represents contracts in `docs/contracts.md` and the entity list in `docs/data-model.md`.
- Migration tests cover clean database creation.
- Migration check is suitable for GitHub Actions.

Docs/contracts touched: contracts, architecture.

Test expectations: migration tests.

## In Progress

No active card.

## Review

No cards.

## Done

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
