# Kanban

Active work only. Completed-card history lives in [Kanban Archive](kanban-archive.md) to keep day-to-day agent context small.

## Backlog

### M23: Browser UI regression testing assessment

Intent: assess and plan whether Playwright, or an equivalent real-browser UI testing path, should be added to catch layout and visual regressions that Vitest/jsdom cannot reliably detect.

Acceptance criteria:

- Current Vitest/jsdom workflow tests, CSS contract tests, manual smoke checks, and real-browser automation coverage are compared.
- Test target is decided: Vite preview app only, Tauri desktop runtime, or both.
- Execution path is decided: default local/CI check, opt-in local smoke check, or release-gate check.
- Evidence model is decided: DOM assertions only, screenshots, visual snapshots, or mixed.
- Runtime ownership is decided for WSL/Nix, native Windows, or a documented split.
- Artifact handling is decided for screenshots, videos, and traces.
- First Playwright implementation slice is either approved and split into tasks, or explicitly rejected/deferred with rationale.

Docs/contracts touched: roadmap, engineering workflow, project practices, possibly ADR if Playwright is accepted as a durable testing boundary.

Test expectations: no Playwright dependency until the assessment approves scope and cost; existing frontend tests continue to pass.

### M24: Research workspace architecture

Intent: plan the future research-workspace feature family before implementation so company timelines, review mode, claim tracking, research questions, AI briefs, digests, source trust signals, reminders, and evidence links share one coherent model.

Acceptance criteria:

- Candidate capabilities are grouped into cohesive implementation milestones instead of ten isolated screens.
- A shared research evidence model is designed for feed items, reports, media items, notes, claims, transcripts, events, questions, reminders, AI briefs, and digests.
- Timeline/read-model ownership is decided so UI screens can aggregate evidence without coupling directly to unrelated storage tables.
- Review workflow semantics are defined, including "last reviewed", "changed since review", company review, and watchlist review.
- Evidence-linking semantics are defined for source-to-note, source-to-claim, event-to-claim, question-to-evidence, AI-brief citations, and digest citations.
- AI brief generation boundaries are planned as pluggable evidence collector, prompt/builder, provider, renderer, and storage surfaces.
- Source quality/trust signal vocabulary is defined without exposing implementation language in normal UI.
- Storage, import/export, backup, retention, and migration impacts are assessed.
- Architectural decisions are captured in an ADR if the model becomes a durable product boundary.
- Resulting implementation milestones are added to roadmap/kanban only after the architecture is clear.

Docs/contracts touched: product spec, roadmap, contracts, data model, UI information architecture, modularization design, AI analysis framework, possibly ADR.

Test expectations: no feature implementation in this milestone; future tests should cover evidence aggregation, review-state updates, linking integrity, AI citation grounding, and UI workflow regressions.

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

### Windows taskbar unread indicator

Intent: show a small Windows taskbar indicator when the Inbox has unread feed items, starting with a dot-style attention marker rather than a numeric badge.

Acceptance criteria:

- Unread Inbox count is mapped to a simple taskbar attention state: visible indicator when unread count is greater than zero, cleared when unread count is zero.
- Implementation is behind a desktop taskbar indicator boundary, not hard-coded into Inbox UI code.
- Windows gets the real adapter; non-Windows and unsupported runtimes get a no-op adapter.
- The first implementation uses a small dot/overlay indicator. Numeric badge behavior is deferred unless Windows support and UX are confirmed.
- Native Windows packaged-app smoke testing verifies the indicator appears and clears.
- Failure to update the taskbar indicator must not block normal Inbox use.

Docs/contracts touched: product spec, architecture or ADR if native Windows APIs are needed, engineering workflow smoke checklist.

Test expectations: frontend unread-state propagation test, Rust boundary/no-op tests where practical, and manual Windows smoke validation.

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

## In Progress

## Review

## Done

Completed-card history has moved to [Kanban Archive](kanban-archive.md) so the active board stays small for day-to-day agent context.
