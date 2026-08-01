// Hostile-content and adversarial scenario overlays (ADR 0081 Q2, Radicle
// a9992e2).
//
// Pure, fixed-ID/fixed-time transformations layered on top of a base
// `ScenarioData` store (`empty | minimal | rich`). Each overlay REASSIGNS the
// collections it touches — never mutates an entity in place — matching the
// same store-mutation contract `runtime.ts` handlers follow (see
// docs/testing.md "mock runtime conventions"). Overlays are additive: they
// never remove base-scenario entities, only add fixed `*_overlay_*`-id
// entities (or, for `partial-data`, remove ONE specific derived read for a
// dedicated overlay-only company so base coverage is untouched).
//
// Application order is fixed by `OVERLAY_ORDER`, not by caller-supplied
// array order, so `applyScenarioOverlays` is deterministic and idempotent for
// a repeated overlay name (a `Set` collapses duplicates before iterating).

import {
  SAMPLE_NOW,
  makeAlertRule,
  makeAttentionEvent,
  makeCompany,
  makeFeedItem,
  makeFinancialPeriod,
  makeResearchEvidenceItem,
  makeSourceAdapter,
  makeSourceIngestionResult,
  type CompanySpec,
} from "./entities";
import type { ScenarioData } from "./scenarios";
import type { FinancialFact, KpiDefinition } from "../../api/financialsTypes";

export type ScenarioOverlayName =
  | "hostile-content"
  | "dense-history"
  | "partial-data"
  | "preliminary-fundamentals"
  | "stale-processing"
  | "conflicting-statuses"
  | "mixed-locale"
  | "attention-overflow"
  | "attention-mixed-severity"
  | "attention-notable-only"
  | "morning-review"
  | "today-dense"
  | "orphaned-evidence"
  | "pruned-feed"
  | "degraded-sources"
  | "failed-autopilot-run"
  | "job-failed-event";

// One fixed, distinct `CompanySpec` per overlay so simultaneous overlays never
// collide on IDs and each overlay's content is independently identifiable.
const HOSTILE_SPEC: CompanySpec = {
  key: "hostile",
  ticker: "ZZZH",
  name: "Zażółć Gęślą Jaźń — Ω Reserved Bardzo Długa Nazwa Testowa Spółki Akcyjnej S.A.",
  sector: "Technology",
};
const DENSE_SPEC: CompanySpec = { key: "dense", ticker: "ZZZD", name: "Dense History Test Sp. z o.o.", sector: "Technology" };
const PARTIAL_SPEC: CompanySpec = { key: "partial", ticker: "ZZZP", name: "Partial Data Test S.A.", sector: "Financials" };
const PRELIMINARY_SPEC: CompanySpec = { key: "preliminary", ticker: "ZZZQ", name: "Preliminary Fundamentals Test S.A.", sector: "Technology" };
const STALE_SPEC: CompanySpec = { key: "stale", ticker: "ZZZS", name: "Stale Processing Test S.A.", sector: "Energy" };
const MIXED_SPEC: CompanySpec = { key: "mixed", ticker: "ZZZM", name: "Mieszany Test Lokalizacji S.A.", sector: "Consumer Staples" };

/** A single unbreakable (no-whitespace) long URL — stresses layout wrapping. */
const HOSTILE_URL = `https://example.test/hostile/${"raport-finansowy-bez-spacji-".repeat(6)}dokument.pdf`;

function applyHostileContent(data: ScenarioData): ScenarioData {
  const company = makeCompany(HOSTILE_SPEC);
  const longTitle =
    "Kwartalny raport finansowy — ".repeat(10) +
    "Zażółć gęślą jaźń, wyniki Q3 2026 (bardzo długi tytuł do testów odpornościowych)";
  const longBody = "Treść komunikatu bieżącego z długim opisem sytuacji spółki. ".repeat(60);
  const feedItem = {
    ...makeFeedItem(HOSTILE_SPEC, 0),
    id: "feed_overlay_hostile_1",
    title: longTitle,
    summary: longBody.slice(0, 400),
    bodyText: longBody,
    sourceUrl: HOSTILE_URL,
    attachments: [{ id: "attach_overlay_hostile_1", label: HOSTILE_URL, url: HOSTILE_URL }],
  };
  return {
    ...data,
    companies: [company, ...data.companies],
    feedItems: [feedItem, ...data.feedItems],
  };
}

/** Hundreds of feed rows — ONLY materializes when explicitly selected. */
function applyDenseHistory(data: ScenarioData): ScenarioData {
  const extra = Array.from({ length: 250 }, (_, index) => ({
    ...makeFeedItem(DENSE_SPEC, index),
    id: `feed_overlay_dense_${index}`,
  }));
  return { ...data, feedItems: [...extra, ...data.feedItems] };
}

/**
 * A populated sibling domain (financial periods) with the relevant read
 * (financial facts) missing for the SAME company — the partial-category
 * failure state J1/J2 must render explicitly, never as silent "quiet".
 */
