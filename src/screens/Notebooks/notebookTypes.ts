import type { ComponentType, FormEvent, ReactNode } from "react";
import type { Company, NotebookEntry, NotebookOrigin, Watchlist } from "../../api/types";
import type {
  MarkdownNoteBodyProps,
  NotebookDateLikeFieldProps,
  NotebookForm,
} from "../../shared/types/notebook";

export type NotebooksScreenProps = {
  companies: Company[];
  totalCompanyCount: number;
  watchlists: Watchlist[];
  notebookEntries: NotebookEntry[];
  selectedNotebookScreenCompany: Company | null;
  selectedNotebookScreenEntries: NotebookEntry[];
  selectedNotebookScreenEntry: NotebookEntry | null;
  isNotebookScreenComposerOpen: boolean;
  isNotebookScreenEditMode: boolean;
  isNotebookScreenEditDirty: boolean;
  notebookScreenKindFilter: string;
  notebookScreenWatchlistFilter: string;
  notebookScreenClaimStatusFilter: string;
  notebookScreenFollowUpFilter: string;
  notebookScreenTagFilter: string;
  notebookScreenForm: NotebookForm;
  notebookScreenEditForm: NotebookForm;
  notebookError: string | null;
  selectNotebookScreenCompany: (company: Company) => void;
  showNotebookCompanyOpenClaims: (company: Company) => void;
  showNotebookCompanyFollowUps: (company: Company) => void;
  focusCompanyWorkspace: (companyId: string) => void;
  toggleNotebookScreenComposer: () => void;
  discardNotebookScreenDraft: () => void;
  createNotebookScreenEntry: (event: FormEvent<HTMLFormElement>) => void;
  toggleNotebookScreenEntry: (entry: NotebookEntry) => void;
  saveNotebookScreenEntry: (event: FormEvent<HTMLFormElement>) => void;
  deleteNotebookScreenEntry: () => void;
  cancelNotebookScreenEdit: () => void;
  setNotebookScreenEditMode: (value: boolean) => void;
  setNotebookScreenKindFilter: (value: string) => void;
  setNotebookScreenWatchlistFilter: (value: string) => void;
  setNotebookScreenClaimStatusFilter: (value: string) => void;
  setNotebookScreenFollowUpFilter: (value: string) => void;
  setNotebookScreenTagFilter: (value: string) => void;
  updateNotebookScreenForm: (field: keyof NotebookForm, value: string) => void;
  updateNotebookScreenEditForm: (field: keyof NotebookForm, value: string) => void;
  NotebookDateField: ComponentType<NotebookDateLikeFieldProps>;
  NotebookQuarterField: ComponentType<NotebookDateLikeFieldProps>;
  MarkdownNoteBody: ComponentType<MarkdownNoteBodyProps>;
  renderNotebookOrigins: (origins: NotebookOrigin[], companyId: string) => ReactNode;
};
