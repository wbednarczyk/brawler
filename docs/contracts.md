# Contracts

This file defines initial contracts for the first implementation. Field names are intentionally stable enough for code scaffolding, but exact serialization may be refined with tests before the first API release.

Use [Project Brief](project-brief.md) for the full documentation map. Related references: [Architecture](architecture.md), [Data Model](data-model.md), [Source Strategy](source-strategy.md), and [Product Spec](product-spec.md).

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

Rules:

- `qualifiedTicker` is unique in local storage.
- `ticker` alone is not unique.
- `isin`, `cik`, and `lei` are optional.
- Source adapters may attach source-specific IDs without changing the canonical identity.
- UI ticker labels may split `qualifiedTicker` into exchange and symbol for styling, including explicit known-exchange colors and deterministic fallback colors for future exchanges, but command payloads and storage continue to use the unchanged `qualifiedTicker` string.

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
  ]
}
```

Rules:

- `publishedAt` may be null only when the source does not provide it.
- `fetchedAt` is always required.
- `dedupeKey` must be stable for the same source item.
- `displayCompany` is allowed in UI-facing read models so the Inbox can show a ticker label even before a full canonical company relationship is available. Canonical storage still uses `companies`/`feed_item_companies`.
- `read` and `saved` are user state and must persist locally.
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

Rules:

- Notes belong to exactly one canonical company.
- Note body format is Markdown in v1.
- Notebook read views may render common Markdown, but stored `body` remains the canonical Markdown source.
- Claim notes may include both `followUpAfter` for quarter/period follow-up and `followUpDate` for exact date follow-up.
- Notes created from feed items or transcripts must retain origin links.
- Notebook UI surfaces should render origin links in note details and make feed-item origins actionable inside the app when the referenced feed item is still available locally.
- Origin links are immutable through normal note editing. Future workflows may add or detach origins through explicit source-link actions, but inline note editing must not rewrite origin records.
- Feed-to-note drafts start from UI-facing feed items, which are scoped to tracked companies; `create_notebook_entry` requires the canonical tracked `companyId`.
- Claim notes should support a future follow-up period, but follow-up automation is not required in the first implementation.
- The Claims tab uses `list_notebook_entries(companyId)` and filters entries whose `kind` is `claim` or whose `claimStatus` is set.
- Claim status update uses `update_notebook_entry(input)` and must preserve company ownership, origin links, note body, tags, and follow-up dates unless the user is editing those fields in a notebook editor.

Initial local commands:

- `list_notebook_entries(companyId)`: returns notebook entries for one company, newest updated first.
- `create_notebook_entry(input)`: creates one Markdown notebook entry for a company.
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

Rules:

- Events belong to exactly one canonical company.
- Event views should be scoped to companies in watchlists by default.
- Sourced events must preserve source URL, attribution, fetched timestamp, and source type when available.
- Sourced event identity uses `(sourceAdapterId, sourceEventKey)` when both are present.
- If an accepted source publishes a changed event under the same source identity, ingestion updates the existing sourced event instead of creating a second correction row.
- `gpw-market-events-rss` consumes GPW's official market-events RSS feed at `https://www.gpw.pl/rss-calendar-of-market-events` and creates events only for tracked companies matched by exact ticker.
- `bankier-kalendarium-html` consumes the public Bankier Kalendarium page at `https://www.bankier.pl/gielda/kalendarium` and creates `public_calendar` events for tracked companies matched by exact ticker.
- The Bankier adapter may fetch week-specific calendar pages using Bankier's `navigation_type=week&navigation_start=<unix timestamp>` query parameters when the Events week view needs a week that is not cached locally.
- Bankier event identity is based on ticker, event category, and event description so a date changed by the source updates the existing event instead of creating a correction row.
- Hidden/empty Bankier calendar RSS endpoints are not accepted as reliable until direct checks prove stable populated content.
- Manual events use `sourceType: "manual"` and `manual: true`.
- Manual events are for missing or user-known dates, not corrections to normal source updates.

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

Rules:

- The UI input label for `sourceUrl` is `URL`.
- `companyId` may be null at job creation time.
- If the user provides a ticker/company before transcription, `companyId` is set immediately.
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

Initial module IDs:

- `ai_analysis`
- `external_ai`
- `sources`
- `scheduler`
- `credentials`
- `storage`
- `transcripts`
- `shortcuts`
- `locale`
- `licensing`
- `packaging`

Initial severity values:

- `debug`
- `info`
- `warning`
- `error`

Stage naming rules:

- Use stable snake-case stage IDs.
- Use past-tense stage IDs for completed steps, for example `context_loaded`, `provider_resolved`, `credential_checked`, `request_sent`, `response_received`, `result_stored`, and `failed`.
- Use job lifecycle stage IDs when describing async work, for example `queued`, `running`, `succeeded`, `cancelled`, and `failed`.
- Reuse stage IDs across modules when the meaning is the same.
- Do not encode dynamic values into stage IDs.

Scope rules:

- `scope.type` identifies the entity category, such as `ai_analysis_job`, `feed_item`, `source_adapter`, `transcript_job`, `setting`, or `shortcut_action`.
- `scope.id` is nullable only when the event is truly global to the module.
- Scope IDs should be stable local IDs, not titles, URLs, prompts, source text, or provider response snippets.

Metadata rules:

- Metadata must be structured JSON and must remain small enough for a timeline row/detail panel.
- Metadata may include stable IDs, provider IDs, model names, adapter IDs, status values, durations, counts, timeout values, retry counts, error classes, and boolean flags.
- Metadata must not include API keys, full prompts, full source bodies, full transcript text, raw provider responses, license private material, or full license secrets by default.
- Redaction must happen before persistence.
- Redacted values should be omitted when possible; when omission would make the event confusing, use a fixed marker such as `[redacted]`.
- The event shape should stay cheap to map to future OpenTelemetry-style event/span fields, but M14 does not implement OpenTelemetry exporters or remote reporting.

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

Rules:

- Diagnostic events are recorded only while Developer mode is enabled.
- Diagnostic events remain local-only in SQLite.
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

Rules:

- Normal app navigation requires `canUseApp = true`; public-opening policy sets `canUseApp = true` for missing, invalid, expired, wrong-version, unsupported-version, and storage-error entitlement states so the open core remains usable.
- Missing, malformed, tampered, expired, unsupported-version, unsupported-channel, and storage-error states must remain recoverable through Settings.
- Supported channels are build-policy specific; unsupported channels are invalid for that build.
- Public-opening entitlement tokens are optional for normal open-core use.
- `appVersionRange` remains compatibility metadata and may be `*` for channels that are not app-version bounded.
- Future entitlement channels may opt into app-version limits through the existing `appVersionRange` policy path.
- `submit_license_key` validates the token offline before saving it. Invalid replacement attempts must not overwrite an existing valid key.
- The raw license token is treated as a bearer secret and stored through the OS keychain.
- SQLite stores only derived redacted license metadata/status and never stores the full token, private signing material, or private key material.
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

Rules (see [ADR 0032](adr/0032-search-and-backup-boundaries.md)):

- One typed search command queries the unified `search_index` FTS5 table; DTOs live in `src/api/search.ts` and command modules contain no SQL.
- `query` is sanitized before reaching `MATCH`; user input is never interpolated as FTS5 syntax. An empty/blank query returns no groups.
- `contentTypes` and `companyId` are optional scoping filters. Omitting `contentTypes` searches all types.
- Matches are ranked by `bm25()` and returned grouped by `contentType`, each carrying `sourceId`, `companyId`, `parentId`, `title`, `snippet`, and `score` — enough context to render and navigate to the specific item.
- `parentId` is the navigational container when `sourceId` is not the navigation target (a transcript segment carries its transcript job id); it is `null` otherwise. Snippet highlight markers are control characters (STX/ETX), not HTML, so callers render snippets as plain text.
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

Rules (see [ADR 0032](adr/0032-search-and-backup-boundaries.md)):

- Typed commands expose backup status, list, create-now, and restore; UI never touches backup files directly.
- Backups and pre-migration snapshots are produced with `VACUUM INTO` into `<app_data_dir>/backups/`; rotating backups keep the last N and prune the oldest.
- `kind` distinguishes `rotating` backups from pre-migration `snapshot` files.
- Restore requires explicit confirmation, is surfaced in Diagnostics, and is applied on app relaunch (staged, not a hot in-place swap).
- Backups are local-only byte-faithful copies of the database; they are distinct from import/export documents and contain no keychain secrets. No cloud backup.

