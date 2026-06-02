import { callCommand } from "./tauri";
import type { NotebookDraftOrigin, NotebookEntry } from "./types";

export type CreateNotebookEntryInput = {
  companyId: string;
  title: string;
  body: string;
  bodyFormat: string;
  tags: string[];
  kind: string;
  claimStatus: string | null;
  eventDate: string | null;
  followUpAfter: string | null;
  followUpDate: string | null;
  origins: NotebookDraftOrigin[];
};

export type UpdateNotebookEntryInput = {
  id: string;
  title: string;
  body: string;
  tags: string[];
  kind: string;
  claimStatus: string | null;
  eventDate: string | null;
  followUpAfter: string | null;
  followUpDate: string | null;
};

export function listNotebookEntries(companyId: string) {
  return callCommand<NotebookEntry[]>("list_notebook_entries", { companyId });
}

export function createNotebookEntry(input: CreateNotebookEntryInput) {
  return callCommand<NotebookEntry>("create_notebook_entry", { input });
}

export function updateNotebookEntry(input: UpdateNotebookEntryInput) {
  return callCommand<NotebookEntry>("update_notebook_entry", { input });
}
