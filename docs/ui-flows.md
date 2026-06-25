# UI Flows

This document plans Brawler from the user experience inward. It defines the first workflows the app must make feel natural before detailed database schema or implementation scaffolding hardens.

Use [Project Brief](project-brief.md) for the full documentation map. Related references: [Product Spec](product-spec.md), [UI Information Architecture](ui-information-architecture.md), and [Contracts](contracts.md).

## UX North Star

Brawler should feel like a personal investor research desk:

- fast to scan each morning or evening
- dense enough for repeated professional use
- calm enough to avoid dashboard noise
- source-first and origin-aware
- easy to turn a report, article, or transcript excerpt into a durable note

The default screen is the investor inbox. The second most important surface is the company notebook.

## Primary Navigation

V1 navigation should use persistent desktop app regions:

- top toolbar for the brand, search, refresh, and source status
- top navigation bar (directly beneath the toolbar) for the primary sections; it wraps to a second line on narrow windows rather than scrolling or hiding items
- central list or workspace for the current task, full-width beneath the chrome
- right detail pane when reviewing a feed item, company, note, or transcript segment

The Inbox workspace splits the feed list and detail pane 50/50 by default; the divider is draggable between 25% and 75% of the row. See [ADR 0047](adr/0047-top-navigation-bar.md) for the navigation-region decision.

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
2. Feed shows newest items first, with a typed-signal badge on classified official filings (e.g. insider transaction, dividend, profit warning).
3. User filters by watchlist, company, item type, signal type, unread, saved, and significance when available.
4. User opens an item in the detail pane.
5. Detail pane shows title, source, publication time, matched companies, source URL, original text or excerpt, the typed signal(s), and AI analysis if available.
6. User marks item read, saves it, opens the original source, or creates a note from it.
7. For a filing typed only by the optional AI fallback, the signal shows as a proposal the user can confirm or reject before it is applied.

Acceptance criteria:

- Feed list remains dense and scannable.
- Source and timestamp are visible without opening the original website.
- Read/unread and saved states are quick actions.
- Note creation from a feed item preserves origin.
- A classified insider transaction is visually distinguishable in the feed and can be filtered to.
- AI-proposed signals are never applied without explicit user confirmation.
- Confirmed signals appear in the company research timeline (filterable as "Signals") and feed the personal digest; a high-signal classification (insider transaction, profit warning) raises a research reminder.

## Journey: Create Note From Feed Item

Intent: turn relevant information into durable company research.

Flow:

1. User opens a feed item.
2. User chooses `Create note`.
3. App opens a note draft for the selected company.
4. Draft includes source origin and optionally prefilled title/body from selected item text.
5. User chooses note kind, tags, event date, and optional follow-up period.
6. User saves the note.
7. Note appears in the company notebook and links back to the feed item.

Acceptance criteria:

- User can edit all generated or copied text before saving.
- Notes support Markdown in v1.
- Claim notes can be marked as open and assigned a future follow-up period.
- Saved note always keeps source origin.

## Journey: Company Notebook Review

Intent: answer "what do I know about this company and what should I check later?"

Flow:

1. User opens a company (from the Companies library, a pinned sidebar entry, a feed item, or global search), landing the curated cockpit dashboard scoped to it ([ADR 0057](adr/0057-composable-views-and-curated-dashboard.md)).
2. The dashboard opens with a calm default panel set (Fundamentals, Feed, Claims, Quality, Report documents, Notebook) and stays composable; the company `Notebook` panel is one of them.
3. Notebook panel lists notes newest first, with filters by tag, kind, claim status, and follow-up period.
4. User opens a note in the detail pane.
5. User edits note content, changes claim status, or opens linked source material.

Acceptance criteria:

- Company notebook is reachable from both company navigation and feed item context.
- Open claims and due follow-up periods are visible.
- Notes can be traced back to feed items, reports, or transcript segments.

## Journey: Track Company Fundamentals From A Report

Intent: turn a periodic report's reported numbers into a structured, source-linked fundamentals view.

Flow:

1. User opens a report feed item (e.g. an ESPI/EBI periodic report) in the detail rail.
2. The rail shows a compact AI KPI extraction launcher; the user opens it, which presents a centered modal.
3. In the modal the user picks a report source: an attached PDF, the company's IR reports page, or a pasted PDF URL.
4. The app extracts reported KPIs for the primary period; the modal shows the job status and proposed values with as-reported figures, confidence, and source snippets.
5. The user reviews each proposal — confirm, edit, or reject — or bulk-confirms known KPIs and accepts out-of-taxonomy suggestions as new company KPIs. No value is committed without confirmation.
6. The user opens the company's Fundamentals tab and sees the confirmed values in the KPI-per-period matrix, with trend charts and click-through provenance. Manual entry and custom per-company KPIs cover anything extraction missed.

Acceptance criteria:

