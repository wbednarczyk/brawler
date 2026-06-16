# ADR 0036: Report-Document File Storage, Retention, and On-Track History Backfill (Design)

Status: Accepted

This ADR captures the **design** for persisting report files and backfilling company history (epic `a3e7555`, milestone `v0.41.0`). It records where report files live on disk, the periodic-only full-file rule, retention/dedup, the on-track backfill action (depth, idempotency, throttling, progress), and how derived calendar events are dated — so the contracts and data model are decision-complete before implementation.

It builds on:

- [ADR 0027](0027-company-fundamentals-scope.md) — fundamentals scope; rejected per-company PDF parsers.
- [ADR 0029](0029-ir-page-report-resolution.md) — the report-document source ladder (ESPI/EBI attachment → IR page → manual URL). This ADR fills in the **attachment** rung's fetch/storage behavior.
- [ADR 0033](0033-feed-retention-policy.md) — feed retention; report-document retention is the file-storage analogue and reuses its protection model.
- [ADR 0034](0034-espi-event-classification.md) — typed `company_signals` and the `derived_event_id` wiring; this ADR completes the §4 derivation it deferred to `v0.41.0`.

## Context

Two gaps remain after typed ESPI classification (`v0.40.0`):

1. **No report file is stored.** `report_documents` (migration 0035) exists and the `user_url`/`article` capture path shipped in `v0.34.0`, but the **ESPI/EBI attachment** rung of the ADR 0029 ladder was never implemented: the active Bankier company-komunikaty article pages expose attachment links, but the app does not fetch or persist them. AI KPI extraction and report-over-report diff have nothing local to operate on for a real filing.
2. **Cold start.** Tracking a new company starts its research history at "now". The Bankier komunikaty JSON listing paginates (`/articles/listing/{page}/{limit}`), so prior filings are reachable, but nothing backfills them.

And one piece of `v0.40.0` was deferred here: deriving a dated `company_events` row from a dividend or general-meeting signal needs the **future date**, which lives only in the filing **body** — reachable only once the article/attachment body-fetch path exists.

Constraints from the existing system:

- The active official-report source is **Bankier company-komunikaty**, not the disabled `gpw-espi-ebi`. All attachment-fetch and backfill behavior targets the active Bankier article/attachment path and stays source-neutral for a future GPW re-enable.
- Report PDFs are multi-MB; a 3-year backfill across many tracked companies × all filings would store a large amount of low-value content (most ESPI/EBI filings are short administrative notices, not periodic reports).
- Source-policy (`AGENTS.md`) requires serialized, throttled, tracked-company-scoped fetching; no broad crawling.

## Decision

### 1. File storage location and the periodic-only full-file rule

- Fetched files are stored under the app data dir in a dedicated `report_documents/` subtree, keyed by company, with `report_documents.local_path` holding the **relative** path (never an absolute path, so the store stays portable across machines and survives import/export).
- **Full files are downloaded and stored only for periodic / financial reports** — the documents that extraction and diff actually consume. Other ESPI/EBI attachments (administrative notices, insider-transaction notifications, etc.) persist as `report_documents` **metadata + URL only** (`local_path` null, `fetch_status = 'metadata_only'`): the row records `url`, `title`, `attribution`, and `origin_ref` for citation and the source ladder, but no bytes are stored.
- "Periodic / financial report" is determined from the filing's classified `company_signals` category and ESPI/EBI report metadata (report-type label/title), not from a per-company rule. A filing that is later reclassified as periodic can have its file fetched on demand via the same capture path.
- The user-supplied URL escape hatch (ADR 0029 rung 3) and IR-page resolution (rung 2) **always** store the full file, because the user explicitly asked for that specific document.

### 2. Fetch lifecycle and dedup

- `fetch_status` extends the existing `pending | fetched | failed` with `metadata_only` (per §1) and is the single source of truth for whether bytes exist locally.
- Document identity stays `UNIQUE(company_id, url)` (migration 0035). Backfill and refresh **upsert** on this key, so re-runs never create a second row for the same source URL.
- `content_hash` (sha256) is computed for fetched files and used as a secondary dedup signal: if a newly fetched file matches an existing document's hash for the same company, the fetch is treated as already-stored rather than duplicated.
- A failed fetch records `fetch_error` and stays retryable; it never blocks ingestion of the originating feed item.

### 3. Retention — reuse the feed-retention protection model

Report files inherit [ADR 0033](0033-feed-retention-policy.md)'s protection-first stance applied to disk:

- **Protected documents are never pruned**: any document referenced by a confirmed `financial_fact` (`source_document_ref`), linked as research evidence, or backing a confirmed `company_signal` derivation.
- Unprotected **full files** past a retention window may have their **bytes** pruned (file deleted, row downgraded to `metadata_only` so the URL/attribution/citation survive) — the metadata row itself is never deleted, because it is cheap and preserves provenance.
- Metadata-only rows are never auto-pruned (negligible size).
- The retention window and on-disk report-document size are surfaced in Settings → Data retention alongside the feed-retention controls from ADR 0033, not as a separate surface.

