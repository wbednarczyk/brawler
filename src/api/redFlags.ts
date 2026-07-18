import { callCommand } from "./tauri";
import type { RedFlagsView } from "./generated/RedFlagsView";

// Company red-flags commands (v0.57 T7, ADR 0083 Decision 8). The panel state is
// a computed read model (active flags + acknowledged history); acknowledgement is
// idempotent and the same evidence never re-raises.
export type { RedFlagsView } from "./generated/RedFlagsView";
export type { RedFlag } from "./generated/RedFlag";

export function getRedFlags(companyId: string) {
  return callCommand<RedFlagsView>("get_red_flags", { companyId });
}

export function acknowledgeRedFlag(flagId: string) {
  return callCommand<RedFlagsView>("acknowledge_red_flag", { input: { flagId } });
}
