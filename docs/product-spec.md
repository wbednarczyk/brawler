# Product Spec

Use [Project Brief](project-brief.md) for the full documentation map. Related references: [UI Flows](ui-flows.md), [UI Information Architecture](ui-information-architecture.md), [Source Strategy](source-strategy.md), and [Contracts](contracts.md).

## V1 Experience

The first screen is an investor inbox: a dense chronological feed of company-specific reports and news. It should support repeated daily use and fast scanning. Each company also has a notebook for durable research notes.

Early sample feed implementations should already obey local watchlist, company, unread, and saved filters, even before real source ingestion exists.
Sample feed implementations may keep read/saved changes in memory, but stored feed items must persist read and saved state locally.
The UI-facing Inbox is scoped to tracked companies. Raw or sample source items that do not match the local company registry may exist internally, but they should not appear in the normal feed until the company is added.
Source URLs in item details must be directly actionable so the user can verify the original report or article quickly.
Feed details should show a short summary first. `summary` is separate from the report title and full body, but when no source or AI summary is available the UI may use the report title as the summary fallback. Full official report body text should be available in the same detail pane but collapsed by default and expanded on demand.
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

The feed detail pane is a fixed-width side rail and is treated as a containment boundary: it shows compact, read-only summaries plus launchers, never wide interactive controls. Rich flows — AI analysis (preset prompts, custom question, full result) and AI KPI extraction (source selection, capture-by-URL, IR fallback, per-value review and confirmation) — open in the centered modal rather than rendering inline in the rail. The rail shows a status pill and a one-line summary (for completed AI analysis, a significance + summary preview), with a button to open the full flow. This keeps the rail readable at the supported narrow window range and prevents a wide descendant from clipping the pane. New rail panels render inside the `DetailSection` primitive, which bakes in the containment contract so the rail cannot regress.
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

Each company has a notebook tied to its canonical company identity. Notes should support manual entry and creation from feed items, reports, transcripts, or AI-suggested excerpts.

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

As of `v0.42.0` ([ADR 0040](adr/0040-management-claims-tracker.md)) the company workspace Claims tab is the **management claims tracker** over a first-class claim entity (no longer a filtered view of notebook entries; existing claim notes are migrated). A claim carries the management statement, a due period (fiscal year + period type, quarters emphasized), an optional quantitative target, source evidence (report document, transcript segment, feed item, or manual), and a user-set verdict (`pending`/`delivered`/`partially_delivered`/`missed`/`revised`). The tracker has three parts: (1) a **claims list** with verdict and due/overdue status; (2) a **review queue** ("claims to verify") that resurfaces a claim when its due-period report arrives — for a quantitative claim the matching confirmed financial fact is shown beside it so the user can resolve the verdict against evidence; (3) **AI claim extraction** from a report document or transcript, where proposed claims require explicit user confirmation before any claim is created (mirroring KPI extraction). There are no automated verdicts: the app surfaces the evidence and the user decides. Claims remain research evidence (they appear in the company research timeline and feed digests/reminders) and are owner-durable in import/export.

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

Feed retention must be designed before v1 ingestion becomes broad. The app should avoid unbounded local growth by defining per-source retention defaults, user-adjustable cleanup settings, and rules that preserve important user-marked content. Saved items, items linked to notes, and items with AI analysis or explicit user decisions should not be removed by routine cleanup without clear user control.

## Typed Filing Signals

Official ESPI/EBI filings should be classified into typed disclosure signals so the feed reads as a signal stream instead of homework. The investor should be able to tell at a glance whether a filing is an insider transaction, a dividend, a profit warning or result estimate, a significant contract, an own-share transaction, or a guidance change.

Behavior:

- A deterministic rule classifier runs during ingestion over the filing's official category label, title, and body, and types the formulaic majority of filings automatically.
- Filings the rules cannot place are left untyped, or — when the optional AI fallback is enabled — produce a proposed type that the user must confirm before it is applied. The app never silently assigns a wrong type.
- Typed filings show a type badge in the Inbox and company feed, and the feed can be filtered by type.
- The digest groups high-signal types (for example, insider activity) so the user sees them together, and high-signal types can drive reminders.
- A typed filing that carries a real future date (such as a dividend record/payment date or a general-meeting date) also appears as a company event in the calendar. Past disclosures stay in the feed and do not create calendar entries.

Typed signals are decision support, not recommendations: a signal states what kind of disclosure occurred, with a link back to the official filing, and never implies a buy/sell/hold action.

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

## AI Analysis

The first AI milestone is summarization and classification:

- concise summary
- significance label
- topic tags
- reasoning
- source references

Gemini should be preferred only for YouTube press conference transcription because of native vendor support for video/audio and YouTube URL input. M10 must end with real Gemini-backed transcript generation working for at least one supported public YouTube URL; offline sample transcripts are only for tests and development. The Gemini transcription model is selectable in Settings and defaults to the cheapest configured model that passed live smoke validation. The Gemini transcription timeout is also configurable in Settings so short provider checks and longer conference videos can use different tolerances. Other AI workflows, including summaries, significance labels, and note extraction, have no preferred provider yet. Provider limits and privacy terms must be shown in settings before use.

The first video AI workflow should support:

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

AI output must be presented as decision support. It must not contain direct buy/sell/hold recommendations.

Default AI analysis mode is source-grounded. A future opinionated mode may be added behind explicit user opt-in, but it must remain source-cited and must not provide buy/sell/hold or personalized portfolio advice.

Visible AI analysis panels should refresh status automatically while the latest selected job is queued or running. The user should not need a dedicated refresh button to see an in-progress analysis complete.

## Settings, Export, And Local Data

The Settings panel edits runtime settings stored locally. English is the default app language, and Settings should let the user switch the app UI to Polish. Locale handling should be an extensible app-locale boundary so future supported languages can be added without rewriting screens. Locale changes affect app-owned UI copy and formatting labels; source-provided text, company names, ticker symbols, URLs, source attribution, transcript text, and notebook bodies keep their original or user-entered language.

Normal user-facing UI should use product language and avoid implementation architecture language. Do not show terms such as SQLite, database engine, Tauri, internal adapter, module, collector, schema, or command boundary in ordinary app copy. Use terms like local data, sources, activity, settings, logs, and diagnostics where the user needs to understand behavior. Technical implementation wording is acceptable in Developer Diagnostics, owner/developer documentation, and code-level contracts.

YAML is accepted as the future import/export/bootstrap format for non-secret settings, but YAML implementation is deferred until the later export/import/backup work. API keys and provider secrets are stored in the OS keychain and must never be exported to YAML.

App data lives in the OS app data directory by default, with development-only override support. V1 uses local logs and local Developer-mode metrics only, with no telemetry. Settings expose runtime log level and rotation limits. Developer mode Diagnostics may show local metrics, a full local log viewer, and open the app-owned logs folder for troubleshooting.

Export is part of normal v1 implementation. M20 exports companies, watchlists, watchlist memberships, notebook entries, and non-secret settings. Research data uses structured JSON. Settings use YAML. Import preview validates supported files before applying changes.

Global search (delivered in `v0.38.0`) lets the user find anything stored locally from one search box in the top toolbar — companies, feed items, notes, transcript text, research briefs, and digests — with ranked results grouped by type and a snippet, and navigation to the matching item. Search is local-only and covers the user's own content.

Automatic local backups (delivered in `v0.38.0`) keep recent copies of the local data so it can be recovered. The app takes a safety copy before any data-structure upgrade and keeps a rotating set of recent backups; if a safety copy cannot be written, the upgrade is stopped. Restore is offered from Developer Diagnostics with explicit confirmation and is applied when the app restarts. Backups stay on the machine and never contain stored API keys. A broader full app-data bundle and hosted data services remain later design discussions.

