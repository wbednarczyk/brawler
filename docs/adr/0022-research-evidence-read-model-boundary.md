# ADR 0022: Research Evidence Read-Model Boundary

Status: accepted

## Context

Brawler is moving toward a research workspace that helps the user understand what changed, why it matters, what management said before, and what should be checked next. That feature family will need to combine feed items, official reports, media items, notebook entries, claims, transcript segments, events, AI outputs, watchlists, sources, future questions, reminders, briefs, and digests.

The existing architecture is modular by current product domains, but there is no dedicated cross-domain research boundary. Adding research-workspace behavior directly to existing screens or app state would couple Companies, Inbox, Notebooks, Events, Transcripts, Sources, and AI workflows together. Adding stored timeline projections before the model is proven would create migration, refresh, retention, import/export, and drift risks.

## Decision

M24 will refactor the boundaries required for research-workspace readiness before user-facing research-workspace features are implemented. The refactor is scoped to research/evidence readiness; unrelated line-count cleanup is out of scope.

Frontend research-workspace behavior will get a dedicated domain/API/controller boundary. Research DTOs will live in focused modules instead of expanding the generic `src/api/types.ts` bucket.

Rust will expose a dedicated research/evidence command and domain boundary. Commands stay thin. Canonical domain tables remain owned by their existing storage modules.

Research evidence uses a hybrid model:

- Existing domain tables remain the canonical source of truth for feed items, notebook entries, claims, transcript segments, events, AI analysis results, companies, watchlists, and sources.
- The research boundary exposes typed evidence and timeline read models assembled from canonical domains.
- Durable cross-domain concepts that need persistence, such as review checkpoints and typed evidence links, get their own storage surfaces.
- Full stored evidence or timeline projections are deferred until performance, review semantics, or synchronization requirements prove they are needed. The research API must leave room to introduce stored projections later without changing UI ownership.

Timeline aggregation is backend-owned through read models first. React should not assemble timelines by calling many unrelated domain APIs and sorting them itself.

Review state is layered:

- Company and watchlist review checkpoints track "last reviewed" and support changed-since-review behavior.
- Existing item-level state, such as feed read/saved, remains in its owning domain.
- Future evidence-level review state may be added where specific workflows require it.

Evidence linking uses typed links alongside existing notebook origin references. Existing origins keep source-to-note provenance. Typed links cover broader relationships such as source-to-claim, event-to-claim, question-to-evidence, AI-brief citations, and digest citations.

Research questions are durable research-owned entities stored outside notebooks. The first visible implementation uses company-scoped questions with `open`, `answered`, and `closed` status, and links questions to evidence through typed evidence links. The storage and command boundary keeps `watchlist` as a future scope type, but normal UI creation remains company-scoped until a watchlist-question workflow is explicitly designed.

AI research briefs are dedicated entities with provider/model/prompt provenance, citations, rendered content, and regeneration semantics. They are not stored as ordinary notebook entries, though a later workflow may let the user create a note from a brief or selected excerpt.

AI brief generation must remain pluggable across:

- evidence collector
- prompt/context builder
- provider job
- citation mapper
- renderer
- persistence surface

## Consequences

M24 may add or extract code to create the research/evidence boundary before visible feature work begins.

The first research implementation should prefer backend read models over stored projections. This avoids early schema lock-in while still keeping the frontend and future TUI/mobile clients decoupled from unrelated storage tables.

Import/export, backup, retention, and migrations must treat research-owned durable entities explicitly. Stored projections, if added later, should either be rebuildable or have a clearly documented export/restore policy.

Research questions and typed evidence links are durable user-owned research data and must be included in research import/export. Broken or dangling imported evidence links should be skipped with preview warnings rather than blocking import of otherwise valid companies, watchlists, notebooks, or questions.

Future research-workspace milestones should build on the research/evidence boundary instead of adding cross-domain aggregation to existing screens.

This ADR does not approve cloud sync, hosted services, telemetry, portfolio tracking, trading signals, or broad cleanup unrelated to research-workspace readiness.
