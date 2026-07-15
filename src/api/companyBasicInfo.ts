import { callCommand } from "./tauri";
import type { CompanyBasicInfo } from "./generated/CompanyBasicInfo";

export type { CompanyBasicInfo };

/// Basic info read model (v0.53 follow-up): identity facts (name, ticker,
/// ISIN), sector with provenance, latest recorded shares_outstanding fact.
export function getCompanyBasicInfo(companyId: string) {
  return callCommand<CompanyBasicInfo>("get_company_basic_info", { companyId });
}
