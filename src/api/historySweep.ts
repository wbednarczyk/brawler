import { invoke } from "@tauri-apps/api/core";
import type { HistorySweep } from "./generated/HistorySweep";
import type { HistorySweepProgress } from "./generated/HistorySweepProgress";

export type { HistorySweep } from "./generated/HistorySweep";
export type { HistorySweepProgress } from "./generated/HistorySweepProgress";

// Start a manual history sweep for a company (ADR 0077 §3): enqueue extraction for
// every canonical periodic report whose period still lacks accepted facts. Gated
// backend-side on automation mode ≠ off. Poll getHistorySweepProgress after.
export async function runHistorySweep(companyId: string): Promise<HistorySweep> {
  return invoke("run_history_sweep", { companyId });
}

// The company's latest history sweep plus per-run progress (terminal runs done /
// failed). Returns a null sweep when the company has never been swept.
export async function getHistorySweepProgress(
  companyId: string,
): Promise<HistorySweepProgress> {
  return invoke("get_history_sweep_progress", { companyId });
}
