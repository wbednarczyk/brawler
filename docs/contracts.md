# Contracts

This file defines initial contracts for the first implementation. Field names are intentionally stable enough for code scaffolding, but exact serialization may be refined with tests before the first API release.

Doc map: [CLAUDE.md](../CLAUDE.md) § Required Reading. Related references: [Architecture](architecture.md), [Data Model](data-model.md), [Source Strategy](source-strategy.md), and [Product Spec](product-spec.md).

## Command Conventions

This file states the wire shapes, commands, and command-specific rules for every entity; **field-level storage rules (uniqueness, FKs, soft references, retention) are canonical in [Data Model](data-model.md)** — each section below points there instead of restating them. Conventions shared by every section, stated once:

- **Structure.** A section shows the wire JSON shape(s) first, then allowed enum values, then `Rules:` (command-specific behavior), then the typed Tauri commands (`Commands:`/`Typed commands:`/"Initial local commands:").
- **Errors.** Typed commands surface failures as a typed command error the frontend maps to a user-facing message; there is no bare-string/panic error path across the Tauri boundary. Async work (jobs, extraction, backfill) reports failure through job status fields (`status: "failed"`, `errorCode`, `error`) rather than a rejected command call.
- **Scope.** Commands accept and return the **canonical id** (`companyId`, `watchlistId`, etc.), never a raw ticker or display string; canonical identity and uniqueness rules live in [Data Model](data-model.md). Company-scoped vs watchlist-scoped behavior is called out per section only where it differs from this default.

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

- `list_source_adapters(input?)`: returns adapter metadata/status rows above. `input` is `{ includeDeveloperOnly? }`; normal callers omit it and get only `required`/`optional` adapters, per the `visibility` rule below.
- `set_source_adapter_enabled(input)`: `{ adapterId, enabled }` → toggles a `userConfigurable` adapter and returns its updated metadata row.

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

## Claim Extraction

AI claim extraction with mandatory user confirmation ([ADR 0040](adr/0040-management-claims-tracker.md)), mirroring KPI extraction. Sources are report documents and transcripts.

```json
{
  "id": "claim_job_01",
  "companyId": "company_gpw_cdr",
  "sourceType": "transcript",
  "sourceId": "transcript_77",
  "providerId": "gemini",
  "model": "gemini-2.5-flash",
  "promptVersion": "claims-v1",
  "status": "succeeded",
  "errorCode": null,
  "error": null,
  "createdAt": "2026-05-28T13:00:00Z",
  "startedAt": "2026-05-28T13:00:02Z",
  "finishedAt": "2026-05-28T13:00:20Z",
  "proposals": [
    {
      "id": "claim_prop_07",
      "jobId": "claim_job_01",
      "statement": "Management expects the next major release in the next two quarters.",
      "dueFiscalYear": 2026,
      "duePeriodType": "Q4",
      "targetMetricKey": null,
      "targetComparator": null,
      "targetValueNumeric": null,
      "targetUnit": null,
      "confidence": "high",
      "sourceSnippet": "...we expect to ship the next major release within two quarters...",
      "sourceEvidenceType": "transcript_segment",
      "sourceEvidenceId": "seg_42",
      "status": "pending",
      "claimId": null
    }
  ]
}
```

Allowed `sourceType`: `report_document`, `transcript`. Allowed proposal `status`: `pending`, `confirmed`, `rejected`.

Local commands:

- `start_claim_extraction(input)`: creates a job over a report document or transcript and spawns the async runner. Idempotent per `(sourceType, sourceId, promptVersion)`.
- `list_claim_extraction(input)`: returns jobs + proposals for a source (`sourceType` + `sourceId`) or company.
- `confirm_claim_proposal(input)`: materializes a `management_claims` row from a proposal, applying optional user overrides; marks the proposal `confirmed` with the new `claimId`.
- `reject_claim_proposal(proposalId)`: marks a proposal `rejected` (retained; suppresses re-proposal).
- `retry_claim_extraction(jobId)`: re-runs a failed job.

Rules:

- Only a confirmed proposal creates a claim; no claim is created automatically.
- The runner uses the provider-neutral AI settings and credential boundary; provider/model/prompt provenance is recorded.
- Extraction never blocks ingestion and is always user-initiated.

Error codes: `extraction_job_not_found`, `proposal_not_found`, `source_not_found`, `proposal_already_resolved`, `provider_unavailable`, `extraction_failed`.

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
- `rename_cockpit_layout(input)`: `{ id, name }` → renames; `name` must be unique and non-empty.
- `delete_cockpit_layout(layoutId)` → removes the layout by id (idempotent — deleting an absent id is a no-op).

