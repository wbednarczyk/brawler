# Data Model

This document maps the UX and contracts to the first local SQLite data model. It is not a final migration file, but it should be concrete enough to guide the first schema implementation.

Use [Project Brief](project-brief.md) for the full documentation map. Related references: [Contracts](contracts.md), [Architecture](architecture.md), and [UI Information Architecture](ui-information-architecture.md).

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
- UI-facing visibility tier derived from source metadata: `required`, `optional`, or `developer`
- user-configurable flag derived from visibility and implementation status
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
- Required sources are protected from disabling.
- Optional implemented sources can be enabled or disabled by the user, and refresh jobs must respect that state.
- Developer-tier source candidates may remain registered for owner/developer visibility, but normal source listing filters them out.

### Company Directory Entries

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
- Directory records are cached source data, not user-owned company records.
- User-created companies are stored in `companies` and must not be overwritten silently by directory refresh.
- Feed matching should resolve source identifiers to ticker through this cache before using ISIN fallback.
- Multiple company-directory sources are supported. GPW main market uses `GPW:<ticker>` and NewConnect uses `NC:<ticker>` behind the same directory boundary.
- Company lookup and autocomplete search all active company-directory records. The exchange typed in the Companies form is used to prefer a match when the same ticker exists on multiple exchanges, but it must not hide companies from other registries.
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

### Company Events

Supports the Events screen, upcoming-company-event review, historical context, and future source-derived calendar ingestion.

Fields:

- `id`
- `company_id`
- `event_type`
- `title`
- `event_date`
- `event_time`
- `status`
- `source_type`
- `source_adapter_id`
- `source_event_key`
- `source_url`
- `attribution`
- `fetched_at`
- `manual`
- `created_at`
- `updated_at`

Rules:

- Events belong to exactly one canonical company.
- `event_date` is required and stored as `YYYY-MM-DD`.
- `event_time` is optional and stored as local source-provided text until a source-specific time zone policy is required.
- Manual events use `source_type = manual` and `manual = 1`.
- Sourced events must preserve source URL, attribution, fetched timestamp, and source event key when available.
- `(source_adapter_id, source_event_key)` deduplicates sourced events when both values are present.
- `gpw-market-events-rss` is a source-backed event adapter and matches only tracked companies by exact ticker.
- `bankier-kalendarium-html` is the active broader public calendar adapter and matches only tracked companies by exact ticker.
- Bankier calendar source keys are based on ticker, event category, and event description so source-side date changes update the existing sourced event row.
- Source-keyed refresh updates sourced event rows when the accepted source changes the event.

### Transcript Jobs

Supports Transcripts screen and company Transcripts tab.

Fields:

- `id`
- `company_id`
- `provider_id`
- `source_type`
- `source_url`
- `source_label`
- `company_resolution_status`
- `recognized_company_candidates_json`
- `status`
- `error_code`
- `created_at`
- `started_at`
- `finished_at`
- `error`

Rules:

- Gemini is preferred only for YouTube transcription jobs.
- Source URL is required.
- The transcript workflow labels the source URL field as `URL`.
- `company_id` is nullable at job creation time.
- When a company/ticker is supplied before transcription, `company_id` is set and `company_resolution_status = provided`.
- When no company/ticker is supplied, the app may transcribe first, then attempt company recognition from transcript/provider output.
- If recognition fails, `company_resolution_status = needs_user_selection` and the UI must require company lookup/selection before transcript segments can become notebook notes.
- Allowed `company_resolution_status` values: `provided`, `recognized`, `unresolved`, `needs_user_selection`.
- Allowed `status` values: `queued`, `running`, `completed`, `failed`.
- Allowed `error_code` values: `provider_not_configured`, `provider_limit`, `provider_unavailable`, `provider_error`, `network_error`, `invalid_source_url`, `parse_error`, `unknown`.
- `error` stores user-readable diagnostic text and must not store provider secrets.
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

