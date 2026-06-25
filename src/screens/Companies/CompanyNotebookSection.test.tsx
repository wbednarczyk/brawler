import { describe, it, expect, vi } from "vitest";
import { render, screen } from "@testing-library/react";
import userEvent from "@testing-library/user-event";
import type { Company, NotebookEntry } from "../../api/types";
import type { NotebookDateLikeFieldProps, MarkdownNoteBodyProps } from "../../shared/types/notebook";
import { CompanyNotebookSection } from "./CompanyNotebookSection";

const company = { id: "c1", qualifiedTicker: "GPW:CDR", displayName: "CD PROJEKT S.A." } as Company;

const entry: NotebookEntry = {
  id: "n1",
  companyId: "c1",
  title: "Margins thesis",
  body: "Watch gross margin trend.",
  bodyFormat: "markdown",
  tags: ["thesis"],
  kind: "observation",
  claimStatus: null,
  eventDate: null,
  followUpAfter: null,
  followUpDate: null,
  createdAt: "2026-06-01T00:00:00Z",
  updatedAt: "2026-06-01T00:00:00Z",
  origins: [],
};

const DateStub = (_props: NotebookDateLikeFieldProps) => null;
const MarkdownStub = ({ body }: MarkdownNoteBodyProps) => <div>{body}</div>;
const noop = () => {};

const baseProps = {
  company,
  notebookForm: { title: "", body: "", tags: "", kind: "manual", claimStatus: "", eventDate: "", followUpAfter: "", followUpDate: "" },
  notebookEditForm: { title: "", body: "", tags: "", kind: "manual", claimStatus: "", eventDate: "", followUpAfter: "", followUpDate: "" },
  notebookError: null,
  updateNotebookForm: noop,
  createNotebookEntry: noop,
  setSelectedNotebookEntryId: noop,
  saveNotebookEntry: noop,
  cancelNotebookEdit: noop,
  updateNotebookEditForm: noop,
  NotebookDateField: DateStub,
  NotebookQuarterField: DateStub,
  MarkdownNoteBody: MarkdownStub,
  renderNotebookOrigins: () => null,
};

describe("CompanyNotebookSection (ADR 0057 dashboard panel)", () => {
  it("toggles the composer open from the New note action", async () => {
    const user = userEvent.setup();
    const setComposerOpen = vi.fn();

    render(
      <CompanyNotebookSection
        {...baseProps}
        notebookEntries={[entry]}
        isComposerOpen={false}
        selectedNotebookEntry={null}
        notebookEditMode={false}
        isNotebookEditDirty={false}
        setComposerOpen={setComposerOpen}
        setNotebookEditMode={noop}
      />,
    );

    expect(screen.getByRole("button", { name: /Select notebook entry: Margins thesis/ })).toBeInTheDocument();
    await user.click(screen.getByRole("button", { name: "New note" }));
    expect(setComposerOpen).toHaveBeenCalledTimes(1);
  });

  it("enters edit mode for the selected note", async () => {
    const user = userEvent.setup();
    const setNotebookEditMode = vi.fn();

    render(
      <CompanyNotebookSection
        {...baseProps}
        notebookEntries={[entry]}
        isComposerOpen={false}
        selectedNotebookEntry={entry}
        notebookEditMode={false}
        isNotebookEditDirty={false}
        setComposerOpen={noop}
        setNotebookEditMode={setNotebookEditMode}
      />,
    );

    expect(screen.getByText("Watch gross margin trend.")).toBeInTheDocument();
    await user.click(screen.getByRole("button", { name: "Edit" }));
    expect(setNotebookEditMode).toHaveBeenCalledWith(true);
  });
});
