import { useCallback, useEffect, useMemo, useState } from "react";
import * as notebooksApi from "../../../api/notebooks";
import type { CreateNotebookEntryInput } from "../../../api/notebooks";
import type { Company, NotebookDraftOrigin, NotebookEntry } from "../../../api/types";
import type { NotebookForm } from "../../../shared/types/notebook";
import type { NotebookDraft } from "../route";
import { useLocale } from "../../../shared/locale";
import { useUndoableDelete } from "../../../ui";
import {
  emptyNotebookForm,
  manualNotebookOrigins,
  notebookCreateInput,
  notebookFormFromEntry,
  notebookUpdateInput,
} from "../../../app/notebookForms";

// Faithful undo restore (ADR 0076 D5): rebuild the create-input from a
// deleted entry, carrying content and provenance origins, dropping the
// server-owned id/createdAt. Mirrors the retired Notebooks-global screen's
// `deleteNotebookScreenEntry` (F4c S2, ADR 0108 amendment) — the per-company
// panel is now the ONLY notebook surface, so it must keep this capability.
function notebookEntryRestoreInput(entry: NotebookEntry): CreateNotebookEntryInput {
  return {
    companyId: entry.companyId,
    title: entry.title,
    body: entry.body,
    bodyFormat: entry.bodyFormat,
    tags: entry.tags,
    kind: entry.kind,
    claimStatus: entry.claimStatus,
    eventDate: entry.eventDate,
    followUpAfter: entry.followUpAfter,
    followUpDate: entry.followUpDate,
    origins: entry.origins.map((origin) => ({
      sourceType: origin.sourceType,
      sourceId: origin.sourceId,
      sourceUrl: origin.sourceUrl,
      label: origin.label,
    })),
  };
}

