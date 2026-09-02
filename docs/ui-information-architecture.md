# UI Information Architecture

This document turns the UX flows into concrete v1 screens, regions, and actions. It should guide the first React/Tauri scaffold before detailed visual design or component implementation.

Doc map: [CLAUDE.md](../CLAUDE.md) § Required Reading. Related references: [UI Flows](ui-flows.md), [Product Spec](product-spec.md), [Contracts](contracts.md), and [ADR 0006: Theme and Visual Direction](adr/0006-theme-and-visual-direction.md).

## App Shell

V1 uses a desktop app shell with persistent navigation and work-focused density.

Regions:

The shell is **mode-based and thesis-centric** ([ADR 0054](adr/0054-mode-based-thesis-centric-shell.md), Accepted) — it leads with the investor's job, not a freeform canvas:

- top toolbar: brand, **global search / "ask anything"** (a first-class session entry point, semantic; focused with **Ctrl+F**), manual refresh, source health indicator, theme/settings access.
- **command palette (⌘K):** a **global** accelerator (v0.50 U6) — **Ctrl/⌘+K opens it from any screen**, complementing (not replacing) the visible spine. It lists app-level commands (navigation and global actions from the rebindable shortcut registry, **`Open company: TICKER` for every tracked company**, and **`Open screen: X`** entries for Events, Report Season and Research, which keep this palette entry alongside their Library nav item (F4b S4 / F4c S3, [ADR 0054](adr/0054-mode-based-thesis-centric-shell.md) amendment — the J4 quick-open path)); while the Spółka screen is active it also contributes contextual `Open <tool>` entries for its workshop tools. Every command carries `{actionKey, verb}` (ADR 0104 dec. 3) and a copy gate (`src/app/paletteCopy.test.ts`) enforces verb-first labels, both locales. Both `app.commandPalette` (⌘K) and `app.focusSearch` (Ctrl+F) are rebindable in Settings → Keyboard shortcuts.
- **left sidebar — the IA spine ([ADR 0107](adr/0107-company-view-paradigm.md) amendment, F3a S3):** grouped into **Modes** (Dziś, Inbox, **Spółka** — Section `Spolka` — opens the last-viewed company, else the first pinned, else the first tracked company; never a blank screen), a **pinned/favorite companies** group (each with a glanceable conviction status — a neutral placeholder until per-company conviction lands), a **Library** group (Companies, Watchlists, Alerts, Events, Report Season, Research, Transcripts, Sources — F4b S4 added Events and Report Season, F4c S3 added Research with Ctrl+4 (#94); [ADR 0054](adr/0054-mode-based-thesis-centric-shell.md) amendment), and a **Utilities** group (Settings, Diagnostics — developer-gated). This is the load-bearing navigation; a blank/freeform workspace is never the entry point. Exactly one sidebar row carries `aria-current="page"` at any time (a pinned+selected company's own row wins over the Spółka mode item).
- **main area — the active mode's content** (see the modes below).
- **focus surfaces:** full-screen reader/writer modes invoked from anywhere (`Esc` back).

Modes / destinations:

- **🏠 Today / Dziś** (default / home) — the morning-review surface (journey J1, **Dziś v2** — F2/#422, epic #410): a **per-day decision queue** anchored to the user's last visit. A **delta header** leads ("what arrived since your last visit" + the screen's ONE filled CTA — the most urgent item's action); below it day sections newest-first (TODAY carries the calendar announcements; counters live in the day headers — no stat tiles), each row typed (report/filing · media clusters per company · `NIE WPŁYNĄŁ` non-arrival · claims to verify · autopilot runs · attention events, root-fed) with a quiet action **naming its destination and landing on the item itself** (`j`/`k` roving over row actions). A reviewed day (manual `Oznacz dzień jako przejrzany`, undoable, or every row read/seen) collapses to one line; days beyond the two freshest fold into a **"Wcześniej" rollup**; the **quiet state is the goal** — a clean morning says so in three beats. The **Archive** of dismissed attention events survives as a quiet footer toggle (dismiss is an acknowledgement, never a delete), as does the **config-state banner**. See [ADR 0068](adr/0068-attention-routing-and-morning-briefing.md) amendment 2026-08-20 (the delta header replaced the briefing strip), [ADR 0087](adr/0087-today-attention-home-v2.md) (typed severity + aging — its stream/grouping surface is superseded by Dziś v2), [ADR 0097](adr/0097-toasts-are-action-feedback-only.md) (ambient awareness = the sidebar Today badge).
- **🔬 Company workspace ([ADR 0107](adr/0107-company-view-paradigm.md), F3a #429)** — selecting a company opens the engine-free **Spółka** screen: glance bar (identity + signals/claims/shorts/events counters with typed drill targets), co-visible core (annual KPI table with provenance tickets, company feed, log-axis daily candles, report coverage, recommendations), bottom workshop bar opening one of 15 typed tools in place of the core (F3a S1–S2, #429). Single-company settings (autopilot mode, IR reports URL): autopilot mode lives in the Fundamentals tool (`Otwórz fundamenty`); the IR reports URL and the sector override live in the Basic info panel's edit mode (hosted by the Spółka `akcjonariat` / `Otwórz akcjonariat` tool); **cross-company settings management** is a separate **master-detail surface** ("Manage settings" in the Companies screen — company multi-select + grouped settings applied to the selection, with watchlist-scope selection), the scalable home for *all* per-company settings ([ADR 0056](adr/0056-per-company-settings-surface.md); v1 ships autopilot, pinned/watchlists next).
- **Compare (Porównaj)** — removed 2026-08-10 (#351, [ADR 0089](adr/0089-cross-company-comparison-and-valuation-l1.md) amendment): the cross-company screen saw no real use; the single-company periods × deltas table stays in the Spółka `Otwórz fundamenty` tool (Fundamentals panel) and the comparison/valuation read models stay agent-reachable over MCP.
- **📖 Focus** — distraction-free reading (a long report diff) / writing (a thesis or note).

Developer-only: Diagnostics, visible only when Developer mode is active.

Landed shell state ([ADR 0054](adr/0054-mode-based-thesis-centric-shell.md), [ADR 0107](adr/0107-company-view-paradigm.md)): the **left-sidebar IA spine + pinned companies**, **Today/Pulse as the default landing**, and the **Spółka** company surface (selecting a company lands there). Today/Pulse is a **grouped, severity-ranked attention stream** ([ADR 0087](adr/0087-today-attention-home-v2.md), redesigned to journey J1 over ADR 0076 U-Rb) produced by a deterministic **dedup → group → rank** pipeline (a pure model, `streamModel.ts`): repeats of one (company, category, evidence) collapse, same category+company items collapse into a **group row with a ×N count** (members newest-first), the rows sort by **typed severity** (urgent → notable → routine) then recency, and — as a second stage — more than three same-category **routine** rows across different companies collapse into one cross-company **"×K spółek" aggregate** (the owner's real wall of 28 routine autopilot runs reads as a single line; urgent/notable rows are never aggregated). It merges five sources — autopilot runs, **fired alerts**, claims due/overdue for the **pinned** companies (a bounded set, overdue first), new report disclosures ("what changed"), and upcoming reports — composed from existing app-wide read models. **Severity is typed, mapped once in the backend from `trigger_type` + signal category** (ADR 0087 dec. 2); the frontend never infers importance from strings — it routes on the `severity` field carried on the attention/autopilot payloads (fired alerts and autopilot runs each supply their own; claims/what-changed/upcoming carry no backend severity and stay `routine`). Every row carries a **severity left-border cue** (red/amber/none), a leading category icon, the **full ticker** (never truncated), a **severity chip** (only when non-routine), a **type badge**, a **×N group chip** when it heads a group, a **full date**, a title wrapping to two lines, and **exactly one Review action** (per-row `data-ux-primary-action`); routine rows are visually dimmed. Group rows **expand in place** (chevron) to their member rows, each a compact row keeping its own single Review; `j`/`k` (and arrows) move a roving focus across rows **and expanded members**, Enter triggers the focused Review. **Dogfooding polish (`v0.60` D7):** the per-company group **×N count chip carries its unit** so "×4" is not opaque — pluralized *zdarzenia* for attention groups, *runy* for autopilot groups (en *events* / *runs*), matching the cross-company *×K spółek* chip; a stored **document title that leads with a filename** (a glued `filename.xhtml`+title, or a raw filename alone) **splits** so the human statement is the row title and the **filename drops to a quiet secondary link line** (metadata, not prose — a filename-only title falls back to the generic statement), the link opening the company's report surface (no per-document open affordance exists yet); a small **info toggle beside the Pilne tile opens a severity legend** popover (the three levels + the 3-day aging rule, mirroring the product-spec taxonomy) so PILNE/UWAGA are self-explaining; and an event raised by the **user's own alert rule** (non-null `ruleId`) carries a **quiet bell-glyph indicator** ("Z reguły alertu" / "From your alert rule") distinguishing it from system events — on a collapsed group/aggregate head only when **every** member is rule-fired, expanded members always showing their own. A narrow **counters column** shows four live tiles — **Pilne** (urgent rows) first, then autopilot / to-verify / upcoming — and each tile toggles a filter (Pilne → urgent rows only; the others → their category). The to-verify and what-changed categories cap at 8 rows (upcoming at 6) before grouping; a capped category ends with a "Show all" link into its full surface (Claims / Inbox / the Report-season screen for upcoming, carrying the full count). A **config-state banner** ([ADR 0087](adr/0087-today-attention-home-v2.md) dec. 5) sits above the stream for a condition that is a property of the app (a source unreachable, a provider misconfigured), stated once and dismissible, never repeated per row. It is wired to **source health**: any enabled source adapter in the `attention` state (its last refresh errored) raises one banner ("Source *name* isn't responding — signals may be delayed") whose **Sources** action opens the Sources surface (F4c S4 — the action names its destination). Each category's fetch failure shows a **per-category inline error strip** (a typed, translated message — never a raw `.message`) with a **Try again** retry that refetches only that category; a failed category is always visibly errored — the **quiet state cannot render while any category errored** (no false-quiet, ADR 0081 Q9). When nothing needs attention the stream shows one calm empty state (the quiet state is the goal), followed by any upcoming rows; a small **Open Inbox** header action stays on the stream. Autopilot rows keep the autonomous-pipeline controls behind an in-place **expandable detail**: **Dismiss**, the **Structure changed** drift block when structured extraction drifted, and — for a run that produced facts — **Undo** (two-step confirm) reverting exactly that run's auto-committed facts, replacing the action with a "Reverted N facts" badge ([ADR 0055](adr/0055-autonomous-report-pipeline-trust-ladder.md) §4 as amended by [ADR 0086](adr/0086-aggregator-primary-fundamentals.md) — facts are review-free in every mode). The full **accept/snooze** triage state (beyond dismiss) is owned by the v0.48 feed-triage epic and plugs into this home. **Fired alerts** ([ADR 0068](adr/0068-attention-routing-and-morning-briefing.md) T4) render in the same stream, grouped by company like every other category, each row showing the rule's trigger context (signal category / autopilot-run-completed / price range / 52-week low) and its fired date, with the same one-Review anatomy (Review marks it seen and jumps to its evidence — a signal's company Feed, a reconciliation's **missed report itself** opened in the system browser via its stored witness URL ([ADR 0097](adr/0097-toasts-are-action-feedback-only.md) dec. 8; a legacy row without a URL falls back to the company Feed), an autopilot run's or a daily quote's company Fundamentals) plus an explicit Dismiss. **No attention event raises a toast** ([ADR 0097](adr/0097-toasts-are-action-feedback-only.md), superseding the ADR 0087 dec. 3 toast leg): ambient awareness is the **sidebar Today badge** — the count of unseen non-routine events, cleared by visiting Today (the stream batch-marks loaded events seen: *seen = was on screen*), plus one polite coalesced screen-reader announcement on count increases after hydration. A compact **morning-briefing strip** ([ADR 0087](adr/0087-today-attention-home-v2.md) dec. 5 mockup amendment, replacing the card wall) sits above the stream: one line with a label, the composed-at ("since") timestamp, and a grouped counts summary, expanding **in place** to the deterministic structured item list ([ADR 0084](adr/0084-retire-in-app-ai-layer.md) — the AI narrative is retired; each item click-throughs to its evidence via the same routing the stream uses) with a secondary **Generate** action that enqueues a fresh compose and polls for the result; a daily auto-trigger composes one automatically while the app is open. **Two views (owner 2026-07-23):** a compact **Aktywne | Archiwum** segmented switch (`SegmentedControl`, `aria-pressed` per option) in the Today header toggles the live stream against a **read-only Archive** — DISMISSED attention events only (subtitle "Odrzucone zdarzenia uwagi"), newest-first, the same row anatomy (severity chip / ticker / title / alert-origin bell) with Review as pure navigation and **no Dismiss control** (already dismissed); dismiss stays an **acknowledgement, never a delete**, so nothing is lost. In the Archive state the counters column, config banner, and briefing strip (all active-stream concerns) are hidden, tiles keep counting only the active stream, and an empty archive shows a quiet "Archiwum jest puste." Archived events load **lazily** on first switch (`list_attention_events` with `includeDismissed`). The archive is the **attention-event** archive only for v0.60 (autopilot runs and other categories are out of scope); **restore/un-dismiss is deliberately not built** (a follow-up candidate). Opening a company lands on the **Spółka** screen ([ADR 0107](adr/0107-company-view-paradigm.md)). **Focus reader/writer modes have landed**: a reusable full-screen, distraction-free `FocusOverlay` (Zen-mode, `Esc` to exit) — the report-over-report diff opens in a **Focus reader**, and a notebook note opens in a **Focus writer** — invoked from their surfaces. The Compare Modes destination was removed 2026-08-10 (#351); per-company conviction status shows a neutral placeholder until its step. Watchlists remains a valid deep-link section as it folds into modes; Events, Report Season and Research are Library nav destinations ([ADR 0054](adr/0054-mode-based-thesis-centric-shell.md) amendment) reachable the same way besides their sidebar item.

Developer-only section:

- Diagnostics, visible only when Developer mode is active

Shell behavior:

- Dark theme is the first-run default.
- The top toolbar and navigation bar stay fixed while the current workspace scrolls.
- The app shell owns the viewport height; screens should use internal panel/list scroll areas instead of relying on page/body scrolling.
- The browser page should not expose a global application scrollbar. The top toolbar, the navigation bar, and each screen's primary header or control bar should remain visible while long lists, detail panes, or subpanels scroll internally.
- The Inbox workspace splits the feed list and detail pane 50/50 by default, **side by side (horizontal)**; the divider is draggable between 25% and 75% of the row. The feed list must remain the dominant flexible scroll region — the feed pane carries heavy fixed chrome (tabs, stats, the filter toolbar), so do not stack the panes vertically without first collapsing that chrome (see [ADR 0047](adr/0047-top-navigation-bar.md), "Rejected alternative").
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

The docking-engine shell (dockview, named views, the frozen legacy dashboards) was retired 2026-08-28 ([ADR 0108](adr/0108-retire-docking-engine.md)); the current shell is the mode-based spine and the engine-free **Spółka** company surface described above (§ App Shell).

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
- company context (latest facts, upcoming events, notebook, claims due) — secondary to the item's own body, collapsed behind a disclosure with a one-line teaser by default, expanded on demand (owner dogfooding finding, 2026-08-27; same collapse for every `presentationKind`)

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

Purpose: the company **library + management** surface ([ADR 0057](adr/0057-composable-views-and-curated-dashboard.md)). Browse/search/add tracked companies and manage per-company settings; opening a company lands the **Spółka workshop** ([ADR 0107](adr/0107-company-view-paradigm.md)) — the deep-dive lives there, not in a tabbed panel inside this screen.

Main regions:

- company search/add control
- company list searchable and filtered by watchlist/exchange
- a `Manage settings` toggle that swaps the list for the per-company settings surface ([ADR 0056](adr/0056-per-company-settings-surface.md))

Actions:

- add company by exchange-qualified ticker
- open the company's Spółka workshop (row click or keyboard)
- delete a tracked company
- manage per-company settings (autopilot, …) via the settings surface

Company rows should show current watchlist memberships for scanning, but membership editing belongs in the dedicated Watchlists menu panel. The company list should not show watchlist create, delete, add, or remove controls.

The company list should own its vertical scrolling so the company add/search/filter controls remain visible while reviewing long tracked-company lists.

Company metadata detail (display name, exchange, ticker, ISIN, CIK, LEI, aliases, source-specific IDs) is shown from the company row/settings surface here, not as a Spółka panel.

## Watchlists Screen

Purpose: groups of companies that Today, Inbox and Report Season filter by — the user's own curation ([F4a, ADR 0104](adr/0104-frontend-v2-design-language.md)).

Main regions (M/L: two panes; S / short: names + counts, and activating a list opens its detail as a stacked view with `Back to lists` — density contract in [ui-authoring](ui-authoring.md)):

- header: title, one-line purpose subtitle, the create form (name field + quiet `Create`)
- names pane: search + rows (name · member count as a `Figure`); the selected row carries the accent inset bar
- selected list: eyebrow + name, meta ("N companies · used by Today, Inbox and Report Season"), quiet `Rename` / `Remove` (inline confirm), the single filled action `Add companies` (`data-ux-primary-action`)
- member rows (non-interactive `DenseRow`): ticker · name (+ ISIN meta at L) · a real `Open company` button (destination) · ghost `Remove from list` (icon + label; icon-only with its accessible name in a narrow detail pane). No action column; the list scrolls internally under a pinned header
- add-companies picker (behind `Add companies`): library search, checkbox rows (already-listed companies disabled with a note), `Cancel` + `Add selected · n`

Actions (dictionary verbs only, `ActionButton`): create, rename, remove a list; add companies; remove a company from the list; open a company (→ the Spółka screen, [ADR 0107](adr/0107-company-view-paradigm.md)). Empty states are invitations (no lists → `Create the first list`; empty list → `Add companies`; no search match → create with the typed name). Keyboard: Tab order row → Open → Remove; Enter on Open navigates.

## Alerts Screen

Purpose: decide what deserves an interruption and see what fired ([ADR 0068](adr/0068-attention-routing-and-morning-briefing.md) T3; F4a language pass). A **Library** sidebar destination.

Main regions, in this order (fired alerts are why the screen exists day to day):

- **fired alerts** — newest first (50, then `Show older`), each row: ticker, what fired, when (`Figure`), the rule; actions `Open …` (destination — the company, the Inbox item or the report) and `Dismiss`
- **your alerts** — the rule list: description, scope chip, `Pause` / `Resume`, in-place price bounds for range rules, `Remove` (undoable)
- **new alert** — the composer: preset chips (what to watch for) → scope (company or list) → the preview sentence → the single filled action `Add alert` (`data-ux-primary-action`). At S the composer folds behind `Add alert`

States: no rules → invitation (`Add alert`); nothing fired → the quiet state (no action — quiet is the goal); a section read failing → its own strip + `Try again` while the rest stays live (`useAlertsQuery` on `useCommandQuery`, [ADR 0106](adr/0106-screen-data-layer-posture.md)). The grouped Today attention list lives on Today (see above); this screen reads the same app-level attention state as Today and the sidebar badge ([ADR 0097](adr/0097-toasts-are-action-feedback-only.md) dec. 6). Browser proof: `tests/browser/alerts.spec.ts`.

## Company panels (Spółka workshop)

Opening a company from the Companies library lands the **Spółka workshop** ([ADR 0107](adr/0107-company-view-paradigm.md), F3a; the docking engine and its frozen legacy dashboards were retired — [ADR 0108](adr/0108-retire-docking-engine.md)): a co-visible core (KPI table, feed, price chart, coverage, recommendations) plus a workshop bar opening one of 15 typed tools in place of the core. The panels below live in `src/screens/Spolka/panels/`, hosted by the workshop's typed tools. Panel keys: Basic info → `basicInfo`, Feed → `companyFeed`, Notebook → `companyNotebook`, Coverage → `coverage` (which now also hosts the flagged-periods review), Decision journal → `decisionJournal`, Short selling (KNF) → `shortPositions`, Warning signals → `redFlags`, Fundamentals/Claims/Quality/Report documents/diff as their own panels. Transcripts remains a future-milestone placeholder; company metadata lives in the Companies library (see there), not a panel.

**Basic info panel** (owner request 2026-07-14, panel key `basicInfo`, mockup `docs/mockups/basic-info-panel.html`): read-only identity facts — name, ticker, ISIN, sector (with a provenance chip: from the registry / manual override), latest recorded shares-outstanding fact with its period. Backed by `get_company_basic_info` (contracts.md § Companies). Edit affordances (sector override, IR reports URL) are hidden behind ONE panel-level Edit toggle — never per-fact buttons; a GLOBAL edit-mode pattern is a separate analysis task. The Fundamentals panel no longer hosts sector/IR fields and orders its sections price context → financial facts → the rest.

**Ownership ("Akcjonariat") section** ([ADR 0072](adr/0072-ownership-structure.md), `v0.56`, storyboard `docs/mockups/v056-ownership-section-storyboard.html`): a **section of the Basic info panel** below the identity facts — not a new panel, no nav entry — backed by `get_ownership_overview` (contracts.md § Ownership). Shows the shareholder structure as a **donut by holder type** (colors fixed per holder TYPE, never cycled; > 4 types fold into "Other") with the derived **free float** as a hatched/neutral "uncertain" slice — free float is `100 − Σ disclosed capital` (the 5% disclosure threshold hides smaller stakes), also surfaced as a Basic-info "Free float (derived)" rowline with an uncertainty hint. Below the donut: a **stakes-over-time** chart (one line per current holder, ▲ markers = ESPI threshold notifications) and **holder rows** with a fixed type-chip slot. Extraction is deterministic + automatic — the section populates with zero interaction once report documents are fetched. States: **empty** → a "Wydobądź z raportów" backfill CTA (`backfill_ownership_extraction`); **loading** → per-document progress ("reading shareholder tables… N/M, deterministic, no AI"); **populated** → donut + history + holder rows; **residual warnbox** → a document the deterministic parser can't read (glyph-encoded font / image table) is reported as an **honest flagged gap** with no run-action; the OCR/AI rescue path is retired ([ADR 0084](adr/0084-retire-in-app-ai-layer.md) dec. 4), so partial ESPI data stays visible and the unreadable document is named rather than guessed; **re-type/undo** → a manual holder-type change (`set_ownership_holder_type`) is authoritative and never overwritten by automation, shown with its prior source (dictionary / AI / manual) and an immediate Undo. AI holder-type classification is retired ([ADR 0084](adr/0084-retire-in-app-ai-layer.md)); holder types come from the deterministic dictionary or the user's own edit. Narrow pane: donut stacks over its legend, the chart spans full width, rows wrap without horizontal scroll (type chip stays in its fixed slot). Below the holder rows, an **"Insiderzy" block** (v0.57.0, ADR 0083 Decision 7, backed by `get_insider_overview`) extends the section: the management/supervisory holdings group ("Zarząd i rada" — person, role, shares or an explicit "not stated", via-vehicle note when indirect), the parsed MAR art. 19 **transaction timeline** (date, person, a role chip in a fixed slot, direction as a signed marker + label, volume/price when known, a link to the filing), and a **rolling aggregates strip** for the trailing 90 days / 12 months (count-based net with the buys/sells split, known-volume sums, a coverage note, and an explicit below-minimum state under 2 transactions — never an aggregate below the minimum). The **skin-in-the-game badge** occupies the holder row's fixed trailing chip slot (corroborated by a parsed management-holdings row or insider transaction, by person or via the founder's vehicle). Decision support only — counts, volumes, and who, never buy/sell language; the block wraps in a narrow pane with no horizontal scroll.

**Add-panel surface — retired** ([ADR 0107](adr/0107-company-view-paradigm.md) decision 5, superseded by [ADR 0108](adr/0108-retire-docking-engine.md)): reaching a panel for a company is the Spółka workshop's typed tool bar; there is no add-panel flow.

**Health scores** (v0.57.0, ADR 0083) — Piotroski F and Altman Z″ render in the Quality panel above the scorecard summary: two score tiles (F `n/9`; Z″ value + band-toned status) with expandable per-component breakdowns (each F signal / Z″ input with its measured values), the published-formula citation line, and explicit `insufficient data` (listing the missing inputs) / `not applicable` (financials) states.

**Warning-signals panel** (v0.57.0, ADR 0083 Decision 8/9, panel key `redFlags`, mockup `docs/mockups/v057-red-flags-panel.html`): a company-scoped panel hosted by the Spółka workshop's `sygnaly` tool, backed by `get_red_flags` (contracts.md § Red flags). **Active** flags list each with a **severity chip in a fixed leading slot** (`high` danger / `medium` warn — chips never migrate between rows), the flag-type label (`Auditor red flag` / `Report delay` / `Fund exit` / `Score deterioration` / `Short-selling spike`), the evidence title, the raised date, an **Open** action that opens the underlying feed item in the Inbox filtered to the company (when one backs the flag), and a per-row **Acknowledge** (inline confirm, `acknowledge_red_flag`). Acknowledged flags collapse into an expandable **"Historia (potwierdzone)"** group; a footnote notes each raised flag also posts a company-feed signal (attachable to an alert rule). The most common state is a **calm explicit empty state** ("Brak aktywnych sygnałów ostrzegawczych" + a reassuring line — never a blank panel). Decision support only — the title states the observed condition, never advice. Narrow pane: the date/actions wrap under the title, the severity chip stays in its fixed slot, the body scrolls internally.

**Analyst-recommendations panel** (v0.58, [ADR 0073](adr/0073-analyst-recommendations-tracking.md), panel key `analystRecommendations`, storyboard `docs/mockups/v058-analyst-recommendations-storyboard.html`): a company-scoped panel hosted by the Spółka workshop's `rekomendacje` tool, backed by `get_analyst_recommendations` (contracts.md § Analyst Recommendations). Header attribution states these are third-party broker opinions, never advice, with the source link. Summary cards: latest target price **with firm + date attribution**, local-history entry count, last-change date. Rows, newest first (publication-date order): a **verbatim rating chip in a fixed leading slot** with a toned direction sub-label (▲ upgrade / ▼ downgrade / new / reiterated), target price + % vs current close (omitted without price context), firm + analyst, publication date, a Broker-PDF external link ("—" when the source has none). Footer honesty line: ratings quoted verbatim; local history accumulates from ingestion start (the free source page carries only the newest entries); last source refresh when known. Empty state is calm and explicit; a read error keeps stale data visible with a retry. The **"vs target" readout** in the Fundamentals price-context section shows the latest target + delta with the same firm+date attribution and a "View recommendations" action that opens this panel. Narrow pane: rows wrap vertically, the rating chip stays in its fixed slot, the list scrolls internally.

Header shows: qualified ticker, display name, exchange, watchlist memberships as read-only context, last feed update, feed/unread/saved counts, and quick actions (refresh company, add note, add transcript).

**Feed panel:** company-filtered feed list with the same feed item detail behavior as Inbox and create-note-from-item. Clicking (or Enter/Space on) a feed row opens its detail inline under that row, collapsing on repeat; Up/Down move through rows while preserving expansion state. Company feed rows share Inbox's unread dot and read/unread typography. The inline detail shows source, type, timestamps, attribution, language, summary, source URL, read/save actions, an action to open the item in the Inbox with the company filter applied, and an action to create an editable note draft with feed-item origin. An empty feed shows an inline empty state with an `Open filtered Inbox` action rather than a blank panel.

**Notebook panel:** notes list newest first, filterable by tag, kind, claim status, follow-up quarter, follow-up date, with a note detail/editor pane and a compact manual Markdown note form (title, body, tags, note kind, optional claim status, event date, follow-up quarter, follow-up date); feed-item note drafts share the same form. The list stays dense enough for dozens of notes per company: title/kind/tags/status/follow-up cues only, no raw body preview. Read mode renders common Markdown structure locally; edit mode exposes the raw Markdown body. Date fields use the native date picker; follow-up-quarter fields use a small quarter picker with a `Today` shortcut.

**Claims panel** ([ADR 0040](adr/0040-management-claims-tracker.md)): first-class claims for the company (statement, due period, optional quantitative target, source evidence, verdict) with a **review queue** ("claims to verify") at the top, bucketed due/overdue/upcoming, surfaced when the due-period report arrives. Verdicts (pending, delivered, partially delivered, missed, revised) are always user-set — the queue surfaces evidence, never assigns a verdict automatically. For a quantitative claim, the matching confirmed financial fact shows beside the claim for in-place resolution. Claims are added and edited manually, with rows expanding in place under the clicked row (the app-wide row interaction pattern); the AI claim-extraction launcher was removed in `v0.59.0` ([ADR 0084](adr/0084-retire-in-app-ai-layer.md)).

**Transcripts panel** (future milestone): transcript jobs for the company, submit YouTube `URL`, review transcript segments, create note from selected segments. Global/notebook-level transcript entry starts from a required `URL` field and optional company/ticker field; if the company is omitted the app tries to recognize it after transcription, but the transcript can stay unlinked for general market videos. Company selection is required only when saving selected segments into a company notebook.

**Fundamentals panel:** reporting periods list and a KPI-per-period matrix (KPI rows × period columns), newest period first; as-reported values in their original scale (e.g. "1 093,6 mln PLN") with localized KPI names, never internal metric ids; a per-KPI trend sparkline. **The panel takes the shape of the report** (epic #398, approved mockup, replacing card #307's always-expanded collapsible groups — a company can carry ~150 rows): a fixed **statement switcher** shows one statement at a time — Kluczowe (key figures, the default view) / Rachunek wyników (income) / Bilans (balance) / Przepływy pieniężne (cash_flow) / Na akcję (per_share) / Operacyjne (`scope='company'` definitions plus the `other` catalog default) — each tab carrying a row-count badge, driven by the durable `kpi_definitions.statement_group` catalog field (migration `0130`, [data-model.md](data-model.md) § kpi_definitions) plus `kpi_relevance` active/primary rows ([ADR 0092](adr/0092-kpi-relevance-lifecycle.md)) for Kluczowe. A **find-a-position** field filters the active statement's rows by name. Statement subtotals (gross/operating/pre-tax/net profit) render heavier with a top rule. A **source/completeness bar** below the matrix shows the active statement's origin tier (or "mixed sources"), "N of M positions" filled in the latest period, and the honest count of rows still awaiting a catalog name — never silently absent. **Clicking a matrix cell opens the fact detail in a `Modal` popup** (replaces the old below-table section, card #307): value large + as-reported form, data-quality/source-tier/validation chips, slot dimensions + extraction method + created/updated + supersession info, a distinctly-styled source citation (document name when resolvable, else the raw citation text), a larger trend chart for the KPI, and Edytuj/Usuń/Zamknij actions — Edytuj switches the same modal into the edit-form fields (value/currency/annotation) in place. Manual fact entry stays a separate below-table form (inline KPI search/datalist, reporting-period selector, value, currency, gated on KPI + period + value); custom per-company KPI management alongside the seeded `canonical`/`sector` taxonomy; automatically extracted facts (aggregator, ESEF, WDF) surface through the same read model as manual facts, each labeled by origin (the positional/tier-3b origin label is retired, [ADR 0095](adr/0095-retire-html-positional-tier.md); its stored facts are deleted by migration `0135` — freed slots re-fill from the surviving tiers under their own provenance). Ingestion/extraction is launched from report feed item detail (see [UI Flows](ui-flows.md)); the matrix stays readable across the supported narrow window range (full panel height, since the detail moved into the popup).

The Fundamentals panel also hosts the **report-over-report diff** entry ([ADR 0052](adr/0052-report-over-report-diff.md)): a stored financial statement offers **Compare with previous**, opening a section-aligned diff against the prior same-type statement (SSF↔SSF, JSF↔JSF) — aligned sections (unchanged/changed/only-in-one) with the changed-section text delta and a citation into each report, readable across the narrow window range (sections stack rather than clip). Extraction-pending and no-text-layer states are shown explicitly rather than as an empty diff; the narrative management report (MD&A) is not diffable.

**Coverage panel** ([ADR 0077](adr/0077-trusted-extraction-foundations.md) §2, [ADR 0086](adr/0086-aggregator-primary-fundamentals.md)): a fundamentals **coverage map** — a period × axis table (Period / Report / Data / Flagged), one row per fiscal period, newest first. The row set follows the **period-union rule**: a period appears iff a canonical periodic report, at least one extracted fact, or at least one flagged outcome names it — so a period known only from facts (aggregator/issuer) or only from a report still shows, and gaps are never silently absent. The Report cell shows the canonical report's kind chip (+ an ESEF chip for a structured document) or an explicit "No report / not found in backfill"; the Data cell splits the period's facts into validated / flagged-or-unvalidated (success vs warning tone), or "not processed → Extract" when a fetched report has no facts yet, or "link-only — no stored file" when the canonical report is metadata-only (no file to extract, aligned with the sweep, which skips these); the **Flagged** cell counts the period's flagged facts — an **informational origin label** (facts are review-free, [ADR 0086](adr/0086-aggregator-primary-fundamentals.md) dec. 5), not a review to-do. **Flagged periods live in this panel's own "Flagged periods" section** (`v0.59.0`, [ADR 0061](adr/0061-deterministic-fundamentals-data-gathering.md) dec. 2 / [ADR 0084](adr/0084-retire-in-app-ai-layer.md)) — the former Review-queue pane was removed with the in-app AI layer, making Coverage the single home for these informational flags. The section renders **outside** the period table (a flagged outcome often has no coverage row, and a period the pipeline could not derive has no period at all), lists each flagged period with its attempted tier, a translated reason for the seven typed `reasonCode` values, the failing-check detail and any structure-drift marker, and offers a per-period **re-run** action. Its empty state ("nothing flagged") is a *good* state and is explicitly distinguishable from a failed read, which is retryable and never degrades to looking empty. Clicking a row (outside the Flagged cell) opens the company's **Report documents** pane. The panel's **history-actions footer** (T3.2, [ADR 0077](adr/0077-trusted-extraction-foundations.md) §3; epic #398 Item B) holds three actions in fixed slots plus a status line: **Fetch older reports** (primary — fetch recent reports; the backend auto-chains a history sweep so the fetched periods are extracted), **Read the ones not read yet** (secondary — run the sweep only, for documents already stored), and **Read everything again** (secondary — re-arm the company's successful ESEF-tier runs whose stored pipeline version is stale, so a widened crosswalk/projection reaches already-landed filings the sweep deliberately never re-touches). The status line names the phase (backfilling → sweep/batch `queued`/`running`/`completed (N runs [· skipped M])`), showing a live drain counter "Extracting… {done}/{total}" while runs settle; a company with automation **off** disables the first two actions with an explicit "automation off" hint rather than a silent no-op — **Read everything again stays enabled** regardless of automation mode (it reprocesses already-stored documents on explicit request, the per-period "Try again" posture, not new automation). A backfill that hit its page cap before the configured depth adds an explicit **truncation warning** ("older filings may be missing"). The sweep's former **AI-call spend** readout is retired with the in-app AI layer ([ADR 0084](adr/0084-retire-in-app-ai-layer.md)): extraction is deterministic, so there is no per-sweep AI budget to spend or display.

**Flagged periods** (a **section of the Coverage panel**, [ADR 0061](adr/0061-deterministic-fundamentals-data-gathering.md) decision 2 / [ADR 0084](adr/0084-retire-in-app-ai-layer.md) decision 4/6, `v0.59`) — the user-facing half of the "never silently wrong" guarantee, backed by `list_flagged_extraction_outcomes` (contracts.md § Structured-first extraction). It lists the periods where the deterministic pipeline **ran and refused to record anything**: no tier could read the document, or the validation gate refused the values. It sits below the coverage map and above the history-actions footer, and renders **independently of the map** — a `no_period_derived` failure has no period, so it can never be a coverage row, and gating it on the table would hide exactly the gaps it exists to show. Not a new panel and no nav entry: coverage is already the company's "what do we have / what is missing / what is flagged" surface, and the Review queue panel that used to host human review was retired with the in-app AI layer.

Each row carries, in **fixed slots** (period · reader chip · reason · drift chip — a chip never migrates because a neighbour is absent): the period, the deterministic **reader (tier)** that attempted it (or an explicit "No reader" when none could), the **reason in plain language** — the backend emits only a typed `reasonCode` (`validation_failed` · `structure_drift` · `witness_disagreement` · `no_deterministic_tier` · `no_period_derived` · `document_unreadable`; `emitted` never appears here) which the frontend translates, so a raw code never reaches the user — and a "Layout changed" chip when structure drift was detected. The row **expands in place** onto the failing check's evidence (the gate's expected/actual/residual, rendered as label/value lines) plus the attempt count. A **`no_period_derived` failure arrives on the sentinel period** (`fiscalYear 0`, empty `periodType`/`periodEnd`, because a period-less failure has no slot key) and renders as **"Period unknown"** — never as a fabricated fiscal year.

The reason vocabulary also carries **`value_divergence`** (epic #229 T5, issue #192): a re-extraction read a **different figure** than the one already on file. The stored value is kept — the row states the disagreement and expands onto the two figures side by side ("on file X, re-read from the report Y"), so a divergence between two reads of the issuer's own filing is reviewable instead of living only in a developer-mode diagnostic.

**Flagged figures** (a second section of the Coverage panel, epic #229 T5, backed by `list_flagged_fact_provenance`): the complement of the section above — the values that **did** land but carry a drift/contradiction. The Flagged table column counts them per period; this section is where the figures themselves are readable. Each row shows the period, the metric under its localized KPI name, the formatted value, the reader that produced it as a fixed trailing chip, and the citation the value was read from. **Informational, never a to-do** (facts are review-free, [ADR 0086](adr/0086-aggregator-primary-fundamentals.md) dec. 5) — the section offers no action. Same three explicit read states as its sibling: loading, the good "no flagged figures" empty state, and a retryable read failure that never degrades into looking empty.

Per row, a **"Try again"** action calls `rerun_extraction_outcome` with the stored outcome id (so the retry can never target a different slot than the one displayed) — **except on a `witness_disagreement` row**, whose slot names an aggregator page rather than a stored document: there is nothing to re-read, so the action is replaced by a plain "nothing to re-read" note in the same reserved slot rather than offering a control that cannot work (the backend refuses such a request with the typed `rerun_not_applicable` code). **Every** re-run action disables while one is in flight, and the list refetches on completion — the backend updates the row in place, so a period whose cause is fixed leaves the list instead of leaving a stale flag beside a fresh success. A successful re-run also reloads the coverage map. The three read states are **explicit and never blurred**: loading ("Checking for flagged periods…"), the **good** empty state ("Nothing flagged — every attempted period produced data."), and a read failure, which states the error and offers **Retry** — a failed read must never look like "nothing flagged". Narrow (S) pane: the four slots stack; the section scrolls with the panel, never sideways.


**Raw report capture** (a third section of the Coverage panel, [ADR 0100](adr/0100-two-layer-tagged-fact-capture-and-ifrs-vocabulary.md) decision 10, epic #398 final slice, owner-approved mockup `docs/mockups/raw-tagged-facts-panel.html`) — the trust proof that every number a tagged report carried is either in Fundamentals or has a reason it isn't, compacted to **one line, not a screen** (per the owner's own review of the mockup). Renders only for a company with tagged capture ≥1 (silent otherwise — an all-zero grid would read as a bug). A compact key/value grid (`getReportTaggedFactCoverage`) shows six counts: numbers in the report, in Fundamentals, split into parts (dimensional), from notes (no primary-statement role), no name yet (uncrosswalked), conflicting values (a genuine disagreement, never resolved by document order). A disclosure button — naming both the action and the count, never the count alone (accessibility) — expands **"Positions the program doesn't know yet"** (`listUncrosswalkedConcepts`): every captured concept with no crosswalk entry at this company, ranked by how many companies across the whole corpus report it, each row showing the human name (the issuer's own published label for an extension concept, or the raw technical name **explicitly marked "no translation yet"** for a standard concept — never a synthesized Polish name), the statement + window it was tagged under, and a **"Show in Fundamentals"** promote action. Promoting swaps the row to an "In Fundamentals" chip in place (no refetch) and materializes the concept's already-captured rows into that company's Fundamentals matrix under a new `scope='company'` definition. Company-scoped only — never a canonical crosswalk entry, and never reachable by an MCP agent (decision 10: "the owner may promote… a machine still may not").

**Review queue panel — REMOVED**, `review` panel key no longer exists ([ADR 0084](adr/0084-retire-in-app-ai-layer.md)); human review now happens in the **Coverage panel's "Flagged periods" section** (above).

**Decision journal panel** ([ADR 0071](adr/0071-judgment-capture.md), panel key `decisionJournal`): the company's chronological record of recorded judgments (`buy` / `pass` / `keep_watching` / `sell_note`) with a Markdown rationale and a decided-on date, newest by **decided_at** (never insertion order). A compact composer (decision kind, decided-on date, Markdown rationale) records an entry; the list∥detail body shows the selected entry's kind, date, and rendered rationale plus an evidence picker that links company-timeline items (feed items, notes, claims, events) to the decision, reusing the research `EvidenceRow` link pattern (`fromType: "decision_entry"`). **Entries are immutable** ([ADR 0071](adr/0071-judgment-capture.md)): there is no edit or delete affordance anywhere — a correction is a follow-up entry created via **Supersede** (linked by `supersededByEntryId`, marked with a "Follow-up" chip). Entries join the company research timeline. Reached via the Spółka workshop bar's **Decision journal** tool, an occasional-entry surface — the ONLY decision-journal surface (the cross-company global screen retired 2026-09-02, F4c S2, ADR 0108 amendment: 2 entries in 3 months, no real cross-company review use; agent reads across companies stay on MCP).

**Short selling (KNF) panel** ([ADR 0069](adr/0069-source-reliability-and-disclosure-signals.md) decision 3, panel key `shortPositions`, mockup `docs/mockups/short-positions-panel.html`): the company's net short positions from the KNF public register (≥ 0.5% threshold). Header + source attribution, three summary tiles (aggregate net short %, holders with a position ≥ 0.5%, 30-day change in pp — an increase reads as supply pressure, warning tone), a current-positions table (holder / net position % / calculation date, with a "changed" chip on holders that moved within 30 days), and a **"Historia zmian"** history list phrased by kind (entered / increased / decreased / exited, with from→to and the domain date). The most common state is **empty** ("Brak zarejestrowanych pozycji krótkich"), which still shows the last remembered register presence ("Ostatnia obecność") when there was one, plus the footnote that appearing in the register raises a signal (attachable to an alert rule). Read-only (register populated by the daily `knf-short-selling` adapter), backed by `list_short_positions` (contracts.md § Short Positions (KNF)). **Not in the curated dashboard defaults** — palette-only (owner decision 2026-07-15). Narrow pane: stat tiles collapse to a column; the positions table scrolls inside its own bounded scroller.

**Report documents panel** ([ADR 0077](adr/0077-trusted-extraction-foundations.md) §2, mockup Panel B): the stored ESPI/EBI attachments and user report links for the company, read from the `get_report_documents_view` read model. By default it **groups documents by fiscal period**, newest first, with a **"Group by period" toggle** back to a flat list. Each group header names the period, the report cadence (annual / half-year / quarterly), and the document count. Within a group the **periodic statements** come first — a **★ marks the period's canonical report** (the same document the Coverage map names) — then audit reports, then a **fold** hiding the signature/data companions (`.xades` signatures, `.xbri`/`.xbrl` data, selected-data extracts); a companion whose **"Extract data" action is available is never folded**, so an actionable row can't hide. Non-periodic filings (announcements, GM materials, other) collect in a separate **"No period" group, collapsed by default** behind a "show all (N)" disclosure. Each row leads with the **document-kind label** (kind chip + an ESEF chip for structured filings), demoting the raw filename to a muted second line whose link keeps the full original filename in its tooltip; the trailing edge holds a fixed storage-status chip slot ("Stored" / "Link only") and a reserved "Extract data" action slot. A **search field** filters across title/filename, and the kind filter + "Refresh classification" action stay. Density (ADR 0076 D6): the kind label + filename + date show at every tier; the kind/status chips appear from M, the extract action gains its label at L, and a short/narrow pane drops the chips + action.

Note work happens in the per-company Spółka **`notatnik`** tool (above); cross-company note review has no dedicated screen (retired 2026-09-02, F4c S2, ADR 0108 amendment — 7 notes across 6 companies in 3 months, no real cross-company review use). Every deep link that used to land on a global Notebooks screen (Inbox "Create note draft", research evidence, global search, a transcript-selection note) now opens the per-company tool via a typed intent, highlighting or prefilling as appropriate. Note detail shows origin links as compact, read-only actions: a feed-item origin's external source link, a URL-backed origin's source link. Agent reads across companies stay on MCP.

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

**Status: not built** ([ADR 0058](adr/0058-investor-week-calendar.md)) — the Events screen's week view (F4b S3) ships the plain working-week calendar only; none of the scope/layer toggles below exist in the code or the screen's props/controller.

The Events screen also offers a **weekly working-day view** (Mon–Fri columns; a weekend column only when populated) — the investor week calendar ([ADR 0058](adr/0058-investor-week-calendar.md), `v0.67.0`), inspired by the Koomberg weekly digest. It composes opt-in **layers** over the same data, with our own UI:

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

A **witness** source (`role = witness`) shows a **Witness** badge on its row so it reads as a health mechanism, not a feed — it reconciles its listings against the primary source and never ingests into the feed ([ADR 0069](adr/0069-source-reliability-and-disclosure-signals.md) D2). `GPW ESPI/EBI` is the witness for the Bankier official-report channel; when it finds an official report the primary missed, that raises an attention event (Today stream + sidebar badge + morning briefing, [ADR 0097](adr/0097-toasts-are-action-feedback-only.md)). The full pair ledger is in Developer Diagnostics.

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
- source reconciliation section: the recent GPW ESPI/EBI witness ↔ Bankier pair ledger (`list_source_reconciliation`), each row showing the disclosure, company, and a `matched` / `Missed by primary` (`espi_only`) / `Bankier only` status chip ([ADR 0069](adr/0069-source-reliability-and-disclosure-signals.md) D2)
- backups section: backup status (last backup time, count), backup list (rotating backups and pre-migration snapshots), a create-backup-now action, and a restore action with explicit confirmation that warns restore is applied on app relaunch (see [ADR 0032](adr/0032-search-and-backup-boundaries.md))
- developer mode status and disable action

Rules:

- The Diagnostics navigation item is hidden unless Developer mode is active.
- Diagnostic events are for troubleshooting only and must not replace normal user-facing status, errors, or progress UI.
- Event details must clearly show that metadata is redacted and local-only.
- Metrics are operational health signals, not product analytics. Process-lifetime counters must be presented as runtime-only signals that reset on app restart.
- Runtime log viewing is available only from Diagnostics while Developer mode is active, even though log configuration is visible in Settings.
- The first rich timeline is video-transcript job progress ([ADR 0084](adr/0084-retire-in-app-ai-layer.md) decision 3 — the only remaining AI job kind), including queued, running, provider resolved, credential checked, request sent, response received, parsed, stored, and failed.
- Non-AI modules may show lightweight baseline events where useful, while detailed logs and metrics remain separate observability surfaces.
- Raw diagnostic JSON/file export is outside M14 scope.

## Settings Screen

Purpose: local preferences and provider configuration.

Settings uses local subnavigation rather than route-level pages. Each section opens in the settings workspace while the settings navigation remains visible.

Sections:

- Appearance: dark/light/system brightness mode, separate accent palette with `night-neon` and `midnight-horizon`, extensible locale setting with English default and Polish as the first additional language
- Sources: polling interval, **backfill history depth** (clickable presets 1/3/5/10 years bound to a slider + numeric input, clamped 1–10, default 3; ADR 0077 §3), import/export status
- AI providers: Gemini configuration for YouTube transcription — the only in-app AI capability ([ADR 0084](adr/0084-retire-in-app-ai-layer.md)) — selectable transcription model, credential configured/not-configured status, credential storage, secret kind
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

A global, keyboard-reachable search box lives in the top toolbar and queries the unified `search_index`. Results are ranked, grouped by content type, and show a snippet; selecting a result navigates to the owning screen/item. Copy is localized (en/pl).

The earlier constraint that kept search workspace-scoped is lifted now that a cross-workspace result model exists. The existing per-workspace search/filter inputs remain: Inbox owns feed-item filtering in its toolbar, Companies owns company-list search, and the Spółka `notatnik` tool owns note filtering for its company. Global search complements, rather than replaces, those local lists.

## Research Workspace

The Research workspace (company/watchlist evidence timeline, review checkpoints, questions, reminders) shipped through `v0.31.0` (its AI brief/digest halves are retired — [ADR 0084](adr/0084-retire-in-app-ai-layer.md)); its live behavior is governed by [ADR 0022](adr/0022-research-evidence-read-model-boundary.md) and specified in [Contracts § Research Evidence Boundary](contracts.md#research-evidence-boundary) and [Data Model § Research Evidence Boundary](data-model.md#research-evidence-boundary). Delivery chronicle (M25/M26/M29/M31) moved to [Kanban Archive](kanban-archive.md#archived-investigation-and-study-notes-moved-2026-07-02).

**Reachability** ([ADR 0107](adr/0107-company-view-paradigm.md), F3a; [ADR 0054](adr/0054-mode-based-thesis-centric-shell.md) amendment, F4c). `ResearchScreen` is reached three ways: its **Library nav entry** (Ctrl+4, #94), the command palette's `Open screen: Research` entry (the same standalone route, no company scope), and the Spółka workshop's **`research`** tool (hosts the same global screen — it is not company-scoped; the company picker inside it selects the scope).

## Responsive Behavior

The primary target is desktop. The first implementation should still avoid layouts that break at narrow widths:

- sidebar may collapse
- detail pane may become an overlay
- dense tables/lists should preserve readable text
- icon buttons need tooltips when meaning is not obvious

## Deferred UI

No UI is built for anything in [Roadmap § Not In V1](roadmap.md#not-in-v1) (portfolio position tracking, trade journal, billing/hosted-licensing infrastructure, cloud sync, multi-user/team/sharing workflows).