function applyPartialData(data: ScenarioData): ScenarioData {
  const company = makeCompany(PARTIAL_SPEC);
  const period = makeFinancialPeriod(PARTIAL_SPEC, 2026);
  return {
    ...data,
    companies: [company, ...data.companies],
    financialPeriods: [period, ...data.financialPeriods],
    // No matching financialFacts row for PARTIAL_SPEC's company — the missing
    // relevant read.
  };
}

/**
 * ADR 0093 dec. 2 preliminary lifecycle (epic #285 T5 dogfooding finding):
 * one period holds a `final` fact that SUPERSEDES a `preliminary` sibling in
 * the same slot (`data_quality` is part of the storage uniqueness key, so
 * both rows coexist there) — the fact matrix must render the final-preferred
 * value, never "whichever fact came last in the array". A second period
 * holds a preliminary-ONLY fact (no final sibling yet), so the marker/chip
 * also render with nothing to prefer over. Facts are ordered `created_at`
 * DESC, matching the real backend contract: the final fact (created later —
 * the audited report lands after the preliminary release) sorts BEFORE its
 * preliminary sibling, which is exactly the ordering that used to trip the
 * fact matrix's old "last object in the array wins" bug (factMatrix.ts)
 * before the ADR 0093 dec. 2 fix. Reproduces the state so it renders in CI
 * forever (docs/testing.md "UI dogfooding finding ⇒ overlay").
 */
function applyPreliminaryFundamentals(data: ScenarioData): ScenarioData {
  const company = makeCompany(PRELIMINARY_SPEC);
  const supersededPeriod = makeFinancialPeriod(PRELIMINARY_SPEC, 2025);
  const openPeriod = makeFinancialPeriod(PRELIMINARY_SPEC, 2026);
  const definition: KpiDefinition = {
    id: "kpidef_overlay_preliminary_net_profit",
    scope: "global",
    companyId: null,
    sector: null,
    metricKey: "net_profit",
    label: "Net profit",
    valueKind: "monetary",
    unit: null,
    computation: "reported",
    formula: null,
    displayFormat: null,
    origin: "seed",
    createdAt: SAMPLE_NOW,
    updatedAt: SAMPLE_NOW,
  };
  const supersededPreliminary: FinancialFact = {
    id: "fact_overlay_preliminary_2025_preliminary",
    companyId: company.id,
    periodId: supersededPeriod.id,
    definitionId: definition.id,
    valueNumeric: "980000000",
    currency: "PLN",
    statementBasis: "consolidated",
    attribution: "total",
    variant: "reported",
    measureWindow: "flow",
    dataQuality: "preliminary",
    asReportedValue: "980.0",
    asReportedScale: "million",
    reportingStandard: "IFRS",
    extractionMethod: "mcp_agent",
    confidence: null,
    confirmationState: "confirmed",
    supersedesId: null,
    sourceDocumentRef: null,
    annotation: null,
    createdAt: "2026-01-20T09:00:00Z",
    updatedAt: "2026-01-20T09:00:00Z",
  };
  const supersedingFinal: FinancialFact = {
    ...supersededPreliminary,
    id: "fact_overlay_preliminary_2025_final",
    valueNumeric: "1010000000",
    asReportedValue: "1010.0",
    dataQuality: "final",
    extractionMethod: "esef",
    supersedesId: supersededPreliminary.id,
    sourceDocumentRef: `report_doc_sample_${PRELIMINARY_SPEC.key}`,
    createdAt: "2026-03-15T09:00:00Z",
    updatedAt: "2026-03-15T09:00:00Z",
  };
  const preliminaryOnly: FinancialFact = {
    ...supersededPreliminary,
    id: "fact_overlay_preliminary_2026_preliminary",
    periodId: openPeriod.id,
    valueNumeric: "1150000000",
    asReportedValue: "1150.0",
    createdAt: "2027-01-20T09:00:00Z",
    updatedAt: "2027-01-20T09:00:00Z",
  };

  return {
    ...data,
    companies: [company, ...data.companies],
    financialPeriods: [supersededPeriod, openPeriod, ...data.financialPeriods],
    kpiDefinitions: [definition, ...data.kpiDefinitions],
    financialFacts: [preliminaryOnly, supersedingFinal, supersededPreliminary, ...data.financialFacts],
  };
}

/** An old, still-visible result alongside an in-flight job for the same scope. */
function applyStaleProcessing(data: ScenarioData): ScenarioData {
  const company = makeCompany(STALE_SPEC);
  const oldEvidence = { ...makeResearchEvidenceItem(STALE_SPEC), id: "research_overlay_stale_1" };
  return {
    ...data,
    companies: [company, ...data.companies],
    researchEvidence: [oldEvidence, ...data.researchEvidence],
  };
}

/**
 * Two independent read models deliberately disagree about the SAME adapter:
 * the adapter card reports `attention`/an error while its latest ingestion
 * result shows a clean successful run.
 */
