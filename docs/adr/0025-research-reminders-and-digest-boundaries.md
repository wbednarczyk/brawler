# ADR 0025: Research Reminders And Digest Boundaries

Status: Accepted.

## Context

The Research workspace now has evidence timelines, review checkpoints, research questions, evidence links, and AI research briefs. The next step is to make the workspace tell the user what needs attention now: open claims, upcoming events, unanswered questions, changed evidence, and reviewable AI output.

This should not become a disconnected generic task manager. Brawler's reminders must stay source-grounded and local-first, and digest generation must reuse the backend research evidence boundary.

## Decision

- Add durable `research_reminders` records as research-owned state.
- A reminder is typed and linked where possible to a company, watchlist, claim, event, research question, note, or other research evidence.
- Reminder kinds begin with `claim_follow_up`, `event_review`, `question_review`, `manual_research`, and `digest_review` (`signal_review` joined in ADR 0034; event/signal derivation is retired — see the amendment below).
- Reminder statuses begin with `open`, `completed`, and `dismissed`.
- The backend may synchronize derived reminders from claims, events, and open research questions, then store completion/dismissal state on the reminder record. *(Events: retired by the 2026-09-10 amendment below.)*
- Reminder completion does not mark the company or watchlist reviewed by default. Review cascade remains an explicit separate action.
- Add separate AI research digest job, digest, and citation records instead of overloading AI research briefs.
- Digest generation is explicit and on-demand in this milestone. Automatic scheduling is deferred behind the same storage/job boundaries.
- Digest collection is backend-owned and uses reminders plus the existing research evidence read model. React displays returned read models and does not assemble digest inputs.
- AI providers expose a digest-specific generation boundary. The first implementation may share the structured cited output schema with briefs, but it must use digest-specific prompt/versioning and must not call the brief generator as a hidden substitute.
- Digest citations must point back to typed evidence references and must not duplicate full source bodies.
- Import/export includes reminders and stored digest snapshots as owner research data without secrets.

## Consequences

- Research stays the owning domain for cross-domain review pressure, reminder state, and digest snapshots.
- Existing canonical domains remain canonical: feed items, notebook entries, events, transcripts, AI analysis, and questions are not copied into reminder rows beyond link metadata and display text.
- Future reminder sources, scheduled digest generation, desktop notifications, or premium alert features can plug into reminder/digest boundaries instead of changing the Research screen contract from scratch.
- The first digest implementation can reuse the provider-neutral AI analysis provider configuration while keeping digest persistence separate from briefs.

## Amendment (2026-09-10, #465) — events and signals no longer derive reminders

Owner decision 2026-09-07 (architecture audit, finding U2). Evidence from the live database: ~830 open reminders, every one auto-generated — 455 `event_review` (one per company event, synchronized on each list call) and 378 `signal_review` (the ADR 0034 §6 hook) — with 0 completed or dismissed since 2026-06-12; 331 of the 378 signal reminders pointed at signals that no longer existed. The feed, the calendar and Today already carry the same events and signals; the reminders duplicated them as work nobody performed, and the daily scan must take seconds.

- The backend derives reminders from claims and open research questions only. Company events and typed signals never create reminders; the ADR 0034 §6 hook is removed.
- Migration `0155` dismissed the automatic rows (dated `dismissed_at`), matched by their deterministic signatures — event ids `reminder_event_*`; signal rows with `source_type = company_signal` and the classifier body. Deliberately created reminders of either kind keep their state; import restores an export's saved state unchanged (a deliberate restoration, never rewritten).
- `event_review` and `signal_review` stay valid kinds for deliberate (manual/MCP) creation.
- The Research review queue shows open reminders by default; completed/dismissed rows sit behind a **History** segment (the Today Active | Archive pattern), where Reopen lives. The queue count and the screen's primary "Mark as reviewed" follow the open count.
- Guard: storage tests assert that a new event, a confirmed high-signal classification and an MCP-classified filing create no reminder (all statuses), and the migration test re-executes the `0155` SQL to prove idempotence.

Rejected: deleting the rows (loses the reminder's own text/lifecycle against the owner's "dismissed, dated"; would not fix the mixed-status clutter that future completed manual reminders cause), blanket dismissal by kind (would close a deliberately created reminder), removing the kinds (strands stored rows, breaks export/import).

