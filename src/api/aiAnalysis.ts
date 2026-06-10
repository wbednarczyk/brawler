import { callCommand } from "./tauri";
import type { AiAnalysisJob } from "./types";

export type StartAiAnalysisInput = {
  feedItemId: string;
  promptPresetId?: string;
  customQuestion?: string;
  providerMode?: string;
};

export type ListAiAnalysisInput = {
  feedItemId: string;
};

export function startAiAnalysis(input: StartAiAnalysisInput) {
  return callCommand<AiAnalysisJob>("start_ai_analysis", { input });
}

export function listAiAnalysis(input: ListAiAnalysisInput) {
  return callCommand<AiAnalysisJob[]>("list_ai_analysis", { input });
}

export function retryAiAnalysis(jobId: string) {
  return callCommand<AiAnalysisJob>("retry_ai_analysis", { jobId });
}
