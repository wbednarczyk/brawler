# UI Information Architecture

This document turns the UX flows into concrete v1 screens, regions, and actions. It should guide the first React/Tauri scaffold before detailed visual design or component implementation.

Use [Project Brief](project-brief.md) for the full documentation map. Related references: [UI Flows](ui-flows.md), [Product Spec](product-spec.md), [Contracts](contracts.md), and [ADR 0006: Theme and Visual Direction](adr/0006-theme-and-visual-direction.md).

## App Shell

V1 uses a desktop app shell with persistent navigation and work-focused density.

Regions:

The shell is **mode-based and thesis-centric** ([ADR 0054](adr/0054-mode-based-thesis-centric-shell.md), Accepted) — it leads with the investor's job, not a freeform canvas:

- top toolbar: brand, **global search / "ask anything"** (a first-class session entry point, semantic), manual refresh, source health indicator, theme/settings access. A **command palette (⌘K)** is a fast accelerator, complementing — not replacing — the visible spine.
- **left sidebar — the IA spine (implemented):** grouped into **Modes** (Today/Pulse · Companies/Company-workspace · Compare; plus an interim **Cockpit** advanced-workspace entry until it folds into the Company workspace), a **pinned/favorite companies** group (each with a glanceable conviction status — a neutral placeholder until per-company conviction lands), a **Library** group (Inbox, Watchlists, Transcripts, Sources), and a **Utilities** group (Settings, Diagnostics — developer-gated). This is the load-bearing navigation; a blank/freeform workspace is never the entry point.
- **main area — the active mode's content** (see the modes below).
- **focus surfaces:** full-screen reader/writer modes invoked from anywhere (`Esc` back).

Modes / destinations:

- **🏠 Today / Pulse** (default / home) — a Triage-style attention queue ("what changed / to verify / stale") plus a watchlist-level conviction rollup; the feed is secondary input.
- **🔬 Company workspace** — a single-company deep-dive in fixed, modular, progressively-disclosed sections (Overview, Fundamentals, Valuation, Quality, What-changed, Claims, Reports, Notebook, Thesis), entered by selecting a company; **dockview's free arrangement is the opt-in "advanced layout"** within it. Single-company settings (autopilot mode, IR reports URL) live inline in Fundamentals; **cross-company settings management** is a separate **master-detail surface** ("Manage settings" in the Companies screen — company multi-select + grouped settings applied to the selection, with watchlist-scope selection), the scalable home for *all* per-company settings ([ADR 0056](adr/0056-per-company-settings-surface.md); v1 ships autopilot, pinned/watchlists next).
- **⚖️ Compare** — cross-company KPI tables (v0.53).
- **📋 Journal** — the decision journal (v0.56).
- **📖 Focus** — distraction-free reading (a long report diff) / writing (a thesis or note).

Developer-only: Diagnostics, visible only when Developer mode is active.

Interim state (the shell is built incrementally, [ADR 0054](adr/0054-mode-based-thesis-centric-shell.md)): the **left-sidebar IA spine + pinned companies** and the **Today/Pulse attention home** have landed, and **Today/Pulse is now the default landing** (replacing the cockpit default). Today/Pulse is a real attention digest — *what changed* (recently-reported companies), *to verify* (claims due/overdue for the **pinned** companies — a bounded set, not every watchlist company), *upcoming reports*, and a watchlist conviction **rollup placeholder** — composed from existing app-wide read models; each item offers a **Review** (navigate) action. The full **accept/snooze/dismiss triage state** and the autonomous-pipeline "one notification" are owned by the v0.48 feed-triage / v0.49 pipeline epics and plug into this home. The **Company workspace** is the sectioned (tabbed) deep-dive by default, and **dockview is now the opt-in "Advanced layout"**: an *Advanced layout* button in the workspace header opens the dockview cockpit **scoped to that company** (it carries forward all the cockpit/selection/preset/pop-out work as its engine). The cockpit also stays reachable as an interim **Cockpit** sidebar entry. **Focus reader/writer modes have landed**: a reusable full-screen, distraction-free `FocusOverlay` (Zen-mode, `Esc` to exit) — the report-over-report diff opens in a **Focus reader**, and a notebook note opens in a **Focus writer** — invoked from their surfaces. **Compare** still renders a minimal mode home (cross-company KPI table is v0.53); per-company conviction status shows a neutral placeholder until its step. Watchlists/Research/Notebooks/Events/Report-season remain valid sections (deep links) as they fold into modes.

