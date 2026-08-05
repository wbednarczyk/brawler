# Modularization Design

This document defines Brawler's code organization rules — the operating guide for keeping development modular by default.

Doc map: [CLAUDE.md](../CLAUDE.md) Required Reading. Related references: [Architecture](architecture.md), [Engineering Workflow](engineering-workflow.md), and [Contracts](contracts.md).

**Do not mirror the code tree in this document.** For the live structure use `repoctx overview`, `repoctx outline <file>`, and `repoctx modules` — a prose copy of the tree rots. This doc carries only the durable part: ownership rules, principles, and checklists.

## Goals

- Keep modules cohesive and logical.
- Make feature work easier for humans and AI agents.
- Preserve typed Tauri command behavior and public contracts during extraction.
- Keep real runtime behavior separate from tests, samples, and mocks.
- Support future providers, sources, credentials, models, collectors, renderers/exporters, storage implementations, screens, and integration adapters without turning core files into dumping grounds.
- Make extension points explicit across the application so future implementations can plug into stable internal contracts instead of requiring rewrites.

## Historical Pain Points

Moved to [Kanban Archive](kanban-archive.md#archived-investigation-and-study-notes-moved-2026-07-02) (the mixed-responsibility files that motivated the original modularization pass, all since resolved).

## Current Status

Frontend and Rust code are organized by domain; the remaining larger files are intentional state roots, facades, composition points, or cohesive domain views rather than mixed command/API/storage/screen implementations. They should be split only when future feature work reveals a real new responsibility boundary. New behavior lands in the matching API, screen, shared, command, storage, provider, source adapter, or job module instead of rebuilding the original monolithic files.

Notable composition points: `src/app/App.tsx` (entry wrapper), `src/app/AppStateRoot.tsx` (React state/workflow composition root), `src-tauri/src/lib.rs` (Tauri setup + command registration facade), `src-tauri/src/storage/mod.rs` (storage facade and shared SQLite state).

## Findings

Moved to [Kanban Archive](kanban-archive.md#archived-investigation-and-study-notes-moved-2026-07-02). The durable rules distilled from these findings are retained live below and in § Frontend/Rust Ownership Rules (e.g. `AppStateRoot.tsx` splits on a feature-driven state-domain boundary, not a line-count target; shared primitives adopt only where they preserve class semantics/accessibility; command modules stay thin).

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

## Frontend Ownership Rules

Live layout: `repoctx outline src/app/AppStateRoot.tsx`, `repoctx modules` (domains: `src/api`, `src/app`, `src/screens/<Domain>`, `src/shared`, `src/ui`, `src/styles`, `src/test`).

### Frontend layer contract (ESLint-enforced)

Import edges between the frontend layers (issue #50; enforced by the
`no-restricted-imports` zone blocks in `eslint.config.js` — a violation
reddens `make check-frontend-static`):

| Layer | May import | Must never import |
| --- | --- | --- |
| `src/api` | its own modules, generated DTOs, the Tauri API | `app/`, `screens/`, `shared/`, `ui/` |
| `src/ui` | sibling primitives, `shared/locale`, `shared/format` (sanctioned display leaves) | `app/`, `screens/`, `api/`, any other `shared/` subtree |
| `src/shared` | `api/`, `ui/`, other `shared/` modules | `app/`, `screens/` (the composition roots — pass data/handlers via props or a composer-provided context) |
| `src/app`, `src/screens` | anything below | — |

Workspaces and tsconfig path aliases stay rejected (decision 2026-06-22,
issue #50): one package, one build, one consumer — the gate encodes the
boundaries without new tooling. Widening an edge (e.g. a new sanctioned
`shared` leaf for `ui`) is a deliberate edit to both this table and the
ESLint block, never an inline disable.

- `src/app/App.tsx` is the app entry wrapper.
- `src/app/AppStateRoot.tsx` owns app-level state wiring and composes controllers/view models. Keep feature-specific logic in the matching app controller, screen, API module, or shared helper. It is being **decomposed into feature-scoped React contexts** (Architecture v2 / [ADR 0050](adr/0050-architecture-v2-domain-stores-source-pipeline-durable-jobs.md)) so a screen subscribes only to the state it uses instead of receiving a mega view-model by prop-drilling. **Realized so far:** `src/app/state/SettingsContext.tsx` — the settings-domain context (`SettingsProvider` + selector hooks `useDeveloperMode`, `useAiFallbackEnabled`, `useAiAnalysisProviderConfigured`). It is now the single source for **every settings-derived UI flag** that was previously prop-drilled from `AppStateRoot`: developer mode (`AppShell`/`SourcesScreen`/`DiagnosticsScreen`), the ESPI AI fallback (`InboxScreen`), and whether an analysis provider is configured (`InboxDetailPane`/`CompanyWorkspace`) — `AppStateRoot` no longer computes or threads any of them. Shared leaf components (`FeedAiAnalysisPanel`, `FeedKpiExtractionPanel`) stay prop-driven (context-agnostic); the screen/pane that composes them reads the context. **Done:** every top-level screen now reads its view-model from a context instead of receiving a prop bundle from `AppStateRoot` — `SourcesContext` plus a `createScreenContext` factory backing `screenViewModels.tsx` (Inbox, Companies, Watchlists, Research, Notebooks, ReportSeason, Events, Transcripts, Settings). `AppStateRoot` assembles each view-model once and wraps the screen in its `Provider`; the screens take zero props. The screen `*ScreenProps` types are retained as the context value shapes. Add new settings selectors to `SettingsContext` rather than re-drilling a flag, and a new screen gets a `createScreenContext` entry rather than a prop bundle.
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

### Locale And Shortcut Ownership

Locale work follows the existing settings and shared-helper boundaries:

- Backend persistence belongs in `src-tauri/src/storage/settings.rs`, exposed through `src-tauri/src/commands/settings.rs`.
- Frontend command calls and DTO changes belong in `src/api/settings.ts` and `src/api/types.ts`.
- App-level locale state wiring belongs in the app controllers that already own settings state.
- Locale resources and typed lookup helpers belong under `src/shared/locale/` or an equivalently focused shared module.
- Screen components should receive localized strings or a narrow locale helper; they should not import Tauri settings APIs directly.
- Source-provided text, company names, ticker symbols, URLs, attribution, transcript text, notebook titles/bodies, and fetched article/report bodies must remain source/user-provided and should not pass through app-locale translation.

Shortcut work stays separate from row-navigation helpers: `src/shared/hooks/useKeyboardListNavigation.ts` remains for local arrow-key list movement; app-wide shortcut registration and the discoverability shell belong in `src/app/` and Settings/Help UI; screen-specific shortcut actions belong in the owning screen/controller, with tests near the owner.

## Rust Ownership Rules

Live layout: `repoctx modules` (domains: `src-tauri/src/{commands,storage,providers,source_adapters,jobs}`).

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
- The single `AppState` storage facade (grown to ~207 methods) is being split into **concrete domain-grouped facade structs** (`WatchlistStore`, `FeedStore`, `CompanyStore`, `ResearchStore`, …), each owning a [`Database`](../src-tauri/src/storage/database.rs) connection-source handle and exposing only its domain; commands depend on the store for their domain, and `AppState` is the thin composition root that wires them. These are concrete SQLite-coupled structs (a structural split), **not** a repository port — the storage non-port stance of [ADR 0039](adr/0039-ports-and-adapters-posture.md) stands. Realized under [ADR 0050](adr/0050-architecture-v2-domain-stores-source-pipeline-durable-jobs.md) (Architecture v2). **Done:** the connection source (the `Db`/`DbGuard`/`checkout` primitive) is extracted from `AppState` into the cheap-clone `Database` handle in `storage/database.rs`, and **every storage domain now has a domain store** (`CompanyStore`, `WatchlistStore`, `FeedStore`, `JobQueueStore`, `ResearchStore`, `FinancialsStore`, `TranscriptStore`, `SignalStore`, `QualityFrameworkStore`, `SourcesStore`, `EventStore`, `SettingsStore`, … — one per `storage/*` module) owning a `Database` and exposing only its domain's operations, reached via the matching `AppState::<domain>()` accessor. `AppState` is now a **composition root**: it holds the `Database` + cross-cutting infra (pool/seed, `checkout`, db status, backup, metrics, backfill progress) and otherwise only constructs the stores; its former per-domain methods are thin one-line delegations to the stores (kept so existing call sites stay green — behavior-preserving). **New storage methods go on the relevant domain store, never as fresh methods on `AppState`.** Call sites may move from `state.foo()` to `state.<domain>().foo()` incrementally with zero behavior change.
- New source/provider orchestration should go through `jobs/*` when it combines storage with adapters/providers.
- Secret handling stays under the provider credential boundary; runtime frontend code must never receive secrets.

## Styling Structure

CSS extraction follows component/screen extraction: new screen-specific selectors go under `src/styles/screens/`, new cross-screen control/layout tokens go in the matching shared style module (`tokens.css`, `layout.css`, `controls.css`, `rows.css`, …). Live inventory: `rtk ls src/styles`.

## Test Structure

- Screen tests live near the screen they verify (`src/screens/*/*.test.tsx`); the shared workflow harness and mock runtime live under `src/test/`.
- Rust: keep unit tests inside the module when small; move large shared test builders into `src-tauri/src/storage/tests/common.rs`, `src-tauri/src/test_support.rs`, or domain-specific `test_support` modules.
- External provider/source tests use injected clients/fetchers and test samples; milestone closure uses real smoke checks where required.

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

## Future Extraction Triggers

This document is a standing architecture checklist, not a backlog of remaining extraction tasks (completion narrative: [Kanban Archive](kanban-archive.md#archived-investigation-and-study-notes-moved-2026-07-02)). Further extraction is expected during normal feature work when a module gains a new reason to change. Good future extraction triggers:

- A cross-domain research workspace needs to aggregate feed items, notes, claims, transcripts, events, AI outputs, sources, review state, and evidence links without coupling screens directly to storage tables or unrelated app controllers.
- A new app workflow adds state that can be isolated into a controller or view-model helper.
- A screen adds a second complex panel, editor, or row/detail pattern.
- A storage domain adds enough independent behavior to justify a focused helper/test module.
- A provider/source adapter needs separate runtime, parsing, mapping, and test-sample boundaries.
- A shared UI behavior is needed by at least two domains.
- A future feature needs stored timeline/evidence projections: add the projection behind the existing research/evidence API rather than moving aggregation into screens or broadening the storage facade.

Known acceptable large files:

- `src/app/AppStateRoot.tsx`, because it is the state/composition root.
- Cohesive screen/domain components such as `CompanyWorkspace.tsx` and `SourceAdapterRow.tsx`.
- Storage domain modules such as `storage/sources.rs` and `storage/transcripts.rs` while they remain domain-focused.

If any of these start mixing layers or unrelated domains, split the new responsibility during the feature slice that introduces the pressure.

M24's large-file audit and pre-implementation research-workspace readiness check (methodology + area-by-area disposition) are chronicle: [Kanban Archive](kanban-archive.md#archived-investigation-and-study-notes-moved-2026-07-02). The accepted boundary they produced is live below.

## Research/Evidence Boundary Ownership

[ADR 0022](adr/0022-research-evidence-read-model-boundary.md) accepts a dedicated research/evidence boundary for research-workspace features.

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
