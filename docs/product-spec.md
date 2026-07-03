# Product Spec

Doc map: [CLAUDE.md](../CLAUDE.md) § Required Reading. Related references: [UI Flows](ui-flows.md), [UI Information Architecture](ui-information-architecture.md), [Source Strategy](source-strategy.md), and [Contracts](contracts.md).

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

The **investor week calendar** (`v0.59.0`, [ADR 0058](adr/0058-investor-week-calendar.md)) extends this week view into a "what matters this week" digest by adding composable, opt-in **layers**: company events gain IPO debut (`DEBIUT`) and ex-dividend (`ODCIĘCIE DYWIDENDY`) dates; an opt-in **whole-market** scope shows untracked GPW tickers alongside the watchlist; a **macro** lane shows economic releases (CPI/PMI/payrolls) with time and country (manual + sample first, a policy-clean live source deferred to a follow-up ADR); and **market holidays** mark closed sessions per exchange (`WOLNE`). Watchlist-first stays the default; foreign-company earnings are out of scope. It remains decision support, not advice, and is not a portfolio calendar.

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

**Per-capability AI provider routing** (ADR 0060 as amended, ADR 0061 decision 5) lets the user route each distinct AI call site (KPI extraction, claim extraction, feed analysis, research brief, research digest, ESPI event-date, ESPI signal classification) to its own **ordered fallback pool** of providers/models, instead of one global provider for everything — e.g. a document-capable provider for KPI extraction and a free open-model host for text analysis. A capability with no configured pool keeps using the single general AI provider, so the feature is fully backward-compatible.

- **When failover happens:** only when a pool member is genuinely unavailable (rate-limited, erroring, timing out, or unreachable) does the app try the next provider in the list. A response that comes back successfully but with content the app cannot use is never treated as a reason to try another provider — that would hide a real problem (a bad prompt, a parsing bug) behind an unrelated provider's answer.
- **Never silently wrong:** pool failover only improves *availability*. Every fact the pool helps produce still passes the same validation it would from a single provider — routing to a fallback never lowers the bar for what counts as trustworthy output.
- **The generic OpenAI-compatible provider** unlocks free/self-hosted open-model hosts (Groq, OpenRouter, Together, Cerebras, local Ollama, and others) that speak the OpenAI chat-completions format, via one user-configured base URL — see `wiki/ai-provider-presets.md` for ready-to-use host presets.
- **No-provider behavior:** when a feed-item AI analysis is started with nothing configured (no general provider, no capability pool), the job now fails with a clear error at run time rather than silently substituting a test-sample result — a deliberate behavior change from the prior silent fallback. KPI extraction, claim extraction, research brief, and research digest jobs are unaffected: they keep pinning the deterministic test-sample provider at enqueue time when nothing is configured, so those workflows still run out of the box.

## Settings, Export, And Local Data

The Settings panel edits runtime settings stored locally. English is the default app language, and Settings should let the user switch the app UI to Polish. Locale handling should be an extensible app-locale boundary so future supported languages can be added without rewriting screens. Locale changes affect app-owned UI copy and formatting labels; source-provided text, company names, ticker symbols, URLs, source attribution, transcript text, and notebook bodies keep their original or user-entered language.

Normal user-facing UI should use product language and avoid implementation architecture language. Do not show terms such as SQLite, database engine, Tauri, internal adapter, module, collector, schema, or command boundary in ordinary app copy. Use terms like local data, sources, activity, settings, logs, and diagnostics where the user needs to understand behavior. Technical implementation wording is acceptable in Developer Diagnostics, owner/developer documentation, and code-level contracts.

YAML is accepted as the future import/export/bootstrap format for non-secret settings, but YAML implementation is deferred until the later export/import/backup work. API keys and provider secrets are stored in the OS keychain and must never be exported to YAML.

App data lives in the OS app data directory by default, with development-only override support. V1 uses local logs and local Developer-mode metrics only, with no telemetry. Settings expose runtime log level and rotation limits. Developer mode Diagnostics may show local metrics, a full local log viewer, and open the app-owned logs folder for troubleshooting.

Export is part of normal v1 implementation. M20 exports companies, watchlists, watchlist memberships, notebook entries, and non-secret settings. Research data uses structured JSON. Settings use YAML. Import preview validates supported files before applying changes.

Global search (delivered in `v0.38.0`) lets the user find anything stored locally from one search box in the top toolbar — companies, feed items, notes, transcript text, research briefs, and digests — with ranked results grouped by type and a snippet, and navigation to the matching item. Search is local-only and covers the user's own content.

Automatic local backups (delivered in `v0.38.0`) keep recent copies of the local data so it can be recovered. The app takes a safety copy before any data-structure upgrade and keeps a rotating set of recent backups; if a safety copy cannot be written, the upgrade is stopped. Restore is offered from Developer Diagnostics with explicit confirmation and is applied when the app restarts. Backups stay on the machine and never contain stored API keys. A broader full app-data bundle and hosted data services remain later design discussions.

The Settings Database section exposes advanced performance tuning (how many simultaneous connections the local data store uses and how long operations wait under contention) with safe defaults, range limits, and a reset control; changes apply on the next launch. Like the Logs section, this is an intentionally technical area; ordinary screens keep plain language.

## Post-v1 Directions

Delivered behavior once narrated here (research workspace, company fundamentals, report documents/history backfill, report-over-report diff) is now canonical in [UI Flows](ui-flows.md), [Contracts](contracts.md), [Data Model](data-model.md), and [ADR 0022](adr/0022-research-evidence-read-model-boundary.md) / [0027](adr/0027-company-fundamentals-scope.md) / [0036](adr/0036-report-document-storage-and-backfill.md) / [0040](adr/0040-management-claims-tracker.md) / [0052](adr/0052-report-over-report-diff.md) / [0061](adr/0061-deterministic-fundamentals-data-gathering.md); its execution chronicle moved to [Kanban Archive](kanban-archive.md#archived-investigation-and-study-notes-moved-2026-07-02).

Remaining post-v1 exploration (terminal interface, mobile and sync) is tracked in [Roadmap](roadmap.md#future-exploration-terminal-interface) and [Roadmap](roadmap.md#future-exploration-mobile-and-sync) — see there, not here.

## Open-Core Posture

Brawler uses an open-core posture. The desktop core is open source under the Mozilla Public License 2.0 and should remain useful without payment. Future hosted services, premium integrations, official distribution infrastructure, gated features, or support may be licensed separately.

Detailed owner-only monetization strategy belongs in the private sibling repository when available locally. Public product docs should describe the posture at a high level without exposing speculative business plans.

Entitlement mechanics (license status model, gated-state handling) are specified in [Contracts § Entitlements](contracts.md#entitlements); the AI/open-core boundary and named future paid areas are recorded in [ADR 0027](adr/0027-company-fundamentals-scope.md).