Restore/fallback behavior, source-of-truth split between `panelsJson`/`layoutJson`, and import/export durability are canonical in [Data Model § Research Cockpit Layouts](data-model.md#research-cockpit-layouts).

Error codes: `cockpit_layout_not_found`, `invalid_cockpit_layout_name`.

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

`get_report_diff(input)` returns the on-demand section diff read model for a chosen pair. `input` is `{ olderReportDocumentId: string, newerReportDocumentId: string }`. Both documents must be the same company and statement type. Sections are aligned by heading + ordinal (positional consumption — duplicate headings never cross-match) with the optional `content_embeddings` similarity enhancer when the embedding strategy is active; each section is classified `unchanged` | `changed` | `only_older` | `only_newer`, and `changed` sections carry a line-level diff with citations (ordinal + offset) into both documents:

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

The autonomous report pipeline (North Star, `v0.49.0`, [ADR 0055](adr/0055-autonomous-report-pipeline-trust-ladder.md)) closes the loop: a tracked, opted-in company's new periodic report is detected, fetched, extracted, diffed, cross-referenced, and surfaced as a single notification — no manual steps. Orchestration is **chained durable-queue jobs** (`fetch → extract → diff → cross_reference → notify`) stamped with one `autopilot_run` id; each stage reuses the existing service (`fetch_report_document`, AI KPI extraction, `get_report_diff`, claims/research cross-reference). Detection is **event-driven off source-refresh completion** and runs **only while the app is open**. The global confirm-before-commit default never changes; automation is a per-company opt-in. Decision-support only — the result reports *what changed / to verify*, never buy/sell/hold ([ADR 0042](adr/0042-advisory-verdict-port-and-open-core-boundary.md)).

**Trust ladder (per-company mode).**

`get_company_autopilot(input)` returns a company's mode. `input` is `{ companyId: string }`; returns `{ companyId, mode }` where `mode` is `off` | `assist` | `autopilot` (a company with no setting reads `off`).

`set_company_autopilot(input)` sets the mode. `input` is `{ companyId, mode }`. `off`: nothing automatic. `assist`: auto-fetch + auto-extract on detection, but facts land `pending` for user confirmation. `autopilot`: full loop; facts auto-committed as `auto_unreviewed` (cited, flagged, reversible). Changing the mode never alters already-produced facts or runs. Error codes: `company_not_found`, `invalid_autopilot_mode`.

**Structured-first extract stage ([ADR 0061](adr/0061-deterministic-fundamentals-data-gathering.md) dec. 3/8/9).** The extract stage runs the deterministic structured pipeline (ESEF/iXBRL or a PDF whose reporting period is derivable from its title/URL) **before AI, in both `assist` and `autopilot` modes** — not autopilot-only. Its outcome sets the per-fact `confirmationState` directly, superseding the flat mode-based default above for structured facts specifically: a validation-clean set (`accepted` / `accepted_via_witness`) auto-confirms outright (`confirmed`) in **both** modes; an uncontradicted-but-unproven set (`accepted_unreviewed`) still follows the mode ladder (`auto_unreviewed` in autopilot, `pending` in assist). A `flagged` outcome (a layout drift or contradiction) emits no structured facts and falls through to the AI proposal path — the AI facts keep the pre-existing mode-based confirmation rule — but the run's `kpiDeltaJson` still carries `structureChanged: true` and `driftJson` (the serialized layout diff) whichever branch's delta ends up composed, so a drifted profile is never silently dropped just because AI ultimately produced the facts.

`list_company_autopilot_modes()` returns every company with an explicit (non-`off`) autopilot mode set — `CompanyAutopilot[]`, each `{ companyId, mode }`. Companies with no row default to `off` and are omitted.

`set_companies_autopilot(input)` ([ADR 0056](adr/0056-per-company-settings-surface.md)) sets the same mode on many companies at once from the master-detail per-company settings surface. `input` is `{ companyIds: string[], mode }`; returns the number of companies updated.

**Runs and review.**

`list_autopilot_runs(input)` returns recent runs for the attention home / review queue. `input` is `{ companyId?: string, notificationState?: "unread" | "read" | "dismissed", limit?: number }`. Each run carries `{ id, companyId, reportDocumentId, trigger, mode, status, stage, summaryText, kpiDeltaJson, reportDiffRef, crossRefsJson, producedFactIds, notificationState, lastError, createdAt }`. `status` is `pending` | `running` | `succeeded` | `failed` | `partial`; `stage` is the current/last stage reached. A `failed`/`partial` run still appears with a summary of how far it got.

`kpiDeltaJson` (extract stage) and `crossRefsJson` (cross-reference stage) are opaque JSON envelopes, not typed fields — each stage owns its own shape (e.g. `{ extractionAvailable, structured?, tier?, factsProposed, factsAutoConfirmed, structureChanged?, driftJson? }` and `{ claimsOverdue, claimsDue, openQuestions }` respectively). `factsProposed`/`factsAutoConfirmed` are honest counts of facts the run actually produced (both tiers write them, bug e77a1a2): `factsProposed` is every fact the run emitted this run (structured or AI), `factsAutoConfirmed` is the subset already `confirmed`/`auto_unreviewed` (no review needed) — never inferred from a raw `produced`/`proposed` count that only one tier happened to populate.

`summaryText` is a Rust-composed, **English-only** notification line — **legacy/fallback only**, kept for backward compatibility and diagnostics. The Today/Pulse run card never renders it directly: it composes its own localized sentence from `kpiDeltaJson`/`reportDiffRef`/`crossRefsJson` via `text()`/`pluralNoun` (`src/screens/Today/autopilotRunSummary.ts`), per the i18n rule (every user-visible string routes through `text()`, [ui-authoring.md](ui-authoring.md)).

`get_autopilot_run(input)` returns one run's full composed result. `input` is `{ runId: string }`.

`set_autopilot_run_notification_state(input)` marks a run's notification `read` or `dismissed` (drives the Today/Pulse "what changed" surface, [ADR 0054](adr/0054-mode-based-thesis-centric-shell.md)). `input` is `{ runId, notificationState }`.

`undo_autopilot_run(input)` reverts exactly the facts a run produced (recorded in `producedFactIds`), reusing the existing fact supersede/reject mechanics. `input` is `{ runId: string }`; returns `{ runId, revertedFactIds }`. Idempotent — undoing an already-undone run is a no-op. Reachable from the Today/Pulse Autopilot run card's **Undo** action (two-step confirm), shown when `mode === "autopilot"` and `producedFactIds` is non-empty (an `assist`-mode run's `pending` facts go through the existing confirm/reject review instead).

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

Status: planned (v0.59.0, ADR 0058)

The investor week calendar ([ADR 0058](adr/0058-investor-week-calendar.md), `v0.59.0`) extends the Events view with composable, opt-in **layers** over a backend-owned read model — no stored weekly projection (the `list_report_season` pattern).

`list_investor_week(input)` returns the week read model. `input` is `{ weekAnchor: "YYYY-MM-DD", scope: "watchlist" | "market", watchlistId?: string, layers: { macro: boolean, holidays: boolean } }`. It returns working-day columns (Mon–Fri; a weekend column only when populated); each column groups items by layer (`company`, `macro`, `holiday`) with per-layer freshness so a stale layer is visible rather than silently empty. The `company` layer unions tracked `company_events` with, when `scope = "market"`, untracked `market_calendar_events`, deduped by ticker.

Macro (`macro_events`) read/write — manual entry ships in `v0.59.0`; a live macro source is deferred to a follow-up ADR:

- `list_macro_events(input)`: `{ from: "YYYY-MM-DD", to: "YYYY-MM-DD" }` → macro releases in range.
- `create_macro_event` / `update_macro_event` / `delete_macro_event`: user-entered releases (`manual = 1`), with `indicatorKey`, `title`, `country`, `eventDate`, optional `eventTime`/`importance`/`actual`/`forecast`/`previous`.

Holidays (`market_holidays`) read — a curated static dataset, no write contract beyond seed/refresh:

- `list_market_holidays(input)`: `{ from, to, markets?: string[] }` → holidays in range, tolerant of an un-seeded year (empty result, never an error).

The active scope and enabled layers persist via `update_settings` (the pinned-companies pattern).
- Manual events use `sourceType: "manual"` and `manual: true`.
- Manual events are for missing or user-known dates, not corrections to normal source updates.

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
- `run_ai_signal_classification()`: runs the opt-in AI classification fallback over official filings the rule classifier left unknown, creating `proposed` signals. A no-op returning `{ enabled: false, examined: 0, proposed: 0, skipped: 0 }` unless the user enables the `espiAiFallbackEnabled` setting; uses the configured general analysis provider/model (ADR 0028). Never auto-commits — results require confirmation.
- `run_ai_event_derivation()`: runs the opt-in AI date-extraction fallback over confirmed dividend/general-meeting signals the deterministic parser could not date, deriving `proposed` calendar events (`v0.41.0`, [ADR 0036](adr/0036-report-document-storage-and-backfill.md)). A no-op returning `{ enabled: false, examined: 0, derived: 0, skipped: 0 }` unless `espiAiFallbackEnabled` is set; uses the same provider/model and never auto-commits — derived events require `confirm_derived_event`.

The opt-in toggle is the `espiAiFallbackEnabled` boolean in user settings (default `false`); no provider call happens until it is enabled. It gates both the classification fallback and the event-date fallback.

Confirm/reject input:

```json
{
  "id": "signal_01"
}
```

Rules:

- Confirming or rejecting applies only to `proposed` signals; `confirmed` signals are terminal except for future reversal flows.
- Confirmation must persist provider provenance and create at most one derived event, idempotently.
- Derived-event date extraction (`v0.41.0`, dividend/general-meeting only) is **deterministic-first** over the fetched filing body, with the **opt-in async AI fallback** when the deterministic parse is not confident. The derived event is created `proposed` and requires `confirm_derived_event` before it appears on the calendar — a guessed-date event is never created. See [ADR 0036](adr/0036-report-document-storage-and-backfill.md).

