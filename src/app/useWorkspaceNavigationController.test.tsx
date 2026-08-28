import { useRef, useState } from "react";
import { describe, expect, it } from "vitest";
import { act, renderHook } from "@testing-library/react";

import { useWorkspaceNavigationController } from "./useWorkspaceNavigationController";
import type { Section } from "./navigation";
import type { SpolkaTransition } from "./useSpolkaScreenWiring";
import { COMPANY_SPECS, makeCompany } from "../test/scenarios/entities";

const company = makeCompany(COMPANY_SPECS[0]);

function useHarness() {
  const [activeSection, setActiveSection] = useState<Section>("Today");
  const [selectedCompanyId, setSelectedCompanyId] = useState<string | null>(null);
  const navigateCallsRef = useRef<SpolkaTransition[]>([]);

  const controller = useWorkspaceNavigationController({
    companiesById: { [company.id]: company },
    feedState: [],
    scopeInboxToCompany: () => {},
    selectedCompanyFeedItemId: null,
    selectedCompanyId,
    setActiveSection,
    setSelectedCompanyFeedItemId: () => {},
    setSelectedCompanyId,
    setSelectedFeedItemId: () => {},
    // A harness stand-in for AppStateRoot's real `navigate` (one guarded
    // commit) — records the transition AND applies its company/section so
    // existing assertions on `activeSection`/`selectedCompanyId` still hold.
    navigate: (transition) => {
      navigateCallsRef.current.push(transition);
      setSelectedCompanyId(transition.companyId);
      setActiveSection(transition.section);
    },
  });

  return {
    controller,
    activeSection,
    selectedCompanyId,
    navigateCalls: navigateCallsRef.current,
  };
}

describe("useWorkspaceNavigationController — openCompanyClaims (F2 S3 nav seam)", () => {
  it("commits company, section, tool and the claim highlight in ONE navigate() call", () => {
    const { result } = renderHook(() => useHarness());

    act(() => {
      result.current.controller.openCompanyClaims(company.id, "claim_target");
    });

    // F3a (ADR 0107): claims open the Spółka screen; company + section + tool
    // + highlight are ONE atomic transition, never set piecewise.
    expect(result.current.activeSection).toBe("Spolka");
    expect(result.current.selectedCompanyId).toBe(company.id);
    expect(result.current.navigateCalls).toEqual([
      {
        companyId: company.id,
        section: "Spolka",
        tool: { t: "tezy", claimId: "claim_target" },
        highlightClaimId: "claim_target",
      },
    ]);
  });
});

describe("useWorkspaceNavigationController — openCompanyWorkspace", () => {
  it("commits company + section with no tool and no claim highlight", () => {
    const { result } = renderHook(() => useHarness());

    act(() => {
      result.current.controller.openCompanyWorkspace(company);
    });

    expect(result.current.navigateCalls).toEqual([{ companyId: company.id, section: "Spolka" }]);
  });
});
