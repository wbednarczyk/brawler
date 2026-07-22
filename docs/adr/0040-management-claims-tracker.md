# ADR 0040: Management Claims Tracker — First-Class Claims, AI Extraction, and Due-Period Resurfacing (Design)

Status: Accepted — **amended 2026-07-20 by [ADR 0084](0084-retire-in-app-ai-layer.md)**: AI claim extraction from report documents/transcripts is removed in `v0.59.0`; claims are created through the manual path (`create_management_claim`), with agent-proposed claims returning via MCP write-tools (`v0.61.0`). Claim entity, due-period derivation, verdicts, and KPI-backed verification are unchanged.

This ADR captures the **design** for the management claims tracker (epic `cbf6999`, milestone `v0.42.0`): tracking management promises from reports and transcripts as first-class claims with a due period and a verdict, so a past statement resurfaces automatically when the due period's report arrives and can be resolved against evidence. It records the decisions made during milestone planning so contracts, data model, product spec, and UI flows are decision-complete before implementation.

It builds on:

- [ADR 0022](0022-research-evidence-read-model-boundary.md) — the research evidence read-model boundary and the generic `evidence_links` graph. Claims are already an evidence type; this ADR promotes the underlying record to its own entity without changing the boundary.
- [ADR 0027](0027-company-fundamentals-scope.md) — fundamentals scope and the financial-fact model a quantitative claim verifies against.
- [ADR 0034](0034-espi-event-classification.md) and [ADR 0036](0036-report-document-storage-and-backfill.md) — typed `company_signals`, `report_documents`, and report-arrival/period eventization. The resurfacing job hooks the arrival path these established.
- The **AI-extraction-with-mandatory-confirmation** pattern shipped for KPI extraction (`kpi_extraction_jobs` → `kpi_extraction_proposals` → confirm/reject → materialized fact; migration 0037). Claim extraction reuses this pattern verbatim in shape.

## Context

Claims already exist, but only as a flavor of note. A claim today is a `notebook_entries` row with `kind = 'claim'`, a `claim_status` enum (`open | delivered | partially_delivered | missed | unknown | not_applicable`), and `follow_up_after` / `follow_up_date` fields. They surface in the research timeline (`evidence_type = 'claim'`), participate in `evidence_links`, can spawn a `claim_follow_up` reminder, and ride the notebook-entry import/export path.

What the epic requires that does not exist:

1. **First-class claim semantics** — a claim is a tracked obligation with a normalized due period and a verdict, not an untyped note. The current model overloads `notebook_entries` and cannot cleanly carry extraction provenance, a verifying-fact link, or a revised-claim relationship.
2. **AI claim extraction** from report documents **and** transcripts, with mandatory user confirmation. No claim-extraction pipeline exists.
3. **Due-period resurfacing** — the exit criterion: when the due period's report arrives, the open claim must resurface for verification. Nothing matches open claims against arriving periods.
4. **Verification linkage** — a quantitative claim should link to the `financial_fact` that confirms or contradicts it.

## Decisions

### 1. Promote claims to a dedicated `management_claims` entity

Claims become their own table rather than a `notebook_entries` flavor. A claim is a first-class tracked obligation:

- core: `id`, `company_id`, `statement` (the promise, verbatim where extracted), `body`/context, `made_at` (when stated), `source_period` (period the statement was made about/in, optional);
- **due period**: a normalized `due_fiscal_year` + `due_period_type` (reusing the `financial_periods` period-type vocabulary: `FY|H1|H2|Q1..Q4|9M|M01..M12`), the matching key the resurfacing job uses;
- **verdict**: a `status` of `pending | delivered | partially_delivered | missed | revised`, user-set (see Decision 4);
- **provenance**: a soft `source_evidence_type` + `source_evidence_id` (a `report_document` or `transcript_segment`) plus the originating `claim_extraction_proposal_id` when AI-extracted;
- **verification**: a soft `verifying_fact_id` (see Decision 5);
- **quantitative target** (optional): a normalized expected `metric_key` / comparator / value for quantitative claims, so the queue can fetch the matching fact.

Migration: existing `notebook_entries` with `kind = 'claim'` (or non-null `claim_status`) are migrated forward into `management_claims` by an **idempotent, self-healing** forward migration (per the append-only migration rule). The prior `claim_status` values map onto the new `status` vocabulary (`open → pending`; `delivered`, `partially_delivered`, `missed` unchanged; `unknown`/`not_applicable` → `pending` with a note); `follow_up_after` is parsed into `due_fiscal_year`/`due_period_type` where it matches a period token, else left null. The research timeline and `evidence_links` keep `evidence_type = 'claim'` but now resolve against `management_claims.id`. Import/export gains a first-class `claims` bundle section; the legacy notebook-entry claim path is retired in the same bundle version with a documented mapping.

**Rejected:** extending `notebook_entries` (keeps overloading a table that should hold notes, and cannot carry the verdict/provenance/fact-link cleanly) and a satellite `claim_tracking` table (splits one concept across two tables and still leaves the note record as the identity). The epic's "first-class claims" language and the verification/automation requirements justify the dedicated entity and its one-time migration cost.

### 2. AI claim extraction reuses the KPI extraction pattern, over reports **and** transcripts

New `claim_extraction_jobs` and `claim_extraction_proposals` tables mirror the KPI extraction shape:

- a **job** records the async run (`company_id`, source ref, `provider_id`, `model`, `prompt_version`, `status`, error, timestamps);
- a **proposal** is a staged candidate claim (`statement`, suggested `due_*`, optional quantitative target, `confidence`, verbatim `source_snippet`, `status = pending | confirmed | rejected`, `claim_id` set on confirm).

Sources are both **`report_documents`** (reusing the document-text path KPI extraction already uses) and **`transcript_segments`** (earnings-call statements are a prime claim source). One job targets one source document/transcript; the proposal carries the snippet evidence. Confirmation is **mandatory**: only a confirmed proposal materializes a `management_claims` row (with provenance back to the proposal); rejected proposals are retained to prevent re-proposal and for audit. No claim is created without user review.

### 3. Resurfacing is a dedicated review queue driven by a due-period derivation job

When a report arrives and a `financial_period` (`company_id`, `fiscal_year`, `period_type`) is created or linked (the ADR 0034/0036 arrival path), a derivation job scans open (`status = pending`) claims for that company whose `due_fiscal_year`/`due_period_type` match the arriving period and adds them to a **"claims to verify" read model** surfaced in the company workspace. The queue buckets claims as **due** (period arrived), **overdue** (period passed, still pending), and **upcoming**. For a quantitative claim, the queue resolves and shows the matching `financial_fact` alongside the claim so the user can set the verdict in place.

Reminders and digests remain integrated but are **not** the primary surface: the same arrival can still create a `claim_follow_up` reminder (existing `research_reminders` kind) and claims continue to be citable evidence in digests. The dedicated queue is the focused verification workflow; reminders/digests are the cross-cutting notification/summary paths.

**Rejected:** reminder-only resurfacing (less visible; buries the verification action inside the generic reminders list) as the *sole* mechanism. We keep the reminder as a secondary path but build the queue as primary.

### 4. Verdict is the claim's own `status`, set by the user; no automated verdicts

The verdict lives on the claim as `status` (`pending | delivered | partially_delivered | missed | revised`). It is set by the user from the review queue. **No automated verdicts** — out of epic scope. A `revised` verdict records that management changed the claim; the superseding claim links to the prior via `evidence_links` (`updates`) and an optional `revises_claim_id`, mirroring how `financial_facts.supersedes_id` models restatement.

### 5. Verification linkage: a direct `verifying_fact_id` on the claim

A quantitative claim carries a soft `verifying_fact_id` referencing `financial_facts.id` — the canonical link the review queue queries to show "the fact next to the claim." When the due-period report arrives, the queue resolves the matching confirmed fact for the claim's `target_metric_key` and offers it as a candidate; the user sets the verdict and the chosen fact is stored on the claim.

Rationale: with claims now a first-class entity (Decision 1), the entity owns its key relationships as columns — consistent with `financial_facts.source_document_ref` and `financial_periods.report_evidence_ref` carrying their canonical soft refs directly. The direct column keeps the queue's hot path a single indexed lookup.

**Implementation finding (v0.42.0):** the original design also registered the claim↔fact relationship in the generic `evidence_links` graph (`supports`/`contradicts`). That graph (ADR 0022) does not list `financial_fact` as an evidence type — adding it requires extending the boundary and teaching the research timeline to render facts as evidence. Because `verifying_fact_id` already provides the canonical, queryable linkage, v0.42.0 ships the direct column only and **defers** the evidence-graph graft (and surfacing facts as timeline evidence) to a follow-up that extends ADR 0022. The `set_claim_verdict` input reserves a `verifying_relation` field for that follow-up.

### 6. Enforcement (per ADR 0038)

The milestone ships the gates that keep the decisions binding:

- migration tests for the `management_claims` forward migration, proving it is idempotent and that legacy `kind='claim'` notes converge to claims with mapped status;
- contract tests for the new commands (CRUD, set-verdict, extraction start/list/confirm/reject) and their error codes;
- a **resurfacing test that proves the exit criterion**: a claim with a due period, given an arriving period for that company, appears in the review queue and can be resolved with a verdict linked to evidence;
- extraction-confirmation tests asserting no claim materializes without a confirmed proposal, over both report and transcript sources;
- import/export round-trip coverage for the new `claims` bundle section.

## Consequences

- A one-time forward migration moves existing claim-notes into the new entity; the timeline, evidence graph, reminders, and export are re-pointed at `management_claims`. Reads of the new tables tolerate absence (safe defaults) so a database mid-migration never crashes startup.
- New storage module, commands, TS API, and a claims-tracker UI (queue + extraction confirmation + verdict actions), authored primitive-first per ADR 0037.
- Two extraction paths (report documents, transcripts) share one job/proposal/confirm pipeline.
- Quantitative claims gain a direct, queryable verification link without weakening the generic evidence boundary.

## Implementation Follow-ups

- The AI claim-extraction **launch** affordance (start extraction over a specific report document or transcript, then confirm proposals in the modal) is mounted in the report-document/transcript context as a fast-follow — the storage, async runner, commands, TS API, and confirmation flow ship in `v0.42.0`; only the in-context launch button is deferred. The claims tracker panel ships the queue, list, verdict, and manual-create flows.
- Registering the claim↔verifying-fact link in the generic `evidence_links` graph (Decision 5) — deferred until the evidence boundary models `financial_fact` as an evidence type.

## Out of Scope

- Automated verdicts without user review.
- Cross-company claim analytics (deferred; the dedicated entity makes it tractable later).
