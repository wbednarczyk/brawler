import { callCommand } from "./tauri";

// Types GENERATED from src-tauri/src/storage/attention.rs via ts-rs (ADR 0048).
import type { AlertRule } from "./generated/AlertRule";
import type { NewAlertRule } from "./generated/NewAlertRule";
import type { AlertRuleUpdate } from "./generated/AlertRuleUpdate";
import type { AttentionEvent } from "./generated/AttentionEvent";
import type { AttentionEventListInput } from "./generated/AttentionEventListInput";

export type { AlertRule } from "./generated/AlertRule";
export type { NewAlertRule } from "./generated/NewAlertRule";
export type { AlertRuleUpdate } from "./generated/AlertRuleUpdate";
export type { AttentionEvent } from "./generated/AttentionEvent";
export type { AttentionEventListInput } from "./generated/AttentionEventListInput";

// --- Alert rules (ADR 0068) ---

/** Create a user-owned alert rule. The backend enforces trigger invariants. */
export function createAlertRule(input: NewAlertRule) {
  return callCommand<AlertRule>("create_alert_rule", { input });
}

/** All alert rules, oldest first. */
export function listAlertRules() {
  return callCommand<AlertRule[]>("list_alert_rules");
}

/** Update mutable fields of a rule; null fields are left unchanged. */
export function updateAlertRule(input: AlertRuleUpdate) {
  return callCommand<AlertRule>("update_alert_rule", { input });
}

/** Enable / disable a rule (disabled rules never fire). */
export function setAlertRuleEnabled(id: string, enabled: boolean) {
  return callCommand<AlertRule>("set_alert_rule_enabled", { input: { id, enabled } });
}

/** Delete a rule (its attention events cascade with it). */
export function deleteAlertRule(id: string) {
  return callCommand<void>("delete_alert_rule", { input: { id } });
}

// --- Attention events (fired alerts, ADR 0068) ---

/** Fired attention events (newest first), optionally filtered. */
export function listAttentionEvents(input?: AttentionEventListInput) {
  return callCommand<AttentionEvent[]>("list_attention_events", input ? { input } : undefined);
}

/** Mark an event seen (read but not dismissed). */
export function markAttentionEventSeen(id: string) {
  return callCommand<void>("mark_attention_event_seen", { input: { id } });
}

/** Dismiss an event (also marks it seen); drops it from the default list. */
export function dismissAttentionEvent(id: string) {
  return callCommand<void>("dismiss_attention_event", { input: { id } });
}