- `company_id` is nullable until the parent transcript job is resolved to a company.
- Segment text is immutable source output in v1.
- Timestamps are optional because providers may return different precision.
- Notes created from transcript segments reference them through origin links.
- Transcript-derived notes are normal notebook entries; each selected segment creates a `transcript_segment` origin containing the segment ID, original video URL, and job/provider/timestamp context in the label.

### AI Analysis Results

Supports feed item summaries, significance labels, tags, and future provider-neutral analysis.

Fields:

- `id`
- `feed_item_id`
- `provider_id`
- `model`
- `prompt_version`
- `summary`
- `significance`
- `reasoning`
- `language`
- `created_at`

Related tables:

- `ai_analysis_jobs`
- `ai_analysis_tags`
- `ai_analysis_source_references`

Rules:

- General AI analysis is implemented later through a provider-neutral boundary. Gemini may be the first live provider, but stored analysis records must not assume Gemini is the only provider.
- M13 analysis runs through async job records with queued, running, succeeded, and failed states.
- Job records preserve prompt preset/custom-question context, provider ID, model, prompt version, error state, and timestamps.
- AI output must not contain buy/sell/hold recommendations.
- Source references are required.

### Research Evidence Boundary

Supports future company timelines, watchlist review, changed-since-review views, evidence links, research questions, reminders, AI briefs, and digests.

M24 uses a hybrid model governed by [ADR 0022](adr/0022-research-evidence-read-model-boundary.md):

- existing domain tables remain canonical
- research evidence and timeline views are read models assembled from canonical domains first
- durable cross-domain state gets explicit research-owned tables
- full stored evidence/timeline projections are deferred until performance or review semantics require them

Likely durable tables:

- `research_review_checkpoints`
- `evidence_links`
- `research_questions`
- `ai_research_briefs`
- `ai_research_brief_citations`
- `research_reminders`
- `ai_research_digests`
- `ai_research_digest_citations`

Recommended `research_review_checkpoints` fields:

- `id`
- `scope_type`
- `scope_id`
- `reviewed_at`
- `created_at`
- `updated_at`

Rules:

- Initial `scope_type` values are `company` and `watchlist`.
- `scope_id` references the owning company or watchlist ID according to `scope_type`.
- Review checkpoints support "last reviewed" and "changed since review" read models.
- M25 uses one company-level review checkpoint per company for the first visible Research screen.
- Timeline summary counts and changed-only filtering are derived read-model behavior; no stored timeline projection is added for M25.
- Feed read/unread and saved state remain in the feed domain.
- Future evidence-level review state should be added only when a concrete workflow needs it.

Recommended `evidence_links` fields:

- `id`
- `from_type`
- `from_id`
- `to_type`
- `to_id`
- `relation_type`
- `created_at`

Rules:

- Evidence links relate existing domain entities without moving their canonical data.
- Existing notebook origin rows remain provenance records and are not replaced by `evidence_links`.
- Evidence links may connect source items, notebook entries, claims, events, transcript segments, AI analysis results, research questions, future reminders, future briefs, and future digests.
- Initial relation types include `originates_from`, `cites`, `supports`, `contradicts`, `updates`, `follows_up`, `answers`, and `related`.
- Link validation should reject unknown entity types and dangling references when practical.

Recommended `research_questions` fields:

- `id`
- `scope_type`
- `scope_id`
- `title`
- `body`
- `status`
- `closed_at`
- `created_at`
- `updated_at`

Rules:

- Initial `scope_type` values are `company` and `watchlist`, but visible v1 question creation is company-scoped first.
- `scope_id` references the owning company or watchlist according to `scope_type`.
- Initial `status` values are `open`, `answered`, and `closed`.
- `closed_at` is set when status becomes `closed` and cleared when a closed question is reopened.
- Research questions are research-owned records, not notebook entries.
- Questions may be linked to feed items, notebook entries, claims, events, transcript segments, AI analysis, and other accepted evidence through `evidence_links`.
- Company-scoped questions appear in the backend research timeline as `research_question` evidence items.

