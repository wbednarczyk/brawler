# Data Model

This document maps the UX and contracts to the first local SQLite data model. It is not a final migration file, but it should be concrete enough to guide the first schema implementation.

Doc map: [CLAUDE.md](../CLAUDE.md) § Required Reading. Related references: [Contracts](contracts.md), [Architecture](architecture.md), and [UI Information Architecture](ui-information-architecture.md).

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
- **Write transactions are `BEGIN IMMEDIATE`** (`transaction_with_behavior(Immediate)`): a DEFERRED read→write under WAL fails instantly with SQLITE_BUSY on snapshot upgrade — `busy_timeout` does not apply to that path (live-drive diagnosis 2026-07-14; enforced by the clippy `disallowed-methods` guardrail in `src-tauri/clippy.toml`).
- **Domain recency is the domain date, never `created_at`.** Selecting or ranking "newest / latest / most recent" of anything (report, feed item, event, signal, fact) must order by the **domain date** — publication / period-end / event / signal date — and never by `created_at`/`updated_at`/ingestion order. `created_at` is the local insert time; a history backfill or re-ingest gives an **old** record a **newer** `created_at`, so created-order silently diverges from chronological order. Any such selection ships a test against **real backfilled data where `created_at` order ≠ domain-date order** (real-data validation). Guardrail from `d60305c` (autopilot detection fired on a 3-year-old report by ranking on `created_at`); policy [ADR 0045](adr/0045-guardrail-harvest-loop.md).

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
- `sector` (nullable) — company sector/industry classification ([ADR 0067](adr/0067-market-data-foundation.md), `v0.53.0`)
- `sector_source` (nullable) — `registry` | `manual`; a `manual` value wins and is never clobbered by a registry refresh
- `created_at`
- `updated_at`

Rules:

- `qualified_ticker` is unique.
- `sector`/`sector_source` are optional; reads tolerate a missing value with a safe default.
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
- `role` derived from source metadata: `primary` (ingests into the feed) or `witness` (reconciles against the primary, never ingests — [ADR 0069](adr/0069-source-reliability-and-disclosure-signals.md) decision 2). `gpw-espi-ebi` is the sole `witness` (v0.55 T3).
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
- `sector` (nullable) — directory-sourced sector/industry ([ADR 0067](adr/0067-market-data-foundation.md), `v0.53.0`)
- `active`
- `created_at`
- `updated_at`

Rules:

- `exchange + ticker` is the uniqueness boundary.
- Directory records are cached source data, not user-owned company records.
- User-created companies are stored in `companies` and must not be overwritten silently by directory refresh.
- On refresh (and on company creation) the cached `sector` is propagated onto the tracked `companies.sector` with `sector_source='registry'`, **unless** that company has a `sector_source='manual'` override — a manual value is never clobbered.
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
- For the investor week calendar ([ADR 0058](adr/0058-investor-week-calendar.md)), `event_type` gains `ipo_debut` (primary-market debut) and `ex_dividend` (ex-dividend / cut-off date, distinct from `dividend` = record/payment). No schema change — `event_type` is a string. The `bankier-kalendarium-html` mapping widens to emit `periodic_report`, `ipo_debut`, and `ex_dividend`; ESPI ex-date derivation extends `event_derivation`.

### Investor Calendar Layers

The investor week calendar ([ADR 0058](adr/0058-investor-week-calendar.md), `v0.67.0`) is a backend-owned **read model** (`list_investor_week`) that unions company events with market-wide layers that have no canonical company. It adds three small domains; `company_events` and its one-canonical-company invariant are unchanged.

`market_calendar_events` — the opt-in **whole-market** layer: GPW calendar events for **untracked** tickers (no `company_id`). Populated by a relaxed whole-page Bankier kalendarium ingest fetched only when the user enables the market scope.

- Fields: `id`, `ticker`, `issuer_name`, `event_type`, `title`, `event_date`, `event_time`, `status`, `source_type`, `source_adapter_id`, `source_event_key`, `source_url`, `attribution`, `fetched_at`, `created_at`, `updated_at`.
- No `company_id`; `ticker` is the only company key. The week read model unions `company_events` (tracked) ∪ `market_calendar_events` (untracked) **deduped by ticker**, so a tracked company never appears twice; a market row whose ticker matches a tracked company links into its workspace.
- `(source_adapter_id, source_event_key)` deduplicates rows; cache-first week navigation reuses the Bankier dated-week pattern.

`macro_events` — the **macro** layer (no company). The model + manual add + a sample seed ship in `v0.67.0`; a policy-clean **live source is deferred to a follow-up ADR** (ADR 0058 §4).

- Fields: `id`, `indicator_key`, `title`, `country`, `event_date`, `event_time`, `importance`, `actual`, `forecast`, `previous`, `source_type`, `source_url`, `attribution`, `fetched_at`, `manual`, `created_at`, `updated_at`.
- `actual`/`forecast`/`previous` are optional text. `manual = 1` for user-entered releases. Reads tolerate an empty table (safe default = no macro lane).

`market_holidays` — the **holidays** layer: a curated, refreshable static dataset (GPW; US NYSE/Nasdaq), **not** a live source.

- Fields: `id`, `market`, `holiday_date`, `name`, `session` (`closed | half_day`), `created_at`, `updated_at`.
- Renders a per-market `WOLNE` badge on closed days. Reads tolerate a missing/un-seeded year (safe default = no holidays) so the week view never crashes.

Active scope (`watchlist | market`) and enabled layers (macro, holidays) persist in `user_settings` with tolerant defaults (the pinned-companies pattern, [ADR 0054](adr/0054-mode-based-thesis-centric-shell.md)).

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
- `classified_by` is `rule`, `ai`, or `agent` — honest provenance of who classified the filing: the deterministic rule classifier, the (retired) in-app AI fallback, or a connected agent via the MCP triage tool `classify_filing` (`agent`, added by migration `0113`, ADR 0088 dec. 4). The CHECK constraint enforces this set; an agent classification is never mislabelled `rule`.
- `status` is `confirmed` or `proposed`. Rule- and agent-classified signals are `confirmed` on creation; AI-classified signals are `proposed` and require user confirmation before becoming `confirmed`.
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

- Seed keys: `insider_transaction` (MAR Art. 19), `dividend`, `profit_warning`, `significant_contract`, `own_shares` (own-share/treasury transactions, purchases and sales; generalized from `buyback` in migration 0044), `guidance_change`, `general_meeting`, `auditor_opinion` (auditor red flags — qualified opinion / disclaimer / negative opinion / going-concern emphasis; migration 0079, ADR 0069), `short_position_change` (KNF short-selling register changes; migration 0080, ADR 0069 — emitted directly by the KNF adapter, empty patterns), `recommendation_change` (analyst-recommendation revisions; migration 0100, ADR 0073 — emitted directly by the recommendation adapter, empty patterns), `other`.
- `rule_definition_json` is consumed by the interpretation-layer `RuleClassifier` ([ADR 0035](adr/0035-two-layer-ai-and-local-interpretative-layer.md)). Shape: `{ "patterns": [..], "confidence": 0.0..1.0 }`, where any case-insensitive substring match against the filing text selects the category. An empty `patterns` list never rule-matches — `other` carries no patterns and is reachable only via the AI fallback.
- `derives_event = 1` marks categories that materialize a derived `company_events` row when the filing carries a future date. Only `dividend` and `general_meeting` derive events (ADR 0034); all other seed categories are `derives_event = 0`.
- The registry is source-neutral so a future GPW re-enable feeds the same classifier.

### Short Positions (KNF)

Mirrors the KNF public national register of net short positions for tracked GPW companies and keeps an append-only change history. Fed by the `knf-short-selling` disclosure adapter (migration 0080, [ADR 0069](adr/0069-source-reliability-and-disclosure-signals.md) decision 3). The adapter is wired into the registry (`SourceAdapterDescriptor`, visibility `optional`) and the refresh dispatch (`Fetcher` trait); an empty register snapshot is rejected at the refresh seam rather than diffed (it would read as a mass exit).

`short_positions` (current-state mirror, one row per holder-in-issuer):

- `id`
- `company_id` (canonical company; matched by ISIN)
- `holder_name` (HTML entities decoded)
- `isin`
- `net_position_pct`
- `position_date`
- `modify_date`
- `exited_at` (NULL = currently in the register at ≥ 0.5%; non-NULL = dropped below the register threshold)
- `created_at` / `updated_at`
- `UNIQUE(company_id, holder_name)`

`short_position_events` (append-only history, one row per detected change):

- `id`
- `company_id`
- `holder_name`
- `kind` — `entered` | `increased` | `decreased` | `exited`
- `from_pct` / `to_pct` (nullable; `from_pct` null on entry, `to_pct` null on exit)
- `position_date`
- `created_at`

Rules:

- Entries are matched to companies by ISIN; unmatched issuers are **skipped**, never auto-created.
- Diffing is idempotent: re-ingesting the same register snapshot detects zero changes and produces no new events, feed items, or signals.
- Each detected change writes one `feed_items` row (`Official report` type, `knf-short-selling` adapter) and one `confirmed` `short_position_change` `company_signals` row, firing matching `signal_category` alert rules ([ADR 0068](adr/0068-attention-routing.md)) on the same path as the ESPI classifier.
- Exit detection treats the current register as complete for positions ≥ 0.5%: a stored live position whose holder is absent from a fresh snapshot is marked `exited`.

