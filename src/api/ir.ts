import { callCommand } from "./tauri";
import type { ReportDocument } from "./reportDocumentsTypes";

export type ResolveIrReportInput = {
  companyId: string;
  periodHint?: string;
  reportType?: string;
  publishedAt?: string;
};

export type IrReportCandidate = {
  url: string;
  label: string;
};

export type IrReportResolution = {
  document: ReportDocument | null;
  candidates: IrReportCandidate[];
  pickedUrl: string | null;
  confidence: string | null;
};

export function resolveIrReport(input: ResolveIrReportInput) {
  return callCommand<IrReportResolution>("resolve_ir_report", { input });
}

export function getCompanyIrReportsUrl(companyId: string) {
  return callCommand<string | null>("get_company_ir_reports_url", {
    companyId,
  });
}

export function setCompanyIrReportsUrl(companyId: string, url: string | null) {
  return callCommand<string | null>("set_company_ir_reports_url", {
    companyId,
    url,
  });
}
