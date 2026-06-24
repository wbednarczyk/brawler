import { describe, it, expect } from "vitest";
import { render, renderHook, screen, act } from "@testing-library/react";
import type { ReactNode } from "react";

import { buildScenario } from "../../test/scenarios/scenarios";
import { CockpitSelectionProvider, useCockpitSelection } from "./CockpitSelectionContext";

// Unit coverage for the cockpit's shared-selection store (decision 6A, ADR 0053):
// the linked-panel "brain". selectFeedItem derives the company from the feed
// item's qualified ticker; selectCompany sets a company directly (the alternate
// source used by Watchlists / Report Season).

const data = buildScenario("minimal");

function wrapper({ children }: { children: ReactNode }) {
  return (
    <CockpitSelectionProvider companies={data.companies} feedItems={data.feedItems}>
      {children}
    </CockpitSelectionProvider>
  );
}

function SelectionProbe() {
  const { selection } = useCockpitSelection();
  return <span data-testid="selected">{selection.feedItemId ?? "none"}</span>;
}

describe("CockpitSelectionContext", () => {
  it("seeds the first feed item once the feed loads after mount (async data)", () => {
    // Real data loads after mount: the provider starts with an empty feed and
    // nothing selected, then the feed arrives and the first item must auto-select
    // so the linked panels are not blank.
    const { rerender } = render(
      <CockpitSelectionProvider companies={data.companies} feedItems={[]}>
        <SelectionProbe />
      </CockpitSelectionProvider>,
    );
    expect(screen.getByTestId("selected")).toHaveTextContent("none");

    rerender(
      <CockpitSelectionProvider companies={data.companies} feedItems={data.feedItems}>
        <SelectionProbe />
      </CockpitSelectionProvider>,
    );
    expect(screen.getByTestId("selected")).toHaveTextContent(data.feedItems[0].id);
  });

  it("opens scoped to a company when initialCompanyId is set (Advanced layout, ADR 0054)", () => {
    const company = data.companies[2];
    function ScopeProbe() {
      const { selection, selectedCompany } = useCockpitSelection();
      return (
        <span data-testid="scoped">{`${selection.feedItemId ?? "none"}:${selectedCompany?.id ?? "none"}`}</span>
      );
    }
    render(
      <CockpitSelectionProvider
        companies={data.companies}
        feedItems={data.feedItems}
        initialCompanyId={company.id}
      >
        <ScopeProbe />
      </CockpitSelectionProvider>,
    );
    // The company is focused directly; no feed item is auto-selected.
    expect(screen.getByTestId("scoped")).toHaveTextContent(`none:${company.id}`);
  });

  it("starts on the first feed item and derives its company", () => {
    const { result } = renderHook(() => useCockpitSelection(), { wrapper });
    const first = data.feedItems[0];
    expect(result.current.selection.feedItemId).toBe(first.id);
    const expected = data.companies.find((company) => company.qualifiedTicker === first.company);
    expect(result.current.selectedCompany?.id ?? null).toBe(expected?.id ?? null);
  });

  it("selectFeedItem moves the selection and re-derives the company", () => {
    const { result } = renderHook(() => useCockpitSelection(), { wrapper });
    const target = data.feedItems.find((item) =>
      data.companies.some((company) => company.qualifiedTicker === item.company),
    );
    expect(target).toBeDefined();
    act(() => result.current.selectFeedItem(target!.id));
    expect(result.current.selection.feedItemId).toBe(target!.id);
    expect(result.current.selectedCompany?.qualifiedTicker).toBe(target!.company);
  });

  it("selectCompany sets the company directly and clears the feed item", () => {
    const { result } = renderHook(() => useCockpitSelection(), { wrapper });
    const company = data.companies[0];
    act(() => result.current.selectCompany(company.id));
    expect(result.current.selection.companyId).toBe(company.id);
    expect(result.current.selection.feedItemId).toBeNull();
    expect(result.current.selectedCompany?.id).toBe(company.id);
  });
});