- No AI-proposed number becomes a stored fact without explicit confirmation.
- Heavy extraction interaction happens in the modal, not crammed into the fixed-width detail rail.
- Confirmed facts appear in the Fundamentals matrix through the same read model as manually entered facts.
- Values display in their original as-reported scale with localized KPI names, never raw integers or internal ids.
- A document that is a web page rather than a report PDF is rejected with an actionable message, not a misleading partial extraction.

## Journey: Track A Management Claim To Verdict

Intent: capture a management promise, then resolve whether it was delivered when the due period's report arrives ([ADR 0040](adr/0040-management-claims-tracker.md)).

Flow:

1. From a report document or a transcript, the user opens the AI claim-extraction launcher (a modal, like KPI extraction). The app proposes candidate claims with the statement, a suggested due period, an optional quantitative target, confidence, and the verbatim source snippet.
2. The user reviews each proposal — confirm (with optional edits), or reject. No claim is created without confirmation. The user can also add a claim manually from the Claims tab.
3. The confirmed claim appears in the company workspace **Claims** tab with verdict `pending` and its due period, source-linked back to the report/transcript.
4. Later, when the due period's report arrives, the claim resurfaces in the **claims to verify** review queue (bucketed due / overdue / upcoming). For a quantitative claim, the matching confirmed financial fact is shown beside the claim.
5. The user sets the verdict (delivered / partially delivered / missed / revised), optionally linking the verifying fact as supporting or contradicting evidence.

Acceptance criteria:

- No AI-proposed claim becomes a tracked claim without explicit confirmation.
- A claim with a due period resurfaces in the review queue when the due-period report arrives, and can be resolved with a verdict linked to evidence (the milestone exit criterion).
- Verdicts are always user-set; the app never assigns a verdict automatically.
- Claims appear in the company research timeline and feed reminders/digests; they are exported with research data.
- Heavy extraction interaction happens in a modal, not crammed into the fixed-width detail rail.

## Journey: Prepare For Report Season

Intent: arrive at each tracked company's report date already knowing what to check, then close the loop when the report lands ([ADR 0044](adr/0044-report-season-cockpit.md)).

Flow:

1. The user opens **Report Season** (next to Inbox). The cockpit lists upcoming report dates across the watchlists, ordered by date; a stale-calendar indicator shows when the calendar is out of date.
2. Each upcoming report shows a pre-report card composed from the company's open research questions, unresolved claims (due / overdue / upcoming), last-period confirmed KPIs, and recent evidence.
3. The user reviews a card and marks the company **prepared**; from the card they can drill into the company workspace, its research questions, or its claims-review queue.
4. When the report arrives, the user marks it **processed**; the card links to the arrived filing and to KPI extraction for the new report, and the resurfaced claims appear in the claims-review queue.

Acceptance criteria:

- For a watchlist with upcoming reports, the cockpit shows dated cards whose contents match the company workspaces (the milestone exit criterion).
- A company can be taken from upcoming → prepared → processed against a real report cycle.
- The cockpit composes existing domains with no duplicated logic and adds no per-company data beyond the prepared/processed workflow state.
- The cockpit never auto-fetches or auto-extracts; processing links to the manual KPI-extraction and claims-review entry points.

## Journey: YouTube Conference To Notes

Intent: capture relevant management statements from a press conference.

Flow:

1. User opens Transcripts or a company workspace.
2. User enters a YouTube video link in a field labeled `URL`.
3. User may optionally provide the target company/ticker before transcription.
4. App creates a Gemini-backed transcript job.
5. If no company was provided, app attempts to recognize the company from the video/transcript result.
6. If recognition fails, the transcript remains available as an unlinked transcript.
7. User can optionally link the transcript to a company through the same local lookup used by Companies.
8. User sees job status while processing.
9. Transcript segments appear with timestamps when available.
10. User reviews transcript segments.
11. User selects one or more whole transcript segments.
12. If the transcript is linked to a company, user edits the note draft and saves it to that company notebook.

Acceptance criteria:

- Gemini is used only as the preferred provider for YouTube transcription.
- M10 completion requires real Gemini transcript generation for a supported public YouTube URL; sample transcript output is only for development and automated tests.
- URL is the primary required input; company is optional upfront and optional after transcription.
- Unlinked transcripts remain valid and viewable.
- Company selection uses the same cached company lookup/autocomplete behavior as Companies when the user wants a company notebook note.
- User confirms note content before saving.
- Saved notes link to transcript segment IDs, original YouTube URL, provider/job context, and timestamp ranges when available.
- Provider limits and privacy implications are visible before sending video data to the provider.

## Journey: Appearance And Locale Settings

Intent: let the user keep the app comfortable for daily use.

Flow:

1. User opens Settings.
2. User selects dark, light, or system theme.
3. App applies the theme immediately.
4. User can select English or Polish as the app language.
5. App applies app-owned UI copy for the selected language.
6. App persists the selected settings locally.

Acceptance criteria:

