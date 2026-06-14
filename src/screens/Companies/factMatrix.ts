import type { FinancialFact, FinancialPeriod, KpiDefinition } from "../../api/financialsTypes";

// The fundamentals panel presents facts as a KPI-row x period-column matrix:
// each KPI definition is a row, each reporting period a column, so a metric's
// trend across periods reads left-to-right. This is the shared, testable builder.

export type FactMatrixRow = {
  definition: KpiDefinition;
  // periodId -> fact for that KPI in that period (sparse).
  cells: Record<string, FinancialFact>;
};

export type FactMatrix = {
  periods: FinancialPeriod[];
  rows: FactMatrixRow[];
};

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

  const definitionIdsWithFacts = new Set(facts.map((fact) => fact.definitionId));
  const rows: FactMatrixRow[] = definitions
    .filter((definition) => definitionIdsWithFacts.has(definition.id))
    .map((definition) => {
      const cells: Record<string, FinancialFact> = {};
      for (const fact of facts) {
        if (fact.definitionId === definition.id) cells[fact.periodId] = fact;
      }
      return { definition, cells };
    });

  return { periods: sortedPeriods, rows };
}
