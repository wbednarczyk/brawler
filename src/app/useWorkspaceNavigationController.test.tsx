import { useState } from "react";
import { describe, expect, it } from "vitest";
import { act, renderHook } from "@testing-library/react";

import { useWorkspaceNavigationController } from "./useWorkspaceNavigationController";
import type { Section } from "./navigation";
import { COMPANY_SPECS, makeCompany } from "../test/scenarios/entities";

const company = makeCompany(COMPANY_SPECS[0]);

function useHarness() {
  const [activeSection, setActiveSection] = useState<Section>("Today");
  const [selectedCompanyId, setSelectedCompanyId] = useState<string | null>(null);
  const [cockpitInitialCompanyId, setCockpitInitialCompanyId] = useState<string | null>(null);

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
    setCockpitInitialCompanyId,
  });

  return { controller, activeSection, selectedCompanyId, cockpitInitialCompanyId };
}

describe("useWorkspaceNavigationController — openCompanyClaims (F2 S3 nav seam)", () => {
  it("opens the company's curated dashboard and carries the claim id to highlight", () => {
    const { result } = renderHook(() => useHarness());

    act(() => {
      result.current.controller.openCompanyClaims(company.id, "claim_target");
    });

    expect(result.current.activeSection).toBe("Cockpit");
    expect(result.current.selectedCompanyId).toBe(company.id);
    expect(result.current.cockpitInitialCompanyId).toBe(company.id);
    expect(result.current.controller.highlightClaimId).toBe("claim_target");
  });

  it("starts with no highlight (openCompanyWorkspace alone never sets one)", () => {
    const { result } = renderHook(() => useHarness());
    expect(result.current.controller.highlightClaimId).toBeNull();
  });
});