Developer-only section:

- Diagnostics, visible only when Developer mode is active

Shell behavior:

- Dark theme is the first-run default.
- The top toolbar and navigation bar stay fixed while the current workspace scrolls.
- The app shell owns the viewport height; screens should use internal panel/list scroll areas instead of relying on page/body scrolling.
- The browser page should not expose a global application scrollbar. The top toolbar, the navigation bar, and each screen's primary header or control bar should remain visible while long lists, detail panes, or subpanels scroll internally.
- The Inbox workspace splits the feed list and detail pane 50/50 by default, **side by side (horizontal)**; the divider is draggable between 25% and 75% of the row. The feed list must remain the dominant flexible scroll region — the feed pane carries heavy fixed chrome (tabs, stats, the filter toolbar, the cleanup footer), so do not stack the panes vertically without first collapsing that chrome (see [ADR 0047](adr/0047-top-navigation-bar.md), "Rejected alternative").
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

### Research workspace shell (mode-based, thesis-centric) — direction

The shell is mode-based ([ADR 0054](adr/0054-mode-based-thesis-centric-shell.md), Accepted): a **left-sidebar IA spine + pinned companies**, a **Today/Pulse Triage home**, a **sectioned Company workspace**, **Compare**, and **Focus** modes, with a **glanceable per-company conviction status** (a composite rolled up from a fixed check set, decomposed into a few named factors, plus a watchlist rollup). The organizing unit is the **company + its thesis/conviction state**; the feed is *input* that moves that state. A cited UX research pass (terminals + retail-research apps + IDE/PKM) grounds this — see [ADR 0054](adr/0054-mode-based-thesis-centric-shell.md) Evidence.

**dockview is the opt-in "advanced layout" engine** inside the Company-workspace and Compare modes (re-scoped from the app-wide grid; [ADR 0053](adr/0053-dockview-layout-pilot.md) amended by [ADR 0054](adr/0054-mode-based-thesis-centric-shell.md)). It is built behind the `src/screens/Cockpit/` adapter; framework comparison and the gradual-migration plan are in ADR 0053. Its behavior (carried forward into the advanced-layout mode):

- **Linked panels.** Panels react to a shared selection: choosing a feed item drives the Inspector and the selected company's Claims and Report-diff panels (triage → read → verify, all visible). This selection propagation is the cockpit's reason to exist — a single-screen shell cannot do it. The selection flows through a **single `CockpitSelectionContext` store** (decision 6A in [ADR 0053](adr/0053-dockview-layout-pilot.md)), consistent with the Architecture-v2 per-domain view-model contexts — not per-panel controllers.
- **Panel chrome stays accessible and primitive-first.** dockview owns only arrangement; pane content composes `src/ui` primitives (ADR 0037). Tabs are accessible (native buttons with `aria-pressed`; never `role="tab"` without a tablist parent, never an interactive control nested in another) and the panel set is fully keyboard-operable (focus, split, move, close) before the cockpit becomes the default shell.
- **Floating now; OS-window pop-out after Tauri validation** (decision 2A). In-app floating works everywhere; pop-out to a separate OS window — the answer to the tall/narrow ultrawide-quarter constraint — is gated behind a Tauri `WebviewWindow` validation sub-spike.
- **Named saved layouts** (decision 3A) let the user switch task-shaped workspaces; persisted in SQLite (`cockpit_layouts`), not `localStorage` ([data-model.md](data-model.md), [contracts.md](contracts.md)).
- **Gradual migration** (decision 1A): the cockpit is now the **default shell**; the top-nav is **slimmed** (the screens hosted as panels are dropped from it) but not yet removed — Inbox/Companies/Watchlists/Sources/Transcripts/Settings remain nav items until they too become panels, at which point ADR 0047 is fully superseded. Screens whose data loaded only while their own section was active also load while the cockpit hosts them (the section gates fire on `Cockpit` too).
- Theming is the `night-neon` token bridge; the cockpit must remain usable across the supported window-size range, including the tall/narrow ultrawide-quarter window.

