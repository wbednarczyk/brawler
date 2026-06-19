import type { ManagementClaim } from "./managementClaims";
import { callCommand } from "./tauri";
import type { ClaimExtractionSourceType } from "./generated/ClaimExtractionSourceType";
import type { ClaimExtractionJob } from "./generated/ClaimExtractionJob";
import type { StartClaimExtractionInput } from "./generated/StartClaimExtractionInput";
import type { ConfirmClaimProposalInput } from "./generated/ConfirmClaimProposalInput";

// GENERATED from src-tauri/src/storage/claim_extraction.rs + commands via ts-rs
// (ADR 0048); ClaimExtractionSourceType is a marker enum (api_ts_unions.rs).
export type { ClaimExtractionSourceType } from "./generated/ClaimExtractionSourceType";
export type { ClaimExtractionProposal } from "./generated/ClaimExtractionProposal";
export type { ClaimExtractionJob } from "./generated/ClaimExtractionJob";
export type { StartClaimExtractionInput } from "./generated/StartClaimExtractionInput";
export type { ConfirmClaimProposalInput } from "./generated/ConfirmClaimProposalInput";

export function startClaimExtraction(input: StartClaimExtractionInput) {
  return callCommand<ClaimExtractionJob>("start_claim_extraction", { input });
}

export function retryClaimExtraction(jobId: string) {
  return callCommand<ClaimExtractionJob>("retry_claim_extraction", { jobId });
}

export function listClaimExtraction(
  sourceType: ClaimExtractionSourceType,
  sourceId: string,
) {
  return callCommand<ClaimExtractionJob[]>("list_claim_extraction", {
    input: { sourceType, sourceId },
  });
}

export function confirmClaimProposal(input: ConfirmClaimProposalInput) {
  return callCommand<ManagementClaim>("confirm_claim_proposal", { input });
}

export function rejectClaimProposal(proposalId: string) {
  return callCommand<ClaimExtractionJob>("reject_claim_proposal", { proposalId });
}
