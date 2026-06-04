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
- UI ticker labels may split `qualifiedTicker` into exchange and symbol for styling, including per-exchange colors, but command payloads and storage continue to use the unchanged `qualifiedTicker` string.

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
- Deleting a company removes its watchlist memberships through local referential integrity.
- UI-facing read models may list watchlist memberships separately from company identity so company identity remains canonical and watchlist-specific views stay explicit.

## Source Adapter

Each source adapter must expose metadata and a fetch operation.

```json
{
  "adapterId": "gpw-espi-ebi",
  "displayName": "GPW ESPI/EBI",
  "sourceType": "official_report",
  "supportedMarkets": ["GPW"],
  "fetchMode": "public_page",
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
- `sourceType` must distinguish `official_report`, `public_media`, `analysis`, `authenticated_research`, and future source categories where relevant.
- `accessMode` must distinguish `public`, `rss`, `paywalled`, `authenticated`, and `manual` sources where relevant.
- Authenticated private sources require a source-specific ADR before implementation. Portal Analiz is governed by [ADR 0014](adr/0014-portal-analiz-authenticated-source-policy.md).
- Authenticated adapters must use OS keychain secrets or session storage and must not export credentials to YAML, backup files, logs, or test samples.
- `portal-analiz` exists only as a disabled late-v1 placeholder until the authenticated-source implementation is explicitly built. It must not be fetched by normal refresh commands.
- The Sources screen reads adapter status from local SQLite before real fetching exists.
- The Sources screen must expose the adapter source URL, rate-limit policy, and source-policy note once live fetching exists.
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

## Licensing

M17 exposes a local signed entitlement gate for author and friend-test builds. It is not the final public license model and does not add hosted activation, billing, telemetry, or cloud accounts.

License status read model:

```json
{
  "status": "valid",
  "canUseApp": true,
  "reason": null,
  "license": {
    "licenseId": "lic_author_001",
    "holder": "Project Author",
    "channel": "author",
    "edition": "author",
    "features": ["*"],
    "issuedAt": "2026-06-01T00:00:00Z",
    "expiresAt": "2099-01-01T00:00:00Z",
    "appVersionRange": "*",
    "keyId": "owner_author_2026_06"
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

- Normal app navigation requires `canUseApp = true` for M17 author and friend-test builds.
- Missing, malformed, tampered, expired, unsupported-version, unsupported-channel, and storage-error states must remain recoverable through the local license screen.
- Supported M17 channels are `author` and `friend_test`; unsupported channels are invalid for this build.
- Author tokens require `edition: "author"`, `features: ["*"]`, and the author signing key id.
- Friend-test tokens require `channel: "friend_test"` and the friend-test signing key id.
- M17 author and friend-test tokens are not app-version bounded; `appVersionRange` remains compatibility metadata and is `*` for generated M17 tokens.
- Future entitlement channels may opt into app-version limits through the existing `appVersionRange` policy path.
- `submit_license_key` validates the token offline before saving it. Invalid replacement attempts must not overwrite an existing valid key.
- The raw license token is treated as a bearer secret and stored through the OS keychain.
- SQLite stores only derived redacted license metadata/status and never stores the full token, private signing material, or private key material.
- React receives only `LicenseStatus`; it never receives private signing material.
- Logs, diagnostics, metrics, settings export, tests, and UI state must not include full license tokens, private signing material, or raw private key material.
- Future community/open-core, paid feature, subscription, or hosted activation policies must be added as entitlement-policy or verifier/storage adapters and require a later ADR when they introduce hosted services or billing.

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
  "shortcutBindings": {}
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
- SQLite is the runtime source of truth for settings.
- YAML is allowed for settings import/export/bootstrap.
- YAML settings import/export/bootstrap is contract-accepted but implementation-deferred until the later export/import/backup roadmap work.
- YAML must not contain secrets.
- API keys and provider secrets live in the OS keychain.
- `.env` or environment-variable API key fallback is allowed for local development and tests only.
- Default AI analysis mode is `source_grounded`.
- Future `opinionated` mode requires explicit user opt-in and still cannot provide buy/sell/hold or personalized portfolio advice.

## Provider Credential Status

```json
{
  "providerId": "provider_gemini",
  "purpose": "youtube_transcription",
  "secretKind": "api_key",
  "configured": true,
  "storage": "os_keychain",
  "label": "Gemini YouTube transcription API key",
  "devFallbackAvailable": false,
  "error": null
}
```

Rules:

- Credential status is non-secret metadata and may be returned to React.
- Secret values must never be returned to React.
- Runtime secret storage uses the OS keychain.
- Development/test fallback may use environment variables, but this must be reported as `storage = "development_environment"` and must not count as exported settings.
- Supported `secretKind` values begin with `api_key`; future supported kinds may include `username_password`, `session_token`, or `oauth_token` after source-specific design.
- `set_gemini_transcription_api_key(apiKey)` stores or replaces only the Gemini YouTube transcription API key.
- `clear_gemini_transcription_api_key()` removes only the OS-keychain Gemini YouTube transcription API key and must not mutate `.env` or process environment values.

## UI-Facing Command Boundaries

Initial Tauri command groups:

- `health`
- `list_companies`
- `create_company`
- `list_watchlists`
- `create_watchlist`
- `list_feed_items`
- `update_feed_item`
- `prune_old_feed_items`
- `delete_unsaved_feed_items`
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
- `create_video_transcript_job`
- `list_video_transcript_jobs`
- `delete_video_transcript_job`
- `update_video_transcript_job`
- `run_video_transcript_job`
- `resolve_transcript_job_company`
- `list_transcript_segments`
- `create_note_from_transcript_selection`
- `get_gemini_transcription_credential_status`
- `set_gemini_transcription_api_key`
- `clear_gemini_transcription_api_key`
- `get_settings`
- `update_settings`

Initial `refresh_gpw_company_registry` behavior:

- Runs the GPW company registry adapter path.
- Fetches the public GPW companies page with a high limit, currently `https://www.gpw.pl/spolki?offset=0&limit=500`, so the cache represents all currently listed GPW companies exposed by that page.
- Stores registry rows in SQLite under `company_registry_entries`.
- Upserts by `exchange + ticker`.
- Preserves user-managed `companies` records and does not overwrite them silently.
- Records adapter attempt/success/error state under `gpw-company-registry`.
- Automated tests use test-sample-backed parser/fetch behavior; default checks do not depend on live GPW availability.

Initial `refresh_gpw_company_registry_if_stale` behavior:

- Runs only for scheduler-triggered refreshes.
- Checks local adapter freshness before making a live request.
- Uses the registry adapter poll interval, initially one day, as the stale threshold.
- Returns no refresh result when the cached registry is still fresh.
- Does not run immediately on app startup; the first scheduled check happens after one full registry poll interval while the app is open.

Initial `list_company_registry_entries` behavior:

- Returns active cached GPW company registry rows from SQLite.
- Includes exchange, ticker, qualified ticker, display name, ISIN, source URL, fetched timestamp, and whether the company is already tracked locally.
- Supports the Companies form registry suggestions and the Sources screen registry detail panel.
- The Companies form can use cached registry matches to fill exchange, ticker, display name, and ISIN while preserving manual company entry.
- The Sources registry list is collapsed by default, searchable by ticker/company/ISIN, and each untracked company can be added to the local company list.
- Does not fetch live data by itself; refresh is handled by `refresh_gpw_company_registry` or lookup bootstrap behavior.

Initial `lookup_company` behavior:

- Looks up GPW companies from the local `company_registry_entries` cache.
- Uses exact ticker first, exact ISIN second, and company-name search only for company-form lookup/enrichment.
- If a GPW lookup misses while the registry cache is empty, the command may refresh the full GPW registry once and retry the lookup.
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
