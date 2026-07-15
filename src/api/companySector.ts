import { callCommand } from "./tauri";

/// Manual sector override (ADR 0067 Decision 3): a registry refresh never
/// clobbers a manually-set sector (`sector_source='manual'`); clearing it
/// (empty/null) reverts to the registry-sourced value.
export function getCompanySector(companyId: string) {
  return callCommand<string | null>("get_company_sector", { companyId });
}

export function setCompanySector(companyId: string, sector: string | null) {
  return callCommand<string | null>("set_company_sector", {
    companyId,
    sector,
  });
}

/// Registry-sourced sector taxonomy (distinct, sorted), for the visual-first
/// preset chips (v0.53 M2 T3, ADR 0067 Decision 3) above the free-entry field.
export function listCompanySectors() {
  return callCommand<string[]>("list_company_sectors");
}