Initial `ai_research_brief_jobs` fields:

- `id`
- `scope_type`
- `scope_id`
- `provider_id`
- `model`
- `prompt_version`
- `evidence_collector_version`
- `renderer_version`
- `status`
- `error_code`
- `error`
- `created_at`
- `started_at`
- `finished_at`

Initial `ai_research_briefs` fields:

- `id`
- `job_id`
- `scope_type`
- `scope_id`
- `provider_id`
- `model`
- `prompt_version`
- `evidence_collector_version`
- `renderer_version`
- `title`
- `summary`
- `content_markdown`
- `language`
- `generated_at`
- `created_at`

Initial `ai_research_brief_citations` fields:

- `id`
- `brief_id`
- `citation_key`
- `evidence_type`
- `evidence_id`
- `label`
- `snippet`
- `created_at`

Rules:

- AI research briefs are not notebook entries.
- Initial `scope_type` values are `company` and `watchlist`.
- Brief generation is explicit and asynchronous.
- Brief jobs use the provider-neutral AI settings and credential boundary.

Initial `research_reminders` fields:

- `id`
- `scope_type`
- `scope_id`
- `company_id`
- `reminder_kind`
- `source_type`
- `source_id`
- `title`
- `body`
- `due_at`
- `status`
- `snoozed_until`
- `completed_at`
- `dismissed_at`
- `created_at`
- `updated_at`

Rules:

- Reminder records are research-owned review pressure, not generic tasks.
- Reminder `source_type` and `source_id` point to the canonical object when a reminder comes from a claim, event, question, digest, or other evidence.
- Derived reminders may be synchronized from claims, events, and open research questions.
- Completion and dismissal are stored on the reminder record and do not modify the linked source object by default.

Initial `ai_research_digest_jobs`, `ai_research_digests`, and `ai_research_digest_citations` mirror the AI brief tables with digest-specific names and version fields.

Rules:

- Research digests are immutable generated snapshots.
- Digest jobs use provider-neutral AI settings and credential boundaries.
- Digest collection combines open reminders and changed research evidence.
- Digest citations point to typed evidence references and do not store full source bodies.
- Briefs are immutable snapshots. Regeneration creates a new successful brief instead of overwriting the previous one.
- A later workflow may let the user create a notebook entry from a brief or selected excerpt, but no note is created automatically.
- Generated briefs must cite source evidence and keep buy/sell/hold recommendation guardrails.
- Citation rows link generated content back to research evidence items and should not duplicate full source bodies.
- Import/export, backup, retention, and migrations must treat research-owned durable state explicitly.

### Company Fundamentals

Report-derived fundamentals follow [ADR 0027](adr/0027-company-fundamentals-scope.md). The KPI side is a three-layer model — catalog (`kpi_definitions`), relevance (`kpi_relevance`), facts (`financial_facts`) — plus `financial_periods` and company-level discriminators. Migration `0034_company_fundamentals.sql`.

Company columns added to `companies`:

- `statement_type`: `industrial` (default) | `bank` | `insurer` | `specialty_finance` | `reit` — selects which canonical packs apply.
- `reporting_standard`: `ifrs` (default) | `us_gaap` | `local`.
- `fiscal_year_end_month`: integer 1–12 (default 12) for non-calendar fiscal years.

`financial_periods`:

- `id`, `company_id` → `companies(id)`, `fiscal_year`, `period_type` (fiscal label: `FY`, `H1`, `H2`, `Q1`–`Q4`, `9M`, `M01`–`M12`), `period_end_date`, `report_evidence_ref` (soft reference to a report document/feed item; FK tightened in a later milestone).
- Unique on `(company_id, fiscal_year, period_type)`.

`kpi_definitions` (catalog — what a metric *is*):

