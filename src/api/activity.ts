import { invoke } from "@tauri-apps/api/core";
import type { ActivityView } from "./generated/ActivityView";
import type { ActivitySummary } from "./generated/ActivitySummary";

export type { ActivityView } from "./generated/ActivityView";
export type { ActivityItem } from "./generated/ActivityItem";
export type { ActivityFamily } from "./generated/ActivityFamily";
export type { ActivityTarget } from "./generated/ActivityTarget";
export type { ActivityTool } from "./generated/ActivityTool";
export type { ActivityProgress } from "./generated/ActivityProgress";
export type { ActivitySummary } from "./generated/ActivitySummary";

// The composed Activity view (ADR 0109, #133): what is running now, what is
// queued, and what finished recently — over the durable queue, the job_runs
// occurrence history, and the direct-activity registry alike. UI-only read,
// not exposed as an MCP tool.
export async function listActivity(): Promise<ActivityView> {
  return invoke("list_activity");
}

// The topbar summary: active/queued counts + last-finished time. No failure
// count by design (ADR 0097 amendment) — the topbar signals work in progress
// only, never a failure.
export async function getActivitySummary(): Promise<ActivitySummary> {
  return invoke("get_activity_summary");
}
