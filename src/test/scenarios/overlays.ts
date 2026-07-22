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
  makeAttentionEvent,
  makeCompany,
  makeFeedItem,
  makeFinancialPeriod,
  makeResearchEvidenceItem,
  makeSourceIngestionResult,
  type CompanySpec,
} from "./entities";
import type { ScenarioData } from "./scenarios";

export type ScenarioOverlayName =
  | "hostile-content"
  | "dense-history"
  | "partial-data"
  | "stale-processing"
  | "conflicting-statuses"
  | "mixed-locale"
  | "attention-overflow";

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

const OVERLAYS: Record<ScenarioOverlayName, (data: ScenarioData) => ScenarioData> = {
  "partial-data": applyPartialData,
  "stale-processing": applyStaleProcessing,
  "conflicting-statuses": applyConflictingStatuses,
  "hostile-content": applyHostileContent,
  "dense-history": applyDenseHistory,
  "mixed-locale": applyMixedLocale,
  "attention-overflow": applyAttentionOverflow,
};

/** Fixed application order — independent of the order the caller supplies. */
const OVERLAY_ORDER: readonly ScenarioOverlayName[] = [
  "partial-data",
  "stale-processing",
  "conflicting-statuses",
  "hostile-content",
  "dense-history",
  "mixed-locale",
  "attention-overflow",
];

/**
 * Apply the named overlays to `data` in the fixed canonical order, regardless
 * of the order `overlays` lists them. A repeated name is idempotent (applied
 * once). Pure: `data` itself is never mutated.
 */
export function applyScenarioOverlays(
  data: ScenarioData,
  overlays: readonly ScenarioOverlayName[],
): ScenarioData {
  const requested = new Set(overlays);
  let next = data;
  for (const name of OVERLAY_ORDER) {
    if (requested.has(name)) {
      next = OVERLAYS[name](next);
    }
  }
  return next;
}
