# Product Spec

Doc map: [CLAUDE.md](../CLAUDE.md) § Required Reading. Related references: [UI Flows](ui-flows.md), [UI Information Architecture](ui-information-architecture.md), [Source Strategy](source-strategy.md), and [Contracts](contracts.md).

## V1 Experience

The first screen is an investor inbox: a dense chronological feed of company-specific reports and news. It should support repeated daily use and fast scanning. Each company also has a notebook for durable research notes.

Early sample feed implementations should already obey local watchlist, company, unread, and saved filters, even before real source ingestion exists.
Sample feed implementations may keep read/saved changes in memory, but stored feed items must persist read and saved state locally.
The UI-facing Inbox is scoped to tracked companies. Raw or sample source items that do not match the local company registry may exist internally, but they should not appear in the normal feed until the company is added.
Source URLs in item details must be directly actionable so the user can verify the original report or article quickly.
Feed details should show a short summary first. `summary` is separate from the report title and full body, but when no source summary is available the UI may use the report title as the summary fallback. Full official report body text should be available in the same detail pane but collapsed by default and expanded on demand.
When filters hide all feed items, the UI must offer a quick way to clear active filters and return to the full feed.
Inbox empty states should distinguish first-run setup from filter misses. If no companies are tracked, the Inbox should point directly to adding a company. If companies exist but no feed items are stored, the Inbox should make clear that source ingestion or refresh is not wired yet and offer a path to source status. If filters hide existing feed items, the Inbox should offer `Clear filters`.
The app shell should expose an Inbox unread count badge when unread feed items exist.
The top toolbar should remain visible while the active workspace scrolls so search, status, refresh placeholders, and theme controls are always reachable.
Inbox feed rows should be usable by mouse and keyboard. Enter or Space on a focused row selects the item and updates the detail pane. Up and Down arrows move focus through feed rows and update the selected detail item.
The selected Inbox row should remain visually and semantically anchored to the detail pane so the user can always tell which item is being inspected.
When the selected item leaves the current filtered feed, for example after marking it read in the Unread filter, the Inbox should move selection to the next visible item or show the relevant empty state.
The Inbox should show a compact review summary for the current filtered set, including visible, unread, and saved counts. This summary is informational only and should not compete with the primary filters.

Expected v1 UI areas:

- feed list with newest items first
- watchlist selector
- company filter
- search across visible feed items
- source/type filter
- unread/read state
- saved items
- item detail pane with original link, compact metadata, and bottom-aligned publication/fetch timestamps
- action to create a company notebook note from a feed item

The feed detail pane is a fixed-width side rail and is treated as a containment boundary: it shows compact, read-only summaries plus launchers, never wide interactive controls. Rich flows open in the centered modal rather than rendering inline in the rail; the rail shows a status pill and a one-line summary with a button to open the full flow. (The AI-analysis and AI-KPI-extraction modals this rule was written for were retired in `v0.59.0` — migration 0102 dropped their stored outputs, nothing survives ([ADR 0084](adr/0084-retire-in-app-ai-layer.md)); the containment rule itself stands for every future rich flow.) This keeps the rail readable at the supported narrow window range and prevents a wide descendant from clipping the pane. New rail panels render inside the `DetailSection` primitive, which bakes in the containment contract so the rail cannot regress.
- manual refresh control
- in-app badges for new/unread items

Milestone 3 introduces the company workspace as the second primary research surface after the Inbox. Opening a company from the Companies screen should show one ticker-focused page with Feed, Notebook, Claims, Transcripts, and Metadata tabs. The first implemented tab is Feed, backed by the same stored feed item model as the Inbox and filtered by the company's exchange-qualified ticker. Notebook, Claims, and Transcripts tabs may begin as placeholders until their dedicated milestones, but the navigation shape should be stable.

The global Notebooks and Transcripts navigation entries may begin as explicit placeholder screens during Milestone 3. They should not be blank dead ends; they should state the planned milestone and scope without pretending the workflows are implemented.

Desktop notifications are out of scope for v1. Portfolio positions, cost basis, and trading workflows are out of scope.

Keyboard shortcuts are in v1 scope as late workflow polish. They should speed up repeated inbox and research actions, but every shortcut action must remain available through visible UI controls. Shortcuts must be discoverable and configurable in the app, including disabling and resetting defined shortcut actions. Shortcuts must not interfere with text editing in search fields, note editors, forms, or transcript selection workflows. Desired notebook shortcuts include `Ctrl+E` to open the editor for the selected note or claim and `Ctrl+S` to save the item currently being edited.

## Theme And Visual Direction

The app must support user-selectable dark, light, and system brightness modes. Dark theme is the default.

The dark visual direction is inspired by the attached blue, pink, and purple night landscape reference:

- deep navy and near-black surfaces
- electric blue/cyan primary accents
- pink and purple secondary accents
- restrained glow or highlight effects for active states, important badges, and focus rings
- high-contrast text suitable for dense investor workflows

The palette should support a serious work-focused desktop app. Use the reference colors for accents and atmosphere, not as a full-screen decorative background for the main product UI.

