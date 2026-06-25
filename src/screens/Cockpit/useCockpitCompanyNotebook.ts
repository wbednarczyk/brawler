import { useCallback, useEffect, useMemo, useState } from "react";
import * as notebooksApi from "../../api/notebooks";
import type { Company, NotebookEntry } from "../../api/types";
import type { NotebookForm } from "../../shared/types/notebook";
import {
  emptyNotebookForm,
  manualNotebookOrigins,
  notebookCreateInput,
  notebookFormFromEntry,
  notebookUpdateInput,
} from "../../app/notebookForms";

// Cockpit-native company notebook state for one company (ADR 0057). It owns the
// entry list, the composer + edit forms, and the create/save/edit commands via
// `api/notebooks` directly — the company-scoped subset of `useNotebookController`
// with none of its cross-screen (Inbox / Notebooks screen / transcript) coupling.
// Mirrors `useCockpitFundamentals` so the dashboard `companyNotebook` panel works
// for any pinned company.
export function useCockpitCompanyNotebook(company: Company) {
  const [entries, setEntries] = useState<NotebookEntry[]>([]);
  const [isComposerOpen, setComposerOpen] = useState(false);
  const [notebookForm, setNotebookForm] = useState<NotebookForm>(emptyNotebookForm);
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
    setComposerOpen(false);
    setNotebookForm(emptyNotebookForm());
    refresh();
  }, [company.id, refresh]);

  const selectedEntry = entries.find((entry) => entry.id === selectedEntryId) ?? null;

  // Seed the edit form from the selected entry whenever the selection changes.
  useEffect(() => {
    setEditForm(selectedEntry ? notebookFormFromEntry(selectedEntry) : emptyNotebookForm());
    setEditMode(false);
  }, [selectedEntryId]); // eslint-disable-line react-hooks/exhaustive-deps -- reseed only when the selected id changes, not on every entries refresh

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
      .createNotebookEntry(notebookCreateInput(company.id, notebookForm, manualNotebookOrigins()))
      .then((created) => {
        setNotebookForm(emptyNotebookForm());
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

  return {
    company,
    entries,
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
    error,
  };
}