function applyConflictingStatuses(data: ScenarioData): ScenarioData {
  const targetId = "bankier-company-komunikaty";
  const hasTarget = data.sourceAdapters.some((adapter) => adapter.id === targetId);
  if (!hasTarget) {
    return data;
  }
  const adapters = data.sourceAdapters.map((adapter) =>
    adapter.id === targetId
      ? {
          ...adapter,
          healthStatus: "attention" as const,
          lastError: "Sample fetch warning (conflicting-statuses overlay)",
          lastErrorAt: SAMPLE_NOW,
        }
      : adapter,
  );
  const cleanIngestion = {
    ...makeSourceIngestionResult(targetId),
    itemsFetched: 20,
    detailItemsFailed: 0,
  };
  return {
    ...data,
    sourceAdapters: adapters,
    lastIngestionResults: [
      cleanIngestion,
      ...data.lastIngestionResults.filter((result) => result.adapterId !== targetId),
    ],
  };
}

/** Realistic Polish + English SOURCE content — never untranslated UI literals. */
function applyMixedLocale(data: ScenarioData): ScenarioData {
  const plItem = {
    ...makeFeedItem(MIXED_SPEC, 0),
    id: "feed_overlay_mixed_pl",
    title: "PKN Orlen ogłasza wyniki za trzeci kwartał — przychody wzrosły o 12%",
    summary: "Zarząd podtrzymuje prognozę wyników na 2026 rok.",
    language: "pl",
  };
  const enItem = {
    ...makeFeedItem(MIXED_SPEC, 1),
    id: "feed_overlay_mixed_en",
    title: "PKN Orlen reports Q3 results — revenue up 12% year over year",
    summary: "Management reaffirms full-year 2026 guidance.",
    language: "en",
  };
  return { ...data, feedItems: [plItem, enItem, ...data.feedItems] };
}

// Persistent-toast overflow cap regression (bug: 19+ unseen attention events
// piled up unbounded persistent toasts, covering the sidebar nav — Toast.tsx
// PERSISTENT_VISIBLE_CAP). 20 unseen events reused across the base scenario's
// companies (falling back to a fixed id when there are none) — Playwright's
// only lever into "many unseen attention events" without a dedicated bridge
// method, mirroring `applyDenseHistory`'s "materializes only when selected".
const ATTENTION_OVERFLOW_COUNT = 20;

function applyAttentionOverflow(data: ScenarioData): ScenarioData {
  const ruleId = data.alertRules[0]?.id ?? "alert_rule_sample_1";
  const companyIds = data.companies.length > 0 ? data.companies.map((c) => c.id) : ["company_overlay_missing"];
  const extra = Array.from({ length: ATTENTION_OVERFLOW_COUNT }, (_, index) => ({
    ...makeAttentionEvent(
      `attn_overlay_overflow_${index}`,
      ruleId,
      companyIds[index % companyIds.length],
    ),
    firedAt: SAMPLE_NOW,
  }));
  return { ...data, attentionEvents: [...extra, ...data.attentionEvents] };
}

// Toast policy v2 (ADR 0087 dec. 3): a persistent toast is reserved for `urgent`
// events only. These two overlays exercise the SELECTIVE side of the cap.
const MIXED_URGENT_COUNT = 5;
const MIXED_NOTABLE_COUNT = 15;
const NOTABLE_ONLY_COUNT = 12;

/**
 * ~20 unseen events of which only SOME (5) are `urgent`; the rest are `notable`.
 * Only the urgent ones may raise persistent toasts, so the persistent stack caps
 * at 3 visible + a "+N more" summary counting ONLY the urgent overflow — never
 * the notable events (which raise transient toasts at most).
 *
 * OWNS the attention set (REPLACES, not appends): the whole point is an exact
 * severity MIX, so a base-scenario urgent event must not perturb the persistent
 * count. This is the documented replace-exception (cf. `partial-data`).
 */
function applyAttentionMixedSeverity(data: ScenarioData): ScenarioData {
  const ruleId = data.alertRules[0]?.id ?? "alert_rule_sample_1";
  const companyIds = data.companies.length > 0 ? data.companies.map((c) => c.id) : ["company_overlay_missing"];
  const urgent = Array.from({ length: MIXED_URGENT_COUNT }, (_, i) => ({
    ...makeAttentionEvent(`attn_overlay_mixed_urgent_${i}`, ruleId, companyIds[i % companyIds.length], "urgent" as const),
    firedAt: SAMPLE_NOW,
  }));
  const notable = Array.from({ length: MIXED_NOTABLE_COUNT }, (_, i) => ({
    ...makeAttentionEvent(`attn_overlay_mixed_notable_${i}`, ruleId, companyIds[i % companyIds.length], "notable" as const),
    evidenceRef: `signal_overlay_mixed_notable_${i}`,
    firedAt: SAMPLE_NOW,
  }));
  return { ...data, attentionEvents: [...urgent, ...notable] };
}

/**
 * Many unseen events, NONE `urgent` (all `notable`). A persistent toast must
 * never appear — the stream is the system of record, and only urgent events are
 * allowed to interrupt (ADR 0087 dec. 3). OWNS the attention set (REPLACES): a
 * base-scenario urgent event would defeat the "zero urgent" premise.
 */
