# UI Information Architecture

This document turns the UX flows into concrete v1 screens, regions, and actions. It should guide the first React/Tauri scaffold before detailed visual design or component implementation.

Use [Project Brief](project-brief.md) for the full documentation map. Related references: [UI Flows](ui-flows.md), [Product Spec](product-spec.md), [Contracts](contracts.md), and [ADR 0006: Theme and Visual Direction](adr/0006-theme-and-visual-direction.md).

## App Shell

V1 uses a desktop app shell with persistent navigation and work-focused density.

Regions:

- left sidebar: primary sections and pinned companies
- top toolbar: manual refresh, source health indicator, theme/settings access
- main workspace: current screen content
- right detail pane: selected feed item, note, transcript segment, or job detail

Primary sections:

- Inbox
- Companies
- Notebooks
- Transcripts
- Sources
- Settings

Developer-only section:

- Diagnostics, visible only when Developer mode is active

Shell behavior:

- Dark theme is the first-run default.
- Sidebar should be collapsible after v1 if space becomes tight, but v1 can keep it fixed.
- The top toolbar stays visible while the current workspace scrolls.
- The app shell owns the viewport height; screens should use internal panel/list scroll areas instead of relying on page/body scrolling.
- The browser page should not expose a global application scrollbar. The left navigation, top toolbar, and each screen's primary header or control bar should remain visible while long lists, detail panes, or subpanels scroll internally.
- Desktop layouts must remain usable outside maximized windows, including a side-region window on an ultrawide monitor. Multi-column screens should stack or simplify around this size before text, buttons, filters, or panels become cramped.
- The Inbox navigation item shows an unread count badge when unread feed items exist.
- The top toolbar source health indicator summarizes locally registered sources and opens the Sources screen with the most relevant source expanded. Manual source refresh remains a separate disabled control until ingestion jobs exist.
- Detail pane should be dismissible.
- Empty states should offer direct actions and avoid marketing copy.
- Common mutations should provide immediate visual confirmation without blocking the workflow.
- Intuitive, responsive UX is a core product requirement for every screen.
- Distinct spaces inside a view must be visually distinguishable at a glance. Prefer reusable section headers with a semantic color accent, compact title, and short supporting label over same-looking boxes stacked together. Color should clarify structure, not decorate randomly.
- Normal user-facing UI copy should use product terms and avoid implementation details such as SQLite, Tauri, database engine, internal adapter, module, collector, schema, or command boundary. Technical terms are reserved for Developer Diagnostics and owner/developer docs.
- Exchange-qualified ticker labels use a shared visual renderer that distinguishes exchange and symbol segments with explicit known-exchange colors plus deterministic fallback palette colors for future exchanges. The renderer must keep the underlying `qualifiedTicker` string contract unchanged.

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
- source URL as an actionable original link
- publication and fetch timestamps as lower-priority footer metadata
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
- company list searchable and filtered by watchlist/exchange
- company metadata summary

Actions:

- add company by exchange-qualified ticker
- edit company metadata
- open company workspace
Company rows should show current watchlist memberships for scanning, but membership editing belongs in the dedicated Watchlists menu panel. The company list and expanded company workspace should not show watchlist create, delete, add, or remove controls. The company workspace can show current memberships as context only.

The company list should own its vertical scrolling so the company add/search/filter controls remain visible while reviewing long tracked-company lists.

## Watchlists Screen

Purpose: manage user-owned company groups used by filters across the app.

Main regions:

- watchlist create control
- watchlist selector/list
- selected-watchlist member companies
- searchable list of already-tracked companies available to add

Actions:

- create, rename, and delete watchlists
- add already-tracked companies to the selected watchlist
- remove companies from the selected watchlist

The Watchlists screen is a dedicated left-menu panel. It should use a watchlist-first dual-pane workflow: select a watchlist, then manage that watchlist's member companies. Renaming a watchlist should preserve the watchlist's stable internal id. Removing a company from a watchlist should happen in this panel without deleting the company itself. Deleting a watchlist should require confirmation and should not delete member companies. If a deleted watchlist is active in a view filter, that filter should reset to `All`.

Milestone 3 implementation starts the company workspace from the Companies screen. Clicking a company row expands the ticker-focused workspace inline directly under that row, and clicking the same row again collapses it. Up and Down arrows move through company rows while preserving expansion state: collapsed lists stay collapsed, and an already-open workspace moves to the focused company. This keeps the expanded context anchored to the company the user selected and avoids adding another row-level button.

## Company Workspace

Purpose: one company page for all research around a ticker.

Header should show:

- qualified ticker
- display name
- exchange
- watchlist memberships as read-only context
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
- submit YouTube `URL`
- review transcript segments
- create note from selected segments