**Read model** (`v0.55` T4b): `short_positions_view(company_id)` composes the cockpit panel view (contract in [Contracts § Short Positions (KNF)](contracts.md#short-positions-knf)) — never a stored projection. Active positions (aggregate = sum of active `net_position_pct`), the change history newest-first by `position_date`, the most recent remembered exit, and `delta_30d_pp` = the **signed sum of in-window event deltas** (entered `+to`, increased/decreased `to−from`, exited `−from`). The signed-delta sum is used because a clean "aggregate 30 days ago" is not reconstructable from the current mirror alone (only the latest per-holder value is stored); it equals `aggregate_now − aggregate_30d_ago` since the ingester writes exactly one event per detected change. The 30-day window is anchored to the read's UTC date.

### Analyst Recommendations

Sell-side recommendations (rating, target price, issuing firm) for tracked GPW companies, kept strictly as attributed third-party opinions — never as advice ([ADR 0073](adr/0073-analyst-recommendations-tracking.md)). Fed by the `biznesradar-rekomendacje` analyst-recommendation adapter (migration 0100; runtime adapter slice A2). The free source page carries only the most recent items, so history accumulates **append-only from ingestion start** — it cannot be backfilled.

`analyst_recommendations` (append-only recommendation history, one row per issued recommendation):

- `id` (deterministic — sha256 of the natural key)
- `company_id` (canonical company; FK CASCADE)
- `firm` (issuing brokerage, verbatim) / `analyst` (nullable)
- `rating` (source vocabulary preserved **verbatim**, e.g. `akumuluj`) / `rating_prev` (nullable — the same-firm prior rating, derived)
- `direction` — `upgrade` | `downgrade` | `initiate` | `reiterate` (CHECK)
- `target_price` / `target_currency` / `target_prev` (nullable; decimal-exact TEXT, the repo convention)
- `price_at_issue` (nullable; decimal-exact TEXT)
- `published_at` (ISO-8601) / `source_url` / `report_url` (nullable broker PDF)
- `created_at` / `updated_at`
- `UNIQUE(company_id, firm, published_at, rating, COALESCE(target_price, ''))` — the natural key (ADR 0073 decision 4); a per-`(company_id, published_at DESC)` index backs the read path.

Rules:

- **Natural-key dedupe**: `INSERT … ON CONFLICT DO NOTHING` on the deterministic id + the natural-key UNIQUE index. Re-ingesting the same page is a no-op — no new rows, feed items, or signals.
- **Direction derivation**: the source page has no "rating before"; `direction`, `rating_prev` and `target_prev` are derived at ingest by comparing each entry against the latest prior stored entry of the **same firm** (`published_at` order). No prior → `initiate`. Known ratings on both sides compare by rank (`kupuj` > `akumuluj` > `trzymaj` > `redukuj` > `sprzedaj`): higher → `upgrade`, lower → `downgrade`, equal → `reiterate` unless the target moved (then follow the target). A rating outside the known vocabulary falls back to the target direction, else `reiterate`.
- Each **newly** inserted recommendation writes one `feed_items` row (`Official report` type, `biznesradar-rekomendacje` adapter) and one `confirmed` `recommendation_change` `company_signals` row, firing matching `signal_category` alert rules ([ADR 0068](adr/0068-attention-routing.md)) on the same path as the KNF short-position ingester.
- Companies are pre-resolved by the adapter (BiznesRadar redirects the GPW ticker to its canonical slug); the ingester takes a `company_id` directly.

**Read models**: `list_analyst_recommendations(company_id)` — the history newest-first by `published_at` (never `created_at`); `latest_target(company_id)` — the newest entry that carries a target price (firm + date + target), for the attributed "vs target" readout beside market data.

### Source Reconciliation (ESPI/EBI witness)

The persisted GPW ESPI/EBI witness ↔ Bankier agreement ledger ([ADR 0069](adr/0069-source-reliability-and-disclosure-signals.md) decision 2, plan v0.55 T3). Migration `0081_espi_witness_reconciliation.sql`. The `gpw-espi-ebi` adapter runs as a **witness** (`role = witness`): it fetches the official ESPI/EBI listing and reconciles it against Bankier-sourced reports **without ever ingesting feed items** (no dual ingestion).

`source_reconciliation_results` — `(id, witness_adapter_id, company_id?, report_number?, report_type?, disclosure_date, witness_title, witness_url?, status, primary_feed_item_id?, created_at, updated_at)`.

- `status` ∈ `matched` | `espi_only` | `bankier_only` (CHECK). `matched` — a witness item a Bankier report also carries (`primary_feed_item_id` links it); `espi_only` — a witness item the primary channel missed; `bankier_only` — a Bankier report inside the window with no witness match.
- `company_id` is nullable and resolved by ISIN; untracked issuers are skipped.
- `id` is a **deterministic, status-independent** key (from witness_adapter + company + disclosure_date + report identity), so a re-reconciled pair UPSERTs in place — idempotent, and its status can flip (`espi_only → matched`) once the primary catches up.
- Matching is tolerant: exact ESPI report-number match (`N/YYYY`, e.g. Bankier "RB 15/2026") first, then a `(company, disclosure date)` fallback. Window = `[earliest witness disclosure date, now]` (default 7-day lookback when the listing is empty).
- An `espi_only` result for a tracked company raises a **system** `attention_events` row (`trigger_type = source_reconciliation`, `evidence_ref = result id`), surfaced through the v0.54 attention routing (Today stream + toast + morning briefing). The full ledger is developer-diagnostics only (`list_source_reconciliation`).

### Ownership Stakes

Who owns each tracked company and how that changes over time ([ADR 0072](adr/0072-ownership-structure.md) — amended 2026-07-16, plan v0.56 T2). Migration `0082_ownership_stakes.sql`. Stored as **append-only snapshots** per `(source, as_of)` — history is the product, matching the financial-facts philosophy. Prior snapshots are never rewritten (one scoped exception: an `aggregator` same-basis re-ingest reconciles that basis's row set, see the witness section). Ingested from the BiznesRadar aggregator (breadth), stored periodic reports and the ESPI `major_holdings_change` signal (depth/freshness); `manual` entry is always available.

`ownership_stakes` (append-only stake snapshots):

- `id` — deterministic (`ownstake_{company}_{source}_{as_of}_{holder_normalized}`, `slug_part` idiom); a given `(company, source, as_of, holder)` maps to one row, so re-ingest **upserts in place**.
- `company_id`
- `holder_name_raw` (as printed) / `holder_name_normalized` (trim + collapse whitespace + uppercase — the **stable grouping key** that derives the stake id, so it is deliberately NOT deepened; T5's legal-form stripping lives in a separate matching-only `canonical_holder_key`, see Holder-type classification below)
- `holder_type` — `founder_insider | family_foundation | tfi | ofe_pension | state_treasury | parent_company | treasury_shares | other_institutional | free_float_rest` (CHECK). **Nullable** = not yet classified (dictionary miss awaiting AI/manual re-type) — a NULL rather than a sentinel value.
- `capital_pct` / `votes_pct` — **two separate** decimal-exact TEXT columns (the `financial_facts.value_numeric` convention, parsed with `rust_decimal`), each **nullable**: reports sometimes disclose only one, and preferred-vote shares make the gap itself investor signal.
- `as_of` (domain date)
- `source` — `report_document | espi_filing | aggregator | manual` (CHECK)
- `report_document_id?` / `feed_item_id?` — provenance to the exact document / filing (nullable; `ON DELETE SET NULL`)
- `created_at`
- `UNIQUE(company_id, source, as_of, holder_name_normalized)`

Rules:

- **Append-only**: same `(company, source, as_of, holder)` updates the classification / percentages / provenance in place (never `created_at` or the domain key); a **new `as_of` always inserts** a fresh row, preserving the timeline.
- **Current state** (`current_state`) = the latest disclosed stake per holder, selected by **`as_of`** (tie-break: latest `created_at`, then id) — **never `created_at`**, which backfill makes diverge from the domain date.
- **Free float is NOT stored** — it is derived at read time: `100 − Σ disclosed capital_pct`, floored at 0, returned by `current_state_with_free_float` together with the component sum (`disclosed_capital_sum`) so the UI can show the uncertainty note (sub-threshold stakes hide in the float). `free_float_history` derives one point per **full-picture basis** (`report_document` and `aggregator` `as_of` groups, identity-deduped first); ESPI single-holder updates are deliberately not bases.
- **Manual re-type** (`set_holder_type`) corrects `holder_type` across the holder's rows only — a classification label, never a new snapshot, and it never touches pct/as_of/history.

`ownership_holder_dictionary` (holder classification, **seeded as data — extensible without code**):

- `alias_normalized` (PK, uppercase normalized alias) → `holder_type` (same CHECK set) + `display_name?`, `created_at` / `updated_at`.
- Seeded idempotently by migration 0082 with a starter set (major Polish TFI, OFE, State entities, treasury-share patterns); the deterministic classifier (`load_holder_dictionary`) looks holders up here, and new entries are added by inserting rows, not changing code.

Owner-durable: the `ownership_stakes` section joins the v2 research import/export bundle. Import is idempotent by deterministic id (existing snapshots are skipped, never rewritten — append-only); provenance ids are kept only when they resolve in the target DB, else nulled.

**Interface note for v0.57 (red-flags / insider sentiment)**: the fund-exit feed is read-only over this model — an exit event = a holder present in the previous **full-picture disclosure basis** and absent from the newest, or an `espi_filing` `major_holdings_change` snapshot whose resulting `capital_pct`/`votes_pct` **crosses below the 5% disclosure threshold** (or states zero). A mere decrease that stays above the threshold is not an exit and raises nothing (refined 2026-07-17 at T7 — the original "decreased" wording over-flagged). Both derive from `ownership_stakes` history + `current_state`; v0.57 adds no ownership tables.

**Current-state read = newest disclosure basis, not latest-per-holder-over-history** (real-data harvest 2026-07-16): a holder who drops below the 5% disclosure threshold *vanishes* from later filings (no "0%" row exists), so a naive latest-per-holder union resurrects stale holders and pushes the disclosed sum past 100% (free float goes to 0). `current_state` therefore scopes to a **baseline** = `max(as_of)` of the company's **full-picture bases** — `report_document` and `aggregator` snapshots (ADR 0072 amendment: the newest full picture wins, whichever source it came from; fallback: any source) — returns the latest per holder at `as_of >= baseline`, and overlays later `espi_filing`/`manual` snapshots. Pre-baseline holders remain in `history` only. **Founder-insider sticky overlay (F-A1, owner dogfooding 2026-07-17):** a `founder_insider` holder is exempt from the vanish rule — a founder crossing below the disclosure threshold is itself an ESPI-disclosable event (not a silent vanish) AND is corroborated by the management/insider substrate, so when a newer full-picture basis only *partially* captures the shareholder table (e.g. an OFE-only quarterly that missed the founders — the live ABE shape), each founder's most-recent stake is overlaid into current state whenever that holder identity is absent from the baseline-scoped set. Without this the founder — and its skin-in-the-game corroboration badge, which attaches only to a surfaced holder — disappears entirely, and free float is overstated. The witness comparison path uses a **disclosed-only reference read** (same baseline logic restricted to non-`aggregator` sources) so the aggregator is never compared against itself. Free float derives from this scoped state. Within the scoped set, rows merge by **holder identity**: a shared dictionary `display_name` when the name resolves to a seeded entity alias (`HolderIdentityMap`; "NN PTE" = "Nationale-Nederlanden PTE S.A.", migration `0086`), else the parenthetical-stripped canonical key ("cyber_Folks S.A." = "cyber_Folks S.A. (akcje własne)"); generic marker aliases (`treasury_shares`) never act as identities. The most specific raw name represents the merged holder; history keeps every variant.

**Report extraction** (plan v0.56 T3, migration `0083_ownership_extraction_residual.sql`, [ADR 0072](adr/0072-ownership-structure.md)): the `ownership_extraction` job parses the mandatory shareholders table of a stored periodic report and writes stakes **directly and finally** with `source = report_document` (owner decision 2026-07-16 — the deterministic parse needs no confirmation). A parse it cannot turn into holder rows (glyph-mangled font, image table, missing section) writes **zero** stakes and is parked in `ownership_extraction_residual` for the later AI/OCR path, whose results always require confirmation.

`ownership_extraction_residual` (the deterministic parser's pending queue):

- `report_document_id` (PK → `report_documents`, `ON DELETE CASCADE`) — one residual per document, so a re-run **upserts in place**; the job **clears** it the moment a (later) parser version succeeds, so a document is never both parsed and residual.
- `company_id` (→ `companies`, `ON DELETE CASCADE`)
- `parse_state` — `section_missing | table_unparsable | glyph_encoded` (CHECK) — why the deterministic parse could not write stakes.
- `detected_as_of?` — the disclosure date resolved for the document, if any (carried so the AI/OCR write reuses it).
- `matched_heading?` — the shareholders heading line that anchored the failed parse, verbatim (NULL for `section_missing`).
- `ocr_state?` (v0.57 T8, migration `0093`) — the tier-4 OCR lifecycle marker: `NULL` = never attempted (eligible for a bulk OCR pass); `proposed` = a pending OCR proposal awaits review; `rejected` = the user rejected the OCR proposal (never re-proposed); `no_table` = OCR ran (or the doc is un-OCRable, e.g. an ESEF/iXBRL route) and yielded no shareholders table. Enforced in code (`OCR_STATE_*`), no CHECK on the ADD COLUMN. A deterministic re-parse (`record_extraction_residual`) never resets it.
- `created_at` / `updated_at`.

**Tier-4 OCR of residuals** (v0.57 T8, migration `0093_ownership_ocr_proposals.sql`, [ADR 0077](adr/0077-trusted-extraction-foundations.md) tier-4 over [ADR 0072](adr/0072-ownership-structure.md) decision 2a): the `ownership_ocr_extraction` job OCRs a residual document through the routable `vision_extraction` capability ([ADR 0060](adr/0060-ai-capability-routing-and-openai-compatible-provider.md) — Mistral OCR; **no** general-analysis fallback, and no vision provider is a **clean no-op**, never an error), parses the shareholders table out of the OCR markdown with the SAME deterministic parser (`parse_ocr_shareholders` — OCR defeats the glyph encoding by reading pixels), and lands the result as a proposal in `ownership_ocr_proposals` — **NEVER auto-applied** ([ADR 0072](adr/0072-ownership-structure.md) decision 2a: OCR/AI results always require confirmation).

**Which file is OCR'd** (real-data gap closed 2026-07-17: on the maintainer DB the `table_unparsable` residuals are 16 PDF / 27 xhtml — a PDF-only gate would skip 61%): a **PDF** residual OCRs itself; an **xhtml** pdf2htmlEX container (unreadable text layer — the very reason it is residual) OCRs its fetched **PDF sibling** of the same company + derived period (`find_pdf_sibling`, the single sibling rule shared with the management-holdings glyph path); an ESEF report package (`.xbri`/`.zip`) or an xhtml with no PDF sibling is un-OCRable → `ocr_state='no_table'`, no provider call. Tier-4 is still PDF-native (the pure-Rust build cannot rasterize) — the sibling is the PDF-native content the xhtml container was rendered from.

`ownership_ocr_proposals` (**transient, not in the import/export bundle**): `report_document_id` (PK → `report_documents`, `ON DELETE CASCADE`) — the residual document this proposal resolves, one proposal per residual (idempotent upsert; a re-OCR reconciles); `source_document_id` (→ `report_documents`) — the document actually OCR'd (equals `report_document_id` for a PDF residual, the PDF sibling for an xhtml residual — OCR-run provenance, surfaced in the review card); `company_id`, `as_of` (the deterministically-resolved disclosure date every written stake carries — the residual's `detected_as_of`, else the document-period derivation; never fabricated), `matched_heading?`, `provider_id?`, `model?`, timestamps. Rows in `ownership_ocr_proposal_rows` (`id` PK, `report_document_id` → cascade, `row_index`, `holder_name_raw`, `capital_pct?`, `votes_pct?`).

**Bulk vs manual selection (parse-state scope)**: the **bulk** pass (`run_ownership_ocr_extraction`) selects residuals whose `parse_state` is `table_unparsable` OR `glyph_encoded` (the card's target population — unreadable tables OCR can defeat) and `ocr_state IS NULL` (a `no_table` doc is never re-spent). `section_missing` residuals (a document genuinely lacking the shareholders section) are **excluded** from bulk to avoid burning provider calls on nothing. The **manual per-company** pass (`run_company_ownership_ocr`) is broader (an explicit user retry): it re-arms the company's `no_table` residuals **and** includes `section_missing`.

**Confirm/reject + re-propose rule** (the `ocr_state` marker gates re-selection): **confirm** writes each proposed row as a `report_document` stake at `as_of` (the standard extraction write path), stamps deterministic holder types, **clears the ORIGINAL residual entirely** (the gap is filled), and deletes the proposal — the residual and proposal never coexist after. Stakes anchor their `report_document_id` provenance to the **residual** document (so the deterministic catch-up sees the period covered and never re-parks/re-OCRs it), while the PDF actually read is preserved on the proposal's `source_document_id`. **Reject** deletes the proposal and parks the residual `ocr_state='rejected'` so it is **not** re-proposed. A provider error creates no proposal and leaves the residual eligible (`NULL`, retryable). A `proposed`/`rejected` residual is never re-selected by either pass.

**`as_of` resolution order** (deterministic and stable across re-runs, so the append-only stake id never churns): (1) the document's linked `financial_periods.period_end_date` (via `period_id`); (2) else the document-period derivation (`derive_report_period` — the ESEF iXBRL context end, else the title/URL end-of-period); (3) else the first date at/after the matched shareholders heading in the extracted section text. A parse that yields rows but no resolvable date writes nothing (never fabricates a date) — it is parked as a `table_unparsable` residual instead.

**Triggers** (deterministic CPU parse on the **autopilot** worker lane, independent of autopilot mode): *on-new-report* (post-refresh) and *app-startup catch-up* enqueue every fetched periodic document lacking coverage (no `report_document` stake and no residual); a *backfill* pass force-enqueues every fetched periodic document of a company (UI/T6 + epic backfill). Residual is transient/derived state — **not** part of the import/export bundle.

**Holder-type classification** (plan v0.56 T5, [ADR 0072](adr/0072-ownership-structure.md) §3). A holder's `holder_type` is resolved in a fixed order, each step only touching still-NULL rows so an earlier decision is never overwritten:

1. **Dictionary** — `ownership_holder_dictionary`, matched on a *canonical key* (`canonical_holder_key`: fold Polish diacritics, drop punctuation, uppercase, collapse whitespace, strip legal-form suffixes/prefixes — S.A./Sp. z o.o./S.à r.l.… — while **keeping** type-signal tokens TFI/OFE/PTE/DFE/FIZ/SFIO; pure, deterministic, idempotent). Two modes: exact alias hit, then containment (a canonical alias appearing as a whole-token run inside the holder key, **longest-alias-wins**).
2. **Heuristic name markers** — an unambiguous signal the name itself carries when the dictionary misses: `OFE`/"otwarty fundusz emerytalny" → `ofe_pension`; `TFI`/"towarzystwo funduszy inwestycyjnych" → `tfi`; a name beginning `FUNDACJA` → `family_foundation`; "akcje własne" → `treasury_shares`; "skarb państwa" → `state_treasury`. A plain issuer name (e.g. "cyber_Folks S.A.") never matches — it stays NULL.
3. **AI classify-with-confirm** — the residual (dictionary miss + no marker) is proposed by the routable `ownership_holder_classification` AI capability ([ADR 0060](adr/0060-ai-capability-routing-and-openai-compatible-provider.md)) into `ownership_holder_type_proposals` and is **never auto-applied**.
4. **Manual re-type** (`set_holder_type`) always wins and is never overwritten by a re-classification.

Steps 1–2 run in `classify_unclassified_for_company` (called by the T3 extraction job and the UI); it stamps NULL rows only, creates no snapshot, and touches no history.

`ownership_holder_type_proposals` (migration `0084_ownership_holder_type_proposals.sql`, AI proposals — **transient, not in the import/export bundle**):

- `id` — deterministic (`ownhtp_{company}_{holder_normalized}`, `slug_part` idiom) + `UNIQUE(company_id, holder_name_normalized)`: one live proposal per holder, so a re-run **upserts in place** (never duplicates), and a **confirmed** proposal is never disturbed.
- `company_id` (→ `companies`, `ON DELETE CASCADE`), `holder_name_normalized` (same key as `ownership_stakes`).
- `proposed_type` (same CHECK set as `ownership_stakes.holder_type`), `confidence?` (REAL 0–1), `rationale?`.
- `status` — `pending | confirmed | rejected` (CHECK, default `pending`). **Confirm** (`confirm_holder_type_proposal`) applies the type across the holder's rows via `set_holder_type`; **reject** (`reject_holder_type_proposal`) just marks it, leaving `holder_type` NULL. A malformed/low-signal model response yields no proposal and never a stamp.
- `provider_id?` / `model?` (capability-routed provenance), `created_at` / `updated_at`.

**ESPI major-holdings stake update** (plan v0.56 T4 stream 2, migration `0085_major_holdings_and_ownership_witness.sql`, [ADR 0072](adr/0072-ownership-structure.md) §2b). Migration 0085 seeds the `major_holdings_change` signal category (rule classifier over formulaic art. 69 title patterns — "znaczny pakiet akcji", "art. 69", "zmiana udziału w ogólnej liczbie głosów", …). When Bankier ingestion classifies a **confirmed** `major_holdings_change` signal, a post-classification sweep (`update_stakes_from_major_holdings`) runs a **conservative deterministic** parse of the notification body (`fundamentals::ownership::espi_notification`):

- A **clean** parse (unambiguous holder + at least one resulting percentage; capital % and votes % kept separate) writes a stake with `source = espi_filing`, `as_of` = the filing disclosure date, and `feed_item_id` provenance. Idempotent: the deterministic stake id + a once-per-filing gate mean re-ingest never duplicates.
- **Any ambiguity is never guessed** (silently-wrong ownership is worse than absent): conflicting before/after percentages, or no confidently-extractable holder, write **zero** stakes and park the filing in `ownership_espi_unparsed` (PK `feed_item_id` → `feed_items` `ON DELETE CASCADE`; `company_id`, `reason`, timestamps) — so each filing is attempted exactly once — plus a paired diagnostic event (`ownership_espi_unparsed`, developer-mode).

**Aggregator ownership breadth source + reversed witness** (plan v0.56 T4 stream 3, migration 0085, [ADR 0072](adr/0072-ownership-structure.md) §2c as amended 2026-07-16). Adapter `biznesradar-akcjonariat` (`ownership` type, `primary` role, `optional`, daily): for each tracked GPW company it fetches BiznesRadar's public `/akcjonariat/<ticker>` page and **writes the "Główni akcjonariusze" table as a full-picture `aggregator` snapshot**. Table scope (real-page harvest 2026-07-16): the page carries TWO identically-headed tables — "Główni akcjonariusze" (the ≥5% disclosure picture; **the only one ingested**, anchored by its section heading with a first-`Akcjonariusz`-header-table fallback) and "Pozostali akcjonariusze" (sub-5% stakes lifted from fund financial statements — deliberately NOT ingested; deferred as a possible fund-positions depth feature). Guards: a row containing any `<th>` cell is a header/summary row ("razem") and is skipped; a row is rejected unless its holder name is non-empty and not itself a percentage and each parsed percentage is ≤ 100; a basis whose disclosed capital sums > 102 is **implausible — nothing is written** and a diagnostic is recorded (counts as a failed page for the all-fail guard). All written rows share one `as_of` = the newest "Data aktualizacji" in the ingested table (fallback: fetch date), so an unchanged page upserts the same basis in place; a **same-basis re-ingest reconciles the row set** (a holder gone from the page within an unchanged `as_of` is deleted — `aggregator` rows at that basis only; prior bases and other sources are never touched). Newly written holders go through the deterministic classification steps 1–2 (`classify_unclassified_for_company`). The refresh then compares the fetched table against the **disclosed-only reference read** (reports/ESPI witness the aggregator). Divergence = a holder present on only one side above the 5% threshold, or a matched holder's capital % (votes % fallback) differing by >1.0 pp; each divergence is a diagnostic event (`witness_divergence`).

`ownership_witness_results` (last comparison per adapter+company — **transient/observability, not in the import/export bundle**):

- `id` — deterministic (`ownwit_{adapter}_{company}`) + `UNIQUE(adapter_id, company_id)`: one row per adapter+company, re-run **upserts in place**.
- `adapter_id`, `company_id` (→ `companies`, `ON DELETE CASCADE`).
- `status` — `agree | diverged | no_reference` (CHECK; `no_reference` = we hold no disclosed stakes to compare yet).
- `holders_compared`, `divergence_count`, `checked_at`, `created_at` / `updated_at`.

A witness run marks the adapter healthy (`source_adapters.last_success_at`); an all-fail run records an error and does not (mirrors the KNF/GPW empty-witness guard).

### Company Health (Insider Substrate + Red-Flag Acks)

Status: planned (v0.57.0, ADR 0083)

Deterministic health scores, the parsed insider substrate, and red-flag acknowledgements ([ADR 0083](adr/0083-company-health-scores-and-red-flags.md)). Scores (Piotroski F, Altman Z″) are **never stored** — computed as-of-period over confirmed facts via the metrics context. v0.57 adds **no ownership tables** (see the Ownership interface note above); the insider tables below are siblings joined by canonical holder identity.

`insider_transactions` (parsed MAR art. 19 notifications; new extraction target — shape refined 2026-07-17 from the hand-labeled real-DB ground truth, 22 filings / 30 transactions):

- `id` — deterministic from `(feed_item_id, unit_index)` (a filing commonly carries several enumerated notifications/transactions; first-match-wins is the known failure mode)
- `company_id`, `feed_item_id` (provenance to the classified `insider_transaction` filing)
- `person_name_raw` / `person_normalized` (ownership normalization idiom; names appear in Polish genitive — normalized form is the join key)
- `role?` — `management | supervisory | closely_associated` (CHECK, **nullable**: a bare "osoba pełniąca obowiązki zarządcze" with no board qualifier stays NULL, never guessed)
- `related_pdmr_raw?` / `related_pdmr_normalized?` / `related_pdmr_role?` — for `closely_associated` rows, the anchoring PDMR (the skin-in-the-game join needs the anchor, not the vehicle entity)
- `direction?` — `buy | sell | other` (CHECK, **nullable**: ~1/4 of cover notes state only "powiadomienie o transakcjach"; the direction lives in the attached PDF)
- `instrument?` — `shares | subscription_warrants | other` (CHECK, nullable)
- `volume?`, `price?`, `currency?` — decimal-exact TEXT, nullable (PDF-only for most filings; never fabricated)
- `tx_date?` (domain date, nullable), `created_at` / `updated_at`

Ground-truth reality (drives the tiering): the Bankier `body_text` is the ESPI **cover note** — person/role/direction parse deterministically from it, but volume/price/tx-date live in the attached notification PDF (not fetched today) for ~90% of transactions. Body parsing emits what the body states and leaves the rest NULL; the attachment-PDF tier is a follow-on card (`T4b`) reusing the ADR 0061 deterministic PDF parser. The seeded `insider_transaction` rule patterns are corrected in the same change (real corpus: 0/22 matched the original seeds) with a reclassification backfill over stored official filings and a corpus test over real title forms.

`insider_espi_unparsed` (parking marker; mirrors `ownership_espi_unparsed`): a classified `insider_transaction` filing whose cover note yields **no** writable unit (all Ambiguous / NotFound) is recorded here once — `feed_item_id` (PK), `company_id`, `reason`, timestamps — so the deterministic parse sweep attempts each filing exactly once and re-runs create zero rows. NO transaction row is written (never guess). Cleared implicitly when a later parse succeeds (the sweep only considers filings with neither a transaction row nor a parking marker). **Triggers (F-B, owner dogfooding 2026-07-17):** the cover-note parse sweep runs both *after each source refresh* AND as an *app-startup catch-up* (`state.insider().parse_pending()`, wired in `lib.rs` setup alongside the ownership and management-holdings catch-ups) — so a cold DB carrying confirmed `insider_transaction` signals from a prior version populates the insider timeline on first launch, not only after a manual refresh. Idempotent (attempt-once), so startup writes zero duplicate rows.

**Attachment-PDF tier (`T4b`, migration 0094) — no new attachment table.** The MAR art. 19 transaction figures live in the attached "Powiadomienie…" notification document, which is **already registered at ingest** as a `report_documents` row (`source_type='espi_attachment'`, `origin_ref=<feed_item_id>`, `fetch_status='metadata_only'` for a non-periodic filing). T4b reuses that row: it fetches the document on demand (the existing report-document fetch infra + `DocumentFetcher`, throttled/politeness inherited), runs the shared ADR 0061 extraction tier (PDF **or** ESAP-derived xhtml — format resolved from `content_type`/URL), and deterministically parses the standard KNF/ESMA notification form (`fundamentals::insider::attachment` — label-anchored person/role and per-transaction figures; `Units | NotFound`, never guessed). A scanned/no-text-layer document is parked for the vision path, never guessed. **Merge rule** (into `insider_transactions`): each parsed transaction row is matched greedily one-to-one to an existing unit by **(person, direction, tx_date)** with NULL-tolerant fields (a NULL body field matches anything; person matching is declension-tolerant, bridging the attachment's nominative names to the cover note's genitive-recovered key). On a match, **only still-NULL fields are filled** (`volume`/`price`/`currency`/`tx_date`/`instrument`/`role`/`related_pdmr`); a disagreement with an existing **non-NULL** value changes nothing and is recorded as a typed conflict. A PDF row matching no existing unit is **appended** as a new unit whose `unit_index` extends `max(existing)` (the CMP second-disposal class). The sentiment read model needs no change — its coverage note (`volume_known`/`volume_total`) rises automatically as NULLs become known.

`insider_attachment_attempts` (attempt-once marker; mirrors the `insider_espi_unparsed` idiom): a classified insider filing whose attachment tier reached a **terminal** outcome is recorded once — `feed_item_id` (PK), `company_id`, `outcome` (`parsed | no_attachment | no_text_layer | not_found` CHECK), and the merge diagnostics `filled` / `appended` / `conflicts` counts, timestamps. The sweep selects only filings with a transaction row and no attempt marker, so each is fetched/parsed exactly once and re-runs issue zero fetches. A **transient fetch failure is not recorded** (the `report_documents` row stays retryable), so it retries on the next sweep; the fill-NULLs merge is idempotent regardless. Backfill (`backfill_company_insider_attachments`) clears a company's markers to force one deterministic re-attempt.

`management_holdings` (parsed from the mandatory management-holdings section of a periodic financial statement **or the management activity report (SzD, "Sprawozdanie z działalności zarządu")**; card `9730f5f` — shape refined 2026-07-17 from the hand-labeled ground truth, 15 documents / 67 person-rows / 13 issuers). The SzD is `doc_kind='other'` (it is deliberately not a financial statement), but the holdings table often lives ONLY there — KRU discloses management holdings solely in its SzD — so the extraction selection matches periodic statements **plus** SzD documents (pure `is_management_report(title,url)` predicate; F-A3, owner dogfooding 2026-07-17):

- `id` — deterministic from `(report_document_id, person_normalized)`
- `company_id`, `report_document_id` (provenance)
- `person_name_raw` / `person_normalized` (uppercase idiom, `normalize_holder_name`), `role?` — `management | supervisory` (CHECK, nullable; from organ keywords / inline role cell / in-table organ subheader)
- `shares?` — decimal-exact TEXT, **nullable**: a zero holding is a real `0` row (zero skin-in-the-game is signal); NULL = stated but unreadable (glyph-blanked digits) **or** a `-`/`nd.` cell (person listed but holding not stated for that as-of — kept as a row with `shares = NULL`, never coerced to `0`, never dropped; ground-truth-validated 2026-07-17) — never guessed.
- `indirect_via_raw?` / `indirect_via_normalized?` — the holding vehicle when the section states "pośrednio poprzez …" / a family foundation; **the founder-badge join bridge** (founders typically hold via vehicles — a person-name-only join misses them)
- `prior_shares?` / `prior_as_of?` — when the table carries a before/after or Nabycie/Zbycie/zmiana column
- `as_of` — explicit "na dzień <date>" on the section/caption when present, else the report `period_end_date` (document-period resolution reused from ownership extraction), `created_at` / `updated_at`

Parser outcome states mirror ownership extraction: `Parsed | ZeroHoldingAggregate` (prose "nie posiadają akcji", kept as an explicit zero picture) `| GlyphEncoded | SectionMissing` — glyph/image/unresolved-heading documents park as residual for the tier-4 OCR path, never guessed. **Person-plausibility gate (F-A2, owner dogfooding 2026-07-17):** at emission each assembled name is dropped when it is unmistakably NOT a natural person — a company/legal-form/counterparty suffix (`SP`/`LIMITED`/`S.A.`…), a street-address prefix (`ul.`/`al.`), an institution/office/role-boilerplate token (`Skarb Państwa`, `Dyrektor Izby`, `Zleceniodawca Wystawca`), a generic-business word (`Marketing`…), a genitive-case surname fragment (`…Wnorowskiego…`, a prose mention), or a plausible YEAR (1990–2035) mis-captured as a share count on a 3+-token all-ASCII name. A leading courtesy title (`Pan`/`Pani`) is stripped, not rejected. The gate is conservative by construction (every blocklist token is a word no Polish personal name carries), validated to hold the ≥95% ground-truth person-recall floor and drive corpus-wide emitted junk to zero (`real_data_management_holdings_junk_rate`). Anchoring requires a holdings token co-occurring with the organ phrase (board-composition / remuneration / diversity-policy sections are known false anchors), scoped to the issuer's own shares (subsidiary-share tables excluded). A glyph-encoded xhtml (a pdf2htmlEX container, VRC class) first attempts its **PDF sibling** — a fetched periodic PDF of the same company and period under the Rust pdf-extract tier — before parking a glyph residual.

**`ZeroHoldingAggregate` representation:** a prose zero statement is stored as `management_holdings` rows with `is_zero_aggregate = 1` and a reserved `person_normalized` sentinel — `__ZERO_MANAGEMENT__` / `__ZERO_SUPERVISORY__` (per-organ, XTB class) or `__ZERO_AGGREGATE__` (whole board, `role` NULL, GPW class). One sentinel row per stated organ; `shares = '0'`. These sentinels are excluded from the by-person read model and never feed the founder-stamping join.

`management_holdings_residual` (parking marker; mirrors `ownership_extraction_residual`): a fetched periodic document whose holdings section the **deterministic** parser could not read is recorded once — `report_document_id` (PK), `company_id`, `parse_state` (`section_missing | table_unparsable | glyph_encoded` CHECK), `detected_as_of?`, `matched_heading?`, timestamps. NO holdings row is written (never guess). Cleared by the extraction job the moment a later parse succeeds, so a document is never both parsed and residual. The catch-up selection enqueues exactly the fetched docs (periodic statements + SzD management reports) with neither a `management_holdings` row nor a residual — the startup catch-up and the on-new-report seam share this one selection so coverage never disagrees.

**Founder/insider stamping (both tables):** one reusable join (`stamp_founder_insiders`, over `management_holdings` + `insider_transactions`) matches canonical identities — by person name, or via the `indirect_via` vehicle (vehicle-normalized ↔ stake holder, `HolderIdentityMap` aliases apply). Exact-name join only — never surname-collapse (real case: Adam vs Michał Kiciński are different holders). `ownership_stakes.holder_type = founder_insider` is stamped on **still-NULL rows only** — an earlier dictionary/AI/manual label is never overridden (the `classify_unclassified` precedence rule; a founder's family foundation legitimately stays `family_foundation` in the donut). The skin-in-the-game **badge therefore keys off the read-model corroboration join** (`skinInTheGame` on the ownership holder), not the literal `holder_type` — so a corroborated vehicle badges regardless of its type label, and a 0-direct management row with an entity-held stake still badges via the vehicle. A no-match writes nothing to ownership.

`red_flag_acks` (the only persisted red-flag state — active flags are a computed read model, `red_flags_view`):

- `flag_id` (PK) — deterministic per flag instance (type + company + evidence identity), so a re-detection never re-raises an acked flag
- `company_id`, `flag_type`, `acked_at`

New `signal_categories` seed rows (empty patterns — emitted by detection jobs, KNF `short_position_change` precedent): `report_delay`, `fund_exit`, `score_deterioration`. Raising a flag writes one synthetic `feed_items` row + one `confirmed` signal, so ADR 0068 `signal_category` alert rules fire unchanged.

New `kpi_definitions` seed rows: reported `current_assets`, `current_liabilities`, `retained_earnings`, `long_term_debt` (ESEF + structured-xHTML mapped); derived `working_capital`, `current_ratio`. The Piotroski F5 leverage input is **total non-current liabilities** (`total_liabilities − current_liabilities`, both full-coverage facts — ADR 0083 D4 amendment); `long_term_debt` remains extracted for future use but is no longer a score input.

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

This is a hybrid model governed by [ADR 0022](adr/0022-research-evidence-read-model-boundary.md):

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
- Review checkpoints support "last reviewed" and "changed since review" read models, one checkpoint per scope (`company` or `watchlist`).
- Timeline summary counts and changed-only filtering are derived read-model behavior (no stored timeline projection — see above).
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

- `statement_type`: `industrial` (default) | `bank` | `insurer` | `specialty_finance` | `reit` — selects which canonical packs apply, and gates the health scores: any non-`industrial` value makes Altman Z″ / Piotroski F `NotApplicable` ([ADR 0083](adr/0083-company-health-scores-and-red-flags.md) D4). Registry-sector backfill is forward-only, idempotent, and rewrites only rows still at the `industrial` default (a manual value is authoritative): `0095` maps banks/insurers/exchanges-and-brokers; `0098` maps debt collectors (`Wierzytelności` → `specialty_finance`, owner decision 2026-07-18). Investment holdings (`Działalność Inwestycyjna`) stay unmapped pending an owner decision.
- `reporting_standard`: `ifrs` (default) | `us_gaap` | `local`.
- `fiscal_year_end_month`: integer 1–12 (default 12) for non-calendar fiscal years.

`financial_periods`:

- `id`, `company_id` → `companies(id)`, `fiscal_year`, `period_type` (fiscal label: `FY`, `H1`, `H2`, `Q1`–`Q4`, `9M`, `M01`–`M12`), `period_end_date`, `report_evidence_ref` (soft reference to a report document/feed item; FK tightened in a later milestone).
- Unique on `(company_id, fiscal_year, period_type)`.
- `period_type` is the canonical fiscal label; the legacy out-of-spec `annual` is the FY alias. `create_financial_period` folds `annual` → `FY` at the write boundary (guardrail, card f64cea2), and migration `0066_period_type_annual_to_fy.sql` (forward, idempotent, self-healing) merges any stored `annual` row into its `(company, fiscal_year, FY)` sibling — repointing `financial_facts` (dropping the annual-side duplicate on an `idx_financial_facts_slot` collision so the canonical FY value wins) plus `report_documents`/`framework_evaluations`/`management_claims`, then deleting it; a lone `annual` with no FY sibling is relabeled in place.

`kpi_definitions` (catalog — what a metric *is*):

- `scope`: `canonical` (app-owned, global) | `sector` (shared within a `sector`) | `company` (bespoke, set `company_id`) | `user` (user-owned, global custom metric — added in `v0.44.0` for quality frameworks, [ADR 0046](adr/0046-quality-frameworks-quantitative.md)).
- `metric_key`, `label`, `value_kind` (`monetary` | `percentage` | `ratio` | `count` | `physical` | `duration`), `unit` (typed: `PLN`/`EUR`/`t`/`m2`/`shares`/`per_share`/`years`/…), `computation` (`reported` | `derived`), `formula` (for derived, over other metric keys), `display_format`.
- Unique on `(metric_key, scope, IFNULL(company_id,''), IFNULL(sector,''))`.
- Seeded packs: universal, industrial, cash flow, capital efficiency (derived), and sector packs `insurance`, `banking`, `specialty_finance`, `reit`.

`kpi_relevance` (selection over time — which KPIs matter for a company):

- `company_id`, `definition_id`, `status` (`active` | `archived`), `source` (`user` | `agent` | `sector` | `core`), `rank` (`primary` | `secondary`), `first_seen_period`, `last_seen_period`. Unique on `(company_id, definition_id)`.
- **Core seed** (`source = 'core'`, migration `0106_seed_core_kpi_relevance.sql`, owner decision 2026-07-21): the table held **zero rows** in production, so [ADR 0061](adr/0061-deterministic-fundamentals-data-gathering.md) decision 2(d)'s completeness check never fired — `expected_primary_metric_keys` returned `None` and extraction recall had no denominator. The migration seeds five `active`/`primary` rows per company — `revenue`, `operating_profit`, `net_profit`, `total_assets`, `total_equity`, defensible for any IFRS reporter — as a **starting** denominator while the durable per-sector/per-company selection is studied (card `3569d99`). Rules: seeds only what is **absent** (`NOT EXISTS` + `INSERT OR IGNORE`), so a curated row is never overwritten, re-ranked or duplicated; deterministic ids (`kpirel_core_<company>_<metric>`) so re-applying converges; a missing canonical definition seeds nothing rather than failing. It seeds the companies that exist when it applies — a company added later has no seeded denominator until the durable selection lands.

`financial_facts` (values — reference a definition, never the relevance profile):

- `value_numeric`: decimal-exact text in base units, signed (parsed with `rust_decimal`); `as_reported_value`/`as_reported_scale` keep the source form (e.g. "245 253 tys. zł").
- `annotation` (migration `0117`, #156): nullable, user-authored one-off note ("includes discontinued operations…") rendered as a visible `*` marker next to the figure — the value itself stays exactly as reported. Never written by any extraction path. Update semantics: field absent keeps the stored note, empty string clears it, text replaces it (pinned by the dual-execution corpus). **Display sign convention** (decided once in the format layer per ADR 0076, never per panel): metric keys stored positive by catalog convention but conventionally read as cash outflows (`capex` — the catalog's `fcf = ocf - capex`) render with a leading minus in the numeric fallback path only; the as-reported figure is never rewritten (`OUTFLOW_DISPLAY_METRIC_KEYS`, `src/shared/format/financialValue.ts`).
- Dimensions: `currency`, `statement_basis` (`consolidated`/`standalone`), `attribution` (`total`/`owners_of_parent`/`nci`), `variant` (`reported`/`adjusted`/`constant_currency`/`continuing`/`discontinued`/`net_of_cancellations`/`lifo_ccs`), `measure_window` (`flow`/`point_in_time`/`trailing`/`cumulative`/`duration`), `data_quality` (`final`/`estimated`), `reporting_standard` (override).
- Provenance: `extraction_method` (`manual`/`ai_extracted`/`api`/`derived`/`html_positional`), `confidence`, `confirmation_state`, `supersedes_id` (final supersedes estimate, history kept), `source_document_ref`. **`confirmation_state` is a frozen compatibility column** ([ADR 0086](adr/0086-aggregator-primary-fundamentals.md) decision 5, amending [ADR 0055](adr/0055-autonomous-report-pipeline-trust-ladder.md)): facts are **review-free** — every automatic writer stamps `confirmed`, and there is no `pending`/`auto_unreviewed` awaiting-confirmation state anymore. Origin truth (issuer vs third-party, proven vs merely-uncontradicted, reversible) lives in `source_tier` + `extraction_method` + `validation_status` + citation, surfaced as labels, never as a confirmation to-do. The `pending`/`auto_unreviewed` literals survive only on historical rows written before the amendment (readers must keep tolerating them; the one-off data rebuild, ADR 0086 dec. 6, unifies them) and in the CHECK constraint's allowed set. Editing stays an option: manual add/edit/delete (and later MCP write-tools) let the user override any value. `html_positional` marks the **tier-3b pdf2htmlEX positional** sub-tier ([ADR 0077](adr/0077-trusted-extraction-foundations.md) T-B2), which persists under `source_tier='pdf'`; the marker is a sub-tier label only, never trust-bearing (`source_tier` + `validation_status` remain the trust signals).
- Unique on `(period_id, definition_id, statement_basis, attribution, variant, measure_window, data_quality)` so estimate and final coexist.
- **Re-observation policy** (T7-F, amended by [ADR 0086](adr/0086-aggregator-primary-fundamentals.md) decision 3): structured-extraction writes resolve the uniqueness slot before inserting. Same stored value → skip (a re-observation, reported as skipped, not produced) — **unless the incoming tier strictly outranks the stored fact's provenance tier**, in which case the slot's label/evidence is **upgraded** to the incoming tier (`StructuredFactCommit::Upgraded`, the value unchanged). Different value → between peers, or against a manual/no-provenance fact, the stored fact is kept (no silent overwrite) and the divergence is surfaced; **a strictly higher tier overwrites the lower tier's slot** (value + provenance — the issuer's number wins its own slot, e.g. an ESEF parse landing after the daily BiznesRadar pull already filled the period). The precedence lives in `record_structured_fact` (`storage/kpi_extraction.rs`), shared by every structured writer. The same ranking governs the **comparative cross-check's prior**: an extraction's prior-period baseline reads only **veto-capable** stored facts (`stored_fact_set_for_cross_check` — manual/no-provenance and tiers the incoming tier does not outrank), so a lower-tier (aggregator-sourced) prior can never fail an issuer filing's comparative check and discard the set (ADR 0086 dec. 4 — the issuer witnesses the aggregator, never the reverse; live regression 2026-07-22, CBF). An ESEF set a veto-capable prior *does* contradict is a **flagged** outcome carrying the failing checks — never a silent empty. A bare `INSERT` into `financial_facts` from an extraction path is a defect.
- **Confirm is slot-aware too** (ADR 0077): confirming a KPI proposal resolves the same slot before writing. An identical fact already in the slot → re-observed (the proposal links to the existing fact id, no duplicate row, its original provenance is left untouched) while still validating and confirming any pending OCR bootstrap profile. A *different* value → a typed `value_conflict` error carrying both values (never a raw sqlite `UNIQUE` error, never an overwrite); the proposal stays pending and the user decides.

Rules:

- Derived metrics (margins, FCF, ROE/ROIC, net-debt/EBITDA) are computed at read time from confirmed facts (TTM where conventional); unavailable when an input is missing. The `formula` is an expression over other metric keys, evaluated by the shared derived-metrics service (`v0.44.0`, [ADR 0046](adr/0046-quality-frameworks-quantitative.md)) using the same expression engine that evaluates quality-framework criteria. The service computes across **all** scopes, so adding a `user`-scope definition extends the computed-metrics list with no code change.
- A formula's intermediates resolve **only** through the catalog (a reported fact or another seeded derived row) — there are no hidden built-in synthetic-key resolvers; the engine's only built-ins are the `_ttm`/`_avg` suffixes and `ttm`/`avg`/`cagr`/`trend` window functions. Capital-efficiency conventions (migration `0050`, fixing issue `674cb5a`): `invested_capital = total_equity + net_debt` and `capital_employed = total_equity + net_debt + cash` are seeded derived rows, and **ROIC is pre-tax** — `roic = operating_profit / invested_capital` — since no income-tax fact is extracted (NOPAT/effective-tax-rate is deliberately not guessed). `roce = operating_profit / capital_employed`.
- A fact may exist for a KPI not yet active in the relevance profile (agent-extracted, awaiting curation).
- Industry-specific classified stocks (reserves with proven/probable, 1P/2P/3P categories) are modeled as company-scoped custom KPIs, not core enums.
- Import/export, retention, and backup must treat fundamentals as owner durable state.

Structured-first extraction provenance ([ADR 0061](adr/0061-deterministic-fundamentals-data-gathering.md), migration `0057_fundamentals_provenance.sql`; provenance is a separate concern joined by fact id — `financial_facts` is not altered):

`financial_fact_provenance` (one row per fact the structured pipeline produced):

- `fact_id` (PK → `financial_facts.id`), `source_tier` (`esef` | `structured_xhtml` | `espi_cover_note` | `pdf` | `html_aggregator` | `ai_text` | `ai`), `validation_status` (`passed` | `witness_confirmed` | `unreviewed` | `flagged`; the legacy `none` is **retired from all production writes** — ADR 0077 §4 / G-1 — but readers must keep tolerating it on historical rows written before the retirement), `drift_json` (serialized `DriftReport` label diff when a drift accompanied the fact), `citation` (source concept/label the value was read from).
- `pdf` also carries the **tier-3b positional** sub-tier ([ADR 0077](adr/0077-trusted-extraction-foundations.md) T-B2): a pdf2htmlEX visual-render XHTML (no `<table>`, no `ix:` tags — e.g. CD PROJEKT interims) that no ESEF tier can read, reconstructed from CSS glyph geometry. It reuses `source_tier='pdf'` (a deterministic visual-PDF reconstruction, no new trust-order variant) and is distinguished only by `financial_facts.extraction_method='html_positional'`. It clears the same `validate` gate as every tier; `validation_status` is never `none`.
- `espi_cover_note`: **tier 2a** ([ADR 0061](adr/0061-deterministic-fundamentals-data-gathering.md) decision 1) — the mandated ESPI "WYBRANE DANE FINANSOWE" cover table read from the **plain-text body of a periodic-report komunikat**, with `financial_facts.extraction_method='espi_cover_note'`. Unlike every other tier it is **not** triggered by a stored report document: extraction runs **at ingest time**, as a post-commit step of `ingest_bankier_company_items` (`storage/espi_cover_note_facts.rs`), because feed retention can delete the carrier body (a measured prune removed 448/451 WDF bodies) — a body not parsed at ingest is lost. Consequences of that seam:
  - `citation` names the **feed item** (`… | feed_item:<id>`), not a document concept/label, so the evidence survives the carrier being pruned; `financial_facts.source_document_ref` likewise holds the feed-item id.
  - The **period** is derived from the komunikat title/URL with the same `period_from_title_url` derivation the document pipeline uses; an underivable period **abstains** (nothing persisted) — never guessed. This tier has a feed item, not a stored file, so it stops at the title/URL grammar — the document pipeline's cover-page fallback below does not apply to it.
  - Facts pass the **same** `validate_parsed_set` gate as every other tier; monetary rows the PLN↔EUR footnote cross-check cannot resolve **abstain** rather than emit.
  - `confirmation_state` is `confirmed` like every other tier — facts are review-free ([ADR 0086](adr/0086-aggregator-primary-fundamentals.md) decision 5). The cover-note figures being issuer-untagged (lower proof) is recorded as provenance (`source_tier='espi_cover_note'`, `validation_status`, citation), not as a review state.
  - **Tier precedence on an occupied slot** (ADR 0061 decision 1 / ADR 0086 decision 3): enforced by the shared `record_structured_fact` precedence (Re-observation policy above) — a stored fact from `esef`/`structured_xhtml`, or from any tier with no/unknown provenance (manual entry), is left untouched; a stored `pdf`/`html_aggregator` fact is **upgraded** in place (value + provenance rewritten to this tier, with the per-metric upgrade evidence recorded as a `tier_upgrade` diagnostic).
  - Outcomes (`emitted` / `empty` with the `WdfEmptyReason` / `flagged` / `no_period` / `error`, each with counts incl. `abstained`) are recorded as `diagnostic_events` rows under `module='espi_cover_note'` plus a structured `module=espi_cover_note stage=…` log line — abstentions and empty reasons are never silent.
- `html_aggregator` as **PRIMARY** ([ADR 0086](adr/0086-aggregator-primary-fundamentals.md) decisions 2–4; write path `record_aggregator_fact`, `storage/kpi_extraction.rs`): the BiznesRadar-primary pull (`jobs/aggregator_fundamentals_pull.rs`) parses **every period column** of the three cached report pages (period derived from the column header — a quarter marker or the annual "(mon YY)" fiscal-year-end month, so shifted fiscal years land in their true period; an unparseable header is skipped, never guessed) and writes `source_tier='html_aggregator'`, `extraction_method='api'`, `citation` = page URL + row label. Slot precedence: an empty slot is written; the aggregator's **own** slot (provenance `html_aggregator`, non-manual) is overwritten in place with a fresh value; a `manual` or higher-tier slot is **never touched** — a divergence against a held slot records an informational `witness_disagreement` extraction outcome (reversed witnessing, decision 4 amended 2026-07-22) when that slot is EITHER an issuer tier (`esef`/`structured_xhtml`/`espi_cover_note`/`pdf` — every `SourceTier::is_issuer` tier, i.e. all but `html_aggregator`; both the pull and `aggregator_owns_slot` parse the enum, never string-match) OR a `manual` slot (the user's own entry — decision 3 "logged, never applied" made concrete, tier `manual`, plus a structured log line). The outcome's `detail_json` is the canonical gate shape (below), so it renders as investor language. An empty/zero aggregator cell is never written at all (the zero rule, ADR 0085 amendment). Monetary values scale by the page's own declared unit; per-share rows are exempt.
- **Unit-scale detection is dominance-weighted** (cards e6ebda3 + 610bcae): the document-wide monetary scale (`w tys. zł` → ×1000, `w mln zł` → ×1e6) is set by the statement's own *declarations* (`w` + a scale word: `w tys.`/`w tysiącach` for thousands, `w mln`/`w milionach` for millions), which real filings repeat across headers, column captions and footnotes. `detect_unit_scale` **counts** the declarations on each side and lets the majority win, so neither a stray narrative `w mln` inside a thousands statement (card 610bcae) nor a stray prose `mln zł` (card e6ebda3) can flip the whole document ×1000. A genuine millions filing ("dane w mln zł", no thousands declaration) still resolves to millions — it carries the only declaration. A **bare** `mln zł` token (no `w`) is prose and breaks a declaration tie only; it never overrides a thousands declaration. History: CD PROJEKT Q3 2023 (declared thousands, two prose "mln zł") over-scaled ×1000 → migration `0099` repaired the two persisted facts; the earlier first-match rule then regressed on documents carrying a couple of stray `w mln` mentions amid many thousands declarations (card 610bcae) → replaced by the dominance count.
- `ai`: the AI-assisted fact sources — (a) the KPI-proposal confirm path (`confirm_kpi_proposal`/`auto_confirm_kpi_proposal`, both manual-confirm and autopilot auto-confirm), and (b) the **tier-4 OCR path** ([ADR 0077](adr/0077-trusted-extraction-foundations.md) §4 dec. 2), where a confirmed OCR profile parses the Mistral-OCR markdown to a validated set — the `citation` is the normalized OCR label the value was read from. Both write `source_tier='ai'` (the OCR-vs-text-LLM mechanism lives in the citation, not the tier). Every such fact carries a provenance row whose `validation_status` is the **real** deterministic verdict over the period's fact set: `passed` (an identity was checked and held), `flagged` (an identity was violated — e.g. a repainted/mis-scaled total), or `unreviewed` (nothing checkable in the period). `drift_json` = `NULL`. The fact persists regardless; the status records only what validation saw. A regression guard (`no_production_path_writes_validation_status_none`) reddens if any production path writes `none` again.
- Reads tolerate a missing row: facts predating the pipeline are treated as unknown tier / unvalidated.

`company_extraction_profile` (the versioned PDF extraction layout per company, ADR 0061 decision 3) — **table kept append-only, its read/write code retired** ([ADR 0086](adr/0086-aggregator-primary-fundamentals.md) decision 1): the PDF fact-extraction arm's bootstrap/merge/drift machinery (`get_profile`/`upsert_profile`) is deleted, so no production path reads or writes this table anymore; existing rows are inert history.

- `company_id` (PK), `template_hash`, `unit_scale` (`Ones` | `Thousands` | `Millions`), `profile_json` (serialized `ExtractionProfile` label map), `version`. Historical only — a row here no longer affects extraction.

`fundamentals_extraction_outcomes` (migration `0103_fundamentals_extraction_outcomes.sql`, [ADR 0061](adr/0061-deterministic-fundamentals-data-gathering.md) decision 2 + [ADR 0084](adr/0084-retire-in-app-ai-layer.md) decision 4/6) — **the persistence half of "never silently wrong"**: one row per attempted extraction *slot*, including the attempts that emitted nothing.

- Why a separate table and not more `financial_fact_provenance` columns: provenance is keyed by `fact_id`, so it structurally **cannot** record a run that produced no fact — which is exactly the Flagged / Empty / unreadable case the user most needs to see. `diagnostic_events` is not a substitute either: it is developer-mode gated and trimmed to 7 days / 1000 rows, so it is observability, not a record.
- Columns: `id` (PK, deterministic `fxo_<sha256(slot)[..32]>`), `company_id` (FK → `companies`, `ON DELETE CASCADE`), `report_document_id`, `fiscal_year`, `period_type`, `period_end`, `tier`, `acceptance`, `reason_code`, `detail_json`, `drift_json`, `structure_changed`, `fact_count`, `attempt_count`, `first_attempted_at`, `last_attempted_at`.
- **Slot uniqueness** on `(company_id, report_document_id, fiscal_year, period_type, period_end)`: a re-run **updates in place** (incrementing `attempt_count`), never appends. So the table always reports the pipeline's *current* verdict — a flag whose cause has been fixed disappears instead of lingering next to a fresh success — and history is deliberately not kept here (the run chronicle is the log/diagnostics, not this record).
- `report_document_id` is intentionally **not** an FK: a document can be re-captured or pruned, and losing it must never delete the evidence that the pipeline tried and what it concluded. The **ingest-time cover-note witness seam** ([ADR 0085](adr/0085-biznesradar-fundamentals-witness.md), `v0.59.0`) is a second writer of these rows: it keys the slot by the **feed item** its facts cite (not a document), recording `accepted_via_witness`/`emitted` on agreement or `flagged`/`witness_disagreement` on disagreement, `tier = espi_cover_note`. A cache miss writes **no row** (the comparison is deferred — a `witness_pending` diagnostic instead), so absence at that slot still means "no verdict yet", never a false agreement.
- `acceptance` (CHECK): `accepted` | `accepted_via_witness` | `accepted_unreviewed` | `flagged` | `empty` — the same vocabulary as `Acceptance::as_str()`.
- `reason_code` (CHECK, widened by migrations `0105_witness_fallback_reason.sql` and `0119_extraction_outcomes_recount_and_superseded.sql`) is **typed, never prose** (ADR 0084 decision 6 — the frontend renders it through the translation layer): `emitted` (the non-failure value) | `validation_failed` | `structure_drift` | `witness_disagreement` | `witness_fallback` | `no_deterministic_tier` | `no_period_derived` | `document_unreadable` | `facts_superseded`. **`facts_superseded`** (issue #243, epic #40 S5 residue) marks a recorded emission whose facts are no longer at the slot (removed with the retired PDF arm or superseded by a better document) — written **only** by repair migration 0119, never by a live run; 0119 also recounts `fact_count` from the facts actually at the slot for the pre-S5 zero-effect rows, so an emitting row can again evidence its claim. The report-documents indicator does not render `has_data` for it. **`witness_fallback`** is **legacy only** ([ADR 0086](adr/0086-aggregator-primary-fundamentals.md) retired the ADR 0085 aggregator gap-fill seam): no run writes it anymore — BiznesRadar sources core KPIs through its own primary pull. Stored fallback rows remain readable and **re-armable** (see [autopilot re-arm](#history-sweeps)) so a period left on third-party numbers is re-extracted with issuer data once readable. `witness_disagreement` is now written only by the reversed-witnessing paths (the BR-primary pull and the WDF ingest seam), never by the extraction pipeline. SQLite cannot alter a CHECK in place, so 0105 is a table rebuild, skipped when the constraint already admits the code. `Flagged` is deliberately split across three of these — "the numbers contradict", "the layout moved" and "the witness disagrees" are three different problems with three different fixes, and one `flagged` label would hide that.
- **History-plausibility quarantine** (`fundamentals::validation::implausible_against_history`, wired into `jobs::structured_extraction`, cards 22ac70c / v0.59): a uniform-scale outlier no same-period identity or comparative column can see — a value ≥100× off the metric's **own stored history** (median magnitude of that company+metric across the OTHER stored periods; the current period is excluded, and CONFIRMED values are **included** as the trust anchor) — is **quarantined per fact**: it is **not persisted**, while the set's plausible siblings still emit with their own `validation_status`. Any quarantine downgrades the **set-level** outcome to `flagged` with `reason_code='validation_failed'`, so the period surfaces in `list_flagged_extraction_outcomes`. The gate **abstains** with fewer than two history periods (no stable median), for split-sensitive metrics (`eps_basic`/`eps_diluted`/`dividend_per_share`/`shares_outstanding`), and when either side is zero; it only ever *withholds* a fact, never modifies anything stored. Migration 0108 runs before extraction, so on the maintainer's machine the medians this reads are already scale-cleaned.
- `detail_json` carries **only the failing** checks (`{ failedIdentities, failedCrossChecks, witnessDisagreements }`, each with `expected`/`actual`/`residual`); a `NotApplicable` check is not a contradiction and is never listed, so a detail payload always means "here is what objected". `NULL` when nothing failed. A **`quarantinedFacts`** payload (`[{ metricKey, value, historyMedian }]`) lists the facts the history-plausibility gate held back — folded onto any identity/cross-check failures the same set carried. `witnessFallback` payloads appear on **legacy rows only** (the ADR 0085 pipeline witness seam is retired by [ADR 0086](adr/0086-aggregator-primary-fundamentals.md)). Fresh `witnessDisagreements` outcomes are written by the BR-primary pull and the WDF ingest seam in the **same** canonical gate shape (`{ failedIdentities:[], failedCrossChecks:[], witnessDisagreements:[{ metricKey, detail:{ expected, actual, residual } }] }`, convention `expected` = aggregator, `actual` = the held issuer/manual value) — the shared shape the Coverage panel renders as investor language, so no raw JSON key reaches the UI. The BR-primary pull nests its extra context (`pageUrl`/`sourceAdapterId`/`issuerTier`) INSIDE that `detail` object, where the panel reads only `expected`/`actual`/`residual` and silently ignores the rest. A **`document_unreadable`** row from the magic-byte router (card `eb71488`) carries `{ detectedContainer, reason }` — the container the bytes actually were (`zip`/`xml`/`html`/`unknown`) under a `.pdf` name — so "this `.pdf` is really a `<zip>`" is reviewable, not a mute failure. A **`witnessCorroboration`** payload (`{ metricKeys, count, pageUrl }`) records the agreement half at the cover-note seam.
- `drift_json` persists the serialized `DriftReport` **even on a non-emitting run** — the pipeline already computes it, and before this table it was dropped on exactly the runs where it mattered. This is where the ADR 0061 decision-3 learning loop reads drift back.
- **Rows are written for emitting runs too.** That is what makes absence meaningful: **no row = never attempted**, so a flagged period can never be confused with an untouched one. The read model (`list_flagged_extraction_outcomes`) narrows to `acceptance IN ('flagged','empty')` — a clean period needs no review.
- **`no_period_derived` sentinel**: a document whose reporting period cannot be derived has no `(year, type, end)` to key its slot by — that absence *is* the failure — so it is recorded with `fiscal_year = 0`, `period_type = ''`, `period_end = ''` and keyed by document alone. Readers must treat an empty `period_type`/`period_end` as "period unknown", **never** as a real period.
- Recording is **best-effort**: the extraction result is the more important guarantee, so a bookkeeping failure is logged (`module=structured_extraction stage=outcome_record_failed`) and never propagated. Every outcome also emits an always-on structured log line (`module=structured_extraction stage=outcome …`), mirroring the ingest-time cover-note tier.
- Reads tolerate a missing row (a period predating this table reads as never attempted).

`fundamentals_witness_pages` (migration `0104_fundamentals_witness.sql`, [ADR 0085](adr/0085-biznesradar-fundamentals-witness.md) decision 3) — the **politeness cache** that makes "one witness page fetch per tracked company per day" real.

- Why it must exist: the extraction pipeline runs per *report document*, so a company with four stored periodic filings would otherwise fetch the same aggregator page four times in one autopilot pass, and again on every manual re-extract.
- Columns: `company_id` (PK, FK → `companies`, `ON DELETE CASCADE`), `page_url`, `html`, `status`, `fetched_at`. One row per company, **upserted in place** — a cache, not history.
- `status` (CHECK): `ok` | `no_coverage` | `fetch_failed`. `html` is stored **only** for `ok`; a degraded attempt caches the *decision*, never a body we do not have. Failures and no-coverage landings are cached too, so a dead slug or a flaky host costs one request per day rather than one per document.
- Reads are window-scoped (`fetched_at` within 24h); an unrecognized `status` marker reads as "nothing cached" rather than being guessed into a verdict — a cache row we cannot interpret must never become a witness opinion.
- Freshness of the adapter itself is on the `source_adapters` row (`biznesradar-fundamenty`, `last_success_at`), written via `record_source_outcome_for_adapter` on every refresh (DoD §C).

`fundamentals_aggregator_pages` (migration `0110_aggregator_fundamentals_pages.sql`, [ADR 0086](adr/0086-aggregator-primary-fundamentals.md) decision 2) — the **per-(company, page kind) politeness cache** behind the BiznesRadar-primary pull; widens the 0104 single-page cache to the three report pages the primary role fetches.

- Columns: `company_id` (FK → `companies`, `ON DELETE CASCADE`), `page_kind` (CHECK: `income` | `balance` | `cashflow`, mapping to the `raporty-finansowe-rachunek-zyskow-i-strat` / `-bilans` / `-przeplywy-pieniezne` paths), `page_url`, `html`, `status`, `fetched_at`; **PK `(company_id, page_kind)`**, upserted in place — a cache, not history. Same `status`/`html` rules and 24h-window reads as `fundamentals_witness_pages` above.
- The legacy `fundamentals_witness_pages` table is **kept** (append-only rule — a migration never deletes user data); its rows are copied here once as the `income` kind (`INSERT OR IGNORE`, idempotent) and the storage layer's reads/writes move here (`get_fresh_kind`/`put_kind`).
- The migration also seeds the `inventories` canonical KPI definition (`kpidef_inventories`), so the balance-sheet `Zapasy` row resolves to a catalog metric.

`company_ocr_extraction_profile` — **dropped** by migration `0102_clean_cut_ai_artifacts.sql` ([ADR 0084](adr/0084-retire-in-app-ai-layer.md) decision 5, clean cut). Not to be confused with `company_extraction_profile` above (ADR 0061 decision 3's deterministic PDF layouts): that table is **kept append-only, its read/write code retired** ([ADR 0086](adr/0086-aggregator-primary-fundamentals.md) decision 1) — a schema-only survival, not an active tier.

`kpi_extraction_jobs.committed_fact_count` (`INTEGER NOT NULL DEFAULT 0`, migration `0064_kpi_extraction_committed_facts.sql`): how many validated facts the run committed directly (tier-4 profile path, [ADR 0077](adr/0077-trusted-extraction-foundations.md) §4 / T4.5). `0` for the classic proposals-only path, so the review panel reads an honest outcome ("N facts committed" vs "N proposals"). Reads tolerate the default on old rows.

`document_derived_periods` (migration `0109_document_derived_periods.sql`) — a **persisted derived index** for one document's reporting period, so the Coverage panel and re-extraction runs read the period instead of recomputing it (Model Principles; CLAUDE.md "read persisted derived indexes instead of recomputing the corpus per call").

- Why it must exist: `derive_report_period` (`jobs::structured_extraction`) resolves a stored document's `(fiscal_year, period_type, period_end)`, and its last-resort tier reads a bare-titled periodic PDF's **cover page**, which costs a full text extraction. `compute_fundamentals_coverage` calls it once per document on **every** panel load, so a company with a few bare-`SSF.pdf` filings re-extracted those PDFs on every open. A document is immutable once ingested, so its derived period is a stable function of its bytes — derive once, persist, read thereafter.
- Columns: `report_document_id` (PK, FK → `report_documents`, `ON DELETE CASCADE` — pure re-derivable cache, so it is swept with its document, unlike `fundamentals_extraction_outcomes` which must outlive its document), `has_period`, `fiscal_year`, `period_type`, `period_end`, `derivation_version`, `derived_at`. A CHECK ties `has_period` to the period columns (all three non-NULL for a hit, all NULL for a marker).
- **The abstention is cached too**: `has_period = 0` is an explicit none-marker for a document whose content yields no derivable period, so the next load does not re-extract a document that will once again yield nothing.
- **Invalidation = `derivation_version`**, the code constant `jobs::structured_extraction::DERIVATION_VERSION`. Since a document's bytes never change, the only reason to re-derive is a change to the derivation grammar itself — bump the constant, and any row stamped with an **older** version is ignored, re-derived and overwritten on next read (forward-only, self-healing). Rows at the current version (or newer) are served without touching the file.
- **Write guard**: only a **fetched** document with a stored file is cached — a not-yet-ingested document's transient `None` is never written, so it is not poisoned into never being re-derived once fetched. Writes are best-effort (a cache-write failure never fails the derivation); reads tolerate a missing row (a period predating this table is simply re-derived and cached).

### Market Data (Daily Quotes)

EOD price series ([ADR 0067](adr/0067-market-data-foundation.md), source selection [ADR 0082](adr/0082-market-data-source-selection.md), `v0.53.0`). Feeds market cap + level-0 ratios (canonical derived metrics) and the workspace price section.

`daily_quotes` — `(company_id, date, open, high, low, close, volume, source_adapter_id, fetched_at)`, PK `(company_id, date)`.

Rules:

- **Append-only**; corrections upsert by `(company_id, date)` — never a wholesale history rewrite. Full available history is backfilled from day one (52-week stats, own-history percentiles, future backtests).
- `source_adapter_id` records provenance (`yahoo-eod`; historical rows may carry a removed provider id — provenance is never rewritten); `fetched_at` drives staleness attribution.
- The PK serves the 52-week / percentile range scans (indexed reads, never a corpus recompute).
- Migrations are append-only (0071); reads of quote-derived ratios tolerate missing facts (ratio resolves to `null`, never a crash).

### FX Rates (NBP)

NBP Table-A mid rates — the FX substrate powering PLN conversion for cross-company comparison ([ADR 0089](adr/0089-cross-company-comparison-and-valuation-l1.md) decision 2, `v0.61.0`). Migration `0115_fx_rates.sql`.

`fx_rates` — `(currency, date, mid_rate, source_adapter_id, fetched_at)`, PK `(currency, date)`. NBP Table-A daily average ("mid") rates (keyless official public API; `mid_rate` is decimal-exact TEXT, never a float — parsed from the raw JSON number token). Append-only; corrections upsert by key like `daily_quotes`, never a wholesale rewrite. Fed by the internal `nbp-fx` source adapter (developer-visibility; never swept by the source scheduler) via the `fx_daily_pull` durable-queue job on the market-data (`sources`) lane — a full-history backfill the first time the daily job sees a needed currency with no stored rows (daily cadence, no synchronous first-need hook — ADR 0089 amendment 2026-07-29; chunked in ≤90-day windows for NBP's 93-day range limit; a 404 window is a non-publication day, not an error), then a recent-window daily pull. Needed currencies are table-driven (defaults EUR/USD/GBP/CHF ∪ any already stored ∪ any requested), never a hardcoded enum.

Feeds the comparison read model's PLN conversion: **flow** KPIs (`measure_window = flow`) at the period-average mid over the period's dates, **stock** KPIs at the last mid ≤ period end; a PLN amount passes through unconverted (basis `native_pln`); ratios/percentages are never converted. A missing rate is a typed per-cell flag, never a silent PLN guess.

### Valuation Runs

Comparative valuation L1 history ([ADR 0089](adr/0089-cross-company-comparison-and-valuation-l1.md) decisions 4–5, `v0.61.0`). Migration `0116_valuation_runs.sql`.

`valuation_runs` — append-only valuation history (L1 now, v0.62 DCF later): `id`, `company_id`, `method` (`pe_multiple | ev_ebitda_multiple | pbv_multiple`; DCF adds new method values, not columns), `inputs_json`, `fair_low` / `fair_base` / `fair_high` (per share, decimal-exact TEXT), `data_as_of`, `confidence_grade` (`A`–`D`), `created_at`.

- **Append-only, immutable once applied** (0116). A run appends only when the input **signature** (`inputs_json`, the canonical serialization of the method's driver + peer-multiple dispersion + `data_as_of`) differs from that `(company_id, method)`'s latest stored run — never a row per render.
- **Newest-run selection orders by the domain `data_as_of` date, never `created_at`** (a late backfill can carry an older domain date than a wall-clock-later insert). `created_at` only tie-breaks within an as-of date. Indexed `(company_id, data_as_of DESC, created_at DESC)` for the `list_valuation_runs` read and `(company_id, method, …)` for the signature lookup.
- The compute-and-persist path is the `compute_comparative_valuation` command (contracts.md); `fair_*` are NULL only on a method's typed absence (an absence never persists a row).

### Attention Routing (Alert Rules + Events)

User-owned alert rules and the attention events their evaluation emits ([ADR 0068](adr/0068-attention-routing-and-morning-briefing.md), `v0.54.0`). Migration `0077_alert_rules_attention_events.sql`.

`alert_rules` — `(id, trigger_type, signal_category, price_min, price_max, scope_type, scope_ref, enabled, created_at, updated_at)`.

- `trigger_type` ∈ `signal_category` | `autopilot_run_completed` | `price_enters_range` | `price_week52_low` (CHECK). Trigger-specific columns stay NULL for triggers that do not use them: `signal_category` (a `signal_categories.key`) for `signal_category`; `price_min`/`price_max` (inclusive close band, `min ≤ max`) for `price_enters_range`.
- `scope_type` ∈ `company` | `watchlist` (CHECK); `scope_ref` is a `company_id` (company scope) or `watchlist_id` (watchlist scope). A watchlist-scoped rule fires for every member company.
- `enabled = 0` rules never fire.
- **Rule ids are content-derived** (`alert_<trigger>_<scope>…`, count-suffixed on base collision — the repo's collision-safe id convention), never row-count-based (a deleted rule must not free an id a survivor still holds — live regression 2026-07-15). Creating a rule identical to an existing one (same trigger + scope + prices) is rejected with the typed `DuplicateAlertRule` error, never inserted as a twin.

`attention_events` — `(id, rule_id, trigger_type, company_id, evidence_type, evidence_ref, fired_at, seen, dismissed, created_at, evidence_title)`, `UNIQUE(rule_id, evidence_type, evidence_ref)`. Migration `0081_espi_witness_reconciliation.sql` made `rule_id` **nullable** and added `trigger_type` to support **system** events (no owning rule); migration `0114_attention_events_evidence_title.sql` appended `evidence_title`; migration `0118_attention_events_nullable_company.sql` **rebuilt the table** to make `company_id` nullable ([ADR 0091](adr/0091-failure-path-and-real-state-testing.md) decision 2).

- `rule_id` is `NULL` for a **system** event; then `trigger_type` on the row carries the trigger directly. The writer **stamps `trigger_type` on every event** (rule-backed too — v0.57 fix wave 2 / W4; migration `0097_backfill_attention_trigger_type.sql` backfills legacy NULL rows from the owning rule) so a direct read / grouping does not depend on a join; the read model still `COALESCE`s over the rule for defense in depth. System triggers: `source_reconciliation` (an `espi_only` reconciliation result — [ADR 0069](adr/0069-source-reliability-and-disclosure-signals.md) D2) and `job_failed` (a background job whose retries are exhausted — [ADR 0091](adr/0091-failure-path-and-real-state-testing.md) decision 1); neither is user-creatable.
- `company_id` is `NULL` **only** for a system event with no company scope — a workspace-wide background job (morning briefing, history sweep, aggregator pull) that failed terminally. Rebuilt nullable by `0118`; every other event still carries its company, and the read models/UI treat the NULL scope explicitly (the Today stream groups such rows under a system scope, never a blank ticker).
- `evidence_type` is `company_signal` (ref = signal id) | `autopilot_run` (ref = run id) | `daily_quote` (ref = quote `date`) | `source_reconciliation` (ref = `source_reconciliation_results.id`) | `job` (ref = `job_queue.id`). Every event links back to the evidence that raised it.
- **Dedup**: rule events — at most one per `(rule, evidence_type, evidence_ref)` (`ON CONFLICT DO NOTHING`). System events (`rule_id IS NULL`) — a partial `UNIQUE(trigger_type, evidence_type, evidence_ref) WHERE rule_id IS NULL` index dedups them (the reconciliation-record id and the job id are stable `evidence_ref`s, so a re-run / a reclaimed job never re-fires). System events carry **no** per-rule daily throttle.
- **`job_failed` read model** ([ADR 0091](adr/0091-failure-path-and-real-state-testing.md) decision 1): a guarded `LEFT JOIN job_queue` on the event's `evidence_ref` supplies the failed job's raw `kind` as `evidence_detail` (the frontend translates it) and its `last_error` as the `evidence_title` fallback when the handler snapshotted no subject — so the row always states WHICH task failed and on WHAT, never a bare category. Severity is **Notable for every job kind** (owner decision).
- **Freshness gate** (v0.57 fix wave 2, [ADR 0068](adr/0068-attention-routing-and-morning-briefing.md) amendment): the **historical-ingest seam** (`classify_and_store_signal`) does not evaluate alert rules for a filing whose **domain** date is older than **14 days** (`SIGNAL_FRESHNESS_DAYS`) relative to wall-clock now — a backfill re-ingesting years of filings stores the signals but raises no wall of stale alerts. A signal with no/unparseable domain date is treated as fresh (never suppress what we cannot prove is old). The gate is on the ingest seam, **not** inside `evaluate_signal_rules`, so present-detection paths (derived red flags, KNF short-position changes) still alert on a current condition even when its underlying report period is old.
- **Per-rule daily throttle**: at most **1 event per rule per WALL-CLOCK day**. `fired_at` is the **wall-clock firing time** (v0.57 fix wave 2 — previously the evidence's domain date, which let distinct historical dates each "count as a different day" and bypass the throttle during a backfill; the evidence's own date lives on the linked signal/quote/run). Migration `0096_dismiss_stale_attention_events.sql` dismisses the pre-existing unseen backlog (`fired_at` > 30 days old).
- Evaluation is **inline** in the evidence-producing job stages (signal classification, autopilot `finalize_notify`, post-daily-pull price check) — no new worker lane; a handful of indexed reads. `price_week52_low` fires only on a **strict new low** vs the trailing 52-week window (by `date`); `enters_range` fires when the latest close is inside `[min, max]`.
- Rows CASCADE-delete with their rule and company; reads of an absent event tolerate it (nothing to show).
- **`evidence_title` — durable fire-time title snapshot** (`v0.60` D7, migration `0114`): the event's concrete "what happened" title, **snapshotted when the event fires** and preferred over the read-time evidence join. It exists because a `company_signal` title is otherwise fatal to feed pruning — `company_signals.feed_item_id` is `ON DELETE CASCADE`, so pruning a feed item cascade-deletes the signal row and the read-time join to `feed_items.title` returns nothing (the row degrades to a bare category). The write path snapshots the title for `company_signal` (its feed_item title, in scope at fire) and `source_reconciliation` (the witness title, in scope at the insert site); `autopilot_run` keeps the join (its `report_documents` row outlives feed pruning) and price events have no title. The read prefers `NULLIF(evidence_title,'')`, else the live join — so legacy rows still resolve. **Never backfilled**: rows orphaned before the column existed (their source data already pruned) keep rendering the generic fallback until aged/dismissed — an accepted, irrecoverable limitation, not a bug.
- **Severity is derived, never a column** ([ADR 0087](adr/0087-today-attention-home-v2.md) decision 2, `v0.60`): the `AttentionEvent`/`AutopilotRun` read models carry a typed `severity` (`urgent` | `notable` | `routine`) **computed at read** by the single backend mapping (`storage::severity`) from `trigger_type` + the signal category (joined from the event's `company_signal` evidence) or the run `status`. There is no `severity` column and none should be added — it is a projection, and the authoritative level → trigger/category table lives in [Product Spec § Attention Routing](product-spec.md#severity-taxonomy).

### Morning Briefing

A daily/on-demand briefing ([ADR 0068](adr/0068-attention-routing-and-morning-briefing.md) decision 4, `v0.54.0`): a deterministically composed item list plus an optional AI narrative. Migration `0078_morning_briefings.sql`.

`morning_briefings` — `(id, composed_at, since, narrative_markdown, narrative_provider_id, narrative_model, language, created_at)`.

- `since` is the domain-date lower bound the composer used (`YYYY-MM-DD`; `''` on the first-ever briefing = include everything). The next compose's `since` = the latest briefing's `composed_at` date.
- `narrative_markdown` is `NULL` when no provider is configured **or** the narrative was rejected on citation integrity — the briefing still persists as the structured item list. A narrative is **decision-support only** (facts + citations, never advice), phrased via the research-digest provider contract (capability `morning_briefing`, [ADR 0060](adr/0060-ai-capability-routing-and-openai-compatible-provider.md)).

`morning_briefing_items` — `(id, briefing_id, position, item_type, company_id, domain_date, citation_key, evidence_type, evidence_ref, title, detail, created_at)`, `UNIQUE(briefing_id, citation_key)`. This table is BOTH the structured list the Today card renders AND the citeable evidence set the narrative resolves against (mirrors `ai_research_digest_citations`).

- `item_type` ∈ `signal` | `autopilot_run` | `claim_due` | `report_date` | `attention_event`. `evidence_ref` is the source id (signal id | run id | claim id | attention-event id | `company:event_key`); a narrative citation resolves against `(evidence_type, evidence_ref)` via `research::supplied_evidence_refs` — an unresolved citation rejects the whole narrative (never store an uncited narrative).
- **`title`/`detail` carry typed payloads (since `v0.60`, [ADR 0087](adr/0087-today-attention-home-v2.md) dec. 4; columns unchanged).** The composer writes only verbatim source data or typed codes/tokens (category code, `trigger_type`/`evidence_type` codes, a `report_processed`/token summary, `status`, `due:<period>:<year>`, qualified ticker) — never composed prose. The Today card translates via `briefingItemText.ts`. Legacy rows composed before `v0.60` keep prose (no migration); the read path is unchanged and tolerant. Composition also **dedups** to at most one item per `(company_id, item_type, evidence_ref)`, keeping the newest by `domain_date` (dec. 1).
- **Composition** is a pure, deterministic function over gathered reads (`storage::compose_briefing`), so it is snapshot-stable. Inputs: new **confirmed** signals (domain date = `signal_date`), completed autopilot runs (`succeeded`/`partial`, domain date = `updated_at`), and fired non-dismissed attention events (domain date = `fired_at`) whose date is **strictly after** `since`; plus the current claims-due (due + overdue) and upcoming-report snapshot (not `since`-bounded). Items are ordered by `domain_date`, then a stable (type, company, evidence-ref) tiebreak — **never `created_at`**; `citation_key` = `b{position+1}`.
- `company_id` is denormalized TEXT (not an FK): a briefing is a historical snapshot, so deleting a company must not rewrite past briefings. Items CASCADE-delete with their briefing.
- The compose runs as the `morning_briefing` durable-queue job (**autopilot** lane since the AI lane was retired, [ADR 0084](adr/0084-retire-in-app-ai-layer.md); the briefing is a purely deterministic composition — the AI narrative half is gone). On-demand (`generate_morning_briefing`, `force`) recomposes even if today's briefing exists; the daily auto-trigger (Rust scheduler, app-open only) is idempotent per day (`briefing_exists_on`).

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

### Decision Journal Entries

The user's own recorded judgments ([ADR 0071](adr/0071-judgment-capture.md), `v0.52.0`) — the early, forward-compatible slice of the ADR 0043 thesis-workbench journal (the `v0.64` workbench extends this table, never migrates away). Decision support only: the app records and mirrors judgments, it never grades them (ADR 0042). Migration `0067_decision_entries.sql`.

`decision_entries`:

- `id`, `company_id` → `companies(id)` (CASCADE).
- `kind`: `buy` | `pass` | `keep_watching` | `sell_note` (SQL CHECK) — recorded actions/judgments, never advice.
- `rationale_md`: Markdown rationale (required).
- `decided_at`: domain date (`YYYY-MM-DD`) the decision was made — the journal's chronology; lists order by `decided_at DESC, id DESC`, never by `created_at`.
- `superseded_by_entry_id` → `decision_entries(id)` self-reference, nullable. Set on a **follow-up** entry: the id of the entry superseded **by** this one. The link lives on the new row pointing back, so appending a correction never updates a prior row.
- `created_at`.

Rules:

- **Immutable once saved**, enforced structurally: `BEFORE UPDATE` and `BEFORE DELETE` triggers `RAISE(ABORT)`; there is no update/delete API. Corrections are appended as follow-up entries.
- The delete trigger carves out exactly one path — the FK cascade when the owning company is deleted (its `WHEN` clause only passes when the parent `companies` row is already gone).
- Evidence links attach through the generic `evidence_links` boundary (`decision_entry` joins the research timeline in `v0.52.0` J2); no link columns live here.
- Owner-durable and retention-exempt; unified-bundle export is a `v0.67` dependency.

### Report Expectations

What the user expected **before** a report landed ([ADR 0071](adr/0071-judgment-capture.md), `v0.52.0`), so hindsight bias has a check. Keyed by the same stable occurrence key as `report_preparations` and resolved at creation to the fiscal period the report covers. Expectation-vs-actual is a read model (`v0.52.0` J2) — no stored projection; the user records their own verdict, the app never scores judgment. Migration `0068_report_expectations.sql`.

`report_expectations`:

- `id`, `company_id` → `companies(id)` (CASCADE).
- `event_key`: the report occurrence's stable `company_events.source_event_key` (same semantics as `report_preparations.event_key`). Unique on `(company_id, event_key)`.
- `fiscal_year`, `period_type` (`financial_periods` vocabulary; `annual` folds to `FY` at the write boundary): the occurrence's resolved period — what the freeze rule joins against the facts coverage.
- `stance_md`: free-text Markdown stance (required).
- `frozen_at`: stamped **once**, on first observation that the resolved period has facts; nullable until then.
- `resolution_note_md`, `resolved_at`: the user's own verdict at review time (recordable after the freeze; `resolved_at` set on first recording).
- `created_at`, `updated_at`.

`report_expectation_metrics` (optional per-metric expectations, replaced wholesale on update):

- `id`, `expectation_id` → `report_expectations(id)` (CASCADE).
- `metric_key`, `comparator` (`lt` | `lte` | `eq` | `gte` | `gt`, SQL CHECK), `expected_value` (decimal-exact text in base units, matching `financial_facts.value_numeric`), `unit` (nullable), `created_at`.

Rules:

- **Freeze**: every update runs in a transaction that first checks whether the resolved `(fiscal_year, period_type)` has facts via the `facts_coverage_by_period` read model; once facts exist the update is refused with the typed `ReportExpectationFrozen` error (command code `conflict`) and `frozen_at` is stamped. Reads also stamp it (freeze-on-read), idempotently.
- Metric outcomes (`met`/`missed`/`unknown`) are computed by a pure, total evaluator (`evaluate_metric_expectation`); unparseable values or unknown comparators yield `unknown`, never a guess.
- **Review read model** (`expectation_review`, `v0.52.0` J2): composed on read, never stored. `facts_available` is true once any facts exist for the resolved period; each metric's `actual_value` is the latest **confirmed** `financial_facts` row for that metric+period (joined via `kpi_definitions.metric_key`, exact `period_type` match), else null → `unknown`. The read stamps `frozen_at` like `list` does.
- Owner-durable and retention-exempt; unified-bundle export is a `v0.67` dependency.

### Quality Frameworks

User-owned quality checklists evaluated against the fundamentals facts by a deterministic rule engine, producing a versioned scorecard ([ADR 0046](adr/0046-quality-frameworks-quantitative.md), `v0.44.0`). A *framework* is a named set of criteria expressed in a free-text DSL over metric keys; the engine evaluates each criterion against confirmed `financial_facts` (latest period/TTM) and records the measured value. The quantitative engine uses no AI; decision-support only (criteria cannot encode buy/sell output). Migration `0048_quality_frameworks.sql`. A framework may also hold **qualitative**, agent-assessed criteria (`v0.50.0`, [ADR 0075](adr/0075-qualitative-assessment-frameworks.md)) — see the `kind`/`assessment_guidance` and agent-assessed `criterion_results` fields below.

`quality_frameworks` (the checklist):

- `id`, `name`, `description`.
- `origin`: `app_template` (ships with the app) | `user` (user-created or cloned) — a **provenance label, not an edit lock**: every framework is editable and deletable in place regardless of origin.
- `template_key`: stable key for an app template (e.g. `kroeze_quality`), null for user frameworks; `cloned_from` → `quality_frameworks(id)` (set when cloned).
- `version`: integer, bumped on edit; pinned into each evaluation.
- `created_at`, `updated_at`.

`framework_criteria` (a single check):

- `id`, `framework_id` → `quality_frameworks(id)`, `ordinal` (display order).
- `label`, `expression` (DSL text, e.g. `roic >= 15%`, `net_debt_to_ebitda < 2.5 AND fcf > 0`), `expression_ast` (cached parsed AST JSON).
- `weight`/`rank` (optional, for scorecard emphasis), `partial_band` (optional near-threshold band that yields a `partial` verdict).
- `kind` (`quantitative` | `qualitative`) + `assessment_guidance` — `v0.50.0` ([ADR 0075](adr/0075-qualitative-assessment-frameworks.md)): a qualitative criterion is agent-assessed and carries an owner-authored `assessment_guidance` prompt seed instead of a DSL expression (empty `expression`, which stays `NOT NULL`); an append-only migration (`0059_qualitative_criteria.sql`) backfills existing rows to `kind = quantitative` and a missing/NULL `kind` reads as `quantitative`.
- `created_at`, `updated_at`.

`framework_evaluations` (one immutable run):

- `id`, `framework_id` → `quality_frameworks(id)`, `framework_version` (pinned at run time), `company_id` → `companies(id)`, `period_id` → `financial_periods(id)` (the assessed period).
- Summary counts: `pass_count`, `partial_count`, `fail_count`, `unavailable_count`; `engine_version`.
- `created_at`. Immutable once written.

`criterion_results` (one immutable per-criterion outcome):

- `id`, `evaluation_id` → `framework_evaluations(id)`, `criterion_id` → `framework_criteria(id)`.
- `expression` (snapshot of the criterion text at run time), `verdict` (`pass` | `partial` | `fail` | `unavailable`).
- `measured_value` (decimal-exact text, the leading metric's measured value), `measured_unit`, `threshold` (snapshot), `inputs_json` (the facts/periods/metric keys used, for audit), `note`.
- Agent-assessed fields — `v0.50.0` ([ADR 0075](adr/0075-qualitative-assessment-frameworks.md), append-only columns in `0059_qualitative_criteria.sql`): `reasoning` (short), `citations` (JSON — typed evidence refs reusing the `ai_research_brief_citations` model: `evidence_type` from `ResearchEvidenceType`, `evidence_id`, `label`, `snippet`), `confidence` (`low` | `medium` | `high`), `prompt_version`, `source`. `source` follows the append-only safe-default pattern: added with `DEFAULT 'engine'` so the existing-table `ALTER … ADD COLUMN` succeeds and pre-migration quantitative rows read as engine-sourced; qualitative rows write `agent`. For qualitative rows `verdict` adds `insufficient_evidence` to the quantitative set. Agent results are opinion, never facts: visually distinct, regeneratable, and they never mutate quantitative data. The Quality panel's current-state read resolves, per criterion, the most-recent `source = agent` row across all snapshots — indexed by `idx_criterion_results_agent(criterion_id, source)` (`0060_qualitative_assessment_index.sql`).
- Immutable once written.

Rules:

- Quantitative evaluation is a manual user action (`evaluate_framework`) over the latest available period/TTM. Qualitative assessment (`v0.50.0`, ADR 0075) is agent-run per criterion (durable `qualitative_assessment` job) and re-enqueued by the assist/autopilot trust rungs on new-report arrival; agent results compose into the same immutable snapshot with a per-criterion `source`.
- A run computes an in-memory metric table (never persisted) via the shared derived-metrics service, then persists only the `framework_evaluations` + `criterion_results` rows. The `measured_value` is pinned to the run and does **not** change when underlying facts later change (a final supersedes an estimate) — the scorecard is the latest run; history is queryable.
- `verdict = unavailable` when a referenced metric cannot be computed (missing fact), distinct from `fail`.
- A quantitative run (`evaluate_framework`) **skips `kind = qualitative` criteria** entirely — they carry no DSL expression and are agent-assessed on a separate snapshot, so a quant run never writes a phantom `unavailable` row for them (ADR 0075, §T6; the quantitative engine's semantics are otherwise untouched).
- **Verdict-change detection** (`qualitative_verdict_changes`) compares, per qualitative criterion, the two most-recent `source = agent` verdicts across snapshots; a differing pair is a change (a first/only assessment is not). Changes surface in the **company research digest** as synthetic `framework_verdict` evidence items, **bounded by the company review checkpoint** (the same `reviewed_at` gate real timeline evidence uses — a change not newer than the checkpoint is not re-emitted, so marking the company reviewed suppresses it; ADR 0075 Decision 5, §T6). The Kroeze app template (`v0.50.0`) ships six qualitative criteria (moat, pricing power, recurring revenue, capital-allocation quality, understandable business, founder/insider ownership) alongside its quantitative checks.
- The framework-criteria **import path applies the same `kind`/`assessment_guidance` validation as the create path** (`normalize_kind` + guidance required for qualitative): an unrecognized `kind` or a guidance-less qualitative criterion fails the import with a typed error — never stored verbatim.
- An evaluation run may be **deleted** from the history (pruning); deletion cascades to its `criterion_results`. Deletion removes a whole run — it never mutates a retained run's snapshotted values, so the immutability guarantee holds for what remains.
- App-template seeds are **bilingual** ([ADR 0076](adr/0076-ui-design-system-and-density-contracts.md) Decision 8): the Rust template constant carries `{pl, en}` for every human-facing string (framework name/description, criterion label, `assessment_guidance`); seed, reset, and top-up resolve the language from the persisted `locale` setting once (fallback `pl`). Criterion keys — `expression`, `partial_band`, `kind` — stay locale-independent.
- App-template updates never overwrite an edited framework. Seeding is idempotent and runs at startup: a template with no framework yet is inserted with localized name/description + all criteria; an **untouched** template framework (`origin = app_template`, `version == 1` — every edit bumps the version) is **topped up** — any template criteria it lacks are added additively, matched by the criterion's stable **index in the constant** (written as `ordinal`), never by label. Top-up never modifies or deletes existing rows, never bumps the version (so later template growth keeps topping up), and is idempotent across restarts; it closes the "new template criteria invisible without a destructive reset" gap for installs predating the criteria. The same untouched framework is also **re-localized** on startup (baba638): its name/description and each criterion's label/`assessment_guidance` are rewritten to the current locale — but only for a field whose stored value still exactly matches the shipped template text in some locale (`IN (pl, en)`), so a framework seeded before the bilingual pass ([ADR 0076](adr/0076-ui-design-system-and-density-contracts.md) Decision 8) stops showing stale-locale strings after a locale switch. Re-localization matches criteria by `ordinal`, never modifies a user-authored value (the field-level match plus the `version == 1` gate both exclude edits), never bumps the version, and is idempotent. Edited (`version > 1`) or user-created frameworks are left untouched. An `app_template`-origin framework also offers an explicit **Reset to template defaults** (`reset_framework_to_template`) that re-derives its criteria (in the current locale) from the shipped Rust template constant — the single source for seed, reset, and top-up.
- Frameworks + criteria (and any `user`-scope `kpi_definitions` a criterion references) are owner durable state, carried in the import/export bundle so an exported framework imports cleanly. Evaluations are reproducible snapshots; their export is optional.
- The migration is append-only, idempotent, and self-healing; adding the `user` value to the `kpi_definitions.scope` CHECK is handled by a guarded table rebuild if the constraint is restrictive.

### Research Cockpit Layouts

Named, user-saved layouts for the research cockpit — the dockview docking shell ([ADR 0053](adr/0053-dockview-layout-pilot.md)). A *layout* is the user's saved panel arrangement (which panels are open + their split/tab geometry + the linked selection), so they can switch task-shaped workspaces ("Earnings season", "Daily triage", "Deep dive"). Owner durable state. Migration `0054_cockpit_layouts.sql` (append-only, idempotent, self-healing). Decision 3A in [ADR 0053](adr/0053-dockview-layout-pilot.md).

`cockpit_layouts`:

- `id`, `name` (user-facing label, unique), `ordinal` (display order).
- `panels_json`: the cockpit's own descriptor of which panels are open and the linked selection — the app-owned, stable part that survives a dockview upgrade. A company-scoped panel carries a `mode`: `follow` (tracks the view company; no `companyId` stored — resolved at render time) or `pinned` (freezes a specific `companyId`, e.g. `fundamentals:company_gpw_cdr`). The descriptor also carries `viewCompanyId` — the view company follow panels resolve to (U-Ra, [ADR 0076](adr/0076-ui-design-system-and-density-contracts.md)). Parsing is tolerant: a legacy entry without `mode` is read as `pinned`, and a missing `viewCompanyId` defaults to null.
- `layout_json`: the serialized **dockview geometry** (`api.toJSON()`) — splits, tab groups, sizes. Opaque to us; restored via `api.fromJSON()`.
- `dockview_version`: the dockview version that produced `layout_json`, so a future format change can be migrated or safely discarded.
- `created_at`, `updated_at`.

Rules:

- **Versioned restore with safe fallback.** On load, if `dockview_version` is incompatible or `fromJSON` throws, the geometry is discarded and the layout is rebuilt from `panels_json` in the default arrangement — a layout never crashes the shell. A panel in `panels_json` referencing a removed company/screen is dropped on restore (tolerate-missing, per the standing migration-resilience rule).
- The geometry (`layout_json`) is a derived convenience; `panels_json` is the source of truth for *what* is open. A layout with only `panels_json` (no geometry) is valid.
- Layouts are owner durable state and are carried in the import/export bundle ([ADR 0018](adr/0018-import-export-boundaries.md), `v0.52.0` per-feature coverage); `dockview_version` travels with the layout so an imported layout restores or falls back correctly.
- Spike note: the proof-of-concept persisted to `localStorage`; the production cockpit uses this table (decision 3A). Do not ship `localStorage` layout persistence.

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

### Durable Job Queue

The `job_queue` table (migration `0051`, Architecture v2 / [ADR 0050](adr/0050-architecture-v2-domain-stores-source-pipeline-durable-jobs.md)) is the persisted work list that replaces fire-and-forget `spawn_blocking` tasks, so work survives a crash mid-job and can be retried/resumed. Distinct from the `jobs` activity log above: `jobs` records what *ran*; `job_queue` holds what *must run*. Driven by `storage::JobQueueStore` + the in-process `jobs::queue::JobWorker`.

Fields:

- `id` — primary key and idempotency key; re-enqueuing the same id is a no-op (dedup).
- `kind` — handler discriminator (a registered `JobHandler` claims this kind).
- `payload` — opaque JSON the handler deserializes.
- `status` — `pending` | `running` | `succeeded` | `failed`.
- `attempts` / `max_attempts` — attempts is incremented at claim time, so a crash mid-run still counts and cannot loop forever.
- `available_at` — earliest claimable time; pushed out by exponential backoff on retry.
- `locked_at` — when last claimed into `running`; used to reclaim crash residue.
- `last_error`, `created_at`, `updated_at`.

Rules:

- **Claim is atomic.** The next runnable row (oldest `pending` with `available_at` passed) is moved to `running` and its `attempts` incremented in a single `UPDATE … RETURNING`, so two workers can never claim the same row. `claim_next_for_kinds(kinds)` scopes the claim to a lane's kinds ([ADR 0059](adr/0059-worker-pools-and-queue-fairness.md)).
- **Retry with backoff.** A failed run with attempts left returns to `pending` with `available_at` pushed out (capped exponential); once `attempts == max_attempts` it becomes terminally `failed`.
- **A terminal failure is user-visible.** When `mark_failed` turns a row terminally `failed`, the queue's single terminal point classifies the kind through `jobs::failure_surface` and — for kinds with no richer domain surface — raises a system `job_failed` attention event whose `evidence_ref` is this row ([ADR 0091](adr/0091-failure-path-and-real-state-testing.md) decision 1). Kinds whose failure already reaches the Sources adapter health row or the autopilot run card keep that surface exclusively; a transient failure with retries left raises nothing.
- **Crash resume + poison guard.** On startup the worker requeues every `running` row back to `pending` — **except** a row whose `attempts >= max_attempts`, which is **dead-lettered** (`failed`) rather than resurrected. A job that hangs (never reaching the retry path) would otherwise be reclaimed and re-run every restart, permanently starving the queue ([ADR 0059](adr/0059-worker-pools-and-queue-fairness.md); the bankier refresh with `attempts=15 > max=2`).
- **Isolated worker lanes.** The worker runs as named pools by category (`sources` / `autopilot`), each with its own threads claiming only its kinds, so a slow source refresh cannot starve latency-sensitive autopilot. The **`ai` lane and the per-AI-provider concurrency semaphore are removed** with the in-app AI analysis layer ([ADR 0084](adr/0084-retire-in-app-ai-layer.md), amending ADR 0059): every kind it drained was retired, and the now-deterministic `morning_briefing` job moved to the autopilot lane. Per-source serialization (exactly one refresh per source at a time) is enforced as a lock shared across lanes; worker counts are settings. See [ADR 0059](adr/0059-worker-pools-and-queue-fairness.md).
- **Per-source serialization + defer.** A worker holds an exclusive in-memory lock for the job's source (its `adapterId`, exactly one at a time) across the run; a worker that cannot acquire it **defers** the job — requeued to `pending` after a short backoff **without** consuming an attempt (distinct from a retry-on-failure, so contention never exhausts `max_attempts`). ([ADR 0059](adr/0059-worker-pools-and-queue-fairness.md).)
- **Chunked company-scoped refresh.** A company-scoped scheduled refresh (bankier-company) is a **planner**: it enqueues one idempotent `source_company_refresh` job (stable id `source_company_refresh:{adapter}:{company}`, re-armed via `reschedule`) **per tracked company** instead of looping all companies in one job. The per-source lock serializes them (politeness preserved), other lanes run alongside, unfinished per-company jobs resume across restarts, and each job rides autopilot detection on its own ingest completion. This retires the monolith that monopolized the single worker for minutes. ([ADR 0059](adr/0059-worker-pools-and-queue-fairness.md).)
- **`enqueue` vs `reschedule` for a reused id (guardrail, bug `dce9ce8`).** `enqueue` is `INSERT OR IGNORE` — safe only when `id` is genuinely fresh. Any producer that may enqueue again under the **same** `id` after that row already reached a terminal state (a recreated run, a `retry_*` command, a re-triggered one-shot job) must use `reschedule`, not `enqueue`: otherwise the later call is a silent no-op against the existing terminal row and the work never runs again. This broke the autopilot pipeline (a recreated `autopilot_run` stuck at `pending`/`fetch` forever behind an already-`succeeded` stage job — `create_run_if_absent` → `enqueue_stage`) and, mechanically identically, `jobs::handlers::enqueue_per_job` (backs `retry_kpi_extraction` / `retry_claim_extraction` / `retry_ai_analysis`, whose per-job handlers always terminal-succeed the queue row regardless of domain outcome).
- Local-first: workers drain the queue only while the app is open. Append-only, idempotent, self-healing migration (`CREATE TABLE IF NOT EXISTS`).

### Autopilot Settings and Runs

Supports the autonomous report pipeline (North Star, `v0.49.0`, [ADR 0055](adr/0055-autonomous-report-pipeline-trust-ladder.md)): per-company opt-in automation and a persisted record of each autonomous run. The confirm-before-commit guarantee is unchanged globally; these tables only scope automation to companies the user opts in.

`company_autopilot_settings` (per-company trust-ladder mode):

- `company_id` — primary key, → `companies(id)`.
- `mode` — `off` (default) | `assist` | `autopilot`. Mode semantics (what each level automates) are canonical in [Contracts § Autonomous Report Pipeline](contracts.md#autonomous-report-pipeline-autopilot).
- `updated_at`.

Rules:

- A company with no row is treated as `off` (reads tolerate a missing row — a safe default, per the migration-safety rule).
- The mode is a single per-company control, not per-step toggles. Changing the mode never touches already-produced facts or runs.

`autopilot_run` (one row per autonomous run; the durable record behind the single notification, the review queue, and run-level undo):

- `id` — primary key and run id stamped on each stage job.
- `company_id` → `companies(id)`; `report_document_id` → `report_documents(id)` (the detected report).
- `trigger` — `detection` (refresh-completion hook) | `manual` (user-initiated re-run) | `history_sweep` (enqueued by a history sweep, [History Sweeps](#history-sweeps) / [ADR 0077](adr/0077-trusted-extraction-foundations.md) §3; widened by migration `0062`). The `extract` stage is now **deterministic-only** for every trigger ([ADR 0084](adr/0084-retire-in-app-ai-layer.md) decision 4 — tier-4 OCR is retired): when no deterministic tier emits, the run records `{ extractionAvailable: false, reason: "no_deterministic_tier" }` (`reason: "witness_fallback"` appears on **legacy** deltas only — the ADR 0085 aggregator gap-fill is retired by [ADR 0086](adr/0086-aggregator-primary-fundamentals.md); stored fallback deltas stay honest gaps and re-armable) and is **flagged** with its completion notification — never a silent skip, never a guess. The AI-era budget gate no longer runs.
- `sweep_id` — the `history_sweeps` row that enqueued this run (`NULL` for detection/manual runs and legacy rows; migration `0065`). The former tier-4 budget charge is gone (ADR 0084); the column stays for history.
- `mode` — the company autopilot mode captured at run time (`assist` | `autopilot`).
- `status` — `pending` | `running` | `succeeded` | `failed` | `partial`.
- `stage` — current/last stage reached: `fetch` | `extract` | `diff` | `cross_reference` | `notify`.
- `summary_text`, `kpi_delta_json`, `report_diff_ref`, `cross_refs_json` — the composed result (what changed + cross-references to claims/questions/evidence).
- `produced_fact_ids_json` — the `financial_facts` ids this run created, so a single "undo this run" reverts exactly those facts.
- `notification_state` — `unread` | `read` | `dismissed` — drives the Today/Pulse "what changed" surface ([ADR 0054](adr/0054-mode-based-thesis-centric-shell.md)).
- `last_error`, `created_at`, `updated_at`.

Rules:

- **Detection is idempotent.** A run is created at most once per `(company_id, report_document_id)`; re-ingesting the same document does not re-fire.
- **Stages are chained durable-queue jobs** ([Durable Job Queue](#durable-job-queue)) stamped with this `id`: `fetch → extract → diff → cross_reference → notify`. A crash mid-stage resumes that stage only; each stage reuses the existing service (fetch, KPI extraction, diff, cross-reference), never a reimplementation.
- **Reversibility** reuses the existing fact supersede/reject mechanics; the run row only adds the grouping (`produced_fact_ids_json`) needed to undo a whole run at once. Facts are review-free ([ADR 0086](adr/0086-aggregator-primary-fundamentals.md) decision 5): auto-committed facts land `confirmation_state = confirmed` in every mode — reversibility is by Undo/edit, not a pending-confirmation queue. Origin (cited, third-party vs issuer, reversible) is carried by the provenance labels, not by a review state.
- A `failed`/`partial` run still produces a notification describing how far it got (no silent dead-end).

### History Sweeps

The `history_sweeps` table (migration `0062`, [ADR 0077](adr/0077-trusted-extraction-foundations.md) §3) is the durable record of a history sweep — the backfill/manual counterpart to the refresh-time detection sweep. A sweep enqueues a full `autopilot_run` (`trigger='history_sweep'`) for every **canonical periodic report whose period still lacks accepted facts**, through the shared `enqueue_extraction_run` (dedup + re-arm identical to detection). Driven by `storage::HistorySweepStore` + the `history_sweep` durable-queue job (autopilot lane).

Fields:

- `id` — primary key (`history_sweep:{company}:{nanos}`, collision-checked).
- `company_id` — → the company being swept.
- `trigger` — `backfill` (chained from `run_backfill`) | `manual` ("Extract missing periods").
- `status` — `queued` (default) | `running` | `completed` | `failed`.
- `candidates_total` — periods that needed extracting when the sweep ran.
- `runs_enqueued` — runs freshly created or re-armed (`Created` | `Rearmed`); `Rearmed` now covers both a non-terminal run's stage re-arm and a **terminal-run re-arm on capability upgrade** (see the re-arm rule below).
- `skipped_existing` — candidates whose extraction run was already terminal **and not re-armable** (`DedupedTerminal`): a run that emitted facts, or a couldn't-extract run whose document is still not extractable.
- `runs_failed` — candidates a storage error prevented enqueuing (`Failed`); a sweep with `runs_failed > 0` still `completed`s (the count records the partial failure, never swallows it).
- `skipped_reason` — why the sweep enqueued nothing, when it did: `automation_off` for a company in mode `off` ([ADR 0077](adr/0077-trusted-extraction-foundations.md) §3 amendment (c) — never a silent skip).
- `enqueued_run_ids_json` — the `autopilot_run` ids this sweep enqueued (JSON array), so progress derives per-run status without a parallel query.
- `ai_calls_used` / `ai_call_limit` — **dropped** by migration `0102_clean_cut_ai_artifacts.sql`. These were the tier-4 AI budget accounting (migration `0065`); tier-4 is retired ([ADR 0084](adr/0084-retire-in-app-ai-layer.md) decision 4) and the clean cut leaves no AI residue on surviving tables. The sweep **rows** and every non-AI field (status, candidate/enqueue counters, trigger, timestamps) are untouched.
- `error` — a storage-level abort that failed the whole sweep.
- `created_at`, `updated_at`.

Rules:

- **Trust ladder.** A sweep (chained or manual) runs only for mode ∈ {`assist`, `autopilot`}; mode `off` ends it with `status='completed'` + `skipped_reason='automation_off'` and zero enqueues.
- **Candidate document = the period's best EXTRACTABLE document, not blindly the coverage canonical** ([ADR 0077](adr/0077-trusted-extraction-foundations.md) §3, T-A2): when the canonical is a non-iXBRL XHTML (a pdf2htmlEX render, extractable by no deterministic tier and ineligible for the PDF-only tier-4), the sweep attacks the period's next-best fetched periodic document that IS extractable (a PDF, or an iXBRL `.xhtml`/`.xbri`/`.zip`), preferring ssf over jsf then the newest; with no extractable sibling the canonical is still enqueued (the run degrades `not_pdf`), never a silent drop. Sweep-layer only — the canonical selection (ADR 0061 dec. 1b) is untouched. A report document's reporting period is self-derived from its iXBRL contexts when it is a valid instance, else from its title/URL (`period_sort_key`); a non-iXBRL XHTML now uses that title/URL fallback like a PDF.
- **AI budget is consumed atomically** (ADR 0077 §6): a single guarded `UPDATE ... SET ai_calls_used = ai_calls_used + 1 WHERE id = ?1 AND (ai_call_limit = 0 OR ai_calls_used < ai_call_limit)` is the whole check-and-consume (`changes() == 1` ⇔ granted), so concurrent extract stages can never over-spend the last unit. Deterministic outcomes never consume — only a run actually entering tier-4 spends a unit; a denied run records `skipped_budget` on its `kpi_delta_json`, never a silent skip. **Statically-ineligible runs never consume either** (ADR 0077 §6 refinement, 2026-07-10): a missing `VisionExtraction` provider, no stored file, or the ESEF/iXBRL route (tier-4 is PDF-only) is decided by a pre-check *before* the guarded UPDATE, so the run degrades honestly (`no_vision_provider` / `no_stored_file` / `not_pdf`) and spends zero units — a unit buys a reachable provider invocation, not a configuration/format check. The computed **coverage read model** (`get_fundamentals_coverage`, [contracts.md](contracts.md)) projects this onto its period: `skippedBudget` is `true` when a period's canonical report's `history_sweep` run carries that reason (the run id is per-`(company, document)` deterministic, so it is a single lookup, not a windowing query).
- **Terminal-run re-arm on capability upgrade** ([ADR 0077](adr/0077-trusted-extraction-foundations.md) §3, 2026-07-10; version gate 2026-07-21). The shared `enqueue_extraction_run` dedup no longer skips a terminal run forever. A terminal **succeeded** run whose `kpi_delta_json` says `extractionAvailable:false` with a re-arm-class reason is **re-armed** (`autopilot().rearm_run`: reset to `pending`/`fetch`, re-stamped with the current `trigger` + `sweep_id`, so the re-run re-enters `stage_extract` and charges the *current* sweep's budget) iff **(a) the pipeline capability version advanced since the run recorded its verdict AND (b) the document is now extractable** — otherwise a period a prior pipeline version couldn't read stays permanently blind to every later version (live: the tier-3b positional tier made CDR interim XHTMLs newly extractable, yet the sweep skipped them). **Version gate** (the storm fix, owner dogfooding 2026-07-21): the false-path delta stamps `pipelineVersion` = `EXTRACTION_PIPELINE_VERSION` (a `u32` const in `jobs::structured_extraction`, bumped only when a tier/parser/derivation change alters what documents can be read — a JSON field, no schema migration; a missing field reads as version 0, the pre-versioning era, eligible for one re-arm). Re-arm requires `stored < EXTRACTION_PIPELINE_VERSION`; because `document_is_extractable` is constant-true for any well-formed PDF, without this gate every sweep pass re-armed every flagged period forever (attempt_count reached ~1100+ in a day, re-running identical file IO + PDF parse). After a re-run records its stamped delta the period settles until the next bump. Classes: `not_extractable` / `not_pdf` / `no_stored_file` / `witness_fallback` re-arm iff `document_is_extractable` is now true (a still-dead unreadable/zero-byte file stays deduped) — `witness_fallback` (a **legacy** class: the ADR 0085 gap-fill is retired by ADR 0086, but stored fallback deltas still re-arm so a period left on third-party numbers is re-extracted with issuer data once readable); `pdf_document` (the BY-DESIGN raw-PDF gap, ADR 0086 dec. 1) **never re-arms** — no capability upgrade makes a PDF machine-readable; `skipped_budget` re-arms so a fresh sweep's fresh budget retries it (the sweep's own `ai_call_limit` caps re-invocations, G-4); `no_vision_provider` re-arms only once a `VisionExtraction` provider is configured. A run that **emitted** facts, or any `partial`/`failed` run, is never re-armed.
- **Backfill chaining.** `run_backfill` chains a `backfill` sweep at its successful end — best-effort (a chaining failure is logged, never fails the backfill).
- **Automatic backfill catch-up** (v0.57 fix wave 2, [ADR 0077](adr/0077-trusted-extraction-foundations.md) amendment — trigger parity). The report-history backfill is no longer manual-only: `enqueue_company_backfill_catch_up` enqueues a durable `company_backfill` job (sources lane, serialized on the Bankier-company source lock) for every **automated** company (`company_autopilot_settings.mode != 'off'`) that has **no fetched periodic report** (`companies_lacking_periodic_coverage`: no `report_documents` row with `fetch_status='fetched'` AND `doc_kind ∈ {periodic_ssf, periodic_jsf}`). Wired at **app startup** AND **after every successful source refresh** (mirroring the ownership / management-holdings catch-up parity). Idempotent two ways: the coverage predicate stops selecting a company once it has history, and a **stable per-company job id** (INSERT-OR-IGNORE) means a queued/running/completed backfill — including a genuinely empty issuer — is attempted **once**, never re-fetched every refresh. A `mode='off'` company (or one with no autopilot row) is skipped with an explicit logged `automation_off` reason, never a silent drop.
- **Backfill truncation honesty.** The in-memory `BackfillProgress` (not persisted; ADR 0036) carries `truncated: bool` — `true` when the page cap (`MAX_BACKFILL_PAGES`) ended the fetch before the configured `backfill_years` cutoff was reached, so older filings may be missing. Surfaced as an explicit coverage-panel warning, never a silent gap.
- Access path: latest sweep per company (`idx_history_sweeps_company_created`). Append-only, idempotent, self-healing migration (`CREATE TABLE IF NOT EXISTS`).

### Content Embeddings (dropped)

The `content_embeddings` vector index (migration `0049`, [ADR 0035](adr/0035-two-layer-ai-and-local-interpretative-layer.md)) was a **disposable, derived index** for the embedding-model similarity strategy. The model was retired by [ADR 0080](adr/0080-retire-embedding-model.md): migration `0069` drops the table and purges queued `content_embedding` jobs (forward, idempotent — zero canonical data loss, exactly the disposability ADR 0035 designed). The `similarity_strategy` settings row may still hold the legacy `embedding` value on old databases; reads map it to `static` and there is no setter. Similarly, migration `0070` drops the write-only `feed_items.story_key` column + index (its consumer, story clustering, was dropped in [ADR 0051](adr/0051-story-clustering-across-sources.md)).

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

### Retired AI Artifacts (ADR 0084 clean cut)

Migration `0102_clean_cut_ai_artifacts.sql` removes the in-app AI analysis
layer's stored artifacts ([ADR 0084](adr/0084-retire-in-app-ai-layer.md)
decision 5, revised 2026-07-20 — supersedes that decision's original
"orphan in place" text). Destructive by design, owner-approved against a
measurement of the live database, and covered by the pre-migration snapshot +
rotating backups (ADR 0032).

**Dropped tables** (18): `ai_analysis_results`, `ai_analysis_jobs`,
`ai_analysis_tags`, `ai_analysis_source_references`; `ai_research_briefs`,
`ai_research_brief_citations`, `ai_research_brief_jobs`; `ai_research_digests`,
`ai_research_digest_citations`, `ai_research_digest_jobs`;
`claim_extraction_jobs`, `claim_extraction_proposals`; `kpi_extraction_jobs`,
`kpi_extraction_proposals`; `ownership_ocr_proposals`,
`ownership_ocr_proposal_rows`, `ownership_holder_type_proposals`;
`company_ocr_extraction_profile`.

**Dropped columns** — in every case the table and all its rows survive; only
dead columns go:

- `morning_briefings.narrative_markdown` / `narrative_provider_id` /
  `narrative_model` — the table and `morning_briefing_items` stay (deterministic
  composition is now the only briefing).
- `history_sweeps.ai_calls_used` / `ai_call_limit` — the tier-4 budget
  accounting. Sweep rows and every non-AI field stay.
- `management_claims.extraction_proposal_id` — a soft reference (no foreign key)
  into the dropped `claim_extraction_proposals`; keeping it would leave a column
  that can only ever hold a dangling id. Claims and every other column stay.

**Deleted rows**: the settings keys listed under [Settings](#settings) as
retired, and the `financial_facts` whose provenance is `source_tier` `ai` or
`ai_text` — both AI-sourced values (`ai` from the AI confirm path, `ai_text`
from the retired tier-4 path) — **together with** those
`financial_fact_provenance` rows. The fact is deleted
before its provenance so no fact is ever left without provenance; references
into a deleted fact are resolved first (`management_claims.verifying_fact_id`
and `financial_facts.supersedes_id` are nulled, and the deleted ids are scrubbed
from `autopilot_run.produced_fact_ids_json`, which is a soft JSON reference with
no foreign key). After the cut no tier could reproduce or validate those values,
and leaving them would inflate the deterministic-coverage measurement.

**KEPT — measured as NOT AI despite their names**: `criterion_results` (all
`source='engine'`; deterministic DSL evaluations per ADR 0046 — the AI-assessor
path never wrote to the owner's database), `company_extraction_profile`
(deterministic PDF layouts as of this migration; its read/write code was later
retired by [ADR 0086](adr/0086-aggregator-primary-fundamentals.md) decision 1 —
the table itself stays kept, append-only), `morning_briefings` / `morning_briefing_items` rows,
`management_claims` (the manual path — rows and every surviving column),
`history_sweeps` rows, deterministic-tier `financial_facts` and their
provenance, and the `youtube_transcription_*` settings.

### Settings

Supports theme selection, polling defaults, local privacy choices, and provider configuration.

Recommended storage:

- `settings` key/value table for simple local preferences
- optional structured JSON values for provider configuration

Initial keys:

- `theme`
- `locale`
- `accent_palette`
- `developer_mode`
- `poll_interval_seconds`
- `backfill_years`
- `youtube_transcription_provider`
- `youtube_transcription_model`
- `youtube_transcription_timeout_seconds`
- `settings_import_export_format`
- `shortcut_bindings`
- `pinned_company_ids`
- `mcp_enabled`
- `mcp_port`
- `mcp_writes_enabled`
- `log_level`
- `log_max_files`
- `log_max_file_bytes`

Field defaults and allowed enum values (theme, accent palette, AI analysis mode, transcription model/timeout) are canonical in [Contracts § User Settings](contracts.md#user-settings).

Rules:

- Default poll interval is `900`.
- `backfill_years` (ADR 0077 §3) is the years of company history the on-track backfill covers. No seed row: tolerant read defaults to `3`, clamped to `[1, 10]` on both read and write (never rejected, like the pool settings). The backfill job reads it live; the const in `jobs/backfill.rs` is only the last-resort fallback if the settings read fails.
- `mcp_enabled` / `mcp_port` (ADR 0078 decision 4): the MCP server toggle (default `false`) and port (default `8317`, clamped `[1024, 65535]` on read and write). No seed rows: tolerant reads fall back to the defaults; the bearer token itself lives in the OS keychain, never in settings.
- `mcp_writes_enabled` ([ADR 0088](adr/0088-mcp-surface-v2-ui-parity.md) M3): the MCP `act` (write) tier gate. Default `false` (stored as a `"true"`/`"false"` string row, like `mcp_enabled`); no seed row, tolerant read. It is the ONLY toggle for agent writes, and `update_settings` is itself excluded from the MCP registry — so a connected agent can never enable its own writes.
- **Retired settings keys** ([ADR 0084](adr/0084-retire-in-app-ai-layer.md)): the in-app AI analysis layer's keys — `general_analysis_provider`, `general_analysis_model`, `general_analysis_timeout_seconds`, `ai_analysis_mode`, `espi_ai_fallback_enabled`, `capability_providers`, `openai_compatible_base_url`, `history_sweep_ai_call_limit`, plus the queue tuning `ai_workers` / `ai_provider_concurrency` — are no longer read or written. Rows a previous version stored are harmless orphans (append-only KV table, never migrated destructively); reads of any removed key are simply gone. Only the Gemini **transcription** provider/model/timeout keys remain.
- Default YouTube transcription provider is `provider_gemini` — the only remaining AI provider setting (transcription is data acquisition, not analysis; ADR 0084 decision 3).
- Default shortcut bindings are defined in code. `shortcut_bindings` stores only user overrides, disabled states, and resettable action-ID keyed changes as JSON.
- `pinned_company_ids` (ADR 0054) stores the companies the user has pinned to the sidebar IA spine as a JSON array of company IDs, in pin order. It is a simple local UI preference: default `[]` when the row is absent (tolerant read, no seed migration), de-duplicated and overwritten wholesale on update. Unknown IDs (deleted companies) are ignored at read time by the frontend.
- YAML import/export excludes secrets; implemented via the settings export/preview/apply commands (see [Contracts § User Settings](contracts.md)).
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

`doc_kind` (migration 0061; nullable `TEXT`, indexed `(company_id, doc_kind)`): the document-kind taxonomy ([ADR 0077](adr/0077-trusted-extraction-foundations.md) §1) — `periodic_ssf | periodic_jsf | auditor_opinion | presentation | governance | other`, marking which stored documents can carry extractable financial data. Classified deterministically from title + URL by `fundamentals::extraction::classify::classify_doc_kind` (Rust, not a SQL backfill). **Set on write**: insert/upsert classify immediately, and an upsert with a changed title reclassifies in place, so new documents never linger `NULL`. `NULL` = not yet classified (rows predating migration 0061); reads tolerate it as "unclassified". The idempotent `reclassify_report_documents` command recomputes the column corpus-wide to backfill/self-heal legacy rows.

Storage, dedup, and retention rules:

- **Mis-association guard + repair** (card 45fcece): tag-listing ingestion (bankier-company-komunikaty) stamps the tag's company onto every attachment. Before a row is created, an attachment that classifies as a **periodic report** (`doc_kind` `periodic_ssf`/`periodic_jsf` — the only kind that can win a period's canonical slot) is **rejected** when its filename names a *different* tracked issuer and no owner mention (name/alias/ticker over the article title, filename, and URL) is found; rejections are logged, no row. An idempotent startup repair (`repair_misassociated_report_documents`, wired in `lib.rs` setup, mirroring the `reclassify_report_documents` precedent) re-scans `espi_attachment` rows with the **same predicate** (no drift) and deletes the mis-associated ones (FK `CASCADE`/`SET NULL` clean their derived rows). Deliberately conservative — foreign detection uses the filename's full company-name/alias phrase only (never a bare ticker, never the URL hosting path), so counterparty/shareholder mentions and Bankier folder names never delete a legitimate filing (validated against the owner DB: the naive url/ticker rule would have wrongly deleted 36 rows; the scoped rule deletes 0).
  - **Known blind spot (metadata-only guard):** the guard reads title + URL, both of which can lie — a Bankier attachment title may be renamed to the owning ticker while the file's real content is a *foreign* issuer, and Bankier reuses one issuer's URL slug across unrelated same-day filings (e.g. a `Grupy-Energa` slug appearing on Vercom/Orlen/PKO documents whose content is those issuers' own). Only document **content** distinguishes these, which the metadata guard cannot see; a durable fix would need content/OCR verification. Migration `0107_repair_misassociation_and_note_ref_facts.sql` is the point-in-time repair for the one content-verified case (four scanned Energa Q3-2024 attachments OCR'd onto cyber_Folks): a **cyber_Folks-scoped** predicate (`company_id='company_gpw_cbf'` + `espi_attachment` + `url LIKE '%energa%'`) — deliberately not a global `url LIKE '%energa%'`, which would delete the legitimate Vercom/Orlen/PKO filings that carry the reused slug. The same migration deletes note-reference-misparsed pdf-tier cash facts (auto-unreviewed, whole multiple of 1000 ≤ 60 000) for re-extraction with the corrected parser (cards 22ac70c / 40281b3). Forward, idempotent, self-healing: a clean database matches nothing.
  - **ESEF-anchored delete-for-refill of misscaled pdf facts** — migration `0108_esef_anchored_refill_misscaled_pdf_facts.sql` (v0.59). A uniformly ×1000/×1e6 misscaled statement passes the balance-sheet identity (the whole statement is off by one multiplier, so it still balances) and has no comparative column to object, so the deterministic gate auto-*confirms* it. The one witness that reads the true scale is the company's **own ESEF filing** for the same metric. 0108 deletes-for-refill every fact that is **all** of: `source_tier='pdf'`, **unconfirmed** (`confirmation_state IN ('auto_unreviewed','pending')` — never `confirmed`), and grossly off its **ESEF anchor** — `MAX(|value|)` over the same company+metric among `esef`-tier facts, any period — by ≥100× (high side) or ≤1/100 (low side). A company+metric with **no** esef anchor is **not** touched (it surfaces through the runtime plausibility gate instead). Detaches soft references (autopilot produced-id list, claim verification link, superseding pointer) before deleting fact-then-provenance, order matching 0102/0107. Forward, idempotent, self-healing: a `confirmed` fact of any magnitude, a within-100× unconfirmed fact, and an anchorless company all survive; a clean database matches nothing. On the maintainer's DB it deletes 91 facts (73 high-side, 18 low-side). Because migrations run before any extraction, 0108 cleans the medians the runtime plausibility gate (below) reads before that gate ever evaluates.
- `local_path` always stores a **relative** path under a dedicated `report_documents/` subtree (keyed by company) so the store stays portable across machines and survives import/export. It is never an absolute path.
- **Full files are stored for periodic / financial reports** (the extraction/diff targets), determined from the filing's classified `company_signals` category and ESPI/EBI report metadata, **and, independently, for any structured ESEF/iXBRL attachment** ([ADR 0061](adr/0061-deterministic-fundamentals-data-gathering.md) decision 1b): an attachment whose URL ends `.xhtml` is always a fetch candidate, even on a filing the periodic-report text classifier does not recognize (a filing can be xhtml-only under the EU ESEF mandate and miss the Polish-language heuristic). A digital-signature attachment (`.xades`) is always `metadata_only` — it carries no financial data, only kept for the audit trail. Every other ESPI/EBI attachment (e.g. a PDF on a non-periodic filing) persists as **metadata + URL only**: `local_path` null, `fetch_status = 'metadata_only'`, preserving `url`/`title`/`attribution`/`origin_ref` for citation and the source ladder. The user-URL and IR-page rungs always store the full file.
- `fetch_status` is `pending | fetched | failed | metadata_only` (the `metadata_only` value is additive over migration 0035 — no new migration). It is the single source of truth for whether bytes exist locally; a failed fetch records `fetch_error`, stays retryable, and never blocks feed-item ingestion.
- Identity is `UNIQUE(company_id, url)`; capture, refresh, and backfill **upsert** on this key. `content_hash` is a secondary same-company dedup signal so a re-fetch of identical bytes is not duplicated.
- Retention reuses the feed-retention protection model ([ADR 0033](adr/0033-feed-retention-policy.md)): a document referenced by a confirmed `financial_fact`, linked as research evidence, or backing a confirmed signal derivation is **protected** and never pruned. Unprotected full files past the retention window have their **bytes** pruned (file deleted, row downgraded to `metadata_only`); the metadata row itself is never deleted. On-disk report-document size and the retention window are surfaced in Settings → Data retention alongside the feed controls.

### Report Document Sections

Supports the report-over-report diff ([ADR 0052](adr/0052-report-over-report-diff.md), `v0.47.0`): the extracted-text section structure of a stored **financial-statement** report document (consolidated SSF / standalone JSF), used as the deterministic substrate the diff compares.

This table is a **disposable, derived index**, never a source of truth. Every row is computed from a `report_documents` row's stored bytes via pure-Rust text extraction ([ADR 0052](adr/0052-report-over-report-diff.md)). Dropping the table loses zero canonical data — it only forces re-extraction.

Fields:

- `report_document_id` → `report_documents(id)` (the source document; cascade-cleaned with it).
- `ordinal` — 0-based position of the section within the document; the stable per-document section identity (heading text is **not** unique, so alignment keys on heading + ordinal, never heading alone — exact-heading-only matching breaks the deterministic self-diff invariant).
- `heading` — the detected section heading text (normalized: leading numbering stripped, lowercased) or `<preamble>` for pre-first-heading content.
- `body` — the section's extracted plain text.
- `content_hash` — sha256 of the source document bytes the extraction ran over, so re-extraction skips unchanged documents and the diff read model can detect staleness.
- `extractor_version` — the extraction-heuristic version, so a heuristic change can invalidate and rebuild affected rows.
- `created_at`

Primary key: (`report_document_id`, `ordinal`).

Rules:

- Population is an async background job offloaded off the UI thread (extraction over a multi-page PDF is CPU work); reads of the diff tolerate a not-yet-extracted document (the diff read model reports `extraction_pending`).
- Extraction handles **both source formats** found across the GPW + NewConnect market ([ADR 0052](adr/0052-report-over-report-diff.md)): **PDF** (`pdf-extract`, run inside `catch_unwind` — it panics on a small fraction of real PDFs) and **ESEF/iXBRL `.xhtml`** (HTML parse, stripping the inline-XBRL header / `display:none` facts; increasingly common under the EU ESEF mandate — some large issuers file xhtml-only with no PDF). Each document records an `extraction_state`: `extracted` | `no_text_layer` | `extraction_failed`. A `pdf-extract` panic is caught and recorded as `extraction_failed` (flagged, not-diffable), never crashing the job.
- Only **financial-statement** report documents are extracted in `v0.47.0`; the narrative management report (MD&A) is out of scope (deferred — [ADR 0052](adr/0052-report-over-report-diff.md)). A scanned/image document with text density (chars/page) below threshold records `no_text_layer` and is not diffable (no OCR) — a real ~10% class across the market, concentrated in small NewConnect issuers.
- The **diff itself is never stored** — it is an on-demand backend read model computed from two documents' sections (heading + positional alignment; no similarity call — verified during [ADR 0080](adr/0080-retire-embedding-model.md)). No AI summary is produced or cached this milestone.
- Append-only, idempotent, self-healing migration (`CREATE TABLE IF NOT EXISTS`); rebuildable regardless of prior state.

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

Product-level feature deferrals are tracked once in [Roadmap § Not In V1](roadmap.md#not-in-v1) (portfolio position tracking, trade journal, billing/payment infrastructure, hosted license activation, multi-user/team accounts, cloud sync and hosted data services) — none of those get tables here either.

Data-model-specific: no `users`/`organizations` tables (single local user, no team permissions); no account-balance or trade-ledger tables backing portfolio tracking.

## Import And Export Documents

M20 import/export documents are portable files, not runtime tables. Document contents (research JSON / settings YAML fields) and exclusions are canonical in [Contracts § Import And Export](contracts.md#import-and-export).

Rules:

- Import validates documents before any storage change.
- Research import applies through existing SQLite tables inside one transaction.
- Settings import writes through the same settings validation path used by normal Settings updates.