Light theme should preserve the same brand accent colors while using readable light surfaces and accessible contrast.

Brightness mode and accent palette are separate user settings. `theme` controls dark, light, or system mode; `accent_palette` controls the named palette mapped onto semantic UI tokens. V1 starts with `night-neon` and adds `midnight-horizon`, a palette based on colors sampled from the project owner's preferred reference image. Future palettes should be added through the same palette boundary instead of hard-coded per-screen colors.

## Watchlists And Companies

Users can maintain multiple watchlists. Companies can be assigned to and removed from watchlists without deleting the company from the local registry. Watchlists should be useful as filters in the Inbox, Events/Calendar, Companies, and Notebooks workspaces. The durable watchlist management workflow belongs in a dedicated Watchlists panel in the left menu. That panel owns creating, renaming, deleting, and selecting watchlists. It also owns adding already-tracked companies to the selected watchlist and removing companies from it. Renaming a watchlist preserves its stable internal watchlist id. Company list rows and the company workspace should only show existing watchlist memberships for scanning and context; they should not expose watchlist create, delete, add, or remove controls. Companies are displayed ticker-first, but canonical storage uses exchange-qualified tickers such as `GPW:CDR` or `NASDAQ:MSFT`.

Watchlists are user-owned company groups. Normal UI copy should describe them in those terms and should not expose storage or architecture language. Future premium alerts may use watchlists as configuration inputs, but alerting is outside the dedicated watchlist-management milestone. M19 should keep backend watchlist boundaries modular and extensible for future features without adding visible placeholders or unused alert-specific fields.

Exchange-qualified ticker labels should visually distinguish the exchange segment from the symbol segment across the app through a shared renderer. Known exchanges should have explicit colors, and future exchanges should receive deterministic palette colors so new company sources remain visually distinguishable without changing storage contracts.

The Companies screen should let the user open a company workspace by clicking the company row itself, not a separate `Open` button. The workspace expands inline directly under the selected company row and collapses when the same row is clicked again. Up/Down arrows move focus through company rows. If a workspace is already open, arrow movement moves the open workspace to the focused company; if no workspace is open, arrow movement keeps the list collapsed. The workspace Feed tab follows the same pattern: clicking a feed row or pressing Enter/Space on a focused feed row expands source details directly under that row, repeating the action collapses the details, Up/Down arrows move focus between company feed rows, and if detail is already open the detail moves to the focused feed row. An explicit action can open the same item in the Inbox with the company filter applied.

If a tracked company has no stored feed items, the company Feed tab should show an explicit empty state. The empty state should explain that the company is tracked but has no sample or ingested items yet, and it should offer a quick path to the Inbox with that company filter applied.
The Inbox detail pane should also let the user open the matched company workspace when the feed item maps to a locally tracked company. Opening from Inbox should keep the Company Feed tab active and expand the same feed item inline in the company workspace.

Company metadata may include:

- display name
- exchange
- ticker
- ISIN
- CIK
- LEI
- aliases
- source-specific IDs

Company entry should support lookup/enrichment. After the user selects an exchange and enters a ticker, ISIN, or company name, the app should be able to fill the remaining company fields when a confident match exists. Exact ticker and ISIN lookups may auto-fill directly. Name lookup may require suggestions or user confirmation when multiple matches exist.

Local test samples for GPW metadata are allowed only in tests and development research, not as target runtime seed data. V1 includes a local GPW company registry cache for all companies exposed by the public GPW company list so company creation, autocomplete, and source matching are not driven by manual entry long term. The registry is stored locally, refreshed manually and by a slow in-app stale-cache scheduler, and may auto-refresh from company lookup when the runtime cache is empty. The Companies form shows cached registry suggestions while the user enters ticker, company name, or ISIN, the tracked company list is searchable, and the Sources registry list remains searchable for diagnostics and bulk inspection. The registry is used for ticker-first matching with ISIN fallback. Company names remain useful for display and suggestions, but should not silently match feed items by themselves.

## Company Notebooks

Each company has a notebook tied to its canonical company identity. Notes should support manual entry and creation from feed items, reports, and transcripts.

Notebook entries should support:

- title
- Markdown body
- tags
- source/origin links
- optional event date
- optional follow-up date
- optional quarter or reporting period
- status for claims that should be checked later

The first claim-tracking workflow should support management statements such as "the board said X should happen in the near future" and later verify whether the company delivered after one or more quarters. Claim follow-up supports both a follow-up quarter and an exact follow-up date, with quarters emphasized in the UI.

Milestone 4 starts by making the company workspace Notebook tab durable. The first implementation supports listing company notes and creating manual Markdown notes with tags, kind, optional claim status, event date, follow-up quarter, follow-up date, and manual origin. Feed item detail views can create editable note drafts in the main Notebooks pane for the matched tracked company. Saving those drafts preserves `feed_item` origin with source URL and feed item identity.

Notebook read mode renders common Markdown locally, including headings, paragraphs, lists, blockquotes, inline code, fenced code, bold, and italics. Edit mode remains a plain Markdown text editor.

