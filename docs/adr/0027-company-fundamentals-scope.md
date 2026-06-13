# ADR 0027: Company Fundamentals Scope And KPI Taxonomy

## Status

Accepted.

## Context

Brawler tracks official company reports as feed items and evidence, but the numbers inside quarterly and annual reports stay locked in unstructured documents. The next product direction adds a fundamentals domain: structured financial facts per company, a fixed KPI taxonomy plus custom per-company KPIs, and simple KPI-over-time charts, with AI-assisted extraction from report documents.

This needs an explicit scope boundary before implementation. Brawler is not a portfolio tracker, trading tool, or market dashboard, and the roadmap excludes price/volume/technical charts. Report-derived fundamentals are decision support grounded in the same source-and-citation discipline as the rest of the app, not a market-data product. The boundary between the two must be unambiguous so later work does not drift into price tooling.

The feature also forces an open-core question: AI extraction calls a provider, and the app must decide whether that is a paid capability.

## Decision

### Scope

- Report-derived fundamental KPIs are in scope: values sourced from a company's periodic reports (and equivalent official disclosures), tracked per fiscal period, charted over time.
- Out of scope: price and volume series, technical-analysis indicators, market dashboards, valuation ratios that require live price data, and any screener over the broader market. These remain excluded as in `roadmap.md` "Not In V1".
- Every financial fact is an evidence-linked entity with provenance (source document reference, `extraction_method`, and confirmation state). A fact is never presented without a traceable source.
- Fundamentals are decision support, not advice. No buy/sell/hold or valuation recommendation is generated, consistent with [ADR 0016](0016-provider-neutral-ai-analysis-framework.md).

### KPI taxonomy

- A fixed, app-owned canonical taxonomy ships first, each entry with a stable key, value type, and display unit:
  - `revenue`, `operating_profit`, `net_profit`, `ebitda`, `net_debt`, `cash` — monetary, in the report currency
  - `eps` — monetary per share
  - `gross_margin`, `operating_margin`, `net_margin` — percentage ratios
- Margins may be stored directly when reported or derived from other facts; a derived margin still records how it was produced.
- Custom per-company KPIs are supported from day one for operating metrics the canonical set does not cover (for example subscribers, stores, order backlog):
  - a custom KPI is scoped to one company and is independent across companies (company A's `subscribers` is unrelated to company B's)
  - it carries a user key namespaced so it cannot collide with canonical keys, a display name, a value type (monetary, percentage, count, ratio), and a display format
  - custom KPI facts follow the same provenance and confirmation rules as canonical facts
- User-editable canonical labels and a shared cross-company custom-KPI vocabulary are deferred until there is real product pressure, mirroring the source-trust taxonomy approach in [ADR 0020](0020-sources-visibility-and-directory-boundaries.md).

### Open-core boundary

- AI fundamentals features (extraction and any later AI assistance over fundamentals) are part of the open core and free to use with a user-supplied provider API key. They are not gated behind a license token.
- The named future paid areas remain managed AI (provider access without the user supplying a key), cloud sync and backup, and official signed installers. These are recorded as direction only; no pricing, packaging, or entitlement detail belongs in public docs.
- This does not change the entitlement module or [ADR 0017](0017-license-gate.md). The fundamentals milestones add no new gated entitlements.

## Consequences

- Implementation milestones can proceed against a decision-complete domain: financial periods, financial facts, and KPI definitions (canonical plus custom) with provenance and confirmation state.
- The price/market boundary is explicit, so AI extraction, charts, and any external fundamentals source (for example a future EODHD study) stay on the report-derived side of the line or require a deliberate ADR to cross it.
- Charts render report-derived fundamentals only; reusing the chart primitives for price series would be a scope change requiring a new decision.
- Keeping AI fundamentals free preserves the local-first, BYO-key posture and draws the paid line at managed infrastructure rather than at core research capability.