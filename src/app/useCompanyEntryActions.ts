import type { Dispatch, SetStateAction } from "react";

import type { Company } from "../api/types";
import type { CompanyWorkspaceTab } from "../screens/Companies/companyTypes";
import type { Tool } from "../screens/Spolka/route";
import type { Section } from "./navigation";
import type { SpolkaTransition } from "./useSpolkaScreenWiring";

type CompanyEntryActionsInput = {
  companies: Company[];
  cockpitInitialCompanyId: string | null;
  pinnedCompanyIds: string[];
  selectedCompanyId: string | null;
  setActiveSection: Dispatch<SetStateAction<Section>>;
  setActiveCockpitLayoutId: Dispatch<SetStateAction<string | null>>;
  setCockpitInitialCompanyId: Dispatch<SetStateAction<string | null>>;
  /** Commits companyId + section + optional tool as ONE guarded transition
   * (the ToolHost seam, F3a S2, sol R1 finding 3) — used to map a
   * `CompanyWorkspaceTab` intent onto its Spółka tool (plan "Mapowanie
   * WSZYSTKICH intencji"). */
  navigate: (transition: SpolkaTransition) => void;
};

// Every `CompanyWorkspaceTab` that opens a Spółka tool (F3a S3, ADR 0107).
// "Feed"/undefined and "Transcripts" are handled separately below — Feed is
// the Spółka core (no tool), Transcripts has no company-scoped Spółka tool
// yet and stays on its legacy global route.
const TAB_TOOL: Partial<Record<CompanyWorkspaceTab, Tool>> = {
  Fundamentals: { t: "fundamenty" },
  Claims: { t: "tezy" },
  Quality: { t: "jakosc" },
  Notebook: { t: "notatnik" },
  Metadata: { t: "akcjonariat" },
};

/**
 * Company deep-dive entry points extracted from AppStateRoot (file-size
 * ratchet, ADR 0103): opening a company lands the Spółka screen by default
 * (F3a S1, ADR 0107); the cockpit dashboard (ADR 0057) stays reachable via
 * `openAdvancedLayout`/a "Dawny dashboard · TICKER" Widoki row (F3a S3),
 * which seeds a default company from the last-viewed one, else pinned, else
 * first.
 */
export function useCompanyEntryActions(input: CompanyEntryActionsInput) {
  function openCompanyWorkspaceById(
    companyId: string,
    tab?: CompanyWorkspaceTab,
  ) {
    if (tab === "Transcripts") {
      // No company-scoped Spółka tool for transcripts yet — the legacy
      // global Transcripts screen (plan "Mapowanie WSZYSTKICH intencji").
      input.navigate({ companyId, section: "Transcripts" });
      return;
    }
    input.navigate({ companyId, section: "Spolka", tool: tab ? TAB_TOOL[tab] : undefined });
  }

  // The legacy dashboard goes through the SAME guarded transition as every
  // Spółka entry (sol R1 finding 3 residual): no setter runs ahead of the guard.
  function openAdvancedLayout(companyId: string) {
    input.navigate({ companyId, section: "Cockpit", cockpitLayoutId: null });
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
      input.setActiveSection("Cockpit");
    }
  }

  // Spółka mode (F3a S3, ADR 0107 amendment, consent 3): opens the
  // last-viewed company — `selectedCompanyId`, already in-memory app state —
  // else the first pinned, else the first tracked company; zero companies
  // lands on Companies, never a blank Spółka screen.
  function openSpolkaMode() {
    const target =
      input.selectedCompanyId ??
      input.pinnedCompanyIds[0] ??
      input.companies[0]?.id ??
      null;
    if (target) {
      openCompanyWorkspaceById(target);
    } else {
      input.setActiveSection("Companies");
    }
  }

  return { openCompanyWorkspaceById, openAdvancedLayout, openDashboard, openSpolkaMode };
}
