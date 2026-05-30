# UI Information Architecture

This document turns the UX flows into concrete v1 screens, regions, and actions. It should guide the first React/Tauri scaffold before detailed visual design or component implementation.

See also [UI Flows](ui-flows.md), [Product Spec](product-spec.md), [Contracts](contracts.md), and [ADR 0006: Theme and Visual Direction](adr/0006-theme-and-visual-direction.md).

## App Shell

V1 uses a desktop app shell with persistent navigation and work-focused density.

Regions:

- left sidebar: primary sections, watchlists, pinned companies
- top toolbar: global search, manual refresh, source health indicator, theme/settings access
- main workspace: current screen content
- right detail pane: selected feed item, note, transcript segment, or job detail

Primary sections:

- Inbox
- Companies
- Notebooks
- Transcripts
- Sources
- Settings

Shell behavior:

- Dark theme is the first-run default.
- Sidebar should be collapsible after v1 if space becomes tight, but v1 can keep it fixed.
- The top toolbar stays visible while the current workspace scrolls.
- The Inbox navigation item shows an unread count badge when unread feed items exist.
- The top toolbar source health indicator summarizes locally registered source adapters and opens the Sources screen with the most relevant adapter expanded. Manual source refresh remains a separate disabled control until ingestion jobs exist.
- Detail pane should be dismissible.
- Empty states should offer direct actions and avoid marketing copy.
- Common mutations should provide immediate visual confirmation without blocking the workflow.
- Intuitive, responsive UX is a core product requirement for every screen.

## Inbox Screen

Purpose: fast daily notes work of new reports and news.

Main regions:

- filter toolbar: watchlist, company, item type, unread, saved, significance
- review summary: visible, unread, and saved counts for the current filtered set
- feed list: newest first, dense rows
- detail pane: selected item content and actions

Feed row should show:

- title
- company ticker(s)
- source name
- item type
- publication time
- unread/saved state
- significance when available

The selected feed row remains visually highlighted and exposes semantic current-item state so it stays clearly connected to the detail pane.

If the selected item disappears from the current filtered set, the next visible row becomes selected. If no rows remain, the feed list shows the appropriate empty state and the detail pane becomes unselected.

Detail pane should show:

- title
- source attribution and source URL
- publication and fetch timestamps
- matched companies
- original excerpt/body when available
- AI summary/significance when available
- actions: mark read/unread, save/unsave, open matched company workspace, open source, create note

Empty states:

- no companies tracked: prompt to add a company
- no stored feed items after companies exist: show refresh pending and link to Sources
- no items for filters: prompt to clear filters
- source errors: link to Sources screen

The UI-facing feed is scoped to tracked companies. Create-note is available for feed items that match a locally tracked company, and the note draft attaches to that company automatically.

## Companies Screen

Purpose: manage tracked companies and open company workspaces.

Main regions:

- company search/add control
- company list grouped or filtered by watchlist/exchange
- company metadata summary

Actions:

- add company by exchange-qualified ticker
- edit company metadata
- add/remove company from watchlist
- open company workspace

Early implementation may expose watchlist assignment directly on company rows. This is acceptable for proving storage and command behavior, but the workflow should be refined before v1 because repeated row-level assign/remove actions are tedious.

Milestone 3 implementation starts the company workspace from the Companies screen. Clicking a company row expands the ticker-focused workspace inline directly under that row, and clicking the same row again collapses it. Up and Down arrows move through company rows while preserving expansion state: collapsed lists stay collapsed, and an already-open workspace moves to the focused company. This keeps the expanded context anchored to the company the user selected and avoids adding another row-level button.

## Company Workspace

Purpose: one company page for all research around a ticker.

Header should show:

- qualified ticker
- display name
- exchange
- watchlist membership
- last feed update
- feed, unread, and saved counts
- quick actions: refresh company, add note, add transcript

Tabs or segmented views:

- Feed
- Notebook
- Claims
- Transcripts
- Metadata

Feed tab:

- company-filtered feed list
- same feed item detail behavior as Inbox
- create note from item

Milestone 3 starts with inline source/origin detail inside the Company Feed tab. The feed row itself is the click target instead of a separate inspect button. Clicking a feed row, or pressing Enter/Space on a focused feed row, expands the detail directly under that row; repeating the action collapses it. Up and Down arrows move through company feed rows while preserving expansion state: collapsed feed details stay collapsed, and already-open detail moves to the focused feed row. Company feed rows use the same unread dot and read/unread typography as Inbox feed rows. The inline detail should show source, type, timestamps, attribution, language, summary, source URL, read/save actions, an explicit action to open the item in the Inbox with the company filter applied, and an action to create an editable note draft with feed-item origin.

If the selected company has no stored feed items, the Feed tab shows an inline empty state instead of a blank panel. The empty state keeps the workspace anchored to the selected company and provides an `Open filtered Inbox` action so the user can verify the same company filter in the main review surface.

Notebook tab:

- notes list newest first
- filters by tag, kind, claim status, follow-up quarter, follow-up date
- note detail/editor pane
- create manual note

Milestone 4 begins with a company-scoped Notebook tab that lists durable notes and provides a compact manual Markdown note form. The form captures title, body, tags, note kind, optional claim status, event date, follow-up quarter, and follow-up date. Feed-item note drafts are added after the base manual-note path is stable.

The Notebook tab should be dense enough for dozens of notes per company. Use a compact selectable note list with title, kind, tags, status, and follow-up cues, plus a selected-note detail/editor area for reading and editing the full Markdown body. Note rows should not show raw body previews because uninterpreted Markdown is noisy in dense lists. The creation form should be available on demand instead of permanently consuming vertical space.