function applyAttentionNotableOnly(data: ScenarioData): ScenarioData {
  const ruleId = data.alertRules[0]?.id ?? "alert_rule_sample_1";
  const companyIds = data.companies.length > 0 ? data.companies.map((c) => c.id) : ["company_overlay_missing"];
  const notable = Array.from({ length: NOTABLE_ONLY_COUNT }, (_, i) => ({
    ...makeAttentionEvent(`attn_overlay_notable_only_${i}`, ruleId, companyIds[i % companyIds.length], "notable" as const),
    evidenceRef: `signal_overlay_notable_only_${i}`,
    firedAt: SAMPLE_NOW,
  }));
  return { ...data, attentionEvents: [...notable] };
}

// Ids that exist in the browser-smoke `companies` projection, so each urgent /
// group row's Review opens a REAL company workspace during the J1 journey. The
// aggregate companies are the synthetic `T0x` smoke tickers — never interacted
// with, only collapsed into the "×N companies" routine aggregate.
const MR_INSIDER_COMPANY = "company_gpw_pkn";
const MR_RECON_COMPANY = "company_gpw_kgh";
const MR_GROUP_COMPANY = "company_gpw_pzu";
const MR_AGGREGATE_COMPANIES = [
  "company_gpw_t01",
  "company_gpw_t02",
  "company_gpw_t03",
  "company_gpw_t04",
  "company_gpw_t05",
  "company_gpw_t06",
];

/** One routine (`succeeded` → routine severity) autopilot run for a company. */
function makeRoutineRun(id: string, companyId: string): ScenarioData["autopilotRuns"][number] {
  return {
    id,
    companyId,
    reportDocumentId: `doc_${id}`,
    trigger: "scheduled",
    sweepId: null,
    mode: "autopilot",
    status: "succeeded",
    stage: "notify",
    summaryText: "New report processed.",
    kpiDeltaJson: null,
    reportDiffRef: null,
    crossRefsJson: null,
    producedFactIds: [],
    notificationState: "unread",
    lastError: null,
    // `succeeded` → routine (product-spec §Attention Routing / `storage::severity`).
    severity: "routine",
    reportDocumentTitle: "Skonsolidowany raport kwartalny Q2 2026",
    createdAt: SAMPLE_NOW,
    updatedAt: SAMPLE_NOW,
  };
}

/**
 * The J1 morning-review seed (ADR 0087, docs/plans/v0.60-today-redesign.md §1):
 * exactly 10 new overnight items so the redesigned Today's grouped, severity-
 * ranked stream can be exercised end-to-end at the acceptance-bar item count.
 *
 *   - 2 URGENT: an insider transaction (PKN) + a missed-report reconciliation
 *     (KGH, a system event with no user rule) — the rows that must LEAD the
 *     stream and raise the (only) persistent toasts.
 *   - a 2-member NOTABLE group (PZU, two same-category fired alerts on distinct
 *     evidence) — the "repeats collapse into one ×N row" case.
 *   - 6 ROUTINE autopilot runs across distinct companies — more than the
 *     aggregate threshold, so they fold into one "×N companies" routine row.
 *
 * Additive (the store-mutation contract): every entity carries a fixed
 * `*_mr_*` id and is prepended, never replacing base-scenario rows.
 */
function applyMorningReview(data: ScenarioData): ScenarioData {
  const ruleId = data.alertRules[0]?.id ?? "alert_rule_sample_1";

  // A genuine insider-transaction rule so the insider event's CAUSE
  // (`insider_transaction`) is distinct from the base sample event's
  // `profit_warning` — otherwise the two urgent same-cause rows would collapse
  // into one cross-company aggregate (ADR 0087 amendment 2026-07-23) and the
  // canonical morning-review scene would lose its distinct leading insider row.
  const insiderRule = {
    ...makeAlertRule("alert_rule_mr_insider", "signal_category", MR_INSIDER_COMPANY),
    signalCategory: "insider_transaction" as const,
  };
  const insider = {
    ...makeAttentionEvent("attn_mr_insider", insiderRule.id, MR_INSIDER_COMPANY, "urgent"),
    evidenceRef: "signal_mr_insider",
    firedAt: SAMPLE_NOW,
  };
  const reconciliation = {
    ...makeAttentionEvent("attn_mr_recon", ruleId, MR_RECON_COMPANY, "urgent"),
    ruleId: null,
    triggerType: "source_reconciliation" as const,
    evidenceType: "source_reconciliation" as const,
    evidenceRef: "recon_mr_1",
    // The concrete missed report + the source that missed it (v0.60 D6): the J1
    // scene must state WHICH report and WHICH source, not a bare category.
    evidenceTitle: "Raport bieżący 15/2026 — zawarcie znaczącej umowy",
    evidenceDetail: "GPW ESPI/EBI",
    firedAt: SAMPLE_NOW,
  };
  // Two notable fired alerts for ONE company, distinct evidence → a ×2 group.
  const group = [0, 1].map((i) => ({
    ...makeAttentionEvent(`attn_mr_group_${i}`, ruleId, MR_GROUP_COMPANY, "notable" as const),
    evidenceRef: `signal_mr_group_${i}`,
    // Distinct fired times so the group orders its members newest-first.
    firedAt: `2026-07-2${i}T09:00:00Z`,
  }));

  const runs = MR_AGGREGATE_COMPANIES.map((companyId, i) => makeRoutineRun(`run_mr_${i}`, companyId));

  return {
    ...data,
    alertRules: [insiderRule, ...data.alertRules],
    attentionEvents: [insider, reconciliation, ...group, ...data.attentionEvents],
    autopilotRuns: [...runs, ...data.autopilotRuns],
  };
}

