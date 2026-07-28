import { callCommand } from "./tauri";

// Types GENERATED from src-tauri/src/storage/management_claims.rs via ts-rs
// (ADR 0048): change the struct and run `make types`. The Claim* unions come
// from the marker enums in src-tauri/src/api_ts_unions.rs.
import type { ManagementClaim } from "./generated/ManagementClaim";
import type { ClaimsToVerify } from "./generated/ClaimsToVerify";
import type { NewManagementClaimInput } from "./generated/NewManagementClaimInput";
import type { SetClaimVerdictInput } from "./generated/SetClaimVerdictInput";

export type { ClaimStatus } from "./generated/ClaimStatus";
export type { ClaimSourceEvidenceType } from "./generated/ClaimSourceEvidenceType";
export type { ClaimTargetComparator } from "./generated/ClaimTargetComparator";
export type { ManagementClaim } from "./generated/ManagementClaim";
export type { NewManagementClaimInput } from "./generated/NewManagementClaimInput";
export type { UpdateManagementClaimInput } from "./generated/UpdateManagementClaimInput";
export type { SetClaimVerdictInput } from "./generated/SetClaimVerdictInput";
export type { VerifyingFactCandidate } from "./generated/VerifyingFactCandidate";
export type { ClaimToVerify } from "./generated/ClaimToVerify";
export type { ClaimsToVerify } from "./generated/ClaimsToVerify";

export function listManagementClaims(companyId: string) {
  return callCommand<ManagementClaim[]>("list_management_claims", { companyId });
}

export function listClaimsToVerify(companyId: string) {
  return callCommand<ClaimsToVerify>("list_claims_to_verify", { companyId });
}

export function createManagementClaim(input: NewManagementClaimInput) {
  return callCommand<ManagementClaim>("create_management_claim", { input });
}

export function setClaimVerdict(input: SetClaimVerdictInput) {
  return callCommand<ManagementClaim>("set_claim_verdict", { input });
}