### 4. On-track history backfill

- Backfill is **always an explicit user action** (on track or from the company workspace) — never automatic. It is the cold-start remedy, consistent with the local-first posture.
- **Depth: ~3 years.** Backfill paginates the Bankier company-komunikaty listing backward until items fall outside the window, ingesting **periodic reports + ESPI/EBI filings**. Files are stored per §1 (full files for periodic reports only).
- **Calendar is out of scope for backfill.** Historical calendar entries are not backfilled; the existing forward-looking calendar adapters (`official_calendar`/`public_calendar`) own upcoming events. Backfilled past filings still classify into `company_signals` and surface on the research timeline with their **original publication dates preserved**.
- **Idempotent.** Backfilled items flow through normal ingestion and reuse existing dedup keys (feed-item `(source_adapter_id, source_event_key)`, report-document `(company_id, url)`, signal `(feed_item_id, category)`). Re-running backfill produces no duplicate feed items, documents, signals, or events.
- **Throttled.** Backfill obeys the existing Bankier rate policy: serialized requests, waits between pages/companies, `LocalInvestorNewsfeed/{version}` user agent. It runs as an async, cancellable job with **progress and per-stage diagnostics** (pages fetched, items ingested, documents stored, errors) surfaced to the user; a partial/cancelled run is safe to resume because of idempotency.
- Backfill runs only while the app is open (it is not the `v0.50.0` autopilot; no closed-app fetching).

### 5. Derived calendar events from dated signals

Completing the [ADR 0034](0034-espi-event-classification.md) §4 derivation deferred to this milestone:

- A `company_events` row is derived **only** for `dividend` and `general_meeting` signals that carry a real future date, and only from **confirmed** signals (rule-confirmed, or AI-proposed-then-user-confirmed).
- **Date extraction is deterministic-first, AI-fallback, always confirm-before-create:**
  1. A deterministic parser scans the fetched filing body for the relevant future date (dividend record/payment date, general-meeting date) using structured/labelled patterns.
  2. If the deterministic parser cannot confidently extract a date, the **opt-in async AI fallback** ([ADR 0028](0028-multi-provider-ai-boundary.md)) extracts it. The AI fallback is disabled by default and makes no provider calls until the user enables it.
  3. Either way, the derived event is created as **proposed** and requires explicit user confirmation before it appears on the calendar. The conservative posture from ADR 0034 holds: **never create a guessed-date calendar event.**
- Derivation is idempotent and carries origin linkage: the event links back via `company_signals.derived_event_id` to the signal and the originating `feed_item`; re-confirming or re-running never duplicates the event, and it dedups against manually created events for the same company/date/type.

## Scope boundary

- In scope (`v0.41.0`): the ESPI/EBI attachment fetch/storage path (periodic-only full files), the user-URL/IR-page full-file rungs already wired, report-file retention reusing the ADR 0033 model, the explicit on-track 3-year backfill of reports + filings, and deterministic-first/AI-fallback/confirm-before-create derivation of dividend + general-meeting calendar events.
- Out of scope: per-company PDF parsers and deterministic ESEF/iXBRL parsing (rejected in ADR 0027 / separate study); backfilling non-official media sources; historical **calendar** backfill; automatic backfill without user action; closed-app fetching (that is the `v0.50.0` autopilot frontier).

## Consequences

- Real periodic-report files exist locally for extraction, diff, and citation, while disk stays bounded because the long tail of administrative filings is metadata-only.
- Tracking a company immediately yields a multi-year research timeline instead of starting at "now".
- New surfaces/behaviors: the attachment fetch path, the `metadata_only` fetch state (additive, no destructive migration — extends the existing `fetch_status` value set), a report-document retention window in Settings, the backfill job + progress/diagnostics + command, and the dividend/GM event-derivation job with confirmation. Contracts and data-model are updated in the same change.
- The `report_documents` schema (migration 0035) is **unchanged** except for the additive `metadata_only` value convention; no new columns are required, so no new migration is needed for storage. Any seed/registry change (e.g. marking categories `derives_event`) already shipped in `v0.40.0`.
- Owner-confirmed decisions (milestone `v0.41.0` start): (a) **full files for periodic reports only**, metadata-only for other filings; (b) **3-year backfill of reports + filings**, calendar left to the existing forward-looking adapters; (c) **deterministic-first → opt-in AI fallback → always confirm-before-create** for derived dividend/GM event dates; (d) this ADR is the canonical home for report-file storage/retention + backfill policy, with event derivation owned jointly with ADR 0034.
- Related: this is the building block under the autonomous report pipeline ([roadmap.md](../roadmap.md) North Star, `v0.50.0`); detection there reuses the attachment path and classified signals, and the backfill job is the manual precursor to autopilot fetch.
