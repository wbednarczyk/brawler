# UI Flows

This document plans Brawler from the user experience inward. It defines the first workflows the app must make feel natural before detailed database schema or implementation scaffolding hardens.

See also [Product Spec](product-spec.md), [UI Information Architecture](ui-information-architecture.md), [Architecture](architecture.md), [Contracts](contracts.md), and [Kanban](kanban.md).

## UX North Star

Brawler should feel like a personal investor research desk:

- fast to scan each morning or evening
- dense enough for repeated professional use
- calm enough to avoid dashboard noise
- source-first and provenance-aware
- easy to turn a report, article, or transcript excerpt into a durable note

The default screen is the investor inbox. The second most important surface is the company notebook.

## Primary Navigation

V1 navigation should use persistent desktop app regions:

- left sidebar for watchlists, companies, and primary sections
- central list or workspace for the current task
- right detail pane when reviewing a feed item, company, note, or transcript segment
- top toolbar for search, refresh, source status, and settings access

Primary sections:

- Inbox
- Companies
- Notebooks
- Transcripts
- Sources
- Settings

## Journey: First Run

Intent: get from empty app to first useful watchlist quickly.

Flow:

1. User opens the app.
2. App starts in dark theme with empty local data.
3. User sees an empty Inbox state with actions to add companies or import a watchlist.
4. User adds a company by exchange-qualified ticker, for example `GPW:CDR`.
5. User creates or selects a watchlist.
6. App confirms the company is tracked and offers manual refresh.

Acceptance criteria:

- First run does not require cloud login.
- Theme defaults to dark.
- User can add at least one company without understanding internal IDs.
- Empty states are useful but not marketing-like.

## Journey: Daily Inbox Review

Intent: review new company-specific reports and news with minimal friction.

Flow:

1. User opens Inbox.
2. Feed shows newest items first.
3. User filters by watchlist, company, item type, unread, saved, and significance when available.
4. User opens an item in the detail pane.
5. Detail pane shows title, source, publication time, matched companies, source URL, original text or excerpt, and AI analysis if available.
6. User marks item read, saves it, opens the original source, or creates a note from it.

Acceptance criteria:

- Feed list remains dense and scannable.
- Source and timestamp are visible without opening the original website.
- Read/unread and saved states are quick actions.
- Note creation from a feed item preserves provenance.

## Journey: Create Note From Feed Item

Intent: turn relevant information into durable company research.

Flow:

1. User opens a feed item.
2. User chooses `Create note`.
3. App opens a note draft for the selected company.
4. Draft includes source provenance and optionally prefilled title/body from selected item text.
5. User chooses note kind, tags, event date, and optional review period.
6. User saves the note.
7. Note appears in the company notebook and links back to the feed item.

Acceptance criteria:

- User can edit all generated or copied text before saving.
- Notes support Markdown in v1.
- Claim notes can be marked as open and assigned a future review period.
- Saved note always keeps source provenance.

## Journey: Company Notebook Review

Intent: answer "what do I know about this company and what should I check later?"

Flow:

1. User opens a company.
2. Company workspace is one company page with tabs or segmented views for Feed, Notebook, Claims, Transcripts, and Metadata.
3. Notebook view lists notes newest first, with filters by tag, kind, claim status, and review period.
4. User opens a note in the detail pane.
5. User edits note content, changes claim status, or opens linked source material.

Acceptance criteria:

- Company notebook is reachable from both company navigation and feed item context.
- Open claims and due review periods are visible.
- Notes can be traced back to feed items, reports, or transcript segments.

## Journey: YouTube Conference To Notes

Intent: capture relevant management statements from a press conference.

Flow:

1. User opens Transcripts or a company workspace.
2. User enters a YouTube URL and selects the target company.
3. App creates a Gemini-backed transcript job.
4. User sees job status while processing.
5. Transcript segments appear with timestamps when available.
6. User reviews transcript segments.
7. User selects transcript segments, text ranges, or AI-suggested note drafts.
8. User edits the note draft and saves it to the company notebook.

Acceptance criteria:

- Gemini is used only as the preferred provider for YouTube transcription.
- User confirms note content before saving.
- Saved notes link to transcript segments and original YouTube URL.
- Provider limits and privacy implications are visible before sending video data to the provider.

