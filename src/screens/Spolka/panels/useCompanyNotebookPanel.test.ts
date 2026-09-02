import { createElement, type ReactNode } from "react";
import { describe, it, expect, vi, beforeEach } from "vitest";
import { renderHook, act, waitFor, screen, fireEvent } from "@testing-library/react";
import type { Company, NotebookEntry } from "../../../api/types";
import { createNotebookEntry, deleteNotebookEntry, listNotebookEntries } from "../../../api/notebooks";
import { ToastProvider } from "../../../ui";
import { useCompanyNotebookPanel } from "./useCompanyNotebookPanel";

vi.mock("../../../api/notebooks", () => ({
  createNotebookEntry: vi.fn(),
  deleteNotebookEntry: vi.fn(),
  listNotebookEntries: vi.fn(),
}));

const createNotebookEntryMock = vi.mocked(createNotebookEntry);
const deleteNotebookEntryMock = vi.mocked(deleteNotebookEntry);
const listNotebookEntriesMock = vi.mocked(listNotebookEntries);

// `useCompanyNotebookPanel` calls `useUndoableDelete()` (ADR 0076 D5), which
// reads `useToast()` from context — every render needs the provider.
function wrapper({ children }: { children: ReactNode }) {
  return createElement(ToastProvider, null, children);
}

const company = { id: "c1", qualifiedTicker: "GPW:CDR", displayName: "CD PROJEKT S.A." } as Company;

function entry(overrides: Partial<NotebookEntry> = {}): NotebookEntry {
  return {
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
    ...overrides,
  };
}

