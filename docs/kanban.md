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

## Ready

### M19: Dedicated watchlist management

Intent: turn watchlists into a real management workflow instead of scattered company-row or company-workspace controls.

Acceptance criteria:

- Companies contains a dedicated Watchlists panel or subview below/inside the Companies section, not a separate top-level navigation item.
- The Watchlists panel owns creating, renaming if supported, deleting, and selecting watchlists.
- The Watchlists panel owns adding companies to a selected watchlist and removing companies from it.
- The Companies list and company workspace show existing watchlist memberships for scanning only.
- No watchlist create/delete/add/remove controls remain visible in the Companies list or company workspace.
- Watchlist filters remain available in Inbox, Events/Calendar, Companies, and Notebooks.
- The watchlist workflow leaves room for future premium alert configuration based on watchlists without adding alerts in M19.

Docs/contracts touched: roadmap, product spec, UI information architecture, contracts if command/read models change.

Test expectations: focused React workflow tests for watchlist create/delete and add/remove membership from the dedicated Watchlists panel, plus regression coverage that Companies shows membership labels without membership controls.

## In Progress

## Review

## Done

### M18: Implement V1 application polish

Delivered:

- Repaired Notebooks layout, scrolling, ticker formatting, and panel resizing for daily note work.
- Refined Inbox row hierarchy, detail-pane layout, scoped search placement, PDF-only attachment presentation, and AI analysis polling.
- Grouped Sources by purpose and improved source/status hierarchy without changing backend source contracts.
- Added Settings subnavigation, theme mode/accent palette settings, `night-neon`, and `midnight-horizon`.
- Added shared exchange/symbol ticker rendering and tuned ticker colors.
- Added watchlist filters across Inbox, Events/Calendar, Companies, and Notebooks while deferring durable watchlist management to M19.
- Tightened Companies, shell/sidebar, topbar, scrolling, focus, selected-row, clear-control, and architecture-copy polish across the app.
- Removed normal user-facing references to implementation details such as SQLite, storage internals, Tauri, adapters, modules, collectors, and similar plumbing outside developer/owner contexts.
- Updated docs/contracts, locale coverage, and focused workflow tests.
- Bumped app version to `0.18.0`.

ADR checkpoint: ADR 0006 covers the theme and visual-direction changes. Existing ADRs cover the source, local-first, diagnostics, logs, metrics, and licensing policies touched by M18; no new ADR was needed for this polish milestone.

Validation:

- User manually reviewed and signed off the M18 UI/UX polish.
- `rtk npm run typecheck` passed.
- `rtk npm test -- --run` passed.
- `rtk npm run build` passed.
- `rtk cargo fmt --check` passed.
- `rtk cargo clippy --all-targets -- -D warnings` passed.
- `rtk cargo test` passed.

### M17: Implement v1 friend-test license gate

Delivered:

- Added ADR 0017 for the local author/friend-test license gate, threat model, offline validation model, key separation, storage/redaction rules, and future adapter/version-limit extension points.
- Added an extensible Rust licensing module with token parsing, Ed25519 verifier registry, entitlement policy evaluation, OS keychain token storage, SQLite redacted metadata, typed Tauri commands, diagnostics, logging, and metrics integration.
- Added separate author and friend-test signing paths; present author and friend-test licenses are all-version while future version-limited licenses remain supported.
- Added the app-level license gate plus Settings license inspect, replace, and clear UI with Polish translations and no unnecessary technical reassurance copy.
- Added owner tooling and docs: `scripts/licensing/*`, `make license-author`, `make license-friend`, key-generation targets, gitignored `private/`, and `docs/license-operations.md`.
- Updated contracts, data model, architecture, product spec, UI information architecture, engineering workflow, licensing strategy, roadmap, practices, and project brief.
- Bumped app version to `0.17.0`.

ADR checkpoint: ADR 0017 records the M17 licensing posture and extension boundaries. No additional ADR was needed for closure.

Validation:

- Manual UI license gate testing passed by user.
- Manual Makefile/token-generation checks passed by user.
- `rtk cargo test licensing` passed.
- `rtk npm run typecheck` passed.
- `rtk npm test -- --run` passed.
- `rtk npm run build` passed.
- `rtk cargo fmt --check` passed.
- `rtk cargo clippy --all-targets -- -D warnings` passed.
- `rtk cargo test` passed.

### M16: Implement local metrics exposure

Delivered:

- Added a dedicated Rust metrics module with typed metric samples, kind/unit enums, privacy-safe label validation, runtime counters, and a static collector registry.
- Implemented on-demand local metrics snapshots from SQLite state plus process-lifetime runtime counters that reset on app restart.
- Kept collectors separate from presentation/export so future Prometheus, OpenTelemetry, file, or other local metrics integrations can be added as adapters over the same internal samples.
- Added Prometheus-friendly internal metric names and units without adding Prometheus, OpenTelemetry, remote export, scrape endpoints, hosted observability, or metrics settings.
- Added collectors for source state, runtime source refreshes, scheduler skips, AI jobs/runs, transcript jobs/runs, credential status/operations, diagnostics, local logs, SQLite database size/table rows, and feed cleanup runs/deletes/durations.
- Added the Developer-mode-gated `get_local_metrics_snapshot` Tauri command and frontend API wrapper.
- Added a Metrics section inside Developer Diagnostics as the first presentation adapter.
- Updated ADR 0015, architecture, contracts, product spec, UI information architecture, engineering workflow, roadmap, project practices, project brief, modularization design, AGENTS, and Kanban.
- Bumped app version to `0.16.0`.

