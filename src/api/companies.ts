import { callCommand } from "./tauri";
import type { Company, CompanyLookupResult } from "./types";
import type { LookupCompanyInput } from "./generated/LookupCompanyInput";
import type { CreateCompanyInput } from "./generated/CreateCompanyInput";

// Input types GENERATED from src-tauri/src/storage/types.rs via ts-rs (ADR 0048).
export type { LookupCompanyInput } from "./generated/LookupCompanyInput";
export type { CreateCompanyInput } from "./generated/CreateCompanyInput";

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
