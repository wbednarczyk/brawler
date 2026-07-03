import { callCommand } from "./tauri";

// Types GENERATED from src-tauri/src/storage/autopilot.rs and
// src-tauri/src/commands/autopilot.rs via ts-rs (ADR 0048): change the struct and
// run `make types`. Autonomous report pipeline (North Star, v0.49.0, ADR 0055).
import type { CompanyAutopilot } from "./generated/CompanyAutopilot";
import type { AutopilotRun } from "./generated/AutopilotRun";
import type { ListAutopilotRunsInput } from "./generated/ListAutopilotRunsInput";
import type { SetCompanyAutopilotInput } from "./generated/SetCompanyAutopilotInput";
import type { SetRunNotificationStateInput } from "./generated/SetRunNotificationStateInput";
import type { TriggerAutopilotRunInput } from "./generated/TriggerAutopilotRunInput";
import type { UndoAutopilotRunResult } from "./generated/UndoAutopilotRunResult";
import type { SetCompaniesAutopilotInput } from "./generated/SetCompaniesAutopilotInput";

export type { CompanyAutopilot } from "./generated/CompanyAutopilot";
export type { AutopilotRun } from "./generated/AutopilotRun";
export type { ListAutopilotRunsInput } from "./generated/ListAutopilotRunsInput";
export type { SetCompanyAutopilotInput } from "./generated/SetCompanyAutopilotInput";
export type { SetRunNotificationStateInput } from "./generated/SetRunNotificationStateInput";
export type { TriggerAutopilotRunInput } from "./generated/TriggerAutopilotRunInput";
export type { UndoAutopilotRunResult } from "./generated/UndoAutopilotRunResult";

/** The per-company trust-ladder mode. `off` is the default for every company. */
export type AutopilotMode = "off" | "assist" | "autopilot";

export function getCompanyAutopilot(companyId: string) {
  return callCommand<CompanyAutopilot>("get_company_autopilot", { input: { companyId } });
}

export function setCompanyAutopilot(input: SetCompanyAutopilotInput) {
  return callCommand<CompanyAutopilot>("set_company_autopilot", { input });
}

/** Every company with an explicit autopilot mode (the rest default to "off"). */
export function listCompanyAutopilotModes() {
  return callCommand<CompanyAutopilot[]>("list_company_autopilot_modes");
}

/** Set the same autopilot mode on many companies at once (ADR 0056). */
export function setCompaniesAutopilot(input: SetCompaniesAutopilotInput) {
  return callCommand<number>("set_companies_autopilot", { input });
}

export type { SetCompaniesAutopilotInput } from "./generated/SetCompaniesAutopilotInput";

export function listAutopilotRuns(input: ListAutopilotRunsInput = {}) {
  return callCommand<AutopilotRun[]>("list_autopilot_runs", { input });
}

export function getAutopilotRun(runId: string) {
  return callCommand<AutopilotRun>("get_autopilot_run", { input: { runId } });
}

export function setAutopilotRunNotificationState(input: SetRunNotificationStateInput) {
  return callCommand<AutopilotRun>("set_autopilot_run_notification_state", { input });
}

export function undoAutopilotRun(runId: string) {
  return callCommand<UndoAutopilotRunResult>("undo_autopilot_run", { input: { runId } });
}

export function triggerAutopilotRun(input: TriggerAutopilotRunInput) {
  return callCommand<AutopilotRun>("trigger_autopilot_run", { input });
}