## AI Analysis Result

```json
{
  "id": "analysis_01",
  "aiAnalysisJobId": "analysis_job_01",
  "feedItemId": "feed_01",
  "providerId": "provider_openai_compatible",
  "model": "configured-model-name",
  "promptVersion": "m13.source_grounded.v1",
  "summary": "Short neutral summary.",
  "significance": "medium",
  "tags": ["earnings", "guidance"],
  "reasoning": "Why this may matter, based on cited source content.",
  "language": "en",
  "sourceReferences": [
    {
      "sourceUrl": "https://www.gpw.pl/komunikaty",
      "label": "GPW ESPI/EBI report"
    }
  ],
  "createdAt": "2026-05-28T13:00:00Z"
}
```

Rules:

- Allowed significance values: `low`, `medium`, `high`, `unknown`.
- AI output must not include buy/sell/hold recommendations.
- AI output must reference source material used for analysis.

## AI Analysis Job

```json
{
  "id": "analysis_job_01",
  "feedItemId": "feed_01",
  "promptPresetId": "default_summary",
  "customQuestion": null,
  "providerId": "provider_gemini",
  "model": "gemini-2.5-flash",
  "promptVersion": "m13.source_grounded.v1",
  "status": "queued",
  "errorCode": null,
  "error": null,
  "createdAt": "2026-06-03T10:00:00Z",
  "startedAt": null,
  "finishedAt": null,
  "result": null
}
```

Rules:

- General AI analysis jobs are local async state for user-triggered feed-item analysis.
- `promptPresetId` is stable and may represent a built-in prompt preset.
- `customQuestion` is optional and stores the user's local question text for the selected feed item.
- Allowed statuses begin with `queued`, `running`, `succeeded`, `failed`, and `cancelled`.
- `errorCode` and `error` are recoverable local diagnostics and must not contain provider secrets or full source text.
- Successful jobs may include the current `AI Analysis Result` read model.
- UI panels showing a selected feed item's latest `queued` or `running` job should poll `list_ai_analysis` until the job reaches a terminal state or the panel is no longer relevant.

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
  "contentTypes": ["company", "watchlist", "feed_item", "notebook_entry", "transcript_segment", "event", "research_brief", "digest"],
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
- Coverage is companies, watchlists, feed items, notebook entries, transcript segments, company events, research briefs, and digests.

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
  "historySweepAiCallLimit": 30,
  "settingsSource": "sqlite",
  "settingsImportExportFormat": "yaml",
  "aiProviders": {
    "youtubeTranscriptionProvider": "provider_gemini",
    "youtubeTranscriptionModel": "gemini-2.5-flash",
    "youtubeTranscriptionTimeoutSeconds": 300,
    "generalAnalysisProvider": null,
    "generalAnalysisModel": "gemini-2.5-flash",
    "generalAnalysisTimeoutSeconds": 90,
    "openaiCompatibleBaseUrl": ""
  },
  "aiAnalysisMode": "source_grounded",
  "shortcutBindings": {},
  "capabilityProviders": {},
  "database": {
    "maxConnections": 4,
    "busyTimeoutMs": 5000,
    "acquireTimeoutMs": 10000
  },
  "queue": {
    "sourcesWorkers": 2,
    "autopilotWorkers": 3,
    "aiWorkers": 2,
    "aiProviderConcurrency": 2
  },
  "pinnedCompanyIds": []
}
```

`update_settings` is atomic: a validation failure on any field rolls back the whole request, leaving every setting (including fields earlier in the same request) untouched.

`backfillYears` (ADR 0077 §3) is the years of company history the on-track backfill fetches. `update_settings` accepts an optional `backfillYears`, clamped to `[1, 10]` on write (never rejected); omitting it leaves the current depth unchanged. No seed row: reads default to `3` and clamp an out-of-range stored value. The Sources settings section exposes it as clickable presets (1/3/5/10) bound to a slider + numeric input.

`historySweepAiCallLimit` (ADR 0077 §6) is the per-history-sweep tier-4 AI call budget (one unit = one tier-4 invocation for one document; `0` = unlimited). `update_settings` accepts an optional `historySweepAiCallLimit`, clamped to `[0, 500]` on write (never rejected); omitting it leaves the current budget unchanged. No seed row: reads default to `30` and clamp an out-of-range stored value. The value is **snapshotted onto each sweep at creation**, so a change only governs future sweeps. Settings → AI exposes it as clickable presets (0/10/30/100) bound to a slider + numeric input; the Coverage panel footer echoes the latest sweep's spend ("AI: {used}/{limit}", or "AI: {used} (no limit)" when the limit is `0`).

`pinnedCompanyIds` (ADR 0054) is the ordered list of company IDs pinned to the
sidebar IA spine. `update_settings` accepts an optional `pinnedCompanyIds`
(full-replacement array, de-duplicated, blanks dropped); omitting it leaves the
current pins unchanged. Defaults to `[]`.

`capabilityProviders` (ADR 0060 as amended, ADR 0061 decision 5) is a map from
capability key to an **ordered** list of `{ provider, model }` entries — the
capability's failover pool, tried in list order. `update_settings` accepts an
optional `capabilityProviders` (full-replacement map, same overwrite contract
as `shortcutBindings`); omitting it leaves the current map unchanged. Defaults
to `{}`. An absent key or an empty list for a key means "use
`generalAnalysisProvider` / `generalAnalysisModel`" — every capability is
backward-compatible with a single global provider.

Capability keys (`AiCapability::key`, fixed set of 9):

| Key | Kind | Provider call |
|---|---|---|
| `kpi_extraction` | document | `complete_document` |
| `claim_extraction` | document | `complete_document` |
| `feed_analysis` | text | `analyze` |
| `research_brief` | text | `generate_research_brief` |
| `research_digest` | text | `generate_research_digest` |
| `event_date` | text | `complete_document` (extracted text) |
| `signal_classification` | text | text |
| `qualitative_assessment` | text | `complete_document` (self-contained prompt) |
| `vision_extraction` | document | OCR (tier-4 last-resort, [ADR 0077](adr/0077-trusted-extraction-foundations.md) §4) |

Validation rules for `capabilityProviders`:

- Every map key must be one of the 8 capability keys above; an unknown key is rejected.
- Every entry's `provider` must be a currently selectable analysis provider id (the AI Provider Catalog below); an entry's `model` must be non-empty.
- `model` must belong to the provider's curated model list, except for `provider_openai_compatible`, which has no curated list and accepts any non-empty freeform model id (same exemption as `generalAnalysisModel`).
- An empty entry list for a key is valid — it is the explicit "use the global fallback" state.
- A `document`-kind capability (`kpi_extraction`, `claim_extraction`, `vision_extraction`) rejects any entry whose provider is not **document-native** (`provider_gemini`, `provider_anthropic`, `provider_mistral`): `provider_openai` and `provider_openai_compatible` are text-only and would hard-fail every document call and poison pool failover, so `update_settings` rejects the combination outright with `"<capability>: provider <id> cannot accept document input; route a document-capable provider (Gemini or Claude)"` rather than letting it surface only at job-run time.

Pool (failover) semantics at run time (ADR 0061 decision 5):

- The list order is the failover order: the first entry is tried first.
- A member is skipped over to the next only on an **availability error** (429 / 5xx / timeout / connection error); a valid 200 response with unparsable/bad content is never a failover trigger — it surfaces immediately.
- A member that just failed an availability error enters a 60-second cooldown: it is tried last (deprioritized), never excluded — the pool always tries every member, so it never dead-ends even when all members are cooling.
- Cooldown state is runtime-only (in-memory), never persisted to settings or the database.
- A pool member with no configured credential (or the compatible provider with no configured base URL) is skipped when the pool is built, with a warning logged.

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
| `aiAnalysisMode` | `source_grounded` | `opinionated` mode requires explicit opt-in and still excludes buy/sell/hold output |
| `aiProviders.generalAnalysisTimeoutSeconds` | `90` | options: 45\|90\|180\|300\|600 |
| `aiProviders.openaiCompatibleBaseUrl` | `""` (unconfigured) | ADR 0060; must start with `http://`/`https://` when set; consulted only when the resolved provider is `provider_openai_compatible` |
| `aiProviders.youtubeTranscriptionModel` | `gemini-2.5-flash` | cheapest M10-validated model; options: gemini-2.5-flash-lite\|gemini-2.5-flash\|gemini-3.1-flash-lite\|gemini-3.5-flash |
| `aiProviders.youtubeTranscriptionTimeoutSeconds` | `300` | options: 45\|90\|180\|300\|600 |
| `database.maxConnections` | `4` | clamped 1–16 |
| `database.busyTimeoutMs` | `5000` | clamped 0–60000 |
| `database.acquireTimeoutMs` | `10000` | clamped 1000–60000; database pool ADR 0032; applied at pool build (next launch) |
| `queue.sourcesWorkers` | `2` | clamped 1–16 |
| `queue.autopilotWorkers` | `3` | clamped 1–16 |
| `queue.aiWorkers` | `2` | clamped 1–16 |
| `queue.aiProviderConcurrency` | `2` | clamped 1–10; queue tuning ADR 0059; indexing stays a constant 1 worker, not user-tunable; applied at next launch |

