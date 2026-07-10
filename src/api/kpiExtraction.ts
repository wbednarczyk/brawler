import { callCommand } from "./tauri";
import type { KpiExtractionProposal } from "./generated/KpiExtractionProposal";
import type { KpiExtractionJob } from "./generated/KpiExtractionJob";
import type { StartKpiExtractionInput } from "./generated/StartKpiExtractionInput";
import type { ConfirmKpiProposalInput } from "./generated/ConfirmKpiProposalInput";
import type { ConfirmedKpiFact } from "./generated/ConfirmedKpiFact";
import type { PendingKpiProposal } from "./generated/PendingKpiProposal";

// GENERATED from src-tauri/src/storage/kpi_extraction.rs + commands via ts-rs (ADR 0048).
export type { KpiExtractionProposal } from "./generated/KpiExtractionProposal";
export type { KpiExtractionJob } from "./generated/KpiExtractionJob";
export type { StartKpiExtractionInput } from "./generated/StartKpiExtractionInput";
export type { ConfirmKpiProposalInput } from "./generated/ConfirmKpiProposalInput";
export type { ConfirmedKpiFact } from "./generated/ConfirmedKpiFact";
export type { PendingKpiProposal } from "./generated/PendingKpiProposal";

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

export function listPendingKpiProposals(companyId: string) {
  return callCommand<PendingKpiProposal[]>("list_pending_kpi_proposals", {
    companyId,
  });
}

export function confirmKpiProposal(input: ConfirmKpiProposalInput) {
  return callCommand<ConfirmedKpiFact>("confirm_kpi_proposal", { input });
}

export function rejectKpiProposal(proposalId: string) {
  return callCommand<KpiExtractionProposal>("reject_kpi_proposal", {
    proposalId,
  });
}