// Company-scoped notebook state for one company (ADR 0057). It owns the
// entry list, the composer + edit forms, and the create/save/edit commands via
// `api/notebooks` directly — the company-scoped subset of `useNotebookController`
// with none of its cross-screen (Inbox / Notebooks screen / transcript) coupling.
// Mirrors `useFundamentalsPanel` so the dashboard `companyNotebook` panel works
// for any pinned company.
export function useCompanyNotebookPanel(
  company: Company,
  options?: {
    /** Deep-link navigation (F4c S2): the entry to select once it loads
     * (`CompanyNotebookSection` scrolls + flashes it). */
    highlightEntryId?: string;
    /** A prefilled-but-unsaved note from a cross-screen caller (Inbox,
     * research evidence, transcript) — opens the composer seeded with it. */
    initialDraft?: NotebookDraft;
  },
) {
  const { text } = useLocale();
  const runUndoableDelete = useUndoableDelete();
  const { highlightEntryId, initialDraft } = options ?? {};
  const [entries, setEntries] = useState<NotebookEntry[]>([]);
  const [isComposerOpen, setComposerOpen] = useState(false);
  const [notebookForm, setNotebookForm] = useState<NotebookForm>(emptyNotebookForm);
  const [draftOrigins, setDraftOrigins] = useState<NotebookDraftOrigin[]>(manualNotebookOrigins);
  const [selectedEntryId, setSelectedEntryId] = useState<string | null>(null);
  const [editMode, setEditMode] = useState(false);
  const [editForm, setEditForm] = useState<NotebookForm>(emptyNotebookForm);
  const [error, setError] = useState<string | null>(null);

  const refresh = useCallback(() => {
    notebooksApi
      .listNotebookEntries(company.id)
      .then(setEntries)
      .catch((reason) => setError(String(reason)));
  }, [company.id]);

  useEffect(() => {
    setSelectedEntryId(null);
    setEditMode(false);
    refresh();
  }, [company.id, refresh]);

  // One-shot draft seed (F4c S2, sol re-review): keyed on the `initialDraft`
  // object's identity — a fresh object per `navigateToCompanyNotebook` call —
  // so a second "create note" deep link while already viewing the same
  // company still opens a fresh composer, but a plain re-render (same props)
  // never re-applies it over whatever the user has typed since.
  useEffect(() => {
    if (!initialDraft) return;
    setComposerOpen(true);
    setNotebookForm(initialDraft.form);
    setDraftOrigins(initialDraft.origins);
  }, [initialDraft]);

  // Deep-link selection (F4c S2): once the target entry has loaded, select it
  // so the detail pane opens on it — `CompanyNotebookSection` scrolls + flashes.
  useEffect(() => {
    if (!highlightEntryId) return;
    if (entries.some((entry) => entry.id === highlightEntryId)) {
      setSelectedEntryId(highlightEntryId);
    }
  }, [highlightEntryId, entries]);

  const selectedEntry = entries.find((entry) => entry.id === selectedEntryId) ?? null;

  // Seed the edit form from the selected entry whenever the selection changes
  // — and also when `selectedEntry` itself resolves (F4c S2 bugfix): after
  // create, `setSelectedEntryId(created.id)` and `refresh()` fire in the same
  // batch, so the render where `selectedEntryId` first changes can still read
  // the PRE-refresh `entries` array and resolve `selectedEntry` to null —
  // without this second key the seed effect never reruns once `entries`
  // catches up, leaving `editForm` stuck empty (Save permanently disabled,
  // caught by tests/browser/notebooks.spec.ts). `entries` only changes here
  // from this hook's own create/save/delete refreshes, never a background
  // poll, so reseeding on a genuine `selectedEntry` change is always correct.
  useEffect(() => {
    setEditForm(selectedEntry ? notebookFormFromEntry(selectedEntry) : emptyNotebookForm());
    setEditMode(false);
  }, [selectedEntryId, selectedEntry]);

  const isEditDirty = useMemo(
    () =>
      selectedEntry
        ? JSON.stringify(editForm) !== JSON.stringify(notebookFormFromEntry(selectedEntry))
        : false,
    [editForm, selectedEntry],
  );

  function updateNotebookForm(field: keyof NotebookForm, value: string) {
    setNotebookForm((current) => ({ ...current, [field]: value }));
  }

  function updateNotebookEditForm(field: keyof NotebookForm, value: string) {
    setEditForm((current) => ({ ...current, [field]: value }));
  }

  function createNotebookEntry(event?: { preventDefault: () => void }) {
    event?.preventDefault();
    notebooksApi
      .createNotebookEntry(notebookCreateInput(company.id, notebookForm, draftOrigins))
      .then((created) => {
        setNotebookForm(emptyNotebookForm());
        setDraftOrigins(manualNotebookOrigins());
        setComposerOpen(false);
        setSelectedEntryId(created.id);
        setError(null);
        refresh();
      })
      .catch((reason) => setError(String(reason)));
  }

  function saveNotebookEntry(event?: { preventDefault: () => void }) {
    event?.preventDefault();
    if (!selectedEntry) return;
    notebooksApi
      .updateNotebookEntry(notebookUpdateInput(selectedEntry.id, editForm))
      .then((updated) => {
        setEntries((current) => current.map((entry) => (entry.id === updated.id ? updated : entry)));
        setSelectedEntryId(updated.id);
        setEditMode(false);
        setError(null);
        refresh();
      })
      .catch((reason) => setError(String(reason)));
  }

  function cancelNotebookEdit() {
    if (selectedEntry) {
      setEditForm(notebookFormFromEntry(selectedEntry));
    }
    setEditMode(false);
  }

  // Reversible destroy (ADR 0076 D5): a note re-creates faithfully via
  // create_notebook_entry — content and provenance origins are preserved — so
  // it deletes immediately with an undo toast (the new id is inconsequential;
  // nothing references a note by id after deletion).
  function deleteNotebookEntry() {
    if (!selectedEntry) return;

    const entry = selectedEntry;

    runUndoableDelete({
      perform: () => notebooksApi.deleteNotebookEntry(entry.id),
      restore: () => notebooksApi.createNotebookEntry(notebookEntryRestoreInput(entry)),
      message: text("Note deleted"),
      undoLabel: text("Undo"),
      onPerformed: () => {
        setSelectedEntryId(null);
        setEditMode(false);
        setError(null);
        refresh();
      },
      onRestored: () => {
        refresh();
      },
      onError: (reason) => setError(String(reason)),
    });
  }

  return {
    company,
    entries,
    highlightEntryId,
    isComposerOpen,
    setComposerOpen,
    notebookForm,
    updateNotebookForm,
    createNotebookEntry,
    selectedEntry,
    setSelectedEntryId,
    editMode,
    setEditMode,
    editForm,
    updateNotebookEditForm,
    isEditDirty,
    saveNotebookEntry,
    cancelNotebookEdit,
    deleteNotebookEntry,
    error,
  };
}
