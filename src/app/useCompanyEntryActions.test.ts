import { useRef, useState } from "react";
import { describe, expect, it } from "vitest";
import { act, renderHook } from "@testing-library/react";

import { useCompanyEntryActions } from "./useCompanyEntryActions";
import type { Section } from "./navigation";
import type { CompanyWorkspaceTab } from "../screens/Companies/companyTypes";
import type { Tool } from "../screens/Spolka/route";
import { COMPANY_SPECS, makeCompany } from "../test/scenarios/entities";

const company = makeCompany(COMPANY_SPECS[0]);
const other = makeCompany(COMPANY_SPECS[1]);

function useHarness(initialSelected: string | null = null) {
  const [activeSection, setActiveSection] = useState<Section>("Today");
  const [selectedCompanyId, setSelectedCompanyId] = useState<string | null>(initialSelected);
  // A ref (not a plain const) — it must survive the re-render `setActiveSection`
  // triggers, or `result.current` after `act()` would read a fresh empty array.
  const openToolCallsRef = useRef<Array<{ companyId: string; tool: Tool }>>([]);

  const actions = useCompanyEntryActions({
    companies: [company, other],
    cockpitInitialCompanyId: null,
    pinnedCompanyIds: [],
    selectedCompanyId,
    setSelectedCompanyId,
    setActiveSection,
    setActiveCockpitLayoutId: () => {},
    setCockpitInitialCompanyId: () => {},
    openTool: (companyId, tool) => {
      openToolCallsRef.current.push({ companyId, tool });
    },
  });
  const openToolCalls = openToolCallsRef.current;

  return { actions, activeSection, selectedCompanyId, openToolCalls };
}

// F3a S3 (ADR 0107, plan "Mapowanie WSZYSTKICH intencji"): every
// CompanyWorkspaceTab intent lands on its typed Spółka tool.
describe("useCompanyEntryActions — openCompanyWorkspaceById tab mapping", () => {
  const tabToTool: Array<[CompanyWorkspaceTab, Tool | null]> = [
    ["Feed", null],
    ["Notebook", { t: "notatnik" }],
    ["Claims", { t: "tezy" }],
    ["Fundamentals", { t: "fundamenty" }],
    ["Quality", { t: "jakosc" }],
    ["Metadata", { t: "akcjonariat" }],
    ["Transcripts", null],
  ];

  it.each(tabToTool)("tab %s lands on its typed Spółka tool", (tab, expectedTool) => {
    const { result } = renderHook(() => useHarness());

    act(() => {
      result.current.actions.openCompanyWorkspaceById(company.id, tab);
    });

    if (tab === "Transcripts") {
      // The only tab with no company-scoped Spółka tool yet — its legacy
      // global route.
      expect(result.current.activeSection).toBe("Transcripts");
    } else {
      expect(result.current.activeSection).toBe("Spolka");
    }
    expect(result.current.selectedCompanyId).toBe(company.id);
    expect(result.current.openToolCalls).toEqual(
      expectedTool ? [{ companyId: company.id, tool: expectedTool }] : [],
    );
  });

  it("no tab: lands on the Spółka core, no tool opened", () => {
    const { result } = renderHook(() => useHarness());

    act(() => {
      result.current.actions.openCompanyWorkspaceById(company.id);
    });

    expect(result.current.activeSection).toBe("Spolka");
    expect(result.current.openToolCalls).toEqual([]);
  });
});

// F3a S3 (ADR 0107 amendment, consent 3): Spółka mode opens the last-viewed
// company, else the first pinned, else the first tracked company; zero
// companies lands on Companies, never a blank Spółka.
describe("useCompanyEntryActions — openSpolkaMode", () => {
  it("opens the last-viewed (selected) company when one exists", () => {
    const { result } = renderHook(() => useHarness(other.id));

    act(() => {
      result.current.actions.openSpolkaMode();
    });

    expect(result.current.activeSection).toBe("Spolka");
    expect(result.current.selectedCompanyId).toBe(other.id);
  });

  it("falls back to the first pinned company when nothing is selected", () => {
    const { result } = renderHook(() =>
      (function useThisHarness() {
        const [activeSection, setActiveSection] = useState<Section>("Today");
        const [selectedCompanyId, setSelectedCompanyId] = useState<string | null>(null);
        const actions = useCompanyEntryActions({
          companies: [company, other],
          cockpitInitialCompanyId: null,
          pinnedCompanyIds: [other.id],
          selectedCompanyId,
          setSelectedCompanyId,
          setActiveSection,
          setActiveCockpitLayoutId: () => {},
          setCockpitInitialCompanyId: () => {},
          openTool: () => {},
        });
        return { actions, activeSection, selectedCompanyId };
      })(),
    );

    act(() => {
      result.current.actions.openSpolkaMode();
    });

    expect(result.current.activeSection).toBe("Spolka");
    expect(result.current.selectedCompanyId).toBe(other.id);
  });

  it("falls back to the first tracked company when nothing selected or pinned", () => {
    const { result } = renderHook(() => useHarness());

    act(() => {
      result.current.actions.openSpolkaMode();
    });

    expect(result.current.activeSection).toBe("Spolka");
    expect(result.current.selectedCompanyId).toBe(company.id);
  });

  it("lands on Companies (never a blank Spółka) with zero tracked companies", () => {
    const { result } = renderHook(() => {
      const [activeSection, setActiveSection] = useState<Section>("Today");
      const [selectedCompanyId, setSelectedCompanyId] = useState<string | null>(null);
      const actions = useCompanyEntryActions({
        companies: [],
        cockpitInitialCompanyId: null,
        pinnedCompanyIds: [],
        selectedCompanyId,
        setSelectedCompanyId,
        setActiveSection,
        setActiveCockpitLayoutId: () => {},
        setCockpitInitialCompanyId: () => {},
        openTool: () => {},
      });
      return { actions, activeSection };
    });

    act(() => {
      result.current.actions.openSpolkaMode();
    });

    expect(result.current.activeSection).toBe("Companies");
  });
});