- `scope`: `canonical` (app-owned, global) | `sector` (shared within a `sector`) | `company` (bespoke, set `company_id`).
- `metric_key`, `label`, `value_kind` (`monetary` | `percentage` | `ratio` | `count` | `physical` | `duration`), `unit` (typed: `PLN`/`EUR`/`t`/`m2`/`shares`/`per_share`/`years`/…), `computation` (`reported` | `derived`), `formula` (for derived, over other metric keys), `display_format`.
- Unique on `(metric_key, scope, IFNULL(company_id,''), IFNULL(sector,''))`.
- Seeded packs: universal, industrial, cash flow, capital efficiency (derived), and sector packs `insurance`, `banking`, `specialty_finance`, `reit`.

`kpi_relevance` (selection over time — which KPIs matter for a company):

- `company_id`, `definition_id`, `status` (`active` | `archived`), `source` (`user` | `agent` | `sector`), `rank` (`primary` | `secondary`), `first_seen_period`, `last_seen_period`. Unique on `(company_id, definition_id)`.

`financial_facts` (values — reference a definition, never the relevance profile):

- `value_numeric`: decimal-exact text in base units, signed (parsed with `rust_decimal`); `as_reported_value`/`as_reported_scale` keep the source form (e.g. "245 253 tys. zł").
- Dimensions: `currency`, `statement_basis` (`consolidated`/`standalone`), `attribution` (`total`/`owners_of_parent`/`nci`), `variant` (`reported`/`adjusted`/`constant_currency`/`continuing`/`discontinued`/`net_of_cancellations`/`lifo_ccs`), `measure_window` (`flow`/`point_in_time`/`trailing`/`cumulative`/`duration`), `data_quality` (`final`/`estimated`), `reporting_standard` (override).
- Provenance: `extraction_method` (`manual`/`ai_extracted`/`api`/`derived`), `confidence`, `confirmation_state` (`confirmed`/`pending`/`auto_unreviewed`), `supersedes_id` (final supersedes estimate, history kept), `source_document_ref`.
- Unique on `(period_id, definition_id, statement_basis, attribution, variant, measure_window, data_quality)` so estimate and final coexist.

Rules:

- Derived metrics (margins, FCF, ROE/ROIC, net-debt/EBITDA) are computed at read time from confirmed facts (TTM where conventional); unavailable when an input is missing.
- A fact may exist for a KPI not yet active in the relevance profile (agent-extracted, awaiting curation).
- Industry-specific classified stocks (reserves with proven/probable, 1P/2P/3P categories) are modeled as company-scoped custom KPIs, not core enums.
- Import/export, retention, and backup must treat fundamentals as owner durable state.

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

### Diagnostic Events

Supports the Developer mode Diagnostics panel and module-scoped local troubleshooting timelines.

Fields:

- `id`
- `occurred_at`
- `module`
- `scope_type`
- `scope_id`
- `stage`
- `severity`
- `message`
- `metadata_json`
- `created_at`

Recommended indexes:

- `occurred_at`
- `module, occurred_at`
- `severity, occurred_at`
- `scope_type, scope_id, occurred_at`

Rules:

- Diagnostic events are stored only while Developer mode is enabled.
- Developer mode is stored as a local setting and defaults to disabled.
- Retention trims diagnostic events to the latest 1,000 events or 7 days, whichever trims first.
- `module`, `stage`, and `severity` use stable contract values so modules can adopt the framework without schema changes.
- `metadata_json` stores small structured JSON after redaction.
- Diagnostic storage must not store API keys, full prompts, full source bodies, full transcript text, raw provider responses, license private material, or full license secrets by default.
- Diagnostic events are not runtime logs, metrics, traces, or user-facing status records.
- Clearing diagnostics deletes diagnostic events but must not change user data, settings, source state, jobs, AI analysis results, notes, or transcripts.

### Settings

Supports theme selection, polling defaults, local privacy choices, and provider configuration.

Recommended storage:

- `settings` key/value table for simple local preferences
- optional structured JSON values for provider configuration

