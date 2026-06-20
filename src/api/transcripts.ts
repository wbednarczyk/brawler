import { callCommand } from "./tauri";
import type { NotebookEntry, TranscriptJob, TranscriptSegment } from "./types";
import type { ListVideoTranscriptJobsInput } from "./generated/ListVideoTranscriptJobsInput";
import type { ResolveTranscriptJobCompanyInput } from "./generated/ResolveTranscriptJobCompanyInput";
import type { UpdateVideoTranscriptJobInput } from "./generated/UpdateVideoTranscriptJobInput";
import type { CreateNoteFromTranscriptSelectionInput } from "./generated/CreateNoteFromTranscriptSelectionInput";
import type { CreateVideoTranscriptJobInput } from "./generated/CreateVideoTranscriptJobInput";
import type { RunVideoTranscriptJobInput } from "./generated/RunVideoTranscriptJobInput";

// Input types GENERATED via ts-rs (ADR 0048); `noteDraft` uses the generated
// TranscriptNoteDraft (shape-identical to the previous inline object).
export type { ListVideoTranscriptJobsInput } from "./generated/ListVideoTranscriptJobsInput";
export type { ResolveTranscriptJobCompanyInput } from "./generated/ResolveTranscriptJobCompanyInput";
export type { UpdateVideoTranscriptJobInput } from "./generated/UpdateVideoTranscriptJobInput";
export type { CreateNoteFromTranscriptSelectionInput } from "./generated/CreateNoteFromTranscriptSelectionInput";
export type { CreateVideoTranscriptJobInput } from "./generated/CreateVideoTranscriptJobInput";
export type { RunVideoTranscriptJobInput } from "./generated/RunVideoTranscriptJobInput";

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
