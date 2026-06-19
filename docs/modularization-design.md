# Modularization Design

This document defines the code organization rules for Brawler after the M13 modularization pass. The broad modularization effort is complete; this document is now an operating guide for keeping future development modular by default.

Use [Project Brief](project-brief.md) for the full documentation map. Related references: [Architecture](architecture.md), [Engineering Workflow](engineering-workflow.md), [Kanban](kanban.md), and [Contracts](contracts.md).

## Goals

- Keep modules cohesive and logical.
- Make feature work easier for humans and AI agents.
- Preserve typed Tauri command behavior and public contracts during extraction.
- Keep real runtime behavior separate from tests, samples, and mocks.
- Support future providers, sources, credentials, models, collectors, renderers/exporters, storage implementations, screens, and integration adapters without turning core files into dumping grounds.
- Make extension points explicit across the application so future implementations can plug into stable internal contracts instead of requiring rewrites.

## Historical Pain Points

The M13 modularization pass was started because these files had become mixed-responsibility architecture debt:

- `src/App.tsx`: app shell, routing, data loading, screen rendering, workflow logic, formatting helpers, and mutations are mixed in one file.
- `src/App.test.tsx`: many unrelated workflows share one mock setup and one large test file.
- `src/styles.css`: app-wide styling and screen-specific styling are mixed.
- `src-tauri/src/storage.rs`: migrations, data types, settings, source state, companies, watchlists, feed, notebooks, events, transcripts, and tests are mixed in one module.
- `src-tauri/src/lib.rs`: Tauri command registration, command handlers, refresh orchestration, transcript runner orchestration, and source adapter dispatch are mixed.

## Current Status

The M13 modularization pass has addressed the original large-file debt by extracting frontend API modules, screens, screen section/row components, company workspace, inbox detail pane, events week/list views, notebook entry editor, transcript job row/detail rendering, transcript segment review, transcript note draft, shared components/hooks/formatting aliases, app-level workflow controllers, app lifecycle effects, app view-model derivations, screen tests, test helper facades, CSS modules, Rust commands, app-state boundary, providers including credential handling, scheduled job helpers, domain storage modules including registry/catalog storage, and domain-split storage tests.

The remaining larger files are intentional state roots, facades, composition points, or cohesive domain views rather than mixed command/API/storage/screen implementations. They should be split only when future feature work reveals a real new responsibility boundary.

Current notable composition points:

- `src/app/App.tsx`: small wrapper for the state root.
- `src/app/AppStateRoot.tsx`: React state and workflow composition root.
- `src-tauri/src/lib.rs`: Tauri app setup and command registration facade.
- `src-tauri/src/storage/mod.rs`: storage facade and shared SQLite state.
- Large domain modules such as `CompanyWorkspace.tsx`, `SourceAdapterRow.tsx`, `storage/sources.rs`, and `storage/transcripts.rs`: acceptable while they remain cohesive domain implementations.

New behavior should land in the matching API, screen, shared, command, storage, provider, source adapter, or job module instead of rebuilding the original monolithic files.

## Findings

- Broad extraction is useful when a file mixes multiple architectural layers; it is less useful when a file is a cohesive domain view or facade.
- `src/app/AppStateRoot.tsx` is intentionally large because it coordinates app state. Splitting it should wait for a feature-driven state-domain boundary, not a line-count target. The AI-analysis domain (per-feed-item jobs/error/in-flight maps, poll timers, start/retry commands, and the load+poll effects) was extracted to `src/app/useAiAnalysisController.ts` on exactly that basis — a cohesive, self-contained boundary. The signals and fundamentals domains were intentionally left in place for now because their state feeds `useAppViewModel` derivations (`signalsByFeedItemId`, `feedSignalCategories`, filtered company lists); extracting them cleanly needs a coordinated controller/view-model boundary, not an in-place lift, so it should be a dedicated incremental step.
- Shared frontend primitives are useful only when they preserve existing class semantics and accessibility. `Button`, `EmptyState`, and `StatusPill` are now adopted for generic controls; segmented controls, row selectors, field clear buttons, collapsible headers, suggestion rows, and anchor links remain native/domain-specific on purpose.
- Screen tests became easier to reason about after screen extraction, but the shared app workflow harness remains useful for integration-style UI flows.
- Rust command modules should stay thin. Most complexity belongs in storage, provider, source adapter, or job modules.
- Storage facades are acceptable when they keep the public boundary clear and delegate domain behavior to focused modules.
- CSS extraction worked best after screen/component extraction stabilized.

