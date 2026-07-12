# UI Information Architecture

This document turns the UX flows into concrete v1 screens, regions, and actions. It should guide the first React/Tauri scaffold before detailed visual design or component implementation.

Doc map: [CLAUDE.md](../CLAUDE.md) § Required Reading. Related references: [UI Flows](ui-flows.md), [Product Spec](product-spec.md), [Contracts](contracts.md), and [ADR 0006: Theme and Visual Direction](adr/0006-theme-and-visual-direction.md).

## App Shell

V1 uses a desktop app shell with persistent navigation and work-focused density.

Regions:

The shell is **mode-based and thesis-centric** ([ADR 0054](adr/0054-mode-based-thesis-centric-shell.md), Accepted) — it leads with the investor's job, not a freeform canvas:

- top toolbar: brand, **global search / "ask anything"** (a first-class session entry point, semantic; focused with **Ctrl+F**), manual refresh, source health indicator, theme/settings access.
- **command palette (⌘K):** a **global** accelerator (v0.50 U6) — **Ctrl/⌘+K opens it from any screen**, complementing (not replacing) the visible spine. It lists app-level commands (navigation and global actions from the rebindable shortcut registry, plus jumps to saved views / pinned companies / "New view"); a screen may contribute contextual commands while mounted (the Cockpit adds its preset/layout commands, its `Switch view company:` retargeting, and its **generic** panel-type + global-panel `Open panel:` entries — no per-company enumeration; see Add-panel surface below). The Cockpit keeps its own local palette instance behind the "Add panel" / "Commands" toolbar buttons and empty-cell "Pick a panel" affordance for the cell-fill flows. Both `app.commandPalette` (⌘K) and `app.focusSearch` (Ctrl+F) are rebindable in Settings → Keyboard shortcuts.
- **left sidebar — the IA spine (implemented):** grouped into **Modes** (Today/Pulse · Companies/Company-workspace; Compare is hidden until v0.53 — see below), followed by the user's **saved named views** (composable dockview layouts, ADR 0057) as their own nav entries and a **"+ New view"** creator, a **pinned/favorite companies** group (each with a glanceable conviction status — a neutral placeholder until per-company conviction lands), a **Library** group (Inbox, Watchlists, Transcripts, Sources), and a **Utilities** group (Settings, Diagnostics — developer-gated). This is the load-bearing navigation; a blank/freeform workspace is never the entry point — there is no standalone "Cockpit" destination (ADR 0057 decision 5).
- **main area — the active mode's content** (see the modes below).
- **focus surfaces:** full-screen reader/writer modes invoked from anywhere (`Esc` back).

Modes / destinations:

- **🏠 Today / Pulse** (default / home) — the morning-review surface (journey J1): a **single prioritized "what changed" stream** (autopilot runs → claims to verify → new report disclosures → upcoming reports, one action per row, `j`/`k` navigation) beside a **narrow counters column** (autopilot / to-verify / upcoming) that doubles as a category filter. The **quiet state is the goal** — when nothing needs attention the stream shows one calm empty state. See [ADR 0076](adr/0076-ui-design-system-and-density-contracts.md) (task U-Rb).
- **🔬 Company workspace** — a single-company deep-dive in fixed, modular, progressively-disclosed sections (Overview, Fundamentals, Valuation, Quality, What-changed, Claims, Reports, Notebook, Thesis), entered by selecting a company; **dockview's free arrangement is the opt-in "advanced layout"** within it. Single-company settings (autopilot mode, IR reports URL) live inline in Fundamentals; **cross-company settings management** is a separate **master-detail surface** ("Manage settings" in the Companies screen — company multi-select + grouped settings applied to the selection, with watchlist-scope selection), the scalable home for *all* per-company settings ([ADR 0056](adr/0056-per-company-settings-surface.md); v1 ships autopilot, pinned/watchlists next).
- **⚖️ Compare** — cross-company KPI tables (v0.53). **Hidden from the sidebar until then** (U-Rc, [ADR 0076](adr/0076-ui-design-system-and-density-contracts.md) Resolved): an empty mode in nav is trust debt. The `Section` value and screen remain (deep links stay valid); the nav entry returns with market data.
- **📋 Journal** — the decision journal (v0.56).
- **📖 Focus** — distraction-free reading (a long report diff) / writing (a thesis or note).

