import type { NotebookDraftOrigin, NotebookEntry } from "../api/types";
import type { NotebookForm } from "../shared/types/notebook";

export function notebookFormFromEntry(entry: NotebookEntry): NotebookForm {
  return {
    title: entry.title,
    body: entry.body,
    tags: entry.tags.join(", "),
    kind: entry.kind,
    claimStatus: entry.claimStatus ?? "",
    eventDate: entry.eventDate ?? "",
    followUpAfter: entry.followUpAfter ?? "",
    followUpDate: entry.followUpDate ?? "",
  };
}

export function emptyNotebookForm(): NotebookForm {
  return {
    title: "",
    body: "",
    tags: "",
    kind: "manual",
    claimStatus: "",
    eventDate: "",
    followUpAfter: "",
    followUpDate: "",
  };
}

export function manualNotebookOrigins(): NotebookDraftOrigin[] {
  return [
    {
      sourceType: "manual",
      sourceId: null,
      sourceUrl: null,
      label: "Manual note",
    },
  ];
}

export function notebookTagFromFeedValue(value: string): string {
  return value.trim().toLowerCase().replace(/\s+/g, "-");
}

export function notebookCreateInput(
  companyId: string,
  form: NotebookForm,
  origins: NotebookDraftOrigin[],
) {
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

export function notebookUpdateInput(entryId: string, form: NotebookForm) {
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
