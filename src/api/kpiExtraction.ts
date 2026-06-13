import { callCommand } from "./tauri";
import type { FinancialFact } from "./financialsTypes";

export type KpiExtractionProposal = {
  id: string;
  jobId: string;
  metricKey: string;
  label: string;
  valueNumeric: string;
  unit: string | null;
  currency: string | null;
  asReportedValue: string | null;
  asReportedScale: string | null;
  measureWindow: string | null;
  confidence: string | null;
  sourceSnippet: string | null;
  isProposedKpi: boolean;
  status: string;
  factId: string | null;
  createdAt: string;
  updatedAt: string;
};

export type KpiExtractionJob = {
  id: string;
  companyId: string;
  reportDocumentId: string;
  providerId: string;
  model: string;
  promptVersion: string;
  periodHint: string | null;
  status: string;
  errorCode: string | null;
  error: string | null;
  detectedFiscalYear: number | null;
  detectedPeriodType: string | null;
  detectedPeriodEndDate: string | null;
  detectedCurrency: string | null;
  detectedLanguage: string | null;
  createdAt: string;
  startedAt: string | null;
  finishedAt: string | null;
  proposals: KpiExtractionProposal[];
};

export type StartKpiExtractionInput = {
  reportDocumentId: string;
  periodHint?: string;
  providerMode?: string;
};

export type ConfirmKpiProposalInput = {
  proposalId: string;
  valueNumeric?: string;
  currency?: string;
  fiscalYear?: number;
  periodType?: string;
  periodEndDate?: string;
  acceptAsNewKpi?: boolean;
};

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
