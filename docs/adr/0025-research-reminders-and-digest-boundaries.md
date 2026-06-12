# ADR 0025: Research Reminders And Digest Boundaries

## Status

Accepted.

## Context

The Research workspace now has evidence timelines, review checkpoints, research questions, evidence links, and AI research briefs. The next step is to make the workspace tell the user what needs attention now: open claims, upcoming events, unanswered questions, changed evidence, and reviewable AI output.

This should not become a disconnected generic task manager. Brawler's reminders must stay source-grounded and local-first, and digest generation must reuse the backend research evidence boundary.

## Decision

- Add durable `research_reminders` records as research-owned state.
- A reminder is typed and linked where possible to a company, watchlist, claim, event, research question, note, or other research evidence.
- Reminder kinds begin with `claim_follow_up`, `event_review`, `question_review`, `manual_research`, and `digest_review`.
- Reminder statuses begin with `open`, `completed`, and `dismissed`.
- The backend may synchronize derived reminders from claims, events, and open research questions, then store completion/dismissal state on the reminder record.
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