Developer-only: Diagnostics, visible only when Developer mode is active.

Interim state (the shell is built incrementally, [ADR 0054](adr/0054-mode-based-thesis-centric-shell.md)): the **left-sidebar IA spine + pinned companies** and the **Today/Pulse attention home** have landed, and **Today/Pulse is now the default landing** (replacing the cockpit default). Today/Pulse is a **single prioritized attention stream** (ADR 0076 U-Rb, redesigned to journey J1) that merges four categories in a fixed priority order — (1) autopilot runs, (2) claims due/overdue for the **pinned** companies (a bounded set, overdue first), (3) new report disclosures ("what changed"), (4) upcoming reports — composed from existing app-wide read models. Every row carries a leading category icon, the **full ticker** (never truncated), a **type badge**, a **full date**, a title/statement wrapping to two lines, and **exactly one Review action**; `j`/`k` (and arrows) move a roving focus across rows, Enter triggers Review. A narrow **counters column** (autopilot / to-verify / upcoming) shows the live counts and each tile toggles a single-category filter on the stream. The to-verify and what-changed categories cap at 8 rows (upcoming at 6); a capped category ends with a "Show all" link into its full surface (Claims / Inbox). When categories 1–3 are all empty the stream shows one calm empty state (the quiet state is the goal), followed by any upcoming rows; a small **Open Inbox** header action stays on the stream. Autopilot rows keep the autonomous-pipeline controls behind an in-place **expandable detail** (chevron): **Dismiss**, the **Structure changed** drift block when structured extraction drifted, and — for an `autopilot`-mode run that produced facts — **Undo** (two-step confirm) to revert exactly the facts that run auto-committed, replacing the action with a "Reverted N facts" badge ([ADR 0055](adr/0055-autonomous-report-pipeline-trust-ladder.md) §4; `assist`-mode facts land `pending` and go through the existing confirm/reject review instead, so they get no Undo). The full **accept/snooze** triage state (beyond dismiss) is owned by the v0.48 feed-triage epic and plugs into this home. Opening a company lands on its **curated cockpit dashboard** ([ADR 0057](adr/0057-composable-views-and-curated-dashboard.md) — the tabbed Company workspace is retired); the ADR 0054 sectioned deep-dive with dockview as an opt-in "Advanced layout" remains future shell work (tracked on the cockpit epic). The cockpit is reached via a saved named view (its own sidebar entry), the **"+ New view"** creator, or a company's curated dashboard — there is no separate blank-canvas **Cockpit** sidebar entry ([ADR 0057](adr/0057-composable-views-and-curated-dashboard.md) decision 5). **Focus reader/writer modes have landed**: a reusable full-screen, distraction-free `FocusOverlay` (Zen-mode, `Esc` to exit) — the report-over-report diff opens in a **Focus reader**, and a notebook note opens in a **Focus writer** — invoked from their surfaces. **Compare** is hidden from the sidebar until v0.53 (U-Rc; the screen itself remains reachable programmatically); per-company conviction status shows a neutral placeholder until its step. Watchlists/Research/Notebooks/Events/Report-season remain valid sections (deep links) as they fold into modes.

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

### Research cockpit shell (mode-based, thesis-centric)

