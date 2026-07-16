import { callCommand } from "./tauri";
import type { OwnershipOverview } from "./generated/OwnershipOverview";
import type { OwnershipClassificationResult } from "./generated/OwnershipClassificationResult";

// GENERATED DTOs from src-tauri/src/commands/ownership.rs via ts-rs (ADR 0048).
export type { OwnershipOverview } from "./generated/OwnershipOverview";
export type { OwnershipHolder } from "./generated/OwnershipHolder";
export type { OwnershipHolderSeries } from "./generated/OwnershipHolderSeries";
export type { OwnershipSeriesPoint } from "./generated/OwnershipSeriesPoint";
export type { OwnershipResidual } from "./generated/OwnershipResidual";
export type { OwnershipProposal } from "./generated/OwnershipProposal";
export type { OwnershipClassificationResult } from "./generated/OwnershipClassificationResult";

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

/// Confirm a pending AI holder-type proposal. Returns the refreshed overview.
export function confirmOwnershipHolderTypeProposal(companyId: string, proposalId: string) {
  return callCommand<OwnershipOverview>("confirm_ownership_holder_type_proposal", {
    companyId,
    proposalId,
  });
}

/// Reject a pending AI holder-type proposal. Returns the refreshed overview.
export function rejectOwnershipHolderTypeProposal(companyId: string, proposalId: string) {
  return callCommand<OwnershipOverview>("reject_ownership_holder_type_proposal", {
    companyId,
    proposalId,
  });
}

/// Run the AI holder-type classify-with-confirm job over residual holders.
export function runOwnershipClassification() {
  return callCommand<OwnershipClassificationResult>("run_ownership_classification");
}
