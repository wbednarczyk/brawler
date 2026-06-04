import { callCommand } from "./tauri";
import type { LocalMetricsSnapshot } from "./types";

export function getLocalMetricsSnapshot() {
  return callCommand<LocalMetricsSnapshot>("get_local_metrics_snapshot");
}
