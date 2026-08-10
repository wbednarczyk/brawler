# Source Strategy

This document defines the first source strategy for Brawler. It focuses on GPW because v1 prioritizes Polish market coverage.

Doc map: [CLAUDE.md](../CLAUDE.md) § Required Reading. Related references: [Product Spec](product-spec.md), [Data Model](data-model.md), [Contracts](contracts.md), and source-specific ADRs.

## Strategy Principles

- Prefer official, public, or RSS-like sources.
- Preserve source attribution and source URLs.
- Store enough origin to audit every feed item.
- Keep adapters modular and source-specific.
- Avoid restricted or fragile scraping unless a source-specific ADR approves it.
- **The app is an automaton — manual import is a failure mode, never the default fallback** (owner standing rule): exhaust automatic, policy-clean sources first (official APIs, robots-allowed endpoints); a feature whose answer is "the user imports a file" is not done.
- Treat paid data products as future options, not v1 assumptions.
- Use RSS/Atom where it is the source's intended distribution format, but do not let secondary RSS replace official issuer-report attribution when the official source is reachable.

## V1 Source Priority

V1 source priority:

1. Bankier Company Komunikaty as the active v1 GPW official-report source.
2. GPW ESPI/EBI official reports as a disabled fallback candidate until the source-specific fetch path is made reliable enough to beat Bankier coverage.
3. Selected Polish public/RSS media and article/news sources after source policy review.
3. Authenticated private research sources explicitly approved by source-specific ADR, starting with Portal Analiz if feasible.
4. Later official/public adapters for SEC EDGAR, Nasdaq RSS, and major European exchange disclosures.