As of `v0.42.0` ([ADR 0040](adr/0040-management-claims-tracker.md)) the company workspace Claims tab is the **management claims tracker** over a first-class claim entity (no longer a filtered view of notebook entries; existing claim notes are migrated). A claim carries the management statement, a due period (fiscal year + period type, quarters emphasized), an optional quantitative target, source evidence (report document, transcript segment, feed item, or manual), and a user-set verdict (`pending`/`delivered`/`partially_delivered`/`missed`/`revised`). The tracker has three parts: (1) a **claims list** with verdict and due/overdue status; (2) a **review queue** ("claims to verify") that resurfaces a claim when its due-period report arrives — for a quantitative claim the matching confirmed financial fact is shown beside it so the user can resolve the verdict against evidence; (3) **manual claim entry** — the AI claim-extraction path was retired with the in-app AI layer ([ADR 0084](adr/0084-retire-in-app-ai-layer.md), `v0.59.0`); an external agent may propose claims over the MCP port with mandatory provenance in a later milestone. There are no automated verdicts: the app surfaces the evidence and the user decides. Claims remain research evidence (they appear in the company research timeline and feed digests/reminders) and are owner-durable in import/export.

Notebook UX must assume a company can accumulate dozens of notes. The company Notebook tab should prefer a compact selectable list plus a selected-note detail/editor area over large stacked cards. The note creation form should stay out of the way when not needed. Notes should be easy to scan, select, read, and edit without losing context.

The current product assumption is that daily note work will happen primarily in the main Notebooks pane, while the company workspace Notebook tab remains the contextual per-company surface. The main Notebooks pane is company-navigable from its first implementation, making it easy to move between companies, create a manual note for the selected company, see each company's loaded note count or follow-up pressure, filter by kind, claim status, tag, and follow-up scheduling presence, and open the selected company's notes without losing cross-company orientation. This assumption may change after hands-on use.

## Sources

V1 focuses on GPW:

- GPW ESPI/EBI official reports
- selected public or RSS media sources where usage is allowed
- selected company-related articles, analysis, and news sources after source-specific review
- authenticated private research sources when explicitly approved by source-specific ADR

Later sources should be possible through adapters:

- SEC EDGAR submissions and XBRL APIs
- Nasdaq RSS feeds
- major European exchange disclosures and RSS feeds

The app should prefer official/public/RSS sources and avoid restricted scraping by default.

Official reports are not the whole v1 feed. The user also wants a company-specific article/news layer so the Inbox can collect media coverage, analysis, and research about tracked companies. Candidate public or semi-public sources include Stooq-style ticker news, XTB market news/analysis, ISBnews-like providers, Bankier article pages, and other Polish market-analysis pages after each source is reviewed. These sources should be represented as adapters with explicit source type, attribution, fetch mode, and matching rules instead of being folded into the GPW official-report adapter.

Portal Analiz is a desired v1 source using the user's own paid personal account. Because it is authenticated, private, and likely lacks a public API, it must not be implemented as a generic scraper or hidden browser automation shortcut. [ADR 0014](adr/0014-portal-analiz-authenticated-source-policy.md) accepts it only as a dedicated authenticated private research adapter with local keychain secrets, conservative user-scoped fetching, source attribution, offline test samples/mocks, and explicit discussion if the source proves technically or policy-wise too troublesome.

The Sources screen shows implemented sources only, with normal-user status, supported markets, optional enable/disable controls, poll timing, last success or error status, source links, and manual refresh actions. It must not show unimplemented source candidates, internal source IDs, fetch modes, rate-limit policy notes, unmatched diagnostics, or other implementation detail in normal mode. Developer mode and docs may expose candidates and technical source detail.

The GPW company list is presented as company-directory and lookup support, not generic source ingestion. Future company-directory sources, including NewConnect, should plug into the same lookup and matching boundary. Company lookup and autocomplete should search every active company directory so companies from all supported registries are discoverable from the same Companies form. The exchange field may disambiguate duplicate tickers, but it should not prevent finding a company that exists only in another supported registry.
Source rows should follow the app-wide row interaction rule: the row is the primary click target, and source operational details expand inline under the selected source row. Enter or Space on a focused source row should expand or collapse the same inline detail.
The topbar should expose a compact source status entry point that summarizes source readiness and opens the Sources screen. Opening source status should expand the most relevant source immediately, preferring sources with errors, then enabled sources, then the first configured source. This is separate from manual source refresh; before real ingestion exists, refresh remains disabled while status inspection is available.

## Ingestion

The default polling interval is 15 minutes while the app is open. Manual source refresh must be supported.
Before real source ingestion exists, the topbar source refresh control is a disabled placeholder. It must eventually trigger or enqueue source refresh jobs.

A local data status indicator may expose a small view refresh action for development and recovery ergonomics. That action reloads local app state such as feed items, companies, watchlists, memberships, and local data status, but it is not the product-level news/source refresh workflow.

Ingestion should preserve source attribution, publication time, fetch time, original language, matched company, and source URL.

