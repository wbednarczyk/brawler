import { callCommand } from "./tauri";
import type { ComparativeValuation } from "./generated/ComparativeValuation";

// GENERATED DTOs from src-tauri/src/valuation/mod.rs + storage/valuation_runs.rs
// via ts-rs (ADR 0048). Comparative valuation L1 (ADR 0089 dec. 4-5;
// contracts.md § Cross-Company Comparison + Comparative Valuation). Peer-multiple
// implied fair-value ranges (P/E, EV/EBITDA, P/BV × peer median), a
// method-convergence spread, and a deterministic confidence grade whose
// components are inspectable. Decision support only — no cheap/expensive or
// buy/sell language.
export type { ComparativeValuation } from "./generated/ComparativeValuation";
export type { ValuationMethodResult } from "./generated/ValuationMethodResult";
export type { ValuationMethod } from "./generated/ValuationMethod";
export type { MethodAbsentReason } from "./generated/MethodAbsentReason";
export type { ValuationEmptyReason } from "./generated/ValuationEmptyReason";
export type { ConvergenceSpread } from "./generated/ConvergenceSpread";
export type { ConfidenceGrade } from "./generated/ConfidenceGrade";
export type { ConfidenceGradeLetter } from "./generated/ConfidenceGradeLetter";
export type { StoredValuationRun } from "./generated/StoredValuationRun";

/// Compute the comparative valuation for one company and append a
/// `valuation_runs` row per method whose input signature changed (no row per
/// render). Offloaded to `spawn_blocking` on the Rust side; reads confirmed data
/// only. Decision support only.
export function computeComparativeValuation(companyId: string) {
  return callCommand<ComparativeValuation>("compute_comparative_valuation", {
    companyId,
  });
}
