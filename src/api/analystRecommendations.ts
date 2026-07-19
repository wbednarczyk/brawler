import { callCommand } from "./tauri";
import type { AnalystRecommendationsView } from "./generated/AnalystRecommendationsView";

// Analyst-recommendations read model (v0.58 A3, ADR 0073). A quiet read surface:
// the per-company history (newest-first), the newest target-carrying entry for
// the attributed "vs target" readout, and the adapter's last refresh for the
// footer. Attributed third-party opinions — never advice.
export type { AnalystRecommendationsView } from "./generated/AnalystRecommendationsView";
export type { AnalystRecommendationRow } from "./generated/AnalystRecommendationRow";
export type { AnalystRecommendationTarget } from "./generated/AnalystRecommendationTarget";

export function getAnalystRecommendations(companyId: string) {
  return callCommand<AnalystRecommendationsView>("get_analyst_recommendations", { companyId });
}
