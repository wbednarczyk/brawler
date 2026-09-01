import { describe, expect, it, vi, beforeEach } from "vitest";
import { act, renderHook } from "@testing-library/react";
import { useState } from "react";

import { useTranscriptController } from "./useTranscriptController";
import { emptyNotebookForm } from "./notebookForms";
import { emptyTranscriptJobForm } from "./transcriptForms";
import type { TranscriptJob, TranscriptSegment } from "../api/types";

// Sol R1 finding 4: request-sequence (last-intent) guard proof for the list
// and per-job segment reads — deferred promises so an out-of-order resolution
// is deterministic, mirroring useAttentionController.test.ts's precedent.
vi.mock("../api/transcripts", () => ({
  listVideoTranscriptJobs: vi.fn(),
  listTranscriptSegments: vi.fn(),
  resolveTranscriptJobCompany: vi.fn(),
  deleteVideoTranscriptJob: vi.fn(),
  updateVideoTranscriptJob: vi.fn(),
  createNoteFromTranscriptSelection: vi.fn(),
  createVideoTranscriptJob: vi.fn(),
  runVideoTranscriptJob: vi.fn(),
}));

import { listTranscriptSegments, listVideoTranscriptJobs } from "../api/transcripts";

type Deferred<T> = { promise: Promise<T>; resolve: (value: T) => void; reject: (cause: unknown) => void };
function deferred<T>(): Deferred<T> {
  let resolve!: (value: T) => void;
  let reject!: (cause: unknown) => void;
  const promise = new Promise<T>((res, rej) => {
    resolve = res;
    reject = rej;
  });
  return { promise, resolve, reject };
}

function job(id: string, overrides: Partial<TranscriptJob> = {}): TranscriptJob {
  return {
    id,
    companyId: null,
    company: null,
    companyName: null,
    providerId: "provider_gemini",
    sourceType: "youtube_url",
    sourceUrl: `https://example.test/${id}`,
    sourceLabel: null,
    companyResolutionStatus: "unresolved",
    recognizedCompanyCandidates: [],
    status: "queued",
    errorCode: null,
    createdAt: "2026-06-01T00:00:00Z",
    startedAt: null,
    finishedAt: null,
    error: null,
    ...overrides,
  };
}

function segment(id: string, jobId: string): TranscriptSegment {
  return {
    id,
    transcriptJobId: jobId,
    companyId: null,
    startSeconds: 0,
    endSeconds: 10,
    speaker: "CEO",
    text: id,
    language: "en",
    createdAt: "2026-06-01T00:00:00Z",
  };
}

// Full harness: `useTranscriptController` is a plain function taking every
// piece of AppStateRoot's transcript state as setters — this wires the SAME
// shape with local `useState` so the request-sequence guard runs against
// real React state updates, not stubs.
function useHarness() {
  const [transcriptJobs, setTranscriptJobs] = useState<TranscriptJob[]>([]);
  const [transcriptJobsError, setTranscriptJobsError] = useState<string | null>(null);
  const [transcriptSegmentsByJobId, setTranscriptSegmentsByJobId] = useState<Record<string, TranscriptSegment[]>>({});
  const [transcriptSegmentsErrorByJobId, setTranscriptSegmentsErrorByJobId] = useState<Record<string, string | null>>({});
  const [selectedTranscriptJobId, setSelectedTranscriptJobId] = useState<string | null>(null);
  const [selectedTranscriptSegmentIdsByJobId, setSelectedTranscriptSegmentIdsByJobId] = useState<Record<string, string[]>>({});
  const [transcriptJobForm, setTranscriptJobForm] = useState(emptyTranscriptJobForm());
  const [transcriptNoteForm, setTranscriptNoteForm] = useState(emptyNotebookForm());
  const [transcriptDescriptionDraftByJobId, setTranscriptDescriptionDraftByJobId] = useState<Record<string, string>>({});

  const controller = useTranscriptController({
    geminiCredentialStatus: { providerId: "provider_gemini", secretKind: "api_key", configured: true, storage: "keychain", label: "Gemini", devFallbackAvailable: false, error: null },
    refreshNotebookEntries: async () => {},
    selectedTranscriptJobId,
    selectedTranscriptSegmentIdsByJobId,
    settings: null,
    setNotebookEntries: () => {},
    setSelectedNotebookCompanyId: () => {},
    setSelectedNotebookScreenEntryId: () => {},
    setSelectedTranscriptJobId,
    setSelectedTranscriptSegmentIdsByJobId,
    setTranscriptDeleteInFlight: () => {},
    setTranscriptDescriptionDraftByJobId,
    setTranscriptDescriptionErrorByJobId: () => {},
    setTranscriptDescriptionSaveInFlight: () => {},
    setTranscriptJobCreateError: () => {},
    setTranscriptJobCreateState: () => {},
    setTranscriptJobForm,
    setTranscriptJobRunInFlight: () => {},
    setTranscriptJobs,
    setTranscriptJobsError,
    setTranscriptLinkErrorByJobId: () => {},
    setTranscriptLinkInFlight: () => {},
    setTranscriptLinkQueryByJobId: () => {},
    setTranscriptNoteDraftJobId: () => {},
    setTranscriptNoteErrorByJobId: () => {},
    setTranscriptNoteForm,
    setTranscriptNoteSaveInFlight: () => {},
    setTranscriptSegmentsByJobId,
    setTranscriptSegmentsErrorByJobId,
    transcriptDescriptionDraftByJobId,
    transcriptJobForm,
    transcriptNoteForm,
  });

  return {
    ...controller,
    transcriptJobs,
    transcriptJobsError,
    transcriptSegmentsByJobId,
    transcriptSegmentsErrorByJobId,
  };
}