describe("useCompanyNotebookPanel (F4c S2, ADR 0108 amendment)", () => {
  beforeEach(() => {
    vi.clearAllMocks();
    listNotebookEntriesMock.mockResolvedValue([]);
  });

  // sol re-review (item B): the tool's `initialDraft` (an Inbox feed-item
  // note draft) must persist its full provenance when saved — before this
  // fix `createNotebookEntry` always used `manualNotebookOrigins()`,
  // silently discarding the feed_item origin the draft carried.
  it("a feed-item draft opens the composer prefilled and persists the full origin on create", async () => {
    const created = entry({ id: "n2", title: "Q1 report note" });
    createNotebookEntryMock.mockResolvedValue(created);

    // Declared outside the render callback so its identity stays stable
    // across re-renders (`navigateToCompanyNotebook`'s real caller — the
    // Spółka tool's committed `Tool` state — is likewise stable until a NEW
    // navigation constructs a fresh draft; an inline literal here would
    // recreate the object every render and defeat the one-shot effect).
    const draft = {
      form: {
        title: "Q1 report note",
        body: "Revenue beat.",
        tags: "feed, official-report",
        kind: "observation",
        claimStatus: "",
        eventDate: "",
        followUpAfter: "",
        followUpDate: "",
      },
      origins: [
        {
          sourceType: "feed_item",
          sourceId: "feed_1",
          sourceUrl: "https://example.test/report",
          label: "GPW ESPI/EBI: Q1 report",
        },
      ],
    };

    const { result } = renderHook(() => useCompanyNotebookPanel(company, { initialDraft: draft }), { wrapper });

    await waitFor(() => expect(listNotebookEntriesMock).toHaveBeenCalledTimes(1));

    expect(result.current.isComposerOpen).toBe(true);
    expect(result.current.notebookForm.title).toBe("Q1 report note");

    act(() => result.current.createNotebookEntry());

    await waitFor(() => expect(createNotebookEntryMock).toHaveBeenCalledTimes(1));
    expect(createNotebookEntryMock).toHaveBeenCalledWith(
      expect.objectContaining({
        companyId: "c1",
        title: "Q1 report note",
        origins: [
          {
            sourceType: "feed_item",
            sourceId: "feed_1",
            sourceUrl: "https://example.test/report",
            label: "GPW ESPI/EBI: Q1 report",
          },
        ],
      }),
    );
  });

  it("with no draft, the composer stays closed and a manual create uses manual provenance", async () => {
    const created = entry({ id: "n3", title: "Manual note" });
    createNotebookEntryMock.mockResolvedValue(created);

    const { result } = renderHook(() => useCompanyNotebookPanel(company), { wrapper });
    await waitFor(() => expect(listNotebookEntriesMock).toHaveBeenCalledTimes(1));

    expect(result.current.isComposerOpen).toBe(false);

    act(() => result.current.updateNotebookForm("title", "Manual note"));
    act(() => result.current.createNotebookEntry());

    await waitFor(() => expect(createNotebookEntryMock).toHaveBeenCalledTimes(1));
    expect(createNotebookEntryMock).toHaveBeenCalledWith(
      expect.objectContaining({
        companyId: "c1",
        origins: [{ sourceType: "manual", sourceId: null, sourceUrl: null, label: "Manual note" }],
      }),
    );
  });

  // Deep-link selection (F4c S2): `highlightEntryId` selects the target
  // entry once it has loaded, so the detail pane opens on it.
  it("highlightEntryId selects the target entry once entries load", async () => {
    const target = entry({ id: "n4" });
    listNotebookEntriesMock.mockResolvedValue([target]);

    const { result } = renderHook(() => useCompanyNotebookPanel(company, { highlightEntryId: "n4" }), { wrapper });

    await waitFor(() => expect(result.current.selectedEntry).toEqual(target));
  });

  // The per-company panel is the ONLY notebook surface after F4c S2 (ADR
  // 0108 amendment retires the global Notebooks screen) — it must keep the
  // reversible-destroy capability (ADR 0076 D5) the retired screen carried:
  // immediate delete + undo toast, restoring via a faithful re-create.
  it("deletes the selected entry immediately and restores it faithfully on undo", async () => {
    const target = entry({
      id: "n5",
      title: "To delete",
      body: "Body to restore",
      tags: ["thesis", "q3"],
      kind: "risk",
      claimStatus: "pending",
      eventDate: "2026-07-01",
      followUpAfter: "P1M",
      followUpDate: "2026-08-01",
      origins: [
        {
          id: "origin_1",
          createdAt: "2026-06-01T00:00:00Z",
          sourceType: "feed_item",
          sourceId: "feed_9",
          sourceUrl: "https://example.test/feed/9",
          label: "GPW ESPI/EBI: Q3 report",
        },
      ],
    });
    listNotebookEntriesMock.mockResolvedValue([target]);
    deleteNotebookEntryMock.mockResolvedValue(undefined);
    createNotebookEntryMock.mockResolvedValue(entry({ id: "n6", title: "To delete" }));

    const { result } = renderHook(() => useCompanyNotebookPanel(company, { highlightEntryId: "n5" }), { wrapper });
    await waitFor(() => expect(result.current.selectedEntry).toEqual(target));

    act(() => result.current.deleteNotebookEntry());

    await waitFor(() => expect(deleteNotebookEntryMock).toHaveBeenCalledWith("n5"));
    await waitFor(() => expect(result.current.selectedEntry).toBeNull());

    // sol fix1 item 8: trigger the toast's Undo action and assert the
    // restore call carries the deleted entry's FULL form AND origins — not
    // merely that the toast/undo affordance exists.
    fireEvent.click(await screen.findByRole("button", { name: "Undo" }));

    await waitFor(() =>
      expect(createNotebookEntryMock).toHaveBeenCalledWith({
        companyId: "c1",
        title: "To delete",
        body: "Body to restore",
        bodyFormat: "markdown",
        tags: ["thesis", "q3"],
        kind: "risk",
        claimStatus: "pending",
        eventDate: "2026-07-01",
        followUpAfter: "P1M",
        followUpDate: "2026-08-01",
        origins: [
          {
            sourceType: "feed_item",
            sourceId: "feed_9",
            sourceUrl: "https://example.test/feed/9",
            label: "GPW ESPI/EBI: Q3 report",
          },
        ],
      }),
    );
  });

  // Found by tests/browser/notebooks.spec.ts (create → edit → Save hung: the
  // Save button stayed permanently disabled). `setSelectedEntryId(created.id)`
  // and `refresh()` fire in the same batch, so the render where
  // `selectedEntryId` first changes can still read the PRE-refresh `entries`
  // array — the edit-form seed effect must reseed once `selectedEntry`
  // actually resolves, not only on a `selectedEntryId` change.
  it("seeds the edit form correctly for a just-created entry (create-then-select race)", async () => {
    const created = entry({ id: "n7", title: "Race note", body: "Race body" });
    listNotebookEntriesMock.mockResolvedValueOnce([]).mockResolvedValueOnce([created]);
    createNotebookEntryMock.mockResolvedValue(created);

    const { result } = renderHook(() => useCompanyNotebookPanel(company), { wrapper });
    await waitFor(() => expect(listNotebookEntriesMock).toHaveBeenCalledTimes(1));

    act(() => result.current.updateNotebookForm("title", "Race note"));
    act(() => result.current.updateNotebookForm("body", "Race body"));
    act(() => result.current.createNotebookEntry());

    await waitFor(() => expect(result.current.selectedEntry).toEqual(created));
    act(() => result.current.setEditMode(true));

    expect(result.current.editForm.title).toBe("Race note");
    expect(result.current.editForm.body).toBe("Race body");
  });
});
