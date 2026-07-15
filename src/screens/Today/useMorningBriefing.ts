import { useCallback, useEffect, useRef, useState } from "react";

import { generateMorningBriefing, getLatestMorningBriefing } from "../../api/briefing";
import type { MorningBriefing } from "../../api/briefing";

// Poll horizon for the on-demand generate -> refetch flow (ADR 0068 decision 4,
// v0.54 §T5). The real compose job runs on the durable queue's `ai` lane, not
// synchronously with `generate_morning_briefing`, so the card cannot just
// re-fetch once — it polls `get_latest_morning_briefing` until a briefing newer
// than the one it started with appears, or gives up. Mirrors the bounded-poll
// idiom `CompanyCoveragePanel` (`src/shared/components/CompanyCoveragePanel.tsx`)
// already uses for backfill/sweep progress: an immediate first tick (a fast
// mock/test compose settles at once) then a fixed interval, capped so a stuck
// queue does not poll forever.
const POLL_INTERVAL_MS = 1500;
const POLL_MAX_ATTEMPTS = 8;

export type MorningBriefingState = {
  briefing: MorningBriefing | null;
  /** Initial fetch of the latest briefing on mount. */
  loading: boolean;
  /** An on-demand generate is in flight (enqueue + poll for the result). */
  generating: boolean;
  error: string | null;
  /** Enqueue a fresh on-demand compose and poll for its result. */
  generate: () => void;
};

/**
 * Today's morning-briefing card data (ADR 0068 decision 4). Loads the latest
 * composed briefing on mount; `generate()` enqueues an on-demand recompose and
 * polls until a new briefing appears (or the poll horizon gives up) — no
 * background/interval refresh otherwise, matching Today's existing
 * mount-only-refresh idiom (`useTodayPulse` has no interval poller either).
 */
export function useMorningBriefing(): MorningBriefingState {
  const [briefing, setBriefing] = useState<MorningBriefing | null>(null);
  const [loading, setLoading] = useState(true);
  const [generating, setGenerating] = useState(false);
  const [error, setError] = useState<string | null>(null);
  const pollRef = useRef<ReturnType<typeof setInterval> | null>(null);
  const priorIdRef = useRef<string | null>(null);

  const stopPolling = useCallback(() => {
    if (pollRef.current !== null) {
      clearInterval(pollRef.current);
      pollRef.current = null;
    }
  }, []);

  useEffect(() => {
    let cancelled = false;
    getLatestMorningBriefing()
      .then((value) => {
        if (cancelled) return;
        setBriefing(value);
        priorIdRef.current = value?.id ?? null;
      })
      .catch(() => {
        if (!cancelled) setBriefing(null);
      })
      .finally(() => {
        if (!cancelled) setLoading(false);
      });
    return () => {
      cancelled = true;
      stopPolling();
    };
  }, [stopPolling]);

  const generate = useCallback(() => {
    if (generating) return;
    setGenerating(true);
    setError(null);
    stopPolling();

    let attempts = 0;
    const tick = () => {
      attempts += 1;
      getLatestMorningBriefing()
        .then((value) => {
          if (value && value.id !== priorIdRef.current) {
            priorIdRef.current = value.id;
            setBriefing(value);
            stopPolling();
            setGenerating(false);
            return;
          }
          if (attempts >= POLL_MAX_ATTEMPTS) {
            stopPolling();
            setGenerating(false);
          }
        })
        .catch(() => {
          // Transient read failure — keep polling until the give-up horizon.
        });
    };

    generateMorningBriefing()
      .then(() => {
        tick();
        pollRef.current = setInterval(tick, POLL_INTERVAL_MS);
      })
      .catch((cause) => {
        setError(cause instanceof Error ? cause.message : String(cause));
        setGenerating(false);
      });
  }, [generating, stopPolling]);

  return { briefing, loading, generating, error, generate };
}