A missing or invalid value for any clamped field falls back to its default so the app always opens. Settings must offer a reset-to-defaults action for the database pool block, and disclose that pool/queue changes take effect on next launch.

Other rules:

- `theme` controls brightness mode only; `accentPalette` controls the semantic color palette. Accent palettes must be added through the settings validation and theme-token registry, not as component-local color overrides.
- Developer mode may be enabled only through intentional local developer mechanisms, not a normal always-visible Settings toggle. Startup activation uses `BRAWLER_DEVELOPER_MODE=1`, `true`, `yes`, or `on`. Runtime author unlock (`unlock_developer_mode({ passphrase })`) may enable Developer mode after the app is already running only when `BRAWLER_DEVELOPER_UNLOCK_CODE` is present in the app process environment and the submitted passphrase matches it; the entry point is hidden from normal UI and must not be registered as a configurable shortcut. Once active, Diagnostics may show status and a disable action (`disable_developer_mode()`).
- Settings must let the user switch the app locale between English and Polish; locale handling is an extensible app-locale boundary so future locales are added through resources/configuration, not per-screen rewrites. Source-provided text, company names, ticker symbols, URLs, source attribution, transcript text, and notebook bodies retain their original or user-entered language.
- General AI analysis runs through asynchronous local job state so provider calls do not block the UI; the provider contract stays provider-neutral so OpenAI/Anthropic/others can be added without rewiring the UI. Settings must let the user choose the general analysis provider/model/timeout from supported configured options.
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
- Credential commands are generic and keyed by `providerId` (`provider_gemini`, `provider_anthropic`, `provider_openai`, `provider_openai_compatible`, `provider_mistral`):
  - `get_provider_credential_status({ providerId })` returns the non-secret status for one provider.
  - `set_provider_api_key({ providerId, apiKey })` stores or replaces only that provider's API key.
  - `clear_provider_api_key({ providerId })` removes only that provider's OS-keychain key and must not mutate `.env` or process environment values.
- Legacy purpose-scoped Gemini credential commands were removed with no backward compatibility; legacy keychain entries are best-effort cleared on startup.

## AI Provider Catalog

`list_ai_provider_catalog()` returns the selectable analysis providers and their curated models (ADR 0028), the single source of truth for the settings provider/model selection UI:

```json
[
  {
    "providerId": "provider_anthropic",
    "label": "Claude (Anthropic)",
    "models": ["claude-sonnet-4-6", "claude-opus-4-8", "claude-haiku-4-5-20251001"],
    "defaultModel": "claude-sonnet-4-6",
    "requiresCredential": true
  },
  {
    "providerId": "provider_openai_compatible",
    "label": "OpenAI-compatible (custom)",
    "models": [],
    "defaultModel": "",
    "requiresCredential": true
  },
  {
    "providerId": "provider_mistral",
    "label": "Mistral",
    "models": ["mistral-small-latest"],
    "defaultModel": "mistral-small-latest",
    "requiresCredential": true
  }
]
```

Rules:

- The active analysis provider and model are the `generalAnalysisProvider` / `generalAnalysisModel` settings (or, per capability, a `capabilityProviders` entry); the selected model must belong to the selected provider's catalog entry.
- Exact model ids are curated server-side; the UI must not hardcode model lists.
- `provider_openai_compatible` (ADR 0060) has an empty curated `models` list and `defaultModel`: its model is a freeform, user-supplied id (any non-empty string), since concrete hosts (Groq, OpenRouter, Ollama, Together, Cerebras, …) each publish their own model names. It speaks the OpenAI chat-completions wire format against a user-configured `openaiCompatibleBaseUrl` and requires a credential like every other selectable provider.
- `provider_mistral` ([ADR 0077](adr/0077-trusted-extraction-foundations.md) §4) is the tier-4 OCR substrate: only its chat model (`mistral-small-latest`) is user-selectable; the OCR model (`mistral-ocr-latest`) is provider-internal (used by `ocr_document`, not exposed as an analysis model) and so is absent from `models`.

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

Initial AI research brief job shape:

```json
{
  "id": "research_brief_job_01",
  "scopeType": "company",
  "scopeId": "company_gpw_cdr",
  "providerId": "provider_gemini",
  "model": "gemini-2.5-flash",
  "promptVersion": "m30.research_brief.v1",
  "evidenceCollectorVersion": "m30.collector.v1",
  "rendererVersion": "m30.renderer.v1",
  "status": "queued",
  "errorCode": null,
  "error": null,
  "createdAt": "2026-06-11T10:00:00Z",
  "startedAt": null,
  "finishedAt": null,
  "brief": null
}
```

Initial AI research brief shape:

```json
{
  "id": "research_brief_company_gpw_cdr_01",
  "jobId": "research_brief_job_01",
  "scopeType": "company",
  "scopeId": "company_gpw_cdr",
  "providerId": "provider_gemini",
  "model": "gemini-2.5-flash",
  "promptVersion": "m30.research_brief.v1",
  "evidenceCollectorVersion": "m30.collector.v1",
  "rendererVersion": "m30.renderer.v1",
  "title": "CD Projekt research brief",
  "summary": "Neutral source-grounded summary.",
  "contentMarkdown": "Rendered brief content with citation markers.",
  "language": "en",
  "generatedAt": "2026-06-11T10:03:00Z",
  "createdAt": "2026-06-11T10:03:00Z",
  "citations": [
    {
      "id": "research_brief_citation_01",
      "briefId": "research_brief_company_gpw_cdr_01",
      "citationKey": "E1",
      "evidenceType": "feed_item",
      "evidenceId": "feed_01",
      "label": "Current report title",
      "snippet": "Short cited snippet or evidence label"
    }
  ]
}
```

