import { useRef, useState } from "react";
import { describe, expect, it } from "vitest";
import { act, renderHook } from "@testing-library/react";

import { useCompanyEntryActions } from "./useCompanyEntryActions";
import type { Section } from "./navigation";
import type { SpolkaTransition } from "./useSpolkaScreenWiring";
import type { CompanyWorkspaceTab } from "../screens/Companies/companyTypes";
import type { Tool } from "../screens/Spolka/route";
import { COMPANY_SPECS, makeCompany } from "../test/scenarios/entities";

const company = makeCompany(COMPANY_SPECS[0]);
const other = makeCompany(COMPANY_SPECS[1]);

function useHarness(initialSelected: string | null = null) {
  const [activeSection, setActiveSection] = useState<Section>("Today");
  const [selectedCompanyId, setSelectedCompanyId] = useState<string | null>(initialSelected);
  // A ref (not a plain const) — it must survive the re-render `navigate`
  // triggers, or `result.current` after `act()` would read a fresh empty array.
  const navigateCallsRef = useRef<SpolkaTransition[]>([]);

  const actions = useCompanyEntryActions({
    companies: [company, other],
    cockpitInitialCompanyId: null,
    pinnedCompanyIds: [],
    selectedCompanyId,
    setActiveSection,
    setActiveCockpitLayoutId: () => {},
    setCockpitInitialCompanyId: () => {},
    // A harness stand-in for AppStateRoot's real `navigate` (one guarded
    // commit) — records the transition AND applies its company/section, so
    // existing assertions on `activeSection`/`selectedCompanyId` still hold.
    navigate: (transition) => {
      navigateCallsRef.current.push(transition);
      setSelectedCompanyId(transition.companyId);
      setActiveSection(transition.section);
    },
  });
  const navigateCalls = navigateCallsRef.current;

  return { actions, activeSection, selectedCompanyId, navigateCalls };
}

// F3a S3 (ADR 0107, plan "Mapowanie WSZYSTKICH intencji"): every
// CompanyWorkspaceTab intent lands on its typed Spółka tool.
describe("useCompanyEntryActions — openCompanyWorkspaceById tab mapping", () => {
  const tabToTool: Array<[CompanyWorkspaceTab, Tool | undefined]> = [
    ["Feed", undefined],
    ["Notebook", { t: "notatnik" }],
    ["Claims", { t: "tezy" }],
    ["Fundamentals", { t: "fundamenty" }],
    ["Quality", { t: "jakosc" }],
    ["Metadata", { t: "akcjonariat" }],
  ];

  it.each(tabToTool)("tab %s lands on its typed Spółka tool via ONE navigate() call", (tab, expectedTool) => {
    const { result } = renderHook(() => useHarness());

    act(() => {
      result.current.actions.openCompanyWorkspaceById(company.id, tab);
    });

    expect(result.current.activeSection).toBe("Spolka");
    expect(result.current.selectedCompanyId).toBe(company.id);
    expect(result.current.navigateCalls).toEqual([
      { companyId: company.id, section: "Spolka", tool: expectedTool },
    ]);
  });

  // The only tab with no company-scoped Spółka tool yet — its legacy global
  // route, still committed atomically (sol R1 finding 3).
  it("tab Transcripts lands on the legacy Transcripts route via ONE navigate() call", () => {
    const { result } = renderHook(() => useHarness());

    act(() => {
      result.current.actions.openCompanyWorkspaceById(company.id, "Transcripts");
    });

    expect(result.current.activeSection).toBe("Transcripts");
    expect(result.current.selectedCompanyId).toBe(company.id);
    expect(result.current.navigateCalls).toEqual([{ companyId: company.id, section: "Transcripts" }]);
  });

  it("no tab: lands on the Spółka core, no tool opened", () => {
    const { result } = renderHook(() => useHarness());

    act(() => {
      result.current.actions.openCompanyWorkspaceById(company.id);
    });

    expect(result.current.activeSection).toBe("Spolka");
    expect(result.current.navigateCalls).toEqual([
      { companyId: company.id, section: "Spolka", tool: undefined },
    ]);
  });

  // sol R1 finding 3 (residual): the legacy dashboard is the same guarded
  // transition — no setter runs ahead of the guard.
  it("openAdvancedLayout commits the cockpit via ONE navigate() call", () => {
    const { result } = renderHook(() => useHarness());

    act(() => {
      result.current.actions.openAdvancedLayout(company.id);
    });

    expect(result.current.activeSection).toBe("Cockpit");
    expect(result.current.navigateCalls).toEqual([
      { companyId: company.id, section: "Cockpit", cockpitLayoutId: null },
    ]);
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
          setActiveSection,
          setActiveCockpitLayoutId: () => {},
          setCockpitInitialCompanyId: () => {},
          navigate: (transition) => {
            setSelectedCompanyId(transition.companyId);
            setActiveSection(transition.section);
          },
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
        setActiveSection,
        setActiveCockpitLayoutId: () => {},
        setCockpitInitialCompanyId: () => {},
        navigate: (transition) => {
          setSelectedCompanyId(transition.companyId);
          setActiveSection(transition.section);
        },
      });
      return { actions, activeSection };
    });

    act(() => {
      result.current.actions.openSpolkaMode();
    });

    expect(result.current.activeSection).toBe("Companies");
  });
});
