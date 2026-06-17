# ADR 0029: Report-Document Source Ladder and AI-Assisted IR-Page Resolution

## Status

Accepted.

## Context

AI KPI extraction (v0.36.0, [ADR 0028](0028-multi-provider-ai-boundary.md) native-first delivery) operates on a stored `report_document`. Getting the right report file in front of the model is its own problem. GPW periodic-report feed items (ESPI/EBI) usually carry the report as an attachment, but not always: some filings reference an investor-relations (IR) page rather than attaching the document, and occasionally a filing is missed entirely.

The owner's framing: a company's **IR reports page URL is durable** — it changes very rarely — so if a report was published, it is ~99% certain to appear on that page. The uncertain part is not the page URL but **locating the specific report link within the page**, whose HTML layout differs per company.

[ADR 0027](0027-company-fundamentals-scope.md) already rejected per-company PDF parsers (per-company × per-layout maintenance). The same logic applies to per-company IR-page scrapers. The source policy in `AGENTS.md` also requires a source-specific ADR before fetching/scraping arbitrary pages.

## Decision

### 1. Report-document source ladder

Resolve a report document in priority order:

1. **ESPI/EBI attachment** (primary) — the filing's own attachment; the most authoritative source, already captured by the report-document machinery.
2. **Per-company IR reports page** (fallback) — when no usable attachment exists, resolve the specific report from the company's stored IR page using the event context (period, report type, publication date).
3. **Manual PDF URL paste** (last resort) — for the rare case both above miss.

This demotes manual paste from a primary path to a true last resort.

### 2. Per-company IR reports-page URL is durable configuration

Each company gains an optional, user-editable `ir_reports_url`. It is stored once and reused; it is not re-derived per report. Attribution for a document fetched via the IR page records the IR page URL.

### 3. AI-assisted generic resolver (no per-company scrapers)

Locating the report on the IR page is done generically:

- fetch the IR page HTML and extract candidate links (generic link/anchor extraction, not per-company selectors);
- hand the AI the candidate list plus the event context; it returns the best-matching report URL;
- on low confidence or ambiguity, surface the candidates for the user to pick;
- the chosen URL flows through the existing `report_documents` capture + fetch path, so downstream extraction is unchanged.

One generic resolver, no per-company code, with a deterministic test path for tests.

### 4. Source-policy approval (scoped)

Fetching a user-configured IR page's HTML for the purpose of locating an official report link is **approved** under the source policy. Scope and limits:

- only a URL the user explicitly configured per company is fetched;
- the fetch retrieves the IR listing page to enumerate candidate links; it is not a crawler and does not follow the page's link graph;
- the resolver selects among already-present links; it does not bypass paywalls, logins, or robots restrictions, and a failed/blocked fetch degrades to manual paste;
- no new hosted dependency or third-party service is introduced.

### 5. User-triggered now; event-driven automation later

In v0.36.0 the resolver is **user-triggered** (a "fetch report from IR page" action). Wiring it to fire automatically on report-event detection is the v0.49.0 autonomous report pipeline (task `05ebf07`/epic `9a607da`), behind the per-company trust ladder. v0.36.0 delivers the durable building block; v0.49.0 delivers the automation.

## Consequences

Positive:

- Extraction is no longer blocked when a filing lacks an attachment; the durable IR URL covers the common gap.
- No per-company scrapers; the resolver is one generic, testable path that reuses the AI provider boundary.
- The IR URL field and resolver are exactly the building blocks the autonomous pipeline (v0.49.0) needs, drawn before automation so the trust ladder composes them rather than retrofitting.

Negative / costs:

- Fetching arbitrary user-configured HTML carries variability (layout, blocking, transient failures); mitigated by graceful degradation to candidate-pick and manual paste, and by keeping the fetch a single-page enumeration rather than a crawl.
- AI link selection can mis-pick; mitigated by confidence-gated candidate surfacing and the mandatory downstream per-fact confirmation (a wrong document still cannot commit a fact without user review).

## Alternatives Considered

- **Per-company deterministic IR scrapers**: rejected — per-company × per-layout maintenance burden, consistent with the per-company-PDF-parser rejection in ADR 0027.
- **Manual-assist only** (open the IR page, user clicks the report): viable and lower-risk, but more clicks and no path to the v0.49.0 automation; kept as the graceful-degradation fallback rather than the primary mechanism.
- **Crawling the IR site / search engines to find reports**: rejected — fragile, broad, and outside the source policy; the durable per-company URL is both simpler and more reliable.

## Implementation

Tracked under epic `9879941` (milestone v0.36.0):

1. This ADR — source ladder and AI-assisted IR-page resolution.
2. Per-company `ir_reports_url` field (migration, storage, command, company UI) — task `8ab19a4`.
3. AI-assisted IR-page report resolver, user-triggered, with candidate-pick fallback and a deterministic test path — task `9d6a3a5`.

Event-driven automation over this building block is deferred to v0.49.0 (epic `9a607da`).