Global or notebook-level transcript entry starts from a required `URL` field and optional company/ticker field. If the company is omitted, the app should try to recognize it after transcription, but the transcript can remain unlinked for general market videos or any non-company-specific recording. Company selection is required only when saving selected segments into a company notebook.

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
- filters: watchlist/company navigator, tag, kind, claim status, follow-up period
- note list
- note detail/editor pane
- selected-company manual note creation form

Use cases:

- create a manual note for the selected company from the daily notes workspace
- navigate notes company-by-company without leaving the Notebooks screen
- find all open claims
- follow-up notes due this quarter
- search personal research across companies

The working assumption is that the Notebooks screen becomes the daily notes workspace. It should therefore make company switching cheap and obvious, not force the user to bounce through the Companies screen for ordinary note follow-up. Its primary layout is a desktop master-detail workspace with independent scroll areas: company navigator, note list/filter column, and selected note/editor pane. Desktop users can resize the company navigator and note-list panels so long company names, dense note lists, or active note editing can get the needed width. Company navigator rows show the full company name first, then the exchange-qualified ticker, plus note count, open-claim count, and follow-up scheduled count when present, and the navigator can be narrowed by watchlist for focused review. The open-claim cue is actionable: selecting it opens that company and applies the `Open` claim-status filter. The follow-up cue is actionable: selecting it opens that company and applies the `Has follow-up` filter. Selected notes open in read mode in the detail pane and switch there to edit mode with the same core editable metadata fields as the company workspace note editor, including event date and exact follow-up date. Opening the full company workspace should be available from the company navigator as a small contextual action, not as a separate toolbar that competes with note reading. The company workspace Notebook tab remains useful when the user is already researching a ticker, but it is not expected to carry the whole notes workflow alone.

Note detail surfaces should show origin links as compact actions. Feed-item origins should open the referenced item in the Inbox with filters adjusted so the item is visible. URL-backed origins should expose an external source action.

The first follow-up filter distinguishes notes that have a follow-up quarter or exact follow-up date from notes without follow-up scheduling. Due-this-quarter and overdue logic are later follow-up automation refinements.

## Transcripts Screen

Purpose: manage YouTube transcription jobs across companies.

Main regions:

- submit job form: YouTube URL, optional target company
- job list: queued, running, succeeded, failed
- transcript segment review
- note draft pane

Rules:

- Gemini is preferred only for YouTube transcription.
- M10 requires a real Gemini run path for supported public YouTube URLs; offline sample output is not a production user workflow.
- Transcript jobs use the app-wide expandable-row pattern: click a job row to show or hide inline details.
- Transcript jobs are standalone records first; company binding is optional.
- Transcript segment text is immutable source output in v1.
- Completed jobs show transcript segments in chronological order with timestamp ranges when available.
- Segment selection works on whole transcript segments; finer text-range selection can be revisited after the first workflow is usable.
- User edits note drafts, not transcript source text.
- Saved company notebook notes preserve transcript segment and YouTube origin.

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

The first implementation may use test-sample-backed events. The UX should still assume future official-source events can coexist with manual events and user corrections without hiding where the date came from.

## Sources Screen

Purpose: show whether data ingestion is healthy.

Main regions:

- implemented source groups by purpose
- source list within each group
- last successful fetch
- last error/warning
- next scheduled poll
- manual refresh action
- source links
- optional source enable/disable controls

Source rows follow the same list/detail behavior as the rest of the app: clicking a source, or pressing Enter/Space on a focused source row, expands operational details inline under that row. Repeating the action collapses the details.

Normal Sources shows required and optional implemented sources only. Required sources, such as company-directory support, can be refreshed but not disabled. Optional implemented sources can be enabled or disabled, and that state affects real refresh behavior. Developer-only source candidates, source IDs, fetch modes, policy notes, rate-limit detail, and unmatched diagnostics are hidden from normal Sources and belong in Developer Diagnostics or docs.

Groups separate official reports, calendar/events, public media/news, and company directory support.

V1 adapters:

- GPW ESPI/EBI
- selected public/RSS media sources when approved

Future adapters:

- SEC EDGAR
- Nasdaq RSS
- major European exchange sources

## Developer Diagnostics Screen

Purpose: inspect local module behavior while Developer mode is active without exposing diagnostics to normal users or sending any event off the machine.

Main regions:

- module filter: AI analysis, external AI, sources, scheduler, credentials, storage, transcripts, shortcuts, locale, licensing, packaging, and future modules
- severity filter: debug, info, warning, error
- timeline list: newest meaningful diagnostic events first
- event detail pane or inline expansion: redacted metadata, scope/entity ID, stage, timestamp, severity, and message
- actions: clear diagnostics and copy redacted diagnostic summary
- metrics tab or section: local operational counters, gauges, and durations from the Developer-mode metrics snapshot
- logs tab or section: full in-app runtime log viewer, log status, copy redacted log output, and open logs folder
- source candidates section: registered developer-tier source candidates, IDs, source types, fetch modes, source URLs, and source-policy notes
- developer mode status and disable action