**Feed content persists; no bulk destructive feed actions (owner decision 2026-08-05, #329).** Both automatic timed cleanup and the manual "Clean up feed now" / "Delete unsaved" controls are removed — an earlier automatic 30-day prune silently removed ~3,900 items, including periodic reports the owner actively researches, and the manual fallback carried the same risk. No feed-specific deletion path exists today (a full-database restore replaces everything wholesale, which is a different operation). The eventual retention design (#175) must bound local growth **without** a bulk-delete control: whatever it chooses, saved items, items linked to notes, and items with explicit user decisions stay, and reintroducing any mass-destructive feed action requires an explicit product decision — never a casual control.

## Typed Filing Signals

Official ESPI/EBI filings should be classified into typed disclosure signals so the feed reads as a signal stream instead of homework. The investor should be able to tell at a glance whether a filing is an insider transaction, a dividend, a profit warning or result estimate, a significant contract, an own-share transaction, or a guidance change.

Behavior:

- A deterministic rule classifier runs during ingestion over the filing's official category label, title, and body, and types the formulaic majority of filings automatically.
- Filings the rules cannot place land in an explicit unclassified bucket, triaged headless via the MCP tool pair (`list_unclassified_filings`/`classify_filing` — BYOA). The app never silently assigns a wrong type.
- Typed filings show a type badge in the Inbox and company feed, and the feed can be filtered by type.
- The digest groups high-signal types (for example, insider activity) so the user sees them together, and high-signal types can drive reminders.
- A typed filing that carries a real future date (such as a dividend record/payment date or a general-meeting date) also appears as a company event in the calendar. Past disclosures stay in the feed and do not create calendar entries.

Typed signals are decision support, not recommendations: a signal states what kind of disclosure occurred, with a link back to the official filing, and never implies a buy/sell/hold action.

## Attention Routing And Alerts

The app tells the investor what deserves a look rather than making them re-scan everything. The investor owns **alert rules**: each rule says what to be alerted about — a **signal category** (e.g. insider activity, a profit warning), an **autopilot run completing**, or a **price condition** (the price *enters a range I set*, or reaches a *52-week low*) — scoped to a **single company** or a whole **watchlist**, and can be enabled or disabled. Price conditions evaluate against the daily quotes on each daily pull. Rules are managed in Settings (a visual **alerts manager**: preset rule chips, a scope picker, and price-range inputs), where fired alerts can also be reviewed.

Some attention events have **no user rule** behind them: the app raises them itself. Besides the missed-report reconciliation, a **background task that gives up** does — when the app's own work (fetching price history, sweeping report history, extracting a shareholder table, composing the briefing) fails and has exhausted its retries, the investor sees it in the Today stream, naming which task failed and on what. Work the app promised must never fail in silence. Tasks that already report themselves elsewhere (a source's health row on Sources, an autopilot run card) state it there instead, so one failure is never announced twice, and a task still retrying stays quiet until it truly gives up.

When a rule fires it records an **attention event** tied to the exact evidence that triggered it — the signal item, the autopilot run card, or the quote context — so every alert traces back to a fact. Events are **deduplicated** (re-ingesting the same filing never re-fires) and each rule is throttled to at most once per day. Fired events surface two ways: a **persistent toast** the investor can click through to the evidence, and a **Today attention list** grouped by company where events are marked seen or dismissed. Attention routing is **in-app only** in v1 (the boundary is shaped for later OS-notification adapters) and stays strictly factual — an alert states what happened with a link, and never phrases it as advice.

### Severity taxonomy

Every attention event and autopilot run carries a typed **severity** — `urgent`, `notable`, or `routine` ([ADR 0087](adr/0087-today-attention-home-v2.md) decision 2) — that fixes how prominently it surfaces. Severity is **derived in exactly one backend place** from the trigger type plus (for signal events) the signal category, and shipped as a typed value on the read models; the UI routes on it and never re-infers importance from strings. It is **computed at read, never stored**. The table below is authoritative — the code (`storage::severity`) mirrors it exactly, and a classification gate reddens if a new trigger type or signal category is added without a severity here.

There is **exactly one frontend-side exception**: claims-to-verify carry no backend severity, so the Today stream classifies them itself (`verifyClaimSeverity`, not `storage::severity`) — the row marked **frontend stream model** below. This is the only place the UI assigns severity, and it does so from the claim's typed due/overdue bucket, not from any string.

