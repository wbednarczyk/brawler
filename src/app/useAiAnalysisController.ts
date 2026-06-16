import { useEffect, useRef, useState } from "react";
import type { AiAnalysisJob, FeedItem } from "../api/types";
import { listAiAnalysis, retryAiAnalysis, startAiAnalysis } from "../api/aiAnalysis";

const aiAnalysisPollIntervalMs = 1500;

type AiAnalysisControllerInput = {
  selectedFeedItem: FeedItem | null;
  selectedCompanyFeedItem: FeedItem | null;
};

// Owns the per-feed-item AI-analysis jobs/error/in-flight maps and the
// background polling timers, plus the start/retry commands and the effects that
// load + poll analysis when the selected inbox or company feed item changes.
// Extracted from AppStateRoot to keep that orchestrator focused.
export function useAiAnalysisController({
  selectedFeedItem,
  selectedCompanyFeedItem,
}: AiAnalysisControllerInput) {
  const aiAnalysisPollTimersRef = useRef<Record<string, number>>({});
  const [aiAnalysisJobsByFeedItemId, setAiAnalysisJobsByFeedItemId] = useState<Record<string, AiAnalysisJob[]>>({});
  const [aiAnalysisErrorByFeedItemId, setAiAnalysisErrorByFeedItemId] = useState<Record<string, string | null>>({});
  const [aiAnalysisRequestInFlightByFeedItemId, setAiAnalysisRequestInFlightByFeedItemId] = useState<Record<string, boolean>>({});

  function storeAiAnalysisJobs(feedItemId: string, jobs: AiAnalysisJob[]) {
    setAiAnalysisJobsByFeedItemId((current) => ({
      ...current,
      [feedItemId]: jobs,
    }));
  }

  function setAiAnalysisFeedError(feedItemId: string, error: string | null) {
    setAiAnalysisErrorByFeedItemId((current) => ({
      ...current,
      [feedItemId]: error,
    }));
  }

  function setAiAnalysisFeedInFlight(feedItemId: string, inFlight: boolean) {
    setAiAnalysisRequestInFlightByFeedItemId((current) => ({
      ...current,
      [feedItemId]: inFlight,
    }));
  }

  function aiAnalysisJobIsActive(job: AiAnalysisJob | null | undefined) {
    return job?.status === "queued" || job?.status === "running";
  }

  function clearAiAnalysisPollTimer(feedItemId: string) {
    const timer = aiAnalysisPollTimersRef.current[feedItemId];
    if (timer === undefined) return;

    window.clearTimeout(timer);
    delete aiAnalysisPollTimersRef.current[feedItemId];
  }

  function scheduleAiAnalysisPoll(feedItemId: string) {
    if (aiAnalysisPollTimersRef.current[feedItemId] !== undefined) return;

    aiAnalysisPollTimersRef.current[feedItemId] = window.setTimeout(() => {
      delete aiAnalysisPollTimersRef.current[feedItemId];
      void pollFeedItemAiAnalysis(feedItemId);
    }, aiAnalysisPollIntervalMs);
  }

  async function refreshFeedItemAiAnalysis(feedItemId: string) {
    setAiAnalysisFeedInFlight(feedItemId, true);
    setAiAnalysisFeedError(feedItemId, null);

    try {
      const jobs = await listAiAnalysis({ feedItemId });
      storeAiAnalysisJobs(feedItemId, jobs);
      if (aiAnalysisJobIsActive(jobs[0])) {
        scheduleAiAnalysisPoll(feedItemId);
      }
    } catch (error) {
      setAiAnalysisFeedError(feedItemId, error instanceof Error ? error.message : String(error));
    } finally {
      setAiAnalysisFeedInFlight(feedItemId, false);
    }
  }

  async function pollFeedItemAiAnalysis(feedItemId: string) {
    try {
      const jobs = await listAiAnalysis({ feedItemId });
      storeAiAnalysisJobs(feedItemId, jobs);
      setAiAnalysisFeedError(feedItemId, null);

      if (aiAnalysisJobIsActive(jobs[0])) {
        scheduleAiAnalysisPoll(feedItemId);
      } else {
        clearAiAnalysisPollTimer(feedItemId);
      }
    } catch (error) {
      setAiAnalysisFeedError(feedItemId, error instanceof Error ? error.message : String(error));
    }
  }

  async function startFeedItemAiAnalysis(item: FeedItem, promptPresetId?: string, customQuestion?: string) {
    setAiAnalysisFeedInFlight(item.id, true);
    setAiAnalysisFeedError(item.id, null);

    try {
      const job = await startAiAnalysis({
        feedItemId: item.id,
        promptPresetId,
        customQuestion,
      });
      storeAiAnalysisJobs(item.id, [job, ...(aiAnalysisJobsByFeedItemId[item.id] ?? []).filter((existingJob) => existingJob.id !== job.id)]);
      if (aiAnalysisJobIsActive(job)) {
        scheduleAiAnalysisPoll(item.id);
      }
    } catch (error) {
      setAiAnalysisFeedError(item.id, error instanceof Error ? error.message : String(error));
    } finally {
      setAiAnalysisFeedInFlight(item.id, false);
    }
  }

  async function retryFeedItemAiAnalysis(jobId: string, itemId: string) {
    setAiAnalysisFeedInFlight(itemId, true);
    setAiAnalysisFeedError(itemId, null);

    try {
      const job = await retryAiAnalysis(jobId);
      storeAiAnalysisJobs(itemId, [job, ...(aiAnalysisJobsByFeedItemId[itemId] ?? []).filter((existingJob) => existingJob.id !== job.id)]);
      if (aiAnalysisJobIsActive(job)) {
        scheduleAiAnalysisPoll(itemId);
      }
    } catch (error) {
      setAiAnalysisFeedError(itemId, error instanceof Error ? error.message : String(error));
    } finally {
      setAiAnalysisFeedInFlight(itemId, false);
    }
  }

  const selectedFeedAiAnalysisJob = selectedFeedItem
    ? aiAnalysisJobsByFeedItemId[selectedFeedItem.id]?.[0] ?? null
    : null;
  const selectedCompanyFeedAiAnalysisJob = selectedCompanyFeedItem
    ? aiAnalysisJobsByFeedItemId[selectedCompanyFeedItem.id]?.[0] ?? null
    : null;

  useEffect(() => {
    if (!selectedFeedItem) return;
    if (!aiAnalysisJobsByFeedItemId[selectedFeedItem.id]) {
      void refreshFeedItemAiAnalysis(selectedFeedItem.id);
      return;
    }

    if (aiAnalysisJobIsActive(selectedFeedAiAnalysisJob)) {
      scheduleAiAnalysisPoll(selectedFeedItem.id);
    }
    // eslint-disable-next-line react-hooks/exhaustive-deps -- trigger/poll AI analysis keyed on the selected item + job id/status; the jobs map and the non-memoized refresh/schedule callbacks are intentionally excluded to avoid re-running every render
  }, [selectedFeedItem?.id, selectedFeedAiAnalysisJob?.id, selectedFeedAiAnalysisJob?.status]);

  useEffect(() => {
    if (!selectedCompanyFeedItem) return;
    if (!aiAnalysisJobsByFeedItemId[selectedCompanyFeedItem.id]) {
      void refreshFeedItemAiAnalysis(selectedCompanyFeedItem.id);
      return;
    }

    if (aiAnalysisJobIsActive(selectedCompanyFeedAiAnalysisJob)) {
      scheduleAiAnalysisPoll(selectedCompanyFeedItem.id);
    }
    // eslint-disable-next-line react-hooks/exhaustive-deps -- trigger/poll AI analysis keyed on the selected company item + job id/status; the jobs map and the non-memoized refresh/schedule callbacks are intentionally excluded to avoid re-running every render
  }, [
    selectedCompanyFeedItem?.id,
    selectedCompanyFeedAiAnalysisJob?.id,
    selectedCompanyFeedAiAnalysisJob?.status,
  ]);

  useEffect(() => {
    return () => {
      // eslint-disable-next-line react-hooks/exhaustive-deps -- unmount cleanup intentionally reads the live timers map at teardown (not a mount snapshot) to clear whatever poll timers exist
      Object.keys(aiAnalysisPollTimersRef.current).forEach(clearAiAnalysisPollTimer);
    };
  }, []);

  return {
    aiAnalysisJobsByFeedItemId,
    aiAnalysisErrorByFeedItemId,
    aiAnalysisRequestInFlightByFeedItemId,
    startFeedItemAiAnalysis,
    retryFeedItemAiAnalysis,
  };
}