Rules:

- The Diagnostics navigation item is hidden unless Developer mode is active.
- Diagnostic events are for troubleshooting only and must not replace normal user-facing status, errors, or progress UI.
- Event details must clearly show that metadata is redacted and local-only.
- Metrics are operational health signals, not product analytics. Process-lifetime counters must be presented as runtime-only signals that reset on app restart.
- Runtime log viewing is available only from Diagnostics while Developer mode is active, even though log configuration is visible in Settings.
- The first rich timeline is AI analysis job progress, including queued, running, context loaded, provider resolved, credential checked, request sent, response received, parsed, stored, and failed.
- Non-AI modules may show lightweight baseline events where useful, while detailed logs and metrics remain separate observability surfaces.
- Raw diagnostic JSON/file export is outside M14 scope.

## Settings Screen

Purpose: local preferences and provider configuration.

Settings uses local subnavigation rather than route-level pages. Each section opens in the settings workspace while the settings navigation remains visible.

Sections:

- Appearance: dark/light/system brightness mode, separate accent palette with `night-neon` and `midnight-horizon`, extensible locale setting with English default and Polish as the first additional language
- Sources: polling interval, feed cleanup status, import/export status
- AI providers: Gemini configuration for YouTube transcription, selectable transcription model, credential configured/not-configured status, credential storage, secret kind, and future general AI provider slots
- Credentials: credential configured/not-configured status, credential storage, secret kind, save/replace/clear controls
- Keyboard shortcuts: discoverable action list, configurable bindings, conflict visibility, disable, and reset controls
- Logs: local runtime log level and rotation limits, with a clear local-only/no-telemetry explanation
- Import and Export: export/import research data, export/import safe preferences, preview import changes before applying them
- License: optional local entitlement status, safe metadata, replace, and clear controls
- Privacy: local data location, provider data disclosure
- About: app name, app version, license status

## License Entitlements

Purpose: show and manage optional local license entitlements without blocking normal open-core app use.

Behavior:

- Normal app navigation remains available without a license key.
- Settings accepts a pasted license key and shows recoverable missing, malformed, invalid, expired, unsupported-version, unsupported-channel, and storage-error states.
- Successful activation can enable current or future gated entitlements.
- Normal Settings/About license controls remain available for inspection, replacement, and clearing.
- The UI must not imply cloud activation, billing, account sync, telemetry, or investment advice.

## Search

Global search should eventually cover:

- companies
- feed items
- notes
- transcript text

V1 keeps search scoped to the workspace that owns the result list. Inbox owns feed-item search inside its filter toolbar, Companies owns company-list search, and Notebooks owns note filtering. The top toolbar must not show a search box until a true cross-workspace result model exists.

## Future Research Workspace

Purpose: provide source-grounded company and watchlist review surfaces once the research/evidence boundary exists.

Candidate regions:

- company or watchlist selector
- evidence timeline
- changed-since-review summary
- open claims and research questions
- upcoming events and reminder pressure
- AI research brief panel with citations
- related evidence links

Rules:

- Do not add visible Research Workspace placeholders before the feature is implemented.
- M25 implements Research as a real top-level screen, scoped to a selected tracked company.
- M25 Research shows a screen header with selected company context, last-reviewed state, total evidence count, changed-since-review count, evidence type filters, changed-only filter, and a `Mark reviewed` action.
- M25 Research evidence rows are compact timeline rows with product-language type/trust labels, occurred timestamp, title, summary when available, changed-since-review state, and owning-domain/source actions where practical.
- M31 Research shows open reminders and generated digest snapshots as real company/watchlist review tools. The screen receives reminder and digest read models from backend commands; it must not assemble digest inputs in React.
- M26 Research adds a Company/Watchlist mode switch in the same screen, not a separate screen.
- Watchlist mode uses a left-side member-company review queue and a right-side evidence timeline for the selected member company.
- Watchlist review defaults to updating the watchlist checkpoint only. The optional cascade to member-company checkpoints must be an explicit user action and backend command input.
- M29 Research adds a company-scoped Questions panel in the same Research screen. The user creates, selects, answers, closes, reopens, and links questions to evidence without leaving the research context.
- The selected research question controls link actions on evidence rows. Evidence rows may show a compact link action only when a selected question can link to that evidence item.
- Future research views consume backend-owned evidence and timeline read models.
- Research views should not independently call Inbox, Notebooks, Events, Transcripts, Sources, and AI APIs to assemble timelines.
- Research summary counts, changed-since-review state, and timeline filtering semantics belong to backend read models. The UI displays the result and captures user intent.
- Review actions update research-owned review checkpoints.
- Evidence-link interactions use typed research links while preserving existing notebook origin links.
- AI brief UI must show citations and provenance enough for source-grounded review, without presenting buy/sell/hold recommendations.

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
- hosted billing/licensing UI
- cloud sync UI
- team or sharing workflows
