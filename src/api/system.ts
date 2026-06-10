import { callCommand } from "./tauri";
import type { DatabaseStatus, HealthResponse } from "./types";

export function getHealth() {
  return callCommand<HealthResponse>("health");
}

export function getDatabaseStatus() {
  return callCommand<DatabaseStatus>("database_status");
}