| Severity | Surface behavior | Triggers / categories |
| --- | --- | --- |
| **urgent** | Leads the stream; the only level that may raise a **persistent** toast | Missed-report reconciliation (`source_reconciliation`); signal categories `insider_transaction`, `profit_warning`, `auditor_opinion` |
| **notable** | Stream + a **transient** toast at most | Fired price alert (`price_enters_range`, `price_week52_low`); autopilot-completion event (`autopilot_run_completed`); an autopilot **run** that ended `failed` or `partial`; a **terminally failed background job** (`job_failed`, every job kind — [ADR 0091](adr/0091-failure-path-and-real-state-testing.md) decision 1); signal categories `dividend`, `own_shares`, `general_meeting`, `significant_contract`, `guidance_change`, `major_holdings_change`, `short_position_change`, `fund_exit`, `recommendation_change`, `report_delay`, `score_deterioration`; and any unknown/unclassified signal category (never silently downgraded to routine) |
| **notable** — *frontend stream model* (`verifyClaimSeverity`, the one FE-side entry) | Stream only — raises **no toast** (a claim is not an attention event) | A **claim overdue for verification** (its due-period report arrived, still unverified). Ranks above routine, below urgent; may fold into a same-cause **notable** aggregate when several pile up, never into a routine one |
| **routine** | Stream only, dimmed | An autopilot **run** that is `succeeded`, `pending`, or `running`; signal category `other`; a claim **due but not yet overdue**, and every other verify/changed/upcoming stream row |
| **urgent → notable** — *aging demotion* | An aged `urgent` event stops leading the stream (still shown, nothing hidden or dismissed) | An `urgent` attention event **unacted for more than 72h after `fired_at`** demotes to `notable` at read ([ADR 0087](adr/0087-today-attention-home-v2.md) amendment 2026-07-23, live-checkpoint finding). Purely **age-based** — seen/dismissed state does not matter. Boundary: at exactly 72h it **stays urgent**; only strictly older demotes |

