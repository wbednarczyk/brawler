import { describe, it, expect } from "vitest";
import { render, within } from "@testing-library/react";

import { ResearchScopeBar } from "./ResearchScopeBar";

// The `ai_analysis` evidence producer was removed with the in-app AI layer
// (ADR 0084). Its scope-bar filter chip now filters to a forever-empty list, so
// it must not be offered. This pins the option set against a silent re-add.
function renderScopeBar() {
  return render(
    <ResearchScopeBar
      companies={[]}
      watchlists={[]}
      mode="company"
      selectedCompanyId={null}
      selectedWatchlistId={null}
      selectedEvidenceTypes={[]}
      changedOnly={false}
      cascadeToCompanies={false}
      setMode={() => {}}
      setSelectedCompanyId={() => {}}
      setSelectedWatchlistId={() => {}}
      setChangedOnly={() => {}}
      setCascadeToCompanies={() => {}}
      toggleEvidenceType={() => {}}
      clearEvidenceTypes={() => {}}
      text={(value) => value}
    />,
  );
}

describe("ResearchScopeBar evidence-type options", () => {
  it("does not offer the retired AI analysis filter", () => {
    const { getByLabelText } = renderScopeBar();
    const filters = getByLabelText("Evidence type filters");
    expect(within(filters).queryByRole("button", { name: "AI analysis" })).toBeNull();
  });

  it("still offers the live evidence-type filters", () => {
    const { getByLabelText } = renderScopeBar();
    const filters = getByLabelText("Evidence type filters");
    for (const label of ["Feed items", "Notes", "Claims", "Events", "Transcripts", "Signals"]) {
      expect(within(filters).getByRole("button", { name: label })).toBeTruthy();
    }
  });
});
