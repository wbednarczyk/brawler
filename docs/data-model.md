# Data Model

This document maps the UX and contracts to the first local SQLite data model. It is not a final migration file, but it should be concrete enough to guide the first schema implementation.

See also [UI Information Architecture](ui-information-architecture.md), [UI Flows](ui-flows.md), [Contracts](contracts.md), and [Architecture](architecture.md).

## Model Principles

- SQLite is the local source of truth in v1.
- IDs are stable application IDs, not user-visible labels.
- Tickers are user-facing, but `qualified_ticker` is the uniqueness boundary.
- Fetched content and notes must preserve origin links.
- Transcript source output is immutable in v1.
- Settings are local and must not require cloud identity.
- Secrets live in the OS keychain, not in SQLite.
- YAML config is import/export/bootstrap, not runtime truth.
- Schema changes must be migration-managed from the first implementation milestone.

## Core Entities

### Companies

Supports Companies screen, company workspace, feed matching, notebooks, and transcript ownership.

Fields:

- `id`
- `exchange`
- `ticker`
- `qualified_ticker`
- `display_name`
- `isin`
- `cik`
- `lei`
- `created_at`
- `updated_at`

Rules:

- `qualified_ticker` is unique.
- `ticker` alone is not unique.
- `isin`, `cik`, and `lei` are optional.

Related tables:

- `company_aliases`
- `company_source_ids`
- `company_registry_entries`

`company_source_ids` stores source-specific identifiers that cannot be derived reliably from the local ticker. For example, Bankier per-company komunikaty uses Bankier instrument slugs and tag IDs because short GPW tickers such as `CDR` may canonicalize to Bankier slugs such as `CDPROJEKT`.

### Watchlists

Supports the sidebar, Inbox filters, and first-run setup.

Fields:

- `id`
- `name`
- `description`
- `created_at`
- `updated_at`

Join table:

- `watchlist_companies`

Rules:

- A company can belong to multiple watchlists.
- Watchlists are user-maintained local data.

### Source Adapters

Supports Sources screen and ingestion jobs.

Fields:

- `id`
- `display_name`
- `source_type`
- `fetch_mode`
- `enabled`
- `default_poll_interval_seconds`
- `last_attempt_at` via `source_adapter_state`
- `last_success_at`
- `last_error_at`
- `last_error`
- `last_items_fetched` via `source_adapter_state`
- `last_items_created` via `source_adapter_state`
- `last_items_matched` via `source_adapter_state`
- `last_items_unmatched` via `source_adapter_state`
- `created_at`
- `updated_at`

Related tables:

- `source_adapter_markets`
- `source_adapter_state`

Rules:

- Adapter IDs should be stable, for example `gpw-espi-ebi`.
- Source-specific cursors or checkpoints live in adapter state.

### Company Registry Entries

Supports company lookup, autocomplete, and ticker-first feed matching.

Fields:

- `id`
- `exchange`
- `ticker`
- `qualified_ticker`
- `display_name`
- `isin`
- `source_adapter_id`
- `source_url`
- `fetched_at`
- `active`
- `created_at`
- `updated_at`

Rules:

- `exchange + ticker` is the uniqueness boundary.
- Registry records are cached source data, not user-owned company records.
- User-created companies are stored in `companies` and must not be overwritten silently by registry refresh.
- Feed matching should resolve source identifiers to ticker through this cache before using ISIN fallback.
- Slow refresh cadence is expected, initially daily or weekly.

### Feed Items

Supports Inbox, company Feed tab, source attribution, notes from feed, and AI analysis.

Fields:

- `id`
- `type`
- `source_adapter_id`
- `source_name`
- `source_url`
- `title`
- `summary`
- `body_text`
- `language`
- `published_at`
- `fetched_at`
- `display_company`
- `dedupe_key`
- `duplicate_signature`
- `read`
- `saved`
- `attribution`
- `created_at`
- `updated_at`

Join table:

- `feed_item_companies`
- `feed_item_attachments`

Rules:

- `dedupe_key` should be unique per source adapter.
- `duplicate_signature` is nullable and may be used for cross-source article/media dedupe when two source adapters publish the same company-related item under different URLs or source-specific IDs.
- `fetched_at` is required.
- `published_at` may be null only when the source does not provide it.
- `display_company` is a UI read-model helper for early feed rows and unmatched/source-derived ticker labels. Canonical company relationships still live in `feed_item_companies`.
- Read and saved state are stored in SQLite and must survive app restart.
- Attachments are stored as ordered source links with `label` and `url`, scoped to a feed item.
- Retention policy must be designed before broad ingestion. Feed item storage should support cleanup without deleting saved items, items linked to notes, items with AI analysis, or items otherwise explicitly preserved by the user.

### Notebook Entries

Supports company notebooks, cross-company Notebooks screen, claims follow-up, and notes from feed/transcripts.

Fields:

- `id`
- `company_id`
- `title`
- `body`
- `body_format`
- `kind`
- `claim_status`
- `event_date`
- `follow_up_after`
- `follow_up_date`
- `created_at`
- `updated_at`

Related tables:

- `notebook_entry_tags`
- `notebook_entry_origins`

Rules:

- `body_format` is `markdown` in v1.
- Notes belong to exactly one company.
- Claim notes may use both `follow_up_after` and `follow_up_date`.
- Origin links are required for notes created from feed items, AI outputs, or transcript segments.

### Transcript Jobs

Supports Transcripts screen and company Transcripts tab.

Fields:

- `id`
- `company_id`
- `provider_id`
- `source_type`
- `source_url`
- `status`
- `created_at`
- `started_at`
- `finished_at`
- `error`

Rules:

- Gemini is preferred only for YouTube transcription jobs.
- Source URL is required.
- Jobs emit status changes to the UI.

### Transcript Segments

Supports transcript review and note creation from selected conference excerpts.

Fields:

- `id`
- `transcript_job_id`
- `company_id`
- `start_seconds`
- `end_seconds`
- `speaker`
- `text`
- `language`
- `created_at`

Rules:

- Segment text is immutable source output in v1.
- Timestamps are optional because providers may return different precision.
- Notes created from transcript segments reference them through origin links.

### AI Analysis Results

Supports feed item summaries, significance labels, tags, and future provider-neutral analysis.

Fields:

- `id`
- `feed_item_id`
- `provider_id`
- `model`
- `summary`
- `significance`
- `reasoning`
- `language`
- `created_at`

Related tables:

- `ai_analysis_tags`
- `ai_analysis_source_references`

Rules:

- General AI analysis has no preferred provider yet.
- AI output must not contain buy/sell/hold recommendations.
- Source references are required.

### Jobs

Supports Sources screen, background ingestion, manual refresh, and transcript processing status.

Fields:

- `id`
- `type`
- `adapter_id`
- `transcript_job_id`
- `status`
- `started_at`
- `finished_at`
- `items_fetched`
- `items_created`
- `warnings_json`
- `error`

Rules:

- `adapter_id` is used for source polling jobs.
- `transcript_job_id` is used for transcript jobs when represented in the shared job list.
- Warnings can be JSON in v1 unless they need filtering.

### Settings

Supports theme selection, polling defaults, local privacy choices, and provider configuration.

Recommended storage:

- `settings` key/value table for simple local preferences
- optional structured JSON values for provider configuration

Initial keys:

- `theme`
- `accent_palette`
- `poll_interval_seconds`
- `youtube_transcription_provider`
- `general_analysis_provider`
- `ai_analysis_mode`
- `settings_import_export_format`

Rules:

- Default theme is `dark`.
- Default accent palette is `night-neon`.
- Default poll interval is `900`.
- General AI provider is null until the user configures one.
- Default AI analysis mode is `source_grounded`.
- Runtime settings live in SQLite.
- YAML import/export excludes secrets and is contract-accepted but implementation-deferred until later export/import/backup work.
- Provider secrets are referenced indirectly and stored in the OS keychain.

## Origin Model

Notebook origin links should be flexible enough to connect notes to different source types.

Fields for `notebook_entry_origins`:

- `id`
- `notebook_entry_id`
- `source_type`
- `source_id`
- `source_url`
- `label`
- `created_at`

Allowed source types:

- `feed_item`
- `transcript_segment`
- `ai_analysis`
- `manual`
- `external_url`

Rules:

- Feed-created notes link to `feed_items`.
- Transcript-created notes link to `transcript_segments` and retain original YouTube URL.
- Manual notes may use a `manual` origin link or no external source.
- Normal note editing preserves existing origin links. Adding or detaching origins requires a future explicit source-link workflow.

## Company Event Model

Company events represent dated items for companies in watchlists. Upcoming events are the default product focus, but historical events are retained for context and comparison. They are not portfolio-position events and do not require holdings.

Fields for `company_events`:

- `id`
- `company_id`
- `event_type`
- `title`
- `event_date`
- `event_time`
- `status`
- `source_type`
- `source_url`
- `attribution`
- `fetched_at`
- `updated_at_source`
- `manual`
- `user_adjusted`
- `created_at`
- `updated_at`

Likely related tables:

- `company_event_origins` or correction/audit table if sourced events can be manually adjusted
- future links to feed items or notebook entries when event dates are discovered through reports or notes

Rules:

- Events belong to exactly one company.
- `event_date` is required.
- `event_time` is optional because many sources publish only a date.
- Manual events must be distinguishable from sourced events.
- User corrections to sourced events must preserve the original sourced date and attribution.

## Search Inputs

The first schema should leave room for search across:

- company ticker and display name
- feed item title and body text
- notebook title and Markdown body
- transcript segment text

SQLite FTS can be added after the base schema is stable.

## First Migration Scope

The first migration should create:

- companies
- company_aliases
- company_source_ids
- watchlists
- watchlist_companies
- source_adapters
- source_adapter_markets
- source_adapter_state
- feed_items
- feed_item_companies
- notebook_entries
- notebook_entry_tags
- notebook_entry_origins
- transcript_jobs
- transcript_segments
- ai_analysis_results
- ai_analysis_tags
- ai_analysis_source_references
- jobs
- settings

## Explicitly Deferred

Do not model these in v1:

- portfolio positions
- trades
- account balances
- billing records
- users or organizations
- cloud sync metadata
- team permissions
- cloud backup/sync metadata
