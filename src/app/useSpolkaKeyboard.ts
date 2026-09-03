import { useCallback } from "react";
import type { Company } from "../api/types";
import type { SpolkaToolHostApi } from "../screens/Spolka/ToolHost";
import { WORKSHOP_TOOLS, workshopIndexOf, type Tool } from "../screens/Spolka/route";
import type { Section } from "./navigation";
import type { SpolkaTransition } from "./useSpolkaScreenWiring";

export type UseSpolkaKeyboardInput = {
  activeSection: Section;
  selectedCompanyId: string | null;
  /** The FULL tracked list, in tracked order — the Spółka H/L-adjacent-
   * company path cycles this (plan § Design 5: "Adjacent = companies
   * order"), not the Companies screen's filtered/sorted view. */
  companies: Company[];
  /** The Companies-screen filtered list — `selectAdjacentCompany`'s ORIGINAL
   * behavior (moved here verbatim from `AppStateRoot.tsx`, sol R1 finding 3
   * era) still cycles this, unchanged, when `activeSection === "Companies"`. */
  filteredCompanies: Company[];
  spolkaTool: SpolkaToolHostApi;
  navigate: (transition: SpolkaTransition) => void;
  setSelectedCompanyId: (companyId: string) => void;
};

/**
 * The Spółka workshop's four keyboard actions (F3c S1, plan § Design 5):
 * H/L cycle workshop tools (wrap, Overview included); Shift+J/K move to the
 * adjacent company. `selectAdjacentCompany` (the Companies-screen behavior)
 * moves here verbatim from `AppStateRoot.tsx:1211` so that file — pinned at
 * 1870 lines — does not grow; the SAME action id now also drives the new
 * Spółka-only company-adjacency path via `navigate`, never a direct
 * `selectedCompanyId` set (ADR 0107 dec. 6 — the tool closes, never
 * retargets across companies).
 */
export function useSpolkaKeyboard(input: UseSpolkaKeyboardInput) {
  const { activeSection, selectedCompanyId, companies, filteredCompanies, spolkaTool, navigate, setSelectedCompanyId } = input;

  const cycleTool = useCallback(
    (direction: 1 | -1) => {
      if (activeSection !== "Spolka" || !selectedCompanyId) return false;
      const count = WORKSHOP_TOOLS.length + 1;
      const currentIndex = workshopIndexOf(spolkaTool.tool);
      const nextIndex = ((currentIndex + direction) % count + count) % count;
      const nextTool: Tool | null = nextIndex === 0 ? null : WORKSHOP_TOOLS[nextIndex - 1].tool;
      navigate({ companyId: selectedCompanyId, section: "Spolka", tool: nextTool });
      return true;
    },
    [activeSection, navigate, selectedCompanyId, spolkaTool.tool],
  );

  const selectAdjacentCompany = useCallback(
    (direction: 1 | -1) => {
      if (activeSection === "Companies") {
        if (filteredCompanies.length === 0) return false;
        const currentIndex = selectedCompanyId
          ? filteredCompanies.findIndex((company) => company.id === selectedCompanyId)
          : -1;
        const nextIndex =
          currentIndex === -1
            ? 0
            : Math.min(Math.max(currentIndex + direction, 0), filteredCompanies.length - 1);
        const next = filteredCompanies[nextIndex];
        if (!next) return false;
        setSelectedCompanyId(next.id);
        return true;
      }

      if (activeSection === "Spolka") {
        if (companies.length === 0 || !selectedCompanyId) return false;
        const currentIndex = companies.findIndex((company) => company.id === selectedCompanyId);
        const nextIndex =
          currentIndex === -1 ? 0 : Math.min(Math.max(currentIndex + direction, 0), companies.length - 1);
        const next = companies[nextIndex];
        if (!next) return false;
        // No `tool` key (ADR 0107 dec. 6): the guarded transition closes
        // whatever tool was open and focuses the company picker.
        navigate({ companyId: next.id, section: "Spolka", focusIntent: "company" });
        return true;
      }

      return false;
    },
    [activeSection, companies, filteredCompanies, navigate, selectedCompanyId, setSelectedCompanyId],
  );

  return {
    nextTool: () => cycleTool(1),
    previousTool: () => cycleTool(-1),
    nextCompany: () => selectAdjacentCompany(1),
    previousCompany: () => selectAdjacentCompany(-1),
  };
}
