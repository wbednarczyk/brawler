import { callCommand } from "./tauri";
import type { TodayView } from "./generated/TodayView";

// GENERATED DTO from src-tauri/src/commands/today.rs via ts-rs (F2 S1, ADR
// 0106 dec. 3, `compute_company_context` pattern).
export type { TodayView } from "./generated/TodayView";
export type { TodayItem } from "./generated/TodayItem";
export type { TodayClaim } from "./generated/TodayClaim";
export type { TodayDeltaSummary } from "./generated/TodayDeltaSummary";
export type { TodaySectionErrors } from "./generated/TodaySectionErrors";

/// The Dziś v2 composed read model (F2 S1, plan decision 1): flat `items[]` +
/// bulk claims-to-verify + a delta summary since `previousVisitAt` (read from
/// KV here, never a caller-supplied anchor — S1 is the one source of truth).
export function getTodayView(dayLimit: number) {
  return callCommand<TodayView>("get_today_view", { dayLimit });
}

/// Stamps the visit anchor with the BACKEND's own clock (plan decision 4) —
/// call ONCE after the first successful render, never on error/unmount.
export function markTodayVisited() {
  return callCommand<string>("mark_today_visited");
}