The shell is mode-based ([ADR 0054](adr/0054-mode-based-thesis-centric-shell.md), Accepted): a **left-sidebar IA spine + pinned companies**, a **Today/Pulse Triage home**, a **sectioned Company workspace**, **Compare**, and **Focus** modes, with a **glanceable per-company conviction status** (a composite rolled up from a fixed check set, decomposed into a few named factors, plus a watchlist rollup). The organizing unit is the **company + its thesis/conviction state**; the feed is *input* that moves that state. A cited UX research pass (terminals + retail-research apps + IDE/PKM) grounds this — see [ADR 0054](adr/0054-mode-based-thesis-centric-shell.md) Evidence. Per-panel content/behavior for the company deep-dive is specified in [Company Cockpit Dashboard Panels](#company-cockpit-dashboard-panels); this subsection is the cockpit **engine** mechanics only.

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

Purpose: the company **library + management** surface ([ADR 0057](adr/0057-composable-views-and-curated-dashboard.md)). Browse/search/add tracked companies and manage per-company settings; opening a company lands the **curated cockpit dashboard** (the deep-dive lives there, not in a tabbed panel inside this screen).

Main regions:

- company search/add control
- company list searchable and filtered by watchlist/exchange
- a `Manage settings` toggle that swaps the list for the per-company settings surface ([ADR 0056](adr/0056-per-company-settings-surface.md))

Actions:

- add company by exchange-qualified ticker
- open the company's cockpit dashboard (row click or keyboard)
- delete a tracked company
- manage per-company settings (autopilot, …) via the settings surface

Company rows should show current watchlist memberships for scanning, but membership editing belongs in the dedicated Watchlists menu panel. The company list should not show watchlist create, delete, add, or remove controls.

The company list should own its vertical scrolling so the company add/search/filter controls remain visible while reviewing long tracked-company lists.

Company metadata detail (display name, exchange, ticker, ISIN, CIK, LEI, aliases, source-specific IDs) is shown from the company row/settings surface here, not as a cockpit dashboard panel.

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

Clicking a company row (or pressing Enter/Space on it) opens the **curated cockpit dashboard** scoped to that company ([ADR 0057](adr/0057-composable-views-and-curated-dashboard.md)). Up and Down arrows move the highlighted row within the library without navigating away. The historical inline-expanding tabbed workspace (Milestone 3 / [ADR 0054](adr/0054-mode-based-thesis-centric-shell.md)) has been **retired** in favor of the dashboard.

## Company Cockpit Dashboard Panels

Opening a company from the Companies library lands the **curated cockpit dashboard** ([ADR 0057](adr/0057-composable-views-and-curated-dashboard.md)): a seeded `cockpit_layout` scoped to the company, opening with a calm default panel set (Fundamentals, Coverage, Feed, Claims, Quality, Report documents, Notebook) that stays composable (add/remove/move panels, then `Save dashboard` to persist per company). Panel keys: Feed → `companyFeed`, Notebook → `companyNotebook`, Coverage → `coverage`, Review queue → `review`, Decision journal → `decisionJournal` (palette-only, not a default), Fundamentals/Claims/Quality/Report documents/diff as their own panels. Transcripts remains a future-milestone placeholder; company metadata lives in the Companies library (see there), not a panel.

**Add-panel surface** (card 106f8a7): the cockpit `Add panel` button and the ⌘K palette's `Open panel:` entries list **generic panel types**, never one entry per tracked company. A **company-scoped type** (Fundamentals, Coverage, Report documents, Quality, Claims, Notebook, Feed, Review queue, Report comparison, Decision journal) opens as a **follow** panel bound to the **current view company** and retargets in place when the view company changes (U-Ra, [ADR 0076](adr/0076-ui-design-system-and-density-contracts.md)); these types are offered once a view company is chosen. A separate group lists the **global panels** (Report Season, Research, Watchlists, Events, Notebook, Journal (all companies)) — app-wide singletons, not company-scoped. Retargeting the whole view to another company stays on the header **View company** selector and the palette's `Switch view company:` entries; freezing one panel to a specific company is the panel tab's **pin** toggle. There is no per-company panel entry in the add flow (the pre-106f8a7 palette enumerated every company × every kind, offering "OTHER companies'" panels the view-company concept already governs).

Header shows: qualified ticker, display name, exchange, watchlist memberships as read-only context, last feed update, feed/unread/saved counts, and quick actions (refresh company, add note, add transcript).

**Feed panel:** company-filtered feed list with the same feed item detail behavior as Inbox and create-note-from-item. Clicking (or Enter/Space on) a feed row opens its detail inline under that row, collapsing on repeat; Up/Down move through rows while preserving expansion state. Company feed rows share Inbox's unread dot and read/unread typography. The inline detail shows source, type, timestamps, attribution, language, summary, source URL, read/save actions, an action to open the item in the Inbox with the company filter applied, and an action to create an editable note draft with feed-item origin. An empty feed shows an inline empty state with an `Open filtered Inbox` action rather than a blank panel.

**Notebook panel:** notes list newest first, filterable by tag, kind, claim status, follow-up quarter, follow-up date, with a note detail/editor pane and a compact manual Markdown note form (title, body, tags, note kind, optional claim status, event date, follow-up quarter, follow-up date); feed-item note drafts share the same form. The list stays dense enough for dozens of notes per company: title/kind/tags/status/follow-up cues only, no raw body preview. Read mode renders common Markdown structure locally; edit mode exposes the raw Markdown body. Date fields use the native date picker; follow-up-quarter fields use a small quarter picker with a `Today` shortcut.

**Claims panel** ([ADR 0040](adr/0040-management-claims-tracker.md)): first-class claims for the company (statement, due period, optional quantitative target, source evidence, verdict) with a **review queue** ("claims to verify") at the top, bucketed due/overdue/upcoming, surfaced when the due-period report arrives. Verdicts (pending, delivered, partially delivered, missed, revised) are always user-set — the queue surfaces evidence, never assigns a verdict automatically. For a quantitative claim, the matching confirmed financial fact shows beside the claim for in-place resolution. An AI claim-extraction launcher (modal) proposes claims from a report document or transcript for mandatory confirmation; claims can also be added/edited manually, with rows expanding in place under the clicked row (the app-wide row interaction pattern). Heavy extraction interaction happens in the modal, not the fixed-width detail rail.

**Transcripts panel** (future milestone): transcript jobs for the company, submit YouTube `URL`, review transcript segments, create note from selected segments. Global/notebook-level transcript entry starts from a required `URL` field and optional company/ticker field; if the company is omitted the app tries to recognize it after transcription, but the transcript can stay unlinked for general market videos. Company selection is required only when saving selected segments into a company notebook.

**Fundamentals panel:** reporting periods list and a KPI-per-period matrix (KPI rows × period columns), newest period first; as-reported values in their original scale (e.g. "1 093,6 mln PLN") with localized KPI names, never internal metric ids; a per-KPI trend sparkline plus a larger trend chart for a selected KPI (app-owned SVG primitives, no chart dependency); fact detail as a readable label/value list with edit and remove (inline-confirm) actions; manual fact entry (inline KPI search/datalist, reporting-period selector, value, currency, gated on KPI + period + value); custom per-company KPI management alongside the seeded `canonical`/`sector` taxonomy; confirmed AI-extracted facts surface through the same read model as manual facts. Ingestion/extraction is launched from report feed item detail (see [UI Flows](ui-flows.md)); the matrix/charts stay readable across the supported narrow window range.

The Fundamentals panel also hosts the **report-over-report diff** entry ([ADR 0052](adr/0052-report-over-report-diff.md)): a stored financial statement offers **Compare with previous**, opening a section-aligned diff against the prior same-type statement (SSF↔SSF, JSF↔JSF) — aligned sections (unchanged/changed/only-in-one) with the changed-section text delta and a citation into each report, readable across the narrow window range (sections stack rather than clip). Extraction-pending and no-text-layer states are shown explicitly rather than as an empty diff; the narrative management report (MD&A) is not diffable.

**Coverage panel** ([ADR 0077](adr/0077-trusted-extraction-foundations.md) §2): a fundamentals **coverage map** — a period × axis table (Period / Report / Data / To review), one row per fiscal period, newest first. The row set follows the **period-union rule**: a period appears iff a canonical periodic report, at least one extracted fact, or at least one pending review proposal names it — so a period known only from facts (aggregator/witness) or only from a report still shows, and gaps are never silently absent. The Report cell shows the canonical report's kind chip (+ an ESEF chip for a structured document) or an explicit "No report / not found in backfill"; the Data cell splits the period's facts into validated / flagged-or-unvalidated (success vs warning tone), or "not processed → Extract" when a fetched report has no facts yet, or "link-only — no stored file" when the canonical report is metadata-only (no file to extract, aligned with the sweep, which skips these); the To-review cell counts pending proposals and, when > 0, is its **own click target** that opens the company's **Review queue** pane (T5.3b). The `skippedBudget` state ("Skipped — AI budget") is rendered but always `false` in v1 — it surfaces once the AI-budget substrate lands and must never drop a period silently. Clicking a row (outside the To-review cell) opens the company's **Report documents** pane. The panel's **history-actions footer** (T3.2, [ADR 0077](adr/0077-trusted-extraction-foundations.md) §3) holds two actions in fixed slots plus a status line: **Backfill history** (primary — fetch recent reports; the backend auto-chains a history sweep so the fetched periods are extracted) and **Extract missing periods** (secondary — run the sweep only, for documents already stored). The status line names the phase (backfilling → sweep `queued`/`running`/`completed (N runs, M skipped)`), showing a live drain counter "Extracting… {done}/{total}" while runs settle; a company with automation **off** disables both actions with an explicit "automation off" hint rather than a silent no-op. A backfill that hit its page cap before the configured depth adds an explicit **truncation warning** ("older filings may be missing"). The status line also echoes the latest sweep's **AI-call spend** (T5.3) whenever a sweep row exists — "AI: {used}/{limit}", or "AI: {used} (no limit)" when the budget is `0`; the budget itself is configurable in Settings → AI.

**Review queue panel** ([ADR 0077](adr/0077-trusted-extraction-foundations.md) §4/§5, T5.3b, mockup F5 `docs/mockups/f5-review-queue.html`): the company's pending KPI proposals awaiting human review, from the `list_pending_kpi_proposals` read model, **grouped by detected period** (newest first). Each row shows the proposed metric + value and a citation line (source snippet + source document), with a **source chip in a fixed trailing slot** — `OCR bootstrap` (accent; value parsed from an unconfirmed OCR profile, confirming it confirms the profile), `OCR · flagged` (warn; deterministic parse the identity validation flagged), or `AI` (an older text-AI proposal) — followed by fixed **Confirm** / **Reject** actions. Confirm runs the real validation ([ADR 0077](adr/0077-trusted-extraction-foundations.md) §4) and surfaces a `flagged` outcome as a non-blocking caution; Reject discards the proposal. Both refetch the queue and reload sibling panels (the Coverage map's counts, Fundamentals). Empty state: "No proposals to review — deterministic extractions write facts directly." **Entry points:** the Coverage map's To-review cell and the cockpit panel palette (kind `review`). Panel key: `review`.

