import { describe, expect, it } from "vitest";
import { factsMeasureKey, periodsMeasureKey } from "./periodMeasureKeys";
import type { FactMatrixRow } from "./factMatrix";
import type { FinancialPeriod } from "../../api/financialsTypes";
import type { KpiComparison } from "../../api/generated/KpiComparison";

// The measuring pass re-runs only when its key changes — so every input that
// can widen a period cell must be in the key (sol R3 blocker 1: a late
// provenance chip and a value edit kept a stale, too-large capacity).

const period = { id: "p1", fiscalYear: 2025, periodType: "annual" } as unknown as FinancialPeriod;
function row(valueNumeric: string, annotation: string | null = null): FactMatrixRow {
  return {
    definition: { id: "d1", valueKind: "monetary", unit: "PLN", metricKey: "revenue" },
    cells: { p1: { id: "f1", valueNumeric, annotation, dataQuality: "final", currency: "PLN" } },
    isSynthetic: false,
  } as unknown as FactMatrixRow;
}
const base = { locale: "pl", periods: [period], rows: [row("100")], latestPeriodId: "p1", originTier: null, originIsMixed: false };

describe("factsMeasureKey", () => {
  it("changes when a value, an annotation marker or the origin chip changes", () => {
    const key = factsMeasureKey(base);
    expect(factsMeasureKey(base)).toBe(key);
    expect(factsMeasureKey({ ...base, rows: [row("1093600000")] })).not.toBe(key);
    expect(factsMeasureKey({ ...base, rows: [row("100", "note")] })).not.toBe(key);
    expect(factsMeasureKey({ ...base, originTier: "aggregator" })).not.toBe(key);
    expect(factsMeasureKey({ ...base, originIsMixed: true })).not.toBe(key);
    expect(factsMeasureKey({ ...base, locale: "en" })).not.toBe(key);
  });
});

describe("periodsMeasureKey", () => {
  const comparison = {
    axis: [{ key: "2025:FY" }],
    series: [{ metricKey: "revenue", valueKind: "monetary", cells: [{ value: "100", currency: "PLN", deltaQoQ: null, deltaYoY: "+5", flags: [] }] }],
  } as unknown as KpiComparison;
  it("changes when a cell value, a delta or a flag changes", () => {
    const key = periodsMeasureKey({ locale: "pl", granularity: "annual", comparison });
    const edited = structuredClone(comparison) as KpiComparison;
    edited.series[0].cells[0].value = "1234000000000";
    expect(periodsMeasureKey({ locale: "pl", granularity: "annual", comparison: edited })).not.toBe(key);
    const shifted = structuredClone(comparison) as KpiComparison;
    shifted.series[0].cells[0].deltaYoY = "+1234.5";
    expect(periodsMeasureKey({ locale: "pl", granularity: "annual", comparison: shifted })).not.toBe(key);
    const flagged = structuredClone(comparison) as KpiComparison;
    (flagged.series[0].cells[0].flags as string[]).push("no_fact");
    expect(periodsMeasureKey({ locale: "pl", granularity: "annual", comparison: flagged })).not.toBe(key);
    expect(periodsMeasureKey({ locale: "pl", granularity: "quarterly", comparison })).not.toBe(key);
  });
});
