import { callCommand } from "./tauri";
import type { FactProvenance } from "./generated/FactProvenance";
import type { RunStructuredExtractionInput } from "./generated/RunStructuredExtractionInput";
import type { StructuredExtractionSummary } from "./generated/StructuredExtractionSummary";

// GENERATED types from src-tauri (ADR 0061 / ts-rs). The structured-first
// fundamentals pipeline: run deterministic extraction and read per-fact
// provenance (source tier + validation verdict) for the KPI badges.
export type { FactProvenance } from "./generated/FactProvenance";
export type { RunStructuredExtractionInput } from "./generated/RunStructuredExtractionInput";
export type { StructuredExtractionSummary } from "./generated/StructuredExtractionSummary";

export function runStructuredExtraction(input: RunStructuredExtractionInput) {
  return callCommand<StructuredExtractionSummary>("run_structured_extraction", {
    input,
  });
}

export function listFactProvenance(factIds: string[]) {
  return callCommand<FactProvenance[]>("list_fact_provenance", {
    input: { factIds },
  });
}

export function listFlaggedFactProvenance() {
  return callCommand<FactProvenance[]>("list_flagged_fact_provenance");
}
