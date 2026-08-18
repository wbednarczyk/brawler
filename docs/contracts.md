# Contracts

This file defines initial contracts for the first implementation. Field names are intentionally stable enough for code scaffolding, but exact serialization may be refined with tests before the first API release.

Doc map: [CLAUDE.md](../CLAUDE.md) § Required Reading. Related references: [Architecture](architecture.md), [Data Model](data-model.md), [Source Strategy](source-strategy.md), and [Product Spec](product-spec.md).

## Command Conventions

This file states the wire shapes, commands, and command-specific rules for every entity; **field-level storage rules (uniqueness, FKs, soft references, retention) are canonical in [Data Model](data-model.md)** — each section below points there instead of restating them. Conventions shared by every section, stated once:

- **Structure.** A section shows the wire JSON shape(s) first, then allowed enum values, then `Rules:` (command-specific behavior), then the typed Tauri commands (`Commands:`/`Typed commands:`/"Initial local commands:").
- **Errors.** Typed commands surface failures as a typed command error the frontend maps to a user-facing message; there is no bare-string/panic error path across the Tauri boundary. Machine-readable failure kinds: [Error codes](#error-codes). Async work (jobs, extraction, backfill) reports failure through job status fields (`status: "failed"`, `errorCode`, `error`) rather than a rejected command call.
- **Scope.** Commands accept and return the **canonical id** (`companyId`, `watchlistId`, etc.), never a raw ticker or display string; canonical identity and uniqueness rules live in [Data Model](data-model.md). Company-scoped vs watchlist-scoped behavior is called out per section only where it differs from this default.

### KPI acquisition workflow tools (ADR 0098 / ADR 0099)

*LIVE — [ADR 0098](adr/0098-mcp-native-kpi-acquisition-lifecycle.md), [ADR 0099](adr/0099-acquisition-mcp-surface-mechanics.md), epic #353. **Shapes frozen here.** The implementation (#382–#386) is written to these shapes; changing them is a deliberate contract change. All nine tools are live: 1, 2, 8, 9 since #384; 3–4 since #385; 5–7 since #386 (the section graduated from Planned with #386).* A compact run-based MCP workflow of **nine tools** behind the acquisition-scoped credential (ADR 0099 dec. 2–3): the eight workflow tools plus `get_kpi_ingest_document` (portable document-byte delivery). `record_financial_facts` remains a low-level repair tool, no longer the normal agent path. Validation and commit execute synchronously inside the tool call (ADR 0099 dec. 1). The mechanics (credential, gate, lease, blobs, profiles, budgets, execution metadata) are canonical in ADR 0099; this section freezes the wire.

Conventions: camelCase, `deny_unknown_fields`, internal ids only. String limits are **UTF-8 byte** limits; control characters (U+0000–U+001F) are rejected in every text field (`invalid_input`) — this bounds JSON-escaping expansion at ×2 and guarantees every schema-valid request stays under the 1 MiB transport cap (a schema-INVALID oversized body is refused by the transport with 413 before JSON parsing). Failures use the `CommandError` envelope with three additive codes (see [Error codes](#error-codes)): `run_lease_expired`, `run_taken_over`, `response_budget_exceeded`.

Shared shapes:

```json
RunStatus = {
  "runId": "kpiing_…",
  "status": "discovered|source_captured|extracting|staged|validation_failed|ready_to_commit|committing|complete|partial|failed|cancelled",
  "revision": 0, "manifestHash": "…|null",
  "documentId": "…", "companyId": "…", "profileVersion": "gpw_ifrs_annual@v1",
  "scope": "standalone|consolidated|null", "dataQuality": "final|preliminary|estimated|null",
  "period": { "fiscalYear": 2026, "periodType": "StoredPeriodType", "periodId": "…|null" },
  "sourceContentHash": "sha256…|null", "attemptCount": 1,
  "expectedKpis": { "schemaVersion": 1, "source": "…", "packVersion": "…|null", "keys": ["…"] },
  "missingReasons": { "<metricKey>": "<reason>" },
  "lease": { "holder": "mcp:kpi_acquisition", "expiresAt": "…" },
  "progress": { "…": "progress_json verbatim" }, "execution": { "…": "cost_json verbatim" },
  "lastError": "…|null", "createdAt": "…", "updatedAt": "…"
}
ExecutionMeta = { "client": "…", "model": "…", "skillVersion": "…", "repairRounds": 0, "tokensIn": 0, "tokensOut": 0, "costUsd": 0.0 }
```

- `period`, `expectedKpis`, `lease`, `progress`, `execution` are nullable objects; `missingReasons` normalizes stored NULL to `{}`. Since #383 a freshly created run carries a non-null `expectedKpis` stamped at creation (`packVersion` = its `profile_version`) — `expectedKpis` is NULL, and `packVersion` nullable, **only** for legacy raw-seeded rows (live-stamped by first validation with `packVersion: null`). `ExecutionMeta.client` is required (≤128 B); the other fields are optional, strings ≤128 B, numerics non-negative; `cost_json` = `{"schemaVersion": 1}` + `ExecutionMeta` verbatim (one schema everywhere).
- **Two period vocabularies**: `StartPeriodType = Q1|H1|9M|FY` (start input — the validator refuses the rest via `run.unsupported_period_grammar`, so a run doomed to fail cannot start); `StoredPeriodType = FY|H1|H2|Q1|Q2|Q3|Q4|9M|M01..M12` (status/list output — durable rows may carry the full storage vocabulary).

The nine tools:

1. **`start_kpi_ingest`** *(shipped, #384)* — two-phase (period/scope/quality are often discoverable only FROM the document, and the document reader requires a run). Fresh variant: `{ documentId, profileId, scope?, dataQuality?, period?: { fiscalYear, periodType } }` → creates the run (idempotent on the active triple), claims the lease, pins the content-addressed source blob, `mark_source_captured`; with all three context values present it proceeds straight to `extracting`, otherwise the run stops at `source_captured` (the agent reads the document in chunks and discovers the context). Resume variant: `{ runId, scope?, dataQuality?, period? }` → idempotent targeted claim (same-holder renewal never increments `attemptCount` — this is the explicit **keepalive**), attaches missing context values **set-once** (a conflicting value → `conflict`), enters `extracting` when complete; bare `{ runId }` is a pure keepalive/resume. Result: `RunStatus`. Typed refusals: document without a local file, `profileId`/`profile_version` outside the registry (`invalid_input`), `run_taken_over`.
2. **`list_pending_kpi_ingests`** *(shipped, #384)* — `{ companyId?, limit? (≤50, default 20), cursor? }` → `{ items: [RunSummary], nextCursor: "…|null" }`. `RunSummary` = `{ runId, documentId, companyId, status, profileVersion, period|null, attemptCount, lease: {expiresAt}|null, createdAt }`. Pending = the claimable states (`discovered`, `source_captured`, `extracting`, `validation_failed`); keyset cursor over `(createdAt, id)`.
3. **`get_kpi_ingest_context`** *(shipped, #385)* — `{ runId, section?: "catalog"|"plausibility"|"manifest", cursor?, limit? }`. The default call (no `section`) returns everything within caps: `{ run: RunStatus, document: { url, title|null, contentType|null, byteSize|null, localPath|null }, derivedPeriod: { fiscalYear, periodType, periodEnd }|null, catalog: [{ definitionId, metricKey, label, unit|null, statementGroup, valueKind, origin }], plausibility: [{ metricKey, slot: { definitionId, scope, attribution, measureWindow }, slotOrigin: "observed"|"candidate", median|null, nonZeroCount, abstentionReason|null, recentPoints: [{ fiscalYear, periodType, value }] }], profileRules: ["…"], manifestAvailable: bool, truncated: { "catalog": "cursor"?, "plausibility": "cursor"?, "manifest": true? } }`. `catalog` = definitions for the expected keys plus the company's minted extras (`label`/`unit`/`statementGroup` are required for interpreting Polish/minted KPIs); `plausibility` is validator-equivalent evidence (exact slot identity, whole-history median, non-zero count, abstention reason, bounded recent points) — `observed` entries are slots the fact store realizes, `candidate` entries are the recommended default slot a staged observation lands in (dated addition, #385); `median` is `null` exactly under `thin_history`; `document.byteSize` is the size of the run's PINNED BLOB, never the mutable document row's; `derivedPeriod` is a hash-guarded hint served only when the cached derivation describes the run's frozen bytes; `localPath` is a local-client convenience, never the delivery contract. **Overflow recovery**: a section exceeding its cap returns truncated with a cursor in `truncated`; the remainder is fetched via section calls — never a dead end. **Section pages** (dated completion, #385): a section call returns `{ runId, section, catalog?|plausibility?|manifest?, nextCursor|null }` carrying only the requested section; an empty terminal page is legal. Cursors are opaque; the special `{}` start-of-section cursor (emitted when the default call had to shrink a section to zero) restarts catalog/plausibility from the beginning and is refused for manifest. The manifest (repair context; it can exceed the budget by itself) is NEVER in the default call — only `section: "manifest"`, paginated over observations (manifest header + `runDiagnostics`/`completeness` on the first page; continuation cursors pin the validation attempt they started from, so a newer attempt never splices into a paginated read; `section: "manifest"` with no attempt yet → `conflict`). Pure read, no side effects.
4. **`get_kpi_ingest_document`** *(shipped, #385)* — `{ runId, offset, length (≤262144) }` → `{ bytesBase64, offset, length, totalBytes, sha256, eof }`. Reads the run's content-addressed blob (never `local_path`), verified against the frozen `sourceContentHash`; available whenever the run HAS a captured source (`source_captured` onward, terminal states included — a run cancelled before capture → `conflict`); a read at/past EOF returns empty bytes with `eof: true` — the foundation of the two-phase start and the only portable delivery path (remote clients, WSL↔Windows, dead URLs).
5. **`stage_kpi_observations`** *(shipped, #386)* — `{ runId, observations: [ObservationInput] (1..100), missingReasons: { "<metricKey>": "<reason>" }, execution?: ExecutionMeta }` → `{ runId, revision, observationCount, status: "staged" }`. **`observations` is the COMPLETE revision snapshot** (storage replaces the whole set per revision — a repair must resend the retained observations). **`missingReasons` is required**; `{}` is the explicit "no declared omissions"/clear — there is no destructive default; it is written in the SAME staging transaction. `ObservationInput` mirrors the staging boundary: `rawLabel`/`rawValue` non-blank ≤256 B; `metricKeyCandidate` ≤256 B; `normalizedValue` decimal string ≤64 B; `rawCurrency`/`rawUnitScale` unnormalized source strings ≤128 B; `currency` = 3 uppercase ASCII letters; `unitScale` ∈ `ones|thousands|millions`; `measureWindow` ∈ `flow|point_in_time|trailing|cumulative|duration`; `attribution` ∈ `total|owners_of_parent|nci`; `scope` ∈ `standalone|consolidated`; `mappingStatus` ∈ `unmapped|mapped|no_definition` (default `unmapped`); `citation: { page? (≥1), table? (≤128 B), row? (≤128 B), quote? (≤1024 B) }`; `missingReasons` ≤128 entries, key ≤128 B, reason ≤512 B. Transport arithmetic (string payloads at every documented maximum; excludes JSON field/structure overhead): 100 max-bounded observations ≈ 269 KiB raw + ≤83 KiB reasons ≈ 352 KiB, ×2 escaping bound ≈ 703 KiB — every schema-valid request reaches tool validation under the 1 MiB cap. Raising the 100 cap is an additive change.
6. **`validate_kpi_ingest`** *(shipped, #386)* — `{ runId, revision }` (**generation-pinned** — the revision from the stage response; the synchronous path pins its generation exactly like the queue). Result: `{ outcome: "ready"|"failed"|"superseded", revision, manifestHash|null, manifest|null, current: { status, revision, manifestHash }|null }`. Exact staged generation → validates and returns the full manifest ([Data Model § validation](data-model.md#kpi-ingest-validation) is the manifest shape; a `failed` manifest IS the typed repair report). A moved generation/state → `superseded` with the current tuple. A broken state, empty staging, or storage failure → `CommandError`.
7. **`commit_kpi_ingest`** *(shipped, #386)* — `{ runId, manifestHash, revision, execution?: ExecutionMeta }` → the receipt, faithful to the durable shape: `{ runId, terminalStatus: "complete"|"partial", periodId|null, acceptedCount, outcomesSchemaVersion, outcomes: [{ observationId, revision, ordinal, metricKey, factId|null, outcome, detail? }], manifestHash, manifestRevision, committedAt }`. `detail` is omitted for ordinary outcomes (only `divergent` carries `{ existingFactId }`). Idempotent replay returns the stored receipt verbatim; a stale tuple → `conflict`.
8. **`get_kpi_ingest_status`** *(shipped, #384)* — `{ runId }` → `RunStatus`. Pure read.
9. **`cancel_kpi_ingest`** *(shipped, #384)* — `{ runId }` → `RunStatus` (`cancelled`). From `committing` or a terminal state → `conflict` (ADR 0098 dec. 6).

Rules:

- **Scope allowlist = exactly these nine tools** (ADR 0099 dec. 3); the acquisition credential discovers and captures nothing — capture and URL→`documentId` resolution belong to the UI/Full scope/#354; "process all pending KPI ingests" presumes pending runs exist. Full scope is a superset.
- **Budgets** (runtime, typed overflow — never silent truncation): stage ≤100 observations with the byte limits above; context pages: catalog ≤64 definitions, plausibility ≤64 slots with ≤8 recent points each, manifest section ≤50 observations; context OUTPUT strings are byte-bounded too (label ≤256 B, unit/statementGroup ≤64 B, profileRule ≤512 B, abstentionReason ≤256 B; overlong stored values are per-field truncated with an `…` marker) so every response is ≤256 KiB by construction; document chunk ≤262144 B; `list_pending` limit ≤50; the scoped `tools/list` carries its own frozen snapshot since #386 (`tools_list_schema_acquisition` + the ≤16 KiB byte gate as regression). Unsatisfiable requests (e.g. out-of-range `limit`) → `response_budget_exceeded`. Enforcement mechanics (as-built, #385): the ≤256 KiB context budget is provable because every variable stored field is bounded at its producer — KPI-definition identities (`metric_key`, imported ids) ≤256 B control-character-free at both definition writers, `last_error` ≤2048 B at its writer, the creation-time expected stamp ≤256 keys (a larger relevance union refuses run creation) — and the default call dynamically shrinks its pageable sections (cursors in `truncated`, `{}` = start-of-section) before a final defensive `response_budget_exceeded` gate that no bounded stored state can reach.
- Reads are side-effect-free; only `start_kpi_ingest` renews the lease. Lease loss → `run_lease_expired`, recovery = `start(runId)`; takeover after expiry → `run_taken_over`, abandon the run.

## Error codes

Commands adopting [ADR 0070](adr/0070-typed-command-error-envelope.md) reject with the `CommandError` envelope; pre-migration commands keep rejecting with bare strings until touched (strangler adoption — the frontend `callCommand` wrapper accepts both shapes). `From<StorageError>` assigns codes centrally in `src-tauri/src/commands/error.rs` with a wildcard-free match, so a new storage variant forces a deliberate code choice.

```json
{ "code": "missing_credential", "message": "no API key stored for provider_openai" }
```

The `code` field is a closed, **additive-only** enum (never removed or repurposed); `message` stays the human-readable detail:

| Code | Meaning | Retry semantics |
| --- | --- | --- |
| `not_found` | A referenced entity does not exist (lookup miss, dangling soft reference). | Not retryable as-is; refresh the referencing view. |
| `invalid_input` | Caller-supplied value failed validation. | Not retryable unchanged; fix the input. |
| `missing_credential` | A required secret is absent from the OS keychain. | Not retryable until configured; UI links to Settings → AI. |
| `network` | A network/HTTP call failed (timeout, DNS, reset). | Retryable; UI offers a retry affordance. |
| `provider` | An upstream AI/source provider rejected or failed the request. | Sometimes retryable; surface the provider detail. |
| `conflict` | The operation conflicts with current state (uniqueness/constraint violation, stale write). | Retryable after refreshing state; UI prompts a refresh. |
| `internal` | Unexpected internal failure with no more specific code. | Not user-retryable; report/log. |
| `run_lease_expired` (#384, ADR 0099) | The caller's ingest-run lease lapsed. | Retryable via `start_kpi_ingest(runId)` (idempotent re-claim). |
| `run_taken_over` (#384, ADR 0099) | Another holder claimed the ingest run after lease expiry. | Not retryable; abandon the run. |
| `response_budget_exceeded` (#384, ADR 0099) | The request cannot be satisfied within the response budget. | Retryable after narrowing/paginating the request. |

## Company Identity

Canonical company identity is exchange-qualified:

```json
{
  "id": "company_gpw_cdr",
  "exchange": "GPW",
  "ticker": "CDR",
  "qualifiedTicker": "GPW:CDR",
  "displayName": "CD PROJEKT S.A.",
  "isin": "PLOPTTC00011",
  "cik": null,
  "lei": null,
  "aliases": ["CD PROJEKT"],
  "sourceIds": {
    "gpw": "PLOPTTC00011"
  }
}
```

Rules (uniqueness of `qualifiedTicker`/`ticker` and optional identifier fields are canonical in [Data Model § Companies](data-model.md#companies)):

- Source adapters may attach source-specific IDs without changing the canonical identity.
- UI ticker labels may split `qualifiedTicker` into exchange and symbol for styling, including explicit known-exchange colors and deterministic fallback colors for future exchanges, but command payloads and storage continue to use the unchanged `qualifiedTicker` string.

`delete_company(companyId)` deletes a company by its canonical id. Owned rows (watchlist memberships, feed item links, notebook entries, claims, events, and other company-scoped data) are removed through local referential integrity.

**Sector** ([ADR 0067](adr/0067-market-data-foundation.md) Decision 3, `v0.53.0`): a company carries a `sector` classification auto-populated from the GPW/NewConnect directory (`sector_source='registry'`), with a manual override that a registry refresh never clobbers. `get_company_sector(companyId)` returns the current sector (or `null`); `set_company_sector(companyId, sector)` sets a manual override (`sector_source='manual'`) and returns the stored value — an empty/null `sector` clears the manual override, letting the next registry refresh fill it. `list_company_sectors()` returns the distinct registry-sourced taxonomy (active directory entries), the preset values the override picker offers so manual entries stay on the same taxonomy the KPI `sector` scope keys off. Field-level storage rules are canonical in [Data Model § Companies](data-model.md#companies). The taxonomy folds case variants (the GPW and NewConnect taxonomies spell shared sectors differently) into one entry, most frequent spelling first; the UI offers it as type-to-filter suggestions, never a full preset wall ([ui-authoring § visual-first](ui-authoring.md)).

**Basic info** (owner request 2026-07-14): `get_company_basic_info(companyId)` returns the read model behind the "Basic info" cockpit panel — identity facts plus sector provenance and the latest recorded shares fact:

```json
{
  "displayName": "CD PROJEKT S.A.",
  "exchange": "GPW",
  "ticker": "CDR",
  "qualifiedTicker": "GPW:CDR",
  "isin": "PLOPTTC00011",
  "sector": "Gry",
  "sectorSource": "registry",
  "sharesOutstanding": "99895500",
  "sharesOutstandingPeriod": "2025 FY"
}
```

`sectorSource` is `"registry"` | `"manual"` | `null`. `sharesOutstanding` is the latest non-superseded `shares_outstanding` fact's stored numeric string (most recent period wins) with its period label; both are `null` when no fact exists — absent values render `—`, never invented. Errors: unknown `companyId` fails with a message.

## Watchlist Membership

Watchlists group canonical company identities for filtering and later ingestion scope.

```json
{
  "id": "watchlist_main_gpw",
  "name": "Main GPW",
  "description": null,
  "companyCount": 12
}
```

Membership mutation input:

```json
{
  "watchlistId": "watchlist_main_gpw",
  "companyId": "company_gpw_cdr"
}
```

Rules:

- Assigning a company to the same watchlist more than once is harmless.
- Removing a company from a watchlist must not delete the company.
- Renaming a watchlist updates its display name and optional description but preserves its stable watchlist `id`.
- Deleting a watchlist removes its memberships and must not delete member companies.
- Deleting a company removes its watchlist memberships through local referential integrity.
- UI-facing read models may list watchlist memberships separately from company identity so company identity remains canonical and watchlist-specific views stay explicit.
- The dedicated watchlist-management UI may add only already-tracked companies to watchlists; adding a brand-new company remains a company-management operation.

## Source Adapter

Each source adapter must expose metadata and a fetch operation.

```json
{
  "adapterId": "gpw-espi-ebi",
  "displayName": "GPW ESPI/EBI",
  "sourceType": "official_report",
  "supportedMarkets": ["GPW"],
  "fetchMode": "public_page",
  "visibility": "optional",
  "userConfigurable": true,
  "healthStatus": "notRefreshed",
  "accessMode": "public",
  "enabled": true,
  "defaultPollIntervalSeconds": 900,
  "sourceUrl": "https://www.gpw.pl/komunikaty",
  "rateLimitPolicy": "Serialized listing request plus up to 5 matched detail requests per refresh, 2 seconds apart",
  "policyNote": "Uses GPW public-page listing fragments and matched report detail pages for official body text and attachments.",
  "lastSuccessAt": null,
  "lastErrorAt": null,
  "lastError": null,
  "lastItemsFetched": null,
  "lastItemsCreated": null,
  "lastItemsMatched": null,
  "lastItemsUnmatched": null,
  "lastDetailItemsAttempted": null,
  "lastDetailItemsStored": null,
  "lastDetailItemsFailed": null,
  "lastDetailWarning": null
}
```

Commands:

- `list_source_adapters(input?)`: returns adapter metadata/status rows above. `input` is `{ includeDeveloperOnly? }`; normal callers omit it and get only `required`/`optional` adapters, per the `visibility` rule below. Each row carries a `role` (`primary` | `witness`) — a `witness` source reconciles against the primary instead of ingesting into the feed (ADR 0069 D2).
- `set_source_adapter_enabled(input)`: `{ adapterId, enabled }` → toggles a `userConfigurable` adapter and returns its updated metadata row.
- `list_source_reconciliation(input?)` (developer-mode only): recent GPW ESPI/EBI witness ↔ Bankier reconciliation results, newest disclosure first. `input` is `{ limit? }` (default 200). Each `ReconciliationResult` row is `{ id, witnessAdapterId, companyId?, qualifiedTicker?, reportNumber?, reportType?, disclosureDate, witnessTitle, witnessUrl?, status, primaryFeedItemId?, createdAt, updatedAt }` where `status` ∈ `matched | espi_only | bankier_only` (ADR 0069 D2, plan v0.55 T3). An `espi_only` result for a tracked company also raises a system attention event (`triggerType: "source_reconciliation"`); the witness never ingests feed items.

Fetch input:

```json
{
  "since": "2026-05-28T12:00:00Z",
  "watchlistCompanyIds": ["company_gpw_cdr"],
  "manualRefresh": false
}
```

Fetch output:

```json
{
  "items": [],
  "cursor": "opaque-source-cursor",
  "fetchedAt": "2026-05-28T13:00:00Z",
  "warnings": []
}
```

Rules:

- Adapters must preserve source URLs and attribution.
- Adapters must declare and respect source-specific rate limits.
- Restricted scraping is not allowed without a source-specific ADR.
- `sourceType` must distinguish `official_report`, `public_media`, `analysis`, `authenticated_research`, `company_registry`, and future source categories where relevant.
- `visibility` must distinguish `required`, `optional`, and `developer` sources.
- Normal source listing returns only `required` and `optional` sources. Developer tooling may request `developer` sources explicitly.
- `userConfigurable` is true only for implemented optional sources that the user may enable or disable.
- `healthStatus` is a simple UI-facing status: `healthy`, `attention`, `notRefreshed`, or `off`.
- `accessMode` must distinguish `public`, `rss`, `paywalled`, `authenticated`, and `manual` sources where relevant.
- Authenticated private sources require a source-specific ADR before implementation. Portal Analiz is governed by [ADR 0014](adr/0014-portal-analiz-authenticated-source-policy.md).
- Authenticated adapters must use OS keychain secrets or session storage and must not export credentials to YAML, backup files, logs, or test samples.
- `portal-analiz` exists only as a developer-tier candidate until the authenticated-source implementation is explicitly built. It must not be fetched by normal refresh commands.
- The Sources screen reads source status from local runtime state before real fetching exists.
- Normal Sources exposes implemented source status, source URL, refresh controls, and optional enablement. Source IDs, fetch modes, rate-limit policy, source-policy notes, unmatched diagnostics, and unimplemented candidates belong in Developer mode and docs.
- The first GPW implementation parses listing HTML test samples before live network fetch is wired.
- GPW listing ingestion upserts feed items by `(sourceAdapterId, dedupeKey)` and must preserve existing `read` and `saved` user state.
- GPW listing ingestion matches companies by ticker first, then exact ISIN fallback. Matched items populate `feed_item_companies` with `matchType: "ticker"` or `"isin"` and use the matched exchange-qualified ticker as `displayCompany`.
- GPW issuer/company name alone is not an automatic match key. Names may support lookup suggestions and diagnostics, but silent feed matching must use ticker or ISIN.
- The GPW company registry cache provides ticker, ISIN, company name, source metadata, freshness, and last-error state for the complete public GPW company list returned by the GPW companies page. Runtime lookup and ingestion use the local SQLite cache populated from accepted source refresh paths; sample data is test-only and must not seed target runtime databases.
- Unmatched GPW listings may be stored locally with source-derived `displayCompany`, but they must not appear in normal Inbox/company feed views until matched to a tracked company.
- In-app official report body access is required for v1 GPW support.
- GPW detail parsing is the primary M6 path for extracting report body text and attachment links for matched GPW feed items.
- GPW detail fetch-and-parse behavior must use an injectable fetch boundary so tests stay offline and default checks do not depend on live GPW detail pages.
- GPW detail evaluation returns an explicit usability signal plus warnings. Missing title or missing/very short body text makes the parsed detail unusable for normal ingestion.
- GPW detail spike aggregation is conservative: rejected samples trigger parser hardening or fallback-source investigation.
- GPW detail fetch policy is conservative: enabled by default for matched items, at most 5 detail pages per refresh, serialized with at least 2 seconds between detail requests.
- The first M8 public media adapter is `bankier-market-rss`, using `https://www.bankier.pl/rss/gielda.xml` as an RSS feed with `Bankier.pl` attribution.
- `bankier-market-rss` stores items as `Public media`, preserves the RSS item link as the source URL, and does not crawl linked article pages in the initial slice.
- `bankier-firma-rss` and `bankier-wiadomosci-rss` are visible as disabled reviewed public RSS candidates; normal refresh commands must not fetch them until matching-quality tests and runtime enablement are explicitly accepted.
- `bankier-market-rss` normalizes RSS item links before storage and dedupe by stripping tracking query parameters such as `utm_*` and URL fragments.
- Public media adapters may set a nullable `duplicateSignature` for cross-source article/media dedupe. Bankier uses a normalized tracked-company plus title signature so obvious syndicated/copied media items do not create duplicate Inbox rows if another media adapter has already stored the same item.
- `bankier-market-rss` matches only tracked GPW companies by strong ticker/name signals found in the RSS item title or description. Unmatched RSS items remain source diagnostics and must not appear in normal Inbox/company feed views.
- `bankier-company-komunikaty` uses Bankier per-company public komunikaty pages only to resolve Bankier instrument slugs and tag IDs, then fetches one public JSON listing page per tracked GPW company from Bankier's article listing endpoint.
- `bankier-company-komunikaty` stores Bankier instrument slugs and tag IDs in `company_source_ids` so repeat refreshes do not re-fetch company pages unless the identifiers are missing.
- `bankier-company-komunikaty` only processes items from the last 7 days relative to the adapter fetch timestamp; older Bankier listing items are ignored.
- `bankier-company-komunikaty` creates visible Inbox/company feed rows as the active v1 official-report source while `gpw-espi-ebi` is disabled.
- `bankier-company-komunikaty` skips article-page detail fetches for items that already have body text stored locally; refresh preserves existing body text and attachments unless a newly fetched detail body is available.
- Refresh commands must not prune Bankier company feed rows. Cleanup/pruning, if needed, is a separate asynchronous maintenance process.
- `gpw-espi-ebi` remains registered as a disabled candidate for later revisit if Bankier Company Komunikaty proves unreliable.
- Source HTTP requests use the neutral app user agent `LocalInvestorNewsfeed/{version}` and must not impersonate a browser or Bankier UI. Source protection relies on low request volume, tracked-company scope, cached identifiers, and serialized refreshes.

## Feed Item

```json
{
  "id": "feed_01",
  "type": "official_report",
  "sourceAdapterId": "gpw-espi-ebi",
  "sourceName": "GPW ESPI/EBI",
  "sourceUrl": "https://www.gpw.pl/komunikaty",
  "title": "Current report title",
  "summary": null,
  "bodyText": null,
  "language": "pl",
  "publishedAt": "2026-05-28T12:04:52Z",
  "fetchedAt": "2026-05-28T12:15:00Z",
  "companies": ["company_gpw_cdr"],
  "displayCompany": "GPW:CDR",
  "dedupeKey": "gpw-espi-ebi:report:example",
  "read": false,
  "saved": false,
  "attribution": "GPW",
  "attachments": [
    {
      "id": "feed_attachment_01",
      "label": "report.pdf",
      "url": "https://www.gpw.pl/pub/GPW/ESPI/example/report.pdf"
    }
  ],
  "signals": [
    {
      "id": "signal_01",
      "category": "insider_transaction",
      "status": "confirmed",
      "classifiedBy": "rule"
    }
  ]
}
```

Rules (storage rules — dedupe, required fields, retention — are canonical in [Data Model § Feed Items](data-model.md#feed-items)):

- `signals` is the list of typed classifications attached to this filing (see [Company Signal](#company-signal)). It is read-only on the feed read model and empty for unclassified items; the full signal contract is served by the signal endpoints.
- Original source text should be retained when legally and technically allowed.
- `summary` is a separate field from `title` and `bodyText`. If a source or AI-generated summary is not available, UI read models may display `title` as the summary fallback.
- Feed details should show the summary/fallback first and keep the full official report body collapsed until the user expands it.
- Attachments are source links parsed from detail pages and stored separately from body text. Attachment URLs must preserve original source attribution.

UI-facing state mutation input:

```json
{
  "id": "feed_01",
  "read": true,
  "saved": true
}
```

Rules:

- `read` and `saved` are independently optional for partial state updates.
- Updating read/saved state must not alter source attribution, timestamps, matched companies, or dedupe identity.

`update_feed_item_state(input)` applies the mutation input above and returns the updated `FeedItem`.

## Notebook Entry

```json
{
  "id": "note_01",
  "companyId": "company_gpw_cdr",
  "title": "Management claim about release schedule",
  "body": "Management said the next major release milestone should happen in the next two quarters.",
  "bodyFormat": "markdown",
  "tags": ["management-guidance", "product"],
  "kind": "claim",
  "claimStatus": "open",
  "eventDate": "2026-05-28",
  "followUpAfter": "2026-Q4",
  "followUpDate": "2026-11-30",
  "createdAt": "2026-05-28T13:20:00Z",
  "updatedAt": "2026-05-28T13:20:00Z",
  "origins": [
    {
      "sourceType": "feed_item",
      "sourceId": "feed_01",
      "sourceUrl": "https://www.gpw.pl/komunikaty",
      "label": "GPW report"
    }
  ]
}
```

Allowed note kinds:

- `manual`
- `observation`
- `claim`
- `question`
- `follow_up`

Allowed claim statuses:

- `open`
- `delivered`
- `partially_delivered`
- `missed`
- `unknown`
- `not_applicable`

Rules (company ownership, body format, and origin-link requirements are canonical in [Data Model § Notebook Entries](data-model.md#notebook-entries)):

- Notebook read views may render common Markdown, but stored `body` remains the canonical Markdown source.
- Claim notes may include both `followUpAfter` for quarter/period follow-up and `followUpDate` for exact date follow-up.
- Notebook UI surfaces should render origin links in note details and make feed-item origins actionable inside the app when the referenced feed item is still available locally.
- Origin links are immutable through normal note editing. Future workflows may add or detach origins through explicit source-link actions, but inline note editing must not rewrite origin records.
- Feed-to-note drafts start from UI-facing feed items, which are scoped to tracked companies; `create_notebook_entry` requires the canonical tracked `companyId`.
- As of `v0.42.0` ([ADR 0040](adr/0040-management-claims-tracker.md)) management claims are a first-class entity with their own commands (see [Management Claim](#management-claim)); `kind = 'claim'` and the `claimStatus`/`followUp*` fields are **legacy**, migrated by `0045` (detail in Data Model). New claims are created and tracked through the claim commands, not `create_notebook_entry`.

Initial local commands:

- `list_notebook_entries(companyId)`: returns notebook entries for one company, newest updated first.
- `create_notebook_entry(input)`: creates one Markdown notebook entry for a company.
- `delete_notebook_entry(id)`: deletes one notebook entry by id.
- `update_notebook_entry(input)`: updates editable note fields and tags while preserving company ownership and immutable origin links.

Initial create input:

```json
{
  "companyId": "company_gpw_cdr",
  "title": "Management claim about release schedule",
  "body": "Markdown body",
  "bodyFormat": "markdown",
  "tags": ["management-guidance"],
  "kind": "claim",
  "claimStatus": "open",
  "eventDate": "2026-05-28",
  "followUpAfter": "2026-Q4",
  "followUpDate": "2026-11-30",
  "origins": [
    {
      "sourceType": "feed_item",
      "sourceId": "feed_01",
      "sourceUrl": "https://www.gpw.pl/komunikaty",
      "label": "GPW report"
    }
  ]
}
```

Initial update input:

```json
{
  "id": "note_company_gpw_cdr_release_schedule",
  "title": "Updated release schedule claim",
  "body": "Updated Markdown body",
  "tags": ["management-guidance", "clarified"],
  "kind": "claim",
  "claimStatus": "unknown",
  "eventDate": "2026-05-28",
  "followUpAfter": "2026-Q4",
  "followUpDate": "2026-11-30"
}
```

Update rules:

- `companyId`, `bodyFormat`, timestamps, and origin links are not edited by this command.
- Tags are replaced atomically from the submitted tag list.
- Empty optional date/status fields are stored as unset values.

## Management Claim

Management claims are first-class tracked promises with a due period and a user-set verdict ([ADR 0040](adr/0040-management-claims-tracker.md), `v0.42.0`). They are the canonical claim entity (replacing `notebook_entries(kind='claim')`).

```json
{
  "id": "claim_01",
  "companyId": "company_gpw_cdr",
  "statement": "Management expects the next major release in the next two quarters.",
  "body": "Stated on the Q1 earnings call.",
  "bodyFormat": "markdown",
  "madeAt": "2026-05-28",
  "sourcePeriodId": "period_cdr_2026_q1",
  "dueFiscalYear": 2026,
  "duePeriodType": "Q4",
  "status": "pending",
  "sourceEvidenceType": "transcript_segment",
  "sourceEvidenceId": "seg_42",
  "extractionProposalId": "claim_prop_07",
  "targetMetricKey": null,
  "targetComparator": null,
  "targetValueNumeric": null,
  "targetUnit": null,
  "verifyingFactId": null,
  "revisesClaimId": null,
  "createdAt": "2026-05-28T13:20:00Z",
  "updatedAt": "2026-05-28T13:20:00Z"
}
```

Allowed verdict statuses: `pending`, `delivered`, `partially_delivered`, `missed`, `revised`.

Allowed `sourceEvidenceType`: `report_document`, `transcript_segment`, `transcript`, `feed_item`, `manual`.

Allowed `targetComparator` (quantitative claims): `gte`, `lte`, `gt`, `lt`, `approx`, `eq`.

Rules (canonical company ownership is in [Data Model § Management Claims](data-model.md#management-claims)):

- The verdict (`status`) is user-set; there are no automated verdicts.
- A claim with both `dueFiscalYear` and `duePeriodType` is eligible for due-period resurfacing; a claim missing either is user-managed only.
- `verifyingFactId` is the direct link to the confirmed fact that verifies a quantitative claim; `verifyingRelation` (`supports`/`contradicts`) is reserved for a follow-up that registers the link in the evidence graph (deferred — `financial_fact` is not yet an evidence type; see [ADR 0040](adr/0040-management-claims-tracker.md) Decision 5).
- A `revised` verdict requires `revisesClaimId` (or creates a superseding claim that sets it); claim history is retained.

Local commands:

- `list_management_claims(companyId)`: returns claims for one company, newest updated first.
- `create_management_claim(input)`: creates one claim (manual or from a confirmed extraction proposal).
- `update_management_claim(input)`: updates editable fields (statement, body, due period, quantitative target, source links) while preserving company ownership and provenance.
- `set_claim_verdict(input)`: sets `status` and optionally links `verifyingFactId` (with the `supports`/`contradicts` relation) and `revisesClaimId`. Idempotent.
- `delete_management_claim(claimId)`: deletes a claim and its derived reminders/queue rows; retains rejected extraction proposals.
- `list_claims_to_verify(input)`: the review-queue read model — claims bucketed `due` | `overdue` | `upcoming` for a company (or watchlist scope), with the resolved verifying fact attached for quantitative claims (see [Claims Review Queue](#claims-review-queue)).

Error codes: `claim_not_found`, `company_not_found`, `invalid_due_period`, `invalid_verdict`, `fact_not_found` (when linking `verifyingFactId`), `invalid_comparator`.

## Claim Extraction — retired ([ADR 0084](adr/0084-retire-in-app-ai-layer.md))

In-app AI claim extraction is removed; the `claim_extraction_proposals` and `claim_extraction_jobs` tables were dropped by migration `0102` (decision 5 — no readable history survives). The **manual claims path stays** — creating a management claim (see [Management Claims](#management-claims)) is how a claim is recorded now. Agent-proposed claims with mandatory provenance return via MCP write-tools (v0.60.0).

## Claims Review Queue

The due-period resurfacing read model ([ADR 0040](adr/0040-management-claims-tracker.md)). When a report arrives and a `financial_period` is created/linked for a company, a derivation job matches open (`status = pending`) claims whose `dueFiscalYear`/`duePeriodType` equal the arriving period and records them as resurfaced; the queue surfaces them for verification.

`list_claims_to_verify(input)` returns, per company (or watchlist scope):

```json
{
  "due": [{ "claim": { "id": "claim_01" }, "arrivedPeriodId": "period_cdr_2026_q4", "verifyingFactCandidate": { "factId": "fact_99", "valueNumeric": "12500000" } }],
  "overdue": [],
  "upcoming": [{ "claim": { "id": "claim_02" }, "dueFiscalYear": 2027, "duePeriodType": "H1" }]
}
```

Rules:

- `due`: the due-period report has arrived and the claim is still `pending`. `overdue`: the due period has passed (a later period arrived) and the claim is still `pending`. `upcoming`: the due period has not yet arrived.
- For a quantitative claim (`targetMetricKey` set), the queue resolves the matching confirmed `financial_fact` for the arrived period and attaches it as `verifyingFactCandidate`; the user confirms the link and verdict via `set_claim_verdict`.
- Resurfacing is idempotent: re-running the derivation never duplicates queue entries for the same `(claim, period)`.
- The same arrival may also create a `claim_follow_up` reminder; the queue is the primary verification surface, reminders/digests are the cross-cutting paths.

## Report-Season Cockpit

The report-season cockpit ([ADR 0044](adr/0044-report-season-cockpit.md), `v0.43.0`): upcoming report dates across watchlists, each with a pre-report card composed from open questions, unresolved claims, last-period KPIs, and recent evidence, plus a prepare→process workflow. The calendar and card are backend-owned **read models** assembled from canonical domains; the only new persisted state is per-occurrence preparation status (`report_preparations`).

`list_report_season(input)` returns the calendar read model. `input` is `{ watchlistId?: string }` (omit for all tracked companies). It aggregates `company_events` with `eventType = 'periodic_report'`, split into `upcoming` (event date ≥ today) and `past`, ordered by date, with calendar freshness so a stale calendar is visible rather than silently empty:

```json
{
  "upcoming": [
    {
      "companyId": "company_gpw_cdr",
      "qualifiedTicker": "GPW:CDR",
      "displayName": "CD Projekt",
      "eventKey": "cdr-periodic_report-2026-05-28",
      "eventDate": "2026-05-28",
      "eventTime": null,
      "title": "Raport za Q1 2026",
      "preparationStatus": "prepared"
    }
  ],
  "past": [],
  "calendarFreshness": { "lastFetchedAt": "2026-06-16T04:00:00Z", "stale": false }
}
```

`get_pre_report_card(input)` returns the per-company pre-report card. `input` is `{ companyId: string, eventKey: string }`. It composes the four owning domains plus the company's preparation state — no duplicated domain logic:

```json
{
  "companyId": "company_gpw_cdr",
  "eventKey": "cdr-periodic_report-2026-05-28",
  "eventDate": "2026-05-28",
  "preparationStatus": "prepared",
  "linkedReportDocumentId": null,
  "openQuestions": [{ "id": "q_01", "prompt": "Czy marża brutto się utrzyma?" }],
  "unresolvedClaims": { "due": [], "overdue": [], "upcoming": [{ "claim": { "id": "claim_02" } }] },
  "lastPeriodKpis": [{ "metricKey": "revenue", "valueNumeric": "950000000", "periodId": "period_cdr_2025_q4" }],
  "recentEvidence": [{ "evidenceType": "company_signal", "id": "sig_12", "occurredAt": "2026-06-10" }]
}
```

Workflow actions write `report_preparations` and are explicit user actions (no automation):

- `mark_report_prepared(input)`: `{ companyId, eventKey }` → sets `status = 'prepared'`, stamps `preparedAt`. Idempotent.
- `mark_report_processed(input)`: `{ companyId, eventKey, linkedReportDocumentId? }` → sets `status = 'processed'`, stamps `processedAt`, links the arrived report when known. On processing the card links to the arrived filing and the existing KPI-extraction entry point and ties back to the claims-review queue; it never auto-extracts or auto-confirms. Idempotent.

Rules (missing-row default is canonical in [Data Model § Report Preparations](data-model.md#report-preparations)):

- `list_report_season` with a `watchlistId` restricts to that watchlist's companies; unscoped returns all tracked companies.
- Allowed `preparationStatus`: `upcoming`, `prepared`, `processed`, validated at the storage boundary.

Error codes: `company_not_found`, `watchlist_not_found`, `invalid_preparation_status`.

## Decision Journal

The decision journal ([ADR 0071](adr/0071-judgment-capture.md), `v0.52.0`): an early, forward-compatible slice of the [ADR 0043](adr/0043-investment-thesis-and-decision-journal.md) thesis-workbench journal that records the user's own judgments so the calibration record starts accumulating now. Entries are **immutable once saved** (DB `BEFORE UPDATE`/`BEFORE DELETE` triggers, no update/delete command); corrections are appended as follow-up entries. Decision support only — the app mirrors judgments back, never grades them ([ADR 0042](adr/0042-advisory-verdict-port-and-open-core-boundary.md)). Entries join the research timeline (evidence type `decision_entry`, `occurredAt = decidedAt`).

`create_decision_entry(input)` records one judgment and returns the saved entry:

```json
{
  "id": "decision_entry_company_gpw_cdr_1",
  "companyId": "company_gpw_cdr",
  "kind": "buy",
  "rationaleMd": "Durable moat and improving cash generation.",
  "decidedAt": "2026-01-15",
  "supersededByEntryId": null,
  "createdAt": "2026-01-15T09:00:00Z"
}
```

The create `input` is `{ companyId, kind, rationaleMd, decidedAt, supersededByEntryId? }`. `list_decision_entries(input)` returns the journal newest-decided first; its `input` is `{ companyId?, kind? }` — omit `companyId` for the global chronological journal, `kind` to include all kinds.

Allowed `kind`: `buy`, `pass`, `keep_watching`, `sell_note` (recorded actions/judgments, never advice).

Rules (field-level storage rules canonical in [Data Model § Decision Journal Entries](data-model.md#decision-journal-entries)):

- Immutable: there is no update or delete command. A correction sets the follow-up entry's `supersededByEntryId` to the id it supersedes (the superseded entry must exist for the same company).
- `rationaleMd` and `decidedAt` are required; `decidedAt` is a `YYYY-MM-DD` domain date and drives timeline ordering (distinct from `createdAt`).

Typed commands ([ADR 0070](adr/0070-typed-command-error-envelope.md) `CommandError`): `create_decision_entry`, `list_decision_entries`. Failure codes: `invalid_input` (bad `kind`/`decidedAt`, empty rationale), `not_found` (unknown company or superseded entry).

## Report Expectations

Pre-report expectations ([ADR 0071](adr/0071-judgment-capture.md), `v0.52.0`): the user's stance plus optional per-metric expectations written down **before** a report lands, keyed by the same `(companyId, eventKey)` occurrence as the report-season cockpit and resolved at creation to the `(fiscalYear, periodType)` the report covers. Editable until the period's facts are recorded, then **frozen** (hindsight-bias check). Expectation-vs-actual is a composed **read model**, never a stored projection — the app records the user's own verdict, it never scores judgment ([ADR 0042](adr/0042-advisory-verdict-port-and-open-core-boundary.md)).

`create_report_expectation(input)` records the stance + metric rows and returns the expectation:

```json
{
  "id": "report_expectation_company_gpw_cdr_evt-h1-2026",
  "companyId": "company_gpw_cdr",
  "eventKey": "evt-h1-2026",
  "fiscalYear": 2026,
  "periodType": "H1",
  "stanceMd": "Expecting margin recovery on the game launch.",
  "frozenAt": null,
  "resolutionNoteMd": null,
  "resolvedAt": null,
  "createdAt": "2026-02-01T09:00:00Z",
  "updatedAt": "2026-02-01T09:00:00Z",
  "metrics": [
    { "id": "report_expectation_..._metric_1", "expectationId": "report_expectation_...", "metricKey": "net_profit", "comparator": "gte", "expectedValue": "40000000", "unit": null, "createdAt": "2026-02-01T09:00:00Z" }
  ]
}
```

`expectation_review(input)` (`input` = `{ companyId, eventKey }`) returns the expectation composed against the occurrence's confirmed facts:

```json
{
  "companyId": "company_gpw_cdr",
  "eventKey": "evt-h1-2026",
  "fiscalYear": 2026,
  "periodType": "H1",
  "stanceMd": "Expecting margin recovery on the game launch.",
  "frozenAt": "2026-08-30T10:00:00Z",
  "factsAvailable": true,
  "resolutionNoteMd": null,
  "resolvedAt": null,
  "metrics": [
    { "metricKey": "net_profit", "comparator": "gte", "expectedValue": "40000000", "unit": null, "actualValue": "52000000", "outcome": "met" }
  ]
}
```

Allowed `comparator`: `lt`, `lte`, `eq`, `gte`, `gt`. `outcome`: `met`, `missed`, `unknown` (unknown = no confirmed actual or an unparseable value — the evaluator mirrors, never guesses).

Rules (field-level storage rules canonical in [Data Model § Report Expectations](data-model.md#report-expectations)):

- One expectation per `(companyId, eventKey)` occurrence (unique).
- `update_report_expectation(input)` (`{ companyId, eventKey, stanceMd?, metrics? }`) edits the stance and/or replaces the metric set wholesale — but only until the period's facts land. Once any facts exist the update is refused (`conflict`) and `frozenAt` is stamped; the freeze is checked **inside the update transaction**. Reads (`list_report_expectations`, `expectation_review`) also stamp `frozenAt` on first observation (freeze-on-read).
- `factsAvailable` is true once any facts exist for the resolved period; a per-metric `actualValue` is the latest **confirmed** fact for that metric+period (joined via `kpi_definitions.metricKey`), else null with `outcome = "unknown"`.
- `record_expectation_resolution(input)` (`{ companyId, eventKey, resolutionNoteMd }`) records the user's own verdict, stays allowed after the freeze, and stamps `resolvedAt` once.
- `list_report_expectations(input)` (`{ companyId? }`) returns expectations newest-created first (omit `companyId` for all).

Typed commands ([ADR 0070](adr/0070-typed-command-error-envelope.md) `CommandError`): `create_report_expectation`, `update_report_expectation`, `list_report_expectations`, `expectation_review`, `record_expectation_resolution`. Failure codes: `invalid_input` (bad comparator/value, empty stance, non-positive fiscal year), `not_found` (unknown company/occurrence), `conflict` (editing a frozen expectation, duplicate occurrence).

## Short Positions (KNF)

Per-company read model for the KNF short-selling register ([ADR 0069](adr/0069-source-reliability-and-disclosure-signals.md) decision 3, `v0.55`). Read-only: the register is populated by the daily `knf-short-selling` adapter; storage rules are canonical in [Data Model § Short Positions (KNF)](data-model.md#short-positions-knf).

`list_short_positions(input)` (`input` = `{ companyId }`) returns the cockpit view:

```json
{
  "positions": [
    { "holderName": "Qube Research & Technologies Ltd", "netPositionPct": 1.81, "positionDate": "2026-07-10", "recentlyChanged": true }
  ],
  "events": [
    { "kind": "increased", "holderName": "Qube Research & Technologies Ltd", "fromPct": 1.49, "toPct": 1.81, "positionDate": "2026-07-10" }
  ],
  "lastExit": { "holderName": "Point72 Asset Management", "exitedOn": "2024-11-03" },
  "aggregatePct": 2.40,
  "delta30dPp": 0.32,
  "registerUpdatedAt": "2026-07-15T06:30:00Z"
}
```

- `positions` are the active mirror rows (`exited_at IS NULL`), largest first; `recentlyChanged` flags a holder with a register change dated within the last 30 days (the "changed" chip).
- `events` is the change history, newest-first by the **domain** `positionDate` (never `createdAt`), capped at 50; `kind` ∈ `entered | increased | decreased | exited`.
- `registerUpdatedAt` mirrors `source_adapters.last_success_at` for `knf-short-selling` (the attribution line's "aktualizacja"); `null` until the first successful pull.
- `lastExit` is the most recent remembered exit (empty-state "Ostatnia obecność"), or null.
- `aggregatePct` is the sum of active `netPositionPct`. `delta30dPp` is the 30-day change in percentage points, defined as the signed sum of in-window event deltas (entered `+to`, increased/decreased `to−from`, exited `−from`) — equal to `aggregate_now − aggregate_30d_ago` because the ingester writes one event per detected change (a clean "aggregate 30 days ago" is not derivable from the current mirror alone).

Typed command ([ADR 0070](adr/0070-typed-command-error-envelope.md) `CommandError`): `list_short_positions`. A company with no register presence reads back the empty view (`positions: []`, `lastExit: null`, zero aggregate/delta). Surfaced by the palette-only `shortPositions` cockpit panel ([UI IA § Company Cockpit Dashboard Panels](ui-information-architecture.md)).

## Analyst Recommendations

Per-company read model for sell-side analyst recommendations ([ADR 0073](adr/0073-analyst-recommendations-tracking.md), `v0.58`). Attributed third-party opinions — **never advice**: every row carries firm + date inseparably from each number, ratings are stored/displayed verbatim in the source vocabulary, and nothing here feeds scorecards or app-generated analysis. Read-only: the append-only history is populated by the `biznesradar-rekomendacje` adapter; storage rules (natural-key dedupe, direction/prev derivation, `recommendation_change` signal emission) are canonical in [Data Model § Analyst Recommendations](data-model.md#analyst-recommendations).

`get_analyst_recommendations(companyId)` returns the panel view:

```json
{
  "companyId": "company_gpw_rec",
  "entries": [
    {
      "firm": "Noble Securities", "analyst": "Mateusz Chrzanowski",
      "rating": "akumuluj", "ratingPrev": "trzymaj", "direction": "upgrade",
      "targetPrice": "250.00", "targetCurrency": "PLN", "targetPrev": "230.00",
      "priceAtIssue": "232.00", "publishedAt": "2026-06-18T08:40:00",
      "reportUrl": "https://.../noble.pdf",
      "sourceUrl": "https://www.biznesradar.pl/rekomendacje-spolki/REC"
    }
  ],
  "latestTarget": { "firm": "Noble Securities", "targetPrice": "250.00", "targetCurrency": "PLN", "publishedAt": "2026-06-18T08:40:00" },
  "lastRefreshedAt": "2026-07-19T08:12:00Z"
}
```

- `entries` is the full local history, **newest-first by `publishedAt`** (never `createdAt`). `publishedAt` is an unqualified **local wall-clock** ISO (Warsaw, no `Z`) — display it as a plain local date, never convert through UTC. Money/target figures are decimal-exact **TEXT**. Row optionals (`analyst`, `ratingPrev`, `targetPrice`, `targetCurrency`, `targetPrev`, `priceAtIssue`, `reportUrl`) are serialized as `null` when absent (not omitted). `direction` ∈ `upgrade | downgrade | initiate | reiterate`, derived at ingest against the latest prior same-firm entry.
- `latestTarget` (optional, omitted when no entry has a target) is the newest target-carrying entry, for the attributed "vs target" readout beside Price context (`PriceContextSection`) — always shown with firm + date.
- `lastRefreshedAt` (optional, omitted before the adapter has ever run) mirrors `source_adapters.last_success_at` for `biznesradar-rekomendacje`, for the footer's honest refresh line — never faked.

Typed command ([ADR 0070](adr/0070-typed-command-error-envelope.md) `CommandError`): `get_analyst_recommendations`. A company with no ingested recommendations reads back the empty view (`entries: []`, `latestTarget`/`lastRefreshedAt` omitted). Surfaced by the palette-only, opt-in `analystRecommendations` cockpit panel ([UI IA § Company Cockpit Dashboard Panels](ui-information-architecture.md)); each new entry also emits a `recommendation_change` signal (feed/Today/digests/alerts).

## Research Cockpit

The research cockpit ([ADR 0053](adr/0053-dockview-layout-pilot.md)): the dockview docking shell. The only persisted state is **named saved layouts** (`cockpit_layouts`); the panel arrangement itself is live UI state. Decision 3A: layouts live in SQLite (not `localStorage`), with versioned dockview geometry and a safe fallback.

`list_cockpit_layouts()` → the saved layouts, ordered by `ordinal`:

```json
[
  {
    "id": "layout_earnings_season",
    "name": "Earnings season",
    "ordinal": 0,
    "panelsJson": "{\"pinned\":[{\"id\":\"follow:fundamentals\",\"kind\":\"fundamentals\",\"mode\":\"follow\"},{\"id\":\"reportDiff:company_gpw_cdr\",\"kind\":\"reportDiff\",\"mode\":\"pinned\",\"companyId\":\"company_gpw_cdr\"}],\"openGlobals\":[],\"closedLinked\":[\"feed\",\"inspector\",\"claims-sel\",\"diff-sel\"],\"selectedFeedItemId\":\"feed_01\",\"grid\":null,\"cells\":null,\"viewCompanyId\":\"company_gpw_cdr\"}",
    "layoutJson": "{ /* dockview api.toJSON() */ }",
    "dockviewVersion": "6.6.1"
  }
]
```

Workflow actions:

- `save_cockpit_layout(input)`: `{ name, panelsJson, layoutJson, dockviewVersion }` → upserts a layout by `name`, returns the saved row. `name` must be non-empty.
- `rename_cockpit_layout(input)`: `{ layoutId, name }` (`RenameCockpitLayoutInput`) → renames the layout **in place** (id, ordinal, panels/layout JSON untouched), returns the updated row (issue #89). The name must be non-empty and unique: because `save_cockpit_layout` upserts BY NAME, a duplicate rename would silently fuse two layouts on the next save — rejected with `duplicate_cockpit_layout_name`. A rename to the layout's own name is a no-op update. **UI entry point**: the pencil affordance on the saved view's sidebar row (inline rename; Enter commits, Escape/blur cancels).
- `delete_cockpit_layout(layoutId)` → removes the layout by id (idempotent — deleting an absent id is a no-op).

Restore/fallback behavior, source-of-truth split between `panelsJson`/`layoutJson`, and import/export durability are canonical in [Data Model § Research Cockpit Layouts](data-model.md#research-cockpit-layouts).

Error codes: `cockpit_layout_not_found`, `invalid_cockpit_layout_name`, `duplicate_cockpit_layout_name`.

## Report-Over-Report Diff

The report-over-report diff ([ADR 0052](adr/0052-report-over-report-diff.md), `v0.47.0`): a pure-Rust, deterministic section-level diff between two consecutive same-type **financial statements** (consolidated SSF / standalone JSF) of one company. Extraction populates the derived `report_document_sections` index; the diff itself is an on-demand backend **read model**, never stored. No AI command this milestone — the narrative MD&A diff and AI delta summary are deferred ([ADR 0052](adr/0052-report-over-report-diff.md)).

`fetch_report_document(input)` downloads a single report document's file on demand. `input` is `{ reportDocumentId: string }`; returns `{ reportDocumentId, fetched }`. Idempotent — an already-downloaded document is a no-op. Reuses the shared report-document fetch/store path; lets the diff compare a `pending` statement without a full backfill (ADR 0052).

`extract_report_sections(input)` enqueues the async extraction job for one stored financial-statement document. `input` is `{ reportDocumentId: string }`. It offloads PDF text extraction off the UI thread and upserts `report_document_sections`; it is idempotent (skips when `content_hash` + `extractor_version` are unchanged). Progress is observed via the document's `extractionStatus` on the read models below.

`list_report_diff_candidates(input)` returns, for a company, the consecutive same-type financial-statement pairs available to diff. Candidates include not-yet-downloaded (`pending`) statements (fetched on demand via `fetch_report_document` when compared), exclude `metadata_only` and non-statement filing components (supervisory-board/audit reports, signature/data files), and are deduplicated to one representative document per period (Polish preferred over an English duplicate). `input` is `{ companyId: string }`. Each candidate carries both documents' identity, period labels, source format (`pdf` | `xhtml` — ESEF/iXBRL is a first-class second format), statement type (`ssf` | `jsf`), and an `extractionStatus` per side (`extracted` | `extraction_pending` | `no_text_layer` | `extraction_failed`):

```json
{
  "companyId": "company_gpw_cbf",
  "candidates": [
    {
      "statementType": "ssf",
      "older": { "reportDocumentId": "doc_…q3", "periodLabel": "2025 Q3", "extractionStatus": "extracted" },
      "newer": { "reportDocumentId": "doc_…q1", "periodLabel": "2026 Q1", "extractionStatus": "extracted" }
    }
  ]
}
```

`get_report_diff(input)` returns the on-demand section diff read model for a chosen pair. `input` is `{ olderReportDocumentId: string, newerReportDocumentId: string }`. Both documents must be the same company and statement type. Sections are aligned by heading + ordinal (positional consumption — duplicate headings never cross-match; no similarity call — verified during [ADR 0080](adr/0080-retire-embedding-model.md)); each section is classified `unchanged` | `changed` | `only_older` | `only_newer`, and `changed` sections carry a line-level diff with citations (ordinal + offset) into both documents:

```json
{
  "statementType": "ssf",
  "alignedCount": 47,
  "sections": [
    {
      "status": "changed",
      "heading": "skonsolidowane sprawozdanie z sytuacji finansowej",
      "olderOrdinal": 12,
      "newerOrdinal": 12,
      "addedLines": 42,
      "removedLines": 43
    }
  ]
}
```

- The diff is **deterministic**: the same two documents always produce the same diff, and a document diffed against itself yields an all-`unchanged`, zero-delta result (a hard test gate).
- If either side is `extraction_pending` the read model returns `extractionStatus: "extraction_pending"` rather than an empty diff; if either side is `no_text_layer` (scanned) or `extraction_failed` (a caught `pdf-extract` panic) it returns `not_diffable`.

Error codes: `company_not_found`, `report_document_not_found`, `statement_type_mismatch`, `company_mismatch`, `not_a_financial_statement`, `extraction_pending`, `not_diffable`.

## Autonomous Report Pipeline (Autopilot)

The autonomous report pipeline (North Star, `v0.49.0`, [ADR 0055](adr/0055-autonomous-report-pipeline-trust-ladder.md)) closes the loop: a tracked, opted-in company's new periodic report is detected, fetched, extracted, diffed, cross-referenced, and surfaced as a single notification — no manual steps. Orchestration is **chained durable-queue jobs** (`fetch → extract → diff → cross_reference → notify`) stamped with one `autopilot_run` id; each stage reuses the existing service (`fetch_report_document`, the deterministic structured/aggregator-primary fundamentals extraction — [ADR 0086](adr/0086-aggregator-primary-fundamentals.md) — `get_report_diff`, claims/research cross-reference). Detection is **event-driven off source-refresh completion** and runs **only while the app is open**. The global confirm-before-commit default never changes; automation is a per-company opt-in. Decision-support only — the result reports *what changed / to verify*, never buy/sell/hold ([ADR 0042](adr/0042-advisory-verdict-port-and-open-core-boundary.md)).

**Trust ladder (per-company mode).**

`get_company_autopilot(input)` returns a company's mode. `input` is `{ companyId: string }`; returns `{ companyId, mode }` where `mode` is `off` | `assist` | `autopilot` (a company with no setting reads `off`).

`set_company_autopilot(input)` sets the mode. `input` is `{ companyId, mode }`. `off`: nothing automatic. `assist`: auto-fetch + auto-extract on detection. `autopilot`: full loop. Facts are **review-free** ([ADR 0086](adr/0086-aggregator-primary-fundamentals.md) decision 5): in **both** `assist` and `autopilot` every extracted fact lands `confirmed` (cited, reversible) — there is no pending-confirmation queue and no per-fact review to-do. `confirmationState` is a frozen compatibility column; origin lives in the provenance labels. Changing the mode never alters already-produced facts or runs. Error codes: `company_not_found`, `invalid_autopilot_mode`.

**Structured-first extract stage ([ADR 0061](adr/0061-deterministic-fundamentals-data-gathering.md) dec. 3/8/9).** The extract stage runs the deterministic structured pipeline (ESEF/iXBRL) **in both `assist` and `autopilot` modes** — not autopilot-only, and with no AI fallback ([ADR 0084](adr/0084-retire-in-app-ai-layer.md)). A stored **PDF** document, or bare non-iXBRL markup (the positional shape — [ADR 0095](adr/0095-retire-html-positional-tier.md), 2026-08-05), is no longer a deterministic-tier candidate at all ([ADR 0086](adr/0086-aggregator-primary-fundamentals.md) decision 1 — the PDF fact-extraction arm is retired; the positional parser that once read the latter shape is ALSO retired): both return a benign empty result with **no** `fundamentals_extraction_outcomes` row, so such a company's core KPIs arrive instead from the BiznesRadar-primary daily pull. Every emitted fact lands `confirmationState = confirmed` in **both** modes ([ADR 0086](adr/0086-aggregator-primary-fundamentals.md) decision 5, amending ADR 0061's mode ladder): facts are review-free, so acceptance strength (`accepted` / `accepted_via_witness` proved a value vs `accepted_unreviewed` merely uncontradicted) is recorded as provenance (`validationStatus` + `sourceTier` + citation), not as a `pending`/`auto_unreviewed` confirmation state. A `flagged` outcome (a contradiction) emits no structured facts. (The PDF profile-drift arm is retired — [ADR 0086](adr/0086-aggregator-primary-fundamentals.md) decision 1 — so no producer sets `structureChanged`/`driftJson` on `kpiDeltaJson` anymore; the `fundamentals_extraction_outcomes` `driftJson`/`structureChanged` **columns** remain, tolerantly read on legacy rows.)

`list_company_autopilot_modes()` returns every company with an explicit (non-`off`) autopilot mode set — `CompanyAutopilot[]`, each `{ companyId, mode }`. Companies with no row default to `off` and are omitted.

`set_companies_autopilot(input)` ([ADR 0056](adr/0056-per-company-settings-surface.md)) sets the same mode on many companies at once from the master-detail per-company settings surface. `input` is `{ companyIds: string[], mode }`; returns the number of companies updated.

**Runs and review.**

`list_autopilot_runs(input)` returns recent runs for the attention home / review queue. `input` is `{ companyId?: string, notificationState?: "unread" | "read" | "dismissed", limit?: number }`. Each run carries `{ id, companyId, reportDocumentId, trigger, mode, status, stage, summaryText, kpiDeltaJson, reportDiffRef, crossRefsJson, producedFactIds, notificationState, lastError, createdAt, severity, reportDocumentTitle }`. `status` is `pending` | `running` | `succeeded` | `failed` | `partial`; `stage` is the current/last stage reached. `reportDocumentTitle` (nullable, since `v0.60`, [ADR 0087](adr/0087-today-attention-home-v2.md) decision 4) is the processed report's document title, resolved by LEFT JOIN on `report_documents` **at read** — a raw source datum so the Today autopilot row states WHICH report it processed instead of a bare "New report processed."; `null` for a document with no stored title or a legacy run whose document is gone (frontend falls back to the token summary). A `failed`/`partial` run still appears with a summary of how far it got. `severity` ∈ `urgent` | `notable` | `routine` (`AttentionSeverity`, since `v0.60`, [ADR 0087](adr/0087-today-attention-home-v2.md) decision 2) is **computed at read** from `status` by the single backend mapping (`storage::severity`) — `failed`/`partial` → `notable`, otherwise `routine` — never stored; the level → status table lives in [Product Spec § Attention Routing](product-spec.md#severity-taxonomy).

`kpiDeltaJson` (extract stage) and `crossRefsJson` (cross-reference stage) are opaque JSON envelopes, not typed fields — each stage owns its own shape (e.g. `{ extractionAvailable, structured?, tier?, factsProposed, factsAutoConfirmed }` and `{ claimsOverdue, claimsDue, openQuestions }` respectively — the retired PDF profile-drift arm no longer writes `structureChanged`/`driftJson` here, [ADR 0086](adr/0086-aggregator-primary-fundamentals.md) decision 1). `factsProposed`/`factsAutoConfirmed` are honest counts of facts the run actually produced (bug e77a1a2): `factsProposed` is every fact the run emitted this run, `factsAutoConfirmed` is the subset `confirmed` — which, facts being review-free ([ADR 0086](adr/0086-aggregator-primary-fundamentals.md) decision 5), equals `factsProposed` for every emitting run — never inferred from a raw `produced`/`proposed` count that only one tier happened to populate.

`summaryText` is a Rust-composed **typed-token stream** — every fragment is a machine token, never user-visible English prose ([ADR 0084](adr/0084-retire-in-app-ai-layer.md) decision 6). Tokens: `report_processed` (nothing notable), `kpi_confirmed:<confirmed>:<proposed>` (emitted in **both** modes now that facts are review-free, [ADR 0086](adr/0086-aggregator-primary-fundamentals.md) decision 5; renders as "{confirmed} of {proposed} KPI recorded"), `kpi_pending:<proposed>` (**legacy only** — no longer emitted; the renderer still reads existing-DB rows tolerantly as the same recorded phrasing), `kpi_extraction_unavailable:<code>` (code ∈ `quota_exhausted` | `provider_not_configured` | `provider_error` | `no_deterministic_tier` | `pdf_document` — the BY-DESIGN raw-PDF gap, ADR 0086 dec. 1: rendered as "core figures arrive from the aggregator source", never a failure framing, never re-armed | `witness_fallback` — the last is **legacy only**, no longer emitted since the ADR 0086 seam retirement; stored rows still render), `report_diff_available`, `claims_to_verify:<n>`, `research_questions:<n>`, `expectations_to_review`; joined with `"; "`. Counts never bake in pluralization — the frontend declines them. `renderAutopilotSummaryTokens` (`src/screens/Today/autopilotRunSummary.ts`) translates each token via `text()`/`pluralNoun` per the i18n rule ([ui-authoring.md](ui-authoring.md)); a stored summary that is not a clean token stream (legacy English-prose rows in an existing DB) passes through **verbatim** (tolerant read), and the Today card falls back to recomposing a localized sentence from `kpiDeltaJson`/`reportDiffRef`/`crossRefsJson` (`composeAutopilotRunSummary`) so pre-token rows still localize.

`get_autopilot_run(input)` returns one run's full composed result. `input` is `{ runId: string }`.

`set_autopilot_run_notification_state(input)` marks a run's notification `read` or `dismissed` (drives the Today/Pulse "what changed" surface, [ADR 0054](adr/0054-mode-based-thesis-centric-shell.md)). `input` is `{ runId, notificationState }`.

`undo_autopilot_run(input)` reverts exactly the facts a run produced (recorded in `producedFactIds`), reusing the existing fact supersede/reject mechanics. `input` is `{ runId: string }`; returns `{ runId, revertedFactIds }`. Idempotent — undoing an already-undone run is a no-op. Reachable from the Today/Pulse Autopilot run card's **Undo** action (two-step confirm), shown whenever `producedFactIds` is non-empty — facts are review-free ([ADR 0086](adr/0086-aggregator-primary-fundamentals.md) decision 5), so both `assist` and `autopilot` runs commit their facts and Undo is the reversal path for either.

`trigger_autopilot_run(input)` manually starts a run for an already-detected report (re-run / explicit kick), enqueuing the first stage. `input` is `{ companyId, reportDocumentId }`. Subject to the same `(companyId, reportDocumentId)` dedup as automatic detection. Error codes: `company_not_found`, `report_document_not_found`, `autopilot_run_in_progress`.

- Detection is **idempotent**: at most one run per `(companyId, reportDocumentId)`.
- With no AI credentials / extraction capability configured, `assist`/`autopilot` degrade to fetch + diff (deterministic) and flag extraction as unavailable rather than looping — AI cost stays bounded (at most one extraction per detected report, opted-in companies only).

## Company Event

Company events represent dated items the user may want to track across watchlists. Upcoming events are the default attention focus, but historical events are retained for context. They are separate from feed items and notebook entries, but may link to source items or notes later.

```json
{
  "id": "event_01",
  "companyId": "company_gpw_cdr",
  "eventType": "periodic_report",
  "title": "Quarterly report publication",
  "eventDate": "2026-08-29",
  "eventTime": null,
  "status": "scheduled",
  "sourceType": "official_calendar",
  "sourceAdapterId": "gpw-market-events-rss",
  "sourceEventKey": "gpw-market-events-rss:2026-06-01:corporate-actions:DIAG",
  "sourceUrl": "https://www.gpw.pl/market-events-calendar?market_section=RGL&market_category=64&date=2026-06-01",
  "attribution": "GPW",
  "fetchedAt": "2026-05-30T12:00:00Z",
  "manual": false,
  "createdAt": "2026-05-30T12:00:00Z",
  "updatedAt": "2026-05-30T12:00:00Z"
}
```

Initial event types:

- `periodic_report`
- `corporate_action`
- `dividend`
- `ex_dividend` — ex-dividend / cut-off date (`ODCIĘCIE DYWIDENDY`), distinct from `dividend` (record/payment); [ADR 0058](adr/0058-investor-week-calendar.md)
- `ipo_debut` — primary-market debut (`DEBIUT`); [ADR 0058](adr/0058-investor-week-calendar.md)
- `shareholder_meeting`
- `conference_call`
- `investor_conference`
- `market_making`
- `listing_change`
- `other_market_event`
- `custom`

Initial statuses:

- `scheduled`
- `confirmed`
- `tentative`
- `changed`
- `cancelled`
- `completed`

Rules (canonical company ownership, source-identity dedup, and refresh-update behavior are in [Data Model § Company Events](data-model.md#company-events)):

- Event views should be scoped to companies in watchlists by default.
- `gpw-market-events-rss` consumes GPW's official market-events RSS feed at `https://www.gpw.pl/rss-calendar-of-market-events` and creates events only for tracked companies matched by exact ticker.
- `bankier-kalendarium-html` consumes the public Bankier Kalendarium page at `https://www.bankier.pl/gielda/kalendarium` and creates `public_calendar` events for tracked companies matched by exact ticker.
- The Bankier adapter may fetch week-specific calendar pages using Bankier's `navigation_type=week&navigation_start=<unix timestamp>` query parameters when the Events week view needs a week that is not cached locally.
- Hidden/empty Bankier calendar RSS endpoints are not accepted as reliable until direct checks prove stable populated content.

Initial local commands:

- `list_company_events(input)`: returns events ordered by `eventDate`, with optional date-range, company, watchlist, event-type, status, and mode filters.
- `create_company_event(input)`: creates one manual event or a source-backed event record from an accepted adapter boundary.

Initial list input:

```json
{
  "mode": "upcoming",
  "companyId": null,
  "watchlistId": null,
  "eventType": null,
  "status": null,
  "dateFrom": null,
  "dateTo": null
}
```

Initial create input:

```json
{
  "companyId": "company_gpw_cdr",
  "eventType": "periodic_report",
  "title": "Quarterly report publication",
  "eventDate": "2026-08-29",
  "eventTime": null,
  "status": "scheduled",
  "sourceType": "manual",
  "sourceAdapterId": null,
  "sourceEventKey": null,
  "sourceUrl": null,
  "attribution": null,
  "fetchedAt": null
}
```

List rules:

- `mode: "upcoming"` returns events from today onward unless explicit dates are supplied.
- `mode: "historical"` returns events before today unless explicit dates are supplied.
- `mode: "all"` allows combined/historical timeline views.
- Initial source ingestion may use this same create contract through storage internals, but UI manual creation must set `sourceType: "manual"`.

### Investor Week Calendar

Status: planned (v0.67.0, ADR 0058)

The investor week calendar ([ADR 0058](adr/0058-investor-week-calendar.md), `v0.67.0`) extends the Events view with composable, opt-in **layers** over a backend-owned read model — no stored weekly projection (the `list_report_season` pattern).

`list_investor_week(input)` returns the week read model. `input` is `{ weekAnchor: "YYYY-MM-DD", scope: "watchlist" | "market", watchlistId?: string, layers: { macro: boolean, holidays: boolean } }`. It returns working-day columns (Mon–Fri; a weekend column only when populated); each column groups items by layer (`company`, `macro`, `holiday`) with per-layer freshness so a stale layer is visible rather than silently empty. The `company` layer unions tracked `company_events` with, when `scope = "market"`, untracked `market_calendar_events`, deduped by ticker.

Macro (`macro_events`) read/write — manual entry ships in `v0.67.0`; a live macro source is deferred to a follow-up ADR:

- `list_macro_events(input)`: `{ from: "YYYY-MM-DD", to: "YYYY-MM-DD" }` → macro releases in range.
- `create_macro_event` / `update_macro_event` / `delete_macro_event`: user-entered releases (`manual = 1`), with `indicatorKey`, `title`, `country`, `eventDate`, optional `eventTime`/`importance`/`actual`/`forecast`/`previous`.

Holidays (`market_holidays`) read — a curated static dataset, no write contract beyond seed/refresh:

- `list_market_holidays(input)`: `{ from, to, markets?: string[] }` → holidays in range, tolerant of an un-seeded year (empty result, never an error).

The active scope and enabled layers persist via `update_settings` (the pinned-companies pattern).
- Manual events use `sourceType: "manual"` and `manual: true`.
- Manual events are for missing or user-known dates, not corrections to normal source updates.

## Market Data

EOD price context ([ADR 0067](adr/0067-market-data-foundation.md), source selection [ADR 0082](adr/0082-market-data-source-selection.md), `v0.53.0`): the company workspace's price section beside fundamentals. Quotes come from the `market_data` adapter (`yahoo-eod`; the `twelvedata-eod` fallback was removed 2026-07-14 — GPW is paid-plan-only there, ADR 0082 amendment; free degraded fallback: card `ee81afe`) into `daily_quotes`; level-0 ratios are canonical derived metrics ([ADR 0046](adr/0046-quality-frameworks-quantitative.md)) evaluated over the latest close × confirmed facts. Decision support only — no cheap/expensive or buy/sell language.

`get_price_context(companyId)` returns the read model (heavy work off the UI thread via `spawn_blocking`):

```json
{
  "lastClose": 231.0,
  "lastDate": "2026-07-13",
  "changeAbs": -2.0,
  "changePct": -0.86,
  "currency": "PLN",
  "week52High": 260.0,
  "week52Low": 180.0,
  "week52HighDistPct": -11.15,
  "week52LowDistPct": 28.33,
  "marketCap": 23100000000.0,
  "ratios": { "pe": 23.1, "pbv": 3.2, "evEbitda": 11.8, "divYield": 0.17, "fcfYield": 3.03, "ownHistPercentile": 0.62 },
  "history": [{ "date": "2026-07-10", "open": 231.0, "high": 235.0, "low": 230.0, "close": 233.0 }],
  "fetchedAt": "2026-07-13T18:00:00Z",
  "emptyReason": null
}
```

Percent fields are scaled to points (`changePct`, `week52*DistPct`, `divYield`, `fcfYield`); `ownHistPercentile` stays a raw `[0,1]` fraction and is `null` until the trailing window has ≥20 sessions of history (a percentile over a handful of bars is noise, not context). Any ratio is `null` when its input facts are absent (renders `—`, never a crash). `emptyReason` is `"no_quotes"` (mapped ticker, no bars yet) or `"unmapped_ticker"` (no `market_data` mapping — non-GPW in v1); when set, price fields are omitted and the UI shows an empty state linking to source health. `history` carries the full OHLC per session (`daily_quotes` stores all four NOT NULL) — the UI renders it as candlesticks. History backfills automatically on company add (`quote_backfill` job); the post-session `quote_daily_pull` appends one bar per session day with a witness cross-check.

Typed command ([ADR 0070](adr/0070-typed-command-error-envelope.md) `CommandError`): `get_price_context`. Failure codes: `not_found` (unknown company).

## Cross-Company Comparison + Comparative Valuation (L1)

Comparison read model + level-1 comparative valuation ([ADR 0089](adr/0089-cross-company-comparison-and-valuation-l1.md)). All reads are typed commands off the UI thread; every value carries its evidence (fact id + provenance `validation_status`). Decision support only — no cheap/expensive or buy/sell language. The comparison read model + its FX layer, sector percentiles, and the valuation commands are all **live** (`v0.61.0`).

### `get_kpi_comparison` (live, v0.61.0)

`get_kpi_comparison(input)` — `input = { companyIds: string[], metricKeys: string[], granularity: "annual" | "quarterly" }` → an aligned period axis plus one series per `(company, metric)`. `companyIds` of length 1 serves the Fundamentals periods×deltas table (same read model, N=1). Offloaded via `spawn_blocking`; reads **confirmed** facts only, never re-parses reports.

- **Axis** — the ascending union of `(fiscalYear, periodType)` coordinates where any requested `(company, metric)` has a canonical fact matching the granularity (`annual` → `FY`; `quarterly` → `Q1`–`Q4`). Each entry: `{ fiscalYear, periodType, key }` (`key` = `"<year>:<periodType>"`).
- **Series** — `{ companyId, metricKey, valueKind, cells }`, cells aligned 1:1 with the axis. `valueKind` is the catalog `value_kind` (or `null` for an unknown metric).
- **Canonical fact per slot** — one fact per `(company, metric, period)`, selected by the same preference as `quality_frameworks::load_period_facts` (`data_quality=final` › `variant=reported` › `statement_basis=consolidated` › `attribution=total`), so the comparison and the derived-metrics engine never disagree about "the" value.
- **Cell** — `{ fiscalYear, periodType, factId, value, currency, valuePln, fxBasis, validationStatus, deltaQoQ, deltaYoY, flags }`. `value`/`valuePln` are decimal-exact TEXT (native + PLN); `fxBasis` is `period_average` | `latest_on_or_before` | `native_pln`; `validationStatus` is the provenance evidence link. A gap carries `flags: ["no_fact"]` with every value field `null` (never `0`, never a PLN guess).
- **FX** (per cell, ADR 0089 dec. 2) — a PLN fact passes through (`valuePln = value`, basis `native_pln`); a non-PLN ISO fact converts at the basis its `measure_window` selects (flow → period-average mid, stock → last mid ≤ period end); a **missing rate** flags `fx_missing` (native value still returned, `valuePln` null); a **NULL or non-ISO-4217-looking currency** (e.g. the `currency='shares'` EPS data bug) flags `currency_unknown` **only for monetary/per-share kinds**. Ratios/percentages are never converted and carry no currency, so a NULL currency on a ratio/percentage cell is correct and is **not** flagged `currency_unknown` (ADR 0089 dec. 8; card b875e69) — its native `value` is the comparable number.
- **Deltas** (server-side) — `deltaQoQ`/`deltaYoY` are decimal-exact TEXT. **Monetary** value kinds: percent change vs the same period-type prior year (YoY) / prior sequential quarter (QoQ), rounded to 2 dp. **Ratio/percentage** value kinds: a **p.p.** delta (plain difference). `deltaQoQ` is always `null` for `annual` granularity (there is no quarter-over-quarter). No prior period (first period, or a `no_fact` gap in the prior slot) → the delta is `null` with no flag.
- **`delta_undefined` rule** — a percent change is emitted as `null` **and** the cell carries `delta_yoy_undefined` / `delta_qoq_undefined` when the prior value is **`<= 0`** (zero or negative base) or the sign flips **positive→negative**: a percentage across a sign change or from a non-positive base is misleading, so an honest typed absence is emitted instead of a fabricated number. (A fall to exactly zero from a positive base is a defined `-100%`, not undefined; p.p. deltas are never undefined.)

### `get_sector_percentiles` (live, v0.61.0)

`get_sector_percentiles(companyId)` — where one company stands against its **tracked** sector peers. The peer set is derived at read time from `companies.sector` (registry- or manually-classified), the company itself included; no new tables, no stored projection. Offloaded via `spawn_blocking`; every value is confirmed/validated data (the level-0 market ratios reuse the `get_price_context` path — **no ratio-formula re-derivation** — and the canonical KPIs come from the derived-metrics resolver over confirmed facts). Decision support only.

- **Peer set** — tracked companies whose `sector` folds (case-insensitive) to the company's. `peerCount` (the set size, company included) is **always** returned; `thin: true` when `peerCount < 4` (GPW-honest — most GPW sectors never reach four tracked names). A company with **no sector** returns `emptyReason: "no_sector"` (`peerCount: 0`, `thin: true`, `metrics: []`) — never a silent absence.
- **Metrics** — `{ metricKey, kind, value, percentile, median, sampleSize, absentReason }`. `kind` is `market_ratio` (P/E, P/BV, EV/EBITDA, dividend yield, FCF yield) or `canonical_kpi` (`roe`, `roa`, `roic`, `fcf_margin`, `net_debt_to_ebitda`). `value`/`percentile`/`median` are decimal-exact TEXT.
- **Percentile method** — the **rank-based inclusive (mid-rank) percentile**, over the companies with a **defined** value for that metric (the company + its defined peers): `percentile = (2·L + E) / (2·N) · 100`, where `N` = defined count (company included), `L` = count strictly below the company's value, `E` = count equal to it (≥ 1). Order-independent, ties resolved by mid-rank, always in `(0, 100]`, rounded to 2 dp. `median` is the same defined set's median; `sampleSize` is `N`.
- **Typed absence** (total function, never `0`/`NaN`) — a metric the company has no value for → `absentReason: "no_company_value"` (`value`/`percentile`/`median` null); fewer than **2** defined peer values → `absentReason: "insufficient_peers"` (the company's `value` is still surfaced, `percentile` null).

### `compute_comparative_valuation` (live, v0.61.0)

`compute_comparative_valuation(companyId)` — the level-1 comparative valuation (ADR 0089 dec. 4): peer-multiple implied fair-value ranges, a method-convergence spread, and a deterministic confidence grade. Offloaded via `spawn_blocking`; peer multiples reuse the `get_price_context` path (**no ratio-formula re-derivation**), target drivers come from the derived-metrics resolver over **confirmed** facts. Decision support only — ranges, percentiles, and a grade; never buy/sell/hold language.

- **Methods** — three per-share methods, always all three rows in order: `pe_multiple` (peer median P/E × TTM net profit), `ev_ebitda_multiple` (peer median EV/EBITDA × TTM EBITDA, less **net debt** — the equity bridge — floored at 0), `pbv_multiple` (peer median P/BV × book equity). Each `/ shares_outstanding`.
- **Peer set** — tracked companies whose `sector` folds (case-insensitive) to the company's; `peerCount` is that set (company included), `thin: true` when `peerCount < 4`. A method's peer multiples **exclude the target itself** (self-valuation is circular) and need **≥ 2** other peers with a defined multiple.
- **Range rule** — per method, `fairLow`/`fairBase`/`fairHigh` are driven by the peer-multiple dispersion: `low` = peers' **25th** percentile multiple, `base` = **median** (50th), `high` = **75th**, each carried through the driver (with the EV→equity bridge for EV/EBITDA). Percentiles use **linear interpolation** between the two nearest order statistics (numpy "linear" / PERCENTILE.INC): `h = (N−1)·p`, `value = sorted[⌊h⌋] + (h−⌊h⌋)·(sorted[⌈h⌉]−sorted[⌊h⌋])`. Ordered multiples × positive driver ⇒ `low ≤ base ≤ high`. Fair values are decimal-exact TEXT (4 dp).
- **Method result** — `{ method, driverKey, driverValue, peerMultipleLow/Base/High, fairLow/Base/High, peerSampleSize, absentReason }`. Typed absence (never NaN/0): `no_driver` (a required driver — the primary, `shares_outstanding`, or **net_debt** for EV/EBITDA — is missing), `non_positive_driver` (driver ≤ 0, e.g. a loss on P/E), `insufficient_peers` (< 2 defined peer multiples). A company with **no sector** returns `emptyReason: "no_sector"` (every method `insufficient_peers`).
- **Convergence** — `{ baseLow, baseHigh, spreadPct, methodCount }` over the methods that produced a base value (≥ 2); `spreadPct = (baseHigh − baseLow) / median_of_bases · 100` (2 dp).
- **Confidence grade** — `{ grade, composite, dataCompleteness, peerDepth, methodConvergence, validation }`, each component a `0..1` decimal TEXT (inspectable). `dataCompleteness` = methods computed ÷ 3; `peerDepth` = `min(1, peerCount / 4)`; `methodConvergence` = `(100 − spreadPct)/100` when ≥ 2 methods else 0; `validation` = share of the five driver facts (shares, net profit, equity, EBITDA, net debt) resolving from confirmed data. `composite = 0.30·completeness + 0.25·peerDepth + 0.25·convergence + 0.20·validation`; **grade** `A` ≥ 0.85, `B` ≥ 0.65, `C` ≥ 0.40, else `D`.
- **Persistence** — appends a `valuation_runs` row per method that produced a range, **only when its input signature differs** from that method's latest stored run (no append-per-render). The **signature** is the canonical `inputs_json` string — a deterministic serialization of `{ method, driverKey, driverValue, peerMultipleLow/Base/High, peerSampleSize, dataAsOf }`. `dataAsOf` is the domain as-of date (the target's latest quote date, or the latest fundamentals period end when no quote resolves).

### `list_valuation_runs` (live, v0.61.0)

`list_valuation_runs(companyId)` — the append-only `valuation_runs` history for one company (what-changed diffs; the v0.62 DCF engine writes the same table). Offloaded via `spawn_blocking`; ordered **newest-first by the domain `dataAsOf` date** (never `createdAt`), `createdAt` tie-breaking within an as-of date. Each row: `{ id, companyId, method, inputsJson, fairLow, fairBase, fairHigh, dataAsOf, confidenceGrade, createdAt }`.

**MCP** (ADR 0088): `list_valuation_runs` joins the **read** tier; `compute_comparative_valuation` is an **act**-tier tool (it persists a run — gated by `mcpWritesEnabled`, mirroring `evaluate_framework`, provenance `None`). Both are classified in the registry and the frozen tools/list snapshot is updated.

## Ownership

Shareholder-structure read model + review commands ([ADR 0072](adr/0072-ownership-structure.md), `v0.56`): the "Akcjonariat" section of the Basic Info panel. A computed read model over the append-only ownership store (no stored projection); storage rules are canonical in [Data Model § Ownership](data-model.md). Decision support only.

`get_ownership_overview(companyId)` returns everything the section renders (heavy work off the UI thread via `spawn_blocking`):

```json
{
  "companyId": "company_gpw_cbf",
  "asOf": "2025-12-31",
  "source": "report_document",
  "freeFloatPct": "46.8",
  "disclosedSum": "53.2",
  "holders": [
    { "holderKey": "JACEK DUCH", "name": "Jacek Duch", "holderType": "founder_insider", "capitalPct": "25.5", "votesPct": "25.5", "asOf": "2025-12-31", "source": "report_document", "skinInTheGame": { "person": "Jacek Duch" } }
  ],
  "history": [
    { "holderKey": "JACEK DUCH", "name": "Jacek Duch", "holderType": "founder_insider", "points": [ { "asOf": "2024-12-31", "capitalPct": "24.0", "source": "espi_filing" }, { "asOf": "2025-12-31", "capitalPct": "25.5", "source": "report_document" } ] }
  ],
  "residuals": [
    { "reportDocumentId": "doc_…", "parseState": "glyph_encoded", "detectedAsOf": "2023-12-31", "matchedHeading": "Akcjonariat" }
  ]
}
```

- Percentages are **decimal-exact TEXT** (the `financial_facts.value_numeric` convention), not floats. `freeFloatPct` is always present (`"100"` when nothing disclosed); `disclosedSum` is Σ of disclosed `capitalPct` across current holders; `asOf`/`source` are omitted when there are no stakes yet. `holderType`/`capitalPct`/`votesPct` are omitted when absent (never a fabricated value).
- `holders` is the current state (latest disclosed stake per holder); `history` is each current holder's chronological capital-% trajectory — each point carrying the `source` that disclosed it, deduped per `asOf` to the latest disclosure for that date. An `espi_filing` point is a **threshold crossing** (Polish law compels that filing only when a holder crosses a statutory band), which is what the stake-over-time chart marks per [ADR 0072](adr/0072-ownership-structure.md) decision 5; `report_document` / `aggregator` / `manual` points are ordinary samples. `residuals` are documents whose shareholders table the deterministic parser could not read — flagged, never guessed (the OCR/AI follow-up and holder-type proposal flows are retired, [ADR 0084](adr/0084-retire-in-app-ai-layer.md)).
- `skinInTheGame` (v0.57, ADR 0083 D6) is present on a holder corroborated by a parsed management-holdings row or an insider transaction — by exact person name, or as the `via` vehicle a founder holds through (`{ person, via? }`). It drives the Ownership "skin in the game" badge; omitted when there is no management/insider match. Founder-name stamping (`founder_insider`) and this corroboration are joined by canonical holder identity — never a shared surname.

Mutations return the **freshly recomputed** `OwnershipOverview` so the UI updates in one round-trip:

- `set_ownership_holder_type(companyId, holderKey, holderType)` — manual re-type across the holder's rows (`holderType` `null` clears it; a manual label is authoritative, never overwritten by automation).

The AI holder-type and OCR shareholder-table **proposal surfaces are gone
entirely** ([ADR 0084](adr/0084-retire-in-app-ai-layer.md) decision 5, clean
cut): migration 0102 drops the holder-type-proposal and OCR-proposal tables, so
the confirm/reject pairs and the overview's `pendingProposals` / `ocrProposals`
fields go with them.
Holder types stay **user-editable** through `set_ownership_holder_type`, and a
document no deterministic parser can read stays an honest residual — flagged,
never OCR-guessed.

`backfill_ownership_extraction(companyId)` force-enqueues deterministic extraction across the company's fetched periodic reports (the "Wydobądź z raportów" CTA) and returns the number of documents queued (extraction drains on the autopilot lane). AI holder-type classification and the tier-4 OCR **generation** passes (`run_ownership_classification`, `run_ownership_ocr_extraction`, `run_company_ownership_ocr`) are retired with the in-app AI layer ([ADR 0084](adr/0084-retire-in-app-ai-layer.md) decisions 1/4): residuals a deterministic tier cannot read are flagged, never OCR-guessed. Holder types stay user-editable.

Typed commands ([ADR 0070](adr/0070-typed-command-error-envelope.md) `CommandError`): `get_ownership_overview`, `backfill_ownership_extraction`, `set_ownership_holder_type`. Surfaced by the Ownership section of the Basic Info panel ([UI IA § Company Cockpit Dashboard Panels](ui-information-architecture.md)).

## Company Health

Deterministic health scores + insider overview + red flags ([ADR 0083](adr/0083-company-health-scores-and-red-flags.md)). All read models are computed (no stored projections); heavy work off the UI thread via `spawn_blocking`. Storage rules canonical in [Data Model § Company Health](data-model.md#company-health-insider-substrate--red-flag-acks). Decision support only — published-formula citations, never verdict language.

### Health scores (v0.57.0)

`get_company_health(companyId)` returns `CompanyHealth` — deterministic Piotroski F (2000) + Altman Z″ EM (1995) computed over confirmed FY facts (ADR 0083 Decisions 2–4):

- `companyId`, `statementType`, and the always-visible `piotroskiVariant` / `altmanVariant` citation labels.
- `latest` (the latest-FY entry, absent when the company has no annual period) and `history` (every FY period, newest-first, including `latest`).
- Each entry (`HealthPeriodScores`) carries `periodId`, `fiscalYear`, and two tagged scores keyed by `state`:
  - `piotroski`: `{ state: "headline", score (0–9), signals[9] }` · `{ state: "insufficient_data", signals, missing[] }` · `{ state: "not_applicable", reason }`. Each `PiotroskiSignal` = `code` (`F1`..`F9`), `name`, `passed`, `points`, and the `inputs` (`{ key, value }`) it measured.
  - `altman`: `{ state: "headline", zScore, band, components[4] }` · `{ state: "insufficient_data", components, missing[] }` · `{ state: "not_applicable", reason }`. `band` ∈ `safe | grey | distress` (>2.6 / 1.1–2.6 / <1.1). Each `AltmanComponent` = `code` (`X1`..`X4`), `name`, `weight`, `ratio`, `contribution`, `inputs`.

Strict completeness (Decision 3): a headline renders only when **every** input is present; a missing balance-sheet input (e.g. `current_liabilities`) is `insufficient_data` (never read as zero); financials (non-`industrial` `statementType`) are `not_applicable`; a company with no prior FY is `insufficient_data (prior_fy_period)`. Never a partial or rescaled headline. F5 leverage uses **total non-current liabilities** (`total_liabilities − current_liabilities`, ADR 0083 D4 amendment) — `long_term_debt` stays extracted but is no longer a score input; the signal's `leverage_input` detail records the basis.

Score scalars `piotroski_f` / `altman_z` (the latest-FY headlines only) are additionally referenceable in scorecard criterion expressions (ADR 0046 amendment in ADR 0083 §2); a non-headline latest FY resolves the scalar `unavailable`.

`backfill_company_health_facts(companyId)` (**headless-only — no UI**, ADR 0083 D4 amendment (d)) force-re-extracts the deterministic ESEF facts over a company's stored packages so the v0.57 health concepts persist as confirmed facts (stored facts predate the concept-map extension; the history sweep only re-attacks zero-fact periods). Additive + idempotent: an existing slot is re-observed, a divergent value surfaced, never overwritten. Returns `{ documentsProcessed, documentsSkippedNoPeriod, factsCreated, factsReobserved, divergences }`. Invoked for the T9 live pass and the T3 validation-gate harness.

### Red flags (v0.57.0)

`RedFlagsView` is a computed read model (no stored projection); both commands are async / `spawn_blocking`. Surfaced by the `redFlags` cockpit panel ([UI IA § Company Cockpit Dashboard Panels](ui-information-architecture.md)).

- `get_red_flags(companyId)` → `RedFlagsView` `{ active: RedFlag[], history: RedFlag[] }`. Each `RedFlag` = `flagId` (deterministic `rf:<type>:<company>:<evidence>`), `flagType` (`auditor_red_flag | report_delay | fund_exit | score_deterioration | short_spike`), `severity` (`high | medium`, ADR 0083 D8 static map), `title`, `raisedDate`, `evidenceUrl?`, `evidenceFeedItemId?`, `ackedAt?` (history only). `active` is highest-severity-first; `history` is newest-ack-first.
- `acknowledge_red_flag(input)`: `{ flagId }` — moves a flag from `active` to `history`; idempotent; the same evidence never re-raises (and its signal, already written, never re-fires). Returns the refreshed `RedFlagsView`.

Three flag types are **detected + raised** at the producing seams (ownership ingest → `fund_exit`; fact-confirmation recompute → `score_deterioration`; refresh completion → `report_delay`) via the KNF pattern — one synthetic `feed_items` row + one `confirmed` `company_signals` row in the empty-pattern category, so existing `signal_category` alert rules fire (Attention Routing below, no new alert commands). `auditor_red_flag` and `short_spike` are **composed at read** from the existing `auditor_opinion` signal and the KNF short-position view (which already alert), raising nothing new; `short_spike` fires above `delta_30d_pp > 0.5` pp.

### Insider overview (v0.57.0)

`get_insider_overview(companyId)` returns `InsiderOverview` — the parsed insider substrate folded into a computed read model (ADR 0083 Decision 7); async / `spawn_blocking`. Surfaced by the "Insiderzy" block of the Ownership area ([UI IA § Company Cockpit Dashboard Panels](ui-information-architecture.md)). Decision support only — counts, volumes, and who; never verdict language.

- `companyId`, `transactions` (the timeline, newest effective-date first), `holdings` (latest disclosure per management/supervisory person, newest first), and `window90d` / `window12m` (the rolling aggregates).
- Each `InsiderTransactionEntry` = `id`, `person`, `role?` (`management | supervisory | closely_associated`), `relatedPdmr?`, `direction?` (`buy | sell | other`), `instrument?`, `volume?` / `price?` / `currency?`, `txDate?`, `effectiveDate?` (the date used for windowing — `txDate`, else the filing signal date), `dateSource` (`transaction | filing | unknown`), `feedItemId`, `sourceUrl?`. Figure fields are nullable and never fabricated (the cover note omits volume/price/date for most filings; T4b fills them from the attachment PDF).
- Each `ManagementHoldingEntry` = `person`, `role?`, `shares?` (nullable — an explicit `"0"` is a real zero, `null` is stated-but-unreadable or a `-`/`nd.` cell, never coerced), `indirectVia?` (the vehicle a founder holds through), `asOf`.
- `WindowAggregate` is a tagged union keyed by `state` so an aggregate can never render below the 2-transaction minimum: `{ state: "belowMinimum", count }` (< 2 in-window transactions — the timeline still shows them, no aggregate) · `{ state: "computed", count, buys, sells, undetermined, net, buyVolume?, sellVolume?, volumeKnown, volumeTotal }`. `net = buys − sells`; directionless transactions (`direction` NULL or `other`) count only in `undetermined`, never in the net. `buyVolume` / `sellVolume` sum only the in-window transactions with a known volume (null when none did); `volumeKnown` / `volumeTotal` are the coverage note. Until T4b (ADR 0083 D6 amendment), the aggregate is count-based net direction with volume-where-known, labeled honestly.

**Window inclusivity rule**: a transaction is in a window when its `effectiveDate` is on or after the window's lower bound **and** on or before the read date — **both boundaries inclusive**. So a transaction dated exactly 90 days (resp. 12 months) before the read date is IN the window; 91 days is out. A transaction with no effective date is listed in the timeline but excluded from every window.

## Company Signal

Company signals are typed classifications of official ESPI/EBI filings. A signal is the canonical output of classification, separate from the raw feed item and from calendar events. See [ADR 0034](adr/0034-espi-event-classification.md) and [data-model.md](data-model.md) (Company Signal Model).

```json
{
  "id": "signal_01",
  "companyId": "company_gpw_cdr",
  "feedItemId": "feed_01",
  "category": "insider_transaction",
  "confidence": 0.98,
  "classifiedBy": "rule",
  "status": "confirmed",
  "signalDate": "2026-05-28",
  "providerId": null,
  "modelId": null,
  "derivedEventId": null,
  "createdAt": "2026-05-28T12:15:00Z",
  "updatedAt": "2026-05-28T12:15:00Z"
}
```

Signal categories (seeded registry, extensible as data):

- `insider_transaction` (MAR Art. 19)
- `dividend`
- `profit_warning`
- `significant_contract`
- `own_shares`
- `guidance_change`
- `general_meeting` (carries a meeting date)
- `auditor_opinion` (auditor red flags: qualified opinion / disclaimer / negative opinion / going-concern emphasis)
- `other`

Statuses:

- `confirmed`
- `proposed`

Field/storage rules (company ownership, classification/confirmation states, provenance, derived-event identity) are canonical in [Data Model § Company Signals](data-model.md#company-signals).

Initial local commands:

- `list_company_signals(input)`: returns signals filtered by company, watchlist, category, and status.
- `confirm_company_signal(input)`: confirms a `proposed` (AI) signal, transitioning it to `confirmed`. For `dividend` and `general_meeting` signals this also derives a `proposed` calendar event when a future date can be extracted from the filing body (`v0.41.0`, see below and [ADR 0036](adr/0036-report-document-storage-and-backfill.md)); other categories transition status and persist provenance only.
- `reject_company_signal(input)`: discards a `proposed` signal without creating a signal/event.
- `confirm_derived_event(input)`: confirms a `proposed` derived calendar event (from a dividend/general-meeting signal) onto the calendar, or rejects it; takes `{ eventId, action: "confirm" | "reject" }`. A guessed-date event is never auto-confirmed (`v0.41.0`, [ADR 0036](adr/0036-report-document-storage-and-backfill.md)).
- `list_unclassified_filings(input)`: the explicit **unclassified** bucket — official-report feed items with no `company_signals` row (the rule classifier could not place them; absence is the definition, never a flag). Takes `{ companyId?, limit? }` (default 50, max 200), newest first. Materializes `UnclassifiedFiling { feedItemId, companyId, title, bodyText, signalDate }`. Headless/MCP-first (`v0.60.0`, [ADR 0088](adr/0088-mcp-surface-v2-ui-parity.md) decision 4); no UI entry point yet (Today/Inbox surfacing is future scope).
- `classify_filing(input)`: agent-driven classification of one unclassified official filing into a `confirmed` signal. Takes `{ feedItemId, category }`; the mandatory `feedItemId` is the signal's evidence anchor. Validates `category` against the seeded taxonomy, that the item is an official filing matched to a company, and that no signal already exists (a second attempt is a `conflict`); unknown category / non-official item are `invalid_input`. The created signal carries **honest provenance** — `classified_by = agent` (never `rule`; the CHECK set is extended to `rule | ai | agent` by migration `0113`), `status = confirmed`, `confidence = 1.0`. Headless/MCP-first (`v0.60.0`, [ADR 0088](adr/0088-mcp-surface-v2-ui-parity.md) decision 4).

The opt-in **AI fallbacks** (`run_ai_signal_classification`, `run_ai_event_derivation`) are retired ([ADR 0084](adr/0084-retire-in-app-ai-layer.md) decision 2). The deterministic ESPI rule classifier and `signal_dates` date parsing stay; filings they cannot classify land in an explicit **unclassified** bucket, surfaced by `list_unclassified_filings` and resolved by `classify_filing` ([ADR 0088](adr/0088-mcp-surface-v2-ui-parity.md) decision 4), never guessed.

Confirm/reject input:

```json
{
  "id": "signal_01"
}
```

Rules:

- Confirming or rejecting applies only to `proposed` signals; `confirmed` signals are terminal except for future reversal flows.
- Confirmation must persist provider provenance and create at most one derived event, idempotently.
- Derived-event date extraction (`v0.41.0`, dividend/general-meeting only) is **deterministic-only** over the fetched filing body (`signal_dates`) — the AI fallback is retired ([ADR 0084](adr/0084-retire-in-app-ai-layer.md) decision 2). The derived event is created `proposed` and requires `confirm_derived_event` before it appears on the calendar — a guessed-date event is never created. See [ADR 0036](adr/0036-report-document-storage-and-backfill.md).

## Attention Routing (Alert Rules + Events)

User-owned alert rules and the attention events their evaluation fires ([ADR 0068](adr/0068-attention-routing-and-morning-briefing.md), `v0.54.0`). Rule evaluation is inline in the evidence-producing jobs (T2); these commands are the CRUD + review surface (T3). Field/storage rules (trigger columns, scope resolution, dedup, per-rule daily throttle) are canonical in [Data Model § Attention Routing](data-model.md#attention-routing-alert-rules--events).

Alert rule (`AlertRule`, returned by the rule commands):

```json
{
  "id": "alert_rule_0001",
  "triggerType": "signal_category",
  "signalCategory": "profit_warning",
  "priceMin": null,
  "priceMax": null,
  "scopeType": "watchlist",
  "scopeRef": "watchlist_core",
  "enabled": true,
  "createdAt": "2026-07-15T10:00:00Z",
  "updatedAt": "2026-07-15T10:00:00Z"
}
```

Trigger types: `signal_category` (needs `signalCategory`, a signal-category key) · `autopilot_run_completed` · `price_enters_range` (needs `priceMin ≤ priceMax`, inclusive close band) · `price_week52_low`. Scope types: `company` (`scopeRef` = `companyId`) · `watchlist` (`scopeRef` = `watchlistId`, fires for every member).

Attention event (`AttentionEvent`, returned by the event commands):

```json
{
  "id": "attn_alert_rule_0001_autopilot_run_run_42",
  "ruleId": "alert_rule_0001",
  "triggerType": "autopilot_run_completed",
  "companyId": "company_gpw_cdr",
  "evidenceType": "autopilot_run",
  "evidenceRef": "run_42",
  "firedAt": "2026-07-15T00:00:00Z",
  "seen": false,
  "dismissed": false,
  "severity": "notable",
  "evidenceTitle": "Skonsolidowany raport kwartalny Q2 2026",
  "evidenceDetail": "succeeded",
  "witnessUrl": null
}
```

`witnessUrl` (nullable, [ADR 0097](adr/0097-toasts-are-action-feedback-only.md) decision 8): for a `source_reconciliation` event, the missed report's own URL from the reconciliation ledger (`witness_url`), so the row's Review can open the report itself — witness items never enter the feed ([ADR 0069](adr/0069-source-reliability-and-disclosure-signals.md)), so no feed navigation can reach it. `null` for every other evidence type or when the ledger row is gone (legacy rows fall back to the company Feed).

`evidenceType` ∈ `company_signal` (ref = signal id) | `autopilot_run` (ref = run id) | `daily_quote` (ref = quote date) | `source_reconciliation` (ref = reconciliation-result id) | `job` (ref = `job_queue.id`).

`ruleId` and `companyId` are both **nullable**. `ruleId` is `null` for a SYSTEM event (no user rule raised it): `source_reconciliation` and `job_failed`. `companyId` is `null` only for a system event with **no company scope** — a workspace-wide background job that failed terminally (since `v0.62`, migration `0118`, [ADR 0091](adr/0091-failure-path-and-real-state-testing.md) decision 2); consumers must handle the null scope (the Today stream groups such rows under a system scope) rather than assuming a company.

`triggerType` `job_failed` ([ADR 0091](adr/0091-failure-path-and-real-state-testing.md) decision 1) is the generic background-job failure surface: it fires **once**, at the queue's single terminal-failure point (retries exhausted), and **only** for job kinds with no richer domain surface of their own — `jobs::failure_surface` classifies every registered kind exclusively onto `TodayAttention` | `SourcesAdapterHealth` | `AutopilotRunCard`, so one failure never appears twice. Severity is `notable` for every kind. `evidenceDetail` carries the job's raw `kind`, `evidenceTitle` the handler's failure subject (a document title, a ticker) or — absent one — the job's own `last_error`; both come from a guarded `LEFT JOIN job_queue`.

`evidenceTitle` / `evidenceDetail` (nullable, since `v0.60`, [ADR 0087](adr/0087-today-attention-home-v2.md) decision 4; experience contract §4): the event's **concrete specifics**, resolved by evidence-type-guarded LEFT JOINs **at read** so every stream row states WHAT happened, not a bare category. `evidenceTitle` = the filing title (`company_signal`), the missed report's witness title (`source_reconciliation` → `witness_title`), or the processed report's document title (`autopilot_run` → `report_documents.title`). The `company_signal` title reads through a **durable fire-time snapshot** (`v0.60` D7): `COALESCE(NULLIF(attention_events.evidence_title,''), feed_items.title)` — the snapshot is written when the event fires so the title survives the feed prune that cascade-deletes the signal row (a `company_signals.feed_item_id` `ON DELETE CASCADE`), which otherwise degrades the row to a bare category; `source_reconciliation` snapshots the witness title at fire time too, `autopilot_run` keeps the live join (its `report_documents` row outlives feed pruning). For `source_reconciliation` the title is a fallback chain — `NULLIF(witness_title,'')` first, else the trimmed `report_type` + `report_number` concatenation (both raw registry strings) — because the live GPW registry parser leaves `witness_title` empty on fresh rows while still parsing the report type/number; both absent → `null`. `evidenceDetail` = a secondary raw datum whose meaning depends on `evidenceType`: the display name of the WITNESS that caught the missed report (`source_reconciliation`, `witness_adapter_id` → registry display name — not the source that missed it; corrected [ADR 0097](adr/0097-toasts-are-action-feedback-only.md) decision 8), the run's raw status (`autopilot_run`), or the failed job's raw `kind` (`job`). Both are **raw source data** — the frontend composes/translates any prose ([ui-authoring §6](ui-authoring.md)); a `null` (legacy row / pruned evidence) means the frontend falls back to generic copy.

The `severity` field ∈ `urgent` | `notable` | `routine` (`AttentionSeverity`, since `v0.60`, [ADR 0087](adr/0087-today-attention-home-v2.md) decision 2). **Computed at read** by a single backend mapping (`storage::severity`) from `triggerType` + the signal category (resolved from the event's evidence for `signal_category` events) — never stored, never re-inferred by the frontend. The authoritative level → trigger/category table lives in [Product Spec § Attention Routing](product-spec.md#severity-taxonomy).

Rule commands:

- `create_alert_rule(input)`: creates a rule from `NewAlertRule` (`{ triggerType, signalCategory, priceMin, priceMax, scopeType, scopeRef }`; `enabled` defaults true). Validates trigger invariants (`signalCategory` present for `signal_category`; `priceMin ≤ priceMax` for `price_enters_range`; known scope; non-empty `scopeRef`) → `InvalidInput` otherwise.
- `list_alert_rules()`: all rules, oldest first (`createdAt, id`).
- `update_alert_rule(input)`: patches an existing rule from `AlertRuleUpdate` (`{ id, enabled?, signalCategory?, priceMin?, priceMax?, scopeType?, scopeRef? }`); `null` fields are left unchanged, and the merged rule is re-validated. `NotFound` for an unknown id.
- `set_alert_rule_enabled(input)`: `{ id, enabled }` → toggles the rule (disabled rules never fire) and returns the updated row. `NotFound` for an unknown id.
- `delete_alert_rule(input)`: `{ id }` → deletes the rule; its attention events CASCADE. Idempotent.

Event commands:

- `list_attention_events(input?)`: fired events newest-first, filtered by the optional `AttentionEventListInput` (`{ companyId?, includeDismissed }`; default excludes dismissed).
- `mark_attention_event_seen(input)`: `{ id }` → marks an event seen (read, not dismissed). Idempotent.
- `mark_attention_events_seen(input)`: `{ ids }` → marks a batch seen in one statement ([ADR 0097](adr/0097-toasts-are-action-feedback-only.md) decision 5): Today calls it for every loaded unseen event when its stream renders, so "seen" means *was on screen the last time Today was open* and the sidebar badge clears on a visit. Unknown ids are ignored; an empty batch is a no-op. Idempotent.
- `dismiss_attention_event(input)`: `{ id }` → dismisses an event (also marks it seen); it drops out of the default list. Idempotent.

## Morning Briefing

A daily/on-demand briefing ([ADR 0068](adr/0068-attention-routing-and-morning-briefing.md) decision 4, `v0.54.0`): a **deterministically composed item list** ("what changed in my companies + what needs doing"), end to end — the optional AI narrative half was retired ([ADR 0084](adr/0084-retire-in-app-ai-layer.md); migration `0102` dropped `narrative_markdown`/`narrative_provider_id`/`narrative_model`). Field/storage rules (composer inputs, ordering, `since` boundary) are canonical in [Data Model § Morning Briefing](data-model.md#morning-briefing).

`MorningBriefing` (returned by `get_latest_morning_briefing`):

```json
{
  "id": "morning_briefing_0001",
  "composedAt": "2026-07-15T06:00:00.000Z",
  "since": "2026-07-14",
  "language": "en",
  "createdAt": "2026-07-15T06:00:00.000Z",
  "items": [
    {
      "id": "morning_briefing_0001_b1",
      "briefingId": "morning_briefing_0001",
      "position": 0,
      "itemType": "signal",
      "companyId": "company_gpw_cdr",
      "domainDate": "2026-07-14",
      "citationKey": "b1",
      "evidenceType": "company_signal",
      "evidenceRef": "signal_42",
      "title": "Profit warning issued",
      "detail": "profit_warning"
    }
  ]
}
```

- `itemType` ∈ `signal` (ref = signal id) | `autopilot_run` (ref = run id) | `claim_due` (ref = claim id) | `report_date` (ref = `companyId:eventKey`) | `attention_event` (ref = attention-event id). Items are ordered by `domainDate` (never `createdAt`); `citationKey` (`b1`, `b2`, …) is a stable per-item reference key that resolves against `(evidenceType, evidenceRef)`. Items are **deduped** to at most one per `(companyId, itemType, evidenceRef)`, keeping the newest by `domainDate` ([ADR 0087](adr/0087-today-attention-home-v2.md) dec. 1).
- **Typed `title`/`detail` (since `v0.60`, [ADR 0087](adr/0087-today-attention-home-v2.md) dec. 4).** The composer writes ONLY verbatim source data or typed codes/tokens into `title`/`detail` — never composed English prose; the frontend (`briefingItemText.ts`) translates. Per `itemType`: **signal** — `title` = signal title, `detail` = category CODE (e.g. `profit_warning`); **attention_event** — `title` = `trigger_type` code (e.g. `signal_category`), `detail` = `evidence_type` code (e.g. `company_signal`); **autopilot_run** — `title` = the run's `summaryText` token stream as stored (`report_processed` fallback), `detail` = `status` code (`succeeded`/`partial`); **claim_due** — `title` = claim statement, `detail` = typed token `due:<periodType>:<fiscalYear>` (e.g. `due:Q2:2026`); **report_date** — `title` = entry title, `detail` = qualified ticker. Legacy briefings composed before `v0.60` keep their English prose (no migration); the frontend reads both tolerantly (a non-token title/detail passes through verbatim).

Commands:

- `generate_morning_briefing()`: enqueues an on-demand compose (forces a fresh compose even if today's briefing exists) on the durable queue (the `morning_briefing` autopilot-lane job). Returns once queued; poll `get_latest_morning_briefing` for the result. A once-per-day auto compose is enqueued by the Rust scheduler while the app is open.
- `get_latest_morning_briefing()`: the most recently composed briefing (`MorningBriefing`), or `null` when none has been composed yet.

## AI Analysis Result / AI Analysis Job — retired ([ADR 0084](adr/0084-retire-in-app-ai-layer.md))

In-app feed-item AI analysis is removed; the `ai_analysis_results` and `ai_analysis_jobs` tables were dropped by migration `0102` (no readable history survives). No new feed-item analysis is produced in-app — intelligence arrives through the MCP port (BYOA) — and `list_ai_analysis` no longer exists as a command.

## Video Transcript Job

```json
{
  "jobId": "job_video_01",
  "companyId": null,
  "providerId": "provider_gemini",
  "sourceType": "youtube_url",
  "sourceUrl": "https://www.youtube.com/watch?v=example",
  "sourceLabel": null,
  "companyResolutionStatus": "unresolved",
  "recognizedCompanyCandidates": [],
  "status": "queued",
  "errorCode": null,
  "createdAt": "2026-05-28T13:30:00Z",
  "startedAt": null,
  "finishedAt": null,
  "error": null
}
```

Rules (`companyId` nullability and resolution-status transitions are canonical in [Data Model § Transcript Jobs](data-model.md#transcript-jobs)):

- The UI input label for `sourceUrl` is `URL`.
- If the user does not provide a ticker/company, the transcript may remain unlinked and visible. The contract reserves `recognizedCompanyCandidates` for future provider-assisted recognition, but M10 does not require automatic company recognition before the transcript can be reviewed.
- Allowed `companyResolutionStatus` values: `provided`, `recognized`, `unresolved`, `needs_user_selection`.
- `recognizedCompanyCandidates` uses the same canonical company identity shape as company lookup results when recognition produces candidates.
- A completed transcript may remain unlinked to any company and must still be viewable on demand.
- Transcript segments can exist while `companyId` is unresolved.
- Company selection is required only when the user wants to save selected segments into a company notebook.
- Allowed `status` values: `queued`, `running`, `completed`, `failed`.
- Allowed `errorCode` values when `status = failed`: `provider_not_configured`, `provider_limit`, `provider_unavailable`, `provider_error`, `network_error`, `invalid_source_url`, `parse_error`, `unknown`.
- `error` is user-readable local diagnostic text and must not store provider secrets.

## Create Video Transcript Job Input

```json
{
  "sourceUrl": "https://www.youtube.com/watch?v=example",
  "sourceLabel": "Q2 investor conference",
  "companyId": null,
  "companyQuery": "CDR",
  "providerId": "provider_gemini"
}
```

Rules:

- `sourceUrl` is required and is shown in the UI as `URL`.
- `sourceLabel` is optional and is shown in the UI as the transcript row title/description when present.
- `sourceLabel` is user-editable after job creation because it is local metadata, not provider transcript source text.
- The UI must not expose generated transcript job IDs as normal row titles.
- `companyId` is optional. If present, it must reference an existing local company.
- `companyQuery` is optional and represents a user-provided ticker/company/ISIN search value. If present without `companyId`, the app should resolve it through the same local lookup used by Companies before creating the job.
- If neither `companyId` nor `companyQuery` resolves a company, the job is created with `companyId = null` and `companyResolutionStatus = unresolved`.
- `providerId` defaults to `provider_gemini` for M10 and must not change the general AI analysis provider preference.
- Duplicate create requests for the same normalized `sourceUrl` and the same company scope must return the existing transcript job instead of creating another row.
- Unlinked jobs and company-linked jobs are separate duplicate scopes for the same `sourceUrl`.
- `delete_video_transcript_job(jobId)` removes the transcript job and its stored transcript segments.
- Deleting a transcript job must not delete notebook entries that were already created from it; saved notebook origins remain historical references.

## Update Video Transcript Job Input

```json
{
  "jobId": "job_video_01",
  "sourceLabel": "Renamed investor conference"
}
```

Rules:

- `update_video_transcript_job(input)` updates editable local transcript job metadata.
- M10 supports `sourceLabel` updates only.
- Blank `sourceLabel` clears the description and returns the UI title fallback to `Untitled transcript`.
- Updating transcript job metadata must not mutate transcript segment text, provider output, source URL, status, or notebook origins.

## Resolve Transcript Job Company Input

```json
{
  "jobId": "job_video_01",
  "companyId": "company_gpw_cdr"
}
```

Rules:

- Resolving a job company sets the job `companyId`.
- Existing transcript segments for the job inherit the resolved company for UI/read-model purposes.
- Resolution is optional for transcript visibility.
- Resolution is required only before `create_note_from_transcript_selection` can save a company notebook entry.

## Run Video Transcript Job Input

```json
{
  "jobId": "job_video_01",
  "providerMode": "provider_gemini"
}
```

Rules:

- Creating a job may auto-start live transcription when Gemini credentials are configured. Failed or queued jobs can be run again through the visible `Retry` action.
- Allowed `providerMode` values: `provider_gemini`, `test_sample`.
- `provider_gemini` is the required M10 live provider path and must require configured credentials.
- `test_sample` uses offline sample transcript output for automated tests and local development only; it cannot satisfy M10 completion.
- Provider runner success stores immutable transcript segments and marks the job `completed`.
- Provider runner failure stores `status = failed`, `errorCode`, and user-readable `error`.
- Live provider calls must happen in Rust-side code, never directly from React.
- Live provider calls must not log API keys or full transcript text.
- Default automated tests must mock Gemini responses or use test samples; they must not require a real Gemini API key.
- M10 uses direct YouTube URL input to Gemini. If Gemini rejects the URL or request, the job fails with a provider error containing the provider cause when available; M10 does not implement hidden audio download/extraction fallback.

## Transcript Segment

```json
{
  "id": "segment_01",
  "transcriptJobId": "job_video_01",
  "companyId": null,
  "startSeconds": 120,
  "endSeconds": 168,
  "speaker": null,
  "text": "Management statement extracted from the conference.",
  "language": "pl",
  "createdAt": "2026-05-28T13:35:00Z"
}
```

Rules:

- Segment timestamps should be stored when the provider returns enough information.
- `companyId` follows the parent transcript job and may be null until company resolution is complete.
- The original YouTube URL must be retained.
- Transcript segment text is immutable source output in v1.
- Notes created from transcript segments are editable before saving.

## Transcript-To-Note Selection

```json
{
  "transcriptJobId": "job_video_01",
  "transcriptSegmentIds": ["segment_01"],
  "noteDraft": {
    "title": "Claim from Q2 conference",
    "body": "Management expects the release milestone within two quarters.",
    "tags": ["conference", "management-guidance"],
    "kind": "claim",
    "claimStatus": "open",
    "followUpAfter": "2026-Q4"
  }
}
```

Rules:

- The user chooses which transcript segments become notes.
- V1 uses whole-segment selection.
- `create_note_from_transcript_selection(input)` creates a company notebook entry, so it must reject unlinked transcript jobs and non-completed jobs.
- AI may suggest note drafts, but the user confirms before saving.
- Saved notes are normal notebook entries with `transcript_segment` origins.
- Saved note origins must retain selected segment IDs, the original video URL, provider/job context, and timestamp ranges when available.

## Scheduler Job Status

```json
{
  "jobId": "job_01",
  "type": "source_poll",
  "adapterId": "gpw-espi-ebi",
  "status": "running",
  "startedAt": "2026-05-28T13:00:00Z",
  "finishedAt": null,
  "itemsFetched": 0,
  "itemsCreated": 0,
  "detailItemsAttempted": 0,
  "detailItemsStored": 0,
  "detailItemsFailed": 0,
  "warnings": [],
  "error": null
}
```

Allowed statuses:

- `queued`
- `running`
- `succeeded`
- `failed`
- `cancelled`

`get_scheduler_status()` returns the Rust-side source scheduler's next-due snapshot ([ADR 0055](adr/0055-autonomous-report-pipeline-trust-ladder.md)), for the UI's "next refresh at …" display; times are epoch milliseconds and the state is in-memory/app-open-only (never persisted):

```json
{
  "sourceNextDueMs": { "gpw-espi-ebi": 1780000000000 },
  "registryNextDueMs": 1780003600000
}
```

## Diagnostic Event

Diagnostic events are local developer-mode records that explain what a module did at a meaningful stage. They are not user-facing errors, runtime logs, metrics, telemetry, traces, or analytics.

```json
{
  "id": "diagnostic_event_01",
  "occurredAt": "2026-06-03T10:15:00Z",
  "module": "ai_analysis",
  "scope": {
    "type": "ai_analysis_job",
    "id": "analysis_job_01"
  },
  "stage": "request_sent",
  "severity": "info",
  "message": "Gemini analysis request sent.",
  "metadata": {
    "providerId": "provider_gemini",
    "model": "gemini-2.5-flash",
    "timeoutSeconds": 90
  }
}
```

Field reference:

| Field | Type | Meaning |
|---|---|---|
| `module` | enum | `ai_analysis`\|`external_ai`\|`sources`\|`scheduler`\|`credentials`\|`storage`\|`transcripts`\|`shortcuts`\|`locale`\|`licensing`\|`packaging` |
| `scope.type` | string | entity category, e.g. `ai_analysis_job`, `feed_item`, `source_adapter`, `transcript_job`, `setting`, `shortcut_action` |
| `scope.id` | string\|null | stable local id (never a title/URL/prompt/source text/provider snippet); null only when the event is truly global to the module |
| `stage` | string | stable snake_case, never encoding dynamic values; past-tense for completed steps (`context_loaded`, `provider_resolved`, `credential_checked`, `request_sent`, `response_received`, `result_stored`, `failed`) or job-lifecycle for async work (`queued`, `running`, `succeeded`, `cancelled`, `failed`); reused across modules when the meaning matches |
| `severity` | enum | `debug`\|`info`\|`warning`\|`error` |
| `message` | string | human-readable summary |
| `metadata` | JSON object | structured, small enough for a timeline row/detail panel; may hold stable IDs, provider IDs, model names, adapter IDs, status values, durations, counts, timeouts, retry counts, error classes, booleans; must never hold API keys, full prompts, full source bodies, full transcript text, raw provider responses, or license secrets — redacted before persistence, with a `[redacted]` marker where omission would confuse |

The event shape stays cheap to map to future OpenTelemetry-style event/span fields, but M14 does not implement OpenTelemetry exporters or remote reporting.

AI analysis diagnostic stages:

- `queued`
- `running`
- `context_loaded`
- `provider_resolved`
- `credential_checked`
- `request_sent`
- `response_received`
- `parsed`
- `stored`
- `failed`

Rules (Developer-mode gating and storage are canonical in [Data Model § Diagnostic Events](data-model.md#diagnostic-events)):

- Normal user-facing UI must not depend on diagnostic events for core behavior.
- Copying a diagnostic summary must produce redacted text.
- Raw diagnostic JSON/file export is outside M14 scope.

Initial local commands:

- `list_diagnostic_events(input)`: returns recent diagnostic events only when Developer mode is active.
- `clear_diagnostic_events()`: deletes diagnostic events only when Developer mode is active.
- `get_diagnostic_summary(input)`: returns redacted plain-text summary text only when Developer mode is active.

Initial list/summary input:

```json
{
  "limit": 200
}
```

Clear result:

```json
{
  "eventsDeleted": 12
}
```

Summary result:

```json
{
  "summary": "Diagnostic summary\nEvents included: 1\n2026-06-03T10:15:00.000Z | info | ai_analysis | ai_analysis_job:analysis_job_01 | request_sent | Gemini analysis request sent.",
  "eventCount": 1
}
```

Command rules:

- Commands must return an error when Developer mode is not active.
- Summary text must not include raw diagnostic JSON or unredacted metadata.
- Summary text is intended for manual copy/paste from the developer-only UI, not automatic export.

## Local Metrics

Local metrics are Developer-mode-only operational health samples. They are separate from user-facing status, diagnostics, logs, telemetry, traces, and analytics.

Initial command:

- `get_local_metrics_snapshot()`: returns a point-in-time metrics snapshot only when Developer mode is active.

Snapshot shape:

```json
{
  "collectedAt": "2026-06-04T10:00:00.000Z",
  "samples": [
    {
      "name": "brawler_source_refresh_total",
      "description": "Process-lifetime source refresh attempts by adapter and status.",
      "kind": "counter",
      "unit": "count",
      "value": 2,
      "labels": [
        { "key": "adapter_id", "value": "bankier-company-komunikaty" },
        { "key": "status", "value": "succeeded" }
      ],
      "collectedAt": "2026-06-04T10:00:00.000Z"
    }
  ]
}
```

Rules:

- Metrics are collected as on-demand snapshots from durable local state plus explicit in-memory runtime counters.
- Runtime counters are process-lifetime signals and reset when the app restarts.
- Metric names use Prometheus-friendly snake case where practical.
- Metric labels must stay low-cardinality and privacy-safe.
- Allowed label keys include `module`, `collector`, `adapter_id`, `provider_id`, `model`, `status`, `severity`, `table`, and `unit`.
- Metric names and labels must not include full URLs, titles, prompts, source bodies, note text, transcript text, company names, ticker symbols, user-entered strings, secrets, or high-cardinality values.
- The in-app Diagnostics Metrics section is the first presentation adapter. Prometheus, OpenTelemetry, file, or other local integrations must be added later as separate adapters over the same internal samples.
- M16 does not expose a Prometheus endpoint, scrape surface, remote export, hosted observability, or metrics settings.

## Runtime Logs

Runtime logs are bounded local file records for troubleshooting normal app execution. They are separate from user-facing errors, structured diagnostic events, metrics, telemetry, traces, and analytics.

Initial log format: JSON Lines.

```json
{
  "timestamp": "2026-06-03T10:15:00Z",
  "level": "info",
  "target": "brawler::jobs::source_refresh",
  "module": "sources",
  "message": "Source refresh completed.",
  "fields": {
    "adapterId": "bankier-company-komunikaty",
    "itemsCreated": 2
  }
}
```

Rules:

- Logs are written under the OS app data logs directory.
- Default log level is `info`.
- Supported log levels are `off`, `error`, `warn`, `info`, `debug`, and `trace`.
- Log level is configurable in Settings and may be overridden by local environment for development.
- Rotation limits are configurable in Settings and default to five files of five MiB each.
- Logs use the shared observability redaction policy before writing fields.
- Logs must not include API keys, full prompts, full source bodies, full transcript text, raw provider responses, license private material, or full license secrets by default.
- Diagnostics may expose a full in-app log viewer, copy-redacted-log action, log status, and open-logs-folder action only while Developer mode is active.
- React may call typed commands for log status, redacted log reads, and opening the app-owned logs directory. It must not receive arbitrary filesystem browsing capability.

Commands:

- `get_log_status()`: returns `{ logsDir, currentFileBytes, rotatedFileCount, level, maxFiles, maxFileBytes }`.
- `list_log_entries(input?)`: returns redacted JSON Lines log entries (`{ fileName, lineNumber, record }`), newest first; `input` is `{ limit? }`.
- `open_logs_directory()`: opens the OS app data logs directory in the system file browser.

## Entitlements

Brawler keeps a local entitlement module for optional or future gated capabilities. Public-opening work makes the open desktop core usable without a license token. The module does not add hosted activation, billing, telemetry, or cloud accounts.

License status read model:

```json
{
  "status": "valid",
  "canUseApp": true,
  "reason": null,
  "license": {
    "licenseId": "lic_example_001",
    "holder": "Example User",
    "channel": "example",
    "edition": "example",
    "features": ["example_feature"],
    "issuedAt": "2026-06-01T00:00:00Z",
    "expiresAt": "2099-01-01T00:00:00Z",
    "appVersionRange": "*",
    "keyId": "example_key"
  },
  "checkedAt": "2026-06-04T10:00:00Z"
}
```

Allowed `status` values:

- `valid`
- `missing`
- `invalid`
- `expired`
- `wrong_version`
- `unsupported_version`
- `storage_error`

Typed commands:

- `get_license_status() -> LicenseStatus`
- `submit_license_key({ licenseKey }) -> LicenseStatus`
- `clear_license_key() -> LicenseStatus`

Rules (keychain/`license_metadata` split and the never-overwrite/never-store-secrets storage rules are canonical in [Data Model § Entitlements](data-model.md#entitlements)):

- Normal app navigation requires `canUseApp = true`; public-opening policy sets `canUseApp = true` for missing, invalid, expired, wrong-version, unsupported-version, and storage-error entitlement states so the open core remains usable.
- Missing, malformed, tampered, expired, unsupported-version, unsupported-channel, and storage-error states must remain recoverable through Settings.
- Supported channels are build-policy specific; unsupported channels are invalid for that build.
- Public-opening entitlement tokens are optional for normal open-core use.
- `appVersionRange` remains compatibility metadata and may be `*` for channels that are not app-version bounded.
- Future entitlement channels may opt into app-version limits through the existing `appVersionRange` policy path.
- `submit_license_key` validates the token offline before saving it.
- React receives only `LicenseStatus`; it never receives private signing material.
- Logs, diagnostics, metrics, settings export, tests, and UI state must not include full license tokens, private signing material, or raw private key material.
- Future paid feature, subscription, or hosted activation policies must be added as entitlement-policy or verifier/storage adapters and require a later ADR when they introduce hosted services or billing.

## Global Search

```json
{
  "query": "profit warning",
  "contentTypes": ["company", "watchlist", "feed_item", "notebook_entry", "transcript_segment", "event"],
  "companyId": null,
  "limit": 50
}
```

Result shape:

```json
{
  "groups": [
    {
      "contentType": "feed_item",
      "matches": [
        {
          "sourceId": "feed_item-id",
          "companyId": "company-id",
          "parentId": null,
          "title": "…",
          "snippet": "…profit warning…",
          "score": 1.83
        }
      ]
    }
  ]
}
```

Rules ([ADR 0032](adr/0032-search-and-backup-boundaries.md); FTS5 schema, sanitization, and `parentId` derivation are canonical in [Data Model § Search Index](data-model.md#search-index)):

- One typed search command queries the unified `search_index` FTS5 table; DTOs live in `src/api/search.ts` and command modules contain no SQL.
- An empty/blank `query` returns no groups.
- `contentTypes` and `companyId` are optional scoping filters. Omitting `contentTypes` searches all types.
- Matches are returned grouped by `contentType`, each carrying `sourceId`, `companyId`, `parentId`, `title`, `snippet`, and `score` — enough context to render and navigate to the specific item. Snippet highlight markers are control characters (STX/ETX), not HTML, so callers render snippets as plain text.
- Coverage is companies, watchlists, feed items, notebook entries, transcript segments, and company events.

## Database Backups

```json
{
  "lastBackupAt": "2026-06-14T10:00:00.000Z",
  "backupCount": 5,
  "backups": [
    { "fileName": "brawler-v0039-2026-06-14T100000Z.sqlite3", "createdAt": "2026-06-14T10:00:00.000Z", "kind": "rotating", "sizeBytes": 1048576 }
  ]
}
```

Rules ([ADR 0032](adr/0032-search-and-backup-boundaries.md); `VACUUM INTO` mechanics, pre-migration snapshots, rotation, and restage-on-relaunch are canonical in [Data Model § Database Safety](data-model.md#database-safety-wal-snapshots-and-backups)):

- Typed commands expose backup status, list, create-now, and restore; UI never touches backup files directly.
- `kind` distinguishes `rotating` backups from pre-migration `snapshot` files.
- Restore requires explicit confirmation before it is staged.

Commands:

- `backup_status()`: returns the shape above (`lastBackupAt`, `backupCount`, `backups`).
- `create_backup()`: creates a backup now and returns the refreshed status.
- `restore_backup({ fileName })`: stages `fileName` for restore on next app relaunch.

### Database Status

`database_status()` returns a lightweight local health snapshot for Diagnostics — applied migration count and row counts for a few key tables:

```json
{ "appliedMigrations": 45, "companies": 12, "sourceAdapters": 3, "settings": 1 }
```

## User Settings

```json
{
  "theme": "dark",
  "locale": "en",
  "accentPalette": "night-neon",
  "developerMode": false,
  "pollIntervalSeconds": 900,
  "backfillYears": 3,
  "settingsSource": "sqlite",
  "settingsImportExportFormat": "yaml",
  "yamlImportExportStatus": "accepted_deferred",
  "aiProviders": {
    "youtubeTranscriptionProvider": "provider_gemini",
    "youtubeTranscriptionModel": "gemini-2.5-flash",
    "youtubeTranscriptionTimeoutSeconds": 300
  },
  "logs": { "level": "info", "maxFiles": 5, "maxFileBytes": 5242880 },
  "shortcutBindings": {},
  "database": {
    "maxConnections": 4,
    "busyTimeoutMs": 5000,
    "acquireTimeoutMs": 10000
  },
  "queue": {
    "sourcesWorkers": 2,
    "autopilotWorkers": 3
  },
  "pinnedCompanyIds": [],
  "mcp": { "enabled": false, "port": 8317, "writesEnabled": false }
}
```

The `aiProviders` block carries ONLY the YouTube-transcription provider fields — the sole in-app AI dependency ([ADR 0084](adr/0084-retire-in-app-ai-layer.md)); the former analysis-provider fields, `aiAnalysisMode`, `capabilityProviders`, `historySweepAiCallLimit`, and the `queue.aiWorkers`/`aiProviderConcurrency` knobs no longer exist in the settings shape (stored legacy rows are ignored on read).

`update_settings` is atomic: a validation failure on any field rolls back the whole request, leaving every setting (including fields earlier in the same request) untouched.

`backfillYears` (ADR 0077 §3) is the years of company history the on-track backfill fetches. `update_settings` accepts an optional `backfillYears`, clamped to `[1, 10]` on write (never rejected); omitting it leaves the current depth unchanged. No seed row: reads default to `3` and clamp an out-of-range stored value. The Sources settings section exposes it as clickable presets (1/3/5/10) bound to a slider + numeric input.

`pinnedCompanyIds` (ADR 0054) is the ordered list of company IDs pinned to the
sidebar IA spine. `update_settings` accepts an optional `pinnedCompanyIds`
(full-replacement array, de-duplicated, blanks dropped); omitting it leaves the
current pins unchanged. Defaults to `[]`.

Allowed theme values:

- `dark`
- `light`
- `system`

Allowed accent palette values:

- `night-neon`
- `midnight-horizon`

Initial allowed locale values:

- `en`
- `pl`

Field defaults and validation ranges:

| Field | Default | Notes |
|---|---|---|
| `theme` | `dark` | brightness only (`dark`\|`light`\|`system`); `system` may be added as a convenience but first run still defaults `dark` |
| `accentPalette` | `night-neon` | semantic palette (`night-neon`\|`midnight-horizon`); `midnight-horizon` maps the owner's sampled reference-image colors: background `#00021E`, surface `#061135`, primary `#63C0E9`, secondary `#55388F`, accent `#C550B9`, highlight `#FB82C0`, text `#EAF7FF` |
| `locale` | `en` | `en`\|`pl`; affects app-owned UI copy/labels only, never source-provided text |
| `developerMode` | `false` | enabled only via `BRAWLER_DEVELOPER_MODE` env or runtime unlock passphrase, never a plain toggle |
| runtime log level | `info` | |
| runtime log rotation | 5 files × 5 MiB | |
| `aiProviders.youtubeTranscriptionModel` | `gemini-2.5-flash` | cheapest M10-validated model; options: gemini-2.5-flash-lite\|gemini-2.5-flash\|gemini-3.1-flash-lite\|gemini-3.5-flash |
| `aiProviders.youtubeTranscriptionTimeoutSeconds` | `300` | options: 45\|90\|180\|300\|600 |
| `database.maxConnections` | `4` | clamped 1–16 |
| `database.busyTimeoutMs` | `5000` | clamped 0–60000 |
| `database.acquireTimeoutMs` | `10000` | clamped 1000–60000; database pool ADR 0032; applied at pool build (next launch) |
| `queue.sourcesWorkers` | `2` | clamped 1–16 |
| `queue.autopilotWorkers` | `3` | clamped 1–16; queue tuning ADR 0059 — indexing stays a constant 1 worker; applied at next launch |

A missing or invalid value for any clamped field falls back to its default so the app always opens. Settings must offer a reset-to-defaults action for the database pool block, and disclose that pool/queue changes take effect on next launch.

Other rules:

- `theme` controls brightness mode only; `accentPalette` controls the semantic color palette. Accent palettes must be added through the settings validation and theme-token registry, not as component-local color overrides.
- Developer mode may be enabled only through intentional local developer mechanisms, not a normal always-visible Settings toggle. Startup activation uses `BRAWLER_DEVELOPER_MODE=1`, `true`, `yes`, or `on`. Runtime author unlock (`unlock_developer_mode({ passphrase })`) may enable Developer mode after the app is already running only when `BRAWLER_DEVELOPER_UNLOCK_CODE` is present in the app process environment and the submitted passphrase matches it; the entry point is hidden from normal UI and must not be registered as a configurable shortcut. Once active, Diagnostics may show status and a disable action (`disable_developer_mode()`).
- Settings must let the user switch the app locale between English and Polish; locale handling is an extensible app-locale boundary so future locales are added through resources/configuration, not per-screen rewrites. Source-provided text, company names, ticker symbols, URLs, source attribution, transcript text, and notebook bodies retain their original or user-entered language.
- Settings must show that `provider_gemini` is selected only for YouTube transcription; must show whether transcription credentials are configured; must let the user save/replace/clear the Gemini API key used only for transcription; must disclose before use that starting a transcript job sends the YouTube URL and video content to Gemini.
- Settings must let the user configure, disable, and reset every defined shortcut action through stable shortcut action IDs. Shortcut binding overrides are stored as a JSON object keyed by action ID (missing entries use the current default). Shortcut conflicts must be visible before an enabled binding can silently shadow another enabled action.
- Settings/About must show local license status and allow valid users to inspect safe metadata, replace the token, and clear the token.
- SQLite is the runtime source of truth for settings. YAML is allowed for settings import/export/bootstrap (allowlisted non-secret settings only) but must never contain secrets; API keys and provider secrets live in the OS keychain. `.env`/environment-variable API key fallback is allowed for local development and tests only.

## Provider Credential Status

```json
{
  "providerId": "provider_gemini",
  "secretKind": "api_key",
  "configured": true,
  "storage": "os_keychain",
  "label": "Gemini API key",
  "devFallbackAvailable": false,
  "error": null
}
```

Rules:

- Credential status is non-secret metadata and may be returned to React.
- Secret values must never be returned to React.
- Runtime secret storage uses the OS keychain.
- One API key per provider (ADR 0028): the same provider key serves all of that provider's usages (analysis and, for Gemini, transcription). Purpose is not part of the credential identity.
- Development/test fallback may use environment variables (`GEMINI_API_KEY`, `ANTHROPIC_API_KEY`, `OPENAI_API_KEY`, `OPENAI_COMPATIBLE_API_KEY`), but this must be reported as `storage = "development_environment"` and must not count as exported settings.
- Supported `secretKind` values begin with `api_key`; future supported kinds may include `username_password`, `session_token`, or `oauth_token` after source-specific design.
- Credential commands are generic and keyed by `providerId` (`provider_gemini`, `provider_anthropic`, `provider_openai`, `provider_openai_compatible`, `provider_mistral`). **Every credential-bearing provider must be enterable from Settings** (owner rule 2026-07-14): the canonical id list is `CREDENTIAL_PROVIDER_IDS` (`providers/credentials.rs`), pinned to `src/test/scenarios/credentialProviders.json`, which the frontend contract test checks against the Settings form list — a descriptor without a form reddens the gate.
  - `get_provider_credential_status({ providerId })` returns the non-secret status for one provider.
  - `set_provider_api_key({ providerId, apiKey })` stores or replaces only that provider's API key.
  - `clear_provider_api_key({ providerId })` removes only that provider's OS-keychain key and must not mutate `.env` or process environment values.
- Legacy purpose-scoped Gemini credential commands were removed with no backward compatibility; legacy keychain entries are best-effort cleared on startup.

## AI Provider Catalog — retired ([ADR 0084](adr/0084-retire-in-app-ai-layer.md))

The selectable analysis-provider catalog, `list_ai_provider_catalog()`, and
capability-provider routing are removed with the in-app AI analysis layer
(decisions 1/7). The **only** remaining AI dependency is the Gemini transcript
provider (see [Credentials](#credentials)) — data acquisition, not analysis.
The app runs fully featured with zero API keys.

## Research Evidence Boundary

The research workspace uses a dedicated research/evidence boundary governed by [ADR 0022](adr/0022-research-evidence-read-model-boundary.md). It is a cross-domain read-model boundary, not a replacement for existing domain contracts. Entity fields and storage rules are canonical in [Data Model § Research Evidence Boundary](data-model.md#research-evidence-boundary); this section keeps the wire shapes, commands, and command-specific rules.

Initial research evidence item shape:

```json
{
  "id": "evidence_feed_01",
  "evidenceType": "feed_item",
  "sourceDomain": "feed",
  "sourceId": "feed_01",
  "companyId": "company_gpw_cdr",
  "occurredAt": "2026-05-28T12:04:52Z",
  "title": "Current report title",
  "summary": "Short source or AI-supported summary",
  "sourceUrl": "https://www.gpw.pl/komunikaty",
  "attribution": "GPW",
  "trustCategory": "official_report",
  "reviewState": {
    "changedSinceCompanyReview": true,
    "changedSinceWatchlistReview": true
  }
}
```

Initial company/watchlist timeline result shape (same result shape for `list_company_timeline` and `list_watchlist_timeline`; watchlist-scoped calls populate `companySummaries`):

```json
{
  "items": [ /* evidence items — shape above */ ],
  "summary": {
    "total": 1,
    "changedSinceReview": 1,
    "lastReviewedAt": null,
    "memberCompanyCount": 0,
    "companiesWithChangedEvidence": 0,
    "companySummaries": []
  }
}
```

Initial research evidence input shape:

```json
{
  "companyId": "company_gpw_cdr",
  "watchlistId": null,
  "evidenceTypes": ["feed_item", "claim"],
  "changedSinceReviewOnly": false,
  "limit": 100
}
```

Initial evidence types:

- `feed_item`
- `notebook_entry`
- `claim`
- `transcript_segment`
- `company_event`
- `ai_analysis`
- `research_question`
- `company_signal` (confirmed typed ESPI/EBI signals; ADR 0034)
- `reminder`
- `ai_brief`
- `digest`

Initial trust categories:

- `official_report`
- `company_publication`
- `public_media`
- `market_calendar`
- `transcript`
- `user_note`
- `ai_generated`
- `unknown`

Rules:

- Existing domain tables remain the source of truth.
- The research boundary returns read models assembled from existing domains first.
- React consumes research read models for timelines and review workflows instead of assembling them from many unrelated APIs.
- Timeline summary and review counts are backend-owned. The frontend may format and display them, but should not reimplement changed-since-review or cross-domain aggregation rules.
- Evidence type filtering and changed-since-review filtering are backend-owned. The frontend sends selected filter intent and renders the returned result.
- `changedSinceReview` summary and filtering use the active review scope. Company-scoped timelines use the company checkpoint. Watchlist-scoped timelines use the watchlist checkpoint.
- `lastReviewedAt` is `null` when the active scope has never been marked reviewed.
- Watchlist-scoped timelines include backend-owned `companySummaries` so the UI can show a company-by-company review queue without recomputing changed-since-review rules.
- Marking a watchlist reviewed updates the watchlist checkpoint only by default. Callers may explicitly send `cascadeToCompanies: true` to also mark the current member companies reviewed.
- Review checkpoints are durable research-owned state.
- Evidence links are durable research-owned relationships between existing domain entities.
- Normal UI labels must use product language for trust categories. Developer-only surfaces may show implementation identifiers.

Initial evidence link shape:

```json
{
  "id": "evidence_link_01",
  "fromType": "notebook_entry",
  "fromId": "note_01",
  "toType": "feed_item",
  "toId": "feed_01",
  "relationType": "cites",
  "createdAt": "2026-05-28T13:20:00Z"
}
```

Initial research question shape:

```json
{
  "id": "research_question_company_gpw_cdr_01",
  "scopeType": "company",
  "scopeId": "company_gpw_cdr",
  "title": "Will margins recover?",
  "body": "Track source reports and follow-up notes.",
  "status": "open",
  "closedAt": null,
  "createdAt": "2026-06-11T10:00:00Z",
  "updatedAt": "2026-06-11T10:00:00Z"
}
```

Initial research question input:

```json
{
  "scopeType": "company",
  "scopeId": "company_gpw_cdr",
  "title": "Will margins recover?",
  "body": "Track source reports and follow-up notes."
}
```

Initial research question update input:

```json
{
  "id": "research_question_company_gpw_cdr_01",
  "title": "Will margins recover after cost cuts?",
  "body": "Track next two reports.",
  "status": "answered"
}
```

Initial research question deletion input:

```json
{
  "id": "research_question_company_gpw_cdr_01"
}
```

Initial research question statuses:

- `open`
- `answered`
- `closed`

Rules:

- Research questions are durable research-owned entities, not notebook entries.
- The first visible workflow supports company-scoped questions only.
- The storage and command shape keeps `scopeType` open for later watchlist-scoped questions, but normal UI must not expose watchlist question creation until that workflow is designed.
- A research question appears in the research evidence timeline as `evidenceType: "research_question"` so it can be reviewed and linked like other evidence.
- Question-to-evidence relationships use typed `evidence_links` with `fromType: "research_question"` and the target evidence type/id.
- Deleting a research question removes that question and any evidence links attached to it. It must not delete linked feed items, notes, events, transcript segments, AI analysis, or other canonical evidence objects.

AI research briefs are retired ([ADR 0084](adr/0084-retire-in-app-ai-layer.md) decision 5): brief generation and the `ai_research_brief*`/`ai_research_digest*` tables are gone (dropped by migration `0102`, no readable history). Talking to research evidence is now an MCP-connected agent's job (BYOA).

Initial research reminder shape:

```json
{
  "id": "research_reminder_company_gpw_cdr_001",
  "scopeType": "company",
  "scopeId": "company_gpw_cdr",
  "companyId": "company_gpw_cdr",
  "reminderKind": "claim_follow_up",
  "sourceType": "claim",
  "sourceId": "note_claim_01",
  "title": "Review management claim",
  "body": "Check whether the promised milestone was delivered.",
  "dueAt": "2026-12-31T00:00:00Z",
  "status": "open",
  "snoozedUntil": null,
  "completedAt": null,
  "dismissedAt": null,
  "createdAt": "2026-06-12T08:00:00Z",
  "updatedAt": "2026-06-12T08:00:00Z"
}
```

Initial reminder kinds:

- `claim_follow_up`
- `event_review`
- `question_review`
- `manual_research`
- `digest_review`
- `signal_review` (generated when a high-signal ESPI/EBI category — insider transaction, profit warning — is classified; ADR 0034)

Initial reminder statuses:

- `open`
- `completed`
- `dismissed`

Rules:

- Research reminders are research-owned records, not notebook entries and not a generic task system.
- Reminders should link to canonical research evidence whenever possible.
- The backend may derive reminders from open claims, scheduled events, and open research questions, then store durable reminder status.
- Completing a reminder does not mark a company or watchlist reviewed by default.
- Deleting a reminder must not delete the linked claim, event, question, note, feed item, or digest.

AI research digests are retired the same as AI research briefs (above): the `ai_research_digest_jobs`/`ai_research_digests`/`ai_research_digest_citations` tables were dropped by migration `0102` (no readable history survives). Their replacement is an MCP-connected agent working over research evidence (BYOA).

Initial relation types:

- `originates_from`
- `cites`
- `supports`
- `contradicts`
- `updates`
- `follows_up`
- `answers`
- `related`

Initial review checkpoint shape:

```json
{
  "id": "review_company_gpw_cdr",
  "scopeType": "company",
  "scopeId": "company_gpw_cdr",
  "reviewedAt": "2026-06-08T10:00:00Z"
}
```

Initial research command candidates:

- `list_research_evidence(input)`, returning a timeline result with `items` and backend-owned `summary`
- `list_company_timeline(companyId)`
- `list_watchlist_timeline(watchlistId)`
- `mark_research_scope_reviewed(input)`
- `list_research_review_state(input)`
- `list_research_questions(input)`
- `create_research_question(input)`
- `update_research_question(input)`
- `delete_research_question(id)`
- `list_evidence_links(input)`
- `create_evidence_link(input)`
- `delete_evidence_link(id)`
- `list_research_reminders(input)`
- `create_research_reminder(input)`
- `update_research_reminder(input)`
- `delete_research_reminder(id)`
Research briefs and digests are **gone entirely** — generation and read alike
([ADR 0084](adr/0084-retire-in-app-ai-layer.md) decision 5, clean cut): the
`ai_research_brief*` / `ai_research_digest*` tables are dropped by migration
0102, so there is nothing left to list. Their replacement arrives as MCP
read/write tools (`v0.60.0`).

These command names may be refined during implementation, but the ownership boundary should remain stable.

## Company Fundamentals

Company fundamentals capture report-derived financial metrics and KPI tracking for each tracked company, governed by [ADR 0027](adr/0027-company-fundamentals-scope.md) and documented in [Data Model](data-model.md#company-fundamentals).

### Financial Period

```json
{
  "id": "period_company_gpw_cdr_fy2025",
  "companyId": "company_gpw_cdr",
  "fiscalYear": 2025,
  "periodType": "FY",
  "periodEndDate": "2025-12-31",
  "reportEvidenceRef": null,
  "createdAt": "2026-06-01T10:00:00Z",
  "updatedAt": "2026-06-01T10:00:00Z"
}
```

Allowed `periodType` values:

- `FY` (fiscal year)
- `H1`, `H2` (half-year)
- `Q1`, `Q2`, `Q3`, `Q4` (quarter)
- `9M` (nine months)
- `M01` through `M12` (individual months)

Field/storage rules (uniqueness, FKs, soft references) are canonical in [Data Model § Company Fundamentals](data-model.md#company-fundamentals).

### KPI Definition

KPI definitions form a three-layer model: canonical catalog (`kpi_definitions`), company-specific selection (`kpi_relevance`), and reported values (`financial_facts`).

```json
{
  "id": "kpi_def_net_revenue",
  "scope": "canonical",
  "companyId": null,
  "sector": null,
  "metricKey": "net_revenue",
  "label": "Net Revenue",
  "valueKind": "monetary",
  "unit": "PLN",
  "computation": "reported",
  "formula": null,
  "displayFormat": "currency_0dp",
  "createdAt": "2026-06-01T10:00:00Z",
  "updatedAt": "2026-06-01T10:00:00Z"
}
```

Allowed `scope` values:

- `canonical` (app-owned global KPIs)
- `sector` (shared within a sector)
- `company` (bespoke company-specific KPIs)

Allowed `valueKind` values:

- `monetary`
- `percentage`
- `ratio`
- `count`
- `physical`
- `duration`

Allowed `computation` values:

- `reported` (sourced directly)
- `derived` (computed at read time from other KPIs)

Rules:

- Sector values in company fundamentals match those used in company statement classification.

Uniqueness, seeded packs, and derived-metric computation rules are canonical in [Data Model § Company Fundamentals](data-model.md#company-fundamentals).

### KPI Relevance

KPI relevance records which metrics matter for a company over time.

```json
{
  "id": "relevance_company_gpw_cdr_net_revenue",
  "companyId": "company_gpw_cdr",
  "definitionId": "kpi_def_net_revenue",
  "status": "active",
  "source": "agent",
  "rank": "primary",
  "firstSeenPeriod": "2025-FY",
  "lastSeenPeriod": null,
  "createdAt": "2026-06-01T10:00:00Z",
  "updatedAt": "2026-06-01T10:00:00Z"
}
```

Allowed `status` values:

- `active`
- `archived`

Allowed `source` values:

- `user` (manually curated)
- `agent` (auto-detected or recommended)
- `sector` (from sector pack)

Allowed `rank` values:

- `primary`
- `secondary`

Field/storage rules (uniqueness, curation-lag behavior) are canonical in [Data Model § Company Fundamentals](data-model.md#company-fundamentals).

### Financial Fact

Financial facts are reported or derived values for a specific KPI in a specific period, with provenance and quality metadata.

```json
{
  "id": "fact_company_gpw_cdr_2025fy_net_revenue",
  "companyId": "company_gpw_cdr",
  "periodId": "period_company_gpw_cdr_fy2025",
  "definitionId": "kpi_def_net_revenue",
  "valueNumeric": "2456789000",
  "currency": "PLN",
  "statementBasis": "consolidated",
  "attribution": "total",
  "variant": "reported",
  "measureWindow": "point_in_time",
  "dataQuality": "final",
  "asReportedValue": "245 678,9 mln zł",
  "asReportedScale": "thousands",
  "reportingStandard": "ifrs",
  "extractionMethod": "manual",
  "confidence": null,
  "confirmationState": "confirmed",
  "supersedesId": null,
  "sourceDocumentRef": "feed_01",
  "createdAt": "2026-06-01T10:00:00Z",
  "updatedAt": "2026-06-01T10:00:00Z"
}
```

Allowed `statementBasis` values:

- `consolidated`
- `standalone`

Allowed `attribution` values:

- `total`
- `owners_of_parent`
- `nci` (non-controlling interests)

Allowed `variant` values:

- `reported`
- `adjusted`
- `constant_currency`
- `continuing`
- `discontinued`
- `net_of_cancellations`
- `lifo_ccs`

Allowed `measureWindow` values:

- `flow` (period flow)
- `point_in_time` (snapshot)
- `trailing` (TTM/LTM)
- `cumulative`
- `duration`

Allowed `dataQuality` values:

- `final`
- `estimated`

Allowed `extractionMethod` values:

- `manual`
- `ai_extracted`
- `api`
- `derived`

Allowed `confirmationState` values (the column is **frozen** — every new write is `confirmed`; facts are review-free, [ADR 0086](adr/0086-aggregator-primary-fundamentals.md) decision 5. `pending`/`auto_unreviewed` remain in the CHECK set only for historical rows and are never written by current code):

- `confirmed`
- `pending` (legacy)
- `auto_unreviewed` (legacy)

Field/storage rules (uniqueness, value encoding, supersession, derived-metric availability) are canonical in [Data Model § Company Fundamentals](data-model.md#company-fundamentals).

### Report Document Capture

Report documents represent financial statements, earnings reports, or other documents captured for a company.

```json
{
  "id": "doc_company_gpw_cdr_2025fy",
  "companyId": "company_gpw_cdr",
  "periodId": "period_company_gpw_cdr_fy2025",
  "sourceType": "official_report",
  "originRef": "feed_01",
  "url": "https://www.gpw.pl/pub/files/example/report.pdf",
  "localPath": "/local/documents/company_gpw_cdr/report_2025fy.pdf",
  "contentType": "application/pdf",
  "contentHash": "sha256:abc123...",
  "byteSize": 2456789,
  "title": "2025 Annual Report",
  "attribution": "GPW",
  "fetchStatus": "success",
  "fetchError": null,
  "fetchedAt": "2026-06-01T10:30:00Z",
  "createdAt": "2026-06-01T10:00:00Z",
  "updatedAt": "2026-06-01T10:30:00Z"
}
```

Allowed `sourceType` values:

- `official_report` (investor/official disclosure)
- `public_media`
- `manual` (user-uploaded or specified)

Allowed `fetchStatus` values:

- `pending` (capture requested, not yet fetched)
- `success` (document downloaded and stored)
- `failed` (fetch error; details in `fetchError`)
- `not_found` (source URL returned 404 or similar)

Rules:

- Report documents belong to exactly one canonical company.
- `periodId` is optional (for forward-looking documents or multi-period reports).
- `originRef` is a soft reference to the feed item from which the document was sourced.
- `localPath` is the app-owned **relative** storage path (under the `report_documents/` subtree) for fetched documents, so the store stays portable across machines and import/export. It is null when no file is stored.
- `contentHash` is SHA256 and enables deduplication and integrity checking.
- `title` is optional user-facing metadata; official report titles are preserved when available.
- `attribution` is source-level credit (e.g., "GPW", "Company Web Site").
- `fetchStatus` is `pending | fetched | failed | metadata_only`. **Full files are stored only for periodic/financial reports**; other ESPI/EBI attachments persist as `metadata_only` (URL + attribution, no bytes). The user-URL and IR-page rungs always store the full file. See [ADR 0036](adr/0036-report-document-storage-and-backfill.md).
- ESPI/EBI attachment ingestion lands in `v0.41.0`: attachments surfaced by the active Bankier company-komunikaty article path are upserted into `report_documents` (identity `(companyId, url)`), full file fetched only for periodic reports per the rule above.

#### On-Track History Backfill (`v0.41.0`)

- `backfill_company_history(input)`: explicit, user-triggered action (`{ companyId }`) that paginates the active official-report listing back over the configurable `backfillYears` depth (default 3, range 1–10; ADR 0077 §3) and ingests periodic reports + ESPI/EBI filings through the normal ingestion path, preserving original publication dates. Calendar entries are **not** backfilled (the forward-looking calendar adapters own upcoming events). Runs as an async, cancellable job; returns a job handle. On successful completion it **chains a history sweep** ([ADR 0077](adr/0077-trusted-extraction-foundations.md) §3) — best-effort (a chaining failure is logged, never fails the backfill) — so the fetched periods are automatically extracted without a second user action, and the returned `BackfillProgress.chainedSweepId` names that sweep (the row is created eagerly at enqueue time, so the id is known before the command returns; `null` when nothing was chained). The coverage panel polls THIS sweep by id, never "the latest sweep", so its status/AI-budget footer settle on the sweep the backfill started. See [ADR 0036](adr/0036-report-document-storage-and-backfill.md).
- `get_backfill_progress(input)`: returns progress/diagnostics for a running or completed backfill (`{ companyId }` or job id): pages fetched, items ingested, documents stored, errors, status, `chainedSweepId` (the auto-chained sweep's id, or `null`), and `truncated` — `true` when the page cap ended the fetch before the `backfillYears` cutoff was reached (older filings may be missing; surfaced as an explicit coverage-panel warning, never silent).
- **Typed failure diagnosis** (card bfc4c98): a failed backfill sets `status: "failed"` with `error` as `"<code>: <detail>"` — the distinct causes never collapse into one message. Codes: `unsupported_market` (the company's market has no history-capable adapter — NewConnect today, since the Bankier company path serves GPW only; ADR 0036), `not_tracked` (no Bankier backfill target for the id), `no_bankier_page` (the company's Bankier page/slug could not be resolved), `http_error` (the Bankier request itself failed), `parse_error` (a page was fetched but was unparseable), `internal` (a storage/internal fault unrelated to the source). The UI maps each prefix to a cause-specific localized message (`CompanyCoveragePanel`). A page fetched with **zero komunikaty** is not a failure — it completes with `itemsIngested: 0`. Every failure emits a `module=backfill … status=failed code=<code>` warn line; the three genuine adapter-interaction faults (`http_error`, `no_bankier_page`, `parse_error`) additionally record a typed source outcome on the `bankier-company-komunikaty` adapter (`last_error`), while pre-fetch eligibility failures (`unsupported_market`, `not_tracked`) do not — they are not adapter faults and must not flag the shared adapter's health.
- Backfill is **idempotent** — it reuses feed-item `(sourceAdapterId, sourceEventKey)`, report-document `(companyId, url)`, and signal `(feedItemId, category)` dedup, so re-runs and resumed partial runs never duplicate. Throttling obeys the existing Bankier rate policy (serialized, waits between pages/companies). Backfill is never automatic.

### Fundamentals Commands

Initial local commands:

- `list_kpi_definitions(input)`: returns KPI definitions, optionally filtered by scope, sector, or company. `companyId` WITHOUT a scope means "the catalog this company can see": shared rows (canonical/sector/user, `companyId` NULL) PLUS that company's own company-scoped rows — never another company's customs (owner-dogfooding fix 2026-07-22; a bare company filter used to hide the whole canonical catalog and the fact matrix synthesized placeholder definitions). With an explicit `scope` the exact filter applies unchanged.
- `create_kpi_definition(input)`: creates one KPI definition at any scope level. `statementGroup` (card #307, optional, default `other`): `income` | `balance` | `cash_flow` | `per_share` | `other` — validated vocabulary, typed refusal on any other value; unlike `origin`, nothing forces this field over MCP, so a caller may set it directly. `periodNature` (ADR 0100 decision 6, epic #398, optional, default `duration`): `instant` | `duration` — same validation shape as `statementGroup`; distinct from TTM eligibility (a `ratio`/`percentage` `valueKind` stays TTM-ineligible even when `duration`).
- `list_financial_periods(input)`: returns financial periods for one company, optionally filtered by fiscal year.
- `create_financial_period(input)`: creates one financial period record.
- `update_financial_period(input)`: updates period end date and report evidence reference.
- `delete_financial_period(id)`: removes one financial period (must not delete financial facts that reference it; cleanup is manual or backend-owned).
- `list_kpi_relevance(companyId)`: returns KPI relevance records for one company.
- `create_kpi_relevance(input)`: marks a KPI as relevant for a company.
- `update_kpi_relevance(input)`: updates relevance status, rank, and period tracking.
- `delete_kpi_relevance(id)`: removes one relevance record.
- `list_financial_facts(input)`: returns financial fact values, optionally filtered by company, period, or KPI definition.
- `create_financial_fact(input)`: records one financial metric value with full provenance.
- `update_financial_fact(input)`: updates fact values, data quality, and confirmation state.
- `delete_financial_fact(id)`: removes one financial fact.
- `capture_report_document(input)`: fetches and stores one report document for a company, returning success/error status.
- `list_report_documents(companyId)`: returns all captured report documents for one company. Each `ReportDocument` carries `docKind` — the classified taxonomy value (`periodic_ssf | periodic_jsf | auditor_opinion | presentation | governance | other`, [ADR 0077](adr/0077-trusted-extraction-foundations.md) §1) or `null` when unclassified (a row predating the taxonomy; classification runs at ingestion for new rows). The UI shows it as a per-row kind badge and filters the list by it.
- `reclassify_report_documents()`: recomputes the `doc_kind` taxonomy ([ADR 0077](adr/0077-trusted-extraction-foundations.md) §1) over every stored report document and returns `{ total, updated, byKind }` (`byKind` = final count per kind — `periodic_ssf | periodic_jsf | auditor_opinion | presentation | governance | other` — over all rows). Classification is deterministic Rust code, so the operation is **idempotent**: a row is rewritten only when its recomputed kind differs from the stored one, and a second run reports `updated == 0` with identical `byKind`. Documents are already classified at ingestion (insert/upsert set `doc_kind`); this command is the backfill/self-heal for rows predating the taxonomy. No input; offloaded (`spawn_blocking`). **UI entry point**: the report-documents panel's "Refresh classification" action (`CompanyReportDocumentsPanel`).
- The report-documents **view** read model — `get_report_documents_view(companyId)` — returns `ReportDocumentsView { companyId, rows: ReportDocumentViewRow[], totals: ReportDocumentCoverageTotals }`, one row per stored document tagged with the period it belongs to and whether it is that period's canonical report ([ADR 0077](adr/0077-trusted-extraction-foundations.md) §1/§2). An assembled read model (ADR 0044; no stored projection), offloaded (`spawn_blocking`, since ESEF period derivation reads the stored file). Each `ReportDocumentViewRow` is `{ document: ReportDocument, fiscalYear: number | null, periodType: string | null, canonical: boolean, extraction?: DocumentExtractionStatus }`:
  - Period fields come from the **same** `document_period` helper the coverage map uses (`derive_report_period` first, then the title/URL fallback), so the two panels can never disagree about a document's period. `fiscalYear`/`periodType` are `null` together when no period can be derived (the common case for non-periodic filings).
  - `canonical` is `true` only for a periodic document selected by `canonical_reports_per_period` over the same inputs the coverage map feeds it — so the panel's ★ marks the very document the coverage map names as the period's report.
  - `extraction` (#155) is the per-document "contains extractable financial data" indicator, aggregated over every `fundamentals_extraction_outcomes` slot recorded for the document: `{ status: "has_data" | "flagged" | "empty", factCount }` — any emitting slot wins (`has_data`, fact counts summed), else any flagged slot (`flagged`), else `empty` (attempted, nothing found). **Absent means never attempted** (no outcome rows — the data-model's "no row = never attempted" rule), so the panel renders no chip rather than a false "no data". UI: a `StatusChip` in the row's kind/chips group (`Financial data` with the fact count as tooltip / `Extraction flagged` / `No extractable data`), so the user knows where running Extract is worth it.
  - `totals` (#174) is the per-company coverage roll-up over the same rows: `{ documents, fetched, pending, metadataOnly, periodicCount, hasPeriodicCoverage }`. `periodicCount` counts `periodic_ssf`/`periodic_jsf` documents in **any** fetch state; `hasPeriodicCoverage` reuses the backfill catch-up's own predicate (`companies_lacking_periodic_coverage` — at least one **fetched** periodic document), so the panel and the backfill can never disagree. `periodicCount > 0` with `hasPeriodicCoverage = false` is the "we know the report exists but hold no bytes" state the roll-up exists to surface. There is deliberately **no** `lastBackfillReason`: nothing persists one today (the sweep logs it), and a header line does not justify a migration.
  - **UI entry point**: `CompanyReportDocumentsPanel` (the redesigned Report documents panel, [ADR 0077](adr/0077-trusted-extraction-foundations.md) §2 / mockup Panel B). By default it **groups documents by fiscal period** (newest first) with a "Group by period" toggle back to a flat list; within a group the periodic statements come first (a ★ on the canonical one), then audit reports, then a fold hiding the signature/data companions (a companion whose "Extract data" action is available is never folded); non-periodic filings collect in a collapsed "No period" group. A search field filters across title/filename, and the kind filter + "Refresh classification" action stay. Fetched via the `getReportDocumentsView(companyId)` wrapper (`src/api/reportDocuments.ts`).

Input shapes follow the corresponding domain types above. Return shapes include all domain fields plus timestamps. Company fundamentals data must be treated as owner-durable state in import/export and backup workflows.

Structured-first extraction commands ([ADR 0061](adr/0061-deterministic-fundamentals-data-gathering.md); generated DTOs `RunStructuredExtractionInput` / `RerunExtractionOutcomeInput` / `StructuredExtractionSummary` / `FactProvenance` / `ExtractionOutcome`):

- `run_structured_extraction(input)`: `{ companyId, reportDocumentId, fiscalYear, periodType, periodEnd, mode? }` → runs the deterministic tiered pipeline (ESEF → EspiCoverNote → `html_aggregator`; the PDF fact-extraction arm AND the positional/tier-3b arm are both retired — [ADR 0086](adr/0086-aggregator-primary-fundamentals.md) decision 1/3, [ADR 0095](adr/0095-retire-html-positional-tier.md) — so a routed-PDF OR routed-positional document takes no rung and emits no outcome) over one stored report document and persists accepted facts with provenance; returns `{ acceptance, tier, emitted, producedFactIds, skippedFactIds, divergentCount, reasonCode }` (the retired PDF profile-drift arm no longer returns a `driftJson` — [ADR 0086](adr/0086-aggregator-primary-fundamentals.md) decision 1). `reasonCode` (issue #244) mirrors the recorded outcome row's typed `reason_code` — the summary's home for "why nothing was produced", wired into the zero-effects `ExplainsEffect` verdict; `null` only for the benign routed-PDF/positional path, which records no outcome. The pipeline is deterministic end to end — the tier-4 OCR fallback is retired with the in-app AI layer ([ADR 0084](adr/0084-retire-in-app-ai-layer.md) decision 4), so a document no tier can read is an explicit flagged gap, never guessed. **Every run that takes a rung — emitting or not — records a `fundamentals_extraction_outcomes` row** with its typed `reasonCode` and any drift payload (the benign routed-PDF/positional early return above is the ONE deliberate exception: no rung is taken and no outcome row is recorded) (ADR 0061 decision 2's guardrail; see `list_flagged_extraction_outcomes` below and [data-model](data-model.md)). Re-extraction is **idempotent** (T7-F): a fact whose uniqueness slot already holds the same value is a re-observation — counted in `skippedFactIds`, never re-inserted; a slot holding a **different** value is never silently overwritten — the stored fact wins, the divergence is counted in `divergentCount` and recorded as a `diagnostic_events` entry for review. Offloaded (`spawn_blocking`). `mode` is the trust-ladder mode (`autopilot` | `assist`, default `autopilot`; any other value is rejected); every emitted fact lands `confirmationState = confirmed` in both modes — facts are review-free ([ADR 0086](adr/0086-aggregator-primary-fundamentals.md) decision 5), and acceptance strength (`accepted`/`accepted_via_witness` vs `accepted_unreviewed`) is provenance (`validationStatus`), never a confirmation state. Period supplied by the caller (the KPI-extraction flow, which knows the detected period). **Headless-only by design — no direct UI entry point** (issue #153 closure): the callers are the autopilot KPI-extraction flow, the server-side wrappers `extract_report_document_data` (the UI's one-click action, which derives the period) and `rerun_extraction_outcome`, and the MCP port; the UI never invokes it directly because it would have to invent the reporting period.
- `extract_report_document_data(input)`: `{ companyId, reportDocumentId, mode? }` → **the reachable one-click "Extract data" action** on a report-document row (the company workspace's Report documents panel) — closes the ADR 0061 S5 live-path gap where the deterministic pipeline had no UI caller outside autopilot. Derives the reporting period **server-side** (the same `derive_report_period` the autopilot stage uses: ESEF self-derived `FY`; PDF from title/URL period classification), so the UI never invents `fiscalYear`/`periodType`/`periodEnd`, then runs `run_structured_extraction` and returns the same `StructuredExtractionSummary`. Offloaded (`spawn_blocking`). Same `mode`/confirmation semantics as `run_structured_extraction` (facts land unchanged — pending vs auto-committed by `mode` + validation outcome). Errors when the period can't be derived (no stored file, unparsable ESEF, or a PDF with no classifiable period). **UI entry point**: `CompanyReportDocumentsPanel` extract action.
- `list_fact_provenance(factIds)`: returns the `FactProvenance` rows (`factId`, `sourceTier`, `validationStatus`, `driftJson`, `citation`, plus the corroboration stamp `witnessValue` / `witnessPageUrl` / `corroboratedAt` — epic #229 T5, `null` = never corroborated, never "disagreed") for the requested facts; facts predating the pipeline have no row (render as unvalidated).
- `list_flagged_fact_provenance(input?)`: `{ companyId? }` → `FlaggedFact[]` — every fact whose provenance verdict is `flagged`, **scoped to one company** when `companyId` is given and app-wide when it is omitted/`null` (the MCP data-quality surface keeps the unscoped read). Each row carries the review context, not just the id: `{ factId, companyId, metricKey, label, valueNumeric, currency, fiscalYear, periodType, sourceTier, validationStatus, driftJson, citation }` (a superset of `FactProvenance`; epic #229 T5 — the old id-and-tier shape is why the read had no UI consumer). Lists flagged **facts**, which by construction exist only where something *was* emitted — for the periods where nothing was, use `list_flagged_extraction_outcomes`. **UI entry point**: the Coverage panel's **Flagged figures** section (`src/shared/components/CoverageFlaggedFacts.tsx`) via the `listFlaggedFactProvenance(companyId)` wrapper — each row shows period · metric (localized KPI label) · formatted value, the reader that produced it as a chip, and the citation; loading / the good "nothing flagged" empty state / an explicit retryable read failure are three distinct states. Informational only — facts are review-free ([ADR 0086](adr/0086-aggregator-primary-fundamentals.md) decision 5), so the section asks nothing of the user.
- `list_flagged_extraction_outcomes(input)`: `{ companyId }` → `ExtractionOutcome[]` — the company's **non-emitting** extraction outcomes, newest attempt first: the periods where the deterministic pipeline ran and refused to emit (ADR 0061 decision 2 "flagged, never silently stored"; [ADR 0084](adr/0084-retire-in-app-ai-layer.md) decision 4/6). Each row: `{ id, companyId, reportDocumentId, fiscalYear, periodType, periodEnd, tier, acceptance, reasonCode, detailJson, driftJson, structureChanged, factCount, attemptCount, firstAttemptedAt, lastAttemptedAt }`. `tier` is `null` when no deterministic tier could read the document. `reasonCode` is a **typed** code the frontend renders through the translation layer, never backend prose (`validation_failed` · `structure_drift` · `witness_disagreement` · `witness_fallback` · `no_deterministic_tier` · `no_period_derived` · `document_unreadable`; the emitting value `emitted` never appears in this read). `no_deterministic_tier` and `structure_drift` remain valid CHECK values (legacy rows keep them, and `no_deterministic_tier` still emits for markup/ESEF gaps a tier declines to read) but a **PDF** document no longer produces either: since [ADR 0086](adr/0086-aggregator-primary-fundamentals.md) decision 1 retired the PDF fact-extraction arm, a routed-PDF document generates no extraction attempt and thus no outcome row at all — `structure_drift` is retired outright (never newly produced by any route). **`witness_fallback`** is **legacy only** ([ADR 0086](adr/0086-aggregator-primary-fundamentals.md) decisions 2–4 retired the ADR 0085 aggregator gap-fill seam): no run produces it anymore — BiznesRadar sources core KPIs through its own primary pull, under the tier precedence, never through an extraction run. Stored rows (with their `witnessFallback` detail payloads) remain readable and re-armable. `witness_disagreement` is likewise no longer produced by extraction runs — it is recorded by the reversed-witnessing paths (the BR-primary pull and the WDF ingest seam) against a held slot that is an issuer tier (`esef`/`structured_xhtml`/`espi_cover_note` — every `SourceTier::is_issuer` tier; the retired positional `pdf` tier's stored rows, pre-[ADR 0095](adr/0095-retire-html-positional-tier.md) deletion, historically also qualified) OR, for the BR pull, a `manual` slot ([ADR 0086](adr/0086-aggregator-primary-fundamentals.md) decision-4 amendment, 2026-07-22 — the user's own entry is untouched but the conflict is logged, tier `manual`). `detailJson` carries the failing identities/cross-checks with expected/actual/residual where the gate produced them — the reversed-witnessing paths use the **same canonical gate shape** (`{ failedIdentities, failedCrossChecks, witnessDisagreements }`), so the Coverage panel renders it as investor language, never raw JSON keys. Clean periods are excluded, and **absence of a row means "never attempted"** — a flagged period is never indistinguishable from an untouched one. Persistence rules: [data-model](data-model.md). **UI entry point**: the Coverage panel's **Flagged periods** section (`src/shared/components/CoverageFlaggedPeriods.tsx`), fetched via the `listFlaggedExtractionOutcomes(companyId)` wrapper (`src/api/fundamentalsExtraction.ts`) — it renders each row's period, attempting tier, and the typed `reasonCode` **translated to plain language** (a raw code never reaches the user), expands onto the failing-check detail, and marks structure drift with a chip. The `no_period_derived` **sentinel period** (`fiscalYear 0`, empty `periodType`/`periodEnd`) renders as "Period unknown", never as a real period. Loading, the good "nothing flagged" empty state, and a failed read (explicit + retryable) are three distinct states — a failed read must never look like "nothing flagged". See [ui-information-architecture](ui-information-architecture.md) § Flagged periods.
- `rerun_extraction_outcome(input)`: `{ outcomeId, mode? }` → `StructuredExtractionSummary` — the **"try again" action on a flagged period**. Takes the company/document/period from the stored outcome row rather than asking the caller to re-derive them, so the retry can never target a different slot than the one displayed. **Per-metric slot refs are resolved to their real document**: a row keyed by the synthetic `documentId#metricKey` (a `value_divergence`, epic #229 T5) re-extracts `documentId` — the suffix is bookkeeping, not identity. A **`witness_disagreement`** row is the one non-re-runnable case: its slot ref is an aggregator **page URL**, so there is no stored document to re-read, and the command refuses it with the typed **`rerun_not_applicable: …`** code (previously an opaque storage error) — such a divergence is resolved by a fresh aggregator pull or a manual correction, never by re-extraction. The UI does not offer the action there at all (see below). The re-run **updates that same row in place** (`attemptCount` increments): a period whose cause has been fixed leaves the flagged list instead of leaving a stale flag beside a fresh success. Offloaded (`spawn_blocking`). Same `mode`/confirmation semantics as `run_structured_extraction`. Errors when no outcome with that id exists. **UI entry point**: the per-row **"Try again"** action in the Coverage panel's Flagged periods section, via the `rerunExtractionOutcome(input)` wrapper (`src/api/fundamentalsExtraction.ts`). Every re-run action disables while one is in flight (no double dispatch), and the list refetches on completion so a fixed period leaves it.
- `run_aggregator_fundamentals_pull()`: no input → `AggregatorPullSummary { companies, pagesResolved, pagesUnavailable, factsWritten, factsUpdated, factsReobserved, slotsSkippedHigherTier, witnessDisagreements, witnessCorroborations, zeroCellsSkipped, pagesEmpty, noDefinition, mappingSuspects }` (`witnessCorroborations` — epic #229 T5: issuer/manual-held slots the aggregator AGREED with, stamped on `financial_fact_provenance`; counts as an effect in the zero-effects verdict) (`pagesEmpty` — issue #244: pages that resolved but parsed to zero cells, a layout change or an empty table — distinct from `pagesUnavailable` (never fetched) and `zeroCellsSkipped` (cells read, all zero); wired into the zero-effects verdict) (the last: metrics whose aggregator value contradicted issuer/manual-held slots at ≥5 distinct companies this run — a dictionary-mapping suspect, testing.md § Mapping guardrails) — runs the **BiznesRadar-primary core-KPI pull** ([ADR 0086](adr/0086-aggregator-primary-fundamentals.md) decision 2) over every tracked company: resolves the three robots-allowed report pages per company (income / balance / cash flow; per-(company, page kind) daily cadence cache — a re-run inside the window fetches nothing), parses **every** period column each page carries, and writes facts under the tier precedence (`manual` > `esef` > `espi_cover_note` > `html_aggregator`; the positional `pdf` tier once ranked here too, retired [ADR 0095](adr/0095-retire-html-positional-tier.md); the aggregator only ever overwrites its own slot; a divergence against a held slot — any issuer tier, or a `manual` slot — records an informational `witness_disagreement` in the canonical gate-detail shape, ADR 0086 decision-4 amendment; **agreement** against the same held slot stamps a positive corroboration on its provenance row and upgrades an ISSUER slot's `passed`/`unreviewed` verdict to `witness_confirmed`, epic #229 T5 — a manual slot is stamped but never re-graded, and the aggregator never witnesses its own slot; an empty/zero cell is never written). Idempotent; offloaded. **Serialized with the daily queue job** (issue #132): the command takes the same per-adapter `biznesradar-fundamenty` lock the queue dispatch holds, so an on-demand run can never double-fetch against an in-flight pull — a racing call fails fast with `aggregator_pull_already_running: …` (retry after the in-flight pull finishes; `rebuild_fundamentals` pass 1 collects the same busy error into `errors` without aborting its other passes). **Headless-only for now**: consumed by the daily scheduler auto-trigger (`jobs/scheduler.rs` → durable-queue job `aggregator_fundamentals_pull`, `sources` lane) and the upcoming rebuild flow (ADR 0086 decision 6) — no direct UI entry point yet; the rebuild slice (plan TOR C C4) surfaces it.
- `rebuild_fundamentals()`: no input → `RebuildFundamentalsSummary { aggregator: AggregatorPullSummary, esefDocumentsProcessed, esefFactsEmitted, esefFactsReobserved, esefDivergences, esefErrors, wdfCarriersScanned, wdfFactsWritten, wdfSkippedNoBody, factsByTier: FactTierBreakdown { byTier: TierFactCount[] { sourceTier, facts }, manualOrUnprovenanced }, errors: string[] }` — the **one-off fundamentals repopulation driver** ([ADR 0086](adr/0086-aggregator-primary-fundamentals.md) decision 6, plan TOR C slice C4). After the owner-approved **manual** wipe of `financial_facts` on the live DB (a deliberate one-time SQL cleanup — NOT a migration; migrations stay schema-only and never delete user data), it runs three sequential passes, each tolerant of per-item failure (errors collected into `errors`, never aborting the rebuild): (1) the BiznesRadar-primary pull (`run_aggregator_fundamentals_pull` core — cached pages parsed without refetch); (2) ESEF re-extraction, force-re-running the deterministic structured extraction over every stored ESEF-route document of every company (PDFs spawn no attempt, post-C3 semantics); (3) WDF re-scan of every stored feed item whose "WYBRANE DANE FINANSOWE" cover-note body survives (pruned bodies are lost by design, counted in `wdfSkippedNoBody`). Facts land review-free (`confirmed`) under the same tier precedence; idempotent (a re-run writes zero new facts). `esefFactsEmitted` counts ESEF facts written **or tier-upgraded** this pass — an ESEF fact may take over a slot pass 1's aggregator pull wrote (issuer tier outranks `html_aggregator`), so it is not "genuinely new"; `factsByTier` is the deduplicated verdict. `factsByTier` is the **before/after verdict** — post-rebuild fact totals grouped by `source_tier` plus the manual / no-provenance bucket. Offloaded. **Headless-only** (rebuild driver — invoked once via live-drive after the manual wipe; no UI entry point).
- `get_fundamentals_coverage(companyId)`: returns `FundamentalsCoverage { companyId, periods: CoveragePeriodRow[] }`, the per-company **coverage map** ([ADR 0077](adr/0077-trusted-extraction-foundations.md) §2). An **assembled read model** (the ADR 0044 pattern — the map itself is computed on demand from the live tables, **no stored projection**; cardinality is a handful of periods per company); offloaded (`spawn_blocking`, since a first-time period derivation reads the stored file). The **per-document period derivation** (`derive_report_period`) is served from a persisted cache (`document_derived_periods`, migration 0109), so repeat panel loads read the index instead of re-extracting bare-titled PDFs' cover pages (data-model). Each `CoveragePeriodRow` is one `{ fiscalYear, periodType }` period:
  - Cell `report` (`CoverageReportCell | null`) — the single **canonical periodic report** for the period (ssf-over-jsf, newest revision; ADR 0077 §1 selection over the **stored `doc_kind` column** + `derive_report_period` — the kind is never re-derived on the fly, so coverage cannot disagree with the documents panel; a `NULL` (unclassified) document is excluded until set-on-write / "Refresh classification" converges it). `{ documentId, docKind (periodic_ssf | periodic_jsf), title, structured, fetched }`; `fetched = fetch_status == "fetched"`, so a link-only (metadata-only) periodic report still yields a cell with `fetched: false` via the title/URL period fallback. `null` when no periodic report names the period.
  - Cell `facts` (`CoverageFactsCell`) — `{ total, validated, unvalidated, flagged }` over the period's `financial_facts` joined to `financial_fact_provenance`: `validated` = provenance `passed`/`witness_confirmed`, `flagged` = provenance `flagged`, `unvalidated` = everything else (no provenance row, `unreviewed`, `none`). `total = validated + unvalidated + flagged`.
  - Cell `review` (`CoverageReviewCell`) — `{ flaggedFacts }`: mirrors `facts.flagged` — flagged deterministic facts are the only review surface ([ADR 0084](adr/0084-retire-in-app-ai-layer.md) decision 5 removed the proposals input with the KPI staging ledger).
  - Field `skippedBudget` (`boolean`) — `true` when the period's canonical report's `trigger='history_sweep'` autopilot run recorded `reason: "skipped_budget"` on its `kpiDeltaJson` (a budget-denied tier-4, [ADR 0077](adr/0077-trusted-extraction-foundations.md) §6). Run ids are per-`(company, document)` deterministic, so there is at most one run per document; a later successful extraction clears the flag two ways (facts appear → non-gap; the run's delta is overwritten). Tolerant: an absent/garbled delta, or any other `reason`, reads `false`.
  - **Period-union rule**: a period appears iff at least one of {a canonical report, ≥1 fact} names it — there is no proposal queue ([ADR 0084](adr/0084-retire-in-app-ai-layer.md) decision 5); rows are sorted newest-first (DESC by `fiscalYear`, then period index `Q1<H1<Q3<FY`).
  - **UI entry point**: the **Coverage panel** (`src/shared/components/CompanyCoveragePanel.tsx`, T2.2) — a company-scoped cockpit pane (kind `coverage`, label "Coverage") seeded into the curated company dashboard. It renders one table row per period (Period / Report / Data / To review); clicking a row opens the company's Report documents pane. Fetched via the `getFundamentalsCoverage(companyId)` wrapper (`src/api/fundamentalsCoverage.ts`), reloading on `companyId` change. Its **history-actions footer** (T3.2) drives "Backfill history" and "Extract missing periods" (below), with a lean status line and a live drain counter while a sweep runs — the sweep is fully deterministic, so there is no AI-call spend to report ([ADR 0084](adr/0084-retire-in-app-ai-layer.md)).

#### History Sweep (`v0.51.0`, [ADR 0077](adr/0077-trusted-extraction-foundations.md) §3)

The history sweep is the backfill/manual counterpart to the refresh-time detection sweep: it enqueues a full autopilot run (`trigger='history_sweep'`, with the sweep's row id stamped as the run's `sweep_id`) for every canonical periodic report whose period still lacks accepted facts. It runs only for a company opted into automation (mode ≠ `off`); a company in mode `off` ends the sweep with `skippedReason='automation_off'` — never a silent skip. Sweep runs are fully deterministic — no AI tier, no per-sweep budget ([ADR 0084](adr/0084-retire-in-app-ai-layer.md); legacy `skipped_budget` markers on stored runs stay readable). Durable state lives in the `history_sweeps` table ([data-model.md](data-model.md)). DTOs `HistorySweep` / `HistorySweepProgress`.

- `run_history_sweep(companyId)`: starts a **manual** history sweep ("Extract missing periods" — the case where documents are already fetched and only extraction is missing, no re-download). Gates on the company existing and mode ≠ `off` (`company_not_found` / `automation_off` errors; the UI disables the button in mode `off`, the command stays honest for a direct call), creates a `manual` sweep row, enqueues the durable `history_sweep` job, and returns the `HistorySweep`. Offloaded (`spawn_blocking`). **UI entry point**: the Coverage panel footer's "Extract missing periods" action.
- `get_history_sweep_progress(companyId)`: returns `HistorySweepProgress { sweep: HistorySweep | null, runsTotal, runsDone, runsFailed }` — the company's latest sweep plus per-run progress derived from its enqueued run ids (terminal = `succeeded`/`partial`/`failed`; `runsFailed` counts `failed`). A null `sweep` means the company has never been swept. Offloaded (`spawn_blocking`). **UI entry point**: polled by the Coverage panel footer's status line after a backfill or sweep.

#### Version-Aware Re-extraction (epic #398 Item B, [ADR 0100](adr/0100-two-layer-tagged-fact-capture-and-ifrs-vocabulary.md))

Existing tagged filings do not widen by themselves: the history sweep deliberately never re-arms a run that already emitted facts (`jobs::autopilot::terminal_run_should_rearm`), so a widened crosswalk/projection only reaches NEW documents unless the user explicitly asks for a re-extraction. A batch selects the company's **successful ESEF-tier runs** (`extractionAvailable:true`, `tier:"esef"`) whose stored `pipelineVersion` is below `EXTRACTION_PIPELINE_VERSION`, and re-arms each through the SAME primitives the sweep's own re-arm uses (`rearm_run` + `autopilot::enqueue_first_stage`) — but a SEPARATE candidate selector and durable record (`pipeline_reextraction_batches`, [data-model.md](data-model.md)), so the sweep's "never re-arm an emitted run" rule stays untouched. Re-armed runs are stamped `trigger='manual'` (reuses the existing generic user-triggered bucket rather than widening `autopilot_run.trigger`'s CHECK for one more value with identical semantics). **NOT gated on automation mode** — it reprocesses already-stored documents on explicit request, the same posture as the per-period "Try again" action (`rerun_extraction_outcome`), not new automation. DTOs `PipelineReextractionBatch` / `PipelineReextractionProgress`.

- `run_pipeline_reextraction(companyId)`: starts a re-extraction batch. Gates only on the company existing (`company_not_found`), creates a `queued` batch row, enqueues the durable `pipeline_reextraction` job, and returns the `PipelineReextractionBatch`. Offloaded (`spawn_blocking`). **UI entry point**: the Coverage panel footer's "Re-extract with latest pipeline" action.
- `get_pipeline_reextraction_progress(companyId)`: returns `PipelineReextractionProgress { batch: PipelineReextractionBatch | null, runsTotal, runsDone, runsFailed }` — the company's latest batch plus per-run progress derived from its re-armed run ids (terminal = `succeeded`/`partial`/`failed`). A null `batch` means the company has never had one. Offloaded (`spawn_blocking`). **UI entry point**: polled by the Coverage panel footer's status line after a re-extraction.

#### Raw Tagged-Fact Coverage + Promotion ([ADR 0100](adr/0100-two-layer-tagged-fact-capture-and-ifrs-vocabulary.md) decision 10, epic #398 final slice)

The Layer 1 trust surface: a compact read model over what `report_tagged_facts` (data-model.md) captured, plus the ONE action decision 10 opens onto it — promoting a captured-but-uncurated concept into that company's Fundamentals. Company-scoped only; the promote command is permanently excluded from the MCP registry ("the owner may promote… a machine still may not").

- `get_report_tagged_fact_coverage(companyId)`: returns `TaggedFactCoverageCounts { rawStored, projected, dimensional, noteLevel, awaitingName, conflicting }` — everything Layer 1 captured for the company, split into what the deterministic projection rule keeps and the reasons the rest is not (yet) in Fundamentals; see data-model.md for the exact bucket semantics (`projected` is a re-derivation, not a `financial_facts` read). Offloaded (`spawn_blocking`) — it re-runs `project_period` per `(report_document, period_end)`. **UI entry point**: the Coverage panel's compact `CoverageRawCapture` line (`src/shared/components/CoverageRawCapture.tsx`) — renders nothing for a company with no tagged capture yet, so the line never shows a confusing all-zero grid.
- `list_uncrosswalked_concepts(companyId)`: returns `UncrosswalkedConceptRow[]` — every concept this company captured with no crosswalk entry, ranked by `companyCount` (how many companies across the WHOLE corpus tag it — the global harvest signal, [ADR 0100](adr/0100-two-layer-tagged-fact-capture-and-ifrs-vocabulary.md) decision 2), each carrying `occurrenceCount` (at this company), `statementGroup`/`periodNature`, `humanLabel`, `labelSource` (`issuer` | `technical` — a typed code the frontend renders through `text()`, never backend prose), `alreadyPromoted`, `promotedDefinitionId`. Offloaded. **UI entry point**: `CoverageUncrosswalkedConcepts.tsx`, behind the Coverage line's disclosure toggle.
- `promote_uncrosswalked_concept(companyId, conceptLocalName)`: returns `PromotedConcept { definitionId, metricKey, label, labelSource, factsProjected }` — ensures a `scope='company'` `kpi_definitions` row (idempotent) named per decision 10 (the issuer's own package label, or the raw technical concept name explicitly marked `labelSource: "technical"` — never invented), then projects the company's already-captured Layer 1 rows for that concept into `financial_facts` (repeat-vs-conflict resolved the same way decision 4 does; a genuine value disagreement is never written). Errors `company_not_found` / `concept_not_captured`. Offloaded. **UI entry point**: the "Show in Fundamentals" button on each `CoverageUncrosswalkedConcepts` row; a promoted row swaps to an "In Fundamentals" chip in place, no refetch required.

### Quality Frameworks

Quality frameworks ([ADR 0046](adr/0046-quality-frameworks-quantitative.md), `v0.44.0`) are user-owned checklists of criteria expressed in a free-text DSL over KPI metric keys, evaluated deterministically against confirmed `financial_facts` into a versioned scorecard. The same expression engine that evaluates `kpi_definitions.formula` evaluates criteria; the criterion grammar adds comparators (`>= <= > < == ~=`), boolean `AND`/`OR`/`NOT`, and percent literals. The user-facing grammar reference is `wiki/dsl-reference.md`. Criteria are decision-support only and must not encode buy/sell/hold output.

Domain types follow the `quality_frameworks` / `framework_criteria` / `framework_evaluations` / `criterion_results` shapes in [data-model.md](data-model.md). Commands:

- `list_quality_frameworks()`: returns all frameworks (origin, version, criteria counts).
- `get_quality_framework(id)`: returns one framework with its criteria.
- `create_quality_framework(input)`: creates a `user`-origin framework.
- `update_quality_framework(input)`: updates name/description; bumps `version`.
- `delete_quality_framework(id)`: removes a framework and its criteria/evaluations (any origin is deletable).
- `clone_framework(input)`: `{ frameworkId, name? }` → duplicates any framework into a new `user`-origin framework with `clonedFrom` set.
- `reset_framework_to_template(id)`: `app_template`-origin only; re-derives the framework's criteria from the shipped Rust template constant. Errors with `not_a_template` for `user`-origin frameworks.
- `create_framework_criterion(input)`: `{ frameworkId, label, expression, weight?, partialBand?, ordinal? }`; validates the expression, caches the AST.
- `update_framework_criterion(input)`: updates label/expression/weight/ordinal; re-validates and re-caches the AST.
- `delete_framework_criterion(id)`: removes one criterion.
- `validate_criterion_expression(expression)`: parses without evaluating; returns `{ ok, error?, referencedMetricKeys[] }` for live editor feedback. Errors carry a human-readable parse message.
- `evaluate_framework(input)`: `{ frameworkId, companyId }` → runs the engine over the company's latest available period/TTM, writes an immutable `framework_evaluations` + `criterion_results` snapshot, and returns the scorecard. `verdict = unavailable` when a referenced metric cannot be computed (missing fact), distinct from `fail`.
- `list_framework_evaluations(input)`: `{ frameworkId, companyId }` → returns the evaluation history (latest first).
- `get_framework_evaluation(id)`: returns one evaluation with its `criterion_results`.
- `delete_framework_evaluation(id)`: removes one evaluation run and its `criterion_results` from the history. This is history pruning, not snapshot mutation — the remaining runs stay immutable.
- `list_available_metric_keys(input?)`: returns the computable metric keys (catalog + derived + `user`-scope) with labels/units, for the criteria editor's discovery/autocomplete.

Frameworks, criteria, and any `user`-scope `kpi_definitions` a criterion references are owner-durable state carried in the import/export bundle so an exported framework imports cleanly; evaluations are reproducible snapshots whose export is optional.

### Qualitative Assessment

Status: retired ([ADR 0084](adr/0084-retire-in-app-ai-layer.md) decision 5)

A quality framework may hold **qualitative** criteria (moat, pricing power, recurring revenue, capital-allocation quality…) that cannot be reduced to a metric comparison ([ADR 0075](adr/0075-qualitative-assessment-frameworks.md), `v0.50.0`). A qualitative criterion is authored in-app like any other criterion: it carries `kind: "qualitative"` and an owner-authored `assessmentGuidance` prompt seed instead of a DSL expression (`create_framework_criterion` / `update_framework_criterion` accept optional `kind` and `assessmentGuidance`; `expression` stays empty for qualitative rows).

The in-app AI assessment run — the durable `qualitative_assessment` job, the per-criterion AI request, and its current-state read — is **removed entirely** (migration `0102`, clean cut). Verdicts now arrive via `set_qualitative_verdicts`, an MCP write tool with mandatory provenance (`v0.60.0`, BYOA): the user's own agent supplies `verdict` (`pass | partial | fail | insufficient_evidence`), short `reasoning`, `citations` (typed evidence refs — `evidenceType`, `evidenceId`, `label`, `snippet`), and `confidence` (`low | medium | high`) for one criterion+company, and the write stamps `source: "agent"`. The write requires a non-empty `citations` array, and every citation's `(evidenceType, evidenceId)` must resolve to an existing row of that evidence type (typed per-table existence check in the shared persist path, #343 — one bad citation refuses the whole batch atomically; existence is app-wide, not company-scoped — #345 tracks company-membership tightening); the stored array is canonicalized (camelCase keys, trimmed values — exactly what was validated). Uncited or fabricated provenance is never stored ([ADR 0088](adr/0088-mcp-surface-v2-ui-parity.md)). Agent results are opinion, never facts — they never mutate quantitative data, and merge into the same immutable evaluation snapshot as quantitative results (per-criterion `source`); `criterion_results` otherwise keeps only its deterministic `source='engine'` DSL rows, which the cut does not touch.

`get_framework_evaluation` / `list_framework_evaluations` return the qualitative fields **as snapshotted in that specific run** — the audit/history view of one evaluation, unchanged and immutable (a snapshot may be quant-only, qual-only, or combined, so "the latest snapshot" alone is not a reliable source of qualitative rows).

### AI KPI Extraction — retired ([ADR 0084](adr/0084-retire-in-app-ai-layer.md))

AI-generated KPI extraction is removed entirely (decision 5): generation, the staging ledger, and the review surface are gone, with `kpi_extraction_jobs`/`kpi_extraction_proposals` dropped by migration `0102`. The deterministic fundamentals pipeline (ESEF → EspiCoverNote → `html_aggregator` BiznesRadar-primary) is the only extractor now — the PDF fact-extraction arm and the positional/tier-3b arm are both retired too ([ADR 0086](adr/0086-aggregator-primary-fundamentals.md) decision 1, [ADR 0095](adr/0095-retire-html-positional-tier.md)) — so a stored PDF or a bare non-iXBRL render takes no rung at all, and a document no surviving tier parses is flagged rather than guessed.

### IR-Page Report Resolution

The report-document source ladder ([ADR 0029](adr/0029-ir-page-report-resolution.md)) is: ESPI/EBI attachment (primary), per-company IR reports page (fallback), manual PDF URL paste (last resort).

- `get_company_ir_reports_url(companyId)` / `set_company_ir_reports_url(companyId, url)`: read/write the durable per-company IR reports page URL (empty clears it).
- `resolve_ir_report(input)`: fetches the company's IR page and extracts candidate links generically (no per-company scrapers), ranked most-report-like first. **Candidates-only** ([ADR 0084](adr/0084-retire-in-app-ai-layer.md) decision 1 retired the AI pick and its confidence-gated auto-capture): the command returns `{ candidates }` and never writes to `report_documents` itself — choosing from the ranked list is the user's (or their MCP agent's) call.

## UI-Facing Command Boundaries

Initial Tauri command groups:

- `health`
- `list_companies`
- `create_company`
- `list_watchlists`
- `list_watchlist_memberships`
- `create_watchlist`
- `rename_watchlist`
- `delete_watchlist`
- `add_company_to_watchlist`
- `remove_company_from_watchlist`
- `list_feed_items`
- `update_feed_item_state`
- `list_research_evidence`
- `list_company_timeline`
- `list_watchlist_timeline`
- `mark_research_scope_reviewed`
- `list_research_review_state`
- `list_research_questions`
- `create_research_question`
- `update_research_question`
- `delete_research_question`
- `list_evidence_links`
- `create_evidence_link`
- `delete_evidence_link`
- `list_research_reminders`
- `create_research_reminder`
- `update_research_reminder`
- `delete_research_reminder`
- `refresh_sources`
- `refresh_source`
- `refresh_gpw_company_registry`
- `refresh_gpw_company_registry_if_stale`
- `list_company_registry_entries`
- `list_unmatched_source_items`
- `get_scheduler_status`
- `list_notebook_entries`
- `create_notebook_entry`
- `update_notebook_entry`
- `list_kpi_definitions`
- `create_kpi_definition`
- `list_financial_periods`
- `create_financial_period`
- `update_financial_period`
- `delete_financial_period`
- `list_kpi_relevance`
- `create_kpi_relevance`
- `update_kpi_relevance`
- `delete_kpi_relevance`
- `list_financial_facts`
- `create_financial_fact`
- `update_financial_fact`
- `delete_financial_fact`
- `capture_report_document`
- `list_report_documents`
- `backfill_company_history`
- `get_backfill_progress`
- `confirm_derived_event`
- `create_video_transcript_job`
- `list_video_transcript_jobs`
- `delete_video_transcript_job`
- `update_video_transcript_job`
- `run_video_transcript_job`
- `resolve_transcript_job_company`
- `list_transcript_segments`
- `create_note_from_transcript_selection`
- `get_provider_credential_status`
- `set_provider_api_key`
- `clear_provider_api_key`
- `get_settings`
- `update_settings`
- `export_research_data`
- `preview_research_import`
- `apply_research_import`
- `export_settings_data`
- `preview_settings_import`
- `apply_settings_import`

## Import And Export

M20 exposes typed commands for portable user-owned setup and research data. It is not a full backup/restore feature.

Research-data export returns a JSON document:

```json
{
  "schemaVersion": 1,
  "exportedAt": "2026-06-05T12:00:00Z",
  "appVersion": "0.20.0",
  "sections": ["companies", "watchlists", "notebooks", "research"],
  "companies": [],
  "watchlists": [],
  "memberships": [],
  "notebookEntries": [],
  "researchQuestions": [],
  "evidenceLinks": [],
  "aiResearchBriefs": [],
  "aiResearchBriefCitations": []
}
```

Settings export returns a YAML document with `schemaVersion`, `exportedAt`, `appVersion`, and an allowlisted `settings` object.

Typed commands:

- `export_research_data() -> ExportPayload`
- `preview_research_import({ contents }) -> ImportPreview`
- `apply_research_import({ contents }) -> ImportApplyResult`
- `export_settings_data() -> ExportPayload`
- `preview_settings_import({ contents }) -> ImportPreview`
- `apply_settings_import({ contents }) -> ImportApplyResult`
- `write_export_file(input) -> string` — `{ path, contents, allowedExtensions, defaultExtension }` (`WriteExportFileInput`): writes an export payload to the **dialog-selected** absolute path and returns the final path. The extension whitelist is enforced backend-side (a path missing an allowed extension gets `defaultExtension` appended); a relative/empty path or an empty whitelist is rejected (`invalid_export_path: …`), an IO failure surfaces as `export_write_failed: …`. Offloaded (`spawn_blocking`). **This command replaced the unscoped `fs:allow-write-text-file` capability** (issue #106): the webview holds no filesystem permission at all — the export write is the strict-permissions posture's typed-command path. Denylisted on the MCP surface (an agent never writes arbitrary files on the owner's machine). **UI entry point**: the Import/Export settings export buttons (`saveExportFile` in `ImportExportSettings.tsx`).

Rules:

- Research-data JSON includes companies, watchlists, memberships, notebook entries, research questions, and evidence links (the AI research-brief tables are gone — [ADR 0084](adr/0084-retire-in-app-ai-layer.md)).
- Settings YAML includes only allowlisted non-secret settings.
- Import preview validates schema version, references, setting keys, setting values, and duplicate note behavior before apply.
- Apply must reject invalid preview states and must be transactional for each import operation.
- Companies match by `qualifiedTicker`; existing local company fields win and missing optional fields may be filled.
- Watchlist IDs are preserved when absent locally. Existing watchlist IDs merge memberships while keeping local name and description.
- Membership companies must resolve from existing companies, companies included in the import, or an explicit future repair result. Placeholder companies are not created.
- Notebook entries import for existing or included companies. Duplicate notebook entry IDs are skipped with preview warnings.
- Notebook origins preserve source URL and label metadata even when referenced feed/transcript records are not part of M20 export.
- Provider secrets, API keys, license tokens, private signing material, logs, diagnostics, metrics, feed items, transcripts, and full backup data are excluded.
- Review checkpoints are excluded from research import/export because they are local review-progress state. Research questions and evidence links are included because they are user-owned research content.

Initial `refresh_gpw_company_registry` behavior:

- Legacy command name retained for frontend compatibility; behavior now refreshes the required company-directory adapter set.
- Runs through the same async blocking-task boundary as other source refresh commands so long network/directory refreshes do not block the app UI.
- Runs the GPW company directory adapter and the NewConnect company directory adapter.
- Fetches the public GPW companies page with a high limit, currently `https://www.gpw.pl/spolki?offset=0&limit=500`, so the cache represents all currently listed GPW companies exposed by that page.
- Fetches the public NewConnect companies page with a high limit, currently `https://newconnect.pl/spolki?offset=0&limit=500`, because the base page renders only the first 10 rows.
- Stores registry rows in SQLite under `company_registry_entries`.
- Upserts by `exchange + ticker`.
- Preserves user-managed `companies` records and does not overwrite them silently.
- Records adapter attempt/success/error state under each concrete directory adapter.
- Automated tests use test-sample-backed parser/fetch behavior; default checks do not depend on live GPW availability.

Initial `refresh_gpw_company_registry_if_stale` behavior:

- Runs only for scheduler-triggered refreshes.
- Runs through the async blocking-task boundary and must not block the app UI while checking or refreshing directory sources.
- Checks required company-directory adapter freshness before making live requests.
- Uses the directory adapter poll interval, initially one day, as the stale threshold.
- Returns no refresh result when all required directory caches are still fresh.
- Does not run immediately on app startup; the first scheduled check happens after one full registry poll interval while the app is open.

Initial `list_company_registry_entries` behavior:

- Returns active cached GPW and NewConnect company directory rows from SQLite.
- Includes source adapter ID, exchange, ticker, qualified ticker, display name, ISIN, source URL, fetched timestamp, and whether the company is already tracked locally.
- Supports the Companies form registry suggestions and the Sources screen registry detail panel.
- The Companies form can use cached registry matches to fill exchange, ticker, display name, and ISIN while preserving manual company entry.
- Each Sources directory row shows its own collapsed company list, searchable by ticker/company/ISIN, and each untracked company can be added to the local company list.
- Does not fetch live data by itself; refresh is handled by `refresh_gpw_company_registry` or lookup bootstrap behavior.

Initial `lookup_company` behavior:

- Looks up companies from the local `company_registry_entries` cache across all active supported company-directory sources, initially GPW main market and NewConnect.
- Uses exact ticker first, exact ISIN second, and company-name search only for company-form lookup/enrichment.
- The submitted exchange is a disambiguation preference, not a hard filter. If the form still says `GPW` and the ticker exists only in NewConnect, lookup returns the `NC:<ticker>` result and fills the form with `NC`.
- If a supported directory lookup misses while the required directory cache is empty, the command may refresh required company directories once and retry the lookup.
- Feed/source matching remains stricter than form lookup: ticker first, ISIN-to-ticker registry resolution second, exact ISIN fallback, and no silent company-name matching.

Initial `refresh_sources` behavior:

- Runs the enabled v1 feed-source adapter paths, currently Bankier Giełda RSS and Bankier Company Komunikaty.
- `gpw-espi-ebi` remains registered but disabled while Bankier Company Komunikaty is the active v1 official-report source; GPW may be revisited if Bankier proves unreliable.
- Fetches Bankier Giełda RSS headlines from `https://www.bankier.pl/rss/gielda.xml` as public media items.
- Fetches Bankier per-company komunikaty JSON only for tracked GPW companies, after resolving and caching Bankier tag IDs.
- Fetches Bankier per-company article pages for matched komunikaty rows so official-report body text and attachments come from the article/report page, not from listing filter tags.
- Manual refresh is available from the topbar, Sources screen, and no-feed Inbox empty state.
- The desktop runtime schedules refreshes in-app while the UI is open, using the SQLite `pollIntervalSeconds` setting.
- Scheduled refreshes do not run immediately on startup; each enabled source adapter gets its own first scheduled run after one full poll interval plus randomized startup jitter.
- Refreshes are guarded against overlap, so a scheduled refresh is skipped while another refresh is already running.
- Refresh attempts record their trigger as `manual` or `scheduler` in source adapter state.
- Scheduled refreshes back off after repeated refresh failures; manual refresh remains available during backoff.
- Settings exposes an editable source poll interval with accepted values of 5 minutes, 15 minutes, 30 minutes, and 1 hour.
- Sources UI shows the expected next in-app poll based on the active interval.
- Refresh results and adapter status expose GPW detail-body counters: attempted, stored, and failed.
- Adapter status exposes the last detail warning when a GPW detail fetch fails or the parser rejects a detail page as unusable.
- Refresh commands must not prune old feed items from any source.
- Automated tests continue to use bundled GPW listing test samples or injected fetchers; default checks must not require live network access.

Feed mass-delete (`prune_old_feed_items`, `delete_unsaved_feed_items`) — removed (#329, owner decision 2026-08-05): feed content persists; no bulk destructive feed actions.

Initial `refresh_source` behavior:

- Runs one source adapter by adapter ID through the same ingestion path used by `refresh_sources`.
- Supports enabled adapters `bankier-market-rss`, `bankier-company-komunikaty`, and `gpw-company-registry`; `gpw-espi-ebi` returns a disabled-source error for now.
- Maps `gpw-company-registry` to the registry refresh path and returns a source-ingestion-shaped summary for UI consistency.
- Rejects disabled placeholders such as `portal-analiz`, `bankier-firma-rss`, and `bankier-wiadomosci-rss` without attempting network access.
- Records adapter attempt/success/error state with the supplied `manual` or `scheduler` trigger.
- Sources UI exposes a per-source manual refresh action from each source detail panel.
- Sources UI exposes a per-source access label such as Public RSS, Public JSON, Authenticated, Paywalled, Manual/local, or Disabled.
- Bankier RSS tests use a bundled RSS test sample or injected fetcher; default checks must not require live Bankier availability.
- Bankier company-komunikaty tests use bundled HTML/JSON test samples or injected fetchers; default checks must not require live Bankier availability.
- Records `last_attempt_at` in source adapter state before fetching.
- Returns source ingestion status with adapter ID, fetched count, created count, matched count, unmatched count, and fetched timestamp.
- A successful fetch with zero parsed listings is recorded as a successful refresh with zero counts and a generated fetched timestamp.
- Persists the last fetched, created, matched, and unmatched counts in source adapter state for later Sources-screen diagnostics.
- On fetch failure, records adapter `lastErrorAt` and `lastError` in SQLite before returning the command error.
- The topbar refresh control exposes the latest refresh failure state until the next successful refresh attempt.
- Refreshes local SQLite feed/source state only; scheduler and live polling are separate M5 slices.

Initial `list_unmatched_source_items` behavior:

- Returns recent stored feed items for one source adapter that do not have a company match.
- Supports Sources-screen diagnostics after a refresh reports unmatched items.
- Must not make unmatched items visible in normal Inbox or company workspace views.
- Returns source URL, source-derived company name, title, publication timestamp, and fetched timestamp when available.

Feed, job, transcript, and notebook changes should be emitted as Tauri events.

## External Surface — MCP Server (capability-tier registry)

A second **driving adapter** ([ADR 0039](adr/0039-ports-and-adapters-posture.md)) over the same domain models: an in-app, localhost-only MCP server ([ADR 0078](adr/0078-mcp-external-surface.md)) — stateless MCP Streamable HTTP subset on `POST http://127.0.0.1:<port>/mcp`, bearer-token auth, off by default, app-open-only lifetime. **Two credentials** ([ADR 0099](adr/0099-acquisition-mcp-surface-mechanics.md) dec. 2), each an OS-keychain token shown once at generation: the **primary** bearer resolves the `Full` scope (the whole exposed surface); the optional **`kpi_acquisition`** bearer resolves the acquisition scope — exactly the nine ingest-workflow tools (empty until they land, #384–#386), and only while the `kpiAcquisitionEnabled` setting is on (off ⇒ that token is rejected at auth like an unknown one, reads included). `tools/list` and `tools/call` are scope-filtered; a tool outside the caller's scope is `-32602` unknown — the surface does not exist for that identity. Auth is oracle-free: every bearer does the same digest + gate-read work before classification, and a gate-read failure fails closed for the acquisition scope only, never for a valid primary. A thin `brawler-mcp-stdio` binary forwards newline-delimited JSON-RPC from stdin to the HTTP endpoint for stdio-only clients (a scoped session is just a different `--token`).

**The registry rule ([ADR 0088](adr/0088-mcp-surface-v2-ui-parity.md)).** Every MCP exposure is a **registry entry** in `src-tauri/src/mcp/registry.rs` — `{ tool_name, command_name, tier, provenance }` — over the typed command layer; no hand-written per-tool wrappers. `tools/list` and `tools/call` route through the registry (never a hand-rolled match). Tool input/output JSON Schemas are **generated from the serde types** by `schemars` (ADR 0088 dec. 1) — never hand-written; the `tools/list` insta snapshot remains the **frozen contract** (ADR 0078 G-1), and regenerating it is a reviewed spec change. **Every Tauri command carries exactly one capability tier**; a command absent from the registry fails the classification gate (a source-scan test over the real `invoke_handler` inventory), so no command can silently leak into — or stay out of — the MCP surface undecided.

| Tier | What | MCP posture |
| --- | --- | --- |
| `read` | Domain reads (companies/watchlists, feed, signals, facts + provenance, coverage, quotes, ownership, insiders, analyst recs, health/red flags, reports/diffs/transcripts, notes, claims, expectations, journal, research questions, quality frameworks, calendar, autopilot runs, attention, briefing). | Active whenever the server is enabled. |
| `act` | Research writes, workspace actions, and job triggers. | Gated at `tools/call` by the live `mcpWritesEnabled` setting (**default OFF**) → typed `writes_disabled` when off; **provenance mandatory** on writes carrying a carrier (`origins` / `sourceEvidenceId` / `citationsJson` / manual-fact citation), checked BEFORE the handler and rejected with typed `provenance_required` if empty (ADR 0088 dec. 3). Both are domain failures (`isError: true`), never protocol errors. |
| `excluded` | Deletes, undo, bulk import/backup, settings/credentials mutations, MCP self-management, dev/diagnostic mutations. | Permanent denylist — UI-only, never reachable over MCP. |

**Current exposure:** **50 `read` tools** (the 4 MVP composites + the per-domain reads incl. the ADR 0089 valuation trio, the two triage reads, the acquisition reads `list_pending_kpi_ingests`/`get_kpi_ingest_status` (#384) + `get_kpi_ingest_context`/`get_kpi_ingest_document` (#385), and the raw-capture reads `get_report_tagged_fact_coverage`/`get_pipeline_reextraction_progress` (ADR 0100 dec. 11, #398)) and **64 `act` tools** (incl. the acquisition acts `start_kpi_ingest`/`cancel_kpi_ingest` (#384) + `stage_kpi_observations`/`validate_kpi_ingest`/`commit_kpi_ingest` (#386)) are dispatchable **on the `Full` scope**; the acquisition scope's allowlist, `KPI_ACQUISITION_TOOLS`, carries exactly the NINE workflow tools at their contract positions — complete since #386, frozen by its own `tools_list_schema_acquisition` snapshot with the ≤16 KiB byte gate. Acquisition-scope act calls bypass the `mcpWritesEnabled` gate — their gate is `kpiAcquisitionEnabled` at authentication (ADR 0099 dec. 2); the same tools called on the Full scope stay writes-gated — the authoritative itemization is `FROZEN_EXPOSED_TOOL_COUNT` in `src-tauri/src/mcp/registry.rs` plus the frozen `tools_list_schema` snapshot; this paragraph records the split, never the inventory (it drifted to "41/55" once — the registry is the count's single home). The agent fact writes shipped by epic #285 ([ADR 0093](adr/0093-agent-acquisition-tier-and-preliminary-lifecycle.md)) — `record_financial_facts` (batch, per-fact citations, `agent` tier, T7) and `capture_report_document` (security-gated fetch, T8) — remain live but are **demoted to low-level repair tools** ([ADR 0098](adr/0098-mcp-native-kpi-acquisition-lifecycle.md), #365): the normal ingestion path is the run-based workflow (see *Planned* above), and the tool descriptions route agents by capability (`start_kpi_ingest` present → run workflow only). Read handlers live in `src-tauri/src/mcp/reads.rs`; **read** company-scoped tools take a **qualified ticker** (`GPW:CDR`, bare ticker when unambiguous), resolved internally. Outputs are the domain read models — decision support only, never buy/sell/hold ([ADR 0042](adr/0042-advisory-verdict-port-and-open-core-boundary.md)).

**Result envelope (ADR 0088 amendment, issue #249).** The MCP spec requires `structuredContent` to be a JSON **object**, so a strict client rejects a bare-array result. Every successful `tools/call` payload therefore passes one envelope at the single serialization choke point (`mcp::tools::run`): a command returning a top-level **array** (every `list_*`) is wrapped as `{ "items": [...] }`, a bare **scalar** as `{ "result": ... }`, and an **object** passes through verbatim. The exposed-tool umbrella tests assert every success payload is an object, so a new tool cannot regress this.

MVP composites (unchanged shapes):

- `get_company_dossier { company }` — identity + fundamentals coverage + confirmed facts slice + scorecard summary.
- `search_research { query, company?, limit? }` — the unified FTS search read model.
- `list_claims_due { company? }` — management claims to verify (the `list_claims_to_verify` read model).
- `get_quality_assessment { company }` — qualitative assessment + framework evaluations.

Read wave, per domain (tool name = backing command; `{ company }` = required qualified ticker):

| Domain | Tools |
| --- | --- |
| Companies / watchlists | `list_companies`, `get_company_basic_info { company }`, `list_watchlists`, `list_watchlist_memberships` |
| Feed / signals / events | `list_feed_items`, `list_company_signals { company }`, `list_company_events { company }` |
| Facts / periods / KPIs | **`list_financial_facts { company }`** (see below), `list_financial_periods { company }`, `list_kpi_definitions`, `list_flagged_fact_provenance` |
| Quotes / ownership / insiders / analysts | `get_price_context { company }`, `get_ownership_overview { company }`, `get_insider_overview { company }`, `list_short_positions { company }`, `get_analyst_recommendations { company }` |
| Health / red flags | `get_company_health { company }`, `get_red_flags { company }` |
| Reports / diffs | `get_report_documents_view { company }`, `list_report_diff_candidates { company }`, `get_report_diff { olderReportDocumentId, newerReportDocumentId }` |
| Transcripts | `list_video_transcript_jobs { company? }`, `list_transcript_segments { transcriptJobId }` |
| Notes / claims | `list_notebook_entries { company }`, `list_management_claims { company }` |
| Expectations / journal / questions | `list_report_expectations { company? }`, `list_decision_entries { company? }`, `list_research_questions` |
| Calendar / attention / briefing / autopilot | `list_report_season`, `list_attention_events { company?, includeDismissed? }`, `get_latest_morning_briefing`, `list_autopilot_runs { company?, limit? }`, `get_autopilot_run { runId }` |
| Quality frameworks | `list_quality_frameworks` |
| Triage (ADR 0088 dec. 4) | `list_flagged_extraction_outcomes { company }` (per-period extraction-coverage gaps), `list_unclassified_filings { company?, limit? }` (official filings the rule classifier could not place; classify one with `classify_filing`) |
| Raw capture (ADR 0100 dec. 11) | `get_report_tagged_fact_coverage { company }` (how much of the company's tagged filings reached Fundamentals, and where the rest went), `get_pipeline_reextraction_progress { company }` (the latest re-extraction batch's progress) |

**Facts + provenance (mandatory carrier, ADR 0088 dec. 2).** `list_financial_facts { company }` returns `{ company, facts: [ …FinancialFact, sourceTier?, validationStatus?, citation? ] }` — every fact is joined (via `fundamentals_provenance().get_many`) with its trust-ladder provenance: `sourceTier` (e.g. `deterministic_esef` / `aggregator` / `manual`), `validationStatus` (`ok` | `flagged`), and free-form `citation`. The three fields are `null` only when a fact has no provenance row. This is the one composition in the MCP read layer.

Deliberately **not exposed** (still `read`-tier, each justified in `registry.rs`): infra/diagnostics/logs/metrics, settings/credentials/adapter config, reference/autocomplete plumbing (`lookup_company`, `list_company_sectors`, criterion-editor helpers), surfaces superseded by a richer tool (`list_report_documents` → `get_report_documents_view`; `list_fact_provenance` → folded into `list_financial_facts`), import/export dumps, research-timeline aggregates, and the unmatched-source triage surface `list_unmatched_source_items` (a separate family; no MCP tool this slice). The flagged-extraction and unclassified-filings triage surfaces are now **exposed** (M4, above).

Read tool inputs are strict (`deny_unknown_fields` ⇒ `additionalProperties: false`, camelCase field names); tool failures map onto the [Error codes](#error-codes) set (ADR 0070) — an unknown ticker is `not_found`, an ambiguous bare ticker is `invalid_input`.

### Act tier (writes, ADR 0088 dec. 2/3, M3)

**Call-time gating.** Act tools are **always listed** in `tools/list` (discoverability; their description notes the write toggle) — the gate is at `tools/call` dispatch (`registry::call`), in order: (1) read the **live** `mcpWritesEnabled` setting — if off, return typed **`writes_disabled`** (`isError: true`, the handler never runs, nothing is written); (2) if the row carries a provenance requirement, run `validate_provenance` on the raw arguments BEFORE the handler — an empty carrier returns typed **`provenance_required`** naming the missing field. Both are domain failures in the tool result, never JSON-RPC protocol errors.

**Self-enable is impossible.** `update_settings` is `excluded` from the registry, so a connected agent can never flip `mcpWritesEnabled` itself — only the user can, in Settings → MCP server. This is the durable safety property behind default-OFF.

**Internal ids, not tickers.** Unlike reads, act tools reference entities by the **internal ids** the read tools return in every payload (e.g. `Company.id`, a period id, a framework/criterion id) — an agent reads the workspace, then writes against the ids it saw. Handlers live in `src-tauri/src/mcp/acts.rs`; each binds to the same `AppState`/sub-facade write the Tauri command delegates to (ADR 0039), so the MCP and UI write paths cannot diverge. When the command wraps additional logic in an extracted `<command>_impl` helper (e.g. `create_company_impl`'s GPW quote-backfill enqueue), the act handler routes through **that same helper**, never the bare storage write — guarded by a source-scan test over `src/commands/` (issue #250).

**Provenance carriers.** Enforced where the input carries the carrier and a new provenance-bearing datum enters: `create_notebook_entry` (`origins`), `create_note_from_transcript_selection` (`transcriptSegmentIds` — the selection is the origin), `create_management_claim` / `update_management_claim` (`sourceEvidenceId`), `create_financial_fact` / `update_financial_fact` (`sourceDocumentRef` only — `attribution` is the fact's slot dimension, never an alternate citation carrier, epic #285 T9), `set_qualitative_verdicts` (every `results[].citationsJson` non-empty), `record_financial_facts` (`DocumentAndPerFactCitations`: a non-blank `reportDocumentId` AND a non-blank `citation` on every entry of `facts` — a single blank citation refuses the WHOLE batch before any write). **No carrier** on `update_notebook_entry` (origins are immutable from creation; the update input has no origins field) and `set_claim_verdict` (a verdict's evidence is the optional `verifyingFactId`, absent for a qualitative claim) — provenance integrity is enforced at create.

**Exposed act catalog** (59 tools; tool name = backing command):

| Group | Tools |
| --- | --- |
| Research writes (provenance) | `create_notebook_entry`, `create_note_from_transcript_selection`, `update_notebook_entry`, `create_management_claim`, `update_management_claim`, `set_claim_verdict`, `create_financial_fact`, `update_financial_fact`, `set_qualitative_verdicts`, `record_financial_facts` |
| Research writes (no carrier) | `capture_report_document` (see below), `create_research_question`, `update_research_question`, `create_evidence_link`, `create_research_reminder`, `update_research_reminder`, `create_decision_entry`, `create_report_expectation`, `update_report_expectation`, `record_expectation_resolution`, `create_company_event`, `create_kpi_definition`, `create_kpi_relevance`, `update_kpi_relevance`, `create_quality_framework`, `update_quality_framework`, `create_framework_criterion`, `update_framework_criterion`, `create_alert_rule`, `update_alert_rule` |
| Workspace actions | `create_company`, `create_watchlist`, `add_company_to_watchlist`, `remove_company_from_watchlist`, `update_feed_item_state`, `mark_report_prepared`, `mark_report_processed`, `mark_research_scope_reviewed`, `confirm_company_signal`, `reject_company_signal`, `classify_filing` (unclassified-bucket triage; `feedItemId` is the evidence anchor, no provenance carrier — ADR 0088 dec. 4), `confirm_derived_event`, `acknowledge_red_flag`, `set_ownership_holder_type`, `mark_attention_event_seen`, `dismiss_attention_event`, `set_autopilot_run_notification_state` |
| Job triggers (light / fail-fast) | `evaluate_framework`, `compute_comparative_valuation` (ADR 0089), `set_alert_rule_enabled`, `trigger_autopilot_run`, `generate_morning_briefing` |
| Job triggers (networked / heavy) | `refresh_sources`, `refresh_source`, `run_aggregator_fundamentals_pull`, `backfill_company_history`, `run_structured_extraction`, `rerun_extraction_outcome`, `run_pipeline_reextraction` (ADR 0100 dec. 11 — re-arms the company's landed ESEF runs whose stored pipeline version is stale; poll `get_pipeline_reextraction_progress`) — gated identically; **invocation-exempt** in the hermetic `every_exposed_act_tool_is_listed_and_gated` umbrella (they run live source/extraction/backfill work with no hermetic seam), exercised by the **M6 live dogfooding ritual** instead |

Act commands **classified but not exposed** (each justified inline in `registry.rs`): niche period plumbing (`create/update_financial_period`), `clone_framework`, company-config setters (`set_company_ir_reports_url`, `set_company_sector`, `rename_watchlist`), report-pipeline document machinery (`fetch_report_document`, `extract_report_sections`, `reclassify_report_documents`, `resolve_ir_report`, `extract_report_document_data`, `resolve_transcript_job_company` — `capture_report_document` itself is now exposed, T8 below), the video-transcript lifecycle (`create/update/run_video_transcript_job` — the only in-app AI dependency), and the **admin / one-off job triggers** (`backfill_company_health_facts`, `backfill_ownership_extraction`, `run_history_sweep`, `rebuild_fundamentals`, `refresh_gpw_company_registry(_if_stale)`) — whole-corpus rebuilds, quote-history sweeps, registry refresh, and derived-fact backfills the exposed extraction/refresh tools already cover.

**`set_qualitative_verdicts { frameworkId, companyId, results: [{ criterionId, verdict, reasoning, citationsJson, confidence }] }`** — the qualitative-verdict WRITE path (successor to the in-app agent writer retired by [ADR 0084](adr/0084-retire-in-app-ai-layer.md) dec. 5). A typed command (`Result<FrameworkEvaluation, CommandError>`, async + `spawn_blocking`) that is **MCP-first / headless — no UI entry point**; it and the MCP `act` handler share `build_persist_qualitative_input` (resolves each result's `ordinal`/`label` from the framework criteria, writes `prompt_version = "mcp"`), persisting one immutable qualitative snapshot via `persist_qualitative_assessment`. Registry: `act` + `CitationsJson`. Read the result back via the `get_quality_assessment` read tool.

**`capture_report_document { companyId, url, periodId?, originRef?, title?, attribution? }`** — registers and fetches the document an agent read, by URL, so `record_financial_facts` has a `reportDocumentId` to cite (ADR 0093 dec. 5, epic #285 T8). Always writes `sourceType: "user_url"`; the field itself is not in the input schema — an agent that sends `sourceType` is refused with the standard `deny_unknown_fields` protocol error, never a silent override (an agent must never mint an `espi_attachment`/other ingest-only row). The fetch runs through a dedicated gated policy (`document_fetcher::HttpDocumentFetcher::agent_capture()`), never the unrestricted fetcher every ingest caller (source refresh, backfill, autopilot, structured extraction, the UI capture command, report-diff fetch-on-demand) keeps using unchanged: **https-only** (any other scheme is a typed refusal), an **SSRF guard** — the host's DNS-resolved addresses must all be public (private `10/8` `172.16/12` `192.168/16`, loopback `127/8`/`::1`, link-local `169.254/16`/`fe80::/10`, and unspecified addresses are rejected), re-checked on **every redirect hop** (reqwest follows redirects by default; a naive guard that only checks the original URL is a no-op against a 302 to an internal address), a **content-type allowlist** (`application/pdf` | `text/html` | `application/xhtml+xml`, parameters tolerated) checked on the response header before any body bytes are read, and a **30 MiB cap** enforced during streaming (never trusting `Content-Length`, which the server can lie about or omit). Idempotent on `(companyId, url)` — a repeat call returns the existing row. A fetch refusal is never a protocol error: the document row is still created (`fetchStatus: "failed"`, the error recorded), so a retry with a corrected URL reuses the same id. Response: the domain `DocumentCaptureResult { documentId, localPath?, success, error? }` verbatim (no MCP-specific reshaping). Registry: `act`, no provenance carrier (the URL itself is the payload) — `writes_disabled` still applies.

**`record_financial_facts { companyId, reportDocumentId, period: { fiscalYear, periodType, periodEnd? }, dataQuality?, facts: [{ metricKey, valueNumeric, currency?, attribution?, measureWindow?, citation }] }`** — **low-level batch fact write** ([ADR 0098](adr/0098-mcp-native-kpi-acquisition-lifecycle.md), demoted in #365): while the run-workflow tools (#353) are absent this remains the only supported temporary report-ingest route; once present it is repair-only — the normal path is start→stage→validate→commit. Originally the batch fact-acquisition write (ADR 0093 dec. 6, epic #285 T7), **MCP-only — no Tauri command, no UI entry point** (like the four MVP read composites; `tool_name == command_name` in the registry). `companyId`/`reportDocumentId` are internal ids; `facts` is capped 1..=100. Whole-batch typed refusals BEFORE any write: the batch-size cap, a `facts[].attribution` token outside `total | owners_of_parent | nci`, an unknown `companyId`, an unknown `reportDocumentId` — plus the provenance gate above (a blank citation anywhere). Otherwise the fiscal period is ensured (idempotent `finper_` id, so `periodId` is always returned even when every fact is skipped) and each fact is judged by the shared `jobs::supplied_set::validate_supplied_set` service (history plausibility + same-period accounting identities) before commit through `record_structured_fact` with `sourceTier: "agent"` / `extractionMethod: "mcp_agent"` / `confirmationState: "confirmed"` (ADR 0093 dec. 1 honesty rule — an agent write never masquerades as manual). Response: `{ periodId, outcomes: [{ metricKey, outcome, detail?, plausibility? }], completeness?: { expected, matched } }`. `outcome` is one of `created | reobserved | upgraded | divergent | no_definition | implausible | identity_violation` — `divergent` reports a disagreement against an issuer/manual-held slot without overwriting it; `identity_violation` (a fact is one of a broken same-period accounting identity's inputs) is never written, mirroring the deterministic pipeline's own `Flagged` ⇒ "do not emit" precedent; `implausible` (≥100× off stored history) is never written; `abstained_thin_history` (<2 history points) IS written (`validationStatus: "unreviewed"`) with `plausibility: "abstained_thin_history"` on its outcome — an honest abstention, never a silent pass. `dataQuality: "preliminary"` (ADR 0093 dec. 2/3) records issuer pre-report releases; a later `final` batch into the same slot stamps `supersedes_id` at creation (both rows coexist — the uniqueness slot includes `dataQuality`). Registry: `act` + `DocumentAndPerFactCitations`.

**Typed commands (UI management surface):**

- `regenerate_mcp_token` — generates + stores the token (keychain slot `brawler/mcp/auth_token`), returns `McpTokenGenerated { token, status }` — the plaintext exactly once; the token never appears in logs or any status payload. **Restart-on-rotation** ([ADR 0099](adr/0099-acquisition-mcp-surface-mechanics.md) dec. 2): the command then restarts the listener from keychain truth, so the new digest is live immediately (the old bearer stops authenticating); the restart outcome is NOT in the response — the UI fetches a fresh `mcp_status` after every rotate/revoke. **Read-back rule:** regenerate succeeds only when the post-store read-back equals the generated plaintext — an environment-provided credential ("rotate the env var") and a write-only keychain both get a typed `conflict`; the server never runs a token storage does not report.
- `revoke_mcp_token` — removes the token from the keychain and restarts the listener (which then stops with a status error — no token); returns the post-clear `CredentialStatus`. Revoke cannot remove an environment-provided credential (dev-only).
- `mcp_token_status` — `CredentialStatus` for the token (`providerId: "mcp"`, `secretKind: "auth_token"`): configured?, storage, dev env fallback (`BRAWLER_MCP_TOKEN`) availability. Never carries the token.
- `regenerate_kpi_acquisition_token` / `revoke_kpi_acquisition_token` / `kpi_acquisition_token_status` — the same lifecycle for the acquisition credential (keychain slot `brawler/mcp/kpi_acquisition_token`, `secretKind: "kpi_acquisition_token"`, env fallback `BRAWLER_MCP_KPI_ACQUISITION_TOKEN`), including restart-on-rotation and the read-back rule; revoking it keeps the server running with the scope unavailable.
- `set_mcp_enabled { enabled }` — persists `mcp.enabled` **and** starts/stops the listener live (no app restart); returns the resulting `McpStatus`. Enabling without a token or on an occupied port is a clean refusal (`running: false` + `error`), never a crash.
- `mcp_status` — `McpStatus { running, port, error, kpiAcquisitionConfigured }`; bind failure / missing token surface here, never as a crash. `port` is the **actually-bound** port while running, else the configured next-start port. `kpiAcquisitionConfigured` is the acquisition scope's **effective** availability: false when not running, when its token is absent/unreadable, or when its digest collides with the primary (equal secrets fail closed — ADR 0099 dec. 2).

All token commands return `Result<T, CommandError>` (ADR 0070; keychain failures map via `From<CredentialError>` — caller-input problems → `invalid_input`, backend/persistence failures → `internal`).

Settings: `mcp` group (`enabled` — default false; `port` — default 8317, clamped to `[1024, 65535]` on write and read; `writesEnabled` — default false, the `act`-tier gate, ADR 0088 M3; `kpiAcquisitionEnabled` — default false, the acquisition-SCOPE gate at auth, ADR 0099 dec. 2) in `UserSettings` (`mcp: { enabled, port, writesEnabled, kpiAcquisitionEnabled }`) / `UpdateSettingsInput` (`mcpEnabled?`, `mcpPort?`, `mcpWritesEnabled?`, `kpiAcquisitionEnabled?`); tolerant reads (missing rows fall back to defaults), upsert writes (no seed-row migration). `update_settings` is itself `excluded` from the MCP registry, so a connected agent can never enable its own writes or its own scope — only the user, in Settings → MCP server. **Port-change semantics:** changing `mcp.port` via `update_settings` while the server is running takes effect on the **next start** (disable→enable, or the next app open) — no hot-rebind; `mcp_status.port` reports the port the listener is actually serving. Lifetime is app-open-only (started in the `lib.rs` setup closure when enabled, torn down with the process).
