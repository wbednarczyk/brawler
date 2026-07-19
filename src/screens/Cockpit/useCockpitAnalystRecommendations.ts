import { useCallback, useEffect, useState } from "react";

import type { Company } from "../../api/types";
import {
  getAnalystRecommendations,
  type AnalystRecommendationsView,
} from "../../api/analystRecommendations";
import { getPriceContext } from "../../api/marketData";

// Cockpit-native analyst-recommendations state for one company (v0.58 A3, ADR
// 0073). Loads the attributed history on company change and, best-effort, the
// current close from Price context so each row can show the "vs price" upside —
// a price failure never blocks the panel. `reload` re-reads on the error-state
// retry (stale data stays visible meanwhile). Mirrors the fetch-on-company-change
// shape of the other company-scoped cockpit hooks.
export function useCockpitAnalystRecommendations(company: Company) {
  const [view, setView] = useState<AnalystRecommendationsView | null>(null);
  const [error, setError] = useState<string | null>(null);
  const [loading, setLoading] = useState(true);
  const [lastClose, setLastClose] = useState<number | null>(null);
  const [currency, setCurrency] = useState<string | null>(null);
  const [reloadKey, setReloadKey] = useState(0);

  useEffect(() => {
    let active = true;
    setLoading(true);
    setError(null);
    getAnalystRecommendations(company.id)
      .then((result) => {
        if (active) setView(result);
      })
      .catch((reason) => {
        // Keep any previously-shown data (stale-but-shown); surface the error.
        if (active) setError(String(reason));
      })
      .finally(() => {
        if (active) setLoading(false);
      });
    return () => {
      active = false;
    };
  }, [company.id, reloadKey]);

  // Current close for the per-row upside — best-effort, never gates the panel.
  useEffect(() => {
    let active = true;
    setLastClose(null);
    setCurrency(null);
    getPriceContext(company.id)
      .then((context) => {
        if (!active) return;
        if (!context.emptyReason) {
          setLastClose(context.lastClose);
          setCurrency(context.currency);
        }
      })
      .catch(() => {
        /* no price context → rows omit the "vs price" delta */
      });
    return () => {
      active = false;
    };
  }, [company.id]);

  const reload = useCallback(() => setReloadKey((key) => key + 1), []);

  return { view, error, loading, lastClose, currency, reload };
}
