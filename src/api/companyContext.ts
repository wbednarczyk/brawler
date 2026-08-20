import { callCommand } from "./tauri";
import type { CompanyContext } from "./generated/CompanyContext";

// GENERATED DTO from src-tauri/src/commands/company_context.rs via ts-rs
// (ADR 0106 dec. 3).
export type { CompanyContext } from "./generated/CompanyContext";
export type { CompanyContextClaimsDue } from "./generated/CompanyContextClaimsDue";
export type { CompanyContextEvent } from "./generated/CompanyContextEvent";
export type { CompanyContextFact } from "./generated/CompanyContextFact";
export type { CompanyContextLatestPeriod } from "./generated/CompanyContextLatestPeriod";
export type { CompanyContextNotebook } from "./generated/CompanyContextNotebook";

/// The Inbox detail pane's company-context block (ADR 0106 dec. 3): latest-
/// period facts, upcoming events, notebook coverage, and claims-due counts,
/// composed in one read instead of a 5-command frontend fan-out.
export function getCompanyContext(companyId: string) {
  return callCommand<CompanyContext>("get_company_context", { companyId });
}
