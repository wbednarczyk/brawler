# Product Spec

See also [Project Brief](project-brief.md), [UI Flows](ui-flows.md), [UI Information Architecture](ui-information-architecture.md), [Source Strategy](source-strategy.md), [Architecture](architecture.md), [Contracts](contracts.md), and [Kanban](kanban.md).

## V1 Experience

The first screen is an investor inbox: a dense chronological feed of company-specific reports and news. It should support repeated daily use and fast scanning. Each company also has a notebook for durable research notes.

Early fixture feed implementations should already obey local watchlist, company, unread, and saved filters, even before real source ingestion exists.
Fixture feed implementations may keep read/saved changes in memory, but stored feed items must persist read and saved state in SQLite.
Source URLs in item details must be directly actionable so the user can verify the original report or article quickly.
When filters hide all feed items, the UI must offer a quick way to clear active filters and return to the full feed.

Expected v1 UI areas:

- feed list with newest items first
- watchlist selector
- company filter
- search across visible feed items
- source/type filter
- unread/read state
- saved items
- item detail pane with source attribution and original link
- action to create a company notebook note from a feed item
- manual refresh control
- in-app badges for new/unread items

Desktop notifications are out of scope for v1. Portfolio positions, cost basis, and trading workflows are out of scope.

Keyboard shortcuts are in v1 scope as late workflow polish. They should speed up repeated inbox and research actions, but every shortcut action must remain available through visible UI controls. Shortcuts must be discoverable in the app and must not interfere with text editing in search fields, note editors, forms, or transcript selection workflows.

## Theme And Visual Direction

The app must support user-selectable dark and light themes. Dark theme is the default.

The dark visual direction is inspired by the attached blue, pink, and purple night landscape reference:

- deep navy and near-black surfaces
- electric blue/cyan primary accents
- pink and purple secondary accents
- restrained glow or highlight effects for active states, important badges, and focus rings
- high-contrast text suitable for dense investor workflows

The palette should support a serious work-focused desktop app. Use the reference colors for accents and atmosphere, not as a full-screen decorative background for the main product UI.

Light theme should preserve the same brand accent colors while using readable light surfaces and accessible contrast.

## Watchlists And Companies

Users can maintain multiple watchlists. Companies can be assigned to and removed from watchlists without deleting the company from the local registry. Company list rows should make existing watchlist memberships visible at a glance. Companies are displayed ticker-first, but canonical storage uses exchange-qualified tickers such as `GPW:CDR` or `NASDAQ:MSFT`.

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

The first implementation may use local fixtures for GPW metadata. Later implementations should replace or extend fixtures with a source-specific company registry adapter and preserve metadata provenance.

## Company Notebooks

Each company has a notebook tied to its canonical company identity. Notes should support manual entry and creation from feed items, reports, transcripts, or AI-suggested excerpts.

Notebook entries should support:

- title
- Markdown body
- tags
- source/provenance links
- optional event date
- optional review date
- optional quarter or reporting period
- status for claims that should be checked later

The first claim-tracking workflow should support management statements such as "the board said X should happen in the near future" and later review whether the company delivered after one or more quarters. Claim review supports both a review quarter and an exact review date, with quarters emphasized in the UI.

## Sources

V1 focuses on GPW:

- GPW ESPI/EBI official reports
- selected public or RSS media sources where usage is allowed

Later sources should be possible through adapters:

- SEC EDGAR submissions and XBRL APIs
- Nasdaq RSS feeds
- major European exchange disclosures and RSS feeds

The app should prefer official/public/RSS sources and avoid restricted scraping by default.

The Sources screen shows locally configured source adapters, supported markets, fetch mode, enabled state, poll interval, and last success or error status. Before real ingestion exists, this screen still reads the seeded adapter registry from SQLite so source monitoring has a stable UI home.

## Ingestion

The default polling interval is 15 minutes while the app is open. Manual source refresh must be supported.
Before real source ingestion exists, the topbar source refresh control is a disabled placeholder. It must eventually trigger or enqueue source refresh jobs.

The DB status indicator may expose a small database-backed view refresh action for development and recovery ergonomics. That action reloads local SQLite-backed app state such as feed items, companies, watchlists, memberships, and database status, but it is not the product-level news/source refresh workflow.

Ingestion should preserve source attribution, publication time, fetch time, original language, matched company, and source URL.

Feed retention must be designed before v1 ingestion becomes broad. The app should avoid unbounded local growth by defining per-source retention defaults, user-adjustable cleanup settings, and rules that preserve important user-marked content. Saved items, items linked to notes, and items with AI analysis or explicit user decisions should not be removed by routine cleanup without clear user control.

## AI Analysis

The first AI milestone is summarization and classification:

- concise summary
- significance label
- topic tags
- reasoning
- source references

Gemini should be preferred only for YouTube press conference transcription because of native vendor support for video/audio and YouTube URL input. Other AI workflows, including summaries, significance labels, and note extraction, have no preferred provider yet. Provider limits and privacy terms must be shown in settings before use.

The first video AI workflow should support:

- entering a YouTube press conference URL
- running a transcription or transcript-like extraction job
- reviewing transcript segments
- selecting transcript segments, text ranges, or AI-suggested claims to add to a specific company's notebook
- preserving the YouTube URL, timestamp range when available, provider, and created note provenance

AI output must be presented as decision support. It must not contain direct buy/sell/hold recommendations.

Default AI analysis mode is source-grounded. A future opinionated mode may be added behind explicit user opt-in, but it must remain source-cited and must not provide buy/sell/hold or personalized portfolio advice.

## Settings, Export, And Local Data

The Settings panel edits runtime settings stored in SQLite. YAML is accepted as the future import/export/bootstrap format for non-secret settings, but YAML implementation is deferred until the later export/import/backup work. API keys and provider secrets are stored in the OS keychain and must never be exported to YAML.

App data lives in the OS app data directory by default, with development-only override support. V1 uses local logs only and no telemetry.

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
