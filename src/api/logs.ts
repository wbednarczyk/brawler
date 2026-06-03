import { callCommand } from "./tauri";
import type { LogEntry, LogStatus } from "./types";

export type ListLogEntriesInput = {
  limit?: number;
};

export function getLogStatus() {
  return callCommand<LogStatus>("get_log_status");
}

export function listLogEntries(input?: ListLogEntriesInput) {
  if (input === undefined) {
    return callCommand<LogEntry[]>("list_log_entries");
  }

  return callCommand<LogEntry[]>("list_log_entries", { input });
}

export function openLogsDirectory() {
  return callCommand<void>("open_logs_directory");
}
