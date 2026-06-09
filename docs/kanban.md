# Kanban

Active work only. Completed-card history lives in [Kanban Archive](kanban-archive.md) to keep day-to-day agent context small.

## Backlog

### M25: Company evidence timeline and review checkpoints

Intent: implement the first visible research-workspace slice using the M24 research/evidence boundary.

Acceptance criteria:

- A company-scoped evidence timeline combines feed items, notes, claims, events, transcripts, and AI analysis through backend research read models.
- Company review checkpoints drive "changed since review" state.
- Existing source items, notes, claims, events, transcripts, and AI analysis remain canonical in their owning domains.
- No stored timeline projection is added unless performance evidence requires it.

Docs/contracts touched: product spec, UI information architecture, contracts, data model.

Test expectations: research read-model tests, company timeline UI workflow tests, and browser layout smoke update if a new screen/panel is added.

### M26: Watchlist review mode

Intent: guide review across all companies in a watchlist using the same evidence/read-model boundary.

Acceptance criteria:

- Watchlist review mode uses watchlist-scoped evidence and review checkpoints.
- Review flow groups or sequences companies without hiding unread items, upcoming events, open claims, or changed-since-review evidence.
- The workflow does not add alert placeholders.

Docs/contracts touched: product spec, UI information architecture, contracts.

Test expectations: watchlist review-state tests and UI workflow tests.

### M27: Research questions and evidence links

Intent: add user-visible research questions/threads and typed evidence-link workflows.

Acceptance criteria:

- Research questions can be linked to source items, notes, claims, events, transcripts, and AI outputs.
- Typed evidence links support question-to-evidence, claim-to-evidence, event-to-claim, and related-item workflows.
- Existing notebook origins remain provenance records and are not replaced.

Docs/contracts touched: product spec, contracts, data model, UI flows.

Test expectations: storage link validation tests, question workflow tests, and import/export policy tests if questions become exportable user data.

### M28: AI research briefs

Intent: generate source-grounded company/watchlist research briefs after evidence collection and citation mapping are stable.

Acceptance criteria:

- Brief generation uses collector, prompt/context builder, provider job, citation mapper, renderer, and persistence boundaries.
- Briefs persist as dedicated entities with provider/model/prompt provenance and citations.
- Briefs do not produce buy/sell/hold recommendations.
- Creating a notebook note from a brief remains an explicit user action.

Docs/contracts touched: AI analysis framework, product spec, contracts, data model.

Test expectations: collector tests, provider-job tests with mocked provider, citation-grounding tests, and UI workflow tests.

### M29: Event-aware reminders and research digest

Intent: add reminders and digest generation once company/watchlist review semantics are proven.

Acceptance criteria:

- Reminders reuse claims, events, questions, and evidence links instead of creating a separate disconnected task system.
- Personal digest generation uses the research evidence boundary and cites source evidence.
- Stored projections are added only if live read-model aggregation proves too slow or semantically insufficient.

Docs/contracts touched: product spec, contracts, data model, AI analysis framework.

Test expectations: reminder storage/read-model tests, digest citation tests, and import/export policy tests.

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
