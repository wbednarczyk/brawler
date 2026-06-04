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

Users can maintain multiple watchlists. Companies can be assigned to and removed from watchlists without deleting the company from the local registry. Watchlists should be useful as filters in the Inbox, Events/Calendar, Companies, and Notebooks workspaces. The durable watchlist management workflow belongs in a dedicated Watchlists panel inside the Companies section. That panel owns creating and deleting watchlists and adding/removing companies from the selected watchlist. Company list rows and the company workspace should only show existing watchlist memberships for scanning and context; they should not expose watchlist create, delete, add, or remove controls. Companies are displayed ticker-first, but canonical storage uses exchange-qualified tickers such as `GPW:CDR` or `NASDAQ:MSFT`.

Watchlists are user-owned company groups. Normal UI copy should describe them in those terms and should not expose storage or architecture language. Future premium alerts may use watchlists as configuration inputs, but alerting is outside the dedicated watchlist-management milestone.

Exchange-qualified ticker labels should visually distinguish the exchange segment from the symbol segment across the app through a shared renderer. The initial exchange color map covers GPW and reserves clear paths for future NASDAQ/NYSE support without changing storage contracts.

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

The company workspace Claims tab is a follow-up surface over notebook entries, not a separate data model. It lists entries where `kind` is `claim` or a claim status is set, expands claim details in place, and lets the user update the claim status while preserving note body, tags, origin, and company ownership.

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

The Sources screen shows locally configured sources, supported markets, fetch mode, enabled state, poll interval, and last success or error status. Before real ingestion exists, this screen still reads the local source registry so source monitoring has a stable UI home.
Source rows should follow the app-wide row interaction rule: the row is the primary click target, and source operational details expand inline under the selected source row. Enter or Space on a focused source row should expand or collapse the same inline detail.
The topbar should expose a compact source status entry point that summarizes source readiness and opens the Sources screen. Opening source status should expand the most relevant source immediately, preferring sources with errors, then enabled sources, then the first configured source. This is separate from manual source refresh; before real ingestion exists, refresh remains disabled while status inspection is available.

## Ingestion

The default polling interval is 15 minutes while the app is open. Manual source refresh must be supported.
Before real source ingestion exists, the topbar source refresh control is a disabled placeholder. It must eventually trigger or enqueue source refresh jobs.

A local data status indicator may expose a small view refresh action for development and recovery ergonomics. That action reloads local app state such as feed items, companies, watchlists, memberships, and local data status, but it is not the product-level news/source refresh workflow.

Ingestion should preserve source attribution, publication time, fetch time, original language, matched company, and source URL.

Feed retention must be designed before v1 ingestion becomes broad. The app should avoid unbounded local growth by defining per-source retention defaults, user-adjustable cleanup settings, and rules that preserve important user-marked content. Saved items, items linked to notes, and items with AI analysis or explicit user decisions should not be removed by routine cleanup without clear user control.

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

Export is part of normal v1 implementation. Notes should export as Markdown with metadata, and watchlists/companies/settings should export as structured JSON or YAML. Import/restore and full local backup are late-v1 items. Cloud backup/sync requires a later design discussion.

## Future Experience Directions

These ideas are intentionally out of v1 scope, but should influence architectural choices where the cost is low.

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
- subscription or monetization implications
- mobile UX scope versus desktop parity
- privacy policy and data deletion guarantees

Until that design exists, v1 remains local-first and single-device.

## Monetization

The app should leave room for future monetization, but the model is undecided. Open core plus paid convenience features is one possible path, but not a committed direction. Potential paid features could include packaged builds, sync, backups, managed AI configuration, and notifications.

Brawler is all rights reserved for now. The exact license, monetization model, and commercial boundary require a future ADR before public release, accepting external contributions, or publishing release artifacts.

V1 author and friend-test builds require a local offline license gate before normal app use. The gate validates signed license tokens with embedded public verification material, stores the raw accepted token in the OS keychain, stores only derived metadata in SQLite, and does not require cloud accounts, telemetry, hosted activation, billing, or remote entitlement checks. Missing, invalid, expired, tampered, unsupported-version, and unsupported-channel states must be clear and recoverable.