Initial AI research brief input shape:

```json
{
  "scopeType": "company",
  "scopeId": "company_gpw_cdr"
}
```

Rules:

- Initial `scopeType` values are `company` and `watchlist`.
- AI research brief generation is explicit and on-demand only.
- Brief jobs use the existing provider-neutral AI configuration and credential boundary.
- Brief generation is asynchronous and must not block the UI.
- Allowed job statuses begin with `queued`, `running`, `succeeded`, `failed`, and `cancelled`.
- Briefs are immutable snapshots. Regeneration creates a new job and, on success, a new brief.
- Briefs are dedicated research-owned entities, not notebook entries.
- Briefs must not include buy/sell/hold recommendations or personalized portfolio advice.
- Brief content must cite research evidence through citation keys mapped back to typed evidence items.
- Citations store evidence references and short labels/snippets only; full copied source bodies should not be duplicated into citation rows.
- Normal UI must show citations and enough provider/model/prompt provenance for source-grounded review without exposing implementation identifiers.
- Creating a notebook note from a brief remains a separate explicit workflow and is not automatic.

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

Initial research digest job shape mirrors AI brief jobs but uses digest-specific versions:

```json
{
  "id": "research_digest_job_01",
  "scopeType": "watchlist",
  "scopeId": "watchlist_main_gpw",
  "providerId": "provider_gemini",
  "model": "gemini-2.5-flash",
  "promptVersion": "m31.research_digest.v1",
  "evidenceCollectorVersion": "m31.digest_collector.v1",
  "rendererVersion": "m31.digest_renderer.v1",
  "status": "queued",
  "errorCode": null,
  "error": null,
  "createdAt": "2026-06-12T08:00:00Z",
  "startedAt": null,
  "finishedAt": null,
  "digest": null
}
```

Research digest rules:

- Initial digest scopes are `company` and `watchlist`.
- Digest generation is explicit and on-demand.
- Digest input collection is backend-owned and combines open reminders with changed research evidence.
- Digest output is an immutable research-owned snapshot with citations.
- Digest output must cite typed evidence and must not include buy/sell/hold recommendations, price targets, portfolio allocation advice, or personalized investment advice.

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
- `start_research_brief(input)`: `{ scopeType, scopeId }` → enqueues an AI research brief job (see the brief job shape above).
- `list_research_briefs(input)`: `{ scopeType, scopeId }` → returns brief jobs for that scope, newest first.
- `start_research_digest(input)`
- `list_research_digests(input)`

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

Allowed `confirmationState` values:

- `confirmed`
- `pending`
- `auto_unreviewed`

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
- Backfill is **idempotent** — it reuses feed-item `(sourceAdapterId, sourceEventKey)`, report-document `(companyId, url)`, and signal `(feedItemId, category)` dedup, so re-runs and resumed partial runs never duplicate. Throttling obeys the existing Bankier rate policy (serialized, waits between pages/companies). Backfill is never automatic.

### Fundamentals Commands

Initial local commands:

- `list_kpi_definitions(input)`: returns KPI definitions, optionally filtered by scope, sector, or company.
- `create_kpi_definition(input)`: creates one KPI definition at any scope level.
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
- The report-documents **view** read model — `get_report_documents_view(companyId)` — returns `ReportDocumentsView { companyId, rows: ReportDocumentViewRow[] }`, one row per stored document tagged with the period it belongs to and whether it is that period's canonical report ([ADR 0077](adr/0077-trusted-extraction-foundations.md) §1/§2). An assembled read model (ADR 0044; no stored projection), offloaded (`spawn_blocking`, since ESEF period derivation reads the stored file). Each `ReportDocumentViewRow` is `{ document: ReportDocument, fiscalYear: number | null, periodType: string | null, canonical: boolean }`:
  - Period fields come from the **same** `document_period` helper the coverage map uses (`derive_report_period` first, then the title/URL fallback), so the two panels can never disagree about a document's period. `fiscalYear`/`periodType` are `null` together when no period can be derived (the common case for non-periodic filings).
  - `canonical` is `true` only for a periodic document selected by `canonical_reports_per_period` over the same inputs the coverage map feeds it — so the panel's ★ marks the very document the coverage map names as the period's report.
  - **UI entry point**: `CompanyReportDocumentsPanel` (the redesigned Report documents panel, [ADR 0077](adr/0077-trusted-extraction-foundations.md) §2 / mockup Panel B). By default it **groups documents by fiscal period** (newest first) with a "Group by period" toggle back to a flat list; within a group the periodic statements come first (a ★ on the canonical one), then audit reports, then a fold hiding the signature/data companions (a companion whose "Extract data" action is available is never folded); non-periodic filings collect in a collapsed "No period" group. A search field filters across title/filename, and the kind filter + "Refresh classification" action stay. Fetched via the `getReportDocumentsView(companyId)` wrapper (`src/api/reportDocuments.ts`).

Input shapes follow the corresponding domain types above. Return shapes include all domain fields plus timestamps. Company fundamentals data must be treated as owner-durable state in import/export and backup workflows.

Structured-first extraction commands ([ADR 0061](adr/0061-deterministic-fundamentals-data-gathering.md); generated DTOs `RunStructuredExtractionInput` / `StructuredExtractionSummary` / `FactProvenance`):

