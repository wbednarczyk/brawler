import { callCommand } from "./tauri";
import type { KpiComparison } from "./generated/KpiComparison";
import type { KpiComparisonInput } from "./generated/KpiComparisonInput";

// GENERATED DTOs from src-tauri/src/commands/comparison.rs via ts-rs (ADR 0048).
// Cross-company comparison read model (ADR 0089 dec. 1; contracts.md
// § Cross-Company Comparison). `companyIds` of length 1 serves the Fundamentals
// periods×deltas table (same read model, N=1).
export type { KpiComparison } from "./generated/KpiComparison";
export type { KpiComparisonInput } from "./generated/KpiComparisonInput";
export type { KpiComparisonSeries } from "./generated/KpiComparisonSeries";
export type { KpiComparisonPeriod } from "./generated/KpiComparisonPeriod";
export type { KpiComparisonCell } from "./generated/KpiComparisonCell";
export type { CellFlag } from "./generated/CellFlag";

/// Aligned cross-company KPI comparison: a shared period axis plus one series per
/// `(company, metric)`, each cell carrying FX-converted value, provenance
/// evidence link, and server-side QoQ/YoY deltas. Offloaded to `spawn_blocking`
/// on the Rust side; reads confirmed facts only, never re-parses reports.
export function getKpiComparison(input: KpiComparisonInput) {
  return callCommand<KpiComparison>("get_kpi_comparison", { input });
}
