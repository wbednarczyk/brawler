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
- Origin links are required for notes created from feed items, AI outputs, or transcript segments.
- `claim_status`, `event_date`, `follow_up_after`, and `follow_up_date` are **legacy** claim columns. As of `v0.42.0` ([ADR 0040](adr/0040-management-claims-tracker.md)) management claims are a first-class entity (see [Management Claims](#management-claims)); the `0045` forward migration moves existing `kind = 'claim'` rows into `management_claims`. The columns remain for backward-compatible reads of any non-claim note that set them; new claims are not written here.

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

### Company Signals

Supports typed ESPI/EBI classification: turns official filings into typed disclosure signals (insider transactions, dividends, profit warnings, significant contracts, own-share transactions, guidance changes). `company_signals` is the canonical classification output, separate from `feed_items` (the raw filing) and `company_events` (the calendar). See [ADR 0034](adr/0034-espi-event-classification.md).

Fields:

- `id`
- `company_id`
- `feed_item_id`
- `category`
- `confidence`
- `classified_by`
- `status`
- `signal_date`
- `provider_id`
- `model_id`
- `derived_event_id`
- `created_at`
- `updated_at`

Related tables:

- `signal_categories`

Rules:

- Signals belong to exactly one canonical company and reference the originating `feed_item` as origin.
- `category` references a row in the seeded, extensible `signal_categories` registry; it is not a hard-coded enum.
- `classified_by` is `rule` or `ai`.
- `status` is `confirmed` or `proposed`. Rule-classified signals are `confirmed` on creation; AI-classified signals are `proposed` and require user confirmation before becoming `confirmed`.
- `signal_date` is the filing publication date and is stored as `YYYY-MM-DD`.
- `provider_id` and `model_id` are populated only for `ai` classifications and record provider provenance for audit and reversibility.
- A `company_events` row is derived (`derived_event_id`) **only** for forward-looking categories carrying a genuine future date (e.g. dividend record/payment date, general-meeting date). Past-disclosure signals do not derive calendar events.
- Classification and event derivation are idempotent: re-ingesting or re-confirming never duplicates a signal or its derived event. Identity is `(feed_item_id, category)`.
- Unknown filings produce no signal (or a `proposed` AI signal); they are never assigned a wrong category silently.

### Signal Categories

The seeded, extensible registry that backs `company_signals.category`. New categories and markets are added as data, not schema changes.

Fields:

- `id`
- `key`
- `display_name`
- `rule_definition_json`
- `derives_event`
- `created_at`
- `updated_at`

Rules:

- Seed keys: `insider_transaction` (MAR Art. 19), `dividend`, `profit_warning`, `significant_contract`, `own_shares` (own-share/treasury transactions, purchases and sales; generalized from `buyback` in migration 0044), `guidance_change`, `general_meeting`, `other`.
- `rule_definition_json` is consumed by the interpretation-layer `RuleClassifier` ([ADR 0035](adr/0035-two-layer-ai-and-local-interpretative-layer.md)). Shape: `{ "patterns": [..], "confidence": 0.0..1.0 }`, where any case-insensitive substring match against the filing text selects the category. An empty `patterns` list never rule-matches — `other` carries no patterns and is reachable only via the AI fallback.
- `derives_event = 1` marks categories that materialize a derived `company_events` row when the filing carries a future date. Only `dividend` and `general_meeting` derive events (ADR 0034); all other seed categories are `derives_event = 0`.
- The registry is source-neutral so a future GPW re-enable feeds the same classifier.

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
- Reminder `source_type` and `source_id` point to the canonical object when a reminder comes from a claim, event, question, digest, signal, or other evidence.
- `reminder_kind` values: `claim_follow_up`, `event_review`, `question_review`, `manual_research`, `digest_review`, and `signal_review` (a high-signal ESPI/EBI classification — insider transaction or profit warning — `source_type = company_signal`; ADR 0034).
- Derived reminders may be synchronized from claims, events, and open research questions.
- Confirmed `company_signals` appear in the backend research timeline as `company_signal` evidence items and so flow into the personal digest; proposed (unconfirmed AI) signals do not.
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

### Management Claims

First-class management claims ([ADR 0040](adr/0040-management-claims-tracker.md), `v0.42.0`): a tracked management promise from a report or transcript, with a normalized due period and a user-set verdict. Replaces the legacy `notebook_entries(kind='claim')` model. Migration `0045_management_claims.sql`.

`management_claims`:

- `id`, `company_id` → `companies(id)`.
- `statement`: the promise text (verbatim where AI-extracted), `body`: optional user context, `body_format` (`markdown`).
- `made_at`: date the statement was made (`YYYY-MM-DD`, optional); `source_period_id` → `financial_periods(id)` optional (the period it was stated in/about).
- Due period (the resurfacing match key): `due_fiscal_year` (integer, optional) + `due_period_type` (reuses the `financial_periods` vocabulary: `FY`/`H1`/`H2`/`Q1`–`Q4`/`9M`/`M01`–`M12`, optional). A claim with no due period never resurfaces and stays user-managed.
- `status` (verdict): `pending` (default) | `delivered` | `partially_delivered` | `missed` | `revised`. User-set; never auto-assigned.
- Provenance: `source_evidence_type` (`report_document` | `transcript_segment` | `transcript` | `manual` | `feed_item`) + `source_evidence_id` (soft reference; a transcript-extracted claim is attributed to its transcript job, a report-extracted claim to its report document); `extraction_proposal_id` → `claim_extraction_proposals(id)` when AI-extracted (null for manual).
- Quantitative target (optional, drives fact lookup): `target_metric_key`, `target_comparator` (`gte`/`lte`/`gt`/`lt`/`approx`/`eq`), `target_value_numeric` (decimal-exact text), `target_unit`.
- Verification: `verifying_fact_id` → `financial_facts(id)` soft reference (set when a fact is linked from the review queue); `revises_claim_id` → `management_claims(id)` (set on a `revised` supersession, history kept).
- `created_at`, `updated_at`.

Rules:

- A claim belongs to exactly one canonical company.
- The verdict is set by the user; there are no automated verdicts (out of [ADR 0040](adr/0040-management-claims-tracker.md) scope). The review queue surfaces the matching fact; the user decides.
- A claim is evidence: it surfaces in the research timeline as `evidence_type = 'claim'` (now resolving against `management_claims.id`) and participates in `evidence_links`. The verifying fact for a quantitative claim is the direct `verifying_fact_id` soft reference; registering that link in the generic `evidence_links` graph is deferred (the graph does not yet model `financial_fact` as an evidence type — see [ADR 0040](adr/0040-management-claims-tracker.md) Decision 5).
- The `0045` migration is idempotent and self-healing: existing `notebook_entries(kind='claim' OR claim_status IS NOT NULL)` rows converge into `management_claims` (status `open → pending`; `delivered`/`partially_delivered`/`missed` unchanged; `unknown`/`not_applicable → pending`), `follow_up_after` parsed into `due_fiscal_year`/`due_period_type` where it matches a period token, and existing `evidence_links`/reminders pointing at the note id are re-pointed at the claim id.
- Import/export, retention, and backup treat claims as owner durable state via a first-class `claims` bundle section.

### Claim Extraction

AI claim extraction with mandatory user confirmation ([ADR 0040](adr/0040-management-claims-tracker.md), `v0.42.0`), mirroring the KPI extraction job→proposal→confirm/reject pattern. Sources are report documents **and** transcripts. Migration `0046_claim_extraction.sql`.

`claim_extraction_jobs`:

- `id`, `company_id` → `companies(id)`.
- Source: `source_type` (`report_document` | `transcript`), `source_id` (→ `report_documents(id)` or `transcript_jobs(id)`).
- `provider_id`, `model`, `prompt_version`.
- `status`: `queued` | `running` | `succeeded` | `failed`; `error_code`, `error`.
- `created_at`, `started_at`, `finished_at`.

`claim_extraction_proposals`:

- `id`, `job_id` → `claim_extraction_jobs(id)`.
- `statement` (candidate claim text), suggested `due_fiscal_year`/`due_period_type`, optional quantitative target (`target_metric_key`/`target_comparator`/`target_value_numeric`/`target_unit`).
- `confidence`, `source_snippet` (verbatim evidence), `source_evidence_type`/`source_evidence_id` (the document or transcript segment the snippet came from).
- `status`: `pending` (default) | `confirmed` | `rejected`; `claim_id` → `management_claims(id)` (set on confirm).
- `created_at`, `updated_at`.

Rules:

- Only a **confirmed** proposal materializes a `management_claims` row; confirmation may carry user overrides of the extracted fields. No claim is created without user review.
- Rejected proposals are retained (audit, and to suppress re-proposal of the same statement).
- One job targets one source document or transcript; extraction is idempotent per `(source_type, source_id, prompt_version)` for re-runs.
- Provider/model/prompt provenance is recorded for audit and reversibility, consistent with KPI extraction and signal classification.

### Report Preparations

Per-occurrence preparation state for the report-season cockpit ([ADR 0044](adr/0044-report-season-cockpit.md), `v0.43.0`). The cockpit's calendar and pre-report card are backend-owned **read models** assembled from canonical domains (`company_events`, `research_questions`, `management_claims`, `financial_facts`/`financial_periods`, the research-timeline read model) with no stored projection; this is the only new persisted state the milestone adds. Migration `0047_report_preparations.sql`.

`report_preparations`:

- `id`, `company_id` → `companies(id)`.
- `event_key`: the stable `company_events.source_event_key` of the report occurrence (not the volatile event row id), so the state survives calendar re-derivation ([ADR 0036](adr/0036-report-document-storage-and-backfill.md)).
- `status`: `upcoming` (default) | `prepared` | `processed`. User-set via workflow actions; never auto-assigned.
- `prepared_at`, `processed_at`: transition timestamps (`YYYY-MM-DDTHH:MM:SSZ`, nullable).
- `linked_report_document_id` → `report_documents(id)` soft reference, set on processing when the arrived report is known (nullable).
- `created_at`, `updated_at`. Unique on `(company_id, event_key)`.

Rules:

- Absence of a row means `status = 'upcoming'`; reads default a missing row to `upcoming` so the cockpit never crashes on un-prepared companies or an absent migration.
- `mark_report_prepared` / `mark_report_processed` are explicit user actions; there is no automated transition (the autonomous path is the North Star, `v0.49.0`).
- The migration is idempotent and self-healing (`CREATE TABLE IF NOT EXISTS`).
- Preparation state is owner durable state; its inclusion in the import/export bundle is a future per-feature coverage item ([roadmap](roadmap.md) `v0.52.0`), not part of `v0.43.0`.

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

## Company Signal Model

Company signals are typed classifications of official ESPI/EBI filings. A signal answers "what kind of disclosure is this filing" — insider transaction, dividend, profit warning, significant contract, own-share transaction, guidance change, or other. Signals are canonical and distinct from calendar events: most disclosures are dated past events, so only forward-looking categories with a real future date derive a `company_events` row. See [ADR 0034](adr/0034-espi-event-classification.md).

Identity and lifecycle:

- A signal is produced by classifying a `feed_item` from an official report source (currently the active Bankier company-komunikaty feed; source-neutral for a future `gpw-espi-ebi` re-enable).
- The rule classifier runs at ingestion and writes `confirmed` signals deterministically. Filings it cannot place go to the opt-in async AI fallback, which writes `proposed` signals that require user confirmation.
- Signal identity is `(feed_item_id, category)`; re-classification updates the existing row rather than inserting a duplicate.
- Derived calendar events link back via `derived_event_id`, and the event carries origin linkage to the signal and the originating filing.

Derived calendar events (`v0.41.0`, [ADR 0036](adr/0036-report-document-storage-and-backfill.md)):

- A `company_events` row is derived **only** for `dividend` and `general_meeting` signals that are **confirmed** and carry a real future date extracted from the filing body.
- Date extraction is **deterministic-first** (labelled-pattern parse of the body), with the **opt-in async AI fallback** when the deterministic parse is not confident; the derived event is created as `status = 'proposed'` and requires user confirmation before it appears on the calendar. A guessed-date event is never created.
- A derived event is represented additively (no migration): `source_type = 'derived_signal'`, `source_adapter_id` = the official-report adapter, and `source_event_key` = the originating signal id, so the event identity is stable and idempotent re-derivation upserts the same row. `company_signals.derived_event_id` points back to the event; the event's source key points back to the signal.
- `dividend` signals derive a `dividend` event; `general_meeting` signals derive a `shareholder_meeting` event. Confirmation flips `status` from `proposed` to `confirmed`; rejection deletes the proposed event and clears `derived_event_id`. Derivation dedups against manually created events for the same company/date/type.

## Report Document Model

Report documents are the persisted report files behind fundamentals and the report-document source ladder ([ADR 0029](adr/0029-ir-page-report-resolution.md), [ADR 0036](adr/0036-report-document-storage-and-backfill.md)). A document originates from an ESPI/EBI attachment, a user-supplied PDF URL, or a captured article URL, and is the citation target for AI KPI extraction, report-over-report diff, and confirmed financial facts. The table shipped in migration 0035; `v0.41.0` implements the ESPI/EBI attachment rung and the storage/retention rules below without a schema change.

Fields (migration 0035): `id`, `company_id` → `companies(id)`, `period_id` → `financial_periods(id)` (nullable), `source_type` (`espi_attachment` | `user_url` | `article`), `origin_ref` (feed item / evidence id), `url` (original source URL), `local_path` (relative path under the app data dir; null when no file is stored), `content_type`, `content_hash` (sha256, optional dedup), `byte_size`, `title`, `attribution`, `fetch_status`, `fetch_error`, `fetched_at`, `created_at`, `updated_at`. `financial_facts.source_document_ref` is a soft reference to `report_documents.id`.

Storage, dedup, and retention rules:

- `local_path` always stores a **relative** path under a dedicated `report_documents/` subtree (keyed by company) so the store stays portable across machines and survives import/export. It is never an absolute path.
- **Full files are stored only for periodic / financial reports** (the extraction/diff targets), determined from the filing's classified `company_signals` category and ESPI/EBI report metadata. Other ESPI/EBI attachments persist as **metadata + URL only**: `local_path` null, `fetch_status = 'metadata_only'`, preserving `url`/`title`/`attribution`/`origin_ref` for citation and the source ladder. The user-URL and IR-page rungs always store the full file.
- `fetch_status` is `pending | fetched | failed | metadata_only` (the `metadata_only` value is additive over migration 0035 — no new migration). It is the single source of truth for whether bytes exist locally; a failed fetch records `fetch_error`, stays retryable, and never blocks feed-item ingestion.
- Identity is `UNIQUE(company_id, url)`; capture, refresh, and backfill **upsert** on this key. `content_hash` is a secondary same-company dedup signal so a re-fetch of identical bytes is not duplicated.
- Retention reuses the feed-retention protection model ([ADR 0033](adr/0033-feed-retention-policy.md)): a document referenced by a confirmed `financial_fact`, linked as research evidence, or backing a confirmed signal derivation is **protected** and never pruned. Unprotected full files past the retention window have their **bytes** pruned (file deleted, row downgraded to `metadata_only`); the metadata row itself is never deleted. On-disk report-document size and the retention window are surfaced in Settings → Data retention alongside the feed controls.

## Search Index

Global full-text search is served by a single unified SQLite FTS5 virtual table, `search_index`. See [ADR 0032](adr/0032-search-and-backup-boundaries.md).

```sql
CREATE VIRTUAL TABLE search_index USING fts5(
  title,
  body,
  content_type UNINDEXED,   -- 'company' | 'watchlist' | 'feed_item' | 'notebook_entry'
                            -- | 'transcript_segment' | 'event' | 'research_brief' | 'digest'
  source_id    UNINDEXED,   -- primary key of the owning source row
  company_id   UNINDEXED,   -- canonical company for scoping/grouping (nullable)
  parent_id    UNINDEXED,   -- navigational container when source_id is not the
                            -- nav target (transcript_segment -> transcript job); nullable
  tokenize = 'unicode61 remove_diacritics 2'
);
```

Indexed content:

- company ticker and display name (`content_type = 'company'`)
- watchlist name and description (`content_type = 'watchlist'`)
- feed item title and body text
- notebook title and Markdown body
- transcript segment text (`parent_id` = owning transcript job)
- company event title and type (`content_type = 'event'`)
- research brief title and body
- digest title and body

Rules:

- `search_index` is **derived state**, not a source of truth. It is populated by per-source `AFTER INSERT/UPDATE/DELETE` triggers and is fully rebuildable from the source tables, so schema evolution rebuilds the index rather than migrating its shape.
- The tokenizer is `unicode61 remove_diacritics 2` (language-neutral, diacritic- and case-folding) for the Polish-primary, English-mixed corpus.
- Existing rows are backfilled when the index migration first applies.
- User query text is sanitized before reaching `MATCH`; it is never interpolated as FTS5 syntax. Ranking uses `bm25()`; results carry `snippet()`, `content_type`, `company_id`, and `parent_id`.
- `parent_id` carries the navigational container when an item's own id is not the navigation target — currently the owning transcript job for a transcript segment — so a result opens the specific item. It is `NULL` for content navigated by `source_id` directly.

## Database Safety: WAL, Snapshots, And Backups

Data-safety guarantees for the local SQLite database (`brawler.sqlite3`). See [ADR 0032](adr/0032-search-and-backup-boundaries.md).

- The database runs in WAL journaling mode (`PRAGMA journal_mode = WAL`); a `-wal`/`-shm` sidecar is expected on disk.
- Backups and pre-migration snapshots are produced with `VACUUM INTO '<path>'` — a consistent, compacted copy safe to take on the live connection.
- **Pre-migration snapshot:** before the migration runner applies any pending migration, it writes a snapshot named with schema version and timestamp. If the snapshot cannot be written, migration is aborted with a clear error and no schema change is attempted; a failed migration leaves the snapshot intact for manual restore.
- **Rotating backups:** periodic and on-close backups are written to `<app_data_dir>/backups/`, keeping the last N (oldest pruned). Backup status (last time, count) is inspectable.
- **Restore** is a restart operation surfaced in Diagnostics: the chosen backup is staged and applied on app relaunch (no hot in-place swap), because live connections hold the database open.
- A backup is a byte-faithful copy of the database only; it is distinct from M20 import/export documents. Secrets live in the OS keychain, never in the database, so they are absent from backups by construction. No cloud backup.

## Connection Model

The app accesses the database through an `r2d2` connection pool, not a single shared connection. See [ADR 0032](adr/0032-search-and-backup-boundaries.md).

- The pool is uniform (any connection may read or write); SQLite's single-writer rule is absorbed by `busy_timeout` rather than a dedicated-writer split.
- Each pooled connection sets `journal_mode = WAL`, `foreign_keys = ON`, and `busy_timeout` on creation. Synchronous rusqlite work runs on blocking tasks.
- **Bootstrap ordering:** a single bootstrap connection runs pending migrations, writes the pre-migration snapshot, and reads pool configuration; the pool is then built from that configuration. Migrations, snapshots, and restore staging run outside the pool.
- Pool configuration (`maxConnections`, `busyTimeoutMs`, `acquireTimeoutMs`) is read from settings (see User Settings in [contracts.md](contracts.md)). Values are validated and clamped to safe ranges with default fallback, so an invalid value can never prevent the database from opening. Pool sizing is applied at startup, so changes take effect on the next launch.

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
