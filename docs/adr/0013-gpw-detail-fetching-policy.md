# ADR 0013: GPW Detail Fetching Policy

Status: Accepted

## Context

One of Brawler's fundamental product requirements is that the user can read the body of an official report inside the app, not only see a title and click out to a browser. Milestone 5 implemented GPW ESPI/EBI listing-level ingestion from the public GPW reports page. Listing-level data is useful, but it is not sufficient for the product because it usually contains only title, source URL, publication timestamp, report category, report number, company label, and ISIN.

Milestone 6 tested whether GPW detail pages can provide report body text and attachment links. Sample pages are public and do not require authentication, but they render inside noisy full GPW pages. The useful report content is currently exposed under `.report-data`, with section labels such as `Treść raportu:` and `Nazwa arkusza:`.

The implementation must not turn "HTML parsing is imperfect" into "report body is optional." If GPW detail-page parsing becomes unreliable, the project must choose another acceptable source path rather than dropping in-app report bodies.

Known fallback candidates:

- PAP Biznes public ESPI/EBI pages and source links, subject to policy and terms review.
- Official issuer investor-relations pages when they expose the same report body or attachment.
- Commercial or third-party APIs only after explicit approval, privacy review, and cost/monetization discussion.

Additional M6 investigation notes:

- GPW's public listing page uses an internal `ajaxindex.php` POST endpoint for listing fragments. This is a better listing transport than parsing the full page, but it is still an implementation detail of the public page, not a documented API contract.
- GPW detail pages remain the primary report-body source because they expose the report content under `.report-data` and preserve the GPW source URL.
- PAP Biznes is the strongest fallback candidate because GPW detail content is PAP-rendered and PAP exposes public ESPI/EBI pages. It still needs policy and direct-link validation before implementation.
- Bankier exposes ESPI/EBI listing JSON and article pages with report text. It is useful for cross-checking and possible emergency fallback, but it is not the original source.
- Bankier and Parkiet expose convenient ESPI/EBI RSS feeds. RSS is lighter and less fragile than GPW HTML detail parsing, but these feeds are secondary republication paths, not canonical GPW/PAP origin paths.
- Stooq appears better suited to general market/company news than official ESPI/EBI report-body ingestion.

## Decision

In-app official report body access is required for v1 GPW support.

GPW detail fetching is accepted as the primary implementation path for GPW report bodies unless it proves technically or policy-wise unacceptable. It should be wired into normal ingestion for matched GPW feed items under conservative limits.

Secondary RSS feeds from Bankier and Parkiet are accepted as supportive signals for cross-checking, diagnostics, and possible fallback/backfill hints. They must be implemented as separate source adapters with their own attribution if used. They must not replace GPW as the canonical source for official report bodies while the official GPW path remains acceptable.

Detail fetching must follow these constraints:

- Keep the injectable fetch boundary so automated tests remain test-sample-backed and offline.
- Fetch details for matched feed items by default.
- Do not fetch detail pages for unmatched diagnostics by default.
- Cap detail fetches to 5 pages per refresh at first.
- Serialize detail requests and wait at least 2 seconds between detail requests.
- Do not automatically backfill old detail pages without a separate user action or roadmap decision.
- Preserve the original GPW detail URL as the source of truth.
- Treat parsed body text as source-derived report text, not as editorial summary.
- Store attachment links only when they are visible and source-attributed.
- Keep listing-level ingestion as a temporary fallback for individual items when detail fetching fails, but surface the failure in source diagnostics.

The parser must evaluate each parsed detail page before ingestion. Missing title or missing/very short report body text makes the detail unusable for body storage. Any rejected sample in the spike report blocks claiming the parser is broadly stable, but does not remove the product requirement. It instead triggers fallback-source investigation or parser hardening.

## Consequences

- M6 is not complete until the plan clearly preserves in-app report body access as required functionality.
- Normal GPW ingestion should gain detail-body fetching for matched items, subject to conservative caps.
- Source status should surface detail-fetch counts and warnings.
- Database retention policy must account for larger source-derived body text before detail bodies are stored broadly.
- The GPW listing path remains a temporary per-item fallback and diagnostic baseline, not the final feature shape.
- If GPW detail-page markup becomes too unstable, PAP/issuer/commercial alternatives must be evaluated rather than dropping body text.
- Listing transport can move from full GPW page parsing to the observed GPW AJAX listing fragment if tests keep the same normalized contract and the adapter still attributes GPW as the source.
- Secondary RSS can reduce missed-item risk, but canonical official report storage should still prefer reconciled GPW/PAP source data over secondary republication text.