Read mode renders common Markdown structure locally. Edit mode exposes the raw Markdown body so the user can make precise changes without a rich-text editor layer.

Notebook date and follow-up-quarter fields should support direct typing plus compact picker controls. Date fields use the native date picker so the operating system/browser can provide localized calendar behavior. Follow-up-quarter fields use a small quarter picker, with `Today` setting the current quarter.

Claims tab:

- claim notes only
- grouped by open, due soon, delivered, missed, unknown
- visible follow-up quarter and follow-up date
- quick status update

Milestone 4 starts this as a compact claim follow-up list backed by notebook entries. Claim rows expand in place under the clicked row, following the app-wide row interaction pattern. The first status workflow updates only claim status and preserves the rest of the note. Later refinement can add stronger grouping, due-soon logic, and batch follow-up.

Transcripts tab:

- transcript jobs for this company
- submit YouTube URL
- review transcript segments
- create note from selected segments

Metadata tab:

- display name
- exchange
- ticker
- ISIN, CIK, LEI
- aliases
- source-specific IDs

## Notebooks Screen

Purpose: cross-company note review.

Main regions:

- company navigator: company list with ticker, display name, note count, open claim count, and due follow-up cues
- filters: company, tag, kind, claim status, follow-up period
- note list
- note detail/editor pane
- selected-company manual note creation form

Use cases:

- create a manual note for the selected company from the daily notes workspace
- navigate notes company-by-company without leaving the Notebooks screen
- find all open claims
- follow-up notes due this quarter
- search personal research across companies

The working assumption is that the Notebooks screen becomes the daily notes workspace. It should therefore make company switching cheap and obvious, not force the user to bounce through the Companies screen for ordinary note follow-up. Its first implementation provides a company navigator, selected-company note creation, tracked-company feed-to-note drafts, compact filters for kind, claim status, tag, and follow-up scheduling presence, and company-scoped note rows that expand in place. Company navigator rows show note count, open-claim count, and follow-up scheduled count when present. The open-claim cue is actionable: selecting it opens that company and applies the `Open` claim-status filter. The follow-up cue is actionable: selecting it opens that company and applies the `Has follow-up` filter. Expanded notes open in read mode and switch in place to edit mode with the same core editable metadata fields as the company workspace note editor, including event date and exact follow-up date. Opening the full company workspace should be available from the company navigator as a small contextual action, not as a separate toolbar that competes with note reading. The company workspace Notebook tab remains useful when the user is already researching a ticker, but it is not expected to carry the whole notes workflow alone.

Note detail surfaces should show origin links as compact actions. Feed-item origins should open the referenced item in the Inbox with filters adjusted so the item is visible. URL-backed origins should expose an external source action.

The first follow-up filter distinguishes notes that have a follow-up quarter or exact follow-up date from notes without follow-up scheduling. Due-this-quarter and overdue logic are later follow-up automation refinements.

## Transcripts Screen

Purpose: manage YouTube transcription jobs across companies.

Main regions:

- submit job form: YouTube URL, target company
- job list: queued, running, succeeded, failed
- transcript segment review
- note draft pane

Rules:

- Gemini is preferred only for YouTube transcription.
- Transcript segment text is immutable source output in v1.
- User edits note drafts, not transcript source text.
- Saved notes preserve transcript segment and YouTube origin.

## Events Screen

Purpose: show company events for companies in the user's watchlists, with upcoming events as the default view and historical dates available on demand.

Main regions:

- date-grouped event list
- watchlist, company, and event-type filters
- date-range mode: upcoming, historical, custom range, or all
- due-soon summary
- historical timeline/search summary
- event detail expansion
- manual event creation or correction workflow

Event rows should follow the app-wide row interaction pattern: the row is the primary click target, and details expand inline under the selected event row. The collapsed row should be compact enough to scan many dates and should show date, company ticker, event type, source/manual marker, and status. Upcoming events should be visually prioritized by default, while historical rows should remain readable but less attention-grabbing. The expanded detail should show source URL, attribution, fetched timestamp, event timestamp/date, related company, and notes about manual corrections if present.

The first implementation may use fixture-backed events. The UX should still assume future official-source events can coexist with manual events and user corrections without hiding where the date came from.

## Sources Screen

Purpose: show whether data ingestion is healthy.

Main regions:

- adapter list
- last successful fetch
- last error/warning
- next scheduled poll
- manual refresh action
- source policy notes or links

Source rows follow the same list/detail behavior as the rest of the app: clicking a source adapter, or pressing Enter/Space on a focused source row, expands operational details inline under that row. Repeating the action collapses the details.

V1 adapters:

- GPW ESPI/EBI
- selected public/RSS media sources when approved

Future adapters:

- SEC EDGAR
- Nasdaq RSS
- major European exchange sources

## Settings Screen

Purpose: local preferences and provider configuration.

Sections:

- Appearance: dark/light/system theme, `night-neon` palette
- Ingestion: polling interval, manual refresh defaults
- AI providers: Gemini configuration for YouTube transcription, future general AI provider slots
- Privacy: local data location, provider data disclosure
- About: codename, app version, license status

## Search

Global search should eventually cover:

- companies
- feed items
- notes
- transcript text

V1 can start with company and feed search, then expand.

## Responsive Behavior

The primary target is desktop. The first implementation should still avoid layouts that break at narrow widths:

- sidebar may collapse
- detail pane may become an overlay
- dense tables/lists should preserve readable text
- icon buttons need tooltips when meaning is not obvious

## Deferred UI

Do not build these in v1:

- portfolio position tracking
- trade journal
- billing/licensing UI
- cloud sync UI
- team or sharing workflows
