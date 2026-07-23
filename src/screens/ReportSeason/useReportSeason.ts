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

export function entryKey(entry: Pick<ReportSeasonEntry, "companyId" | "eventKey">) {
  return `${entry.companyId}::${entry.eventKey}`;
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
  error: string | null;
  expandedKey: string | null;
  cards: Record<string, PreReportCard>;
  cardLoadingKey: string | null;
  actionInFlightKey: string | null;
  expectations: Record<string, ExpectationEntryState>;
  expectationBusyKey: string | null;
  toggleExpanded: (entry: ReportSeasonEntry) => void;
  prepare: (entry: ReportSeasonEntry) => void;
  process: (entry: ReportSeasonEntry) => void;
  writeExpectation: (entry: ReportSeasonEntry, draft: ExpectationDraft) => Promise<void>;
  resolveExpectation: (entry: ReportSeasonEntry, note: string) => Promise<void>;
  /** Refetch the season read model (Today per-category error-strip retry, ADR 0087). */
  reload: () => void;
};

export function useReportSeason(watchlistId: string | null): UseReportSeasonResult {
  const [season, setSeason] = useState<ReportSeasonResult | null>(null);
  const [loading, setLoading] = useState(true);
  const [error, setError] = useState<string | null>(null);
  const [expandedKey, setExpandedKey] = useState<string | null>(null);
  const [cards, setCards] = useState<Record<string, PreReportCard>>({});
  const [cardLoadingKey, setCardLoadingKey] = useState<string | null>(null);
  const [actionInFlightKey, setActionInFlightKey] = useState<string | null>(null);
  const [expectations, setExpectations] = useState<Record<string, ExpectationEntryState>>({});
  const [expectationBusyKey, setExpectationBusyKey] = useState<string | null>(null);

  const refreshSeason = useCallback(async () => {
    setLoading(true);
    setError(null);
    try {
      const result = await listReportSeason({ watchlistId });
      setSeason(result);
    } catch (cause) {
      setError(cause instanceof Error ? cause.message : String(cause));
    } finally {
      setLoading(false);
    }
  }, [watchlistId]);

  useEffect(() => {
    // Reset expansion when the scope changes; cards are scope-independent but
    // the visible set is not.
    setExpandedKey(null);
    void refreshSeason();
  }, [refreshSeason]);

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
      setError(cause instanceof Error ? cause.message : String(cause));
    }
  }, []);

  const loadCard = useCallback(
    async (entry: ReportSeasonEntry) => {
      const key = entryKey(entry);
      setCardLoadingKey(key);
      try {
        const card = await getPreReportCard({
          companyId: entry.companyId,
          eventKey: entry.eventKey,
        });
        setCards((current) => ({ ...current, [key]: card }));
        await loadExpectation(entry);
      } catch (cause) {
        setError(cause instanceof Error ? cause.message : String(cause));
      } finally {
        setCardLoadingKey((current) => (current === key ? null : current));
      }
    },
    [loadExpectation],
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
      setError(null);
      try {
        await action(entry);
        await refreshSeason();
        if (expandedKey === key) {
          await loadCard(entry);
        }
      } catch (cause) {
        setError(cause instanceof Error ? cause.message : String(cause));
      } finally {
        setActionInFlightKey((current) => (current === key ? null : current));
      }
    },
    [expandedKey, loadCard, refreshSeason],
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
      setError(null);
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
        // The freeze race: facts landed between opening the composer and saving.
        // Reload so the UI flips to the read-only review state (ADR 0071).
        if (cause instanceof CommandInvocationError && cause.code === "conflict") {
          await loadExpectation(entry);
          return;
        }
        setError(cause instanceof Error ? cause.message : String(cause));
        throw cause;
      } finally {
        setExpectationBusyKey((current) => (current === key ? null : current));
      }
    },
    [expectations, loadExpectation],
  );

  const resolveExpectation = useCallback(
    async (entry: ReportSeasonEntry, note: string) => {
      const key = entryKey(entry);
      setExpectationBusyKey(key);
      setError(null);
      try {
        await recordExpectationResolution({
          companyId: entry.companyId,
          eventKey: entry.eventKey,
          resolutionNoteMd: note,
        });
        await loadExpectation(entry);
      } catch (cause) {
        setError(cause instanceof Error ? cause.message : String(cause));
        throw cause;
      } finally {
        setExpectationBusyKey((current) => (current === key ? null : current));
      }
    },
    [loadExpectation],
  );

  return useMemo(
    () => ({
      season,
      loading,
      error,
      expandedKey,
      cards,
      cardLoadingKey,
      actionInFlightKey,
      expectations,
      expectationBusyKey,
      toggleExpanded,
      prepare,
      process,
      writeExpectation,
      resolveExpectation,
      reload: () => void refreshSeason(),
    }),
    [
      season,
      loading,
      error,
      expandedKey,
      cards,
      cardLoadingKey,
      actionInFlightKey,
      expectations,
      expectationBusyKey,
      toggleExpanded,
      prepare,
      process,
      writeExpectation,
      resolveExpectation,
      refreshSeason,
    ],
  );
}