**Decision journal panel** ([ADR 0071](adr/0071-judgment-capture.md), panel key `decisionJournal`): the company's chronological record of recorded judgments (`buy` / `pass` / `keep_watching` / `sell_note`) with a Markdown rationale and a decided-on date, newest by **decided_at** (never insertion order). A compact composer (decision kind, decided-on date, Markdown rationale) records an entry; the list∥detail body shows the selected entry's kind, date, and rendered rationale plus an evidence picker that links company-timeline items (feed items, notes, claims, events) to the decision, reusing the research `EvidenceRow` link pattern (`fromType: "decision_entry"`). **Entries are immutable** ([ADR 0071](adr/0071-judgment-capture.md)): there is no edit or delete affordance anywhere — a correction is a follow-up entry created via **Supersede** (linked by `supersededByEntryId`, marked with a "Follow-up" chip). Entries join the company research timeline. **Not in the curated dashboard defaults** — an occasional-entry surface reached via the panel palette / Add panel. Its cross-company sibling is the **global decision journal** (palette label "Journal (all companies)", global panel `decisionJournalGlobal`): a read-only chronological list across all companies with filters by decision kind and company — the calibration-loop review surface.

**Report documents panel** ([ADR 0077](adr/0077-trusted-extraction-foundations.md) §2, mockup Panel B): the stored ESPI/EBI attachments and user report links for the company, read from the `get_report_documents_view` read model. By default it **groups documents by fiscal period**, newest first, with a **"Group by period" toggle** back to a flat list. Each group header names the period, the report cadence (annual / half-year / quarterly), and the document count. Within a group the **periodic statements** come first — a **★ marks the period's canonical report** (the same document the Coverage map names) — then audit reports, then a **fold** hiding the signature/data companions (`.xades` signatures, `.xbri`/`.xbrl` data, selected-data extracts); a companion whose **"Extract data" action is available is never folded**, so an actionable row can't hide. Non-periodic filings (announcements, GM materials, other) collect in a separate **"No period" group, collapsed by default** behind a "show all (N)" disclosure. Each row leads with the **document-kind label** (kind chip + an ESEF chip for structured filings), demoting the raw filename to a muted second line whose link keeps the full original filename in its tooltip; the trailing edge holds a fixed storage-status chip slot ("Stored" / "Link only") and a reserved "Extract data" action slot. A **search field** filters across title/filename, and the kind filter + "Refresh classification" action stay. Density (ADR 0076 D6): the kind label + filename + date show at every tier; the kind/status chips appear from M, the extract action gains its label at L, and a short/narrow pane drops the chips + action.

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