Initial keys:

- `theme`
- `accent_palette`
- `developer_mode`
- `poll_interval_seconds`
- `youtube_transcription_provider`
- `youtube_transcription_model`
- `youtube_transcription_timeout_seconds`
- `general_analysis_provider`
- `ai_analysis_mode`
- `settings_import_export_format`
- `shortcut_bindings`

Rules:

- Default theme is `dark`.
- Default accent palette is `night-neon`.
- `theme` stores brightness mode only: `dark`, `light`, or `system`.
- `accent_palette` stores the named semantic color palette. Initial allowed values are `night-neon` and `midnight-horizon`.
- Default poll interval is `900`.
- Default YouTube transcription provider is `provider_gemini`.
- Default YouTube transcription model is `gemini-2.5-flash`.
- Default YouTube transcription timeout is `300` seconds.
- Default shortcut bindings are defined in code. `shortcut_bindings` stores only user overrides, disabled states, and resettable action-ID keyed changes as JSON.
- General AI provider remains unset until the AI analysis framework milestone configures one. The first live implementation may use Gemini, but provider storage must remain extensible.
- Default AI analysis mode is `source_grounded`.
- Runtime settings live in SQLite.
- YAML import/export excludes secrets and is contract-accepted but implementation-deferred until later export/import/backup work.
- Provider secrets are referenced indirectly and stored in the OS keychain.

### Entitlements

The local entitlement module stores accepted entitlement evidence through separate local stores:

- Raw entitlement material: OS keychain.
- Derived non-secret status/metadata: local `license_metadata`.

Recommended `license_metadata` fields:

- `id`: singleton row, currently `1`
- `status`
- `reason`
- `license_id`
- `holder`
- `channel`
- `edition`
- `features_json`
- `issued_at`
- `expires_at`
- `app_version_range`
- `key_id`
- `checked_at`
- `updated_at`

Rules:

- `license_metadata` must never store the full entitlement token, private signing material, or private key material.
- Clearing the license deletes the keychain token and removes derived metadata.
- Invalid replacement attempts do not overwrite an existing valid keychain token.
- Future entitlement policies may add derived metadata fields through migrations, but raw tokens and private signing material must remain outside SQLite.

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
- Transcript-created notes link to selected `transcript_segments` and retain original YouTube URL through origin rows.
- Transcript-created notes require a resolved transcript job company before save.
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
- `source_adapter_id`
- `source_event_key`
- `source_url`
- `attribution`
- `fetched_at`
- `manual`
- `created_at`
- `updated_at`

Likely related tables:

- future `company_event_origins` if event dates are discovered through feed items, notebook entries, or transcript segments

Rules:

- Events belong to exactly one company.
- `event_date` is required.
- `event_time` is optional because many sources publish only a date.
- Manual events must be distinguishable from sourced events.
- Sourced events must preserve source URL, attribution, fetched timestamp, and source event key when available.
- Sourced event identity uses `(source_adapter_id, source_event_key)` when both values are present.
- Sourced event refreshes update the existing source-keyed row when event date, title, status, source URL, attribution, or fetched timestamp changes.
- Manual events are for missing or user-known dates, not normal corrections to changed sourced events.

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
- ai_analysis_jobs
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
- hosted data-service metadata

## Import And Export Documents

M20 import/export documents are portable files, not runtime tables.

Rules:

- Research data is exported as JSON with schema version, export timestamp, app version, companies, watchlists, memberships, notebook entries, research questions, evidence links, AI research briefs, and brief citations.
- Settings are exported as YAML with schema version, export timestamp, app version, and allowlisted non-secret settings.
- Import validates documents before any storage change.
- Research import applies through existing SQLite tables inside one transaction.
- Settings import writes through the same settings validation path used by normal Settings updates.
- Provider secrets, license tokens, private signing material, logs, diagnostics, metrics, feed items, transcripts, and full backup data are not represented in M20 documents.