## Reusable UI Foundation

Shared app UI primitives live under `src/ui`. Screens should use these primitives for recurring visual and interaction patterns before adding screen-local variants.

Initial primitive families:

- navigation: subnavigation and later sidebar-like section menus
- fields: field rows, select fields, text fields, and action rows
- buttons: standard button variants and icon-only affordances
- surfaces: panels, section headers, empty states, and repeated cards
- status: badges, pills, counters, and compact metadata chips
- lists: dense selectable row shells and row action clusters
- confirmations and disclosures: inline confirmation prompts and expandable row/detail wrappers during compatibility migration

The primitive APIs should remain Brawler-owned and semantic. If a future UI framework is adopted, screens should not be rewritten to framework-specific components directly; the framework should be wrapped behind the existing `src/ui` boundary where practical. This keeps the app extensible while allowing the current implementation to stay plain CSS and lightweight.

Migration is incremental. New screens should use shared primitives from the start. Existing screens should be moved view by view, starting with the areas that have caused repeated layout or consistency defects.

Dense list rows should share state behavior before sharing body layout. The reusable row shell owns selected, unread, disabled, hover, and focus treatment. Row body structure stays screen-owned until a specific row family proves reusable enough to migrate. This avoids hiding domain differences between feed evidence, companies, sources, transcripts, events, shortcuts, and settings forms behind one generic component.

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

## Report Season Screen

Purpose: a time-driven "what's coming" surface ([ADR 0044](adr/0044-report-season-cockpit.md)) placed next to Inbox — the report-season cockpit. It prepares the investor for report season by listing upcoming report dates across watchlists, each with a pre-report card.

Main regions:

- watchlist scope filter (all tracked companies by default)
- upcoming reports list ordered by date, with a stale-calendar indicator when the calendar is out of date
- per-company pre-report card: open research questions, unresolved claims, last-period KPIs, and recent evidence
- a past-reports section for recently passed report dates

Actions:

- mark a company prepared ahead of its report; mark processed once the report has been handled
- drill into the company workspace, its research questions, and its claims-review queue
- when a report has arrived, jump to KPI extraction for the new filing

Empty states:

- no watchlists/companies tracked: prompt to add a company
- no upcoming reports in scope: show the empty cockpit with a hint to widen the watchlist scope
- stale calendar: show the staleness indicator and link to Sources

The cockpit composes existing domains and adds no per-company data of its own except the prepared/processed workflow state; it never auto-fetches or auto-extracts (the autonomous path is the `v0.49.0` North Star).

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

The Watchlists screen is a dedicated navigation section. It should use a watchlist-first dual-pane workflow: select a watchlist, then manage that watchlist's member companies. Renaming a watchlist should preserve the watchlist's stable internal id. Removing a company from a watchlist should happen in this panel without deleting the company itself. Deleting a watchlist should require confirmation and should not delete member companies. If a deleted watchlist is active in a view filter, that filter should reset to `All`.

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
- Fundamentals
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

Claims tab (management claims tracker, [ADR 0040](adr/0040-management-claims-tracker.md)):

- first-class claims for this company (statement, due period, optional quantitative target, source evidence, verdict)
- a **review queue** ("claims to verify") at the top, bucketed due / overdue / upcoming, surfaced when the due-period report arrives
- the verdicts: pending, delivered, partially delivered, missed, revised — user-set
- for a quantitative claim, the matching confirmed financial fact shown beside the claim for in-place verdict resolution
- AI claim extraction launcher (modal) over a report document or transcript, proposing claims for mandatory confirmation
- add/edit a claim manually; claim rows expand in place under the clicked row, following the app-wide row interaction pattern

