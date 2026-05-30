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

## V1 Source Priority

V1 source priority:

1. GPW ESPI/EBI official reports.
2. Selected Polish public/RSS media sources after source policy review.
3. Later official/public adapters for SEC EDGAR, Nasdaq RSS, and major European exchange disclosures.

## GPW ESPI/EBI Adapter

Adapter ID: `gpw-espi-ebi`

Source URL: `https://www.gpw.pl/komunikaty`

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

Milestone 5 starts with fixture-backed listing parsing and SQLite ingestion before wiring an explicit manual live fetch. Automated tests must stay fixture-backed or use injected fetchers so default checks do not depend on GPW availability.

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

- ISIN from the report listing to `companies.isin`.

Fallback matches:

- exact normalized company name to company aliases
- manually configured source-specific IDs
- exchange-qualified ticker when a reliable ticker appears in a future source field

Rules:

- Do not rely on ticker alone for GPW matching.
- Unmatched items can still be stored, but they should not appear in company-specific views until matched.
- The Sources screen should expose unmatched counts and recent unmatched item diagnostics after implementation supports them.
- Matched listing ingestion writes `feed_item_companies.match_type = "isin"`.
- Re-ingesting the same listing updates source metadata but preserves read/saved user state.

## Detail Fetching

V1 should start with listing-level ingestion. Detail-page fetching is allowed only when:

- the detail URL is public,
- fetching does not require authentication,
- source attribution is preserved,
- adapter tests use fixtures,
- rate limiting is conservative.

Detail fetch should capture:

- original report body or excerpt when legally and technically acceptable
- attachment links if visible and relevant
- source URL

If detail pages are unstable, the adapter should still store listing-level feed items and link to the original source.

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

Media/RSS sources are in scope for v1 only after each source is reviewed.

Each media/RSS source needs:

- source URL
- usage policy note
- attribution label
- rate limit
- supported language
- matching strategy
- dedupe strategy

Do not add broad website scraping as a generic capability in v1.

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
- No documented public GPW ESPI/EBI API has been accepted for v1 yet. M5 uses the public listing page until a stable documented API or acceptable internal pagination endpoint is confirmed.
- PAP Biznes exposes ESPI/EBI-related public pages, including `https://biznes.pap.pl/espi` and `https://biznes.pap.pl/ebi/company`, and describes ESPI/EBI communications in its business service offer. Treat PAP as a source candidate only after policy and terms review; do not implement PAP scraping by default.

## Open Source Questions

- Is there an undocumented network endpoint behind GPW's "Pokaż więcej" pagination that can be used more reliably than parsing HTML?
- Do GPW detail pages expose stable report body structure and attachments?
- Are there explicit GPW terms that constrain automated polling of this page?
- Is PAP Biznes usable under acceptable terms for local personal polling, or is it a commercial/protected source unsuitable for v1 ingestion?
- Which Polish media/RSS sources should be considered after GPW official reports?
