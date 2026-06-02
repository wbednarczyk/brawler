# Modularization Design

This document defines the target code organization for Brawler. It is a design guide for incremental extraction, not a mandate for a risky all-at-once rewrite.

See also [Architecture](architecture.md), [Project Practices](project-practices.md), [Kanban](kanban.md), and [Contracts](contracts.md).

## Goals

- Keep modules cohesive and logical.
- Make feature work easier for humans and AI agents.
- Preserve typed Tauri command behavior and public contracts during extraction.
- Keep real runtime behavior separate from tests, samples, and mocks.
- Support future providers, sources, credentials, models, and screens without turning core files into dumping grounds.

## Current Pain Points

Current large files are already architecture debt:

- `src/App.tsx`: app shell, routing, data loading, screen rendering, workflow logic, formatting helpers, and mutations are mixed in one file.
- `src/App.test.tsx`: many unrelated workflows share one mock setup and one large test file.
- `src/styles.css`: app-wide styling and screen-specific styling are mixed.
- `src-tauri/src/storage.rs`: migrations, data types, settings, source state, companies, watchlists, feed, notebooks, events, transcripts, and tests are mixed in one module.
- `src-tauri/src/lib.rs`: Tauri command registration, command handlers, refresh orchestration, transcript runner orchestration, and source adapter dispatch are mixed.

## Design Principles

- Split by domain first, then by technical layer.
- Keep screen modules responsible for presentation and local UI state.
- Keep API/client modules responsible for Tauri command calls and TypeScript DTOs.
- Keep Rust command modules thin: parse command input, call domain/storage/provider code, map errors.
- Keep storage modules domain-focused and SQLite-specific.
- Keep provider modules independently testable with injected clients/fetchers.
- Keep shared helpers boring and small.
- Avoid one file per tiny component; split when a file has multiple reasons to change.

## Target Frontend Structure

```text
src/
  app/
    App.tsx
    navigation.ts
    types.ts
  api/
    tauri.ts
    companies.ts
    watchlists.ts
    feed.ts
    notebooks.ts
    events.ts
    sources.ts
    transcripts.ts
    settings.ts
    credentials.ts
  screens/
    Inbox/
      InboxScreen.tsx
      InboxDetailPane.tsx
      inboxTypes.ts
    Companies/
      CompaniesScreen.tsx
      CompanyWorkspace.tsx
      companyTypes.ts
    Notebooks/
      NotebooksScreen.tsx
      NotebookEntryEditor.tsx
      notebookTypes.ts
    Events/
      EventsScreen.tsx
      WeekEventsView.tsx
      EventListView.tsx
      eventTypes.ts
    Transcripts/
      TranscriptsScreen.tsx
      TranscriptJobRow.tsx
      TranscriptSegmentReview.tsx
      TranscriptNoteDraft.tsx
      transcriptTypes.ts
    Sources/
      SourcesScreen.tsx
      SourceAdapterRow.tsx
      sourceTypes.ts
    Settings/
      SettingsScreen.tsx
      AppearanceSettings.tsx
      SourceSettings.tsx
      AiSettings.tsx
      CredentialSettings.tsx
      settingsTypes.ts
  shared/
    components/
      Button.tsx
      EmptyState.tsx
      ExpandableRow.tsx
      InlineConfirm.tsx
      StatusPill.tsx
    hooks/
      useAsyncAction.ts
      useKeyboardListNavigation.ts
    formatting/
      dates.ts
      enums.ts
      timestamps.ts
    styles/
      shared.css
      forms.css
      tables.css
```

### Frontend Ownership Rules

- `src/app/App.tsx` should become the shell only: navigation, active section, topbar, global status, and screen composition.
- Screen modules should not call unrelated APIs.
- API modules should be the only place where frontend code calls `invoke`.
- Shared components should be generic enough to be reused by at least two domains.
- Domain-specific components stay under their screen directory.
- Screen tests should live near the screen they verify once the screen is extracted.

## Target Rust Structure

