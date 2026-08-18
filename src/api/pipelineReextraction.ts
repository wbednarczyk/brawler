import { invoke } from "@tauri-apps/api/core";
import type { PipelineReextractionBatch } from "./generated/PipelineReextractionBatch";
import type { PipelineReextractionProgress } from "./generated/PipelineReextractionProgress";

export type { PipelineReextractionBatch } from "./generated/PipelineReextractionBatch";
export type { PipelineReextractionProgress } from "./generated/PipelineReextractionProgress";

// Start a version-aware re-extraction batch for a company (epic #398 Item B):
// re-arm the company's successful ESEF-tier runs whose stored pipeline
// version is stale, so a widened crosswalk/projection reaches already-landed
// filings. NOT gated on automation mode (it reprocesses already-stored
// documents on explicit request — the "Try again" posture). Poll
// getPipelineReextractionProgress after.
export async function runPipelineReextraction(
  companyId: string,
): Promise<PipelineReextractionBatch> {
  return invoke("run_pipeline_reextraction", { companyId });
}

// The company's latest re-extraction batch plus per-run progress (terminal
// runs done / failed). Returns a null batch when the company has never had
// one.
export async function getPipelineReextractionProgress(
  companyId: string,
): Promise<PipelineReextractionProgress> {
  return invoke("get_pipeline_reextraction_progress", { companyId });
}