The Settings Database section exposes advanced performance tuning (how many simultaneous connections the local data store uses and how long operations wait under contention) with safe defaults, range limits, and a reset control; changes apply on the next launch. Like the Logs section, this is an intentionally technical area; ordinary screens keep plain language.

## Future Experience Directions

These ideas are intentionally out of v1 scope, but should influence architectural choices where the cost is low.

### Research Workspace

A future product-differentiation direction is to make the app a personal research memory system for public companies, not only a feed reader. The goal is to help the user answer what changed, why it matters, what management said before, and what should be checked next.

Candidate capabilities:

- company change timeline combining feed items, official reports, media items, notes, transcripts, claims, calendar events, and future AI outputs
- "what changed since last review" views for a company, watchlist, or time window
- expanded management claim tracking with source, expected period, follow-up date, related future events, and status history
- source-grounded company or watchlist research brief with cited evidence and no buy/sell/hold recommendations
- open research questions or threads per company, linked to notes, claims, events, and source items
- watchlist review mode that guides the user company-by-company through unread items, upcoming events, unresolved claims, and open questions
- event-aware reminders such as unresolved claims tied to an upcoming reporting period
- source quality and trust signals that distinguish official reports, company publications, media articles, opinion, paid research, and other source types
- daily or weekly personal research digest generated from the user's local watchlists and source items
- evidence linking between related source items, notes, claims, transcripts, questions, AI briefs, and events

This direction should not be implemented as ten separate isolated screens. Before implementation, a dedicated planning milestone should define a shared research evidence model, timeline/read-model boundary, review-session model, evidence-linking model, AI brief-building boundary, storage implications, and import/export impact. Existing notebooks, claims, feed items, transcripts, and future events should plug into this model through stable contracts instead of direct cross-screen coupling.

M24 accepts that research-workspace features should be built on a dedicated research/evidence boundary. Existing feed items, notebook entries, claims, transcript segments, events, AI analysis results, watchlists, and sources remain canonical in their owning domains. Research views consume backend-owned evidence and timeline read models rather than independently combining unrelated screen APIs. Durable research-owned state starts with review checkpoints and typed evidence links; full stored timeline projections wait until performance or review semantics require them.

M31 adds event-aware research reminders and on-demand personal research digests. Reminders are research-owned review pressure derived from open claims, upcoming events, and open research questions, plus explicit manual reminders. Digests are immutable, cited AI snapshots generated from backend-collected changed evidence and open reminders for the selected company or watchlist.

M25 adds the first visible Research screen as a top-level navigation item. The first slice is company-scoped: the user selects a tracked company and sees a newest-first evidence timeline combining feed items, notebook entries, claims, events, transcript segments, and AI analysis through the backend research read model. The screen header shows the selected company, last reviewed state, total visible evidence, and changed-since-review count. Evidence type filters and a changed-only filter are sent to the backend; the UI renders the returned timeline result instead of recomputing review counts.

The M25 review action is a single company-level `Mark reviewed` action. It stores a research review checkpoint for the selected company and refreshes the timeline so changed-since-review state is updated. Evidence rows should use product-language labels such as report, note, claim, event, transcript, or AI analysis instead of raw implementation identifiers. Where practical, an evidence row should offer an owning-domain action such as opening the source URL, opening the Inbox item, or moving to the relevant app area.

M26 adds watchlist review mode inside the same Research screen. The user can switch between company and watchlist review, select a watchlist, and work through member companies using a compact company queue with backend-owned changed evidence counts. Marking a watchlist reviewed updates only the watchlist checkpoint by default. The UI exposes an explicit option to also mark current member companies reviewed, but the cascade behavior is owned by the backend command.

M29 adds company-scoped research questions inside the Research screen. A question is a durable research-owned item with title, optional context, and `open`, `answered`, or `closed` status. The user can select a company, create questions for that company, select a question, delete a question, and link visible evidence rows to it through typed evidence links. Questions are shown in the company evidence timeline and imported/exported with research data. Watchlist-scoped questions remain a backend-compatible extension point, but normal UI creates company questions only until a dedicated watchlist-question workflow is designed.

