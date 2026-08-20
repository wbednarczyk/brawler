import type { Dispatch, FormEvent, SetStateAction } from "react";
import * as notebooksApi from "../api/notebooks";
import type {
  Company,
  FeedItem,
  NotebookDraftOrigin,
  NotebookEntry,
} from "../api/types";
import type { Section } from "./navigation";
import type { NotebookForm } from "../shared/types/notebook";
import type { UndoableDeleteConfig } from "../ui";
import type { CreateNotebookEntryInput } from "../api/notebooks";
import {
  emptyNotebookForm,
  manualNotebookOrigins,
  notebookCreateInput,
  notebookFormFromEntry,
  notebookTagFromFeedValue,
  notebookUpdateInput,
} from "./notebookForms";

type PreventableFormEvent = Pick<FormEvent<HTMLFormElement>, "preventDefault">;

// Faithful undo restore (ADR 0076 D5): rebuild the create-input from a deleted
// entry, carrying content and provenance origins (NotebookOrigin → NewNotebookOrigin,
// dropping the server-owned id/createdAt).
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

type NotebookControllerInput = {
  companies: Company[];
  notebookEditForm: NotebookForm;
  notebookForm: NotebookForm;
  notebookScreenDraftOrigins: NotebookDraftOrigin[];
  notebookScreenEditForm: NotebookForm;
  notebookScreenForm: NotebookForm;
  selectedCompany: Company | null;
  selectedNotebookCompanyId: string | null;
  selectedNotebookEntry: NotebookEntry | null;
  selectedNotebookScreenCompany: Company | null;
  selectedNotebookScreenEntry: NotebookEntry | null;
  setActiveSection: Dispatch<SetStateAction<Section>>;
  setNotebookComposerOpen: Dispatch<SetStateAction<boolean>>;
  setNotebookEditForm: Dispatch<SetStateAction<NotebookForm>>;
  setNotebookEditMode: Dispatch<SetStateAction<boolean>>;
  setNotebookEntries: Dispatch<SetStateAction<NotebookEntry[]>>;
  setNotebookError: Dispatch<SetStateAction<string | null>>;
  setNotebookForm: Dispatch<SetStateAction<NotebookForm>>;
  setNotebookScreenComposerOpen: Dispatch<SetStateAction<boolean>>;
  setNotebookScreenDraftOrigins: Dispatch<SetStateAction<NotebookDraftOrigin[]>>;
  setNotebookScreenEditForm: Dispatch<SetStateAction<NotebookForm>>;
  setNotebookScreenEditMode: Dispatch<SetStateAction<boolean>>;
  setNotebookScreenFollowUpFilter: Dispatch<SetStateAction<string>>;
  setNotebookScreenForm: Dispatch<SetStateAction<NotebookForm>>;
  setNotebookScreenKindFilter: Dispatch<SetStateAction<string>>;
  setNotebookScreenClaimStatusFilter: Dispatch<SetStateAction<string>>;
  setNotebookScreenTagFilter: Dispatch<SetStateAction<string>>;
  setSelectedNotebookCompanyId: Dispatch<SetStateAction<string | null>>;
  setSelectedNotebookEntryId: Dispatch<SetStateAction<string | null>>;
  setSelectedNotebookScreenEntryId: Dispatch<SetStateAction<string | null>>;
  text: (value: string) => string;
  runUndoableDelete: (config: UndoableDeleteConfig) => void;
};

function updateNotebookFormState(
  setForm: Dispatch<SetStateAction<NotebookForm>>,
  field: keyof NotebookForm,
  value: string,
) {
  setForm((current) => ({
    ...current,
    [field]: value,
  }));
}

// Shared root for every render site (Inbox, Cockpit, Company feed): a
// "filing" is a bare official-report notice with no attachments, so its
// stored summary is the dead "Komunikat ESPI/EBI" literal — suppress it here
// once rather than forking a guard into each caller.
export function feedItemSummary(item: FeedItem) {
  if (item.presentationKind === "filing") {
    return "";
  }
  return item.summary.trim() || item.title;
}

