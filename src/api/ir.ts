import { callCommand } from "./tauri";
import type { ResolveIrReportInput } from "./generated/ResolveIrReportInput";
import type { IrReportResolution } from "./generated/IrReportResolution";

// GENERATED from src-tauri/src/ir_resolution.rs via ts-rs (ADR 0048).
export type { ResolveIrReportInput } from "./generated/ResolveIrReportInput";
export type { IrReportCandidate } from "./generated/IrReportCandidate";
export type { IrReportResolution } from "./generated/IrReportResolution";

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
