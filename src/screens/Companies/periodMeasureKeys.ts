import type { KpiComparison } from "../../api/generated/KpiComparison";
import type { FinancialPeriod } from "../../api/financialsTypes";
import type { FactMatrixRow } from "./factMatrix";

// Presentation signatures for `useVisiblePeriods`'s measuring pass: every
// input that can change a period cell's rendered width is in the key, so a
// value edit, a late provenance chip or a flag change re-measures.

export function factsMeasureKey(input: {
  locale: string;
  periods: readonly FinancialPeriod[];
  rows: readonly FactMatrixRow[];
  latestPeriodId: string | null;
  originTier: string | null;
  originIsMixed: boolean;
}): string {
  const periods = input.periods.map((period) => `${period.id}:${period.fiscalYear}${period.periodType}`).join(",");
  const rows = input.rows
    .map((row) => {
      const cells = input.periods
        .map((period) => {
          const fact = row.cells[period.id];
          if (!fact) return "-";
          return [
            fact.valueNumeric,
            fact.asReportedValue ?? "",
            fact.asReportedScale ?? "",
            fact.currency ?? "",
            fact.dataQuality,
            fact.annotation ? "a" : "",
          ].join("~");
        })
        .join("|");
      return `${row.definition.id}:${row.definition.valueKind}:${row.definition.unit ?? ""}:${row.definition.metricKey}=${cells}`;
    })
    .join(";");
  return `${input.locale}|${periods}|${input.latestPeriodId ?? ""}|${input.originTier ?? ""}|${input.originIsMixed ? "mixed" : ""}|${rows}`;
}

export function periodsMeasureKey(input: {
  locale: string;
  granularity: string;
  comparison: KpiComparison | null;
}): string {
  if (!input.comparison) return `${input.locale}|${input.granularity}|`;
  const axis = input.comparison.axis.map((period) => period.key).join(",");
  const series = input.comparison.series
    .map(
      (row) =>
        `${row.metricKey}:${row.valueKind}=${row.cells
          .map((cell) => [cell.value ?? "", cell.currency ?? "", cell.deltaQoQ ?? "", cell.deltaYoY ?? "", cell.flags.join("+")].join("~"))
          .join("|")}`,
    )
    .join(";");
  return `${input.locale}|${input.granularity}|${axis}|${series}`;
}
