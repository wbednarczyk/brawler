import { describe, it, expect, vi, beforeEach } from "vitest";
import { render, screen, waitFor } from "@testing-library/react";
import userEvent from "@testing-library/user-event";
import type { Company } from "../../api/types";
import type { DecisionEntry } from "../../api/decisionJournal";
import { listDecisionEntries } from "../../api/decisionJournal";
import { DecisionJournalGlobalPanel } from "./DecisionJournalGlobalPanel";

vi.mock("../../api/decisionJournal", () => ({
  listDecisionEntries: vi.fn(),
}));

const listDecisionEntriesMock = vi.mocked(listDecisionEntries);

const companies = [
  { id: "c1", qualifiedTicker: "GPW:CDR", displayName: "CD PROJEKT S.A." } as Company,
  { id: "c2", qualifiedTicker: "GPW:PKN", displayName: "ORLEN S.A." } as Company,
];

const entry: DecisionEntry = {
  id: "d1",
  companyId: "c1",
  kind: "buy",
  rationaleMd: "Entering a position on the pipeline.",
  decidedAt: "2026-06-01",
  supersededByEntryId: null,
  createdAt: "2026-06-01T00:00:00Z",
};

describe("DecisionJournalGlobalPanel (ADR 0071, J3)", () => {
  beforeEach(() => {
    vi.clearAllMocks();
    listDecisionEntriesMock.mockResolvedValue([entry]);
  });

  it("lists decisions across companies with the company ticker", async () => {
    render(<DecisionJournalGlobalPanel companies={companies} />);
    await waitFor(() => expect(screen.getByText("GPW:CDR")).toBeInTheDocument());
    expect(screen.getByText("Entering a position on the pipeline.")).toBeInTheDocument();
    // The global list opens unfiltered (no company/kind).
    expect(listDecisionEntriesMock).toHaveBeenCalledWith({});
  });

  it("marks the leading header compact (ADR 0076 D6) with the count in meta", async () => {
    // The dock tab already reads "Journal (all companies)"; the panel must not
    // repeat that identity as a visible in-pane heading (K3 double chrome). The
    // leading SectionHeader carries `paneLead` so `.cockpit-pane` clips its title
    // (kept in the accessible tree) — and the entry count moves to the meta slot,
    // which survives compaction, instead of the dropped subtitle.
    const { container } = render(<DecisionJournalGlobalPanel companies={companies} />);
    await waitFor(() => expect(screen.getByText("GPW:CDR")).toBeInTheDocument());
    const leadHeader = container.querySelector(".ui-section-header");
    expect(leadHeader).toHaveClass("ui-pane-lead-header");
    expect(leadHeader?.querySelector(".ui-section-header-meta")).toHaveTextContent("1");
    // Heading stays in the accessible tree so aria/axe still resolve the name.
    expect(screen.getByRole("heading", { name: "Decision journal" })).toBeInTheDocument();
  });

  it("refetches filtered by kind and company", async () => {
    const user = userEvent.setup();
    render(<DecisionJournalGlobalPanel companies={companies} />);
    await waitFor(() => expect(listDecisionEntriesMock).toHaveBeenCalledTimes(1));

    await user.selectOptions(screen.getByLabelText("Filter by decision kind"), "pass");
    await waitFor(() =>
      expect(listDecisionEntriesMock).toHaveBeenLastCalledWith({ kind: "pass" }),
    );

    await user.selectOptions(screen.getByLabelText("Filter by company"), "c2");
    await waitFor(() =>
      expect(listDecisionEntriesMock).toHaveBeenLastCalledWith({ kind: "pass", companyId: "c2" }),
    );
  });
});
