import type { FinancialFact, FinancialPeriod, KpiDefinition } from "../../api/financialsTypes";

// The fundamentals panel presents facts as a KPI-row x period-column matrix:
// each KPI definition is a row, each reporting period a column, so a metric's
// trend across periods reads left-to-right. This is the shared, testable builder.

export type FactMatrixRow = {
  definition: KpiDefinition;
  // periodId -> fact for that KPI in that period (sparse).
  cells: Record<string, FinancialFact>;
};

export type FactMatrixGroupKey =
  | "income"
  | "balance"
  | "cash_flow"
  | "per_share"
  | "company"
  | "other";

export type FactMatrixGroup = {
  key: FactMatrixGroupKey;
  rows: FactMatrixRow[];
};

export type FactMatrix = {
  periods: FinancialPeriod[];
  rows: FactMatrixRow[];
  // Rows grouped under a display group, in the fixed approved order, with
  // empty groups omitted (card #307). Derived from `rows` — always in sync.
  groups: FactMatrixGroup[];
};

// Display order (approved mockup, card #307): Rachunek wyników / Bilans /
// Przepływy pieniężne / Na akcję / KPI operacyjne spółki / Pozostałe. Labels
// are localized by the caller (FundamentalsPanel) — this module stays
// locale-free, carrying one stable key per group.
const GROUP_ORDER: FactMatrixGroupKey[] = [
  "income",
  "balance",
  "cash_flow",
  "per_share",
  "company",
  "other",
];

// Frontend display rule (card #307): a company-scoped definition (an agent's
// or the owner's own issuer-characteristic KPI) renders under "KPI operacyjne
// spółki" regardless of `statementGroup` — UNLESS it carries an explicit
// non-'other' `statementGroup` (future), in which case it sorts into that
// statement group like any other row. So `scope === 'company'` only wins when
// `statementGroup` is still at the backend's `other` default.
function groupKeyFor(row: FactMatrixRow): FactMatrixGroupKey {
  const { scope, statementGroup } = row.definition;
  const isDefaultOther = !statementGroup || statementGroup === "other";
  if (scope === "company" && isDefaultOther) return "company";
  switch (statementGroup) {
    case "income":
    case "balance":
    case "cash_flow":
    case "per_share":
      return statementGroup;
    default:
      return "other";
  }
}

// Groups matrix rows under their display group, in the fixed approved order,
// omitting any group with no rows (the mockup's "hide empty groups" rule).
// Row order within a group preserves `rows`' incoming order.
export function groupFactMatrixRows(rows: FactMatrixRow[]): FactMatrixGroup[] {
  const buckets = new Map<FactMatrixGroupKey, FactMatrixRow[]>();
  for (const row of rows) {
    const key = groupKeyFor(row);
    const bucket = buckets.get(key);
    if (bucket) bucket.push(row);
    else buckets.set(key, [row]);
  }
  return GROUP_ORDER.map((key) => ({ key, rows: buckets.get(key) ?? [] })).filter(
    (group) => group.rows.length > 0,
  );
}

const PERIOD_RANK: Record<string, string> = {
  q1: "03",
  q2: "06",
  h1: "06",
  q3: "09",
  q4: "12",
  // Full-year/annual covers the whole year; sort it after the quarters.
  annual: "13",
  fy: "13",
};

// Sortable key, oldest -> newest. Prefers the real period end date; otherwise
// derives one from fiscal year + period type so columns still order sensibly.
export function periodSortKey(period: FinancialPeriod): string {
  if (period.periodEndDate) return period.periodEndDate;
  const rank = PERIOD_RANK[period.periodType.toLowerCase()] ?? "13";
  return `${period.fiscalYear}-${rank}`;
}

export function buildFactMatrix(
  periods: FinancialPeriod[],
  facts: FinancialFact[],
  definitions: KpiDefinition[],
): FactMatrix {
  const sortedPeriods = [...periods].sort((a, b) =>
    periodSortKey(a).localeCompare(periodSortKey(b)),
  );

  const definitionsById = new Map(definitions.map((definition) => [definition.id, definition]));

  // Row order: known definitions (in their listed order) first, then any
  // definition ids that appear in facts but aren't loaded. Never drop a fact for
  // a missing definition — synthesize a minimal row so it stays visible.
  const definitionIdsWithFacts = new Set(facts.map((fact) => fact.definitionId));
  const orderedIds = [
    ...definitions.map((definition) => definition.id).filter((id) => definitionIdsWithFacts.has(id)),
    ...[...definitionIdsWithFacts].filter((id) => !definitionsById.has(id)),
  ];

  const rows: FactMatrixRow[] = orderedIds.map((definitionId) => {
    const definition = definitionsById.get(definitionId) ?? syntheticDefinition(definitionId);
    const cells: Record<string, FinancialFact> = {};
    // Final-preferred cell selection (ADR 0093 dec. 2): `preliminary` and
    // `final` facts coexist in the same slot (same period + definition,
    // distinguished only by `dataQuality` — part of the storage uniqueness
    // key), and facts arrive `created_at DESC`. A plain "last object in the
    // array wins" assignment would let a preliminary sibling shadow its final
    // sibling whenever the final fact (created later, so it sorts first) is
    // followed by the preliminary one. Mirrors the backend idiom
    // (`metric_history`/`stored_fact_set_filtered`, storage/financials.rs):
    // `rank` 0 = final, 1 = anything else; only a strictly lower rank may
    // replace the cell, so a final fact can never be shadowed regardless of
    // array order, while a preliminary-only fact (no final sibling) still
    // shows, with `dataQuality` exposed on the chosen fact for the caller.
    const cellRank: Record<string, number> = {};
    for (const fact of facts) {
      if (fact.definitionId !== definitionId) continue;
      const rank = fact.dataQuality === "final" ? 0 : 1;
      const existingRank = cellRank[fact.periodId];
      if (existingRank !== undefined && existingRank <= rank) continue;
      cells[fact.periodId] = fact;
      cellRank[fact.periodId] = rank;
    }
    return { definition, cells };
  });

  return { periods: sortedPeriods, rows, groups: groupFactMatrixRows(rows) };
}

// Placeholder definition for a fact whose KPI definition isn't loaded, so the
// value still renders (labelled by its metric id) instead of vanishing.
function syntheticDefinition(definitionId: string): KpiDefinition {
  const metricKey = definitionId.replace(/^kpidef_/, "");
  return {
    id: definitionId,
    scope: "unknown",
    companyId: null,
    sector: null,
    metricKey,
    label: metricKey.replace(/_/g, " ").replace(/\b\w/g, (c) => c.toUpperCase()),
    valueKind: "monetary",
    unit: null,
    computation: "reported",
    formula: null,
    displayFormat: null,
    origin: "user",
    statementGroup: "other",
    createdAt: "",
    updatedAt: "",
  };
}
