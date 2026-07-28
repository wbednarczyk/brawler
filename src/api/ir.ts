import { callCommand } from "./tauri";

// GENERATED from src-tauri/src/ir_resolution.rs via ts-rs (ADR 0048).
export type { ResolveIrReportInput } from "./generated/ResolveIrReportInput";
export type { IrReportCandidate } from "./generated/IrReportCandidate";
export type { IrReportResolution } from "./generated/IrReportResolution";

// NOTE: `resolve_ir_report` is a headless command (autopilot document-tie
// stage); it deliberately has no frontend wrapper (issue #131).

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