/**
 * A DENSE Today (ADR 0087 dec. 1, docs/plans/v0.60-today-redesign.md §11 Dense
 * row): one routine autopilot run for EVERY company in the store (28 in the
 * browser projection). Well over the aggregate threshold, so the whole wall
 * folds into a single "×N companies" routine aggregate row — the stream must
 * stay a screenful, never a page-overflowing wall.
 */
function applyTodayDense(data: ScenarioData): ScenarioData {
  const runs = data.companies.map((company, i) => makeRoutineRun(`run_dense_${i}`, company.id));
  return { ...data, autopilotRuns: [...runs, ...data.autopilotRuns] };
}

// UI dogfooding finding ⇒ overlay (docs/testing.md standing rule; owner
// dogfooding 2026-07-23). The two data states the owner's real database exposed
// on Today. Each carries a fixed `*_overlay_orphan_*` / `*_overlay_pruned_*` id
// and is additive (prepended), never replacing base rows.

// One fixed CompanySpec per orphan row so simultaneous overlays never collide.
const ORPHAN_SPECS: CompanySpec[] = [
  { key: "orphanA", ticker: "ZZO1", name: "Orphan Evidence A S.A.", sector: "Technology" },
  { key: "orphanB", ticker: "ZZO2", name: "Orphan Evidence B S.A.", sector: "Energy" },
  { key: "orphanC", ticker: "ZZO3", name: "Orphan Evidence C S.A.", sector: "Financials" },
];
const PRUNED_CLEAN_SPEC: CompanySpec = { key: "prunedClean", ticker: "ZZP1", name: "Pruned Feed Clean S.A.", sector: "Consumer Staples" };
const PRUNED_GLUED_SPEC: CompanySpec = { key: "prunedGlued", ticker: "ZZP2", name: "Pruned Feed Glued S.A.", sector: "Consumer Staples" };

// A clean human snapshot title — survives the pruned feed row and renders verbatim.
export const PRUNED_CLEAN_SNAPSHOT = "Skonsolidowany raport kwartalny za III kwartał 2026 r.";
// The owner's real glued case: a filename fused onto the human title. The row must
// split it — the human part is the statement, the filename drops to a link line —
// so a document extension NEVER lands in a rendered row statement.
export const PRUNED_GLUED_SNAPSHOT = "Y24_25_Sprawozdanie jednostkowe.xhtmlJednostkowe Sprawozdanie Finansowe AB S.A.";
export const PRUNED_GLUED_HUMAN = "Jednostkowe Sprawozdanie Finansowe AB S.A.";
export const PRUNED_GLUED_FILENAME = "Y24_25_Sprawozdanie jednostkowe.xhtml";

/**
 * Orphaned attention evidence (owner dogfooding 2026-07-23: 27 real orphans). A
 * signal-triggered attention event whose `evidenceTitle` is null AND whose rule +
 * signal rows are GONE (a cascade-pruned rule id that no longer resolves): the FE
 * must render the category fallback, never a blank statement or a crash. A
 * REPRESENTATIVE few (the real count is immaterial; the null-title + dangling-rule
 * STATE is what regresses) across distinct companies so each stays its own
 * `notable` row (below the notable cross-company fold threshold), not an aggregate.
 */
function applyOrphanedEvidence(data: ScenarioData): ScenarioData {
  const companies = ORPHAN_SPECS.map(makeCompany);
  const events = ORPHAN_SPECS.map((spec, i) => ({
    ...makeAttentionEvent(
      `attn_overlay_orphan_${i}`,
      // A dangling rule id — no matching alertRules row, so the rule lookup misses
      // and the title composer falls back to the trigger category, never crashes.
      "alert_rule_orphan_pruned_missing",
      makeCompany(spec).id,
      "notable" as const,
    ),
    // The orphan STATE: no snapshot title survived, and the signal it cited is gone.
    evidenceTitle: null,
    evidenceDetail: null,
    evidenceRef: `signal_orphan_pruned_gone_${i}`,
    firedAt: SAMPLE_NOW,
  }));
  return {
    ...data,
    companies: [...companies, ...data.companies],
    attentionEvents: [...events, ...data.attentionEvents],
  };
}

