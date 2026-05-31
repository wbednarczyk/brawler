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
- `prune_old_feed_items`
- `delete_unsaved_feed_items`
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
- `list_transcript_segments`
- `create_note_from_transcript_selection`
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
