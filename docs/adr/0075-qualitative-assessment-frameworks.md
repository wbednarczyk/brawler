# ADR 0075: Qualitative Assessment Frameworks — Agent-Assessed Criteria with Citations

Status: Accepted (v0.50.0)

Quality frameworks ([ADR 0046](0046-quality-frameworks-quantitative.md), shipped v0.44) evaluate a
user-owned checklist of **quantitative** criteria (a DSL over KPI metric keys) deterministically into an
immutable, versioned scorecard. Much of what separates a good business from a cheap one is **qualitative**
— moat, pricing power, recurring revenue, capital-allocation quality — and cannot be reduced to a metric
comparison. This ADR extends the existing framework/scorecard model with agent-assessed qualitative
criteria that carry citations to app-held evidence, compose into the same snapshot, and are re-evaluated by
autopilot — without touching the quantitative engine's semantics.

## Context

- The framework storage (`quality_frameworks` / `framework_criteria` / `framework_evaluations` /
  `criterion_results`, migration `0048_quality_frameworks.sql`) already models a versioned checklist and an
  immutable per-criterion outcome. Extending it in place is cheaper and keeps one scorecard than adding a
  parallel qualitative store.
- Per-capability AI routing ([ADR 0060](0060-ai-capability-routing-and-openai-compatible-provider.md), as
  amended) already gives every distinct AI call site a named, poolable capability with a global fallback —
  the seam a new qualitative capability plugs into.
- The research-brief path already grounds a text generation in app-held evidence and **rejects citations
  that do not reference supplied evidence** (`ai_research_brief_citations` + the
  `rejects_unknown_citation_keys` guard). Qualitative assessment reuses that reference model and that
  rejection discipline rather than inventing a new one.
- The app's standing boundary is decision-support, never advice ([ADR 0042](0042-ai-decision-support-not-advice.md)
  posture): stored agent reasoning must not phrase buy/sell/hold or allocation output.

## Decision

1. **Criterion model.** `framework_criteria` gains `kind` (`quantitative` | `qualitative`) and
   `assessment_guidance` (owner-authored prompt seed, per criterion). A qualitative criterion carries
   `assessment_guidance` and **no DSL expression**; a quantitative criterion is unchanged. Existing rows
   backfill to `kind = quantitative` via an idempotent, safe-default migration (a missing/NULL `kind` reads
   as `quantitative`). The migration is append-only ([data-model.md](../data-model.md) migration rules); the
   shipped `expression` column stays `NOT NULL`, so qualitative rows store an empty-string `expression`
   (guidance lives in `assessment_guidance`, not `expression`).

2. **New AI capability `QualitativeAssessment`.** Added to the ADR 0060 capability map as a **text** capability
   (settings key `qualitative_assessment`), routed through the text-capable provider pool and falling back to
   `general_analysis_provider` when unmapped. One provider request **per criterion per company** — small,
   citable, retryable — never one mega-prompt per framework.

3. **Evidence scope (app-held only, no web).** Each request is grounded exclusively in app-held sources for
   the company: stored report documents (latest periodic + latest annual), research evidence links, claims
   and their verdicts, recent typed signals, and notebook notes. The prompt lists the typed evidence
   identifiers it was given; the response **must cite them**. No web access.

4. **Result shape.** `criterion_results` is extended (agent-assessed rows only) with `verdict`
   (`pass` | `partial` | `fail` | `insufficient_evidence` — the qualitative verdict set adds
   `insufficient_evidence` to the quantitative `pass`/`partial`/`fail`/`unavailable`), `reasoning` (short),
   `citations` (typed evidence refs — the `ai_research_brief_citations` reference model: `evidence_type`
   from `ResearchEvidenceType`, `evidence_id`, `label`, `snippet`), `confidence` (`low` | `medium` | `high`),
   `prompt_version`, and `source` (`agent` for these rows; `source` is added `DEFAULT 'engine'` so the
   append-only `ALTER … ADD COLUMN` succeeds and pre-migration quantitative rows read as engine-sourced).
   There is **no user-confirmation gate** (results are labeled
   agent opinion, not facts — unlike KPI extraction, which confirms into `financial_facts`), but results are
   visually distinct, regeneratable, and never mutate quantitative data.

