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
  deleteNotebookEntry: noop,
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

  // U7-C density contract (ADR 0076 D6): at the S width / short height tiers the
  // panel is single-column — selecting a note swaps the list for the detail with
  // a back-to-list affordance. jsdom has no container queries, so we assert the
  // toggle STATE (the data flag the tier CSS keys off + the back control); the
  // visual collapse is browser-tested.
  it("marks detail-open and shows a back-to-list control when a note is selected", async () => {
    const user = userEvent.setup();
    const setSelectedNotebookEntryId = vi.fn();

    const { container, rerender } = render(
      <CompanyNotebookSection
        {...baseProps}
        notebookEntries={[entry]}
        isComposerOpen={false}
        selectedNotebookEntry={null}
        notebookEditMode={false}
        isNotebookEditDirty={false}
        setComposerOpen={noop}
        setNotebookEditMode={noop}
      />,
    );

    expect(container.querySelector(".notebook-workspace")).not.toHaveAttribute("data-detail-open");
    expect(screen.queryByRole("button", { name: "Back to note list" })).not.toBeInTheDocument();

    rerender(
      <CompanyNotebookSection
        {...baseProps}
        notebookEntries={[entry]}
        isComposerOpen={false}
        selectedNotebookEntry={entry}
        notebookEditMode={false}
        isNotebookEditDirty={false}
        setComposerOpen={noop}
        setSelectedNotebookEntryId={setSelectedNotebookEntryId}
        setNotebookEditMode={noop}
      />,
    );

    expect(container.querySelector(".notebook-workspace")).toHaveAttribute("data-detail-open");
    const back = screen.getByRole("button", { name: "Back to note list" });
    await user.click(back);
    expect(setSelectedNotebookEntryId).toHaveBeenCalledWith(null);
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

  // The per-company panel is the ONLY notebook surface after F4c S2 (ADR
  // 0108 amendment retires the global Notebooks screen) — it must keep the
  // delete capability the retired screen carried (ADR 0076 D5: reversible,
  // no blocking confirm).
  it("deletes the selected note", async () => {
    const user = userEvent.setup();
    const deleteNotebookEntry = vi.fn();

    render(
      <CompanyNotebookSection
        {...baseProps}
        notebookEntries={[entry]}
        isComposerOpen={false}
        selectedNotebookEntry={entry}
        notebookEditMode={false}
        isNotebookEditDirty={false}
        setComposerOpen={noop}
        setNotebookEditMode={noop}
        deleteNotebookEntry={deleteNotebookEntry}
      />,
    );

    await user.click(screen.getByRole("button", { name: "Delete" }));
    expect(deleteNotebookEntry).toHaveBeenCalledTimes(1);
  });

  // Deep-link navigation (F4c S2, ADR 0108 amendment): the Spółka `notatnik`
  // tool's `entryId` param must actually land on that entry —
  // `CompanyReportDocumentsPanel`'s `highlightDocumentRef` precedent.
  it("highlighted entry scrolls into view and is marked", () => {
    const scrollIntoView = vi.fn();
    Element.prototype.scrollIntoView = scrollIntoView;

    render(
      <CompanyNotebookSection
        {...baseProps}
        highlightEntryId="n1"
        notebookEntries={[entry]}
        isComposerOpen={false}
        selectedNotebookEntry={null}
        notebookEditMode={false}
        isNotebookEditDirty={false}
        setComposerOpen={noop}
        setNotebookEditMode={noop}
      />,
    );

    const row = screen.getByRole("button", { name: /Select notebook entry: Margins thesis/ });
    expect(row).toHaveAttribute("data-notebook-entry-id", "n1");
    expect(row).toHaveAttribute("data-notebook-entry-highlighted", "true");
    expect(scrollIntoView).toHaveBeenCalledWith({ block: "nearest" });
  });

  it("does not highlight anything when no entry is targeted", () => {
    render(
      <CompanyNotebookSection
        {...baseProps}
        notebookEntries={[entry]}
        isComposerOpen={false}
        selectedNotebookEntry={null}
        notebookEditMode={false}
        isNotebookEditDirty={false}
        setComposerOpen={noop}
        setNotebookEditMode={noop}
      />,
    );

    expect(document.querySelector("[data-notebook-entry-highlighted]")).toBeNull();
  });
});
