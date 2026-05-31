# Source Strategy

This document defines the first source strategy for Brawler. It focuses on GPW because v1 prioritizes Polish market coverage.

See also [Product Spec](product-spec.md), [Data Model](data-model.md), [Contracts](contracts.md), [Architecture](architecture.md), and [ADR 0004: Source and AI Policy](adr/0004-source-and-ai-policy.md).

## Strategy Principles

- Prefer official, public, or RSS-like sources.
- Preserve source attribution and source URLs.
- Store enough origin to audit every feed item.
- Keep adapters modular and source-specific.
- Avoid restricted or fragile scraping unless a source-specific ADR approves it.
- Treat paid data products as future options, not v1 assumptions.
- Use RSS/Atom where it is the source's intended distribution format, but do not let secondary RSS replace official issuer-report attribution when the official source is reachable.

## V1 Source Priority

V1 source priority:

1. GPW ESPI/EBI official reports.
2. Selected Polish public/RSS media and article/news sources after source policy review.
3. Authenticated private research sources explicitly approved by source-specific ADR, starting with Portal Analiz if feasible.
4. Later official/public adapters for SEC EDGAR, Nasdaq RSS, and major European exchange disclosures.

## GPW ESPI/EBI Adapter

Adapter ID: `gpw-espi-ebi`

Source URL: `https://www.gpw.pl/komunikaty`

Observed listing endpoint: `https://www.gpw.pl/ajaxindex.php`

Fetch mode: `public_page`

Default poll interval: 15 minutes.

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
- If the GPW listing source only provides ISIN, resolve ISIN to ticker through the local GPW company registry cache first, then match the user's stored company by ticker.
- If no registry ticker is available, fall back to an exact ISIN match.
- Unmatched items can still be stored, but they should not appear in company-specific views until matched.
- The Sources screen should expose unmatched counts and recent unmatched item diagnostics after implementation supports them.
- Matched listing ingestion writes `feed_item_companies.match_type = "ticker"` or `"isin"`.
- Re-ingesting the same listing updates source metadata but preserves read/saved user state.

## GPW Company Registry

Manual company management should not be the long-term default. The app has a local GPW company registry cache with ticker, ISIN, display name, and source metadata for all currently listed companies exposed by GPW's public company list. It is used for lookup, autocomplete groundwork, and ticker-first source matching.

Registry requirements:

- Fetch the registry from GPW's public company list.
- Store the registry in SQLite so lookup, autocomplete, and source matching do not require live network access.
- Refresh the registry on a slow cadence, initially daily or weekly, not on every app start.
- Preserve manual companies and user edits separately from the remote registry cache.
- Use the registry to resolve source identifiers into ticker-first matches.
- Show registry freshness and last error in Sources or Settings.

The implementation stores registry rows in SQLite and exposes a manual registry refresh command from Sources. Runtime refresh fetches `https://www.gpw.pl/spolki?offset=0&limit=500`, which currently covers the full public list shown by GPW. The desktop UI schedules a slow in-app registry refresh check using the registry adapter interval, currently one day; the scheduler does not run immediately on startup and only fetches when the cached registry is stale. Company-form lookup may auto-bootstrap the registry on a miss when the runtime cache is empty, then retry the lookup. Parser tests remain test-sample-backed so default checks do not depend on live GPW availability, but test samples must not seed target runtime databases.

The Sources screen should not show unmatched-feed diagnostics for the company registry adapter because the registry is not a feed source. Its detail panel should instead expose a collapsed searchable cached-companies list with tracked/untracked state and an add action for untracked companies.

Accepted matching priority:

1. Match tracked companies by ticker.
2. If a source item only exposes ISIN, resolve ISIN to ticker through `company_registry_entries`, then match by ticker.
3. If registry resolution is unavailable, fall back to exact ISIN.
4. Do not silently match by company name.

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

The current source ranking for GPW report bodies is:

