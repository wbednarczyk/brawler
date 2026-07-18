import { callCommand } from "./tauri";
import type { InsiderOverview } from "./generated/InsiderOverview";

// GENERATED DTOs from src-tauri/src/commands/insider.rs via ts-rs (ADR 0048/0083).
export type { InsiderOverview } from "./generated/InsiderOverview";
export type { InsiderTransactionEntry } from "./generated/InsiderTransactionEntry";
export type { ManagementHoldingEntry } from "./generated/ManagementHoldingEntry";
export type { WindowAggregate } from "./generated/WindowAggregate";

/// Insider transaction timeline + latest management holdings + rolling
/// net-direction aggregates (90d / 12m) for the Ownership area's "Insiderzy"
/// block (ADR 0083 Decision 7). Computed read model; decision support only.
export function getInsiderOverview(companyId: string) {
  return callCommand<InsiderOverview>("get_insider_overview", { companyId });
}
