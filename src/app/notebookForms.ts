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
