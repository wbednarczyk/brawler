import { callCommand } from "./tauri";
import type { NotebookEntry, TranscriptJob, TranscriptSegment } from "./types";

export type ListVideoTranscriptJobsInput = {
  companyId: string | null;
};

export type ResolveTranscriptJobCompanyInput = {
  jobId: string;
  companyId: string;
};

export type UpdateVideoTranscriptJobInput = {
  jobId: string;
  sourceLabel: string | null;
};

export type CreateNoteFromTranscriptSelectionInput = {
  transcriptJobId: string;
  transcriptSegmentIds: string[];
  noteDraft: {
    title: string;
    body: string;
    tags: string[];
    kind: string;
    claimStatus: string | null;
    eventDate: string | null;
    followUpAfter: string | null;
    followUpDate: string | null;
  };
};

export type CreateVideoTranscriptJobInput = {
  sourceUrl: string;
  companyId: string | null;
  providerId: string;
  sourceLabel: string | null;
  recognizedCompanyCandidates: null;
};

export type RunVideoTranscriptJobInput = {
  jobId: string;
  providerMode: string;
};

export function listVideoTranscriptJobs(input: ListVideoTranscriptJobsInput) {
  return callCommand<TranscriptJob[]>("list_video_transcript_jobs", { input });
}

export function listTranscriptSegments(transcriptJobId: string) {
  return callCommand<TranscriptSegment[]>("list_transcript_segments", { transcriptJobId });
}

export function resolveTranscriptJobCompany(input: ResolveTranscriptJobCompanyInput) {
  return callCommand<TranscriptJob>("resolve_transcript_job_company", { input });
}

export function deleteVideoTranscriptJob(jobId: string) {
  return callCommand<void>("delete_video_transcript_job", { jobId });
}

export function updateVideoTranscriptJob(input: UpdateVideoTranscriptJobInput) {
  return callCommand<TranscriptJob>("update_video_transcript_job", { input });
}

export function createNoteFromTranscriptSelection(input: CreateNoteFromTranscriptSelectionInput) {
  return callCommand<NotebookEntry>("create_note_from_transcript_selection", { input });
}

export function createVideoTranscriptJob(input: CreateVideoTranscriptJobInput) {
  return callCommand<TranscriptJob>("create_video_transcript_job", { input });
}

export function runVideoTranscriptJob(input: RunVideoTranscriptJobInput) {
  return callCommand<TranscriptJob>("run_video_transcript_job", { input });
}
