import { callCommand } from "./tauri";
import type { DecisionEntry } from "./generated/DecisionEntry";
import type { NewDecisionEntry } from "./generated/NewDecisionEntry";
import type { DecisionEntryListInput } from "./generated/DecisionEntryListInput";

// Decision journal commands (ADR 0071, J2). Entries are IMMUTABLE once saved —
// there is no update/delete command; a correction is a follow-up entry linked via
// `supersededByEntryId`. Evidence links reuse the shared research machinery
// (`api/research` create/list evidence link) with `fromType: "decision_entry"`.
export type { DecisionEntry } from "./generated/DecisionEntry";
export type { NewDecisionEntry } from "./generated/NewDecisionEntry";
export type { DecisionEntryListInput } from "./generated/DecisionEntryListInput";

export function createDecisionEntry(input: NewDecisionEntry) {
  return callCommand<DecisionEntry>("create_decision_entry", { input });
}

// Company/kind filters (both optional); no company ⇒ the global chronological
// journal. Ordering is by `decided_at` (the decision's date), never insertion.
export function listDecisionEntries(input: DecisionEntryListInput = {}) {
  return callCommand<DecisionEntry[]>("list_decision_entries", { input });
}
