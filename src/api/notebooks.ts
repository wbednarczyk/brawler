import { callCommand } from "./tauri";
import type { NotebookEntry } from "./types";
import type { CreateNotebookEntryInput } from "./generated/CreateNotebookEntryInput";
import type { UpdateNotebookEntryInput } from "./generated/UpdateNotebookEntryInput";

// Input types GENERATED from src-tauri/src/storage/types.rs via ts-rs (ADR 0048).
// `origins` uses the generated NewNotebookOrigin (shape-identical to the
// frontend NotebookDraftOrigin).
export type { CreateNotebookEntryInput } from "./generated/CreateNotebookEntryInput";
export type { UpdateNotebookEntryInput } from "./generated/UpdateNotebookEntryInput";

export function listNotebookEntries(companyId: string) {
  return callCommand<NotebookEntry[]>("list_notebook_entries", { companyId });
}

export function createNotebookEntry(input: CreateNotebookEntryInput) {
  return callCommand<NotebookEntry>("create_notebook_entry", { input });
}

export function updateNotebookEntry(input: UpdateNotebookEntryInput) {
  return callCommand<NotebookEntry>("update_notebook_entry", { input });
}

export function deleteNotebookEntry(id: string) {
  return callCommand<void>("delete_notebook_entry", { id });
}
