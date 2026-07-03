# ADR 0064: Resolved V1 UX Decisions (Consolidated Record)

Status: Accepted

This ADR consolidates a set of V1 UX decisions previously recorded under the
"Open UX Questions" heading in [docs/ui-flows.md](../ui-flows.md) — a
legacy/mislabeled heading: every entry it held was already a resolved
decision written ADR-style (Decision / Why it matters / Considered options /
V1 decision), never an open question. This ADR is their one canonical home
(per [ADR 0063](0063-claude-native-context-architecture.md)'s lean-docs
layering policy: rationale and rejected options live in an ADR; current
behavior lives in ui-flows.md). Two of the six were later elaborated by
their own dedicated ADRs ([ADR 0036](0036-report-document-storage-and-backfill.md)
for report backfill, [ADR 0052](0052-report-over-report-diff.md) for
report-over-report diff) — those cross-references are kept. One
(Company Workspace Structure) was later **superseded** by
[ADR 0057](0057-composable-views-and-curated-dashboard.md), noted inline.
Current, up-to-date behavior for each decision is stated in the relevant
journey in [docs/ui-flows.md](../ui-flows.md); this ADR is the historical
rationale record, not the behavior spec.

## Company Workspace Structure

Decision: when you open a company such as `GPW:CDR`, it uses one company
page with tabs or segmented views.

Why it matters: this decides how quickly you can move between a company's
latest feed items, notebook, claims, transcripts, and metadata.

Considered options:

- Tabs or segmented views: one company page with sections like Feed,
  Notebook, Claims, Transcripts, and Metadata. Keeps company research in one
  place; the recommended default at the time.
- Split panes: company page shows multiple areas at once, e.g. feed on the
  left and notes on the right. Powerful but can become cramped.
- Route-per-section: Feed, Notebook, Claims, and Transcripts as separate
  pages/routes. Simple technically but can make research feel scattered.

V1 decision (original): tabs or segmented views inside one company
workspace.

**Superseded by [ADR 0057](0057-composable-views-and-curated-dashboard.md):**
the click-through tabbed workspace was retired in favor of a curated,
composable company dashboard (a seeded `cockpit_layout` scoped to the
company). Current behavior is documented in ui-flows.md's "Journey: Company
Notebook Review."

## Report Backfill And Document Capture (v0.41.0)

Decision: tracking a company offers an explicit **"Backfill history"**
action, and report files are captured into the company's evidence rather
than left as external links only. See
[ADR 0036](0036-report-document-storage-and-backfill.md) for the full
design (storage, retention, dedup, throttling).

Why it matters: a freshly tracked company otherwise starts with an empty
timeline, and AI extraction/diff have no local document to work from.

Flow (at decision time):

1. The user tracks a company (or opens its workspace) and triggers
   **Backfill history**. A progress indicator shows pages fetched, items
   ingested, and documents stored, with a cancel control; diagnostics
   surface any per-item fetch errors.
2. As backfill and normal refresh ingest official filings, periodic-report
   attachments are stored as full report documents and appear linked from
   the company's Fundamentals/timeline; routine filings show as linked
   metadata only.
3. Re-running backfill or refreshing again does not create duplicate items,
   documents, or events; backfilled items keep their original publication
   dates.
4. A dividend or general-meeting filing with a stated future date surfaces
   a **proposed** calendar event; the user confirms or rejects it before it
   appears on the calendar. A date is never placed on the calendar without
   confirmation.

V1 decision: backfill is user-triggered, app-open-only, ~3-year depth,
official sources only; historical calendar entries are not backfilled.

## Report-Over-Report Diff (v0.47.0)

Decision: a stored periodic **financial statement** can be diffed
section-by-section against the previous same-type filing, from the company
workspace and on new-report arrival. See
[ADR 0052](0052-report-over-report-diff.md) for the full design.

Why it matters: an investor should see what changed since last quarter
without rereading an 80-page filing.

Flow (at decision time):

1. From the company's report documents (Fundamentals/timeline) the user
   picks a financial statement and chooses **Compare with previous**; the
   app pairs it with the prior same-type statement (SSF↔SSF, JSF↔JSF). When
   a new periodic report arrives, the diff is offered as an entry point.
2. The diff view shows aligned sections: unchanged, changed, only-in-older,
   only-in-newer. Changed sections show the textual delta with both reports
   cited (the user can open either source).
3. The diff is deterministic and local (no AI, no network); reopening the
   same pair shows the same result. While a report's text is still being
   extracted, the view shows an extraction-pending state; a scanned report
   with no text layer shows an explicit "can't diff" state.

V1 decision: structured financial statements only; the narrative
management report (MD&A) and an AI delta summary are deferred
([ADR 0052](0052-report-over-report-diff.md)); no cross-company diff.

## Claim Follow-up Periods

Decision: when you write a note like "management said X should happen
soon", the app supports both a follow-up quarter and an exact follow-up
date.

Why it matters: company promises are often tied to quarters, but sometimes
you may want an exact date reminder.

Considered options:

- Quarters only: examples `2026-Q3`, `2026-Q4`. Matches earnings/reporting
  cadence and is simple for investor workflows.
- Exact dates only: examples `2026-09-30`, `2026-12-15`. Precise but less
  natural for statements like "in the next two quarters".
- Both quarters and exact dates: store an optional quarter and optional
  exact date. More flexible; the recommended default.

V1 decision: support both, but make quarter selection the most visible
control for claim notes.

## Transcript Editability

Decision: after Gemini produces transcript segments from a YouTube
conference, transcript segments are treated as source output and are not
edited directly in v1. Notes created from transcripts are editable.

Why it matters: AI transcripts can contain mistakes. But if we edit the
transcript directly, we lose a clean record of what the provider originally
returned.

Considered options:

- Immutable transcript, editable note drafts: transcript segments are
  stored as source output and cannot be changed; you edit the note before
  saving. Preserves origin; the recommended default.
- Editable transcript plus editable notes: user can correct transcript text
  and then create notes from the corrected text. Convenient but needs audit
  history.
- Store original and corrected transcript: preserve provider output and
  allow a corrected user version. Best long-term but more complex for v1.

V1 decision: immutable transcript segments with editable note drafts.

Selection behavior did not need to be fully designed before implementation
scaffolding, but the v1 UX must support at least one way to choose source
material before creating a note. Acceptable interaction patterns included
selecting whole transcript segments, selecting text ranges inside a
segment, or accepting an AI-suggested note draft. The saved note must keep
origin to the original segment and YouTube URL even if the note text is
edited. Implemented behavior (whole-segment selection) is documented in
ui-flows.md's "Journey: YouTube Conference To Notes."

## Source Status Placement

Decision: source and adapter health is shown in a dedicated Source Status
screen.

Why it matters: you need confidence that the app is actually pulling GPW
reports and other sources, but source diagnostics should not clutter daily
reading.

Considered options:

- Full Source Status screen: a dedicated section showing adapters, last
  successful fetch, errors, rate limits, and manual refresh. Transparent
  and useful for troubleshooting.
- Settings subsection: source status lives under Settings. Keeps
  navigation smaller but hides important operational information.
- Compact toolbar status plus detailed settings: toolbar shows a small
  health indicator; detailed status lives elsewhere. Useful later, but
  still needs a detailed home.

V1 decision: full Source Status screen, with a compact indicator in the top
toolbar later.