1. GPW detail page body extraction remains the primary path. The listing page links to `komunikat?geru_id=...` detail pages, and sampled detail pages include the official PAP-rendered report content under `.report-data`. The markup is noisy, but the report body and sections are present without login.
2. PAP Biznes is the strongest official-adjacent fallback candidate because GPW detail pages embed PAP report markup and rewrite some report links to `https://espiebi.pap.pl/espi/pl/reports/view/...`. PAP also exposes public ESPI/EBI pages on `biznes.pap.pl`. PAP terms and direct URL patterns still need a separate review before implementation.
3. Bankier and Parkiet RSS are technically useful as secondary cross-check, diagnostics, and emergency fallback signals. They are not original GPW/PAP source paths, so they should not become the canonical official-report source without a later ADR.
4. Stooq is useful for general company and market news, but current investigation did not find a dedicated ESPI/EBI report-body path comparable to GPW or Bankier. Treat it as a later media/news candidate, not a primary report-body source.

Do not weaken the v1 requirement because a source is inconvenient. If GPW parsing becomes unreliable, the next step is to harden the parser or evaluate PAP/Bankier/issuer alternatives, then explicitly discuss the trade-off before changing the roadmap.

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
- fetch mode
- last successful fetch
- last error/warning
- next scheduled poll
- manual refresh action
- source policy note

For GPW ESPI/EBI, the source policy note should mention that v1 uses the public GPW report page and that paid processed GPW data products may be evaluated later.

## Media/RSS Sources

Media, RSS, article, and analysis sources are in scope for v1 after each source is reviewed. These sources are separate from official-report ingestion: they can enrich the Inbox with coverage and analysis, but their source type and attribution must make clear that they are not official issuer reports unless the source itself provides official filings.

Initial candidate sources:

- Bankier RSS:
  - `https://www.bankier.pl/rss/gielda.xml` for market/company news.
  - `https://www.bankier.pl/rss/wiadomosci.xml` for broader financial news.
  - `https://www.bankier.pl/rss/espi.xml` remains classified as secondary ESPI/EBI, not media analysis.
- Investing.com Poland RSS directory, especially company news, stock-market news, analyst ratings, earnings-related news, and analysis categories. Exact feed URLs need discovery from the RSS directory before implementation.
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

## Price And Fundamentals Context Sources

Price/fundamentals enrichment is useful for later context around reports and news, but it is not the same as official-report ingestion.

Potential price source:

- Stooq exposes simple CSV-style quote/history endpoints used by community tools, for example historical daily data under `https://stooq.pl/q/d/l/?s={ticker}.PL&i=d` and latest quote-style CSV under `https://stooq.pl/q/l/?s={ticker}.PL&f=sd2t2ohlcv&h&e=csv`.

Potential fundamentals sources:

- Notoria is a likely structured-data provider behind some Polish finance portals, but should be treated as a commercial/contact-first option rather than scraped.
- BiznesRadar may be useful for public ratios and company context if its terms and markup are acceptable.
- Per-company investor-relations sites may be useful for redundancy, English materials, reports, presentations, and calendars, especially for larger issuers.

Price and fundamentals adapters should be separate from feed/news adapters and should not delay M6 GPW report-body work.

## Authenticated Private Sources

Authenticated private sources are in v1 scope only as explicitly named adapters with source-specific approval. Portal Analiz is the first desired adapter in this category because the user has a paid personal account and wants company research from that source inside the local Inbox.

Portal Analiz requirements before implementation:

- dedicated ADR covering source policy, user-account usage, terms/usage risk, and whether scraping is acceptable for personal local use,
- OS keychain storage for credentials or session secrets,
- no credential export through YAML, backup, logs, screenshots, or test samples,
- conservative rate limits and no background crawling outside user-approved scope,
- test-sample-backed tests without live credentials,
- clear source attribution and original source URL,
- adapter status that indicates authenticated/paywalled behavior,
- explicit discussion if the source proves technically or policy-wise too troublesome.

Authenticated sources must never become a generic "log in and scrape any website" subsystem in v1. Each one gets its own adapter, tests, rate policy, and ADR.

## Future Official Sources

Later adapters should follow the same source adapter contract.

Candidates:

- SEC EDGAR submissions and company facts APIs
- Nasdaq RSS feeds
- major European exchange disclosure pages or feeds

These are not v1 implementation requirements unless explicitly moved into Ready.

## Source Research Notes

Current checked references:

- GPW ESPI/EBI report listing: `https://www.gpw.pl/komunikaty`
- GPW public page shows report listings under "Raporty Spółek ESPI/EBI" with timestamps, report type, ESPI/EBI label, company name/ISIN, report title, and detail links.
- GPW also describes processed information products available in CSV or XLS under its information services area; these are future options, not v1 assumptions.
- No documented public GPW ESPI/EBI API has been accepted for v1 yet. M6 found an internal GPW AJAX listing endpoint used by the public page; this is cleaner than full-page listing parsing, but still must be treated as a public-page implementation detail rather than a contracted API.
- PAP Biznes exposes ESPI/EBI-related public pages, including `https://biznes.pap.pl/espi` and `https://biznes.pap.pl/ebi/company`, and describes ESPI/EBI communications in its business service offer. Treat PAP as a source candidate only after policy and terms review; do not implement PAP scraping by default.
- Bankier exposes ESPI/EBI listings and article pages, including a JSON listing endpoint used by its UI. Treat Bankier as a secondary cross-check/fallback candidate, not the primary v1 official source.
- Bankier exposes `https://www.bankier.pl/rss/espi.xml`, which is a convenient secondary ESPI feed. It should not replace GPW canonical ingestion.
- Bankier's RSS directory also lists general market/news channels, including Giełda and Wiadomości dnia categories. Treat these as media/news candidates, not official-report sources.
- Parkiet exposes `https://www.parkiet.com/rss/7111-komunikaty`, which is a convenient secondary ESPI/EBI feed. It should not replace GPW canonical ingestion.
- Stooq exposes ticker news pages and PAP-sourced market news, but current investigation did not reveal a dedicated ESPI/EBI report-body source path suitable for v1 report ingestion.
- Stooq also exposes CSV quote/history endpoints used by community tooling. Treat this as a later price-context candidate, not as a news or report-body source.
- Investing.com Poland exposes an RSS directory with news and analysis categories, including company-news-style categories. Exact feed URLs and GPW ticker matching quality need source review.
- `wegar-2/pyespiebipapapi` is a community scraping wrapper for PAP ESPI/EBI pages and can inform PAP fallback research, but it is not an official PAP API and should not be depended on directly in the Rust/Tauri app.
- XTB market news and analysis pages are candidate media/analysis sources, not official-report sources, and need source review before ingestion.
- Portal Analiz is desired for v1 as an authenticated private research source using the user's own paid account, but requires a dedicated ADR before implementation.

## Open Source Questions

- Should the GPW listing fetcher switch from full-page GET to the observed `ajaxindex.php` listing fragment POST?
- Do GPW detail pages expose stable report body structure and attachments across enough report types?
- Are there explicit GPW terms that constrain automated polling of this page?
- Is PAP Biznes usable under acceptable terms for local personal polling, or is it a commercial/protected source unsuitable for v1 ingestion?
- Can PAP detail URLs be mapped directly from GPW/PAP identifiers without relying on search?
- Can Bankier/Parkiet RSS items be reliably reconciled with GPW report IDs or PAP report IDs?
- Which Bankier RSS channels have enough company-specific signal to justify a v1 media adapter?
- Which Investing.com RSS categories are relevant to GPW companies, and do they provide stable source URLs and enough ticker/company identifiers?
- Should Stooq price data become a context-enrichment adapter in v1, or stay post-v1 until news ingestion is stable?
- Are StockWatch/BiznesRadar technically and legally acceptable as scraping targets, or should they stay manual/open-in-browser references?
- Which Polish media/RSS sources should be considered after GPW official reports?
- Which public Polish article/news source should be implemented first after GPW details: Stooq, XTB, Bankier, or another source?
- What exact Portal Analiz scope is acceptable for v1: followed companies only, watchlist-only search results, selected pages saved manually, or automated polling?
