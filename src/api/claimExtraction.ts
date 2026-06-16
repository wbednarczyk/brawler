import type { ManagementClaim } from "./managementClaims";
import { callCommand } from "./tauri";

export type ClaimExtractionSourceType = "report_document" | "transcript";

export type ClaimExtractionProposal = {
  id: string;
  jobId: string;
  statement: string;
  dueFiscalYear: number | null;
  duePeriodType: string | null;
  targetMetricKey: string | null;
  targetComparator: string | null;
  targetValueNumeric: string | null;
  targetUnit: string | null;
  confidence: string | null;
  sourceSnippet: string | null;
  sourceEvidenceType: string | null;
  sourceEvidenceId: string | null;
  status: "pending" | "confirmed" | "rejected";
  claimId: string | null;
  createdAt: string;
  updatedAt: string;
};

export type ClaimExtractionJob = {
  id: string;
  companyId: string;
  sourceType: ClaimExtractionSourceType;
  sourceId: string;
  providerId: string;
  model: string;
  promptVersion: string;
  status: "queued" | "running" | "succeeded" | "failed";
  errorCode: string | null;
  error: string | null;
  createdAt: string;
  startedAt: string | null;
  finishedAt: string | null;
  proposals: ClaimExtractionProposal[];
};

export type StartClaimExtractionInput = {
  sourceType: ClaimExtractionSourceType;
  sourceId: string;
  providerMode?: string | null;
};

export type ConfirmClaimProposalInput = {
  proposalId: string;
  statement?: string | null;
  dueFiscalYear?: number | null;
  duePeriodType?: string | null;
  targetMetricKey?: string | null;
  targetComparator?: string | null;
  targetValueNumeric?: string | null;
  targetUnit?: string | null;
};

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
