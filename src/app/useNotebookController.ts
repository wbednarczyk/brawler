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
import {
  emptyNotebookForm,
  manualNotebookOrigins,
  notebookFormFromEntry,
  notebookTagFromFeedValue,
} from "./notebookForms";

type NotebookControllerInput = {
  claimStatusDraft: string;
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
  setSelectedClaimEntryId: Dispatch<SetStateAction<string | null>>;
  setSelectedNotebookCompanyId: Dispatch<SetStateAction<string | null>>;
  setSelectedNotebookEntryId: Dispatch<SetStateAction<string | null>>;
  setSelectedNotebookScreenEntryId: Dispatch<SetStateAction<string | null>>;
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

function notebookCreateInput(companyId: string, form: NotebookForm, origins: NotebookDraftOrigin[]) {
  return {
    companyId,
    title: form.title,
    body: form.body,
    bodyFormat: "markdown",
    tags: form.tags
      .split(",")
      .map((tag) => tag.trim())
      .filter(Boolean),
    kind: form.kind,
    claimStatus: form.claimStatus || null,
    eventDate: form.eventDate || null,
    followUpAfter: form.followUpAfter || null,
    followUpDate: form.followUpDate || null,
    origins,
  };
}

function notebookUpdateInput(entryId: string, form: NotebookForm) {
  return {
    id: entryId,
    title: form.title,
    body: form.body,
    tags: form.tags
      .split(",")
      .map((tag) => tag.trim())
      .filter(Boolean),
    kind: form.kind,
    claimStatus: form.claimStatus || null,
    eventDate: form.eventDate || null,
    followUpAfter: form.followUpAfter || null,
    followUpDate: form.followUpDate || null,
  };
}

function feedItemSummary(item: FeedItem) {
  return item.summary.trim() || item.title;
}

export function useNotebookController({
  claimStatusDraft,
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
  setSelectedClaimEntryId,
  setSelectedNotebookCompanyId,
  setSelectedNotebookEntryId,
  setSelectedNotebookScreenEntryId,
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

  function createNotebookEntry(event: FormEvent<HTMLFormElement>) {
    event.preventDefault();

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

  function saveNotebookEntry(event: FormEvent<HTMLFormElement>) {
    event.preventDefault();

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

  function toggleClaimEntry(entry: NotebookEntry) {
    setSelectedClaimEntryId((current) => (current === entry.id ? null : entry.id));
  }

  function saveClaimStatus(entry: NotebookEntry) {
    if (!selectedCompany) {
      return;
    }

    notebooksApi.updateNotebookEntry({
      id: entry.id,
      title: entry.title,
      body: entry.body,
      tags: entry.tags,
      kind: entry.kind,
      claimStatus: claimStatusDraft || null,
      eventDate: entry.eventDate,
      followUpAfter: entry.followUpAfter,
      followUpDate: entry.followUpDate,
    })
      .then((updated) => {
        setNotebookEntries((current) =>
          current.map((notebookEntry) => (notebookEntry.id === updated.id ? updated : notebookEntry)),
        );
        setSelectedClaimEntryId(updated.id);
        setNotebookError(null);
        refreshNotebookEntries(selectedCompany.id);
      })
      .catch((error) => {
        setNotebookError(String(error));
      });
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

  function createNotebookScreenEntry(event: FormEvent<HTMLFormElement>) {
    event.preventDefault();

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

  function saveNotebookScreenEntry(event: FormEvent<HTMLFormElement>) {
    event.preventDefault();

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
    discardNotebookScreenDraft,
    feedItemSummary,
    openFeedItemNoteDraft,
    refreshNotebookEntries,
    saveClaimStatus,
    saveNotebookEntry,
    saveNotebookScreenEntry,
    selectNotebookScreenCompany,
    showNotebookCompanyFollowUps,
    showNotebookCompanyOpenClaims,
    toggleClaimEntry,
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