- First-run theme is dark.
- First-run locale is English.
- Dark theme uses the `night-neon` palette.
- Light theme preserves the same accent identity.
- Polish locale is available from Settings.
- Locale handling is extensible so future supported languages can be added through locale resources/configuration instead of per-screen rewrites.
- Source-provided text, company names, ticker symbols, URLs, source attribution, transcript text, and notebook bodies keep their original or user-entered language.

## Journey: Global Search

Intent: find anything stored locally from anywhere in the app.

Flow:

1. User opens global search from the top-toolbar search box (or its keyboard shortcut).
2. User types a query.
3. App shows ranked results grouped by content type (companies, feed items, notes, transcript segments, research briefs, digests), each with a snippet.
4. User selects a result.
5. App navigates to the owning screen/item.

Acceptance criteria:

- Searching a known phrase from feed, note, and transcript content returns it and navigates correctly.
- Results are ranked and grouped by content type with a snippet.
- The per-workspace search/filter inputs (Inbox, Companies, Notebooks) still work independently.
- Search copy is available in English and Polish.

## Journey: Restore From Backup

Intent: recover the local data from an automatic backup after loss or a bad upgrade.

Flow:

1. User opens Developer Diagnostics and the backups section.
2. User reviews backup status and the list of rotating backups and pre-migration snapshots.
3. User chooses a backup and selects restore.
4. App requires explicit confirmation and warns that restore is applied on relaunch.
5. App stages the chosen backup and relaunches to apply it.

Acceptance criteria:

- A backup exists before every data-structure migration; a failed migration leaves the snapshot intact.
- Rotating backups keep the last N and prune the oldest.
- Restore from Diagnostics recovers a verified earlier state after relaunch.
- Restore never performs a hot in-place swap while connections are open.

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

### Report Backfill And Document Capture (v0.41.0)

Decision: tracking a company offers an explicit **"Backfill history"** action, and report files are captured into the company's evidence rather than left as external links only. See [ADR 0036](adr/0036-report-document-storage-and-backfill.md).

Why it matters: a freshly tracked company otherwise starts with an empty timeline, and AI extraction/diff have no local document to work from.

Flow:

1. The user tracks a company (or opens its workspace) and triggers **Backfill history**. A progress indicator shows pages fetched, items ingested, and documents stored, with a cancel control; diagnostics surface any per-item fetch errors.
2. As backfill and normal refresh ingest official filings, periodic-report attachments are stored as full report documents and appear linked from the company's Fundamentals/timeline; routine filings show as linked metadata only.
3. Re-running backfill or refreshing again does not create duplicate items, documents, or events; backfilled items keep their original publication dates.
4. A dividend or general-meeting filing with a stated future date surfaces a **proposed** calendar event; the user confirms or rejects it before it appears on the calendar. A date is never placed on the calendar without confirmation.

V1 decision: backfill is user-triggered, app-open-only, ~3-year depth, official sources only; historical calendar entries are not backfilled.

### Report-Over-Report Diff (v0.47.0)

Decision: a stored periodic **financial statement** can be diffed section-by-section against the previous same-type filing, from the company workspace and on new-report arrival. See [ADR 0052](adr/0052-report-over-report-diff.md).

Why it matters: an investor should see what changed since last quarter without rereading an 80-page filing.

Flow:

1. From the company's report documents (Fundamentals/timeline) the user picks a financial statement and chooses **Compare with previous**; the app pairs it with the prior same-type statement (SSF↔SSF, JSF↔JSF). When a new periodic report arrives, the diff is offered as an entry point.
2. The diff view shows aligned sections: unchanged, changed, only-in-older, only-in-newer. Changed sections show the textual delta with both reports cited (the user can open either source).
3. The diff is deterministic and local (no AI, no network); reopening the same pair shows the same result. While a report's text is still being extracted, the view shows an extraction-pending state; a scanned report with no text layer shows an explicit "can't diff" state.

V1 decision: structured financial statements only; the narrative management report (MD&A) and an AI delta summary are deferred ([ADR 0052](adr/0052-report-over-report-diff.md)); no cross-company diff.

### Claim Follow-up Periods

Decision: when you write a note like "management said X should happen soon", the app supports both a follow-up quarter and an exact follow-up date.

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

- Immutable transcript, editable note drafts: transcript segments are stored as source output and cannot be changed; you edit the note before saving. This preserves origin and is the recommended default.
- Editable transcript plus editable notes: user can correct transcript text and then create notes from the corrected text. This is convenient but needs audit history.
- Store original and corrected transcript: preserve provider output and allow a corrected user version. This is best long-term but more complex for v1.

V1 decision: immutable transcript segments with editable note drafts.

Selection behavior does not need to be fully designed before implementation scaffolding, but the v1 UX must support at least one way to choose source material before creating a note. Acceptable interaction patterns include selecting whole transcript segments, selecting text ranges inside a segment, or accepting an AI-suggested note draft. The saved note must keep origin to the original segment and YouTube URL even if the note text is edited.

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
- Claim follow-up supports both follow-up quarter and exact follow-up date.
- Transcript segments are immutable source output; notes created from them are editable.
- Source status has a dedicated screen.