/**
 * Pruned feed with a surviving snapshot title (owner dogfooding 2026-07-23). An
 * attention event whose `evidenceTitle` was snapshotted at fire time but whose
 * live feed item was later pruned (`evidenceRef` no longer resolves): the row must
 * render the SNAPSHOT, not blank. Two rows on distinct companies: a CLEAN human
 * snapshot (renders verbatim) and a GLUED filename+title snapshot (the row splits
 * it so the extension never lands in the statement — the anti-filename gate's
 * adversarial case). `notable` so both stay their own rows.
 */
function applyPrunedFeed(data: ScenarioData): ScenarioData {
  const cleanCompany = makeCompany(PRUNED_CLEAN_SPEC);
  const gluedCompany = makeCompany(PRUNED_GLUED_SPEC);
  const ruleId = data.alertRules[0]?.id ?? "alert_rule_sample_1";
  const clean = {
    ...makeAttentionEvent("attn_overlay_pruned_clean", ruleId, cleanCompany.id, "notable" as const),
    evidenceTitle: PRUNED_CLEAN_SNAPSHOT,
    evidenceDetail: null,
    evidenceRef: "feed_overlay_pruned_clean_gone",
    firedAt: SAMPLE_NOW,
  };
  const glued = {
    ...makeAttentionEvent("attn_overlay_pruned_glued", ruleId, gluedCompany.id, "notable" as const),
    evidenceTitle: PRUNED_GLUED_SNAPSHOT,
    evidenceDetail: null,
    evidenceRef: "feed_overlay_pruned_glued_gone",
    firedAt: SAMPLE_NOW,
  };
  return {
    ...data,
    companies: [cleanCompany, gluedCompany, ...data.companies],
    attentionEvents: [clean, glued, ...data.attentionEvents],
  };
}

// Poor-state seeds (epic #40 S2, ADR 0091). The two states a failure-path walk
// needs: the automation FAILED overnight, and a source is DEGRADED. Both are
// additive with fixed `*_overlay_*` ids so they compose with every existing
// overlay, and both seed a COHERENT set of rows — the same triple/tuple the real
// read models produce — so a walk asserts on the app's rendering, never on a
// state the backend could never emit.

const FAILED_RUN_SPEC: CompanySpec = {
  key: "failedRun",
  ticker: "ZZZA",
  name: "Failed Autopilot Test S.A.",
  sector: "Technology",
};

/** The processed report's document title (`report_documents.title`, joined at read). */
export const FAILED_RUN_REPORT_TITLE =
  "Skonsolidowany raport roczny 2026 — Failed Autopilot Test S.A.";
/** The run's raw `last_error` — a concrete cause, never a bare "failed". */
export const FAILED_RUN_LAST_ERROR =
  "stage extract failed: report parser returned no periods (stored document unreadable)";

/**
 * A coherent FAILED autopilot run (epic #40 S2). The three rows a real terminal
 * failure leaves behind, consistent with the backend read models:
 *
 *   - the run: `status: "failed"` + a concrete `lastError` + `severity: "notable"`
 *     (`storage::severity::severity_for_autopilot_run` — `failed`/`partial` are
 *     notable, never routine), still `unread` so Today reads it;
 *   - its completion attention event: `autopilot_run_completed` (also `notable`,
 *     `severity_for_trigger`), `evidenceType: "autopilot_run"` pointing at the run,
 *     `evidenceTitle` = the report's document title and `evidenceDetail` = the run's
 *     RAW status (`attention.rs` read model — the frontend translates it, so the row
 *     states "<report> — Failed");
 *   - the user rule it fired from: an `autopilot_run_completed` event is never a
 *     system event (`evaluate_autopilot_completion` fires enabled rules only), so a
 *     matching enabled rule must exist or the event would be unreachable.
 *
 * The failure must be NAMED on Today — a failed overnight run that reads like a
 * quiet morning is the defect class ADR 0091 exists to catch.
 */
function applyFailedAutopilotRun(data: ScenarioData): ScenarioData {
  const company = makeCompany(FAILED_RUN_SPEC);
  const rule = makeAlertRule("alert_rule_overlay_failed_run", "autopilot_run_completed", company.id);
  const run: ScenarioData["autopilotRuns"][number] = {
    id: "run_overlay_failed_1",
    companyId: company.id,
    reportDocumentId: "doc_overlay_failed_1",
    trigger: "scheduled",
    sweepId: null,
    mode: "autopilot",
    status: "failed",
    // The stage it died in — the run stopped short of `notify`.
    stage: "extract",
    summaryText: null,
    kpiDeltaJson: null,
    reportDiffRef: null,
    crossRefsJson: null,
    producedFactIds: [],
    notificationState: "unread",
    lastError: FAILED_RUN_LAST_ERROR,
    severity: "notable",
    reportDocumentTitle: FAILED_RUN_REPORT_TITLE,
    createdAt: SAMPLE_NOW,
    updatedAt: SAMPLE_NOW,
  };
  const event = {
    ...makeAttentionEvent("attn_overlay_failed_run", rule.id, company.id, "notable" as const),
    triggerType: "autopilot_run_completed" as const,
    evidenceType: "autopilot_run" as const,
    evidenceRef: run.id,
    evidenceTitle: FAILED_RUN_REPORT_TITLE,
    // For an autopilot event `evidenceDetail` carries the run's raw status.
    evidenceDetail: run.status,
    firedAt: SAMPLE_NOW,
  };
  return {
    ...data,
    companies: [company, ...data.companies],
    alertRules: [rule, ...data.alertRules],
    autopilotRuns: [run, ...data.autopilotRuns],
    attentionEvents: [event, ...data.attentionEvents],
  };
}

