import { callCommand } from "./tauri";
import type { OwnershipOverview } from "./generated/OwnershipOverview";

// GENERATED DTOs from src-tauri/src/commands/ownership.rs via ts-rs (ADR 0048).
export type { OwnershipOverview } from "./generated/OwnershipOverview";
export type { OwnershipHolder } from "./generated/OwnershipHolder";
export type { OwnershipHolderSeries } from "./generated/OwnershipHolderSeries";
export type { OwnershipSeriesPoint } from "./generated/OwnershipSeriesPoint";
export type { OwnershipResidual } from "./generated/OwnershipResidual";

/// Ownership overview for the Basic Info panel's Akcjonariat section.
export function getOwnershipOverview(companyId: string) {
  return callCommand<OwnershipOverview>("get_ownership_overview", { companyId });
}

/// Force-enqueue deterministic ownership extraction across the company's fetched
/// periodic reports (the "Wydobądź z raportów" CTA). Returns the enqueued count.
export function backfillOwnershipExtraction(companyId: string) {
  return callCommand<number>("backfill_ownership_extraction", { companyId });
}

/// Manual re-type (or clear, with `null`) a holder's classification. Returns the
/// refreshed overview.
export function setOwnershipHolderType(
  companyId: string,
  holderKey: string,
  holderType: string | null,
) {
  return callCommand<OwnershipOverview>("set_ownership_holder_type", {
    companyId,
    holderKey,
    holderType,
  });
}

// ADR 0084 decision 5 (clean cut): the AI holder-type classifier, the tier-4 OCR
// passes AND the stored proposals they left behind are gone — tables dropped, so
// there is no confirm/reject surface left to read. Holder types stay fully
// user-editable through `setOwnershipHolderType`.
