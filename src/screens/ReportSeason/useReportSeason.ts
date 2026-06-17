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

export function entryKey(entry: Pick<ReportSeasonEntry, "companyId" | "eventKey">) {
  return `${entry.companyId}::${entry.eventKey}`;
}

export type UseReportSeasonResult = {
  season: ReportSeasonResult | null;
  loading: boolean;
  error: string | null;
  expandedKey: string | null;
  cards: Record<string, PreReportCard>;
  cardLoadingKey: string | null;
  actionInFlightKey: string | null;
  toggleExpanded: (entry: ReportSeasonEntry) => void;
  prepare: (entry: ReportSeasonEntry) => void;
  process: (entry: ReportSeasonEntry) => void;
};

export function useReportSeason(watchlistId: string | null): UseReportSeasonResult {
  const [season, setSeason] = useState<ReportSeasonResult | null>(null);
  const [loading, setLoading] = useState(true);
  const [error, setError] = useState<string | null>(null);
  const [expandedKey, setExpandedKey] = useState<string | null>(null);
  const [cards, setCards] = useState<Record<string, PreReportCard>>({});
  const [cardLoadingKey, setCardLoadingKey] = useState<string | null>(null);
  const [actionInFlightKey, setActionInFlightKey] = useState<string | null>(null);

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

  const loadCard = useCallback(async (entry: ReportSeasonEntry) => {
    const key = entryKey(entry);
    setCardLoadingKey(key);
    try {
      const card = await getPreReportCard({
        companyId: entry.companyId,
        eventKey: entry.eventKey,
      });
      setCards((current) => ({ ...current, [key]: card }));
    } catch (cause) {
      setError(cause instanceof Error ? cause.message : String(cause));
    } finally {
      setCardLoadingKey((current) => (current === key ? null : current));
    }
  }, []);

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

  return useMemo(
    () => ({
      season,
      loading,
      error,
      expandedKey,
      cards,
      cardLoadingKey,
      actionInFlightKey,
      toggleExpanded,
      prepare,
      process,
    }),
    [
      season,
      loading,
      error,
      expandedKey,
      cards,
      cardLoadingKey,
      actionInFlightKey,
      toggleExpanded,
      prepare,
      process,
    ],
  );
}
