import { callCommand } from "./tauri";

export type ClaimStatus =
  | "pending"
  | "delivered"
  | "partially_delivered"
  | "missed"
  | "revised";

export type ClaimSourceEvidenceType =
  | "report_document"
  | "transcript_segment"
  | "feed_item"
  | "manual";

export type ClaimTargetComparator = "gte" | "lte" | "gt" | "lt" | "approx" | "eq";

export type ManagementClaim = {
  id: string;
  companyId: string;
  statement: string;
  body: string;
  bodyFormat: string;
  madeAt: string | null;
  sourcePeriodId: string | null;
  dueFiscalYear: number | null;
  duePeriodType: string | null;
  status: ClaimStatus;
  sourceEvidenceType: ClaimSourceEvidenceType;
  sourceEvidenceId: string | null;
  extractionProposalId: string | null;
  targetMetricKey: string | null;
  targetComparator: ClaimTargetComparator | null;
  targetValueNumeric: string | null;
  targetUnit: string | null;
  verifyingFactId: string | null;
  revisesClaimId: string | null;
  createdAt: string;
  updatedAt: string;
};

export type NewManagementClaimInput = {
  companyId: string;
  statement: string;
  body?: string | null;
  madeAt?: string | null;
  sourcePeriodId?: string | null;
  dueFiscalYear?: number | null;
  duePeriodType?: string | null;
  status?: ClaimStatus | null;
  sourceEvidenceType?: ClaimSourceEvidenceType | null;
  sourceEvidenceId?: string | null;
  extractionProposalId?: string | null;
  targetMetricKey?: string | null;
  targetComparator?: ClaimTargetComparator | null;
  targetValueNumeric?: string | null;
  targetUnit?: string | null;
};

export type UpdateManagementClaimInput = {
  id: string;
  statement?: string | null;
  body?: string | null;
  madeAt?: string | null;
  dueFiscalYear?: number | null;
  duePeriodType?: string | null;
  sourceEvidenceType?: ClaimSourceEvidenceType | null;
  sourceEvidenceId?: string | null;
  targetMetricKey?: string | null;
  targetComparator?: ClaimTargetComparator | null;
  targetValueNumeric?: string | null;
  targetUnit?: string | null;
};

export type SetClaimVerdictInput = {
  claimId: string;
  status: ClaimStatus;
  verifyingFactId?: string | null;
  verifyingRelation?: "supports" | "contradicts" | null;
  revisesClaimId?: string | null;
};

export type VerifyingFactCandidate = {
  factId: string;
  valueNumeric: string;
};

export type ClaimToVerify = {
  claim: ManagementClaim;
  arrivedPeriodId: string | null;
  verifyingFactCandidate: VerifyingFactCandidate | null;
};

export type ClaimsToVerify = {
  due: ClaimToVerify[];
  overdue: ClaimToVerify[];
  upcoming: ClaimToVerify[];
};

export function listManagementClaims(companyId: string) {
  return callCommand<ManagementClaim[]>("list_management_claims", { companyId });
}

export function listClaimsToVerify(companyId: string) {
  return callCommand<ClaimsToVerify>("list_claims_to_verify", { companyId });
}

export function createManagementClaim(input: NewManagementClaimInput) {
  return callCommand<ManagementClaim>("create_management_claim", { input });
}

export function updateManagementClaim(input: UpdateManagementClaimInput) {
  return callCommand<ManagementClaim>("update_management_claim", { input });
}

export function setClaimVerdict(input: SetClaimVerdictInput) {
  return callCommand<ManagementClaim>("set_claim_verdict", { input });
}

export function deleteManagementClaim(id: string) {
  return callCommand<void>("delete_management_claim", { id });
}
