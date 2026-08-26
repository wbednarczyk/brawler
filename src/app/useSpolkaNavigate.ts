import { useCallback, useState, type Dispatch, type SetStateAction } from "react";

import type { SpolkaToolHostApi } from "../screens/Spolka/ToolHost";
import type { Section } from "./navigation";
import type { SpolkaTransition } from "./useSpolkaScreenWiring";

type SpolkaNavigateInput = {
  spolkaTool: SpolkaToolHostApi;
  setSelectedCompanyId: Dispatch<SetStateAction<string | null>>;
  setCockpitInitialCompanyId: Dispatch<SetStateAction<string | null>>;
  setActiveSectionRaw: Dispatch<SetStateAction<Section>>;
  setActiveCockpitLayoutId: Dispatch<SetStateAction<string | null>>;
};

/**
 * The ONE atomic company-workspace transition (F3a S2, ADR 0107, sol R1
 * finding 3), extracted from AppStateRoot (file-size ratchet, ADR 0103):
 * every entry point that switches company (`openCompanyWorkspaceById`,
 * `openCompanyWorkspace`, `openCompanyClaims`, pinned row, GlobalSearch,
 * palette "Open company:", Today "Open thesis") commits companyId +
 * section + optional tool + optional claim highlight together, behind ONE
 * `guardNavigation` call — never `selectedCompanyId` ahead of the guard.
 * `spolkaTool.commitTool` (not `openTool`, which re-guards) sets the tool
 * as part of the SAME already-guarded commit.
 *
 * `highlightClaimId` is owned here (not `useWorkspaceNavigationController`,
 * sol R1 finding 3) so `navigate` can commit it in the SAME guarded
 * transition as the company/section/tool, never a step split off unguarded.
 */
export function useSpolkaNavigate(input: SpolkaNavigateInput) {
  const [highlightClaimId, setHighlightClaimId] = useState<string | null>(
    null,
  );

  const navigate = useCallback(
    (transition: SpolkaTransition) => {
      input.spolkaTool.guardNavigation(() => {
        input.setSelectedCompanyId(transition.companyId);
        input.setCockpitInitialCompanyId(transition.companyId);
        input.setActiveSectionRaw(transition.section);
        if (transition.section === "Cockpit") {
          input.setActiveCockpitLayoutId(transition.cockpitLayoutId ?? null);
        }
        if (transition.highlightClaimId !== undefined) {
          setHighlightClaimId(transition.highlightClaimId);
        }
        input.spolkaTool.commitTool(transition.companyId, transition.tool ?? null);
      });
    },
    // eslint-disable-next-line react-hooks/exhaustive-deps -- input is a fresh object every render; guardNavigation/commitTool are its only stable (useCallback) pieces read here, and listing the whole object would make navigate's identity churn every render
    [input.spolkaTool.guardNavigation, input.spolkaTool.commitTool],
  );

  return { navigate, highlightClaimId, setHighlightClaimId };
}
