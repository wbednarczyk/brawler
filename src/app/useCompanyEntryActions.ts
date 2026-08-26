import type { Dispatch, SetStateAction } from "react";

import type { Company } from "../api/types";
import type { CompanyWorkspaceTab } from "../screens/Companies/companyTypes";
import type { Section } from "./navigation";

type CompanyEntryActionsInput = {
  companies: Company[];
  cockpitInitialCompanyId: string | null;
  pinnedCompanyIds: string[];
  setSelectedCompanyId: Dispatch<SetStateAction<string | null>>;
  setActiveSection: Dispatch<SetStateAction<Section>>;
  setActiveCockpitLayoutId: Dispatch<SetStateAction<string | null>>;
  setCockpitInitialCompanyId: Dispatch<SetStateAction<string | null>>;
  setCockpitInitialPresetId: Dispatch<SetStateAction<string | null>>;
};

/**
 * Company deep-dive entry points extracted from AppStateRoot (file-size
 * ratchet, ADR 0103): opening a company lands the Spółka screen by default
 * (F3a S1, ADR 0107); the cockpit dashboard (ADR 0057) stays reachable via
 * `openAdvancedLayout`/the left-nav Dashboard entry (epic c793ca1), which
 * seeds a default company from the last-viewed one, else pinned, else first.
 */
export function useCompanyEntryActions(input: CompanyEntryActionsInput) {
  function openCompanyWorkspaceById(
    companyId: string,
    _tab?: CompanyWorkspaceTab,
  ) {
    input.setSelectedCompanyId(companyId);
    input.setActiveSection("Spolka");
  }

  function openAdvancedLayout(
    companyId: string,
    presetId: string | null = null,
  ) {
    input.setSelectedCompanyId(companyId);
    input.setActiveCockpitLayoutId(null);
    input.setCockpitInitialCompanyId(companyId);
    input.setCockpitInitialPresetId(presetId);
    input.setActiveSection("Cockpit");
  }

  function openDashboard() {
    const target =
      input.cockpitInitialCompanyId ??
      input.pinnedCompanyIds[0] ??
      input.companies[0]?.id ??
      null;
    if (target) {
      openAdvancedLayout(target);
    } else {
      input.setActiveCockpitLayoutId(null);
      input.setCockpitInitialCompanyId(null);
      input.setCockpitInitialPresetId(null);
      input.setActiveSection("Cockpit");
    }
  }

  return { openCompanyWorkspaceById, openAdvancedLayout, openDashboard };
}
