import { callCommand } from "./tauri";
import type { AiAnalysisJob } from "./types";
import type { StartAiAnalysisInput } from "./generated/StartAiAnalysisInput";
import type { ListAiAnalysisInput } from "./generated/ListAiAnalysisInput";

// Input types GENERATED from src-tauri/src/commands/ai_analysis.rs via ts-rs (ADR 0048).
export type { StartAiAnalysisInput } from "./generated/StartAiAnalysisInput";
export type { ListAiAnalysisInput } from "./generated/ListAiAnalysisInput";

export function startAiAnalysis(input: StartAiAnalysisInput) {
  return callCommand<AiAnalysisJob>("start_ai_analysis", { input });
}

export function listAiAnalysis(input: ListAiAnalysisInput) {
  return callCommand<AiAnalysisJob[]>("list_ai_analysis", { input });
}

export function retryAiAnalysis(jobId: string) {
  return callCommand<AiAnalysisJob>("retry_ai_analysis", { jobId });
}
