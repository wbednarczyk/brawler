import { useRef, useState } from "react";
import type { Dispatch, FormEvent, KeyboardEvent, SetStateAction } from "react";
import * as transcriptsApi from "../api/transcripts";
import type {
  Company,
  CredentialStatus,
  NotebookEntry,
  TranscriptJob,
  TranscriptSegment,
  UserSettings,
} from "../api/types";
import { transcriptUrlValidationMessage } from "../screens/Transcripts/transcriptHelpers";
import type { TranscriptJobForm } from "../screens/Transcripts/transcriptTypes";
import type { DbRefreshState } from "./appTypes";
import { emptyNotebookForm } from "./notebookForms";
import { emptyTranscriptJobForm, uniqueTranscriptJobs } from "./transcriptForms";
import type { NotebookForm } from "../shared/types/notebook";

type TranscriptControllerInput = {
  geminiCredentialStatus: CredentialStatus | null;
  refreshNotebookEntries: (companyId: string) => Promise<void>;
  selectedTranscriptJobId: string | null;
  selectedTranscriptSegmentIdsByJobId: Record<string, string[]>;
  settings: UserSettings | null;
  setNotebookEntries: Dispatch<SetStateAction<NotebookEntry[]>>;
  setSelectedNotebookCompanyId: Dispatch<SetStateAction<string | null>>;
  setSelectedNotebookScreenEntryId: Dispatch<SetStateAction<string | null>>;
  setSelectedTranscriptJobId: Dispatch<SetStateAction<string | null>>;
  setSelectedTranscriptSegmentIdsByJobId: Dispatch<SetStateAction<Record<string, string[]>>>;
  setTranscriptDeleteInFlight: Dispatch<SetStateAction<string | null>>;
  setTranscriptDescriptionDraftByJobId: Dispatch<SetStateAction<Record<string, string>>>;
  setTranscriptDescriptionErrorByJobId: Dispatch<SetStateAction<Record<string, string | null>>>;
  setTranscriptDescriptionSaveInFlight: Dispatch<SetStateAction<string | null>>;
  setTranscriptJobCreateError: Dispatch<SetStateAction<string | null>>;
  setTranscriptJobCreateState: Dispatch<SetStateAction<DbRefreshState>>;
  setTranscriptJobForm: Dispatch<SetStateAction<TranscriptJobForm>>;
  setTranscriptJobRunInFlight: Dispatch<SetStateAction<string | null>>;
  setTranscriptJobs: Dispatch<SetStateAction<TranscriptJob[]>>;
  setTranscriptJobsError: Dispatch<SetStateAction<string | null>>;
  setTranscriptLinkErrorByJobId: Dispatch<SetStateAction<Record<string, string | null>>>;
  setTranscriptLinkInFlight: Dispatch<SetStateAction<string | null>>;
  setTranscriptLinkQueryByJobId: Dispatch<SetStateAction<Record<string, string>>>;
  setTranscriptNoteDraftJobId: Dispatch<SetStateAction<string | null>>;
  setTranscriptNoteErrorByJobId: Dispatch<SetStateAction<Record<string, string | null>>>;
  setTranscriptNoteForm: Dispatch<SetStateAction<NotebookForm>>;
  setTranscriptNoteSaveInFlight: Dispatch<SetStateAction<string | null>>;
  setTranscriptSegmentsByJobId: Dispatch<SetStateAction<Record<string, TranscriptSegment[]>>>;
  setTranscriptSegmentsErrorByJobId: Dispatch<SetStateAction<Record<string, string | null>>>;
  transcriptDescriptionDraftByJobId: Record<string, string>;
  transcriptJobForm: TranscriptJobForm;
  transcriptNoteForm: NotebookForm;
};

const missingGeminiCredentialMessage =
  "Gemini transcription credentials are not configured in this app runtime. Save the Gemini API key in Settings before running a transcript job.";

function withoutKey<T>(record: Record<string, T>, key: string) {
  const next = { ...record };
  delete next[key];
  return next;
}

