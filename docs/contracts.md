# Contracts

This file defines initial contracts for the first implementation. Field names are intentionally stable enough for code scaffolding, but exact serialization may be refined with tests before the first API release.

See also [Project Brief](project-brief.md), [Architecture](architecture.md), [Data Model](data-model.md), [Source Strategy](source-strategy.md), [Project Practices](project-practices.md), and [Product Spec](product-spec.md).

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
  "enabled": true,
  "defaultPollIntervalSeconds": 900,
  "lastSuccessAt": null,
  "lastErrorAt": null,
  "lastError": null
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
- The Sources screen reads adapter status from local SQLite before real fetching exists.

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
  "attribution": "GPW"
}
```

Rules:

- `publishedAt` may be null only when the source does not provide it.
- `fetchedAt` is always required.
- `dedupeKey` must be stable for the same source item.
- `displayCompany` is allowed in UI-facing read models so the Inbox can show a ticker label even before a full canonical company relationship is available. Canonical storage still uses `companies`/`feed_item_companies`.
- `read` and `saved` are user state and must persist locally.
- Original source text should be retained when legally and technically allowed.

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
  "sourceUrl": "https://www.gpw.pl/komunikaty",
  "attribution": "GPW",
  "fetchedAt": "2026-05-30T12:00:00Z",
  "updatedAtSource": null,
  "manual": false,
  "userAdjusted": false
}
```

Initial event types:

- `periodic_report`
- `dividend`
- `shareholder_meeting`
- `conference_call`
- `investor_conference`
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
- Manual events use `sourceType: "manual"` and `manual: true`.
- User corrections to sourced events must not erase the original source record; later implementation may store corrections as separate fields or linked audit records.

## AI Analysis Result

```json
{
  "id": "analysis_01",
  "feedItemId": "feed_01",
  "providerId": "provider_openai_compatible",
  "model": "configured-model-name",
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

## Video Transcript Job

```json
{
  "jobId": "job_video_01",
  "companyId": "company_gpw_cdr",
  "providerId": "provider_gemini",
  "sourceType": "youtube_url",
  "sourceUrl": "https://www.youtube.com/watch?v=example",
  "status": "queued",
  "createdAt": "2026-05-28T13:30:00Z",
  "startedAt": null,
  "finishedAt": null,
  "error": null
}
```

## Transcript Segment

```json
{
  "id": "segment_01",
  "transcriptJobId": "job_video_01",
  "companyId": "company_gpw_cdr",
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
- The original YouTube URL must be retained.
- Transcript segment text is immutable source output in v1.
- Notes created from transcript segments are editable before saving.

## Transcript-To-Note Selection

```json
{
  "companyId": "company_gpw_cdr",
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
- Selection can be implemented as whole-segment selection, text-range selection, or accepting an AI-suggested draft.
- AI may suggest note drafts, but the user confirms before saving.
- Saved notes must link back to transcript segments and the original video URL.

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

## User Settings

```json
{
  "theme": "dark",
  "accentPalette": "night-neon",
  "pollIntervalSeconds": 900,
  "settingsSource": "sqlite",
  "settingsImportExportFormat": "yaml",
  "aiProviders": {
    "youtubeTranscriptionProvider": "provider_gemini",
    "generalAnalysisProvider": null
  },
  "aiAnalysisMode": "source_grounded"
}
```

Allowed theme values:

- `dark`
- `light`
- `system`

Rules:

- The default theme is `dark`.
- `system` may be added to the UI as a convenience, but first-run behavior still defaults to `dark` until the user changes it.
- The initial accent palette is `night-neon`, inspired by deep navy, electric blue/cyan, pink, and purple.
- General AI analysis has no default provider yet.
- SQLite is the runtime source of truth for settings.
- YAML is allowed for settings import/export/bootstrap.
- YAML settings import/export/bootstrap is contract-accepted but implementation-deferred until the later export/import/backup roadmap work.
- YAML must not contain secrets.
- API keys and provider secrets live in the OS keychain.
- Default AI analysis mode is `source_grounded`.
- Future `opinionated` mode requires explicit user opt-in and still cannot provide buy/sell/hold or personalized portfolio advice.

## UI-Facing Command Boundaries

Initial Tauri command groups:

- `health`
- `list_companies`
- `create_company`
- `list_watchlists`
- `create_watchlist`
- `list_feed_items`
- `update_feed_item`
- `refresh_sources`
- `list_jobs`
- `list_notebook_entries`
- `create_notebook_entry`
- `update_notebook_entry`
- `create_video_transcript_job`
- `list_transcript_segments`
- `create_note_from_transcript_selection`
- `get_settings`
- `update_settings`

Feed, job, transcript, and notebook changes should be emitted as Tauri events.