AI research briefs are separate research entities with citations and provider/model/prompt provenance. They are not ordinary notebook entries, though a future workflow may let the user create a note from a brief or selected excerpt.

M30 adds on-demand AI research briefs for company and watchlist research scopes. Brief generation uses backend-owned evidence collection, prompt/context building, provider execution, citation mapping, rendering, and persistence boundaries. Generated briefs are immutable snapshots with visible citations and provenance. They are imported/exported with research data. Briefs must not contain buy/sell/hold recommendations, and creating a notebook note from a brief is not automatic.

The management claims tracker (`v0.42.0`, [ADR 0040](adr/0040-management-claims-tracker.md)) delivers the "expanded management claim tracking" and "unresolved claims tied to an upcoming reporting period" capabilities above. Claims become a first-class entity with a due period, quantitative target, source evidence, and a user-set verdict. AI extraction proposes claims from report documents and transcripts for mandatory confirmation. A due-period derivation job resurfaces an open claim into a "claims to verify" review queue when the due period's report arrives, attaching the matching confirmed financial fact for quantitative claims; the user resolves the verdict against evidence. Claims plug into the existing research evidence boundary (timeline, evidence links, reminders, digests) and remain owner-durable in import/export. There are no automated verdicts.

### Company Fundamentals

Milestones v0.34.0–v0.37.0 add a fundamentals view for tracked companies: key financial figures pulled from quarterly and annual reports, tracked per reporting period, and charted over time. The intent is to cut the time an investor spends re-reading reports to find the same handful of numbers each quarter.

User-facing behavior:

- a fundamentals panel in the company workspace presenting figures as a KPI-row × reporting-period-column matrix, where every value links back to the report it came from; a fact's detail shows its period, value, source, and status
- figures are shown the way the report printed them (e.g. `319,7 mln PLN`, `8,63 PLN`) using the as-reported figure captured at extraction, falling back to a locale-aware formatted base value for manually entered numbers; KPI names are localized
- a fixed set of standard figures (revenue, operating profit, net profit, EBITDA, EPS, gross/operating/net margin, net debt, cash) plus the ability to define custom figures for a company that the standard set does not cover, such as subscribers, stores, or order backlog; the add-fact KPI picker supports inline search
- AI assistance that reads a stored report and proposes figures, reviewed in a centered modal where the user confirms, edits, or rejects each value, with bulk "confirm all known" and "accept all suggestions" actions; the modal auto-closes when nothing is left to review and no AI-proposed number is stored as a confirmed figure without review
- the user can also enter and correct figures manually against a report
- inline sparkline trends per KPI plus a larger per-KPI trend chart over comparable periods (built with in-house SVG primitives, no charting dependency), and (in v0.53.0) side-by-side comparison of the same figure across companies
- the panel and the review modal remain usable in tall, narrow windows (e.g. a quarter of an ultrawide monitor), stacking or shrinking rather than clipping

Scope boundary: this covers report-derived fundamentals only. Price and volume charts, technical indicators, valuation tooling that needs live prices, and market dashboards stay out of scope, as recorded in [ADR 0027](adr/0027-company-fundamentals-scope.md). AI fundamentals features are part of the open core and free to use with a user-supplied provider API key.

### Report Documents And History Backfill

Milestone v0.41.0 persists the actual report files and removes the cold-start problem when tracking a company ([ADR 0036](adr/0036-report-document-storage-and-backfill.md)).

User-facing behavior:

- official ESPI/EBI report attachments are stored locally as report documents with durable attribution and linked from the company's evidence/timeline; periodic/financial reports keep the full file (so AI extraction and diff have a real document to cite), while routine filings keep only the link and title
- the existing escape hatch — pasting a report PDF URL, or resolving one from the company's IR page — still stores the full file and flows through the same downstream path
- an explicit **"Backfill history"** action on a tracked company (on track, or from the company workspace) fetches roughly the last 3 years of periodic reports and ESPI/EBI filings, so the company's research timeline is populated with prior years instead of starting at "now"; items appear with their original publication dates
- backfill shows progress and diagnostics while it runs, can be cancelled, and is safe to re-run (no duplicates); it never runs automatically and only runs while the app is open
- dividend and general-meeting filings that state a future date produce a **proposed** calendar event the user confirms before it appears on the calendar; a date is never guessed onto the calendar