5. **Scorecard composition.** Agent results merge into the **same** immutable evaluation snapshot with a
   per-criterion `source`; a snapshot records the prompt versions used. Change detection compares verdicts
   against the previous snapshot; verdict changes surface in digests and on autopilot re-evaluation. The
   assist/autopilot trust rungs re-enqueue assessment on new-report arrival; `off` never auto-runs
   ([ADR 0055](0055-autonomous-report-pipeline-trust-ladder.md)).

   **Two read surfaces, fixed boundary.** Because a snapshot may be quant-only, qual-only, or combined,
   "the latest snapshot" is not a reliable source of qualitative rows. The evaluation reads
   (`get_framework_evaluation` / `list_framework_evaluations`) return qualitative fields **as snapshotted in
   that run** (immutable audit/history). The panel's **current-state** read returns, per qualitative
   criterion, the **most recent agent-assessed row across all snapshots** for the company × framework — so a
   later quant-only run never blanks an existing assessment. A never-assessed criterion is absent (empty
   state), distinct from an `insufficient_evidence` verdict.

6. **Decision-support hard rule.** The prompt template forbids buy/sell/hold and allocation language, and a
   test asserts no such phrasing is present in stored `reasoning` (guardrail below).

## Consequences

- The framework/scorecard model becomes mixed quantitative + qualitative behind one snapshot; the
  quantitative engine, its verdicts, and its determinism are untouched. Quality panels gain agent-assessed
  rows that are labeled, cited, and regeneratable.
- New surface: two `framework_criteria` columns + five `criterion_results` columns (append-only migration,
  T2), the `QualitativeAssessment` capability + prompt (T3), a durable `qualitative_assessment` job (T4),
  three planned commands + DTOs and the Quality-panel/editor UI (T5), the Kroeze qualitative criteria set +
  combined-snapshot composition + autopilot wiring (T6).
- Uncited or hallucinated-reference reasoning is **never stored**: the job rejects a response whose citations
  do not resolve to supplied evidence ids (research-brief precedent), and no provider means the job fails
  with a clear error rather than degrading.
- A qualitative verdict is **agent opinion**, always visually distinct from measured quantitative outcomes;
  it never writes or overrides a `financial_fact` or a quantitative `criterion_result`.

## Guardrail (ADR 0045)

- **Citation integrity:** a stored qualitative result's every citation must reference an evidence id supplied
  to that request; the job rejects otherwise (a rejection test pins this). Loosening citation validation to
  accommodate a provider response is a STOP, not a fix — never store uncited reasoning.
- **No advice:** a phrase guard on the prompt template and a test over stored `reasoning` assert the absence
  of buy/sell/hold / allocation language.
- **Quantitative isolation:** any change to quantitative evaluation semantics is out of scope for this ADR.

## Open / to confirm at implementation

- Evidence-bundle size vs provider context: if the app-held evidence for a company exceeds the provider
  context window, the bundle-pruning policy is an **owner decision** (STOP-AND-ASK) — not silently truncated.
- Whether qualitative and quantitative criteria share one evaluation run trigger or two: composition into one
  snapshot is decided (Decision 5); the run/enqueue ergonomics are settled in T4/T6.
- The `CriterionVerdict` ts-union gains `insufficient_evidence`; whether qualitative reuses the same union or
  gets a sibling union is a T2/T5 DTO detail (the stored value set is fixed here).

### As-implemented resolutions (v0.50.0, T6 quality gate)

- **Report documents in the bundle (Decision 3).** The codebase's periodic-report axis is
  consolidated (`ssf`) vs standalone (`jsf`) statement types, not "periodic vs annual" — the bundle
  includes the **newest stored report document per statement type (≤2 documents)**, selected by
  domain period (`classify_statement` + `period_sort_key`), each as a citable `report_document`
  evidence item. **Pruning bound (owner-ratified default): 12 000 characters of extracted text per
  document, marked `[truncated]` when cut**; a document without an extractable text layer falls back
  to a citable title-only entry.
- **Run/enqueue ergonomics (Decision 5).** One durable job row per `company × framework`; a request
  arriving while that job is running parks in a deterministic `…:followup` row (merged like pending
  requests) and a symmetric claim-time guard prevents the pair from ever running concurrently — a
  re-run request is never silently dropped and a paid request is never duplicated.
- **Digest surfacing (Decision 5).** Verdict-change items in the company digest are bounded by the
  company review checkpoint (same `reviewed_at` gate as timeline evidence), so a surfaced change is
  suppressed once the company is marked reviewed.