export function useTranscriptController({
  geminiCredentialStatus,
  refreshNotebookEntries,
  selectedTranscriptJobId,
  selectedTranscriptSegmentIdsByJobId,
  settings,
  setNotebookEntries,
  setSelectedNotebookCompanyId,
  setSelectedNotebookScreenEntryId,
  setSelectedTranscriptJobId,
  setSelectedTranscriptSegmentIdsByJobId,
  setTranscriptDeleteInFlight,
  setTranscriptDescriptionDraftByJobId,
  setTranscriptDescriptionErrorByJobId,
  setTranscriptDescriptionSaveInFlight,
  setTranscriptJobCreateError,
  setTranscriptJobCreateState,
  setTranscriptJobForm,
  setTranscriptJobRunInFlight,
  setTranscriptJobs,
  setTranscriptJobsError,
  setTranscriptLinkErrorByJobId,
  setTranscriptLinkInFlight,
  setTranscriptLinkQueryByJobId,
  setTranscriptNoteDraftJobId,
  setTranscriptNoteErrorByJobId,
  setTranscriptNoteForm,
  setTranscriptNoteSaveInFlight,
  setTranscriptSegmentsByJobId,
  setTranscriptSegmentsErrorByJobId,
  transcriptDescriptionDraftByJobId,
  transcriptJobForm,
  transcriptNoteForm,
}: TranscriptControllerInput) {
  // F4b S2 (docs/plans/f4b-contracts/s2-transcripts.md item 5, "Stale" state
  // matrix row): a request-sequence (last-intent) guard per read — only the
  // NEWEST list/segments response for a given key may apply, mirroring
  // useAttentionController's `requestSeqRef` precedent. `transcriptsLoading`
  // is new controller state (contract "Assumptions").
  const [transcriptsLoading, setTranscriptsLoading] = useState(true);
  const listRequestSeqRef = useRef(0);
  const segmentsRequestSeqRef = useRef<Record<string, number>>({});

  function refreshTranscriptJobs(companyId: string | null = null) {
    const requestSeq = (listRequestSeqRef.current += 1);
    setTranscriptsLoading(true);
    return transcriptsApi.listVideoTranscriptJobs({ companyId })
      .then((response) => {
        if (listRequestSeqRef.current !== requestSeq) return;
        setTranscriptJobs(uniqueTranscriptJobs(response));
        setTranscriptJobsError(null);
        const jobIds = new Set(response.map((job) => job.id));
        setTranscriptSegmentsByJobId((current) =>
          Object.fromEntries(Object.entries(current).filter(([jobId]) => jobIds.has(jobId))),
        );
        setSelectedTranscriptSegmentIdsByJobId((current) =>
          Object.fromEntries(Object.entries(current).filter(([jobId]) => jobIds.has(jobId))),
        );
        setTranscriptSegmentsErrorByJobId((current) =>
          Object.fromEntries(Object.entries(current).filter(([jobId]) => jobIds.has(jobId))),
        );
        setSelectedTranscriptJobId((current) => (current && jobIds.has(current) ? current : null));
      })
      .catch((error) => {
        if (listRequestSeqRef.current !== requestSeq) return;
        setTranscriptJobs([]);
        setTranscriptJobsError(String(error));
      })
      .finally(() => {
        if (listRequestSeqRef.current === requestSeq) setTranscriptsLoading(false);
      });
  }

  function refreshTranscriptSegments(jobId: string) {
    const requestSeq = (segmentsRequestSeqRef.current[jobId] = (segmentsRequestSeqRef.current[jobId] ?? 0) + 1);
    return transcriptsApi.listTranscriptSegments(jobId)
      .then((response) => {
        if (segmentsRequestSeqRef.current[jobId] !== requestSeq) return;
        setTranscriptSegmentsByJobId((current) => ({
          ...current,
          [jobId]: response,
        }));
        setTranscriptSegmentsErrorByJobId((current) => ({
          ...current,
          [jobId]: null,
        }));
        setSelectedTranscriptSegmentIdsByJobId((current) => {
          const availableIds = new Set(response.map((segment) => segment.id));
          const selectedIds = current[jobId]?.filter((segmentId) => availableIds.has(segmentId)) ?? [];

          return {
            ...current,
            [jobId]: selectedIds,
          };
        });
      })
      .catch((error) => {
        if (segmentsRequestSeqRef.current[jobId] !== requestSeq) return;
        setTranscriptSegmentsByJobId((current) => ({
          ...current,
          [jobId]: [],
        }));
        setTranscriptSegmentsErrorByJobId((current) => ({
          ...current,
          [jobId]: String(error),
        }));
      });
  }

  /** Row error's "Fetch segments again" (item 3) — re-reads the same job's segments. */
  function retryTranscriptSegments(jobId: string) {
    return refreshTranscriptSegments(jobId);
  }

  function toggleTranscriptJob(job: TranscriptJob) {
    setSelectedTranscriptJobId((current) => (current === job.id ? null : job.id));

    if (job.status === "completed") {
      void refreshTranscriptSegments(job.id);
    }
  }

  function toggleTranscriptJobFromKeyboard(
    event: KeyboardEvent<HTMLElement>,
    job: TranscriptJob,
  ) {
    if (event.key === "Enter" || event.key === " ") {
      event.preventDefault();
      toggleTranscriptJob(job);
    }
  }

  function toggleTranscriptSegment(jobId: string, segmentId: string) {
    setSelectedTranscriptSegmentIdsByJobId((current) => {
      const selectedIds = current[jobId] ?? [];
      const nextSelectedIds = selectedIds.includes(segmentId)
        ? selectedIds.filter((selectedId) => selectedId !== segmentId)
        : [...selectedIds, segmentId];

      return {
        ...current,
        [jobId]: nextSelectedIds,
      };
    });
  }

  function openTranscriptNoteDraft(job: TranscriptJob, selectedSegments: TranscriptSegment[]) {
    if (!job.companyId) {
      setTranscriptNoteErrorByJobId((current) => ({
        ...current,
        [job.id]: "Choose a company to save this as a company notebook note. The transcript can remain unlinked.",
      }));
      return;
    }

    if (selectedSegments.length === 0) {
      setTranscriptNoteErrorByJobId((current) => ({
        ...current,
        [job.id]: "Select at least one transcript segment.",
      }));
      return;
    }

    setTranscriptNoteDraftJobId(job.id);
    setTranscriptNoteForm({
      title: job.sourceLabel ?? "Transcript note",
      body: selectedSegments.map((segment) => `> ${segment.text}`).join("\n\n"),
      tags: "transcript",
      kind: "observation",
      claimStatus: "",
      eventDate: "",
      followUpAfter: "",
      followUpDate: "",
    });
    setTranscriptNoteErrorByJobId((current) => ({
      ...current,
      [job.id]: null,
    }));
  }

  function discardTranscriptNoteDraft() {
    setTranscriptNoteDraftJobId(null);
    setTranscriptNoteForm(emptyNotebookForm());
  }

  function updateTranscriptNoteForm(field: keyof NotebookForm, value: string) {
    setTranscriptNoteForm((current) => ({
      ...current,
      [field]: value,
    }));
  }

  function updateTranscriptLinkQuery(jobId: string, value: string) {
    setTranscriptLinkQueryByJobId((current) => ({
      ...current,
      [jobId]: value,
    }));
  }

  function linkTranscriptJobCompany(jobId: string, company: Company) {
    setTranscriptLinkInFlight(jobId);
    void transcriptsApi.resolveTranscriptJobCompany({
      jobId,
      companyId: company.id,
    })
      .then((updated) => {
        setTranscriptJobs((current) => current.map((job) => (job.id === updated.id ? updated : job)));
        setTranscriptLinkQueryByJobId((current) => ({
          ...current,
          [jobId]: "",
        }));
        setTranscriptLinkErrorByJobId((current) => ({
          ...current,
          [jobId]: null,
        }));
        setTranscriptNoteErrorByJobId((current) => ({
          ...current,
          [jobId]: null,
        }));
        void refreshTranscriptSegments(jobId);
      })
      .catch((error) => {
        setTranscriptLinkErrorByJobId((current) => ({
          ...current,
          [jobId]: String(error),
        }));
      })
      .finally(() => {
        setTranscriptLinkInFlight(null);
      });
  }

  // Cascading (ADR 0076 D5): deleting a transcript job also removes its stored
  // segments, which cannot be faithfully re-created, so the confirm gate is an
  // InlineConfirm at the row call site.
  function deleteTranscriptJob(job: TranscriptJob) {
    setTranscriptDeleteInFlight(job.id);
    void transcriptsApi.deleteVideoTranscriptJob(job.id)
      .then(() => {
        setTranscriptJobs((current) => current.filter((entry) => entry.id !== job.id));
        setTranscriptSegmentsByJobId((current) => withoutKey(current, job.id));
        setSelectedTranscriptSegmentIdsByJobId((current) => withoutKey(current, job.id));
        setTranscriptSegmentsErrorByJobId((current) => withoutKey(current, job.id));
        setTranscriptNoteErrorByJobId((current) => withoutKey(current, job.id));
        setTranscriptLinkErrorByJobId((current) => withoutKey(current, job.id));
        setTranscriptLinkQueryByJobId((current) => withoutKey(current, job.id));
        setTranscriptDescriptionDraftByJobId((current) => withoutKey(current, job.id));
        setTranscriptDescriptionErrorByJobId((current) => withoutKey(current, job.id));
        setSelectedTranscriptJobId((current) => (current === job.id ? null : current));
        setTranscriptNoteDraftJobId((current) => (current === job.id ? null : current));
        setTranscriptJobsError(null);
      })
      .catch((error) => {
        setTranscriptJobsError(String(error));
        void refreshTranscriptJobs();
      })
      .finally(() => {
        setTranscriptDeleteInFlight(null);
      });
  }

  function updateTranscriptJobDescription(job: TranscriptJob) {
    const draft = transcriptDescriptionDraftByJobId[job.id] ?? job.sourceLabel ?? "";

    setTranscriptDescriptionSaveInFlight(job.id);
    setTranscriptDescriptionErrorByJobId((current) => ({
      ...current,
      [job.id]: null,
    }));

    void transcriptsApi.updateVideoTranscriptJob({
      jobId: job.id,
      sourceLabel: draft.trim() || null,
    })
      .then((updated) => {
        setTranscriptJobs((current) => current.map((entry) => (entry.id === updated.id ? updated : entry)));
        setTranscriptDescriptionDraftByJobId((current) => ({
          ...current,
          [updated.id]: updated.sourceLabel ?? "",
        }));
      })
      .catch((error) => {
        setTranscriptDescriptionErrorByJobId((current) => ({
          ...current,
          [job.id]: String(error),
        }));
      })
      .finally(() => {
        setTranscriptDescriptionSaveInFlight(null);
      });
  }

  function createTranscriptNotebookEntry(job: TranscriptJob, event: FormEvent<HTMLFormElement>) {
    event.preventDefault();

    const selectedSegmentIds = selectedTranscriptSegmentIdsByJobId[job.id] ?? [];

    if (!job.companyId) {
      setTranscriptNoteErrorByJobId((current) => ({
        ...current,
        [job.id]: "Choose a company to save this as a company notebook note. The transcript can remain unlinked.",
      }));
      return;
    }

    if (selectedSegmentIds.length === 0) {
      setTranscriptNoteErrorByJobId((current) => ({
        ...current,
        [job.id]: "Select at least one transcript segment.",
      }));
      return;
    }

    const companyId = job.companyId;
    setTranscriptNoteSaveInFlight(job.id);
    void transcriptsApi.createNoteFromTranscriptSelection({
      transcriptJobId: job.id,
      transcriptSegmentIds: selectedSegmentIds,
      noteDraft: {
        title: transcriptNoteForm.title,
        body: transcriptNoteForm.body,
        tags: transcriptNoteForm.tags
          .split(",")
          .map((tag) => tag.trim())
          .filter(Boolean),
        kind: transcriptNoteForm.kind,
        claimStatus: transcriptNoteForm.claimStatus || null,
        eventDate: transcriptNoteForm.eventDate || null,
        followUpAfter: transcriptNoteForm.followUpAfter || null,
        followUpDate: transcriptNoteForm.followUpDate || null,
      },
    })
      .then((created) => {
        setNotebookEntries((current) => [
          created,
          ...current.filter((entry) => entry.id !== created.id),
        ]);
        setSelectedNotebookCompanyId(companyId);
        setSelectedNotebookScreenEntryId(created.id);
        setTranscriptNoteDraftJobId(null);
        setTranscriptNoteForm(emptyNotebookForm());
        setTranscriptNoteErrorByJobId((current) => ({
          ...current,
          [job.id]: null,
        }));
        setSelectedTranscriptSegmentIdsByJobId((current) => ({
          ...current,
          [job.id]: [],
        }));
        void refreshNotebookEntries(companyId);
      })
      .catch((error) => {
        setTranscriptNoteErrorByJobId((current) => ({
          ...current,
          [job.id]: String(error),
        }));
      })
      .finally(() => {
        setTranscriptNoteSaveInFlight(null);
      });
  }

  function selectTranscriptCompany(company: Company) {
    setTranscriptJobForm((current) => ({
      ...current,
      companyId: company.id,
      companyQuery: company.qualifiedTicker,
    }));
  }

  function runTranscriptJob(jobId: string) {
    if (!geminiCredentialStatus?.configured) {
      setTranscriptJobsError(missingGeminiCredentialMessage);
      return;
    }

    setTranscriptJobRunInFlight(jobId);
    setTranscriptJobs((current) =>
      current.map((job) =>
        job.id === jobId
          ? {
              ...job,
              status: "running",
              error: null,
              errorCode: null,
            }
          : job,
      ),
    );

    void transcriptsApi.runVideoTranscriptJob({
      jobId,
      providerMode: settings?.aiProviders.youtubeTranscriptionProvider ?? "provider_gemini",
    })
      .then((updated) => {
        setTranscriptJobs((current) => current.map((job) => (job.id === updated.id ? updated : job)));
        setTranscriptJobsError(null);
        if (selectedTranscriptJobId === updated.id && updated.status === "completed") {
          void refreshTranscriptSegments(updated.id);
        }
        return refreshTranscriptJobs();
      })
      .catch((error) => {
        setTranscriptJobsError(String(error));
      })
      .finally(() => {
        setTranscriptJobRunInFlight(null);
      });
  }

  function createTranscriptJob(event: FormEvent<HTMLFormElement>) {
    event.preventDefault();

    const url = transcriptJobForm.url.trim();
    const validationMessage = transcriptUrlValidationMessage(url);

    if (validationMessage) {
      setTranscriptJobCreateError(validationMessage);
      return;
    }

    setTranscriptJobCreateError(null);
    setTranscriptJobCreateState("refreshing");

    void transcriptsApi.createVideoTranscriptJob({
      sourceUrl: url,
      companyId: transcriptJobForm.companyId || null,
      providerId: settings?.aiProviders.youtubeTranscriptionProvider ?? "provider_gemini",
      sourceLabel: transcriptJobForm.label.trim() || null,
      recognizedCompanyCandidates: null,
    })
      .then((created) => {
        setTranscriptJobForm(emptyTranscriptJobForm());
        setTranscriptJobs((current) => [
          created,
          ...current.filter((job) => job.id !== created.id),
        ]);
        setTranscriptJobCreateState("done");
        if (geminiCredentialStatus?.configured) {
          runTranscriptJob(created.id);
          return Promise.resolve();
        }

        setTranscriptJobsError(missingGeminiCredentialMessage);
        return refreshTranscriptJobs();
      })
      .catch((error) => {
        setTranscriptJobCreateError(String(error));
        setTranscriptJobCreateState("idle");
      });
  }

  return {
    createTranscriptJob,
    createTranscriptNotebookEntry,
    deleteTranscriptJob,
    discardTranscriptNoteDraft,
    linkTranscriptJobCompany,
    openTranscriptNoteDraft,
    refreshTranscriptJobs,
    refreshTranscriptSegments,
    retryTranscriptSegments,
    runTranscriptJob,
    selectTranscriptCompany,
    toggleTranscriptJob,
    toggleTranscriptJobFromKeyboard,
    toggleTranscriptSegment,
    transcriptsLoading,
    updateTranscriptJobDescription,
    updateTranscriptLinkQuery,
    updateTranscriptNoteForm,
  };
}
