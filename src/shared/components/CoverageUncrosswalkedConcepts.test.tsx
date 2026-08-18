import { describe, it, expect, vi, beforeEach } from "vitest";
import { render, screen } from "@testing-library/react";
import userEvent from "@testing-library/user-event";

import { CoverageUncrosswalkedConcepts } from "./CoverageUncrosswalkedConcepts";
import { listUncrosswalkedConcepts, promoteUncrosswalkedConcept } from "../../api/taggedFactPromotion";
import type { UncrosswalkedConceptRow } from "../../api/taggedFactPromotion";

vi.mock("../../api/taggedFactPromotion", () => ({
  listUncrosswalkedConcepts: vi.fn(),
  promoteUncrosswalkedConcept: vi.fn(),
}));

const listUncrosswalkedConceptsMock = vi.mocked(listUncrosswalkedConcepts);
const promoteUncrosswalkedConceptMock = vi.mocked(promoteUncrosswalkedConcept);

function row(overrides: Partial<UncrosswalkedConceptRow> = {}): UncrosswalkedConceptRow {
  return {
    conceptLocalName: "DeferredTaxLiabilities",
    conceptNamespaceUri: "http://xbrl.ifrs.org/taxonomy/2023/ifrs-full",
    companyCount: 6,
    occurrenceCount: 12,
    statementGroup: "balance",
    periodNature: "instant",
    humanLabel: "DeferredTaxLiabilities",
    labelSource: "technical",
    alreadyPromoted: false,
    promotedDefinitionId: null,
    ...overrides,
  };
}

describe("CoverageUncrosswalkedConcepts", () => {
  beforeEach(() => {
    vi.clearAllMocks();
  });

  it("ranks the list by the order the read model returns (company count, then narrower)", async () => {
    listUncrosswalkedConceptsMock.mockResolvedValue([
      row({ conceptLocalName: "WidelyTagged", companyCount: 6, humanLabel: "WidelyTagged" }),
      row({ conceptLocalName: "NarrowlyTagged", companyCount: 1, humanLabel: "NarrowlyTagged" }),
    ]);
    render(<CoverageUncrosswalkedConcepts companyId="company_gpw_cdr" />);

    const titles = await screen.findAllByRole("listitem");
    expect(titles[0]).toHaveTextContent("WidelyTagged");
    expect(titles[1]).toHaveTextContent("NarrowlyTagged");
    expect(listUncrosswalkedConceptsMock).toHaveBeenCalledWith("company_gpw_cdr");
  });

  it("shows the technical name with an explicit no-translation marker for a standard concept", async () => {
    listUncrosswalkedConceptsMock.mockResolvedValue([row({ labelSource: "technical" })]);
    render(<CoverageUncrosswalkedConcepts companyId="company_gpw_cdr" />);

    expect(await screen.findByText("DeferredTaxLiabilities")).toBeInTheDocument();
    expect(screen.getByText(/no translation yet/)).toBeInTheDocument();
  });

  it("shows the issuer's own published label for an extension concept, never a synthesized one", async () => {
    listUncrosswalkedConceptsMock.mockResolvedValue([
      row({
        conceptLocalName: "PozostaleUslugiObce",
        humanLabel: "Pozostałe usługi obce",
        labelSource: "issuer",
      }),
    ]);
    render(<CoverageUncrosswalkedConcepts companyId="company_gpw_cdr" />);

    expect(await screen.findByText("Pozostałe usługi obce")).toBeInTheDocument();
    expect(screen.queryByText(/no translation yet/)).not.toBeInTheDocument();
  });

  it("promoting a row calls the command and the row reflects the result", async () => {
    const user = userEvent.setup();
    listUncrosswalkedConceptsMock.mockResolvedValue([row()]);
    promoteUncrosswalkedConceptMock.mockResolvedValue({
      definitionId: "kpidef_company_x_deferredtaxliabilities",
      metricKey: "DeferredTaxLiabilities",
      label: "DeferredTaxLiabilities",
      labelSource: "technical",
      factsProjected: 12,
    });
    render(<CoverageUncrosswalkedConcepts companyId="company_gpw_cdr" />);

    const promoteButton = await screen.findByRole("button", { name: /Show in Fundamentals/ });
    await user.click(promoteButton);

    expect(promoteUncrosswalkedConceptMock).toHaveBeenCalledWith(
      "company_gpw_cdr",
      "DeferredTaxLiabilities",
    );
    expect(await screen.findByText("In Fundamentals")).toBeInTheDocument();
    expect(screen.queryByRole("button", { name: /Show in Fundamentals/ })).not.toBeInTheDocument();
  });

  it("an already-promoted row shows the in-Fundamentals state, not a promote action", async () => {
    listUncrosswalkedConceptsMock.mockResolvedValue([
      row({ alreadyPromoted: true, promotedDefinitionId: "kpidef_x" }),
    ]);
    render(<CoverageUncrosswalkedConcepts companyId="company_gpw_cdr" />);

    expect(await screen.findByText("In Fundamentals")).toBeInTheDocument();
    expect(screen.queryByRole("button", { name: /Show in Fundamentals/ })).not.toBeInTheDocument();
  });

  it("shows a reassuring empty state when nothing is captured yet", async () => {
    listUncrosswalkedConceptsMock.mockResolvedValue([]);
    render(<CoverageUncrosswalkedConcepts companyId="company_gpw_cdr" />);

    expect(
      await screen.findByText(
        "Nothing captured yet that the program doesn't already know how to name.",
      ),
    ).toBeInTheDocument();
  });

  it("shows an explicit, retryable error when the read fails", async () => {
    listUncrosswalkedConceptsMock.mockRejectedValueOnce(new Error("read exploded"));
    render(<CoverageUncrosswalkedConcepts companyId="company_gpw_cdr" />);

    expect(await screen.findByText("Couldn't load unnamed positions.")).toBeInTheDocument();
    expect(screen.getByText("read exploded")).toBeInTheDocument();
  });
});
