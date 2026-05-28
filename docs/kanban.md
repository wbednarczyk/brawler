# Kanban

## Backlog

### Decide repository license

Intent: choose the open-core license posture before public release.

Acceptance criteria:

- ADR records selected license family.
- Commercial boundary is documented.
- Root license file is added when appropriate.

Docs/contracts touched: future ADR, project brief.

Test expectations: none.

### Scaffold desktop application

Intent: create the Tauri + React + TypeScript desktop shell with Rust domain modules.

Acceptance criteria:

- Tauri app starts on the development machine.
- React UI renders a basic investor inbox shell.
- UI supports dark and light theme selection with dark as the default.
- Initial visual tokens implement the night-neon blue, pink, and purple palette.
- Rust command `health` returns app status.
- Tauri events can notify the UI about job/feed updates.

Docs/contracts touched: architecture, contracts.

Test expectations: desktop smoke test and Rust command test.

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
- Initial schema represents contracts in `docs/contracts.md`.
- Migration tests cover clean database creation.

Docs/contracts touched: contracts, architecture.

Test expectations: migration tests.

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
- Rate limit and source policy are documented.

Docs/contracts touched: contracts, architecture, source-specific ADR if scraping is required.

Test expectations: adapter unit tests with fixtures.

## Ready

No cards.

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
