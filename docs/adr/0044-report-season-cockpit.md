# ADR 0044: Report-Season Cockpit — Composed Read Models and Preparation State (Design)

Status: Accepted

This ADR captures the **design** for the report-season cockpit (epic `75001e4`, milestone `v0.43.0`): one time-driven view that prepares the investor for report season — upcoming report dates across watchlists, each with a pre-report card assembled from open research questions, unresolved management claims, last-period confirmed KPIs, and recent evidence, plus a prepare→process workflow that closes the loop when the report arrives. It records the decisions made during milestone planning so contracts, data model, product spec, and UI flows are decision-complete before implementation.

It builds on:

- [ADR 0022](0022-research-evidence-read-model-boundary.md) — the research evidence read-model boundary: cross-domain views are **backend-owned read models assembled from canonical domains first**, with no stored projection until performance or review semantics require one. The cockpit read models follow this rule verbatim.
- [ADR 0034](0034-espi-event-classification.md) and [ADR 0036](0036-report-document-storage-and-backfill.md) — typed `company_signals`, `report_documents`, and the derived calendar events (`event_type = 'periodic_report' | 'dividend' | 'shareholder_meeting'`) that supply the cockpit's report dates.
- [ADR 0027](0027-company-fundamentals-scope.md) — the `financial_periods` / `financial_facts` / `kpi_definitions` model the card's last-period KPIs read from.
- [ADR 0040](0040-management-claims-tracker.md) — first-class management claims and the due-period resurfacing read model the card composes for "claims to verify before the report".
- [ADR 0038](0038-enforcement-as-guardrails.md) — enforcement-as-guardrails.

## Context

Everything the cockpit displays already exists in its owning domain: report dates as derived `company_events`, open questions in `research_questions`, unresolved claims in `management_claims`, last-period KPIs in `financial_facts`, and recent activity in the research-timeline read model. What does **not** exist:

1. A view that **aggregates upcoming report dates across watchlists** and orders them by date — `list_company_events` is single-company / single-watchlist scoped and event-centric, not a season overview.
2. A **per-company pre-report card** that answers "what should I check before this company reports" by composing the four domains in one backend call.
3. **Preparation workflow state** — there is no place to record that the user has reviewed a company ahead of its report, or that an arrived report has been processed. This is the one genuinely new piece of persisted state.

## Decisions

### 1. Cockpit views are backend-owned read models, not stored projections

The report calendar and the pre-report card are derived read models assembled in the backend from their canonical domains, per [ADR 0022](0022-research-evidence-read-model-boundary.md). They add **no** stored projection table and **no** duplicated domain logic: the calendar reuses the `company_events` query path; the card composes `list_research_questions`, the `management_claims` due-period read model, the `financial_facts`/`financial_periods` KPI reads, and the research-timeline recent-evidence read. If aggregation cost ever becomes a concern, a stored projection is an additive future step, not part of this milestone.

The **report calendar** read model aggregates `company_events` with `event_type = 'periodic_report'` for companies in a watchlist scope (or all tracked companies when unscoped), split into `upcoming` (event date ≥ today) and `past`, ordered by date. It surfaces calendar **freshness/diagnostics** (last fetch, staleness) so a stale calendar is visible rather than silently empty.

The **pre-report card** read model, keyed by `(companyId, eventKey)`, composes for one company: open research questions, unresolved claims (the due-period buckets from [ADR 0040](0040-management-claims-tracker.md)), last-period confirmed KPIs, and recent evidence — plus the company's preparation state (Decision 2).

### 2. Preparation/processed state is a dedicated `report_preparations` table

The prepare→process workflow needs durable per-occurrence state. We add a small `report_preparations` table keyed by `(company_id, event_key)`, where `event_key` is the stable `company_events.source_event_key` of the report occurrence:

- `status`: `upcoming | prepared | processed` (default `upcoming` — absence of a row means `upcoming`).
- `prepared_at`, `processed_at`: timestamps for the transitions.
- `linked_report_document_id`: nullable soft reference to the arrived `report_documents` row, set on processing.

We chose a dedicated table over extending `company_events` because derived calendar events are **additive and may be re-derived/replaced** ([ADR 0036](0036-report-document-storage-and-backfill.md)); preparation is user-owned workflow state that must survive event re-derivation and has its own lifecycle. The table is keyed by the stable `source_event_key`, not the volatile event row id, so it survives re-derivation. Reads of preparation state **tolerate a missing row** and default to `upcoming`, so an absent migration or a not-yet-prepared company never crashes the cockpit.

### 3. Workflow transitions, not automation

`mark_report_prepared` and `mark_report_processed` are explicit user actions. On processing, the card links to the arrived filing and the existing KPI-extraction entry point, and ties back to the claims-review queue — it does **not** auto-extract or auto-confirm anything. The confirm-before-commit guarantee is unchanged; the autonomous path is the separate North Star (`v0.49.0`).

### 4. IA: a time-driven surface adjacent to Inbox

The cockpit is a primary-navigation surface placed next to Inbox (a "what's coming" view parallel to the unread feed), not a tab inside Companies. Rationale: it is time/season-driven like the feed, spans the whole watchlist rather than one company, and is the natural launch point into per-company workspaces during report season. It drills into the company workspace, its research questions, and its claims.

### 5. Enforcement (per ADR 0038)

- The `report_preparations` migration is idempotent and self-healing (`CREATE TABLE IF NOT EXISTS`), and reads default a missing row to `upcoming`.
- `status` values are validated against the allowed set at the storage boundary, matching the `company_events` validation pattern.
- en/pl translation parity and the existing translation/pluralization/a11y guards cover the new screen copy.
- Storage tests assert the read models compose the owning domains (no duplicated logic) and that the prepare→process transitions persist; a workflow test covers cockpit rendering and drill-in navigation.

## Consequences

- The milestone stays small: two derived read models, one tiny new table, three new commands, and one new screen. No new external source, no new ingestion.
- Preparation state is portable: it joins the import/export research-data bundle as a future per-feature coverage item (`v0.52.0`), not in this milestone.
- The cockpit becomes the launch point the North Star (`v0.49.0`) autonomous pipeline later automates; the manual prepare→process loop defined here is the workflow autopilot will eventually drive.

## Out of Scope

- Price-reaction views and market-wide earnings calendars beyond tracked companies.
- Any auto-fetch/auto-extract on report arrival (North Star, `v0.49.0`).
- A stored cockpit projection table (additive future optimization only).
