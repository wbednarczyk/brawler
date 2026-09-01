import { useCallback, useEffect, useMemo, useState } from "react";
import {
  getPreReportCard,
  listReportSeason,
  markReportPrepared,
  markReportProcessed,
  type PreReportCard,
  type ReportSeasonEntry,
  type ReportSeasonResult,
} from "../../api/reportSeason";
import {
  createReportExpectation,
  expectationReview,
  listReportExpectations,
  recordExpectationResolution,
  updateReportExpectation,
  type ExpectationReview,
  type NewExpectationMetric,
  type ReportExpectation,
} from "../../api/reportExpectations";
import { CommandInvocationError } from "../../api/tauri";
import { useCommandQuery } from "../../shared/state/useCommandQuery";

export function entryKey(entry: Pick<ReportSeasonEntry, "companyId" | "eventKey">) {
  return `${entry.companyId}::${entry.eventKey}`;
}

function errorMessage(cause: unknown): string {
  return cause instanceof Error ? cause.message : String(cause);
}

/** The recorded expectation (if any) for an occurrence plus its review read model. */
export type ExpectationEntryState = {
  expectation: ReportExpectation | null;
  review: ExpectationReview | null;
};

/** What the composer submits — the occurrence-agnostic parts of the expectation. */
export type ExpectationDraft = {
  fiscalYear: number;
  periodType: string;
  stanceMd: string;
  metrics: NewExpectationMetric[];
};

export type UseReportSeasonResult = {
  season: ReportSeasonResult | null;
  loading: boolean;
  /** The season-scope read failure only (F4b S4: split from per-card failures below). */
  error: string | null;
  /** Per-card load/action failure, keyed by `entryKey` — `Refresh card` retries just that card. */
  cardErrors: Record<string, string>;
  expandedKey: string | null;
  cards: Record<string, PreReportCard>;
  cardLoadingKey: string | null;
  actionInFlightKey: string | null;
  expectations: Record<string, ExpectationEntryState>;
  expectationBusyKey: string | null;
  toggleExpanded: (entry: ReportSeasonEntry) => void;
  /** `Refresh card` (F4b S4) — re-reads one card + its expectation after a load failure. */
  reloadCard: (entry: ReportSeasonEntry) => void;
  prepare: (entry: ReportSeasonEntry) => void;
  process: (entry: ReportSeasonEntry) => void;
  writeExpectation: (entry: ReportSeasonEntry, draft: ExpectationDraft) => Promise<void>;
  resolveExpectation: (entry: ReportSeasonEntry, note: string) => Promise<void>;
  /** Refetch the season read model (Today per-category error-strip retry, ADR 0087). */
  reload: () => void;
};

/**
 * F4b S4 (contract § Report Season data layer): `season`/`loading`/`error`
 * now come from `useCommandQuery` — one shared self-fetch seam instead of a
 * hand-rolled effect, with `refetch` awaitable so mutations can `await
 * refetch()` before reloading the expanded card (today's ordering kept:
 * mutate → refetch season → reload the expanded card). Per-card/expectation
 * load AND action failures land in `cardErrors[key]`, keyed by occurrence —
 * the season-level `error` is now the read failure only.
 */