## Journey: Theme Selection

Intent: let the user keep the app comfortable for daily use.

Flow:

1. User opens Settings.
2. User selects dark, light, or system theme.
3. App applies the theme immediately.
4. App persists the selected setting locally.

Acceptance criteria:

- First-run theme is dark.
- Dark theme uses the `night-neon` palette.
- Light theme preserves the same accent identity.

## Screen Inventory

V1 screens:

- Inbox
- Company workspace
- Notebook list/detail
- Transcript jobs and transcript review
- Source status
- Settings

Deferred screens:

- portfolio positions
- trading activity
- cloud sync
- billing/licensing
- team collaboration

## Open UX Questions

### Company Workspace Structure

Decision: when you open a company such as `GPW:CDR`, it uses one company page with tabs or segmented views.

Why it matters: this decides how quickly you can move between a company's latest feed items, notebook, claims, transcripts, and metadata.

Considered options:

- Tabs or segmented views: one company page with sections like Feed, Notebook, Claims, Transcripts, and Metadata. This keeps company research in one place and is the recommended default.
- Split panes: company page shows multiple areas at once, for example feed on the left and notes on the right. This is powerful but can become cramped.
- Route-per-section: Feed, Notebook, Claims, and Transcripts are separate pages with URLs/routes. This is simple technically but can make research feel scattered.

V1 decision: tabs or segmented views inside one company workspace.

### Claim Review Periods

Decision: when you write a note like "management said X should happen soon", the app supports both a review quarter and an exact review date.

Why it matters: company promises are often tied to quarters, but sometimes you may want an exact date reminder.

Considered options:

- Quarters only: examples `2026-Q3`, `2026-Q4`. This matches earnings/reporting cadence and is simple for investor workflows.
- Exact dates only: examples `2026-09-30`, `2026-12-15`. This is precise but less natural for statements like "in the next two quarters".
- Both quarters and exact dates: store an optional quarter and optional exact date. This is more flexible and is the recommended default.

V1 decision: support both, but make quarter selection the most visible control for claim notes.

### Transcript Editability

Decision: after Gemini produces transcript segments from a YouTube conference, transcript segments are treated as source output and are not edited directly in v1. Notes created from transcripts are editable.

Why it matters: AI transcripts can contain mistakes. But if we edit the transcript directly, we lose a clean record of what the provider originally returned.

Considered options:

- Immutable transcript, editable note drafts: transcript segments are stored as source output and cannot be changed; you edit the note before saving. This preserves provenance and is the recommended default.
- Editable transcript plus editable notes: user can correct transcript text and then create notes from the corrected text. This is convenient but needs audit history.
- Store original and corrected transcript: preserve provider output and allow a corrected user version. This is best long-term but more complex for v1.

V1 decision: immutable transcript segments with editable note drafts.

Selection behavior does not need to be fully designed before implementation scaffolding, but the v1 UX must support at least one way to choose source material before creating a note. Acceptable interaction patterns include selecting whole transcript segments, selecting text ranges inside a segment, or accepting an AI-suggested note draft. The saved note must keep provenance to the original segment and YouTube URL even if the note text is edited.

### Source Status Placement

Decision: source and adapter health is shown in a dedicated Source Status screen.

Why it matters: you need confidence that the app is actually pulling GPW reports and other sources, but source diagnostics should not clutter daily reading.

Considered options:

- Full Source Status screen: a dedicated section showing adapters, last successful fetch, errors, rate limits, and manual refresh. This is transparent and useful for troubleshooting.
- Settings subsection: source status lives under Settings. This keeps navigation smaller but hides important operational information.
- Compact toolbar status plus detailed settings: toolbar shows a small health indicator; detailed status lives elsewhere. This is useful later, but still needs a detailed home.

V1 decision: full Source Status screen, with a compact indicator in the top toolbar later.

## Resolved UX Decisions

- Notes support Markdown in v1.
- Company workspace uses one company page with tabs or segmented views.
- Claim review supports both review quarter and exact review date.
- Transcript segments are immutable source output; notes created from them are editable.
- Source status has a dedicated screen.