## User Settings

```json
{
  "theme": "dark",
  "locale": "en",
  "accentPalette": "night-neon",
  "developerMode": false,
  "pollIntervalSeconds": 900,
  "settingsSource": "sqlite",
  "settingsImportExportFormat": "yaml",
  "aiProviders": {
    "youtubeTranscriptionProvider": "provider_gemini",
    "youtubeTranscriptionModel": "gemini-2.5-flash",
    "youtubeTranscriptionTimeoutSeconds": 300,
    "generalAnalysisProvider": null,
    "generalAnalysisModel": "gemini-2.5-flash",
    "generalAnalysisTimeoutSeconds": 90
  },
  "aiAnalysisMode": "source_grounded",
  "shortcutBindings": {},
  "database": {
    "maxConnections": 4,
    "busyTimeoutMs": 5000,
    "acquireTimeoutMs": 10000
  }
}
```

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

Rules:

- The default theme is `dark`.
- `theme` controls brightness mode only. `accentPalette` controls the semantic color palette.
- The default accent palette is `night-neon`.
- `midnight-horizon` maps the project owner's sampled reference-image colors onto semantic UI tokens: background `#00021E`, surface `#061135`, primary `#63C0E9`, secondary `#55388F`, accent `#C550B9`, highlight `#FB82C0`, and text `#EAF7FF`.
- The default locale is `en`.
- The default Developer mode setting is `false`.
- The default runtime log level is `info`.
- The default runtime log rotation limit is five files of five MiB each.
- Settings must expose local runtime log level and rotation limits as normal visible settings.
- Developer mode may be enabled only through intentional local developer mechanisms, not through a normal always-visible Settings toggle.
- Startup activation uses `BRAWLER_DEVELOPER_MODE=1`, `true`, `yes`, or `on`.
- Runtime author unlock may enable Developer mode after the app is already running only when `BRAWLER_DEVELOPER_UNLOCK_CODE` is present in the app process environment and the submitted passphrase matches it.
- The runtime author unlock entry point is hidden from normal UI and must not be registered as a configurable shortcut.
- Once Developer mode is active, the Diagnostics panel may show active status and a disable action.
- Settings must let the user switch the app locale between English and Polish in M12.
- Locale handling must be implemented as an extensible app-locale boundary so future supported locales can be added through locale resources/configuration instead of per-screen rewrites.
- Locale changes affect app-owned UI copy and formatting labels only.
- Source-provided text, company names, ticker symbols, URLs, source attribution, transcript text, and notebook bodies retain their original or user-entered language.
- `system` may be added to the UI as a convenience, but first-run behavior still defaults to `dark` until the user changes it.
- Accent palettes must be added through the settings validation and theme-token registry, not as component-local color overrides.
- General AI analysis is deferred until the AI analysis framework milestone. That milestone may enable `provider_gemini` first, but the contract must remain provider-neutral so future OpenAI, Anthropic, and other providers can be added without rewiring the UI.
- General AI analysis runs through asynchronous local job state so provider calls do not block the UI.
- Settings must let the user choose the general analysis provider and model from supported configured options once M13 is implemented.
- Settings must let the user choose the general analysis timeout from supported configured options: `45`, `90`, `180`, `300`, and `600` seconds.
- The default general analysis timeout is `90` seconds.
- Settings must show that `provider_gemini` is selected only for YouTube transcription.
- Settings must let the user choose the Gemini transcription model from supported configured options: `gemini-2.5-flash-lite`, `gemini-2.5-flash`, `gemini-3.1-flash-lite`, and `gemini-3.5-flash`.
- The default YouTube transcription model is the cheapest configured model validated by M10 live smoke, currently `gemini-2.5-flash`.
- Settings must let the user choose the Gemini transcription timeout from supported configured options: `45`, `90`, `180`, `300`, and `600` seconds.
- The default YouTube transcription timeout is `300` seconds. Shorter values are useful for smoke testing provider availability; longer values are intended for real conference videos.
- Settings must show whether YouTube transcription credentials are configured.
- Settings must let the user save, replace, and clear the Gemini API key used only for YouTube transcription.
- Settings must disclose before use that starting a transcript job sends the YouTube URL and video content to Gemini.
- Settings must let the user configure, disable, and reset every defined shortcut action through stable shortcut action IDs.
- Settings/About must show local license status and allow valid users to inspect safe metadata, replace the token, and clear the token.
- Shortcut binding overrides are stored as a JSON object keyed by action ID. Missing entries use the current default binding for that action.
- Shortcut conflicts must be visible before an enabled binding can silently shadow another enabled action.
- Settings expose database connection-pool tuning under `database`: `maxConnections`, `busyTimeoutMs`, and `acquireTimeoutMs` (see [ADR 0032](adr/0032-search-and-backup-boundaries.md)).
- The default pool configuration is `maxConnections` 4, `busyTimeoutMs` 5000, `acquireTimeoutMs` 10000.
- Pool values are validated and clamped to safe ranges (`maxConnections` 1–16, `busyTimeoutMs` 0–60000, `acquireTimeoutMs` 1000–60000); a missing or invalid value falls back to the default so the database can always open.
- Pool sizing is applied when the pool is built at startup, so changes persist immediately but take effect on the next app launch; Settings must disclose this.
- Settings must offer a reset-to-defaults action for database pool configuration.
- SQLite is the runtime source of truth for settings.
- YAML is allowed for settings import/export/bootstrap.
- YAML settings import/export/bootstrap is contract-accepted but implementation-deferred until the later export/import/backup roadmap work.
- M20 implements YAML settings import/export for allowlisted non-secret settings only.
- YAML must not contain secrets.
- API keys and provider secrets live in the OS keychain.
- `.env` or environment-variable API key fallback is allowed for local development and tests only.
- Default AI analysis mode is `source_grounded`.
- Future `opinionated` mode requires explicit user opt-in and still cannot provide buy/sell/hold or personalized portfolio advice.

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
- Development/test fallback may use environment variables (`GEMINI_API_KEY`, `ANTHROPIC_API_KEY`, `OPENAI_API_KEY`), but this must be reported as `storage = "development_environment"` and must not count as exported settings.
- Supported `secretKind` values begin with `api_key`; future supported kinds may include `username_password`, `session_token`, or `oauth_token` after source-specific design.
- Credential commands are generic and keyed by `providerId` (`provider_gemini`, `provider_anthropic`, `provider_openai`):
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
  }
]
```

Rules:

- The active analysis provider and model are the `generalAnalysisProvider` / `generalAnalysisModel` settings; the selected model must belong to the selected provider's catalog entry.
- Exact model ids are curated server-side; the UI must not hardcode model lists.

## Research Evidence Boundary

The research workspace uses a dedicated research/evidence boundary governed by [ADR 0022](adr/0022-research-evidence-read-model-boundary.md). It is a cross-domain read-model boundary, not a replacement for existing domain contracts.

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

Initial company timeline result shape:

```json
{
  "items": [
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
  ],
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
- Stored timeline/evidence projections are deferred until performance or review semantics require them.
- Review checkpoints are durable research-owned state.
- Evidence links are durable research-owned relationships between existing domain entities.
- Existing notebook origins remain provenance records and are not replaced by evidence links.
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

Rules:

- Financial periods belong to exactly one canonical company.
- Periods are unique on `(companyId, fiscalYear, periodType)`.
- `periodEndDate` is optional and records the reported end date for the period.
- `reportEvidenceRef` is a soft reference to a source document or feed item for future audit linkage.

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

- Definitions are unique on `(metricKey, scope, IFNULL(companyId, ''), IFNULL(sector, ''))`.
- Canonical packs are seeded and include universal, industrial, cash flow, capital efficiency (derived), and sector-specific packs (insurance, banking, specialty finance, REIT).
- Sector values in company fundamentals match those used in company statement classification.
- Derived metrics (margins, FCF, ROE/ROIC, net-debt/EBITDA) are computed at read time from confirmed financial facts.

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

Rules:

- Relevance records are unique on `(companyId, definitionId)`.
- A financial fact may exist for a KPI not yet active in the relevance profile (awaiting curation).
- Relevance tracks the first and last period in which a KPI was reported or relevant.

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

Rules:

- Financial facts are unique on `(periodId, definitionId, statementBasis, attribution, variant, measureWindow, dataQuality)` so estimated and final values coexist for the same fact.
- `valueNumeric` is stored as exact decimal text in base units (signed). Negative values are supported for liabilities and losses.
- `asReportedValue` and `asReportedScale` preserve the source form for auditability (e.g., "245 253 tys. zł").
- `supersedesId` marks a fact that was superseded (final replaces estimate), keeping history for audit trails.
- `sourceDocumentRef` is a soft reference to the feed item or report document from which the value was extracted.
- Derived metrics are computed at read time from confirmed facts and are unavailable when required inputs are missing.

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
- `localPath` is the app-owned storage path for fetched documents. Paths are relative to app data directory or absolute, depending on platform.
- `contentHash` is SHA256 and enables deduplication and integrity checking.
- `title` is optional user-facing metadata; official report titles are preserved when available.
- `attribution` is source-level credit (e.g., "GPW", "Company Web Site").
- ESPI-attachment ingestion and automatic backfill are deferred and not yet implemented.

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
- `list_report_documents(companyId)`: returns all captured report documents for one company.

Input shapes follow the corresponding domain types above. Return shapes include all domain fields plus timestamps. Company fundamentals data must be treated as owner-durable state in import/export and backup workflows.

### AI KPI Extraction

AI KPI extraction ([ADR 0028](adr/0028-multi-provider-ai-boundary.md), [ADR 0029](adr/0029-ir-page-report-resolution.md)) reads a stored report document with the selected AI provider and produces **proposals**. A proposal never becomes a `financial_fact` until the user confirms it; confirming materialises the period, resolves (or, for accepted suggestions, creates) the KPI definition, and writes the fact with `extractionMethod = "ai"` and the source document reference. Confirmed proposals are retained as the provenance trail (provider, model, prompt version live on the job; the verbatim source snippet and confidence on the proposal). Rejected proposals never persist a value.

An extraction job carries the detected primary period (`detectedFiscalYear`, `detectedPeriodType`, `detectedPeriodEndDate`), default currency/language, and its proposals. Each proposal carries `metricKey`, `label`, `valueNumeric` (decimal base-units text), `asReportedValue`/`asReportedScale`, `confidence` (`low|medium|high`), `sourceSnippet`, `isProposedKpi` (true for metrics beyond the supplied taxonomy), `status` (`pending|confirmed|rejected`), and `factId` once confirmed. Only the primary period is extracted; prior-year comparative columns are ignored.

Commands:

- `start_kpi_extraction(input)`: queues an async extraction over a report document (`reportDocumentId`, optional `periodHint`, optional `providerMode`); returns the queued job. A document that resolves to a web page rather than a report PDF (content type `text/html`, e.g. an IR landing page captured as a document) is rejected with error code `non_pdf_document` and an actionable message instead of producing a misleading partial extraction.
- `retry_kpi_extraction(jobId)`: re-queues an existing job.
- `list_kpi_extraction(input)`: returns extraction jobs (with proposals) for one report document.
- `confirm_kpi_proposal(input)`: commits one proposal as a `financial_fact` (`proposalId`, optional `valueNumeric`/`currency` edit, optional `fiscalYear`/`periodType`/`periodEndDate` period override, `acceptAsNewKpi` for out-of-taxonomy suggestions); returns the created fact.
- `reject_kpi_proposal(proposalId)`: marks a proposal rejected; never writes a fact.

### IR-Page Report Resolution

The report-document source ladder ([ADR 0029](adr/0029-ir-page-report-resolution.md)) is: ESPI/EBI attachment (primary), per-company IR reports page (fallback), manual PDF URL paste (last resort).

- `get_company_ir_reports_url(companyId)` / `set_company_ir_reports_url(companyId, url)`: read/write the durable per-company IR reports page URL (empty clears it).
- `resolve_ir_report(input)`: fetches the company's IR page, extracts candidate links generically (no per-company scrapers), and has the AI pick the report matching the event context (`companyId`, optional `periodHint`/`reportType`/`publishedAt`). A confident pick is captured into `report_documents` and returned as `document`; otherwise `document` is null and `candidates` is returned for the user to choose. Event-driven automatic resolution is deferred to v0.47.0.

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
- `update_feed_item`
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
- `list_jobs`
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