export function useNotebookController({
  companies,
  notebookEditForm,
  notebookForm,
  notebookScreenDraftOrigins,
  notebookScreenEditForm,
  notebookScreenForm,
  selectedCompany,
  selectedNotebookCompanyId,
  selectedNotebookEntry,
  selectedNotebookScreenCompany,
  selectedNotebookScreenEntry,
  setActiveSection,
  setNotebookComposerOpen,
  setNotebookEditForm,
  setNotebookEditMode,
  setNotebookEntries,
  setNotebookError,
  setNotebookForm,
  setNotebookScreenComposerOpen,
  setNotebookScreenDraftOrigins,
  setNotebookScreenEditForm,
  setNotebookScreenEditMode,
  setNotebookScreenFollowUpFilter,
  setNotebookScreenForm,
  setNotebookScreenKindFilter,
  setNotebookScreenClaimStatusFilter,
  setNotebookScreenTagFilter,
  setSelectedNotebookCompanyId,
  setSelectedNotebookEntryId,
  setSelectedNotebookScreenEntryId,
  text,
  runUndoableDelete,
}: NotebookControllerInput) {
  function findCompanyForFeedItem(item: FeedItem) {
    return companies.find((company) => company.qualifiedTicker === item.company) ?? null;
  }

  function refreshNotebookEntries(companyId: string) {
    return notebooksApi.listNotebookEntries(companyId)
      .then((response) => {
        setNotebookEntries((current) => [
          ...response,
          ...current.filter((entry) => entry.companyId !== companyId),
        ]);
        setSelectedNotebookEntryId((current) => {
          if (current && response.some((entry) => entry.id === current)) {
            return current;
          }

          return response[0]?.id ?? null;
        });
        setSelectedNotebookScreenEntryId((current) => {
          if (selectedNotebookCompanyId !== companyId) {
            return current;
          }

          if (current && response.some((entry) => entry.id === current)) {
            return current;
          }

          return null;
        });
        setNotebookError(null);
      })
      .catch((error) => {
        setNotebookEntries((current) => current.filter((entry) => entry.companyId !== companyId));
        setNotebookError(String(error));
      });
  }

  function openFeedItemNoteDraft(item: FeedItem) {
    const company = findCompanyForFeedItem(item);

    if (!company) {
      return;
    }

    setSelectedNotebookCompanyId(company.id);
    setSelectedNotebookScreenEntryId(null);
    setNotebookScreenEditMode(false);
    setNotebookScreenComposerOpen(true);
    setNotebookScreenForm({
      title: item.title,
      body: item.bodyText || feedItemSummary(item),
      tags: ["feed", notebookTagFromFeedValue(item.type), notebookTagFromFeedValue(item.source)]
        .filter(Boolean)
        .join(", "),
      kind: "observation",
      claimStatus: "",
      eventDate: "",
      followUpAfter: "",
      followUpDate: "",
    });
    setNotebookScreenDraftOrigins([
      {
        sourceType: "feed_item",
        sourceId: item.id,
        sourceUrl: item.sourceUrl,
        label: `${item.source}: ${item.title}`,
      },
    ]);
    setNotebookError(null);
    setActiveSection("Notebooks");
    refreshNotebookEntries(company.id);
  }

  function createNotebookEntry(event?: PreventableFormEvent) {
    event?.preventDefault();

    if (!selectedCompany) {
      return;
    }

    notebooksApi.createNotebookEntry(
      notebookCreateInput(selectedCompany.id, notebookForm, manualNotebookOrigins()),
    )
      .then((created) => {
        setNotebookForm(emptyNotebookForm());
        setNotebookComposerOpen(false);
        setSelectedNotebookEntryId(created.id);
        setNotebookError(null);
        refreshNotebookEntries(selectedCompany.id);
      })
      .catch((error) => {
        setNotebookError(String(error));
      });
  }

  function saveNotebookEntry(event?: PreventableFormEvent) {
    event?.preventDefault();

    if (!selectedNotebookEntry || !selectedCompany) {
      return;
    }

    notebooksApi.updateNotebookEntry(notebookUpdateInput(selectedNotebookEntry.id, notebookEditForm))
      .then((updated) => {
        setNotebookEntries((current) =>
          current.map((entry) => (entry.id === updated.id ? updated : entry)),
        );
        setSelectedNotebookEntryId(updated.id);
        setNotebookEditMode(false);
        setNotebookError(null);
        refreshNotebookEntries(selectedCompany.id);
      })
      .catch((error) => {
        setNotebookError(String(error));
      });
  }

  function cancelNotebookEdit() {
    if (selectedNotebookEntry) {
      setNotebookEditForm(notebookFormFromEntry(selectedNotebookEntry));
    }

    setNotebookEditMode(false);
  }

  function selectNotebookScreenCompany(company: Company) {
    setSelectedNotebookCompanyId(company.id);
    setSelectedNotebookScreenEntryId(null);
    setNotebookScreenEditMode(false);
    setNotebookScreenComposerOpen(false);
    setNotebookScreenForm(emptyNotebookForm());
    setNotebookScreenDraftOrigins(manualNotebookOrigins());
    refreshNotebookEntries(company.id);
  }

  function showNotebookCompanyFollowUps(company: Company) {
    selectNotebookScreenCompany(company);
    setNotebookScreenKindFilter("all");
    setNotebookScreenClaimStatusFilter("all");
    setNotebookScreenFollowUpFilter("has_follow_up");
    setNotebookScreenTagFilter("");
  }

  function showNotebookCompanyOpenClaims(company: Company) {
    selectNotebookScreenCompany(company);
    setNotebookScreenKindFilter("all");
    setNotebookScreenClaimStatusFilter("open");
    setNotebookScreenFollowUpFilter("all");
    setNotebookScreenTagFilter("");
  }

  function toggleNotebookScreenComposer() {
    setNotebookScreenComposerOpen((current) => {
      const next = !current;

      if (next) {
        setNotebookScreenForm(emptyNotebookForm());
        setNotebookScreenDraftOrigins(manualNotebookOrigins());
      }

      return next;
    });
  }

  function discardNotebookScreenDraft() {
    setNotebookScreenComposerOpen(false);
    setNotebookScreenForm(emptyNotebookForm());
    setNotebookScreenDraftOrigins(manualNotebookOrigins());
  }

  function toggleNotebookScreenEntry(entry: NotebookEntry) {
    setSelectedNotebookScreenEntryId((current) => {
      const next = current === entry.id ? null : entry.id;

      if (next === null) {
        setNotebookScreenEditMode(false);
      }

      return next;
    });
  }

  function createNotebookScreenEntry(event?: PreventableFormEvent) {
    event?.preventDefault();

    if (!selectedNotebookScreenCompany) {
      return;
    }

    notebooksApi.createNotebookEntry(
      notebookCreateInput(
        selectedNotebookScreenCompany.id,
        notebookScreenForm,
        notebookScreenDraftOrigins,
      ),
    )
      .then((created) => {
        setNotebookScreenForm(emptyNotebookForm());
        setNotebookScreenDraftOrigins(manualNotebookOrigins());
        setNotebookScreenComposerOpen(false);
        setSelectedNotebookScreenEntryId(created.id);
        setNotebookScreenEditMode(false);
        setNotebookError(null);
        refreshNotebookEntries(selectedNotebookScreenCompany.id);
      })
      .catch((error) => {
        setNotebookError(String(error));
      });
  }

  function saveNotebookScreenEntry(event?: PreventableFormEvent) {
    event?.preventDefault();

    if (!selectedNotebookScreenEntry || !selectedNotebookScreenCompany) {
      return;
    }

    notebooksApi.updateNotebookEntry(notebookUpdateInput(selectedNotebookScreenEntry.id, notebookScreenEditForm))
      .then((updated) => {
        setNotebookEntries((current) =>
          current.map((entry) => (entry.id === updated.id ? updated : entry)),
        );
        setSelectedNotebookScreenEntryId(updated.id);
        setNotebookScreenEditMode(false);
        setNotebookError(null);
        refreshNotebookEntries(selectedNotebookScreenCompany.id);
      })
      .catch((error) => {
        setNotebookError(String(error));
      });
  }

  // Reversible destroy (ADR 0076 D5): a note re-creates faithfully via
  // create_notebook_entry — content and provenance origins are preserved — so it
  // deletes immediately with an undo toast (the new id is inconsequential; nothing
  // references a note by id after deletion).
  function deleteNotebookScreenEntry() {
    if (!selectedNotebookScreenEntry || !selectedNotebookScreenCompany) {
      return;
    }

    const entry = selectedNotebookScreenEntry;
    const company = selectedNotebookScreenCompany;

    runUndoableDelete({
      perform: () => notebooksApi.deleteNotebookEntry(entry.id),
      restore: () => notebooksApi.createNotebookEntry(notebookEntryRestoreInput(entry)),
      message: text("Note deleted"),
      undoLabel: text("Undo"),
      onPerformed: () => {
        setNotebookEntries((current) => current.filter((item) => item.id !== entry.id));
        setSelectedNotebookScreenEntryId(null);
        setNotebookScreenEditMode(false);
        setNotebookError(null);
        refreshNotebookEntries(company.id);
      },
      onRestored: () => {
        refreshNotebookEntries(company.id);
      },
      onError: (error) => {
        setNotebookError(String(error));
      },
    });
  }

  function cancelNotebookScreenEdit() {
    if (selectedNotebookScreenEntry) {
      setNotebookScreenEditForm(notebookFormFromEntry(selectedNotebookScreenEntry));
    }

    setNotebookScreenEditMode(false);
  }

  return {
    cancelNotebookEdit,
    cancelNotebookScreenEdit,
    createNotebookEntry,
    createNotebookScreenEntry,
    deleteNotebookScreenEntry,
    discardNotebookScreenDraft,
    feedItemSummary,
    openFeedItemNoteDraft,
    refreshNotebookEntries,
    saveNotebookEntry,
    saveNotebookScreenEntry,
    selectNotebookScreenCompany,
    showNotebookCompanyFollowUps,
    showNotebookCompanyOpenClaims,
    toggleNotebookScreenComposer,
    toggleNotebookScreenEntry,
    updateNotebookEditForm: (field: keyof NotebookForm, value: string) =>
      updateNotebookFormState(setNotebookEditForm, field, value),
    updateNotebookForm: (field: keyof NotebookForm, value: string) =>
      updateNotebookFormState(setNotebookForm, field, value),
    updateNotebookScreenEditForm: (field: keyof NotebookForm, value: string) =>
      updateNotebookFormState(setNotebookScreenEditForm, field, value),
    updateNotebookScreenForm: (field: keyof NotebookForm, value: string) =>
      updateNotebookFormState(setNotebookScreenForm, field, value),
  };
}
