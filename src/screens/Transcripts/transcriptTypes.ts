import type {
  ComponentType,
  Dispatch,
  FormEvent,
  KeyboardEvent,
  SetStateAction,
} from "react";
import type {
  Company,
  CredentialStatus,
  TranscriptJob,
  TranscriptSegment,
} from "../../api/types";
import type { CompanyWorkspaceTab } from "../Companies/companyTypes";
import type { NotebookDateLikeFieldProps, NotebookForm } from "../../shared/types/notebook";

export type TranscriptJobForm = {
  url: string;
  label: string;
  companyQuery: string;
  companyId: string;
};

export type TranscriptNoteForm = NotebookForm;

export type TranscriptsScreenProps = {
  companies: Company[];
  geminiCredentialStatus: CredentialStatus | null;
  transcriptJobs: TranscriptJob[];
  transcriptJobsError: string | null;
  /** True while the list request is in flight (F4b S2 — new controller state). */
  transcriptsLoading: boolean;
  transcriptJobForm: TranscriptJobForm;
  transcriptJobCreateError: string | null;
  transcriptJobCreateState: string;
  transcriptJobRunInFlight: string | null;
  selectedTranscriptJobId: string | null;
  transcriptSegmentsByJobId: Record<string, TranscriptSegment[]>;
  transcriptSegmentsErrorByJobId: Record<string, string | null>;
  transcriptSegmentSearchByJobId: Record<string, string>;
  selectedTranscriptSegmentIdsByJobId: Record<string, string[]>;
  transcriptNoteDraftJobId: string | null;
  transcriptNoteForm: TranscriptNoteForm;
  transcriptNoteErrorByJobId: Record<string, string | null>;
  transcriptNoteSaveInFlight: string | null;
  transcriptLinkQueryByJobId: Record<string, string>;
  transcriptLinkErrorByJobId: Record<string, string | null>;
  transcriptLinkInFlight: string | null;
  transcriptDeleteInFlight: string | null;
  transcriptDescriptionDraftByJobId: Record<string, string>;
  transcriptDescriptionErrorByJobId: Record<string, string | null>;
  transcriptDescriptionSaveInFlight: string | null;
  transcriptCompanySuggestions: Company[];
  NotebookDateField: ComponentType<NotebookDateLikeFieldProps>;
  NotebookQuarterField: ComponentType<NotebookDateLikeFieldProps>;
  setTranscriptJobForm: Dispatch<SetStateAction<TranscriptJobForm>>;
  setTranscriptJobCreateError: Dispatch<SetStateAction<string | null>>;
  setTranscriptSegmentSearchByJobId: Dispatch<SetStateAction<Record<string, string>>>;
  setTranscriptDescriptionDraftByJobId: Dispatch<SetStateAction<Record<string, string>>>;
  refreshTranscriptJobs: () => Promise<void> | Promise<unknown>;
  /** Re-reads one job's segments after a failed read (item 3 — the row error's `Fetch segments again`). */
  retryTranscriptSegments: (jobId: string) => Promise<void> | Promise<unknown>;
  createTranscriptJob: (event: FormEvent<HTMLFormElement>) => void;
  toggleTranscriptJob: (job: TranscriptJob) => void;
  toggleTranscriptJobFromKeyboard: (
    event: KeyboardEvent<HTMLElement>,
    job: TranscriptJob,
  ) => void;
  runTranscriptJob: (jobId: string) => void;
  deleteTranscriptJob: (job: TranscriptJob) => void;
  updateTranscriptJobDescription: (job: TranscriptJob) => void;
  updateTranscriptLinkQuery: (jobId: string, value: string) => void;
  linkTranscriptJobCompany: (jobId: string, company: Company) => void;
  toggleTranscriptSegment: (jobId: string, segmentId: string) => void;
  openTranscriptNoteDraft: (job: TranscriptJob, selectedSegments: TranscriptSegment[]) => void;
  createTranscriptNotebookEntry: (
    job: TranscriptJob,
    event: FormEvent<HTMLFormElement>,
  ) => void;
  discardTranscriptNoteDraft: () => void;
  updateTranscriptNoteForm: (field: keyof TranscriptNoteForm, value: string) => void;
  selectTranscriptCompany: (company: Company) => void;
  /** Success strip's "Open notebook" destination (item 3). */
  openCompanyWorkspaceById: (companyId: string, tab?: CompanyWorkspaceTab) => void;
  /** The composer's/empty-state's "Open settings" destination (missing Gemini key). */
  openSettings: () => void;
};
