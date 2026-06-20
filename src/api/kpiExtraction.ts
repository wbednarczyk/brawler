import { callCommand } from "./tauri";
import type { FinancialFact } from "./financialsTypes";
import type { KpiExtractionProposal } from "./generated/KpiExtractionProposal";
import type { KpiExtractionJob } from "./generated/KpiExtractionJob";
import type { StartKpiExtractionInput } from "./generated/StartKpiExtractionInput";
import type { ConfirmKpiProposalInput } from "./generated/ConfirmKpiProposalInput";

// GENERATED from src-tauri/src/storage/kpi_extraction.rs + commands via ts-rs (ADR 0048).
export type { KpiExtractionProposal } from "./generated/KpiExtractionProposal";
export type { KpiExtractionJob } from "./generated/KpiExtractionJob";
export type { StartKpiExtractionInput } from "./generated/StartKpiExtractionInput";
export type { ConfirmKpiProposalInput } from "./generated/ConfirmKpiProposalInput";

export function startKpiExtraction(input: StartKpiExtractionInput) {
  return callCommand<KpiExtractionJob>("start_kpi_extraction", { input });
}

export function retryKpiExtraction(jobId: string) {
  return callCommand<KpiExtractionJob>("retry_kpi_extraction", { jobId });
}

export function listKpiExtraction(reportDocumentId: string) {
  return callCommand<KpiExtractionJob[]>("list_kpi_extraction", {
    input: { reportDocumentId },
  });
}

export function confirmKpiProposal(input: ConfirmKpiProposalInput) {
  return callCommand<FinancialFact>("confirm_kpi_proposal", { input });
}

export function rejectKpiProposal(proposalId: string) {
  return callCommand<KpiExtractionProposal>("reject_kpi_proposal", {
    proposalId,
  });
}
