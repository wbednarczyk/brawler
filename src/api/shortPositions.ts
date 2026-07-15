import { callCommand } from "./tauri";
import type { ShortPositionsView } from "./generated/ShortPositionsView";
import type { ShortPositionsInput } from "./generated/ShortPositionsInput";

// KNF short-selling read command (v0.55 T4b, ADR 0069 decision 3). Read-only:
// the register is populated by the daily `knf-short-selling` adapter; this
// surfaces the per-company cockpit view (active positions, change history, the
// last remembered exit, aggregate net short %, and the 30-day pp change).
export type { ShortPositionsView } from "./generated/ShortPositionsView";
export type { ShortPositionEventRow } from "./generated/ShortPositionEventRow";

export function listShortPositions(input: ShortPositionsInput) {
  return callCommand<ShortPositionsView>("list_short_positions", { input });
}