Walls fold cross-company ([ADR 0087](adr/0087-today-attention-home-v2.md) amendment 2026-07-23): when several company-rows share **one cause** and **one severity**, they collapse into a single `×K spółek` aggregate that expands in place — urgent from 2+ (systemic alarm), notable from 4+ (the routine "more than 3" wall applied to notable, so a demoted urgent wall still folds), routine from 4+ (category-keyed). Collapse partitions **by severity first** (a cause's urgent and notable rows never merge) and **different causes never merge** (a price-alert day folds per cause). An attention aggregate carries a two-step **"Dismiss all"**.

## Morning Briefing

At the top of Today the investor sees a **morning briefing** answering "what changed in my companies and what needs doing" — a deterministically composed list of **new signals since the last briefing, autopilot runs, claims due, upcoming report dates, and fired alerts**, ordered by when each thing happened. It refreshes **once a day automatically** while the app is open and can be regenerated **on demand**.

The briefing is deterministic end to end and always renders as the **structured item list** — the optional AI narrative half was retired with the in-app AI layer ([ADR 0084](adr/0084-retire-in-app-ai-layer.md)). Like every surface it is decision support: facts and citations, never a buy/sell/hold call.

## Company Health

Status: planned (v0.57.0, ADR 0083)

The app computes published, citable health formulas over a company's confirmed financial facts and raises red flags on its own — so concerns surface before the investor reads a report ([ADR 0083](adr/0083-company-health-scores-and-red-flags.md)).

Behavior:

- **Health scores**: Piotroski F (0–9) and Altman Z″ (emerging-markets variant; safe/grey/distress bands) computed deterministically from confirmed facts for annual periods, shown in the Quality area with an expandable per-component breakdown and the formula citation. A score renders only when every input is present; otherwise an explicit "insufficient data" state lists what computed and what is missing — never a partial or rescaled headline number. Banks, insurers, and other financial-statement companies show "not applicable" for Z″. Scores are also usable in scorecard criteria.
- **Insider activity**: MAR art. 19 notifications are parsed into who bought or sold, in what role, how much, and when; the mandatory periodic-report management-holdings table is parsed the same way. Together they mark founder/management shareholders in the Ownership section (skin-in-the-game badge) and feed an insider timeline with rolling 90-day and 12-month net buy/sell aggregates (shown only once at least 2 transactions exist).
- **Red flags**: a per-company panel collects auditor red flags (qualified opinion / going concern), report publication delays (expected calendar date passed with no filing), fund exits (a disclosed holder vanishing from the newest ownership picture), score deteriorations, and short-position spikes — each with a fixed severity, a link to its evidence, and an acknowledge action (acknowledged flags move to history and never re-raise for the same evidence). New flags raise typed signals, so existing alert rules and the morning briefing pick them up.
- **Analyst recommendations** (v0.58, [ADR 0073](adr/0073-analyst-recommendations-tracking.md)): sell-side ratings and target prices tracked strictly as **attributed third-party opinions**. Each entry preserves the source vocabulary verbatim (e.g. "akumuluj") plus a derived direction (upgrade/downgrade/initiate/reiterate vs the same firm's prior entry), target price with currency, price at issuance, publication date, analyst + issuing firm, and a broker-PDF link. History is **append-only and accumulates locally from ingestion start** (the free BiznesRadar page shows only the latest ~3–5 entries; the panel footer states this honestly). A new entry raises the `recommendation_change` typed signal (feed badge, Today/briefing, alert rules). Surfaces: an **opt-in** cockpit panel (every row shows firm + date; summary card = latest target with full attribution) and a "vs target" readout beside the price context that always carries firm + date under the number. Never advice: recommendations are quoted, never aggregated into the app's own stance, and never feed scorecards or valuation.
- Everything is decision support: scores cite their published formulas, flags state facts with evidence, and no surface phrases a buy/sell/hold action or composes the inputs into a single conviction rating.

## Report Documents

The company workspace lists the stored report documents for a company, each shown with its **document kind** — consolidated report, standalone report, audit report, presentation, governance, or other — so the investor can tell a periodic report from supporting material at a glance. By default the list is **grouped by reporting period** (newest first), with the period's **canonical report starred**, its audit reports beside it, and the signature/data companion files folded out of the way; filings that belong to no period (announcements, general-meeting materials, and the like) collect in a separate collapsed group. A **Group by period** toggle returns to a flat chronological list. The list can be **searched** by title and **filtered by kind** (including unclassified), and a **Refresh classification** action re-derives the kind of every stored document at once, for documents captured before the classification existed. Kinds and grouping are decision-support presentation only; the classification is deterministic and never alters the stored document or its source attribution.

## Coverage Map

The Coverage panel gives the investor a per-company **map of what data exists for each reporting period** — one row per fiscal period showing whether a canonical report was found, and how many facts exist for the period with their provenance and validation state. Its purpose is that **gaps are always visible, never silent**: a period with no report, or one no deterministic tier could read, appears explicitly rather than simply going missing, so the user can trust that an empty cell means "nothing here" and not "quietly dropped". Clicking a period opens that period's report documents. **Facts are review-free** ([ADR 0086](adr/0086-aggregator-primary-fundamentals.md)): every fact lands already confirmed and is honestly labeled by origin (source tier + citation) — there is no proposal queue and nothing to ratify. Every writer runs the same accounting-identity/comparative validation regardless of source; a value that fails is flagged, never trusted silently.

The panel's footer carries the two **history actions** that close those gaps in bulk. **Backfill history** fetches the last few years of reports and filings, then automatically extracts the periods it just fetched — the single action behind the acceptance journey "add a company → Backfill → data appears" (the backend chains a **history sweep** onto a completed backfill). **Extract missing periods** runs that sweep on its own, for the case where the documents are already stored and only the extraction is missing (no re-download). A history sweep enqueues extraction for every canonical periodic report whose period still lacks facts, running the deterministic tiers only — no AI, no per-call budget. Both actions require the company to be opted into automation: a company with automation **off** shows the actions disabled with an explicit "automation off" status rather than silently doing nothing. Backfill **depth is configurable** in Settings → Sources (1–10 years, default 3), and when a backfill hits its page cap before reaching that depth the panel reports the **truncation** explicitly ("older filings may be missing") — never a silent gap. While a sweep drains, the status line shows the live extraction counter ("Extracting… {done}/{total}").

**Automatic, deterministic extraction; PDFs are for humans, not fact-scraping** ([ADR 0086](adr/0086-aggregator-primary-fundamentals.md), `v0.59.0`; positional tier retired [ADR 0095](adr/0095-retire-html-positional-tier.md)). Core KPIs arrive on their own: the **BiznesRadar aggregator is the primary source**, pulled daily per tracked company (every period column its statement pages carry), and issuer filings corroborate where they exist — **ESEF/iXBRL** (annual) and the **ESPI cover-note "wybrane dane finansowe" (WDF)** table lifted from the ingested komunikat (the `structured_xhtml` tier value survives only on historical rows — no live writer produces it). The **PDF fact-extraction arm is retired**, and so is the **positional (visual-render) parser** that used to also read visual-only interims — no machine scrapes financial values out of statement PDFs or bare pdf2htmlEX renders anymore; some GPW issuers (CD PROJEKT among them) publish interims only as such a visual XHTML render, and those now arrive, if at all, only from the BiznesRadar-primary pull or an MCP agent's manual read, never a bespoke layout parser. Stored files stay on disk for the investor to read, and PDF text still serves only document period derivation and insider/ownership parsing. Precedence is per slot (`manual` > `esef` > `espi_cover_note` > `html_aggregator`): a strictly higher tier upgrades a lower tier's slot, the aggregator only ever overwrites its own, and **manual values are never touched** by any automatic path. Where an issuer tier holds a value and the aggregator disagrees, the disagreement is recorded informationally and never overwrites the issuer figure. When no tier can place a document, the app **says so** — the period is flagged with a typed reason, never guessed and never silently absent. Coverage numbers therefore describe deterministic coverage only.

**A reported measure is never repainted onto a generic concept** (owner decision, [ADR 0077](adr/0077-trusted-extraction-foundations.md) decision 8). When an issuer reports something that does *not* equal the catalog's generic concept — its own revenue definition, a segment figure, an industry stock like proven/probable reserves — the app **never renames it onto the generic key** to make a chart line up. It is tracked as a **company-scoped KPI**: its own catalog entry under that company, its own history, never folded into the generic metric's series or into a cross-company comparison. What makes a company's set of tracked measures *characteristic* is the selection layer, not a renaming: the KPIs marked primary for that company. The investor curates them, and every company also starts with a small IFRS core set so completeness is measurable from day one. The cost of the rule is an occasional missing generic data point; the benefit is that a number always means what the issuer said it means.

Periods a deterministic tier could not resolve — the gate refused the values, no tier could read the document, drift was detected, or the reporting period itself could not be derived — collect in the Coverage panel's **"Flagged periods"** section (`v0.59.0`; the separate Review-queue pane was removed with the in-app AI layer, [ADR 0084](adr/0084-retire-in-app-ai-layer.md)). Each entry names the period, the tier that attempted it, a plain-language reason, the failing check, and whether the report's layout changed, and offers a **re-run**. An empty section means nothing is flagged — a good state, and one the UI keeps distinguishable from a failed read.

## Company Events Calendar

V1 should include a company-events calendar view for companies in the user's watchlists. The main default goal is to answer: "what company-specific dates should I pay attention to next?" The same screen should also support historical dates so the user can understand what already happened and compare current notes, reports, dividends, and meetings against prior context.

Initial event types should include:

- periodic report publication dates
- dividend-related dates when available
- shareholder meetings, investor conferences, and conference calls when available from accepted sources
- manual events added by the user when sourced calendars are incomplete

The view should be watchlist-first and date-first. By default it should show the current week as working-day columns, with previous/next week navigation and compact company ticker, event type, source, and status. Weekend events may be shown separately only when present. A secondary list view should provide upcoming, historical, all, and custom date-range modes for broader search and review. It is not a portfolio calendar and should not require positions or holdings.

Events created from official or public sources must retain source URL, attribution, fetched timestamp, publication/update timestamp when available, company match, and origin/source type. Manual events must be clearly marked as manual. If an accepted source changes a source-keyed event, source refresh should update the existing sourced event. Manual events are for missing or user-known dates, not normal corrections to source-backed rows.

This milestone depends on either test-sample-backed events or source adapters capable of producing calendar-like events. It should be implemented after the first GPW ingestion work is stable enough to prove the source model.

The **investor week calendar** (`v0.67.0`, [ADR 0058](adr/0058-investor-week-calendar.md)) extends this week view into a "what matters this week" digest by adding composable, opt-in **layers**: company events gain IPO debut (`DEBIUT`) and ex-dividend (`ODCIĘCIE DYWIDENDY`) dates; an opt-in **whole-market** scope shows untracked GPW tickers alongside the watchlist; a **macro** lane shows economic releases (CPI/PMI/payrolls) with time and country (manual + sample first, a policy-clean live source deferred to a follow-up ADR); and **market holidays** mark closed sessions per exchange (`WOLNE`). Watchlist-first stays the default; foreign-company earnings are out of scope. It remains decision support, not advice, and is not a portfolio calendar.

## Intelligence via MCP (BYOA)

The in-app AI analysis layer is **retired** ([ADR 0084](adr/0084-retire-in-app-ai-layer.md)). Brawler is a **deterministic research substrate with an MCP port** — the user brings their own agent (BYOA). Decision support arrives two ways: deterministic computation inside the app (the fundamentals pipeline, ESPI rule classification, company health, red flags, the composed briefing), and the user's own agent working over the local MCP port with a frontier model, user-owned prompts, and full context via typed tools. "AI decision support" as an in-app feature family — summaries, significance labels, feed analysis, claim/KPI proposals, research briefs/digests, qualitative auto-verdicts, briefing prose — is gone; those jobs re-enter as MCP read/write tools where an external agent does them better and writes back with mandatory provenance (`v0.60.0`). The app runs fully featured with **zero API keys**.

Feature losses are explicit and accepted: no in-app claim proposals, no feed-analysis panel, no AI briefing prose, no qualitative auto-verdicts, no OCR coverage for scanned-only documents (now honest flagged gaps, never guessed). Migration 0102 was a clean cut: analysis results, research briefs/digests, and extraction proposals were dropped outright — nothing from the retired feature survives. Saved notes and claims the user created (even ones originating from that AI) remain as ordinary user data. Re-introducing any in-app inference requires a fresh eval-gated ADR that beats the deterministic baseline on real data (the ADR 0080 bar).

### Video transcription — the one remaining in-app AI

Transcription is data acquisition (speech→text), not interpretation, so it stays behind its provider trait. Gemini is preferred for YouTube press-conference transcription because of native vendor support for video/audio and YouTube URL input; real Gemini-backed transcript generation must work for a supported public YouTube URL (offline sample transcripts are only for tests and development). The Gemini transcription model is selectable in Settings and defaults to the cheapest configured model that passed live smoke validation. The transcription timeout is configurable so short provider checks and longer conference videos can use different tolerances. The Gemini key is the **only** provider credential the app asks for, and it is optional. Provider limits and privacy terms are shown in Settings before use.

The video transcription workflow should support:

- entering a YouTube press conference URL in a field labeled `URL`
- optionally providing the ticker/company before transcription
- reserving space for future company recognition from the video/transcript when the user does not provide one upfront
- allowing transcripts to remain unlinked to any company, including general market videos
- offering optional company linking after transcription, using the same local company lookup as Companies
- running a transcription or transcript-like extraction job
- surfacing Gemini rejection/error causes when a direct YouTube URL cannot be processed
- reviewing transcript segments
- selecting whole transcript segments to add to a specific company's notebook when the transcript is linked to a company
- preserving the YouTube URL, timestamp range when available, provider, and created note origin

Any decision support — whether the app's deterministic analytics or an external agent over MCP — must be presented as decision support. It must not contain direct buy/sell/hold recommendations. Agent output arriving over MCP is the user's own agent working their own prompts; the app's contribution stays deterministic and source-cited.

There is no in-app AI provider/model selection ([ADR 0084](adr/0084-retire-in-app-ai-layer.md)). Honest failure reporting replaces guessed diagnoses: the deterministic pipeline emits typed reason codes (e.g. `no_deterministic_tier`) rendered through the translation layer, so distinct causes stay distinguishable.

## Settings, Export, And Local Data

The Settings panel edits runtime settings stored locally. English is the default app language, and Settings should let the user switch the app UI to Polish. Locale handling should be an extensible app-locale boundary so future supported languages can be added without rewriting screens. Locale changes affect app-owned UI copy and formatting labels; source-provided text, company names, ticker symbols, URLs, source attribution, transcript text, and notebook bodies keep their original or user-entered language.

Normal user-facing UI should use product language and avoid implementation architecture language. Do not show terms such as SQLite, database engine, Tauri, internal adapter, module, collector, schema, or command boundary in ordinary app copy. Use terms like local data, sources, activity, settings, logs, and diagnostics where the user needs to understand behavior. Technical implementation wording is acceptable in Developer Diagnostics, owner/developer documentation, and code-level contracts.

YAML is accepted as the future import/export/bootstrap format for non-secret settings, but YAML implementation is deferred until the later export/import/backup work. API keys and provider secrets are stored in the OS keychain and must never be exported to YAML.

App data lives in the OS app data directory by default, with development-only override support. V1 uses local logs and local Developer-mode metrics only, with no telemetry. Settings expose runtime log level and rotation limits. Developer mode Diagnostics may show local metrics, a full local log viewer, and open the app-owned logs folder for troubleshooting.

Export is part of normal v1 implementation. M20 exports companies, watchlists, watchlist memberships, notebook entries, and non-secret settings. Research data uses structured JSON. Settings use YAML. Import preview validates supported files before applying changes.

Global search (delivered in `v0.38.0`) lets the user find anything stored locally from one search box in the top toolbar — companies, watchlists, feed items, notes, transcript text, and company events — with ranked results grouped by type and a snippet, and navigation to the matching item. Search is local-only and covers the user's own content.

Automatic local backups (delivered in `v0.38.0`) keep recent copies of the local data so it can be recovered. The app takes a safety copy before any data-structure upgrade and keeps a rotating set of recent backups; if a safety copy cannot be written, the upgrade is stopped. Restore is offered from Developer Diagnostics with explicit confirmation and is applied when the app restarts. Backups stay on the machine and never contain stored API keys. A broader full app-data bundle and hosted data services remain later design discussions.

The Settings Database section exposes advanced performance tuning (how many simultaneous connections the local data store uses and how long operations wait under contention) with safe defaults, range limits, and a reset control; changes apply on the next launch. Like the Logs section, this is an intentionally technical area; ordinary screens keep plain language.

## Post-v1 Directions

Delivered behavior once narrated here (research workspace, company fundamentals, report documents/history backfill, report-over-report diff) is now canonical in [UI Flows](ui-flows.md), [Contracts](contracts.md), [Data Model](data-model.md), and [ADR 0022](adr/0022-research-evidence-read-model-boundary.md) / [0027](adr/0027-company-fundamentals-scope.md) / [0036](adr/0036-report-document-storage-and-backfill.md) / [0040](adr/0040-management-claims-tracker.md) / [0052](adr/0052-report-over-report-diff.md) / [0061](adr/0061-deterministic-fundamentals-data-gathering.md); its execution chronicle moved to [Kanban Archive](kanban-archive.md#archived-investigation-and-study-notes-moved-2026-07-02).

Remaining post-v1 exploration (terminal interface, mobile and sync) is tracked in [Roadmap](roadmap.md#future-exploration-terminal-interface) and [Roadmap](roadmap.md#future-exploration-mobile-and-sync) — see there, not here.

## Open-Core Posture

Brawler uses an open-core posture. The desktop core is open source under the Mozilla Public License 2.0 and should remain useful without payment. Future hosted services, premium integrations, official distribution infrastructure, gated features, or support may be licensed separately.

Detailed owner-only monetization strategy belongs in the private sibling repository when available locally. Public product docs should describe the posture at a high level without exposing speculative business plans.

Entitlement mechanics (license status model, gated-state handling) are specified in [Contracts § Entitlements](contracts.md#entitlements); the AI/open-core boundary and named future paid areas are recorded in [ADR 0027](adr/0027-company-fundamentals-scope.md).
