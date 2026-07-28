import { callCommand } from "./tauri";
import type { ExtractionOutcome } from "./generated/ExtractionOutcome";
import type { ExtractReportDocumentDataInput } from "./generated/ExtractReportDocumentDataInput";
import type { FactProvenance } from "./generated/FactProvenance";
import type { RerunExtractionOutcomeInput } from "./generated/RerunExtractionOutcomeInput";
import type { StructuredExtractionSummary } from "./generated/StructuredExtractionSummary";

// GENERATED types from src-tauri (ADR 0061 / ts-rs). The structured-first
// fundamentals pipeline: run deterministic extraction and read per-fact
// provenance (source tier + validation verdict) for the KPI badges.
export type { ExtractionOutcome } from "./generated/ExtractionOutcome";
export type { ExtractReportDocumentDataInput } from "./generated/ExtractReportDocumentDataInput";
export type { FactProvenance } from "./generated/FactProvenance";
export type { RerunExtractionOutcomeInput } from "./generated/RerunExtractionOutcomeInput";
export type { RunStructuredExtractionInput } from "./generated/RunStructuredExtractionInput";
export type { StructuredExtractionSummary } from "./generated/StructuredExtractionSummary";

// NOTE: `run_structured_extraction` and `list_flagged_fact_provenance` are
// headless-only commands (autopilot flow / MCP port — contracts.md); they have
// no frontend wrapper on purpose (issues #153/#131 — orphaned exports).

// The reachable, one-click "Extract data" action over a single stored report
// document (ADR 0061 S5). The period is derived server-side (no Inbox round-trip
// and the UI never invents the reporting period); confirmation semantics are
// unchanged from the pipeline.
export function extractReportDocumentData(input: ExtractReportDocumentDataInput) {
  return callCommand<StructuredExtractionSummary>("extract_report_document_data", {
    input,
  });
}

export function listFactProvenance(factIds: string[]) {
  return callCommand<FactProvenance[]>("list_fact_provenance", {
    input: { factIds },
  });
}

// The company's NON-EMITTING extraction outcomes, newest attempt first (ADR 0061
// decision 2, ADR 0084 decision 4/6): the periods where the deterministic
// pipeline ran and refused to emit. Its sibling `list_flagged_fact_provenance`
// lists flagged FACTS, which by construction only exist where something WAS
// emitted — this read covers the periods where nothing was. Absence of a row
// means "never attempted", so a flagged period is never indistinguishable from
// an untouched one.
export function listFlaggedExtractionOutcomes(companyId: string) {
  return callCommand<ExtractionOutcome[]>("list_flagged_extraction_outcomes", {
    input: { companyId },
  });
}

// The "try again" action on a flagged period. Company/document/period come from
// the stored outcome row, so the retry can never target a different slot than
// the one displayed; the re-run updates that same row in place (attemptCount
// increments), so a fixed period leaves the flagged list instead of leaving a
// stale flag beside a fresh success.
export function rerunExtractionOutcome(input: RerunExtractionOutcomeInput) {
  return callCommand<StructuredExtractionSummary>("rerun_extraction_outcome", {
    input,
  });
}