/** The degraded PRIMARY source — the one a triage flow must land on first. */
export const DEGRADED_REPORTS_SOURCE_NAME = "ZZZ Official Reports Feed";
export const DEGRADED_REPORTS_LAST_ERROR =
  "HTTP 503 Service Unavailable from https://example.test/zzz/espi";
/** The only concrete failure string the Sources screen renders today (detail panel). */
export const DEGRADED_REPORTS_DETAIL_WARNING =
  "3 of 12 report details could not be fetched (HTTP 503 from the publisher)";
export const DEGRADED_MEDIA_LAST_ERROR = "RSS feed returned HTTP 429 (rate limited)";

/**
 * Degraded source adapters (epic #40 S2): two adapters whose last fetch FAILED —
 * `lastError` + `lastErrorAt` set and `healthStatus: "attention"`, the pairing the
 * backend produces (`record_source_adapter_error`). Additive synthetic adapters
 * (fixed `zzz-degraded-*` ids), never a rewrite of a base adapter, so this never
 * fights `conflicting-statuses` (which owns `bankier-company-komunikaty`) and works
 * on every base — `minimal` seeds a different adapter set than `rich`.
 *
 * The reports adapter is PREPENDED first because the triage entry point
 * (`openSourceStatus`) selects the first adapter carrying a `lastError`; it also
 * carries the failed-detail counters + `lastDetailWarning`, the concrete failure
 * text the Sources detail panel actually renders.
 */
function applyDegradedSources(data: ScenarioData): ScenarioData {
  const reports = {
    ...makeSourceAdapter({
      id: "zzz-degraded-official-reports",
      displayName: DEGRADED_REPORTS_SOURCE_NAME,
      sourceType: "official_report",
      fetchMode: "public_json",
      visibility: "optional",
      userConfigurable: true,
      healthStatus: "attention",
      enabled: true,
      sourceUrl: "https://example.test/zzz/espi",
      markets: ["GPW"],
    }),
    lastError: DEGRADED_REPORTS_LAST_ERROR,
    lastErrorAt: SAMPLE_NOW,
    // The listing succeeded, the detail fetches did not — a partially degraded
    // run, the realistic shape (a total outage would fetch nothing at all).
    lastItemsFetched: 12,
    lastItemsCreated: 12,
    lastItemsMatched: 9,
    lastItemsUnmatched: 3,
    lastDetailItemsAttempted: 12,
    lastDetailItemsStored: 9,
    lastDetailItemsFailed: 3,
    lastDetailWarning: DEGRADED_REPORTS_DETAIL_WARNING,
  };
  const media = {
    ...makeSourceAdapter({
      id: "zzz-degraded-media-rss",
      displayName: "ZZZ Market Media RSS",
      sourceType: "public_media",
      fetchMode: "rss",
      visibility: "optional",
      userConfigurable: true,
      healthStatus: "attention",
      enabled: true,
      sourceUrl: "https://example.test/zzz/rss.xml",
      markets: ["GPW"],
    }),
    lastError: DEGRADED_MEDIA_LAST_ERROR,
    lastErrorAt: SAMPLE_NOW,
    // Rate-limited before anything was read: nothing fetched at all.
    lastItemsFetched: 0,
    lastItemsCreated: 0,
    lastItemsMatched: 0,
    lastItemsUnmatched: 0,
  };
  return { ...data, sourceAdapters: [reports, media, ...data.sourceAdapters] };
}

const JOB_FAILED_SPEC: CompanySpec = {
  key: "jobFailed",
  ticker: "ZZZJ",
  name: "Failed Job Test S.A.",
  sector: "Energy",
};

/** The company-scoped failure's subject — the report the extraction died on. */
export const JOB_FAILED_SUBJECT = "Skonsolidowany raport kwartalny Q3 2026 — Failed Job Test S.A.";
/** The kind of the company-scoped failed job (raw backend token). */
export const JOB_FAILED_KIND = "ownership_extraction";
/** The workspace-wide failure's statement — a job with no subject falls back to
 * the queue's own `last_error`, which the read model then supplies. */
export const JOB_FAILED_SYSTEM_ERROR =
  "HTTP 503 from biznesradar.pl (retries exhausted after 3 attempts)";
/** The kind of the workspace-wide failed job (raw backend token). */
export const JOB_FAILED_SYSTEM_KIND = "aggregator_fundamentals_pull";

