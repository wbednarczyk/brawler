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

Purpose: fast daily review of new reports and news.

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

Milestone 3 starts with inline source/provenance detail inside the Company Feed tab. The feed row itself is the click target instead of a separate inspect button. Clicking a feed row, or pressing Enter/Space on a focused feed row, expands the detail directly under that row; repeating the action collapses it. Up and Down arrows move through company feed rows while preserving expansion state: collapsed feed details stay collapsed, and already-open detail moves to the focused feed row. Company feed rows use the same unread dot and read/unread typography as Inbox feed rows. The inline detail should show source, type, timestamps, attribution, language, summary, source URL, read/save actions, and an explicit action to open the item in the Inbox with the company filter applied.

If the selected company has no stored feed items, the Feed tab shows an inline empty state instead of a blank panel. The empty state keeps the workspace anchored to the selected company and provides an `Open filtered Inbox` action so the user can verify the same company filter in the main review surface.

Notebook tab:

- notes list newest first
- filters by tag, kind, claim status, review quarter, review date
- note detail/editor pane
- create manual note

Claims tab:

- claim notes only
- grouped by open, due soon, delivered, missed, unknown
- visible review quarter and review date
- quick status update

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

- filters: company, tag, kind, claim status, review period
- note list
- note detail/editor pane

Use cases:

- find all open claims
- review notes due this quarter
- search personal research across companies

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
- Saved notes preserve transcript segment and YouTube provenance.

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