```text
src-tauri/src/
  lib.rs
  app_state.rs
  commands/
    mod.rs
    health.rs
    companies.rs
    watchlists.rs
    feed.rs
    notebooks.rs
    events.rs
    sources.rs
    transcripts.rs
    settings.rs
    credentials.rs
  storage/
    mod.rs
    migrations.rs
    settings.rs
    companies.rs
    watchlists.rs
    feed.rs
    notebooks.rs
    events.rs
    sources.rs
    transcripts.rs
    registry.rs
    types.rs
  providers/
    mod.rs
    credentials.rs
    transcripts/
      mod.rs
      gemini.rs
      test_sample.rs
  source_adapters/
    existing adapter modules
  jobs/
    scheduler.rs
    source_refresh.rs
    transcript_runner.rs
```

### Rust Ownership Rules

- `lib.rs` should mostly build the Tauri app, register plugins, manage state, and register command modules.
- `commands/*` should be thin wrappers around storage/domain/provider functions.
- `storage/*` owns SQLite schema interaction and row mapping.
- `providers/*` owns external AI/provider HTTP behavior and provider-specific parsing.
- `source_adapters/*` owns source fetching/parsing/normalization.
- `jobs/*` owns orchestration that combines storage, adapters, providers, and scheduler behavior.
- Tests should live close to the module being tested when possible.

## Styling Structure

Short term:

- Keep `src/styles.css` while extracting screens.
- Add comments or sections only where they help navigation.

Target:

```text
src/styles/
  tokens.css
  layout.css
  controls.css
  rows.css
  forms.css
  screens/
    inbox.css
    companies.css
    notebooks.css
    events.css
    transcripts.css
    sources.css
    settings.css
```

CSS extraction should follow component/screen extraction. Do not split CSS ahead of the UI module split.

## Test Structure

Frontend target:

```text
src/test/
  testData.ts
  mockTauri.ts
  renderApp.tsx
src/screens/Transcripts/TranscriptsScreen.test.tsx
src/screens/Settings/SettingsScreen.test.tsx
```

Rust target:

- Keep unit tests inside the module when small.
- Move large shared test builders into `src-tauri/src/test_support.rs` or domain-specific `test_support` modules.
- External provider/source tests use injected clients/fetchers and test samples, while milestone closure uses real smoke checks where required.

## Extraction Order

1. Extract frontend API modules.
   - Move `invoke` wrappers and DTO types out of `App.tsx`.
   - Keep UI unchanged.

2. Extract `SettingsScreen`.
   - Smallest useful screen extraction after credential/model work.
   - Moves credential/model settings out of the app shell.

3. Extract `TranscriptsScreen`.
   - Natural continuation of M10.
   - Moves transcript job form, job rows, segment review, company linking, and note draft behavior out of `App.tsx`.

4. Extract Rust transcript provider modules.
   - Move Gemini/test-sample transcript providers under `providers/transcripts/`.
   - Keep command behavior unchanged.

5. Extract Rust command modules.
   - Start with `commands/transcripts.rs`, `commands/settings.rs`, and `commands/credentials.rs`.

6. Extract Rust storage modules.
   - Start with `storage/settings.rs` and `storage/transcripts.rs`.
   - Leave shared connection/state in `storage/mod.rs`.

7. Split frontend tests by screen.
   - Only after screen extraction stabilizes.

8. Split CSS by screen and shared controls.
   - Only after component/screen boundaries are stable.

## Refactor Safety Rules

- Each extraction slice must preserve behavior and public Tauri command names.
- Each extraction slice must pass the normal local check set.
- Do not combine behavior changes with broad file movement unless the behavior change is tiny and unavoidable.
- Prefer extracting one domain at a time.
- Avoid changing database schema during pure module extraction.
- Use `git diff --stat` or RTK summaries to keep each extraction reviewable.

## Near-Term Recommendation

Do not interrupt M10 live Gemini validation with a broad refactor. The next practical extraction after M10.12 is either:

- `SettingsScreen`, because credential/model settings just expanded, or
- `TranscriptsScreen`, because M10 made it a real product workflow.

If M10.12 needs only a small smoke path, finish M10 first. If M10.12 requires more transcript UI changes, extract `TranscriptsScreen` before adding more transcript-specific UI logic to `App.tsx`.
