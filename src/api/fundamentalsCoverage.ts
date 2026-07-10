import { invoke } from "@tauri-apps/api/core";
import type { FundamentalsCoverage } from "./fundamentalsCoverageTypes";

// The fundamentals coverage map for one company (ADR 0077 §2): one row per fiscal
// period, unioned from the canonical periodic reports, the extracted facts, and
// the pending review queue. Read-only; the panel reloads it on companyId change.
export async function getFundamentalsCoverage(
  companyId: string,
): Promise<FundamentalsCoverage> {
  return invoke("get_fundamentals_coverage", { companyId });
}
