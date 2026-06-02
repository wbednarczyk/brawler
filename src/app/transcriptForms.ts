import type { TranscriptJob } from "../api/types";
import type { TranscriptJobForm } from "../screens/Transcripts/transcriptTypes";

export function emptyTranscriptJobForm(): TranscriptJobForm {
  return {
    url: "",
    label: "",
    companyQuery: "",
    companyId: "",
  };
}

export function uniqueTranscriptJobs(jobs: TranscriptJob[]) {
  const seenJobIds = new Set<string>();

  return jobs.filter((job) => {
    if (seenJobIds.has(job.id)) {
      return false;
    }

    seenJobIds.add(job.id);
    return true;
  });
}
