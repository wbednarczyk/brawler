# Contracts

This file defines initial contracts for the first implementation. Field names are intentionally stable enough for code scaffolding, but exact serialization may be refined with tests before the first API release.

See also [Project Brief](project-brief.md), [Architecture](architecture.md), and [Product Spec](product-spec.md).

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

## Source Adapter

Each source adapter must expose metadata and a fetch operation.

```json
{
  "adapterId": "gpw-espi-ebi",
  "displayName": "GPW ESPI/EBI",
  "sourceType": "official_report",
  "supportedMarkets": ["GPW"],
  "fetchMode": "public_page",
  "defaultPollIntervalSeconds": 900
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
- Original source text should be retained when legally and technically allowed.

## Notebook Entry

```json
{
  "id": "note_01",
  "companyId": "company_gpw_cdr",
  "title": "Management claim about release schedule",
  "body": "Management said the next major release milestone should happen in the next two quarters.",
  "tags": ["management-guidance", "product"],
  "kind": "claim",
  "claimStatus": "open",
  "eventDate": "2026-05-28",
  "reviewAfter": "2026-Q4",
  "createdAt": "2026-05-28T13:20:00Z",
  "updatedAt": "2026-05-28T13:20:00Z",
  "provenance": [
    {
      "type": "feed_item",
      "id": "feed_01",
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
- Notes created from feed items or transcripts must retain provenance.
- Claim notes should support a future review period, but review automation is not required in the first implementation.

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
- Transcript text should be editable before turning it into a note.

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
    "reviewAfter": "2026-Q4"
  }
}
```

Rules:

- The user chooses which transcript segments become notes.
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
  "aiProviders": {
    "youtubeTranscriptionProvider": "provider_gemini",
    "generalAnalysisProvider": null
  }
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
