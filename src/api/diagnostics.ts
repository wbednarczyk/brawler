import { callCommand } from "./tauri";
import type { ClearDiagnosticEventsResult, DiagnosticEvent, DiagnosticSummary } from "./types";

export type ListDiagnosticEventsInput = {
  limit?: number;
};

export function listDiagnosticEvents(input?: ListDiagnosticEventsInput) {
  if (input === undefined) {
    return callCommand<DiagnosticEvent[]>("list_diagnostic_events");
  }

  return callCommand<DiagnosticEvent[]>("list_diagnostic_events", { input });
}

export function clearDiagnosticEvents() {
  return callCommand<ClearDiagnosticEventsResult>("clear_diagnostic_events");
}

export function getDiagnosticSummary(input?: ListDiagnosticEventsInput) {
  if (input === undefined) {
    return callCommand<DiagnosticSummary>("get_diagnostic_summary");
  }

  return callCommand<DiagnosticSummary>("get_diagnostic_summary", { input });
}
