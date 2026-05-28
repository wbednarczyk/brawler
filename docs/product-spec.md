# Product Spec

See also [Project Brief](project-brief.md), [UI Flows](ui-flows.md), [UI Information Architecture](ui-information-architecture.md), [Source Strategy](source-strategy.md), [Architecture](architecture.md), [Contracts](contracts.md), and [Kanban](kanban.md).

## V1 Experience

The first screen is an investor inbox: a dense chronological feed of company-specific reports and news. It should support repeated daily use and fast scanning. Each company also has a notebook for durable research notes.

Expected v1 UI areas:

- feed list with newest items first
- watchlist selector
- company filter
- source/type filter
- unread/read state
- saved items
- item detail pane with source attribution and original link
- action to create a company notebook note from a feed item
- manual refresh control
- in-app badges for new/unread items

Desktop notifications are out of scope for v1. Portfolio positions, cost basis, and trading workflows are out of scope.

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

Users can maintain multiple watchlists. Companies are displayed ticker-first, but canonical storage uses exchange-qualified tickers such as `GPW:CDR` or `NASDAQ:MSFT`.

Company metadata may include:

- display name
- exchange
- ticker
- ISIN
- CIK
- LEI
- aliases
- source-specific IDs

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

## Ingestion

The default polling interval is 15 minutes while the app is open. Manual refresh must be supported.

Ingestion should preserve source attribution, publication time, fetch time, original language, matched company, and source URL.

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

The Settings panel edits runtime settings stored in SQLite. YAML is supported as import/export/bootstrap format for non-secret settings. API keys and provider secrets are stored in the OS keychain and must never be exported to YAML.

App data lives in the OS app data directory by default, with development-only override support. V1 uses local logs only and no telemetry.

Export is part of normal v1 implementation. Notes should export as Markdown with metadata, and watchlists/companies/settings should export as structured JSON or YAML. Import/restore and full local backup are late-v1 items. Cloud backup/sync requires a later design discussion.

## Monetization

The app should leave room for future monetization, but the model is undecided. Open core plus paid convenience features is one possible path, but not a committed direction. Potential paid features could include packaged builds, sync, backups, managed AI configuration, and notifications.

Brawler is all rights reserved for now. The exact license, monetization model, and commercial boundary require a future ADR before public release, accepting external contributions, or publishing release artifacts.