beforeEach(() => {
  vi.clearAllMocks();
});

describe("useTranscriptController — request-sequence guard (sol R1 finding 4)", () => {
  it("list read: a stale SUCCESS resolving after a newer one is discarded", async () => {
    const first = deferred<TranscriptJob[]>();
    const second = deferred<TranscriptJob[]>();
    vi.mocked(listVideoTranscriptJobs).mockReturnValueOnce(first.promise).mockReturnValueOnce(second.promise);

    const { result } = renderHook(() => useHarness());
    let firstCall!: Promise<void>;
    let secondCall!: Promise<void>;
    act(() => {
      firstCall = result.current.refreshTranscriptJobs() as Promise<void>;
      secondCall = result.current.refreshTranscriptJobs() as Promise<void>;
    });
    expect(result.current.transcriptsLoading).toBe(true);

    // The STALE (first) request resolves LAST — must not win.
    await act(async () => {
      second.resolve([job("fresh")]);
      await secondCall;
      first.resolve([job("stale")]);
      await firstCall;
    });

    expect(result.current.transcriptJobs.map((j) => j.id)).toEqual(["fresh"]);
    expect(result.current.transcriptsLoading).toBe(false);
    expect(result.current.transcriptJobsError).toBeNull();
  });

  it("list read: a stale ERROR resolving after a newer success does not clobber it or reopen loading", async () => {
    const first = deferred<TranscriptJob[]>();
    const second = deferred<TranscriptJob[]>();
    vi.mocked(listVideoTranscriptJobs).mockReturnValueOnce(first.promise).mockReturnValueOnce(second.promise);

    const { result } = renderHook(() => useHarness());
    let firstCall!: Promise<void>;
    let secondCall!: Promise<void>;
    act(() => {
      firstCall = result.current.refreshTranscriptJobs() as Promise<void>;
      secondCall = result.current.refreshTranscriptJobs() as Promise<void>;
    });

    await act(async () => {
      second.resolve([job("fresh")]);
      await secondCall;
      first.reject(new Error("stale failure"));
      await firstCall.catch(() => {});
    });

    expect(result.current.transcriptJobs.map((j) => j.id)).toEqual(["fresh"]);
    expect(result.current.transcriptJobsError).toBeNull();
    expect(result.current.transcriptsLoading).toBe(false);
  });

  it("segment read: fresh-before-stale for ONE job id — a different job's read is unaffected", async () => {
    const staleA = deferred<TranscriptSegment[]>();
    const freshA = deferred<TranscriptSegment[]>();
    const otherB = deferred<TranscriptSegment[]>();
    // Invocation order below is synchronous within `act()`: job_a (stale),
    // job_a (fresh), job_b — `mockReturnValueOnce` queues match 1:1.
    vi.mocked(listTranscriptSegments)
      .mockReturnValueOnce(staleA.promise)
      .mockReturnValueOnce(freshA.promise)
      .mockReturnValueOnce(otherB.promise);

    const { result } = renderHook(() => useHarness());
    let staleCall!: Promise<void>;
    let freshCall!: Promise<void>;
    let otherCall!: Promise<void>;
    act(() => {
      staleCall = result.current.refreshTranscriptSegments("job_a") as Promise<void>;
      freshCall = result.current.refreshTranscriptSegments("job_a") as Promise<void>;
      otherCall = result.current.refreshTranscriptSegments("job_b") as Promise<void>;
    });

    await act(async () => {
      // job_b's own read resolves independently of job_a's race.
      otherB.resolve([segment("seg_b", "job_b")]);
      await otherCall;
      freshA.resolve([segment("seg_fresh", "job_a")]);
      await freshCall;
      // The stale job_a response arrives last — discarded.
      staleA.resolve([segment("seg_stale", "job_a")]);
      await staleCall;
    });

    expect(result.current.transcriptSegmentsByJobId.job_a?.map((s) => s.id)).toEqual(["seg_fresh"]);
    expect(result.current.transcriptSegmentsByJobId.job_b?.map((s) => s.id)).toEqual(["seg_b"]);
    expect(result.current.transcriptSegmentsErrorByJobId.job_a).toBeNull();
  });

  it("segment read: a stale ERROR after a newer success does not clobber that job's segments", async () => {
    const stale = deferred<TranscriptSegment[]>();
    const fresh = deferred<TranscriptSegment[]>();
    vi.mocked(listTranscriptSegments).mockReturnValueOnce(stale.promise).mockReturnValueOnce(fresh.promise);

    const { result } = renderHook(() => useHarness());
    let staleCall!: Promise<void>;
    let freshCall!: Promise<void>;
    act(() => {
      staleCall = result.current.refreshTranscriptSegments("job_a") as Promise<void>;
      freshCall = result.current.refreshTranscriptSegments("job_a") as Promise<void>;
    });

    await act(async () => {
      fresh.resolve([segment("seg_fresh", "job_a")]);
      await freshCall;
      stale.reject(new Error("stale segment failure"));
      await staleCall.catch(() => {});
    });

    expect(result.current.transcriptSegmentsByJobId.job_a?.map((s) => s.id)).toEqual(["seg_fresh"]);
    expect(result.current.transcriptSegmentsErrorByJobId.job_a).toBeNull();
  });
});