**Agent acquisition path** ([ADR 0093](adr/0093-agent-acquisition-tier-and-preliminary-lifecycle.md), epic #285): the owner's own agent (BYOA, over the MCP port) is a first-class acquisition source for publications no automatic tier reaches yet — e.g. preliminary-results PDFs published hours before any structured filing exists. Trust-ranked below every issuer tier and above the aggregator (`source_tier='agent'`): the issuer's later filing upgrades agent-written slots. Every agent write registers the document it read (security-gated `capture_report_document`) and cites it per fact — an uncited agent figure is rejected at the port boundary, never stored.

## GPW ESPI/EBI Adapter

> **Live as witness (`v0.55.0`, [ADR 0069](adr/0069-source-reliability-and-disclosure-signals.md) D2 as amended 2026-07-15):** this adapter — disabled since migration 0011 in favor of the Bankier per-company path — runs as a **reconciliation second witness** (`role: witness`, visibility `optional`, re-enabled by migration 0081): listings are matched to Bankier-sourced reports by exact ESPI report number first, then (company, disclosure date) fallback; pair results (`matched | bankier_only | espi_only`) persist in `source_reconciliation_results`. It **never ingests feed items** (no dual ingestion — guarded by test). An `espi_only` result on a tracked company raises a system attention event (Today stream + sidebar badge + morning briefing, [ADR 0097](adr/0097-toasts-are-action-feedback-only.md); its Review opens the missed report via the stored witness URL); the full ledger lives in Diagnostics ("Uzgadnianie źródeł"). Bankier stays the primary ingestion path; promotion to co-primary is a later, evidence-gated decision. This closes the Bankier single-point-of-failure and the long-open reconciliation question — and yields the first measured completeness data (first live run: 15 witness items, 5 matched, 0 espi_only).

Adapter ID: `gpw-espi-ebi`

Source URL: `https://www.gpw.pl/komunikaty`

Observed listing endpoint: `https://www.gpw.pl/ajaxindex.php`

Fetch mode: `public_page`

Default poll interval: disabled for now.

Current status: registered but disabled. The global GPW listing slice missed tracked-company reports that Bankier per-company komunikaty pages exposed, so v1 uses Bankier Company Komunikaty as the active official-report feed. GPW should only be re-enabled after the adapter uses a reliable tracked-company-scoped fetch path or otherwise proves it can provide equal or better coverage.

The public GPW ESPI/EBI report page currently exposes report listings with:

- publication timestamp
- report category, for example current/periodic/quarterly/annual
- ESPI or EBI system label
- report number
- company name
- ISIN
- title
- detail link

The adapter should normalize each listing into a `feed_item`.

M6 source investigation found that GPW's public page uses an internal AJAX listing endpoint behind the "Pokaż więcej" behavior. The endpoint is not a separately documented public API, but it returns a much cleaner HTML listing fragment than the full page when posted with the page's own form parameters, including `action=GPWEspiReportUnion`, `start=ajaxSearch`, `page=komunikaty`, `format=html`, `lang=PL`, `offset`, `limit`, `categoryRaports[]=EBI`, `categoryRaports[]=ESPI`, and report type filters. The live adapter uses this endpoint for listings while preserving `https://www.gpw.pl/komunikaty` as the human source page and attribution URL.

Milestone 5 starts with test-sample-backed listing parsing and SQLite ingestion before wiring an explicit manual live fetch. Automated tests must stay test-sample-backed or use injected fetchers so default checks do not depend on GPW availability.

Required normalized fields:

- `type`
- `source_adapter_id`
- `source_name`
- `source_url`
- `title`
- `language`
- `published_at`
- `fetched_at`
- `dedupe_key`
- `attribution`
- matched company IDs when possible

Recommended dedupe key:

```text
gpw-espi-ebi:{system}:{isin}:{report_number}:{published_at}
```

If any component is missing, use the detail URL plus title and timestamp checksum as fallback.

## Company Matching

Primary company match:

- exchange-qualified ticker, for example `GPW:CLC`.

Fallback match:

- ISIN from the report listing to `companies.isin`.

Rules:

- Prefer ticker-first matching because that is how the user manages and thinks about GPW watchlists.
- Do not use issuer/company name alone as an automatic GPW feed match key. Names are display/search metadata and can be used for suggestions or diagnostics, but they are too fuzzy for silent Inbox matching.
- If the GPW listing source only provides ISIN, resolve ISIN to ticker through the GPW company directory cache first, then match the user's stored company by ticker.
- If no directory ticker is available, fall back to an exact ISIN match.
- Unmatched items can still be stored, but they should not appear in company-specific views until matched.
- Unmatched counts and recent unmatched item diagnostics belong in Developer mode unless they become a clear normal-user action.
- Matched listing ingestion writes `feed_item_companies.match_type = "ticker"` or `"isin"`.
- Re-ingesting the same listing updates source metadata but preserves read/saved user state.

## GPW Company Directory

Manual company management should not be the long-term default. The app has a GPW company directory cache with ticker, ISIN, display name, and source metadata for all currently listed companies exposed by GPW's public company list. It is used for lookup, autocomplete groundwork, and ticker-first source matching.

Directory requirements:

- Fetch the directory from GPW's public company list.
- Store the directory cache so lookup, autocomplete, and source matching do not require live network access.
- Refresh the directory on a slow cadence, initially daily or weekly, not on every app start.
- Preserve manual companies and user edits separately from the remote directory cache.
- Use the directory to resolve source identifiers into ticker-first matches.
- Show directory freshness and last error in Sources or Settings.

The implementation stores directory rows and exposes a manual directory refresh command from Sources. Runtime refresh fetches `https://www.gpw.pl/spolki?offset=0&limit=500`, which currently covers the full public list shown by GPW. Directory refresh commands run through the same async blocking-task boundary as feed and event source refreshes so live network work does not block the app UI. The desktop UI schedules a slow in-app directory refresh check using the directory source interval, currently one day; the scheduler does not run immediately on startup and only fetches when the cached directory is stale. Company-form lookup may auto-bootstrap required company directories on a miss when the runtime cache is empty, then retry the lookup. Parser tests remain test-sample-backed so default checks do not depend on live GPW availability, but test samples must not seed target runtime databases.

The Sources screen should not show unmatched-feed diagnostics for company directory sources because directories are not feed sources. Each directory source detail panel should expose only that source's collapsed searchable company list with tracked/untracked state and an add action for untracked companies.

Accepted matching priority:

1. Match tracked companies by ticker.
2. If a source item only exposes ISIN, resolve ISIN to ticker through `company_registry_entries`, then match by ticker.
3. If registry resolution is unavailable, fall back to exact ISIN.
4. Do not silently match by company name.

## Company Directory Sources

Company directory sources are source-backed company identity catalogs used for company lookup, autocomplete, and source matching. They are not feed/news sources and should be presented to normal users as company directory or lookup support, not as generic ingestion plumbing.

M22 confirmed that the current company registry model is extensible enough for a second directory source without a schema refactor.

Implemented company directory sources:

- GPW main-market companies from `https://www.gpw.pl/spolki?offset=0&limit=500`, using `GPW:<ticker>` company identity.
- NewConnect listed companies from `https://newconnect.pl/spolki?offset=0&limit=500`, using `NC:<ticker>` company identity. The high limit is required because the base page renders only the first 10 rows.

Expected future direction:

- GPW main-market companies and NewConnect companies use the same company-directory boundary.
- Later market directories, including US and European company lists, should be addable without rewriting company identity, matching, or source status flows.
- The data model should support multiple directory sources, source-specific identifiers, active/inactive listing state, refresh state, and source attribution.
- Source matching should continue to prefer exchange-qualified ticker identity and exact authoritative identifiers such as ISIN when available.
- Normal UI should expose directory freshness and lookup support in product language. Developer mode may expose source IDs, candidate status, and diagnostic detail.

M22 directory decisions:

- `company_registry_entries` supports multiple directory sources through `source_adapter_id`, `exchange`, `ticker`, `qualified_ticker`, active state, and source URL.
- GPW main market and NewConnect use separate source adapters behind the same company-directory interface.
- Company lookup/autocomplete searches all active company-directory entries. The user-entered exchange only prefers a matching exchange when duplicate tickers exist; it is not a hard filter that can hide companies from another supported registry.
- Directory bootstrap and stale checks apply to all enabled `company_registry` adapters, not a hard-coded list of exchanges.
- `company_source_ids` does not need a schema change for NewConnect because the directory cache is keyed by exchange-qualified ticker and ISIN.
- The source adapter read model distinguishes company-directory sources through `source_type = company_registry`.
- NewConnect required one new adapter registration migration and one source adapter implementation over the existing tables.

When adding the next company-directory source:

- Register a separate `source_adapters` row with `source_type = company_registry` and the supported market in `source_adapter_markets`.
- Store refreshed directory rows in `company_registry_entries` with the source adapter ID, exchange, ticker, qualified ticker, display name, authoritative identifier when available, source URL, fetched timestamp, and active state.
- Keep directory rows separated by source adapter in Sources while keeping lookup/autocomplete shared across all active company-directory entries.
- Do not hard-code the new exchange in lookup, company creation, ticker rendering, import/export, watchlist membership, or normal UI filters.
- Wire the new directory into the company-directory refresh boundary; an enabled directory source without a refresh implementation should fail tests because stale/bootstrap checks will keep seeing it as incomplete.
- If duplicate tickers can exist across exchanges, keep user-selected exchange as a preference only; exact authoritative identifiers should still be able to resolve the intended row.
- Add or update parser/refresh tests for the new directory, storage lookup/create tests for its exchange, Companies UI lookup/suggestion/add tests, and Sources UI tests if the new source is visible.
- Re-run source matching checks if any feed, report, event, or media adapter should use the new directory for ticker/ISIN matching.

## Detail Fetching

V1 should start with listing-level ingestion. Detail-page fetching is allowed only when:

- the detail URL is public,
- fetching does not require authentication,
- source attribution is preserved,
- adapter tests use test samples,
- rate limiting is conservative.

Detail fetch should capture:

- original report body or excerpt when legally and technically acceptable
- attachment links if visible and relevant
- source URL

Milestone 6 starts with parser test samples and policy checks, but the product requirement is broader than the spike: in-app access to official report body text is required for v1 GPW support. GPW detail-page fetching is the primary implementation path unless it proves technically or policy-wise unacceptable.

Observed M6 sample pages render the ESPI/EBI detail content inside a noisy full GPW page. The useful report area is exposed under `.report-data`, with a content-specific `h1`, a listing timestamp row, and report sections separated by labels such as `Treść raportu:` and `Nazwa arkusza:`. Parser test samples should model this structure instead of a clean article-only page. Test samples should cover both attachment and no-attachment reports and verify that English/entity/signature sections are not mixed into the main body text.

The detail fetch boundary must stay injectable, just like listing fetches, so automated checks can exercise fetch-and-parse behavior with local test samples. Normal ingestion should fetch detail bodies for matched GPW items under the conservative policy defined below.

M6 detail evaluation should produce an explicit usability signal before any promotion decision. A detail page is considered usable for ingestion only when the parser extracts a non-empty content title and meaningful report body text. Missing title or missing/very short body text should produce warnings and keep the detail page out of normal ingestion.

The M6 spike report should aggregate test-sample evaluations. Any rejected sample means the parser cannot be called broadly stable yet, but it does not make report body optional. Rejected samples trigger parser hardening or fallback-source investigation.

Decision: [ADR 0013: GPW Detail Fetching Policy](adr/0013-gpw-detail-fetching-policy.md) makes in-app official report body access required for v1 GPW support and accepts GPW detail fetching as the primary implementation path under strict constraints.

M6 detail fetch policy:

- detail fetching is enabled by default for matched GPW feed items,
- do not fetch details for unmatched diagnostics by default,
- fetch only a small bounded number of detail pages per refresh; initial cap is 5,
- serialize detail requests and wait at least 2 seconds between detail requests,
- never backfill old detail pages automatically without a separate user action or roadmap decision,
- preserve the original GPW detail URL as the source of truth even when body text is extracted.

Current M6 implementation stores official GPW detail body text and parsed attachment links on matched feed items when the parser evaluation accepts the detail page. Listing-level ingestion remains the fallback for individual detail failures. Sources diagnostics retain the last detail warning so the user can see whether detail-body ingestion failed because of a fetch error or because the parser rejected the page as unusable.

If GPW detail pages are unstable, the adapter should still store listing-level feed items and link to the original source for that item, but the project must evaluate fallback body sources rather than dropping the in-app report body requirement. Fallback candidates include PAP Biznes public ESPI/EBI pages, issuer investor-relations pages, and commercial/third-party APIs only after explicit approval.

PAP-specific research note: a community Python project, `wegar-2/pyespiebipapapi`, claims to scrape ESPI/EBI data from `https://espiebi.pap.pl/` and parse nodes by date or node ID. This is useful for reverse-engineering PAP structure, but it is not an official API, has no releases, and must not be added as an application dependency without a separate source-policy review. If used during research, use it only as a reference for test samples and endpoint discovery.

## Secondary ESPI/EBI RSS Policy

Bankier and Parkiet expose convenient ESPI/EBI RSS feeds:

- Bankier ESPI RSS: `https://www.bankier.pl/rss/espi.xml`
- Parkiet ESPI/EBI RSS: `https://www.parkiet.com/rss/7111-komunikaty`

These feeds are easier and lighter to parse than GPW detail pages. They are useful as secondary signals, cross-checks, diagnostics, and possible backfill hints. They must not replace GPW as the canonical v1 source for official report bodies while the official GPW path remains technically and policy-wise acceptable.

Secondary RSS usage rules:

- Store Bankier/Parkiet RSS items under their own adapter IDs and attribution, not as GPW-origin items.
- Do not treat secondary RSS descriptions as official report body text unless an item is explicitly linked back to and reconciled with an official source.
- Prefer matching secondary RSS items to GPW items by company, title similarity, publication timestamp, ESPI/EBI label, and later any discovered stable identifiers.
- Use secondary RSS to flag possible missed GPW items, detect duplicates, and provide fallback user visibility if official detail parsing fails for an individual item.
- If a secondary item is promoted into the normal Inbox, the UI must make the source clear and preserve the secondary source URL.

Observed source clues:

- Bankier marks ESPI articles as `Komunikaty spółek (ESPI)` and its company/report pages include a `Dane dostarcza: Notoria.pl` notice, suggesting at least some structured data is provider-backed.
- Parkiet marks ESPI/EBI report pages with `Źródło: ESPI` and an `espi` author/source label.
- These clues do not prove the exact upstream feed used by either site, so the app should not assume Bankier or Parkiet are official origin sources.

## M6 Source Investigation Results

Investigation chronicle moved to [Kanban Archive](kanban-archive.md#archived-investigation-and-study-notes-moved-2026-07-02); superseded by the M8 decision below. Current, live rule: **Bankier Company Komunikaty is the active v1 official-report source** (see "GPW ESPI/EBI Adapter" and "Media/RSS Sources").

## Rate Limits And Scheduling

Default v1 behavior:

- poll every 15 minutes while the app is open
- support manual refresh
- serialize GPW adapter requests
- use conditional requests if the source supports them
- back off scheduled refreshes after repeated errors while preserving manual refresh
- implement live fetch through an injectable fetch boundary so tests can remain offline

The adapter should record:

- last successful fetch
- last error
- last warning
- last cursor/checkpoint if available

## Source Status UI Requirements

The Sources screen should show for each adapter:

- enabled/disabled state
- simple health status
- last successful fetch
- last error/warning
- next scheduled poll
- manual refresh action
- source page link
- optional enable/disable control for implemented optional sources

Normal Sources must not show developer-only candidates, source IDs, fetch modes, rate-limit policy notes, source-policy notes, or unmatched diagnostics. Developer mode and docs may expose those details.

## Media/RSS Sources

Media, RSS, article, and analysis sources are in scope for v1 after each source is reviewed. These sources are separate from official-report ingestion: they can enrich the Inbox with coverage and analysis, but their source type and attribution must make clear that they are not official issuer reports unless the source itself provides official filings.

Initial candidate sources:

- Bankier RSS:
  - `https://www.bankier.pl/rss/gielda.xml` for market/company news.
  - `https://www.bankier.pl/rss/wiadomosci.xml` for broader financial news.
  - `https://www.bankier.pl/rss/espi.xml` remains classified as secondary ESPI/EBI, not media analysis.
- Investing.com Poland RSS directory — **reviewed and rejected for ingestion (2026-07-29, issue #73)**: the feeds are reachable and well-formed (`https://pl.investing.com/rss/news_25.rss` etc.), but the site terms explicitly prohibit storing/reproducing site data without Fusion Media's written consent (fatal for an ingest-and-persist adapter in a public open-core repo), items carry no `<guid>`/`<description>` (headline+link only; article pages are Cloudflare-403 to bots), and GPW companies appear only incidentally inside global categories (1/15 sampled items; the Company News category had zero GPW hits) — strictly weaker signal than the already-live Bankier channels. Revisit only with written permission from Fusion Media.
- Stooq ticker news and company pages.
- XTB market news/analysis pages, including `https://www.xtb.com/pl/analizy-rynkowe/wiadomosci-rynkowe`.
- Bankier article/news pages as a possible secondary source or cross-check.
- BiznesRadar company pages as a later candidate for ratios, fundamentals, consensus, and company context if scraping is acceptable.
- StockWatch.pl analysis pages as a high-value but likely scraping/paywall-sensitive candidate. Treat as later research unless a public feed or acceptable usage path is found.
- ISBnews-style market-news providers, subject to access, paywall, licensing, and attribution review.
- Other Polish market-analysis pages proposed during source review.

Each media/RSS source needs:

- source URL
- usage policy note
- attribution label
- rate limit
- supported language
- matching strategy
- dedupe strategy
- whether it is public, RSS-like, paywalled, authenticated, or manually configured

Do not add broad website scraping as a generic capability in v1.

M8 public-source ranking:

1. Bankier Giełda RSS. Accepted and implemented first because it is public, RSS-native, market-focused, attributable, and low-risk to poll without crawling article pages.
2. Bankier per-company komunikaty JSON and article pages. Accepted and implemented as the active v1 official-report visibility source because the public company pages expose stable Bankier instrument slugs/tag IDs, the listing endpoint provides ESPI/EBI rows for tracked companies, and article pages expose report body text and attachments.
3. Bankier Firma RSS. Keep as a reviewed follow-up candidate. It is public and RSS-native, but the current feed is broader business news with weaker listed-company signal than Giełda; do not enable by default until matching quality is proven against tracked GPW companies.
4. Bankier Wiadomości RSS. Keep as a low-priority follow-up candidate. It is public and RSS-native, but it mixes broad national/world/personal-finance coverage, includes stale backfill in the live feed, and would likely create noisy unmatched diagnostics.
5. Bankier ESPI RSS and Parkiet ESPI/EBI RSS. Keep as secondary official-report cross-check candidates only. They must not replace canonical GPW ingestion and need reconciliation rules before runtime ingestion.
6. Stooq ticker news, XTB analysis, BiznesRadar, StockWatch, and ISBnews-style providers. Keep behind source-specific review. Do not implement scraping, paywalled access, authenticated access, or commercial-provider assumptions without a later ADR or explicit source-policy decision. (Investing.com Poland RSS: reviewed and rejected 2026-07-29 — see the candidate list above. Portal Analiz: blocked on explicit publisher permission — see Authenticated Private Sources.)

Current implementation posture:

- The Bankier RSS parser/fetcher is channel-aware so accepted RSS channels can reuse the same parsing, entity decoding, timestamp, and dedupe behavior.
- Runtime ingestion currently enables `bankier-market-rss` and `bankier-company-komunikaty`.
- Additional Bankier RSS channels require their own adapter IDs, source status rows, matching-quality tests, and a deliberate runtime enablement decision. `bankier-firma-rss` and `bankier-wiadomosci-rss` are present only as disabled reviewed placeholders.
- Bankier company-komunikaty requests use `LocalInvestorNewsfeed/{version}`, are serialized, are scoped to tracked GPW companies, cache Bankier tag IDs in `company_source_ids`, fetch one listing JSON page per company, and fetch linked article pages only when local report body text is missing.
- `portal-analiz` is visible only as a disabled late-v1 authenticated research placeholder. It has no fetch path, credential flow, or scheduler behavior in M8.

Accepted first M8 media slice:

- Adapter ID: `bankier-market-rss`.
- Display name: `Bankier Giełda RSS`.
- Source URL: `https://www.bankier.pl/rss/gielda.xml`.
- Human RSS directory: `https://www.bankier.pl/rss`.
- Source type: `public_media`.
- Fetch mode/access mode: `rss`.
- Attribution: `Bankier.pl`.
- Language: Polish.
- Rate policy: manual refresh plus the normal in-app source scheduler; fetch only the RSS feed and do not crawl linked article pages in this slice.
- Matching policy: match only tracked GPW companies by strong ticker/name signals found in the RSS item title or description; keep unmatched items available only for source diagnostics.
- Dedupe policy: use RSS `guid` when present, otherwise a normalized item link, otherwise title plus publication timestamp. Link normalization strips RSS tracking parameters such as `utm_*` and fragments before storage/dedupe. For cross-source media dedupe, store a nullable duplicate signature based on matched tracked companies plus normalized title so obvious syndicated/copied media items do not create duplicate Inbox rows.
- Bankier ESPI RSS remains a separate secondary official-report/cross-check candidate and is not part of this media adapter.

Accepted Bankier company-komunikaty slice:

- Adapter ID: `bankier-company-komunikaty`.
- Display name: `Bankier Company Komunikaty`.
- Company page URL pattern: `https://www.bankier.pl/gielda/notowania/akcje/{TICKER}/komunikaty`.
- JSON listing endpoint pattern: `https://api.bankier.pl/articles/listing/{page}/{limit}` with Bankier `tags_ids` and `pub_id=3,379`.
- Source type: `official_report`.
- Fetch mode/access mode: `public_json`.
- Attribution: `Bankier.pl`.
- Language: Polish.
- Scope: tracked GPW companies only.
- Identifier policy: resolve Bankier canonical instrument slug and tag ID from the company page when missing, then cache them in `company_source_ids`.
- Rate policy: manual refresh plus the normal in-app source scheduler; serialize company requests, wait between companies, fetch one listing page per company, and fetch article detail pages only for items whose body text is not already stored locally.
- Dedupe policy: use Bankier article ID for adapter-local dedupe and title/company comparison to avoid creating a second visible item if `gpw-espi-ebi` is later re-enabled.
- Source-policy note: Bankier Company Komunikaty is the active v1 official-report source while `gpw-espi-ebi` remains registered but disabled until a later reliability pass proves it should be re-enabled.

## ESPI/EBI Filing Samples In The Public Repo

A narrow, deliberate carve-out ([ADR 0094](adr/0094-committed-public-espi-report-samples.md), amending [ADR 0091](adr/0091-failure-path-and-real-state-testing.md) decision 4): complete, selected **official ESPI/EBI periodic filing files** (mandated-public disclosures) may be committed as test samples under `src-tauri/samples/reports/` — ≤ 5 MB total, machine-checked `MANIFEST.json` attribution (issuer, period, truthful distributor URL, sha256). Aggregator/API content stays non-redistributable (ADR 0072/0082/0085/0086 unaffected); nothing from the maintainer's research database ever enters the repo.

## ESPI/EBI Filing Classification

Typed ESPI/EBI event classification (`v0.40.0`, [ADR 0034](adr/0034-espi-event-classification.md)) runs over the **active official-report feed**, currently `bankier-company-komunikaty`, which exposes the ESPI/EBI category label, report title, and body text needed to classify. The classifier is source-neutral: it reads whichever official-report adapter is enabled, so a future `gpw-espi-ebi` re-enable feeds the same classifier without changes. Classification produces typed `company_signals` (insider transaction, dividend, profit warning, significant contract, buyback, guidance change, other) rather than altering the source adapter contract. The deferred ESPI/EBI **attachment ingestion** and **on-track backfill** work (milestone `v0.41.0`) likewise targets the active Bankier article/attachment path, not the disabled GPW detail flow.

### Report Document Ingestion And On-Track Backfill (`v0.41.0`)

Report-file persistence and history backfill ([ADR 0036](adr/0036-report-document-storage-and-backfill.md)) target the active Bankier company-komunikaty article/attachment path:

- **Attachment ingestion.** Attachment links surfaced on the Bankier komunikaty article page are upserted into `report_documents` (identity `(company_id, url)`). The full file is downloaded for periodic/financial reports **and, independently, for any structured ESEF/iXBRL attachment (`.xhtml`)** regardless of the filing's classification ([ADR 0061](adr/0061-deterministic-fundamentals-data-gathering.md) decision 1b — the periodic-report text classifier can miss an xhtml-only ESEF filing). A digital-signature attachment (`.xades`) always persists as metadata + URL only (no financial data). Every other ESPI/EBI attachment persists as metadata + URL only (`fetch_status = metadata_only`). Attribution stays `Bankier.pl`. Tag-listing ingestion stamps the tag's company onto every attachment, so a **mis-association guard** (card 45fcece) rejects a periodic-report attachment whose filename names a *different* tracked issuer (details + the matching startup repair: [data-model.md](data-model.md) `report_documents`).
- **Cover-note fundamentals (zero-fetch).** The komunikat **body text** the adapter already stores is itself a data source: a periodic-report body carries the mandated ESPI "WYBRANE DANE FINANSOWE" table, which is parsed into validated `financial_facts` at **ingest time** as tier 2a of the deterministic fundamentals pipeline ([ADR 0061](adr/0061-deterministic-fundamentals-data-gathering.md) decision 1). No extra request, no new host, no attachment fetch — the Bankier company path now yields headline fundamentals on publication day, including for companies whose filings are PDF-only. Extraction is ingest-time by necessity (feed retention can delete the carrier body) and best-effort by design (a parse problem never fails or rolls back feed ingestion). Only periodic-report komunikaty are parsed, via the same `is_periodic_report_item` classifier that gates attachment fetching. Persistence/precedence/observability contract: [data-model.md](data-model.md) `financial_fact_provenance` → `espi_cover_note`.
- **On-track backfill.** An explicit, user-triggered action paginates the Bankier komunikaty JSON listing (`/articles/listing/{page}/{limit}`) **backward ~3 years** for one tracked company, ingesting periodic reports + ESPI/EBI filings through the normal ingestion path with original publication dates preserved. It obeys the existing Bankier rate policy (serialized, waits between pages, `LocalInvestorNewsfeed/{version}` agent), runs as a cancellable async job with progress/diagnostics, and is idempotent via the existing dedup keys. Eligibility is **data-driven from `source_adapter_markets`** for the history-capable `bankier-company-komunikaty` adapter (currently `GPW`), not a hardcoded exchange: a company on an ineligible market (e.g. NewConnect — the Bankier company path has no history support for NC/EBI issuers) fails fast with the machine-readable `unsupported_market` error before any fetch (card bfc4c98). Every failure carries a **typed reason code** so the distinct causes never collapse into one message — `unsupported_market` / `not_tracked` / `no_bankier_page` / `http_error` / `parse_error` / `internal` (full list in [contracts.md](contracts.md) `get_backfill_progress`) — and leaves a `module=backfill … code=<code>` `log::warn!` trail; a genuine adapter-interaction fault additionally records a typed source outcome on the adapter, while a pre-fetch eligibility failure does not (it is not an adapter fault). A page fetched with zero komunikaty completes honestly (`itemsIngested: 0`), never a masked failure. **Historical calendar entries are not backfilled** — the forward-looking `official_calendar`/`public_calendar` adapters own upcoming events. Backfill never runs automatically and never fetches while the app is closed (that is the `v0.49.0` autopilot frontier).
- **Source-neutral.** Both paths read whichever official-report adapter is active, so a future `gpw-espi-ebi` re-enable reuses them without changes.

## Price And Fundamentals Context Sources

Price/fundamentals enrichment is useful for later context around reports and news, but it is not the same as official-report ingestion.

**Decided price source (2026-07-03 [ADR 0067](adr/0067-market-data-foundation.md); source selection amended 2026-07-14 [ADR 0082](adr/0082-market-data-source-selection.md), milestone `v0.53.0`):**

- **EOD quotes** are a `market_data`-type adapter: full-history backfill on company add (throttled) + one post-session daily pull per company; EOD-only (decision support, not a trading feed), durable attribution, source-neutral boundary.
  - **Primary `yahoo-eod`** — Yahoo Finance v8 chart API (`.../v8/finance/chart/<ticker>.WA`), keyless, PLN, full history to each ticker's GPW debut. Accepted **ToS-gray** for local-first personal EOD/watchlist-only use, no redistribution (ADR 0082).
  - **No secondary provider today.** Twelve Data was selected as the official free fallback and then **removed 2026-07-14**: the live smoke proved GPW `time_series` is paid-plan-only (free tier covers metadata, not quotes) — ADR 0082 amendment. A Yahoo failure raises source-health and skips the day; the self-heal backfill catches history up on recovery. Free degraded fallback (Bankier/StockWatch current close, GPW-archive real-browser probe): card `ee81afe`, v0.55.
  - **Witnesses (compare-only):** BiznesRadar `/notowania`, Bankier, StockWatch — robots-allowed current-quote pages cross-check the close; divergence → source-health, never a silent overwrite.
  - **Rejected for automation** (probe evidence in ADR 0082): Stooq (proof-of-work gate), BiznesRadar `/notowania-historyczne` + `/profile-history` (robots-disallowed), Google Finance (dead history + consent wall), investing.com/Bloomberg/MarketWatch/FT (locked/paid/anti-bot), CNBC (no GPW).
  - **PAP rejected as an ESPI source** (owner-requested probe 2026-07-18, card `bdda6cf`): `espiebi.pap.pl` is `robots.txt: Disallow /`; `www.pap.pl`/`biznes.pap.pl` pages sit behind an Incapsula anti-bot WAF (Stooq-class); the only open endpoint (`biznes.pap.pl/rss`) is a content-poor publication calendar; the PAP Biznes API is commercial. Decisive: ESPI content is PAP-distributed by construction, and the same stream is already ingested via Bankier (primary) + GPW `/komunikaty` (witness) — PAP adds no content, only cost/risk. Third-party bots crediting "PAP" reflect the content's origin, not a policy-clean fetch path. Follow-up instead: the ESPI periodic-report form's mandatory "Wybrane dane finansowe" table as a deterministic tier-0 facts source (spike `bdda6cf`, v0.58).
- This resolves the former "potential price source" status and the roadmap's open Stooq question. BiznesRadar's other robots-allowed data is accepted as a source for later milestones (v0.55 fundamentals witness — HTML, not the Premium CSV; v0.58 recommendations; v0.56 ownership; v0.57/v0.66 ratios/dividends), each behind its own policy note. The fundamentals-aggregator role for Stooq remains declined per [ADR 0061](adr/0061-deterministic-fundamentals-data-gathering.md).

**Fundamentals witness:**

- **BiznesRadar financial-report pages** — **Live (`v0.59.0`)**, adapter `biznesradar-fundamenty`, `fundamentals` type, visibility `optional`, **`primary` role for core KPIs** (*planned — [ADR 0098](adr/0098-mcp-native-kpi-acquisition-lifecycle.md): demoted to witness/complement per slot on completion of the agent-first run workflow*) ([ADR 0086](adr/0086-aggregator-primary-fundamentals.md) decision 2, promoted from the [ADR 0085](adr/0085-biznesradar-fundamentals-witness.md) witness; pattern: ownership, ADR 0072). **Three** robots-allowed pages per tracked GPW company — `/raporty-finansowe-rachunek-zyskow-i-strat/<ticker>`, `/raporty-finansowe-bilans/<ticker>`, `/raporty-finansowe-przeplywy-pieniezne/<ticker>` — parsed by the source-generic aggregator parser (`fundamentals/extraction/html.rs`); the GPW ticker resolves to the canonical slug via the same 301 redirect the ownership/recommendations adapters rely on, so no slug table is maintained. **Pull model:** the daily `aggregator_fundamentals_pull` durable-queue job (`sources` lane, armed by the app-open scheduler tick, and on demand via `run_aggregator_fundamentals_pull` / the rebuild flow) reads **every period column** each cached page carries (period derived from the column header, incl. shifted fiscal-year ends; unparseable headers are skipped, never guessed) and writes `source_tier='html_aggregator'` facts citing the page URL + row label. **Precedence and reversed witnessing** (ADR 0086 decisions 3–4): `manual` and issuer tiers (ESEF / WDF — the positional tier that once also outranked the aggregator here is retired, [ADR 0095](adr/0095-retire-html-positional-tier.md)) always outrank the aggregator; the aggregator only ever overwrites its own slot; a divergence against an issuer-held slot records an informational `witness_disagreement` outcome (the issuer value stays); an empty/zero aggregator cell is **never** written or used as evidence (the zero-guard, ADR 0085 amendment). The ingest-time WDF witness comparison (cache-only, never a synchronous fetch in the feed path) and its `witness_pending` deferral survive unchanged from ADR 0085. **Politeness:** per-(company, page kind) daily cadence cached in `fundamentals_aggregator_pages` (migration 0110; failures and no-coverage landings cached too), sequential per host, shared BiznesRadar posture, at most one fetch per page per company per day; deliberately absent from `runtime_adapters()` — a manual "Odśwież źródła" sweep never fetches it, and the pull records only its **outcome** (`last_success_at`/`last_error`), never a refresh **attempt** (2026-07-21 live regression). Policy posture: `robots.txt` allows `/raporty-finansowe-*` (its four disallows cover exactly the historical-quote paths [ADR 0082](adr/0082-market-data-source-selection.md) rejected), read-only, attribution stays visible, no redistribution (sui-generis posture as ADR 0072). Drift guard: stored sample pages pin the parse (`samples/biznesradar_fundamenty_cdr.html`, `samples/biznesradar_bilans_cdr.html`, `samples/biznesradar_przeplywy_cdr.html`).

Potential fundamentals sources:

- Notoria is a likely structured-data provider behind some Polish finance portals, but should be treated as a commercial/contact-first option rather than scraped.
- BiznesRadar may be useful for public ratios and company context if its terms and markup are acceptable.
- Per-company investor-relations sites may be useful for redundancy, English materials, reports, presentations, and calendars, especially for larger issuers.

Price and fundamentals adapters should be separate from feed/news adapters and should not delay M6 GPW report-body work.

## Company Event Sources

M9 event ingestion starts with source-neutral local storage and then adds real sources in conservative order.

Accepted event sources:

- Adapter ID: `gpw-market-events-rss`.
- Display name: `GPW Market Events RSS`.
- Source URL: `https://www.gpw.pl/rss-calendar-of-market-events`.
- Source type: `official_calendar`.
- Fetch mode/access mode: `rss`.
- Attribution: `GPW`.
- Scope: tracked GPW companies only.
- Matching policy: exact ticker from the RSS item title/description. Issuer name alone is not enough.
- Initial event coverage: GPW market/corporate-action events such as corporate actions, listing changes, and market-making activity.
- Rate policy: manual refresh plus normal in-app source scheduler; the feed is low-volume and should be fetched at low frequency.
- Dedupe policy: stable source event key built from event date, event label, instrument type, and ticker.

- Adapter ID: `bankier-kalendarium-html`.
- Display name: `Bankier Kalendarium`.
- Source URL: `https://www.bankier.pl/gielda/kalendarium`.
- Source type: `public_calendar`.
- Fetch mode/access mode: `public_page`.
- Attribution: `Bankier.pl`.
- Scope: tracked GPW companies only.
- Matching policy: exact ticker from the calendar company symbol. Issuer name alone is not enough.
- Initial event coverage: broader company calendar events such as report dates, dividends, shareholder meetings, tender offers, and primary-market events.
- Rate policy: manual refresh plus normal in-app source scheduler; fetch the current public calendar page at low frequency and fetch dated week pages on demand from the Events week view.
- Dedupe policy: stable source event key built from ticker, category, and event description so a changed source date updates the existing event row instead of creating a correction row.
- Week navigation policy: Events view is cache-first. Changing weeks updates the local date filter immediately, then fetches the Bankier dated calendar URL for that week in the background and reloads local events when ingestion finishes.
- Investor week calendar ([ADR 0058](adr/0058-investor-week-calendar.md), `v0.67.0`): the mapping widens to emit `periodic_report`, `ipo_debut` (`DEBIUT`), and `ex_dividend` (`ODCIĘCIE DYWIDENDY`) in addition to today's `dividend`/`shareholder_meeting`. An **opt-in whole-market scope** relaxes the tracked-ticker filter for the current week page only, persisting untracked-ticker rows into `market_calendar_events` (ticker + issuer name, no canonical company); the week read model dedups them against tracked `company_events` by ticker. The relaxed fetch runs only when the user enables the market scope. **Macro releases (CPI/PMI/payrolls) ship model + manual entry + sample seed only this milestone; a policy-clean live macro source is deferred to a follow-up ADR** (aggregated economic calendars are paid or fragile/restricted scraping — rejected; the later candidate is official primary calendars GUS/NBP/US BLS/Fed or a curated dataset). Market holidays are a curated static dataset, not a live source.

Fallback candidate order:

1. Strefa Inwestorów report calendar for periodic-report dates. Registered as disabled adapter candidate `strefa-report-calendar`.
2. Money.pl report/calendar pages as cross-checks. Registered as disabled adapter candidate `money-calendar`.
3. Per-company investor-relations calendars for selected issuers when a company-specific source review accepts them.

Disabled candidates are visible only in Developer mode and docs. They must stay out of normal Sources and cannot become user-enableable until source-specific parser tests, attribution rules, and ticker/ISIN matching quality are accepted.

Rejected or unproven for now:

- Bankier hidden calendar-like RSS endpoints such as `rss/kalendarium.xml`, `rss/dywidendy.xml`, and `rss/raporty.xml` returned RSS content types but empty bodies during direct checks. They must not be treated as reliable until stable populated content is proven.

## Authenticated Private Sources

Authenticated private sources are in v1 scope only as explicitly named adapters with source-specific approval. Portal Analiz is the first desired adapter in this category because the user has a paid personal account and wants company research from that source inside the local Inbox.

Decision: [ADR 0014: Portal Analiz Authenticated Source Policy](adr/0014-portal-analiz-authenticated-source-policy.md) accepts Portal Analiz as a v1 authenticated private research source candidate only under a dedicated adapter with strict local credential, attribution, scope, and rate-limit boundaries. The ADR does not approve generic scraping infrastructure or bypassing access controls.

**Source research update (2026-07-29, issue #116): blocked on explicit publisher permission.** Live probing found `portalanaliz.pl/robots.txt` disallowing named AI/scraper agents site-wide (ClaudeBot, GPTBot, CCBot, …) **and** `Disallow: /feed/` for *every* user agent, with Cloudflare returning 403 to all non-browser fetches (homepage, terms, articles — only robots.txt was servable). No public RSS/API/sitemap exists; the analytical content sits behind the paid membership (~299 PLN/yr). Building even the conservative user-authenticated adapter ADR 0014 sketched would mean defeating active anti-bot defenses — the "hidden full-session browser automation" ADR 0014 lists as a non-goal. Verdict: the disabled-placeholder posture stays; the next step is the **owner asking Portal Analiz for explicit written permission** for a personal, low-volume, authenticated desktop client (no redistribution, no AI training). The license-gated-premium product angle is moot until that permission exists.

Portal Analiz requirements before implementation:

- explicit written permission from the publisher for personal authenticated client access (2026-07-29 finding, above),
- source research confirming an acceptable user-account usage path under ADR 0014,
- OS keychain storage for credentials or session secrets,
- no credential export through YAML, backup, logs, screenshots, or test samples,
- conservative rate limits and no background crawling outside user-approved scope,
- test-sample-backed tests without live credentials,
- clear source attribution and original source URL,
- adapter status that indicates authenticated/paywalled behavior,
- explicit discussion if the source proves technically or policy-wise too troublesome.

Authenticated sources must never become a generic "log in and scrape any website" subsystem in v1. Each one gets its own adapter, tests, rate policy, and ADR.

## KNF Short-Selling Registry

**Live (`v0.55.0`, [ADR 0069](adr/0069-source-reliability-and-disclosure-signals.md)).** Adapter ID: `knf-short-selling`, `disclosure`-type, visibility `optional`, daily polling. Data path: the register's stable public JSON endpoint (`POST /rss_pub/JSON`, current-register method) — no HTML scraping; holder names HTML-entity-decoded. Per-company net-short entries (holder, size %, dates) are mirrored in `short_positions` with an append-only `short_position_events` history (entered / increased / decreased / exited); each change for a tracked company (ISIN-matched) emits one feed item plus a `short_position_change` typed signal that participates in alert rules. An empty register response is rejected at the refresh seam (a transient fault must not read as a mass exit). Register threshold: positions ≥ 0.5%. Dashboard panel: cockpit `shortPositions` (palette). Standard attribution ("KNF — Rejestr krótkiej sprzedaży").

## Ownership Structure Sources

[ADR 0072](adr/0072-ownership-structure.md) (amended 2026-07-16) — three streams, no new scraping surface. Role split after the v0.56 real-data pivot: the **aggregator is the automatic breadth source** (covers every tracked GPW company); **reports + ESPI are depth/freshness** where they exist (provenance, votes-vs-capital gap, threshold events).

- **Stored periodic reports** (already ingested): extraction of the mandatory "shareholders ≥5% of votes" section via the deterministic parser — **final by design**, no AI fallback ([ADR 0084](adr/0084-retire-in-app-ai-layer.md)): a section the parser cannot read (glyph-mangled text, an image table, a missing section) is flagged as a residual and writes zero stakes rather than guessing. Depth stream: real-data evidence (9/50 companies — 35/50 have no fetched documents) disqualified it as the breadth source.
- **ESPI major-holdings notifications** — **Live (`v0.56.0`)**: threshold-crossing filings (art. 69) classified as the `major_holdings_change` typed signal (seeded rule category; formulaic Polish title patterns). A confirmed signal triggers a **deterministic** stake update — a conservative parse of the notification body extracts the resulting holder + capital % and votes % (kept separate) and writes an `espi_filing` snapshot with disclosure-date `as_of` and filing provenance. **Never guessed**: conflicting before/after percentages or an unresolvable holder yield no stake — the filing is marked unparsed and a diagnostic is recorded. Keeps stakes fresh between reports.
- **Aggregator ownership pages** — **Live (`v0.56.0`)**, adapter `biznesradar-akcjonariat`, `ownership` type, visibility `optional`, `primary` role, daily cadence, one polite page fetch per tracked GPW company. **Automatic breadth source** (ADR 0072 amendment 2026-07-16): each refresh parses the "Główni akcjonariusze" table of BiznesRadar's public `/akcjonariat/<ticker>` page (holder, capital %, votes %/WZA; the sub-5% "Pozostali akcjonariusze" fund-statement table is deliberately not ingested; summary/"razem" rows and implausible rows/bases are rejected — rules in data-model.md) and writes it as a full-picture `aggregator` snapshot — single `as_of` = newest per-row "Data aktualizacji" (fallback: fetch date), unchanged pages upsert the same basis in place, a same-basis re-ingest reconciles the row set (`aggregator` rows only; prior bases immutable). The refresh still runs the comparison, direction reversed: our disclosed reports/ESPI state witnesses the aggregator — divergences (a holder present on only one side above the 5% threshold, or a matched percentage differing by >1.0 pp) are recorded as diagnostics plus a per-company witness result. **BiznesRadar chosen over Bankier "Akcjonariat"** (probe 2026-07-16): BiznesRadar's `robots.txt` is fully permissive (`Allow: /`, no akcjonariat/gielda restriction) and its host is independent of the primary Bankier official-report channel; Bankier's robots is AI-bot-hostile. Both are server-rendered with separate capital/votes columns; the GPW ticker resolves to BiznesRadar's canonical slug via redirect. Legal posture: local personal reader in v1; database-right review gates any future open-core redistribution.

## Analyst Recommendation Sources

Scope ([ADR 0073](adr/0073-analyst-recommendations-tracking.md)): recommendation items (firm, rating change, target price, date) from policy-reviewed public paths only; no scraping beyond policy, no paid consensus feeds (an aggregate-consensus adapter stays deferred behind a flag + ADR). Presentation is always attributed third-party opinion — tracking, never advice.

- **BiznesRadar recommendations** — **Live (`v0.58.0`)**, adapter `biznesradar-rekomendacje`, `analyst_recommendation` type, visibility `optional`, `primary` role, daily cadence, one polite page fetch per tracked GPW company. Parses the public `/rekomendacje-spolki/<slug>` table (verbatim rating, target price + currency, price at issuance, publication datetime, analyst + issuing firm, broker PDF link; the derived kurs-aktualny/CD-K columns are not stored); the GPW ticker resolves to the canonical slug via the same 301 redirect the ownership adapter relies on (policy probe 2026-07-19: `robots.txt` `Allow: /` with disallows not covering this path; no anti-bot). **Depth honesty:** the free page carries only the ~3–5 most recent entries — the full history is BR Plus (paid) and stays out of policy scope; local history accumulates append-only from ingestion start, per the ADR 0073 premise. Rating vocabulary is preserved verbatim; the normalized direction (upgrade/downgrade/initiate/reiterate) is derived against the same firm's prior stored entry.
- **Bankier recommendation items / brokerage RSS** — deferred candidates; each needs its own policy review before an adapter exists.

## Future Official Sources

Later adapters should follow the same source adapter contract.

**Live-probe rule (guardrail, 2026-07-19):** an adapter implementation contract is written
against **freshly probed** source facts (exact URL shape, robots, redirect behavior, content
depth, table anatomy — probe date recorded in the policy note), never solely against recorded
research notes. Research claims age: the v0.58 recommendations source had drifted from its
v0.53 research on both the URL path (`/rekomendacje-spolki/`, not `/rekomendacje/`) and the
free-page depth (~3–5 newest entries, not "dozens" — the full history is paywalled).

Candidates:

- SEC EDGAR submissions and company facts APIs
- Nasdaq RSS feeds
- major European exchange disclosure pages or feeds

These are not v1 implementation requirements unless explicitly moved into Ready.

## Source Candidate Study

M22 candidate-by-candidate study and disposition moved to [Kanban Archive](kanban-archive.md#archived-investigation-and-study-notes-moved-2026-07-02). Current candidate status lives in the live sections above/below (e.g. "Media/RSS Sources", "Authenticated Private Sources"); a candidate must not be user-enableable until its own source-specific review/ADR is complete.

## Source Research Notes

Checked-reference notes from the M6–M8 research pass moved to [Kanban Archive](kanban-archive.md#archived-investigation-and-study-notes-moved-2026-07-02). Current accepted source URLs/endpoints are documented in the live adapter sections above.

## Open Source Questions

Resolved/superseded items moved to [Kanban Archive](kanban-archive.md#archived-investigation-and-study-notes-moved-2026-07-02). Genuinely open items are tracked in [Roadmap § Open Source-Strategy Questions](roadmap.md#open-source-strategy-questions).