Milestone v0.42.0 makes claims a first-class entity (existing claim notes are migrated). The review queue is the primary verification surface; reminders and digests remain the cross-cutting paths. No verdict is ever assigned automatically — the queue surfaces the evidence and the user decides. Heavy extraction interaction happens in a modal, not in the fixed-width detail rail (mirroring KPI extraction).

Transcripts tab:

- transcript jobs for this company
- submit YouTube `URL`
- review transcript segments
- create note from selected segments

Global or notebook-level transcript entry starts from a required `URL` field and optional company/ticker field. If the company is omitted, the app should try to recognize it after transcription, but the transcript can remain unlinked for general market videos or any non-company-specific recording. Company selection is required only when saving selected segments into a company notebook.

Fundamentals tab:

- reporting periods list and a KPI-per-period matrix (KPI rows × period columns), newest period first
- as-reported values shown in their original scale (e.g. "1 093,6 mln PLN") with localized KPI names, not internal metric ids
- a per-KPI trend column (sparkline) plus a larger trend chart for a selected KPI, drawn with the app-owned SVG primitives (no chart dependency)
- fact detail as a readable label/value list, with edit and remove (inline-confirm) actions
- manual fact entry: inline KPI search (datalist), a reporting-period selector, value, and currency; the submit is gated on KPI + period + value
- custom per-company KPI management (create/edit `company`-scope KPI definitions) alongside the seeded `canonical`/`sector` taxonomy
- confirmed AI-extracted facts surface here through the same read model as manual facts

The Fundamentals tab is the panel half of the company-fundamentals feature; the ingestion/extraction half is launched from report feed item detail (see the AI KPI extraction flow in [UI Flows](ui-flows.md)). The matrix and charts must remain readable across the supported narrow window range; values never render as raw integers or internal ids.

The Fundamentals tab also hosts the **report-over-report diff** entry ([ADR 0052](adr/0052-report-over-report-diff.md), `v0.47.0`): a stored financial statement offers a **Compare with previous** action that opens a section-aligned diff against the prior same-type statement (SSF↔SSF, JSF↔JSF). The diff view lists aligned sections (unchanged / changed / only-in-one) with the changed-section text delta and a citation into each report; it must remain readable across the narrow window range (sections stack rather than clip). Extraction-pending and no-text-layer states are shown explicitly rather than as an empty diff. The narrative management report (MD&A) is not diffable in v0.47.0.

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
- backups section: backup status (last backup time, count), backup list (rotating backups and pre-migration snapshots), a create-backup-now action, and a restore action with explicit confirmation that warns restore is applied on app relaunch (see [ADR 0032](adr/0032-search-and-backup-boundaries.md))
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
- Database: connection-pool tuning (`maxConnections`, `busyTimeoutMs`, `acquireTimeoutMs`) with safe-range clamping, a reset-to-defaults control, and a clear "applied on next launch" note (see [ADR 0032](adr/0032-search-and-backup-boundaries.md)); backup and restore controls remain in Diagnostics
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

Global search (delivered in `v0.38.0`, see [ADR 0032](adr/0032-search-and-backup-boundaries.md)) covers:

- companies
- watchlists
- feed items
- notebook entries
- transcript segments
- company events
- research briefs
- digests

A global, keyboard-reachable search box lives in the top toolbar and queries the unified `search_index`. Results are ranked, grouped by content type, and show a snippet; selecting a result navigates to the owning screen/item. Copy is localized (en/pl).

The earlier constraint that kept search workspace-scoped is lifted now that a cross-workspace result model exists. The existing per-workspace search/filter inputs remain: Inbox owns feed-item filtering in its toolbar, Companies owns company-list search, and Notebooks owns note filtering. Global search complements, rather than replaces, those local lists.

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