## Design Principles

- Split by domain first, then by technical layer. This is the package-by-feature half of the project's Ports and Adapters posture ([ADR 0039](adr/0039-ports-and-adapters-posture.md)): hexagonal at the external seams (sources, AI providers, interpretative capabilities, credentials, search/backup, import/export, licensing, the UI↔Rust seam), domain-sliced inside the core.
- Treat extensibility as a default module design goal. Prefer stable contracts plus adapters for plausible future implementations over hard-coded one-off paths. Do not add a port whose population is permanently one adapter (e.g. storage is intentionally SQLite-coupled, not a repository port — see the storage-port trigger in [ADR 0039](adr/0039-ports-and-adapters-posture.md)).
- Keep collection, orchestration, storage access, presentation, and export/integration concerns separate when a module is likely to grow more than one implementation.
- Keep screen modules responsible for presentation and local UI state.
- Keep API/client modules responsible for Tauri command calls and TypeScript DTOs.
- Keep Rust command modules thin: parse command input, call domain/storage/provider code, map errors.
- Keep storage modules domain-focused and SQLite-specific.
- Keep provider modules independently testable with injected clients/fetchers.
- Keep shared helpers boring and small.
- Avoid one file per tiny component; split when a file has multiple reasons to change.
- Treat modularity as a continuous maintenance rule, not a one-time milestone.
- Prefer adopting existing module boundaries during nearby feature work over doing unrelated cleanup churn.
- Do not split a cohesive file only because it is long; split when it has separate reasons to change, separate owners, or mixed layers.

## Current Frontend Structure

```text
src/
  app/
    App.tsx
    AppStateRoot.tsx
    AppShell.tsx
    appTypes.ts
    navigation.ts
    workflow/data/view-model controllers
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
    system.ts
    types.ts
  screens/
    Inbox/
      InboxScreen.tsx
      InboxDetailPane.tsx
      InboxScreen.test.tsx
      inboxTypes.ts
    Companies/
      CompaniesScreen.tsx
      CompanyWorkspace.tsx
      CompaniesScreen.test.tsx
      companyTypes.ts
    Notebooks/
      NotebooksScreen.tsx
      NotebookEntryEditor.tsx
      NotebooksScreen.test.tsx
      notebookTypes.ts
    Events/
      EventsScreen.tsx
      WeekEventsView.tsx
      EventListView.tsx
      EventsScreen.test.tsx
      eventTypes.ts
    Transcripts/
      TranscriptsScreen.tsx
      TranscriptJobRow.tsx
      TranscriptSegmentReview.tsx
      TranscriptNoteDraft.tsx
      TranscriptJobComposer.tsx
      TranscriptRuntimeStrip.tsx
      TranscriptsScreen.test.tsx
      transcriptTypes.ts
    Sources/
      SourcesScreen.tsx
      SourceAdapterRow.tsx
      SourcesScreen.test.tsx
      sourceTypes.ts
    Settings/
      SettingsScreen.tsx
      AppearanceSettings.tsx
      SourceSettings.tsx
      AiSettings.tsx
      CredentialSettings.tsx
      SettingsScreen.test.tsx
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
      date.ts
      labels.ts
    locale/
      index.ts
      locale.test.ts
    types/
      events.ts
      notebook.ts
    styles/
      shared.css
      forms.css
      tables.css
  styles/
    tokens.css
    layout.css
    controls.css
    rows.css
    screens/
      inbox.css
      companies.css
      company-workspace.css
      notebooks.css
      events.css
      transcripts.css
      sources.css
      settings.css
  test/
    appWorkflowHarness.tsx
    mockTauri.ts
    renderApp.tsx
    setup.ts
    testData.ts
```

### Frontend Ownership Rules