Scope boundary: backfill covers official report sources only (not media), does not backfill historical calendar entries (the calendar focuses on upcoming events), and does no per-company PDF parsing or ESEF/iXBRL parsing.

### Report-Over-Report Diff

Milestone v0.47.0 lets the investor see what actually changed between two consecutive periodic **financial statements** instead of rereading the whole filing ([ADR 0052](adr/0052-report-over-report-diff.md)).

User-facing behavior:

- from a company's report documents (and on new periodic-report arrival), the investor can open a **section-by-section diff** of a financial statement against the previous same-type filing (consolidated SSF vs the prior SSF, standalone JSF vs the prior JSF)
- each section is shown as unchanged, changed, or only-in-one-report; changed sections show the textual differences, with both reports cited so the investor can jump to the source
- the diff is **deterministic and fully local** — pure-Rust text extraction, no AI and no network; the same two reports always produce the same diff, and a report compared against itself shows no changes
- a report whose PDF has no extractable text layer (scanned) shows an explicit "can't diff — no extractable text" state rather than a misleading empty diff

Scope boundary: v0.47.0 diffs the **structured financial statements only**. The narrative management report (MD&A) diff and an AI-written "what changed / new risks / tone shift" delta summary are deferred to a later milestone — a real-data spike showed narrative section headings drift too much for trustworthy deterministic alignment ([ADR 0052](adr/0052-report-over-report-diff.md)). Financial-table *value* reconciliation stays with KPI extraction, not the diff. No cross-company diffing.

### Terminal Interface

A future terminal/TUI version may provide a keyboard-first investor research experience. It should reuse the core local domain and storage model instead of becoming a separate product.

The intended feeling is:

- loosely similar to `k9s` in navigation density, speed, and operational ergonomics
- retro terminal style adapted to the Brawler night-neon palette
- dark, high-contrast blue, pink, purple, and cyan accents
- fast watchlist/feed/company switching
- keyboard-first commands for reading, filtering, saving, and opening notes
- optional synthwave-style background music as an explicit opt-in ambience feature

The TUI should remain useful without sound, animation, or decorative effects. Music must never start automatically.

### Mobile And Sync

A much later product direction may include mobile clients with data sync across desktop and mobile devices.

This is not part of v1 and requires separate design work for:

- sync ownership and hosting model
- encryption and key management
- conflict resolution
- offline-first behavior
- product, distribution, or support implications
- mobile UX scope versus desktop parity
- privacy policy and data deletion guarantees

Until that design exists, v1 remains local-first and single-device.

## Open-Core Posture

Brawler uses an open-core posture. The desktop core is open source under the Mozilla Public License 2.0 and should remain useful without payment. Future hosted services, premium integrations, official distribution infrastructure, gated features, or support may be licensed separately.

Detailed owner-only monetization strategy belongs in the private sibling repository when available locally. Public product docs should describe the posture at a high level without exposing speculative business plans.

The local entitlement module validates signed license tokens with embedded public verification material, stores the raw accepted token in the OS keychain, stores only derived metadata in SQLite, and does not require cloud accounts, telemetry, hosted activation, billing, or remote entitlement checks. The open desktop core remains usable without a license token. Missing, invalid, expired, tampered, unsupported-version, and unsupported-channel states must be clear and recoverable in Settings for gated entitlements.

AI features, including fundamentals extraction, are part of the open core and free to use with a user-supplied provider API key. The named future paid areas are managed AI (provider access without the user supplying a key), cloud sync and backup, and official signed installers. These are direction only; pricing and packaging detail stay in the private sibling repository. See [ADR 0027](adr/0027-company-fundamentals-scope.md).
