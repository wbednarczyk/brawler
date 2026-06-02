import { callCommand } from "./tauri";
import type { Company, CompanyLookupResult } from "./types";

export type LookupCompanyInput = {
  exchange: string;
  ticker: string | null;
  displayName: string | null;
  isin: string | null;
};

export type CreateCompanyInput = {
  exchange: string;
  ticker: string;
  displayName: string;
  isin: string | null;
  cik: string | null;
  lei: string | null;
};

export function listCompanies() {
  return callCommand<Company[]>("list_companies");
}

export function lookupCompany(input: LookupCompanyInput) {
  return callCommand<CompanyLookupResult | null>("lookup_company", { input });
}

export function createCompany(input: CreateCompanyInput) {
  return callCommand<Company>("create_company", { input });
}

export function deleteCompany(companyId: string) {
  return callCommand<void>("delete_company", { companyId });
}
