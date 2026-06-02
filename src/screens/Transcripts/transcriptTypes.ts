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
  UserSettings,
} from "../../api/types";
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
  settings: UserSettings | null;
  geminiCredentialStatus: CredentialStatus | null;
  transcriptJobs: TranscriptJob[];
  transcriptJobsError: string | null;
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
  formatAiProvider: (value: string | null | undefined) => string;
  formatGeminiModel: (value: string | null | undefined) => string;
  formatCredentialConfigured: (status: CredentialStatus | null) => string;
  formatCredentialStorage: (value: string | null | undefined) => string;
  formatEnumLabel: (value: string) => string;
};