ADR checkpoint: ADR 0015 covers the local-only observability policy and was updated for the pluggable metrics collector/adapter boundary. No new ADR was needed because M16 did not add a new exposure surface.

Validation:

- `rtk cargo test metrics` passed.
- `rtk npm typecheck` passed.
- `rtk npm test -- App.test.tsx` passed.
- `rtk cargo fmt --check` passed.
- `rtk cargo clippy --all-targets -- -D warnings` passed.
- `rtk cargo test` passed.
- `rtk npm test -- --run` passed.
- `rtk npm build` passed.

### Documentation Context Optimization

Intent: reduce the amount of documentation context future agents need to load while preserving all canonical project value.

Delivered:

- Added [Kanban Archive](kanban-archive.md) and moved completed-card history there.
- Kept active Backlog, Ready, In Progress, Review, and a Done archive pointer in [Kanban](kanban.md).
- Updated [AGENTS.md](../AGENTS.md) with task-scoped required reading.
- Reworked the [Project Brief](project-brief.md) document map into a task-oriented docs router.
- Shortened broad cross-document reference lines across the canonical docs.
- Updated architecture wording to treat [Modularization Design](modularization-design.md) as the current ownership guide, not a future extraction-order plan.

Validation:

- Confirmed active Kanban context points to [Kanban Archive](kanban-archive.md) for completed-card history.
- Confirmed [AGENTS.md](../AGENTS.md) required reading is task-scoped.
- Confirmed [Project Brief](project-brief.md) provides a task-oriented document map.
- Confirmed canonical docs route readers through the project brief with only local relevant references.
- Confirmed this docs-only optimization did not change product, contract, source-policy, security, testing, or modularization requirements.

### M15: Implement local logs framework

Delivered:

- Added local JSON Lines runtime logging through the Rust `log` facade.
- Added app data `logs` directory initialization and `brawler.log` file output.
- Added configurable log settings in SQLite: level, max files, and max file size.
- Added environment overrides: `BRAWLER_LOG_LEVEL`, `BRAWLER_LOG_MAX_FILES`, and `BRAWLER_LOG_MAX_FILE_MEGABYTES`.
- Added bounded rotation with defaults of five files and five MiB each.
- Extracted shared observability redaction for diagnostics and logs.
- Added Settings controls for local log level and rotation limits.
- Added Developer-mode Diagnostics log status, full in-app log viewer, redacted copy action, and open-logs-folder action.
- Added typed log commands and frontend API wrappers.
- Added operational log producers for startup, source refresh, AI analysis, and credential workflows.
- Updated ADR 0015, roadmap, architecture, contracts, product spec, UI information architecture, engineering workflow, and Kanban.
- Bumped app version to `0.15.0`.

Validation:

- `rtk cargo test storage::tests::settings` passed.
- `rtk cargo test storage::tests::schema` passed.
- `rtk cargo test observability` passed.
- `rtk cargo test logging` passed.
- `rtk cargo test jobs::ai_analysis` passed.
- `rtk cargo test jobs::source_refresh` passed.
- `rtk npm typecheck` passed.
- `rtk npm test -- App.test.tsx` passed.
- `rtk npm test -- --run` passed.
- `rtk cargo fmt --check` passed.
- `rtk cargo clippy --all-targets -- -D warnings` passed.
- `rtk npm build` passed.
- `rtk cargo test` passed.
- `rtk git diff --check` passed.

### Implement developer mode diagnostics framework

Intent: create a developer-only diagnostics framework that any app module can report into, starting with AI analysis but not limited to AI.

Delivered:

- Completed Developer mode and local diagnostics framework implementation.
- Added ADR 0015 for local observability and Developer mode policy.
- Added ADR 0016 for the provider-neutral AI analysis framework decision that feeds the first rich diagnostic producer.
- Added persisted Developer mode setting, environment startup activation, hidden runtime passphrase unlock, and app disable action.
- Added SQLite-backed diagnostic events with typed module/scope/stage/severity/message/metadata fields.
- Added redaction before persistence and bounded retention by latest 1,000 events or 7 days.
- Added typed diagnostics commands and frontend API wrappers.
- Added developer-only Diagnostics navigation and panel with filters, event expansion, refresh, clear, copy summary, and disable Developer mode action.
- Added AI analysis lifecycle diagnostics plus lightweight source refresh and credential diagnostics.
- Kept Diagnostics last in the left navigation when Developer mode is active.
- Manual app smoke passed before milestone closure.
- Bumped app version to `0.14.0`.

Validation:

- `rtk cargo test diagnostics` passed.
- `rtk cargo test source_adapters::bankier_company::tests` passed.
- `rtk npm test -- App.test.tsx` passed.
- `rtk npm typecheck` passed.
- `rtk npm run typecheck` passed.
- `rtk npm test -- --run` passed.
- `rtk npm run build` passed.
- `rtk cargo fmt --check` passed.
- `rtk cargo clippy --all-targets -- -D warnings` passed.
- `rtk cargo test` passed.

Completed-card history has moved to [Kanban Archive](kanban-archive.md) so the active board stays small for day-to-day agent context.