### Investor week view (layers)

The Events screen also offers a **weekly working-day view** (Mon–Fri columns; a weekend column only when populated) — the investor week calendar ([ADR 0058](adr/0058-investor-week-calendar.md), `v0.59.0`), inspired by the Koomberg weekly digest. It composes opt-in **layers** over the same data, with our own UI:

- **Scope toggle** — watchlist (default) ↔ **whole market** (untracked GPW tickers via the opt-in relaxed Bankier ingest).
- **Layer toggles** — company events (reports, `DEBIUT`/IPO debut, `ODCIĘCIE DYWIDENDY`/ex-dividend), **macro** (CPI/PMI/payrolls with time + country flag; manual + sample now, live source later), and **market holidays** (per-market `WOLNE` badge on closed days).
- Each layer renders as a lane within a day column; lanes and columns must stack/degrade in tall-narrow windows. Scope and enabled layers persist in settings.

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
- Sources: polling interval, **backfill history depth** (clickable presets 1/3/5/10 years bound to a slider + numeric input, clamped 1–10, default 3; ADR 0077 §3), feed cleanup status, import/export status
- AI providers: Gemini configuration for YouTube transcription, selectable transcription model, credential configured/not-configured status, credential storage, secret kind, future general AI provider slots, and the **history-sweep AI call budget** (clickable presets 0/10/30/100 bound to a slider + numeric input, clamped 0–500, default 30, 0 = unlimited; ADR 0077 §6)
- Credentials: credential configured/not-configured status, credential storage, secret kind, save/replace/clear controls
- Keyboard shortcuts: discoverable action list, configurable bindings, conflict visibility, disable, and reset controls
- Logs: local runtime log level and rotation limits, with a clear local-only/no-telemetry explanation
- Database: connection-pool tuning (`maxConnections`, `busyTimeoutMs`, `acquireTimeoutMs`) with safe-range clamping, a reset-to-defaults control, and a clear "applied on next launch" note (see [ADR 0032](adr/0032-search-and-backup-boundaries.md)); backup and restore controls remain in Diagnostics
- Import and Export: export/import research data, export/import safe preferences, preview import changes before applying them
- MCP server ([ADR 0078](adr/0078-mcp-external-surface.md)): enable toggle with a live Active/Stopped status pill (refusal reasons — missing token, port in use — surface inline), listen port (commit-on-blur, clamped 1024–65535, applies on next start), access-token lifecycle (generate with a one-time copyable reveal, revoke behind an inline confirm, configured/storage status), and copyable example connection snippets for Claude Code (HTTP) and the stdio adapter
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

## Research Workspace

The Research screen (company/watchlist evidence timeline, review checkpoints, questions, reminders, briefs/digests) shipped through `v0.31.0`; its live behavior is governed by [ADR 0022](adr/0022-research-evidence-read-model-boundary.md) and specified in [Contracts § Research Evidence Boundary](contracts.md#research-evidence-boundary) and [Data Model § Research Evidence Boundary](data-model.md#research-evidence-boundary). Delivery chronicle (M25/M26/M29/M31) moved to [Kanban Archive](kanban-archive.md#archived-investigation-and-study-notes-moved-2026-07-02). No further post-v1 direction is tracked for this screen beyond the roadmap items already covering research-adjacent epics (see [Roadmap](roadmap.md)).

## Responsive Behavior

The primary target is desktop. The first implementation should still avoid layouts that break at narrow widths:

- sidebar may collapse
- detail pane may become an overlay
- dense tables/lists should preserve readable text
- icon buttons need tooltips when meaning is not obvious

## Deferred UI

No UI is built for anything in [Roadmap § Not In V1](roadmap.md#not-in-v1) (portfolio position tracking, trade journal, billing/hosted-licensing infrastructure, cloud sync, multi-user/team/sharing workflows).