- `src/app/App.tsx` is the app entry wrapper.
- `src/app/AppStateRoot.tsx` owns app-level state wiring and composes controllers/view models. Keep feature-specific logic in the matching app controller, screen, API module, or shared helper.
- `src/app/AppShell.tsx` owns shell layout.
- Screen modules should not call unrelated APIs.
- API modules should be the only place where frontend code calls `invoke`.
- Shared components should be generic enough to be reused by at least two domains.
- When touching an existing screen, adopt existing shared components/hooks for the touched area when they preserve behavior and class semantics. Do not mass-convert unrelated markup during feature work.
- New reusable UI/control behavior should land in `src/shared/` first when it applies to two or more domains.
- Domain-specific components stay under their screen directory.
- Screen tests should live near the screen they verify once the screen is extracted.
- Segmented controls, row selectors, field-clear buttons, collapsible headers, suggestion rows, and anchor links may remain native/domain-specific when a shared component would obscure semantics.
- New screen behavior should get a screen-level test or a targeted app workflow test near the owning screen.

### M12 Locale And Shortcut Ownership

Locale work should follow the existing settings and shared-helper boundaries:

- Backend persistence belongs in `src-tauri/src/storage/settings.rs`, exposed through `src-tauri/src/commands/settings.rs`.
- Frontend command calls and DTO changes belong in `src/api/settings.ts` and `src/api/types.ts`.
- App-level locale state wiring belongs in `src/app/AppStateRoot.tsx`, `src/app/useAppDataController.ts`, `src/app/useSettingsController.ts`, and related app controllers only where they already own settings state.
- Locale resources and typed lookup helpers belong under `src/shared/locale/` or an equivalently focused shared module.
- Screen components should receive localized strings or a narrow locale helper; they should not import Tauri settings APIs directly.
- Source-provided text, company names, ticker symbols, URLs, attribution, transcript text, notebook titles/bodies, and fetched article/report bodies must remain source/user-provided and should not pass through app-locale translation.

Shortcut work should be separate from existing row-navigation helpers:

- `src/shared/hooks/useKeyboardListNavigation.ts` remains for local arrow-key list movement.
- A new shortcut manager/hook should live under `src/shared/hooks/` or `src/app/` depending on whether it is generic registration logic or app-level command wiring.
- App-wide shortcut registration and the discoverability shell belong in `src/app/` and Settings/Help UI.
- Screen-specific shortcut actions belong in the owning screen or screen controller.
- Shortcut tests should live near the screen for screen-owned actions, with shared hook tests for suppression/registration behavior.

## Current Rust Structure

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
    error.rs
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
    tests/
      mod.rs
      common.rs
      domain test modules
  providers/
    mod.rs
    credentials.rs
    transcripts/
      mod.rs
      gemini.rs
      test_sample.rs
      types.rs
  source_adapters/
    existing adapter modules
  jobs/
    feed_cleanup.rs
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
- New typed Tauri commands should be registered through `commands/mod.rs` and implemented in the matching command module.
- Command modules should not accumulate storage SQL, provider HTTP details, or source parsing logic.
- New storage behavior should live in the domain storage module and be exposed through the storage facade only when other domains or commands need it.
- New source/provider orchestration should go through `jobs/*` when it combines storage with adapters/providers.
- Secret handling stays under the provider credential boundary; runtime frontend code must never receive secrets.

## Styling Structure

Current structure:

```text
src/styles/
  tokens.css
  layout.css
  controls.css
  rows.css
  shell.css
  membership.css
  notebook-shared.css
  responsive.css
  utilities.css
  screens/
    inbox.css
    companies.css
    company-workspace.css
    notebooks.css
    events.css
    transcripts.css
    sources.css
    settings.css
```

CSS extraction should follow component/screen extraction. New screen-specific selectors should go under `src/styles/screens/`. New cross-screen control/layout tokens should go under the matching shared style module.

## Test Structure

Frontend structure:

```text
src/test/
  appWorkflowHarness.tsx
  testData.ts
  mockTauri.ts
  renderApp.tsx
  setup.ts
src/screens/*/*.test.tsx
```

Rust target:

- Keep unit tests inside the module when small.
- Move large shared test builders into `src-tauri/src/storage/tests/common.rs`, `src-tauri/src/test_support.rs`, or domain-specific `test_support` modules.
- External provider/source tests use injected clients/fetchers and test samples, while milestone closure uses real smoke checks where required.

## Continuous Development Checklist

Every non-trivial code change should consider modularity before implementation:

1. Identify the owning domain and layer before editing.
   - Frontend command calls: `src/api/*`.
   - App-level workflow/state: `src/app/*`.
   - Screen UI and screen-local interactions: `src/screens/<Domain>/*`.
   - Cross-screen UI/helpers/types: `src/shared/*`.
   - Tauri command handlers: `src-tauri/src/commands/*`.
   - SQLite persistence: `src-tauri/src/storage/*`.
   - Provider runtime/client behavior: `src-tauri/src/providers/*`.
   - Source fetching/parsing/normalization: `src-tauri/src/source_adapters/*`.
   - Cross-domain orchestration: `src-tauri/src/jobs/*`.

2. Prefer the existing module boundary over adding logic to a nearby large file.

3. If a touched file is gaining a second reason to change, extract a cohesive child module as part of that feature slice.

4. If a UI control/pattern already exists in `src/shared/`, adopt it for the touched area when behavior, class names, and accessibility remain equivalent.

5. If new UI/control behavior applies to two or more domains, add it to `src/shared/` before copying it between screens.

6. If a new provider/source/model/credential setting is introduced, make the provider/source/model/credential boundary explicit instead of hard-coding it into UI or command code.

7. Keep tests near the owner being changed.
   - Screen behavior: screen test.
   - App workflow spanning screens: app workflow harness or focused app test.
   - Storage behavior: storage domain tests.
   - Provider/source parsing: provider/source adapter tests with injected clients/fetchers and test samples.

8. Update docs/contracts when a public contract, ownership boundary, or feature workflow changes.

## Refactor Safety Rules

- Each extraction slice must preserve behavior and public Tauri command names.
- Each extraction slice must pass the normal local check set.
- Do not combine behavior changes with broad file movement unless the behavior change is tiny and unavoidable.
- Prefer extracting one domain at a time.
- Avoid changing database schema during pure module extraction.
- Use `git diff --stat` or RTK summaries to keep each extraction reviewable.
- Do not introduce a shared component/hook only to satisfy structure. It should either be used immediately or have a clear near-term reuse case.
- Do not weaken product requirements or contracts to make extraction easier.
- Leave domain-specific controls native when shared abstraction would hide important semantics.
- Do not split cohesive files simply to reduce line count; line count is a signal to inspect, not a standalone requirement.

## Completion Note

The broad M13 modularization effort is complete. Future work should treat this document as a standing architecture checklist, not as a backlog of remaining extraction tasks.

Further extraction is still expected during normal feature work when a module gains a new reason to change. Good future extraction triggers include:

- A cross-domain research workspace needs to aggregate feed items, notes, claims, transcripts, events, AI outputs, sources, review state, and evidence links without coupling screens directly to storage tables or unrelated app controllers.
- A new app workflow adds state that can be isolated into a controller or view-model helper.
- A screen adds a second complex panel, editor, or row/detail pattern.
- A storage domain adds enough independent behavior to justify a focused helper/test module.
- A provider/source adapter needs separate runtime, parsing, mapping, and test-sample boundaries.
- A shared UI behavior is needed by at least two domains.

Known acceptable large files after M13:

- `src/app/AppStateRoot.tsx`, because it is the state/composition root.
- Cohesive screen/domain components such as `CompanyWorkspace.tsx` and `SourceAdapterRow.tsx`.
- Storage domain modules such as `storage/sources.rs` and `storage/transcripts.rs` while they remain domain-focused.

If any of these start mixing layers or unrelated domains, split the new responsibility during the feature slice that introduces the pressure.

## M24 Large-File Responsibility Audit

M24 reviewed the large/coordinating files in light of the research-workspace roadmap. The rule remains responsibility-first: line count is a signal to inspect, not a reason to split by itself.

Required M24 extractions completed:

- `src/api/types.ts`: research/evidence DTOs moved to `src/api/researchTypes.ts` instead of expanding the generic API type bucket.
- `src/api/`: research command calls added as `src/api/research.ts`.
- `src-tauri/src/storage/mod.rs`: kept as the storage facade, with research behavior extracted to `src-tauri/src/storage/research.rs`.
- `src-tauri/src/commands/`: research commands added as `src-tauri/src/commands/research.rs`.
- `src-tauri/src/storage/tests/`: research boundary tests added as `src-tauri/src/storage/tests/research.rs`.