export function useReportSeason(watchlistId: string | null): UseReportSeasonResult {
  const season = useCommandQuery([watchlistId], () => listReportSeason({ watchlistId }));

  const [expandedKey, setExpandedKey] = useState<string | null>(null);
  const [cards, setCards] = useState<Record<string, PreReportCard>>({});
  const [cardLoadingKey, setCardLoadingKey] = useState<string | null>(null);
  const [cardErrors, setCardErrors] = useState<Record<string, string>>({});
  const [actionInFlightKey, setActionInFlightKey] = useState<string | null>(null);
  const [expectations, setExpectations] = useState<Record<string, ExpectationEntryState>>({});
  const [expectationBusyKey, setExpectationBusyKey] = useState<string | null>(null);

  useEffect(() => {
    // Reset expansion when the scope changes; cards are scope-independent but
    // the visible set is not.
    setExpandedKey(null);
  }, [watchlistId]);

  function setCardError(key: string, cause: unknown) {
    setCardErrors((current) => ({ ...current, [key]: errorMessage(cause) }));
  }

  function clearCardError(key: string) {
    setCardErrors((current) => {
      if (!(key in current)) return current;
      const next = { ...current };
      delete next[key];
      return next;
    });
  }

  // The occurrence's recorded expectation + review. Fetched via the J2 commands
  // rather than baked into the pre-report card read model: the card is loaded
  // lazily on expand already, so this matches that data-flow and keeps
  // `get_pre_report_card` minimal (ADR 0071 J4 design note).
  const loadExpectation = useCallback(async (entry: ReportSeasonEntry) => {
    const key = entryKey(entry);
    try {
      const list = await listReportExpectations({ companyId: entry.companyId });
      const expectation = list.find((item) => item.eventKey === entry.eventKey) ?? null;
      const review = expectation
        ? await expectationReview({ companyId: entry.companyId, eventKey: entry.eventKey })
        : null;
      setExpectations((current) => ({ ...current, [key]: { expectation, review } }));
    } catch (cause) {
      setCardError(key, cause);
    }
  }, []);

  const loadCard = useCallback(
    async (entry: ReportSeasonEntry) => {
      const key = entryKey(entry);
      setCardLoadingKey(key);
      clearCardError(key);
      try {
        const card = await getPreReportCard({
          companyId: entry.companyId,
          eventKey: entry.eventKey,
        });
        setCards((current) => ({ ...current, [key]: card }));
        await loadExpectation(entry);
      } catch (cause) {
        setCardError(key, cause);
      } finally {
        setCardLoadingKey((current) => (current === key ? null : current));
      }
    },
    [loadExpectation],
  );

  const reloadCard = useCallback(
    (entry: ReportSeasonEntry) => {
      void loadCard(entry);
    },
    [loadCard],
  );

  const toggleExpanded = useCallback(
    (entry: ReportSeasonEntry) => {
      const key = entryKey(entry);
      setExpandedKey((current) => {
        if (current === key) {
          return null;
        }
        if (!cards[key]) {
          void loadCard(entry);
        }
        return key;
      });
    },
    [cards, loadCard],
  );

  const runAction = useCallback(
    async (entry: ReportSeasonEntry, action: (entry: ReportSeasonEntry) => Promise<unknown>) => {
      const key = entryKey(entry);
      setActionInFlightKey(key);
      clearCardError(key);
      try {
        await action(entry);
        await season.refetch();
        if (expandedKey === key) {
          await loadCard(entry);
        }
      } catch (cause) {
        setCardError(key, cause);
      } finally {
        setActionInFlightKey((current) => (current === key ? null : current));
      }
    },
    // season.refetch is stable (useCommandQuery's own useCallback, empty
    // deps) — omitted from deps to avoid re-creating this on every season
    // state change for no behavioral gain (mirrors useAlertsQuery.refetch).
    // eslint-disable-next-line react-hooks/exhaustive-deps
    [expandedKey, loadCard, season.refetch],
  );

  const prepare = useCallback(
    (entry: ReportSeasonEntry) => {
      void runAction(entry, (target) =>
        markReportPrepared({ companyId: target.companyId, eventKey: target.eventKey }),
      );
    },
    [runAction],
  );

  const process = useCallback(
    (entry: ReportSeasonEntry) => {
      void runAction(entry, (target) =>
        markReportProcessed({
          companyId: target.companyId,
          eventKey: target.eventKey,
          linkedReportDocumentId: null,
        }),
      );
    },
    [runAction],
  );

  const writeExpectation = useCallback(
    async (entry: ReportSeasonEntry, draft: ExpectationDraft) => {
      const key = entryKey(entry);
      const existing = expectations[key]?.expectation ?? null;
      setExpectationBusyKey(key);
      try {
        if (existing) {
          await updateReportExpectation({
            companyId: entry.companyId,
            eventKey: entry.eventKey,
            stanceMd: draft.stanceMd,
            metrics: draft.metrics,
          });
        } else {
          await createReportExpectation({
            companyId: entry.companyId,
            eventKey: entry.eventKey,
            fiscalYear: draft.fiscalYear,
            periodType: draft.periodType,
            stanceMd: draft.stanceMd,
            metrics: draft.metrics,
          });
        }
        await loadExpectation(entry);
      } catch (cause) {
        // The freeze race: facts landed between opening the composer and
        // saving. Reload the whole card (not just the expectation) so the UI
        // flips to the read-only review state with fresh data (ADR 0071;
        // F4b S4 contract § Report Season data layer).
        if (cause instanceof CommandInvocationError && cause.code === "conflict") {
          await loadCard(entry);
          return;
        }
        throw cause;
      } finally {
        setExpectationBusyKey((current) => (current === key ? null : current));
      }
    },
    [expectations, loadCard, loadExpectation],
  );

  const resolveExpectation = useCallback(
    async (entry: ReportSeasonEntry, note: string) => {
      const key = entryKey(entry);
      setExpectationBusyKey(key);
      try {
        await recordExpectationResolution({
          companyId: entry.companyId,
          eventKey: entry.eventKey,
          resolutionNoteMd: note,
        });
        await loadExpectation(entry);
      } finally {
        setExpectationBusyKey((current) => (current === key ? null : current));
      }
    },
    [loadExpectation],
  );

  const error = season.status === "error" ? errorMessage(season.error) : null;

  return useMemo(
    () => ({
      season: season.data,
      loading: season.status === "loading",
      error,
      cardErrors,
      expandedKey,
      cards,
      cardLoadingKey,
      actionInFlightKey,
      expectations,
      expectationBusyKey,
      toggleExpanded,
      reloadCard,
      prepare,
      process,
      writeExpectation,
      resolveExpectation,
      reload: () => void season.refetch(),
    }),
    // season.data/status/refetch (not the whole `season` object, a new
    // identity every render) are the pieces this memo actually reads.
    // eslint-disable-next-line react-hooks/exhaustive-deps
    [
      season.data,
      season.status,
      season.refetch,
      error,
      cardErrors,
      expandedKey,
      cards,
      cardLoadingKey,
      actionInFlightKey,
      expectations,
      expectationBusyKey,
      toggleExpanded,
      reloadCard,
      prepare,
      process,
      writeExpectation,
      resolveExpectation,
    ],
  );
}