/**
 * Terminally failed background jobs (epic #40 S3, ADR 0091 dec. 1) — the generic
 * failure surface, in BOTH scopes the backend can emit:
 *
 *   - company-scoped: the handler named the company (`company_scope`) and the
 *     report it died on (`failure_subject`), so the row reads
 *     "<task> failed — <report>" under the company's ticker;
 *   - workspace-wide: `companyId: null` (nullable since migration 0118) with no
 *     subject, so the read model states the job's own `last_error` instead.
 *
 * Both are SYSTEM events: `ruleId: null` (no user rule can fire a job failure),
 * `evidenceType: "job"` pointing at the `job_queue` row, `evidenceDetail` = the
 * RAW job kind the frontend translates, and `notable` for every kind. A background
 * job that dies in silence is the exact defect class this epic exists to catch.
 */
function applyJobFailedEvent(data: ScenarioData): ScenarioData {
  const company = makeCompany(JOB_FAILED_SPEC);
  const scoped = {
    ...makeAttentionEvent("attn_overlay_job_failed", "", company.id, "notable" as const),
    ruleId: null,
    triggerType: "job_failed" as const,
    evidenceType: "job" as const,
    evidenceRef: "job_overlay_failed_1",
    evidenceTitle: JOB_FAILED_SUBJECT,
    evidenceDetail: JOB_FAILED_KIND,
    firedAt: SAMPLE_NOW,
  };
  const systemWide = {
    ...scoped,
    id: "attn_overlay_job_failed_system",
    companyId: null,
    evidenceRef: "job_overlay_failed_2",
    evidenceTitle: JOB_FAILED_SYSTEM_ERROR,
    evidenceDetail: JOB_FAILED_SYSTEM_KIND,
  };
  return {
    ...data,
    companies: [company, ...data.companies],
    attentionEvents: [scoped, systemWide, ...data.attentionEvents],
  };
}

const OVERLAYS: Record<ScenarioOverlayName, (data: ScenarioData) => ScenarioData> = {
  "partial-data": applyPartialData,
  "preliminary-fundamentals": applyPreliminaryFundamentals,
  "stale-processing": applyStaleProcessing,
  "conflicting-statuses": applyConflictingStatuses,
  "hostile-content": applyHostileContent,
  "dense-history": applyDenseHistory,
  "mixed-locale": applyMixedLocale,
  "attention-overflow": applyAttentionOverflow,
  "attention-mixed-severity": applyAttentionMixedSeverity,
  "attention-notable-only": applyAttentionNotableOnly,
  "morning-review": applyMorningReview,
  "today-dense": applyTodayDense,
  "orphaned-evidence": applyOrphanedEvidence,
  "pruned-feed": applyPrunedFeed,
  "degraded-sources": applyDegradedSources,
  "failed-autopilot-run": applyFailedAutopilotRun,
  "job-failed-event": applyJobFailedEvent,
};

/**
 * Fixed application order — independent of the order the caller supplies.
 * Exported so every enumeration of the overlay set (unit tests, docs checks)
 * reads the ONE list instead of hand-copying it (the drift class epic #40 S1
 * killed for `ScenarioOverlayName`).
 */
export const SCENARIO_OVERLAY_NAMES: readonly ScenarioOverlayName[] = [
  "partial-data",
  "preliminary-fundamentals",
  "stale-processing",
  "conflicting-statuses",
  "hostile-content",
  "dense-history",
  "mixed-locale",
  "attention-overflow",
  "attention-mixed-severity",
  "attention-notable-only",
  "morning-review",
  "today-dense",
  "orphaned-evidence",
  "pruned-feed",
  // The poor-state seeds apply LAST: both are additive, and `attention-mixed-
  // severity` / `attention-notable-only` REPLACE the attention set, so a
  // failed-run event seeded before them would be silently dropped whenever they
  // compose. Applying last makes every combination keep the poor state.
  "degraded-sources",
  "failed-autopilot-run",
  // Same reasoning as the two above: the job-failure seeds are additive and must
  // survive whichever attention overlay composed before them.
  "job-failed-event",
];

/**
 * Apply the named overlays to `data` in the fixed canonical order, regardless
 * of the order `overlays` lists them. A repeated name is idempotent (applied
 * once). Pure: `data` itself is never mutated.
 *
 * An overlay name missing from `SCENARIO_OVERLAY_NAMES` THROWS: a registered
 * overlay left out of the order list would otherwise be silently skipped, and a
 * spec asking for it would fail far from the cause (epic #40 S2 hit exactly this
 * while the seeds were still being written).
 */
export function applyScenarioOverlays(
  data: ScenarioData,
  overlays: readonly ScenarioOverlayName[],
): ScenarioData {
  const requested = new Set(overlays);
  for (const name of requested) {
    if (!SCENARIO_OVERLAY_NAMES.includes(name)) {
      throw new Error(`Unknown scenario overlay: ${name} (missing from SCENARIO_OVERLAY_NAMES)`);
    }
  }
  let next = data;
  for (const name of SCENARIO_OVERLAY_NAMES) {
    if (requested.has(name)) {
      next = OVERLAYS[name](next);
    }
  }
  return next;
}