Deferred until feature pressure:

- `src/app/AppStateRoot.tsx`: still an app state/composition root. Do not split for M24 because no visible research UI/state was added. M25 should add a focused research controller/view-model instead of placing company timeline/review state directly in `AppStateRoot`.
- `src/test/appWorkflowHarness.tsx`: keep as the current app workflow harness. Split only when new research UI workflow tests make setup or test-data ownership harder to reason about.
- `src-tauri/src/storage/import_export.rs`: defer. M24 research-owned durable state is not exported by M20 files until a visible research workflow makes it normal user-owned data.
- `CompanyWorkspace.tsx` and `NotebooksScreen.tsx`: defer. M25/M27 can extract research-facing timeline, review, or evidence-link UI components when those visible workflows are implemented.

No action in M24:

- `src-tauri/src/lib.rs` and `src-tauri/src/commands/mod.rs`: remain registration facades. Adding the research command module did not introduce mixed runtime behavior.
- `src-tauri/src/storage/sources.rs`, `src-tauri/src/storage/transcripts.rs`, source adapter modules, and source tests: still domain-owned and outside M24 research/evidence foundation scope.
- `src-tauri/src/storage/metrics.rs`: cohesive observability domain, unrelated to research-workspace readiness.

Future trigger:

- If a future feature needs stored timeline/evidence projections, add the projection implementation behind the existing research/evidence API rather than moving aggregation into screens or broadening the storage facade.

## Research Workspace Readiness Check

Before implementing the future research-workspace direction, run a focused modularization readiness check. The check should decide whether the existing boundaries can support a shared research evidence model, timeline/read model, review workflow, research questions, claim expansion, source-grounded AI briefs, digests, reminders, and evidence links.

Areas to inspect:

- frontend app state and view-model ownership, especially whether `AppStateRoot` should compose a new research controller instead of owning research workflow state directly
- frontend API DTO ownership, especially whether research evidence/timeline/review types should live outside the generic `src/api/types.ts` file
- screen ownership between Companies, Notebooks, Events, Inbox, Sources, and a future Research/Review surface
- Rust command ownership for research evidence, review state, links, questions, reminders, and generated briefs
- storage ownership for cross-domain evidence/read models versus existing feed, notebook, event, transcript, and AI-analysis tables
- import/export, backup, retention, and migration impact for new research-workspace entities
- AI brief boundaries for evidence collection, prompt/building, provider execution, citation mapping, rendering, and persistence
- test ownership for evidence aggregation, review state, linking integrity, citation grounding, and real-browser workflow/layout coverage

The output should be a refactor-before-feature decision list:

- required before research workspace implementation
- can be folded into the first feature slice
- defer until concrete pressure appears
- no action needed because the current boundary is already sufficient

### Accepted M24 Research Boundary

ADR 0022 accepts a dedicated research/evidence boundary before user-facing research-workspace features are implemented.

Frontend ownership:

- Research-workspace aggregation belongs in a focused frontend domain/API/controller boundary, not directly in `AppStateRoot` or individual screens.
- Research/evidence DTOs should live in focused modules rather than expanding `src/api/types.ts`.
- Screens should consume research read models and render them; they should not independently call multiple unrelated domain APIs to assemble timelines.

Rust ownership:

- Research/evidence commands should be thin wrappers around a dedicated research/evidence domain boundary.
- Existing domain storage modules remain the canonical owners of feed items, notebook entries, transcript segments, events, AI analysis results, companies, watchlists, and source state.
- The research/evidence boundary owns cross-domain read models, review checkpoints, evidence links, and later AI brief/read-model orchestration.

Storage posture:

- Canonical domain tables remain the source of truth.
- Durable cross-domain concepts such as review checkpoints and typed evidence links may get their own storage surfaces.
- Full stored timeline/evidence projections are deferred until performance, review semantics, sync, or import/export requirements prove they are needed. If added later, they must stay behind the research API so frontend ownership does not change.

AI brief posture:

- AI research briefs are dedicated entities with citations and provider/model/prompt provenance, not ordinary notebook entries.
- Generation should stay split across evidence collection, prompt/context building, provider job execution, citation mapping, rendering, and persistence.