- `run_structured_extraction(input)`: `{ companyId, reportDocumentId, fiscalYear, periodType, periodEnd, mode? }` → runs the deterministic tiered pipeline (ESEF → PDF+profile → HTML witness) over one stored report document and persists accepted facts with provenance; returns `{ acceptance, tier, emitted, producedFactIds, skippedFactIds, divergentCount, driftJson, tier4, tier4Proposals }`. **Tier-4 OCR fallback** ([ADR 0077](adr/0077-trusted-extraction-foundations.md) §4): when determinism ends `flagged`/`empty` and the caller gate allows it (manual/detection triggers always; a history-sweep run when a unit of its sweep's AI budget is granted — F3b, ADR 0077 §6), the Mistral-OCR `vision_extraction` path runs — a confirmed per-company OCR profile parses the document to VALIDATED facts (`source_tier='ai'`, folded into `producedFactIds`, `tier` reported as `ai_text`), or (never-bootstrapped company / a validation-failing parse) lands proposals for the confirm flow (`tier4Proposals`). `tier4` is the honest outcome string (`facts_emitted` · `bootstrap_proposals` · `proposals_flagged` · `no_vision_provider` · `not_pdf` · `provider_error:<code>` · `empty`), `null` when tier-4 did not run. A transient OCR provider failure propagates as an error (queue backoff retry); a terminal one degrades with its reason. Tier-4 is PDF-only (an ESEF/structured document degrades `not_pdf`). Re-extraction is **idempotent** (T7-F): a fact whose uniqueness slot already holds the same value is a re-observation — counted in `skippedFactIds`, never re-inserted; a slot holding a **different** value is never silently overwritten — the stored fact wins, the divergence is counted in `divergentCount` and recorded as a `diagnostic_events` entry for review. Offloaded (`spawn_blocking`). `mode` is the trust-ladder mode (`autopilot` | `assist`, default `autopilot`; any other value is rejected); the per-fact `confirmationState` is derived from the validation outcome (`accepted`/`accepted_via_witness` → `confirmed` in both modes; `accepted_unreviewed` → `auto_unreviewed`/`pending` by mode) — never from a caller-chosen literal (ADR 0061 dec. 3/8/9). Period supplied by the caller (the KPI-extraction flow, which knows the detected period).
- `extract_report_document_data(input)`: `{ companyId, reportDocumentId, mode? }` → **the reachable one-click "Extract data" action** on a report-document row (the company workspace's Report documents panel) — closes the ADR 0061 S5 live-path gap where the deterministic pipeline had no UI caller outside autopilot. Derives the reporting period **server-side** (the same `derive_report_period` the autopilot stage uses: ESEF self-derived `FY`; PDF from title/URL period classification), so the UI never invents `fiscalYear`/`periodType`/`periodEnd`, then runs `run_structured_extraction` and returns the same `StructuredExtractionSummary`. Offloaded (`spawn_blocking`). Same `mode`/confirmation semantics as `run_structured_extraction` (facts land unchanged — pending vs auto-committed by `mode` + validation outcome). Errors when the period can't be derived (no stored file, unparsable ESEF, or a PDF with no classifiable period). **UI entry point**: `CompanyReportDocumentsPanel` extract action.
- `list_fact_provenance(factIds)`: returns the `FactProvenance` rows (`factId`, `sourceTier`, `validationStatus`, `driftJson`, `citation`) for the requested facts; facts predating the pipeline have no row (render as unvalidated).
- `list_flagged_fact_provenance()`: returns every provenance row with `validationStatus = "flagged"` (the drift/notification read model).
- `get_fundamentals_coverage(companyId)`: returns `FundamentalsCoverage { companyId, periods: CoveragePeriodRow[] }`, the per-company **coverage map** ([ADR 0077](adr/0077-trusted-extraction-foundations.md) §2). An **assembled read model** (the ADR 0044 pattern — computed on demand from the live tables, **no stored projection and no table**; cardinality is a handful of periods per company); offloaded (`spawn_blocking`, since ESEF period derivation reads the stored file). Each `CoveragePeriodRow` is one `{ fiscalYear, periodType }` period:
  - Cell `report` (`CoverageReportCell | null`) — the single **canonical periodic report** for the period (ssf-over-jsf, newest revision; ADR 0077 §1 selection over the **stored `doc_kind` column** + `derive_report_period` — the kind is never re-derived on the fly, so coverage cannot disagree with the documents panel; a `NULL` (unclassified) document is excluded until set-on-write / "Refresh classification" converges it). `{ documentId, docKind (periodic_ssf | periodic_jsf), title, structured, fetched }`; `fetched = fetch_status == "fetched"`, so a link-only (metadata-only) periodic report still yields a cell with `fetched: false` via the title/URL period fallback. `null` when no periodic report names the period.
  - Cell `facts` (`CoverageFactsCell`) — `{ total, validated, unvalidated, flagged }` over the period's `financial_facts` joined to `financial_fact_provenance`: `validated` = provenance `passed`/`witness_confirmed`, `flagged` = provenance `flagged`, `unvalidated` = everything else (no provenance row, `unreviewed`, `none`). `total = validated + unvalidated + flagged`.
  - Cell `review` (`CoverageReviewCell`) — `{ pendingProposals, flaggedFacts }`: `pendingProposals` = `pending` `kpi_extraction_proposals` whose job detected this `(fiscalYear, periodType)`; `flaggedFacts` mirrors `facts.flagged`.
  - Field `skippedBudget` (`boolean`) — `true` when the period's canonical report's `trigger='history_sweep'` autopilot run recorded `reason: "skipped_budget"` on its `kpiDeltaJson` (a budget-denied tier-4, [ADR 0077](adr/0077-trusted-extraction-foundations.md) §6). Run ids are per-`(company, document)` deterministic, so there is at most one run per document; a later successful extraction clears the flag two ways (facts appear → non-gap; the run's delta is overwritten). Tolerant: an absent/garbled delta, or any other `reason`, reads `false`.
  - **Period-union rule**: a period appears iff at least one of {a canonical report, ≥1 fact, ≥1 pending proposal} names it; rows are sorted newest-first (DESC by `fiscalYear`, then period index `Q1<H1<Q3<FY`).
  - **UI entry point**: the **Coverage panel** (`src/shared/components/CompanyCoveragePanel.tsx`, T2.2) — a company-scoped cockpit pane (kind `coverage`, label "Coverage") seeded into the curated company dashboard. It renders one table row per period (Period / Report / Data / To review); clicking a row opens the company's Report documents pane. Fetched via the `getFundamentalsCoverage(companyId)` wrapper (`src/api/fundamentalsCoverage.ts`), reloading on `companyId` change. Its **history-actions footer** (T3.2) drives "Backfill history" and "Extract missing periods" (below); the footer's status line also echoes the latest sweep's **AI-call spend** (T5.3 — "AI: {used}/{limit}", or "AI: {used} (no limit)" when the budget is `0`).

#### History Sweep (`v0.51.0`, [ADR 0077](adr/0077-trusted-extraction-foundations.md) §3)

The history sweep is the backfill/manual counterpart to the refresh-time detection sweep: it enqueues a full autopilot run (`trigger='history_sweep'`, with the sweep's row id stamped as the run's `sweep_id`) for every canonical periodic report whose period still lacks accepted facts. It runs only for a company opted into automation (mode ≠ `off`); a company in mode `off` ends the sweep with `skippedReason='automation_off'` — never a silent skip. Sweep runs are **AI-budget-gated** (F3b, [ADR 0077](adr/0077-trusted-extraction-foundations.md) §6): the `HistorySweep` DTO carries `aiCallsUsed` / `aiCallLimit` (the budget snapshotted from `historySweepAiCallLimit` at creation; `0` = unlimited) — a run whose deterministic pipeline emits nothing enters tier-4 only while a unit remains, and otherwise records `skipped_budget` on its `kpiDeltaJson`. Durable state lives in the `history_sweeps` table ([data-model.md](data-model.md)). DTOs `HistorySweep` / `HistorySweepProgress`.

- `run_history_sweep(companyId)`: starts a **manual** history sweep ("Extract missing periods" — the case where documents are already fetched and only extraction is missing, no re-download). Gates on the company existing and mode ≠ `off` (`company_not_found` / `automation_off` errors; the UI disables the button in mode `off`, the command stays honest for a direct call), creates a `manual` sweep row, enqueues the durable `history_sweep` job, and returns the `HistorySweep`. Offloaded (`spawn_blocking`). **UI entry point**: the Coverage panel footer's "Extract missing periods" action.
- `get_history_sweep_progress(companyId)`: returns `HistorySweepProgress { sweep: HistorySweep | null, runsTotal, runsDone, runsFailed }` — the company's latest sweep plus per-run progress derived from its enqueued run ids (terminal = `succeeded`/`partial`/`failed`; `runsFailed` counts `failed`). A null `sweep` means the company has never been swept. Offloaded (`spawn_blocking`). **UI entry point**: polled by the Coverage panel footer's status line after a backfill or sweep.

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

Status: implemented (v0.50.0, ADR 0075) — pending T7 real-company validation

(Commands + UI shipped in v0.50 T5, validated so far only against sample/mock data. Real-company validation and milestone closure are reserved for T7 per the [v0.50 plan](plans/v0.50-quality-frameworks-qualitative.md). This section is NOT tagged `Status: planned`: the docs-drift gate reads "planned" as "not yet in code" and fails when a planned section's commands already exist — these do.)

Qualitative assessment ([ADR 0075](adr/0075-qualitative-assessment-frameworks.md), `v0.50.0`) extends quality frameworks with **agent-assessed** criteria (moat, pricing power, recurring revenue, capital-allocation quality…) that cannot be reduced to a metric comparison. A qualitative criterion carries `kind: "qualitative"` and an owner-authored `assessmentGuidance` prompt seed instead of a DSL expression; the existing criterion writes (`create_framework_criterion` / `update_framework_criterion`) gain optional `kind` and `assessmentGuidance` fields for this (empty `expression` for qualitative rows). Assessment is one AI request **per criterion per company** (the `qualitative_assessment` capability, ADR 0060), grounded only in app-held evidence — report documents, research evidence links, claims + verdicts, recent signals, notebook notes; **no web access**. Each result is agent opinion (not a fact): `verdict` (`pass | partial | fail | insufficient_evidence`), short `reasoning`, `citations` (typed evidence refs reusing the research-evidence citation model — `evidenceType`, `evidenceId`, `label`, `snippet`), `confidence` (`low | medium | high`), `promptVersion`, and `source: "agent"`. Results are labeled, regeneratable, and never mutate quantitative data; stored `reasoning` must contain no buy/sell/hold or allocation language. Agent results merge into the same immutable evaluation snapshot as quantitative results (per-criterion `source`); verdict changes vs the previous snapshot surface in digests and on autopilot re-evaluation.

Two read surfaces with a fixed boundary (a snapshot may be quant-only, qual-only, or combined, so "the latest snapshot" is **not** a reliable source of qualitative rows):

- `get_framework_evaluation` / `list_framework_evaluations` return the qualitative fields **as snapshotted in that specific run** — the audit/history view of one evaluation, unchanged and immutable.
- `get_qualitative_assessment` is the Quality panel's **current-state** read: per qualitative criterion, the most recent agent-assessed row (`source = "agent"`) for the company × framework **across all snapshots**, so a later quant-only run never blanks an existing assessment.

Commands:

- `run_qualitative_assessment(input)`: `{ companyId, frameworkId }` → enqueues the durable `qualitative_assessment` job over the framework's qualitative criteria for the company; asynchronous, surfaced via the jobs read model. Fails with a clear error when no text-capable provider is configured (matches feed-analysis behavior).
- `rerun_qualitative_criterion(input)`: `{ companyId, frameworkId, criterionId }` → re-enqueues assessment for a single qualitative criterion (the panel's re-run action).
- `get_qualitative_assessment(input)`: `{ companyId, frameworkId }` → returns, **per qualitative criterion, the most recent agent-assessed result** (`source = "agent"`, latest by run) across the framework's evaluation snapshots for the company, with resolved citations (opening the cited evidence), confidence, prompt version, and `source`, for the Quality panel. A criterion with no assessment yet is omitted (empty state), distinct from an `insufficient_evidence` verdict.
- `get_qualitative_assessment_status(input)`: `{ companyId, frameworkId }` → `{ status: "idle" | "queued" | "running" | "failed" | "succeeded", attempts, lastError }` — the lifecycle of the durable `qualitative_assessment:<company>:<framework>` job row, so the panel can stop its bounded poll and surface a terminal failure (the backend `lastError`, e.g. no configured provider) instead of silently clearing the "queued" hint. Maps the queue row honestly (`pending → queued`); a missing row is `succeeded` when an assessment is already stored, else `idle`. `lastError` is only meaningful on `failed`.

Citations are rejected when they do not reference an evidence id supplied to the request (the research-brief `rejects_unknown_citation_keys` precedent): uncited reasoning is never stored.

**Output language** (owner decision 2026-07-07): AI prose output follows the persisted app `locale` — the qualitative-assessment prompt instructs the model to write `reasoning` and citation `label` in Polish/English accordingly (prompt version `qualitative-assessment.v2`), while citation `snippet` stays **verbatim in the evidence's original language** (attribution durability — a quote must remain exact). Unknown locale codes degrade to English. Existing stored assessments keep the language they were generated in; only new runs follow the setting. The same rule extends to the other prose-producing prompts (feed analysis summary, research brief/digest) as a carded follow-up.

### AI KPI Extraction

AI KPI extraction ([ADR 0028](adr/0028-multi-provider-ai-boundary.md), [ADR 0029](adr/0029-ir-page-report-resolution.md)) reads a stored report document with the selected AI provider and produces **proposals**. A proposal never becomes a `financial_fact` until the user confirms it; confirming materialises the period, resolves (or, for accepted suggestions, creates) the KPI definition, and writes the fact with `extractionMethod = "ai"` and the source document reference. Confirmed proposals are retained as the provenance trail (provider, model, prompt version live on the job; the verbatim source snippet and confidence on the proposal). Rejected proposals never persist a value.

An extraction job carries the detected primary period (`detectedFiscalYear`, `detectedPeriodType`, `detectedPeriodEndDate`), default currency/language, and its proposals. Each proposal carries `metricKey`, `label`, `valueNumeric` (decimal base-units text), `asReportedValue`/`asReportedScale`, `confidence` (`low|medium|high`), `sourceSnippet`, `isProposedKpi` (true for metrics beyond the supplied taxonomy), `status` (`pending|confirmed|rejected`), and `factId` once confirmed. Only the primary period is extracted; prior-year comparative columns are ignored.

Commands:

- `start_kpi_extraction(input)`: queues an async extraction over a report document (`reportDocumentId`, optional `periodHint`, optional `providerMode`); returns the queued job. **Rewired to the tier-4 OCR path** ([ADR 0077](adr/0077-trusted-extraction-foundations.md) §4 / T4.5) — one implementation with autopilot's fallback: the LLM never reads numbers. The reporting period is derived deterministically (title/URL/ESEF, not the model); a company with a confirmed OCR profile parses to VALIDATED facts committed directly (the job completes with zero proposals and an honest `committedFactCount`), while a never-bootstrapped company bootstraps the profile (labels only, via `vision_extraction`) and lands proposals for confirmation. A non-PDF document, a missing vision provider, or a bootstrap that returns no usable layout fails the job with an actionable error (`non_pdf_document` / `provider_error` / `parse_error`). Transient provider failures engage the queue's capped-backoff retry (ADR 0077 pacing fix).
- `retry_kpi_extraction(jobId)`: re-queues an existing job.
- `list_kpi_extraction(input)`: returns extraction jobs (with proposals) for one report document.
- `list_pending_kpi_proposals(companyId)`: the **F5 review-queue** read model ([ADR 0077](adr/0077-trusted-extraction-foundations.md) §4/§5, T5.3b). Returns `PendingKpiProposal[]` — every `pending` proposal for the company (confirmed/rejected excluded) joined to its job's detected period (`fiscalYear`/`periodType`, the grouping axis) and its source document (`documentId`/`documentTitle`/`documentUrl`), plus the `sourceSnippet` whose `ocr_bootstrap` / `ocr_pending_profile` / `ocr_flagged` prefix the UI derives the source chip from. Offloaded off the UI thread (three-table join). Confirm/reject reuse `confirm_kpi_proposal` / `reject_kpi_proposal`.
- `confirm_kpi_proposal(input)`: commits one proposal as a `financial_fact` (`proposalId`, optional `valueNumeric`/`currency` edit, optional `fiscalYear`/`periodType`/`periodEndDate` period override, `acceptAsNewKpi` for out-of-taxonomy suggestions). Returns `ConfirmedKpiFact { fact, validationStatus }`: the confirm now **validates** the value over the period's fact set like every other fact source ([ADR 0077](adr/0077-trusted-extraction-foundations.md) §4), recording a real `validationStatus` (`passed` | `flagged` | `unreviewed`; the retired `none` is never written) on the provenance row and surfacing it so the UI can flag a contradicted total. The confirmed fact always persists — the status records what validation saw, it does not block the confirm.
- `reject_kpi_proposal(proposalId)`: marks a proposal rejected; never writes a fact.

### IR-Page Report Resolution

The report-document source ladder ([ADR 0029](adr/0029-ir-page-report-resolution.md)) is: ESPI/EBI attachment (primary), per-company IR reports page (fallback), manual PDF URL paste (last resort).

- `get_company_ir_reports_url(companyId)` / `set_company_ir_reports_url(companyId, url)`: read/write the durable per-company IR reports page URL (empty clears it).
- `resolve_ir_report(input)`: fetches the company's IR page, extracts candidate links generically (no per-company scrapers), and has the AI pick the report matching the event context (`companyId`, optional `periodHint`/`reportType`/`publishedAt`). A confident pick is captured into `report_documents` and returned as `document`; otherwise `document` is null and `candidates` is returned for the user to choose. Event-driven automatic resolution is deferred to v0.49.0.

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
- `prune_old_feed_items`
- `delete_unsaved_feed_items`
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
- `start_research_digest`
- `list_research_digests`
- `start_ai_analysis`
- `list_ai_analysis`
- `retry_ai_analysis`
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
- `run_ai_event_derivation`
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
- `list_ai_provider_catalog`
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

Rules:

- Research-data JSON includes companies, watchlists, memberships, notebook entries, research questions, evidence links, AI research briefs, and AI research brief citations.
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

Initial `prune_old_feed_items` behavior:

- Runs as a separate asynchronous maintenance command, not as part of any source refresh path.
- Deletes unsaved feed items older than the requested retention window, initially scheduled by the desktop runtime once per day with a 30-day retention window.
- Settings exposes the current cleanup status, retention window, interval, and protected item class. Last-run and enable/configuration controls remain later v1 hardening work.
- Settings exposes the last cleanup timestamp and deleted item count for the current app session after the background cleanup command runs.
- Preserves saved feed items regardless of age.
- Removes dependent feed item company links, attachments, and AI analysis rows before deleting pruned feed items.
- Returns retention days, deleted item count, and prune timestamp.

Initial `delete_unsaved_feed_items` behavior:

- Runs only from an explicit user action with confirmation copy in the UI.
- Deletes all unsaved feed items regardless of source or age.
- Preserves saved feed items.
- Removes dependent feed item company links, attachments, and AI analysis rows before deleting feed items.
- Does not fetch, poll, or refresh external sources after deletion; the UI only reloads local SQLite-backed state.
- Returns deleted item count and deletion timestamp.

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

## Interpretation — Embedding Model

Typed commands for the interpretative AI layer's embedding-model strategy ([ADR 0035](adr/0035-two-layer-ai-and-local-interpretative-layer.md)), added in `v0.45.0`. All are local-only: the model runs on-device, no content leaves the machine, no API key.

### Embedding Model Status

`get_embedding_model_status` read model — the local model + index state surfaced in Settings and Developer-mode diagnostics.

```json
{
  "modelId": "intfloat/multilingual-e5-small",
  "dim": 384,
  "weightsState": "absent",
  "downloadProgress": null,
  "activeSimilarityStrategy": "static",
  "embeddedCounts": { "feed_item": 0 },
  "indexModelId": null
}
```

Rules:

- `weightsState` is one of `absent`, `downloading`, `ready`, `error`.
- `downloadProgress` is `null` unless `weightsState` is `downloading`; then it is a 0–100 integer percent.
- `activeSimilarityStrategy` is `static` or `embedding`; it must never report `embedding` while `weightsState` is not `ready`.
- `indexModelId` is the `model_id` the current vectors were built with, or `null` when the index is empty. A mismatch with `modelId` means a re-embed is pending.
- The command must not return weight bytes, file paths to secrets, or any provider key.

### Download Embedding Model

`download_embedding_model` — begin the optional one-time weights download into the app data directory; idempotent (a no-op when already `ready`).

Rules:

- Downloads `safetensors` + `tokenizer.json` for `modelId` and checksum-verifies before marking `ready`.
- Runs async; progress is observed via `get_embedding_model_status` (or an emitted event), not by blocking.
- A failed or interrupted download leaves `weightsState` at `error`/`absent` and never partially activates the model.
- Default CI and tests must not invoke a live download; the model-backed eval uses the locally-cached model and skips when absent.

### Select Similarity Strategy

`set_similarity_strategy` — choose the active `SimilarityProvider` implementation.

```json
{ "strategy": "embedding" }
```

Rules:

- `strategy` is `static` or `embedding`. Selecting `embedding` while weights are not `ready` is rejected with a recoverable error; it does not silently fall back.
- The selection is persisted as a local setting (see [Settings](data-model.md#settings)); the default is `static`.
- Switching to `embedding` enqueues the embed job to populate any missing vectors; switching to `static` leaves the index in place (it is disposable and may be reused later).

### Rebuild Embedding Index

`rebuild_embedding_index` — manually re-runs the embed/re-embed job (the same job `set_similarity_strategy` enqueues on switch to `embedding`) over any content missing a vector or embedded with a stale `modelId`, then returns the refreshed `get_embedding_model_status` shape. Offloaded (`spawn_blocking`); surfaces a job error (e.g. a model load/forward failure) rather than swallowing it.

### Find Similar Content (diagnostics)

`find_similar_content` — developer-mode/diagnostics command to exercise the active `SimilarityProvider` over real stored content; the `v0.45.0` demoable surface for the embedding model. (Its first intended product consumer, story clustering `v0.46`, was evaluated and dropped — see [ADR 0051](adr/0051-story-clustering-across-sources.md); the `SimilarityProvider` / embedding model is re-pointed at ranking/retrieval consumers — semantic search `v0.48` and RAG retrieval for the AI milestones — where the user or an LLM makes the final call.)

```json
{
  "contentType": "feed_item",
  "contentId": "feed_01",
  "k": 10
}
```

Returns ranked `{ contentType, contentId, score }` items, highest score first, plus the `strategyId` that produced them so the model-vs-static result is visible. Scores are relative within one call and not comparable across strategies.
