import { describe, it, expect, vi } from "vitest";
import { render, screen } from "@testing-library/react";
import userEvent from "@testing-library/user-event";
import { axe } from "jest-axe";
import type { Company } from "../../../api/types";
import type { DecisionEntry } from "../../../api/decisionJournal";
import type { DecisionJournalSectionProps } from "./DecisionJournalSection";
import { DecisionJournalSection } from "./DecisionJournalSection";

const company = { id: "c1", qualifiedTicker: "GPW:CDR", displayName: "CD PROJEKT S.A." } as Company;

const entry: DecisionEntry = {
  id: "d1",
  companyId: "c1",
  kind: "buy",
  rationaleMd: "Strong pipeline, entering a position.",
  decidedAt: "2026-06-01",
  supersededByEntryId: null,
  createdAt: "2026-06-01T00:00:00Z",
};

const noop = () => {};

const baseProps: DecisionJournalSectionProps = {
  company,
  entries: [entry],
  isComposerOpen: false,
  form: { kind: "buy", rationaleMd: "", decidedAt: "2026-06-10" },
  supersedingEntry: null,
  selectedEntry: null,
  evidenceCandidates: [],
  linkedEvidenceKeys: new Set(),
  error: null,
  setComposerOpen: noop,
  updateForm: noop,
  createEntry: noop,
  startSupersede: noop,
  cancelSupersede: noop,
  setSelectedEntryId: noop,
  linkEvidence: noop,
  formatTimestamp: (value) => value ?? "",
};

describe("DecisionJournalSection (ADR 0071, J3)", () => {
  it("lists entries by their decision kind and date", () => {
    render(<DecisionJournalSection {...baseProps} />);
    expect(
      screen.getByRole("button", { name: /Select decision entry: Buy 2026-06-01/ }),
    ).toBeInTheDocument();
  });

  // ADR 0071: entries are immutable — there must be NO edit or delete affordance
  // anywhere, on the list row or the detail. The only correction is "Supersede".
  it("exposes no edit or delete affordance for a selected entry", () => {
    render(<DecisionJournalSection {...baseProps} selectedEntry={entry} />);
    expect(screen.queryByRole("button", { name: /Edit/i })).not.toBeInTheDocument();
    expect(screen.queryByRole("button", { name: /Delete/i })).not.toBeInTheDocument();
    expect(screen.queryByRole("button", { name: /Remove/i })).not.toBeInTheDocument();
    // The immutable correction path is present instead.
    expect(screen.getByRole("button", { name: "Supersede" })).toBeInTheDocument();
  });

  it("starts a supersede follow-up from the selected entry", async () => {
    const user = userEvent.setup();
    const startSupersede = vi.fn();
    render(
      <DecisionJournalSection {...baseProps} selectedEntry={entry} startSupersede={startSupersede} />,
    );
    await user.click(screen.getByRole("button", { name: "Supersede" }));
    expect(startSupersede).toHaveBeenCalledWith(entry);
  });

  // v0.52 dogfooding fix: the composer date field must render through the
  // DateField primitive (design-system box), not a raw native date input.
  it("renders the composer date control via the DateField primitive", () => {
    render(<DecisionJournalSection {...baseProps} isComposerOpen />);
    const dateInput = screen.getByLabelText("Decision date");
    expect(dateInput).toHaveAttribute("type", "date");
    expect(dateInput).toHaveClass("ui-date-input");
    expect(dateInput).toHaveValue("2026-06-10");
  });

  it("opens the composer from the New entry action", async () => {
    const user = userEvent.setup();
    const setComposerOpen = vi.fn();
    render(<DecisionJournalSection {...baseProps} setComposerOpen={setComposerOpen} />);
    await user.click(screen.getByRole("button", { name: "New entry" }));
    expect(setComposerOpen).toHaveBeenCalledWith(true);
  });

  it("renders an empty state when there are no entries", () => {
    render(<DecisionJournalSection {...baseProps} entries={[]} />);
    expect(screen.getByText(/No decisions recorded for/)).toBeInTheDocument();
  });

  it("keeps a clean accessibility baseline with the composer open", async () => {
    const { container } = render(
      <DecisionJournalSection {...baseProps} isComposerOpen selectedEntry={entry} />,
    );
    const results = await axe(container, {
      rules: {
        region: { enabled: false },
        "color-contrast": { enabled: false },
        "heading-order": { enabled: false },
      },
    });
    expect(results.violations.map((violation) => violation.id)).toEqual([]);
  });
});
