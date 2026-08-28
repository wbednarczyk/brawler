import { useCallback, useEffect, useState } from "react";

import type { Company } from "../../../api/types";
import { acknowledgeRedFlag, getRedFlags, type RedFlagsView } from "../../../api/redFlags";

// Cockpit-native red-flags state for one company (v0.57 T7, ADR 0083 D8). Loads
// the computed panel view (active flags + acknowledged history) on company change
// and exposes an idempotent acknowledge that swaps in the refreshed view the
// command returns. Mirrors the fetch-on-company-change shape of the other
// company-scoped cockpit hooks.
export function useRedFlagsPanel(company: Company) {
  const [view, setView] = useState<RedFlagsView | null>(null);
  const [error, setError] = useState<string | null>(null);

  useEffect(() => {
    let active = true;
    setView(null);
    setError(null);
    getRedFlags(company.id)
      .then((result) => {
        if (active) setView(result);
      })
      .catch((reason) => {
        if (active) setError(String(reason));
      });
    return () => {
      active = false;
    };
  }, [company.id]);

  const acknowledge = useCallback((flagId: string) => {
    return acknowledgeRedFlag(flagId)
      .then((result) => setView(result))
      .catch((reason) => setError(String(reason)));
  }, []);

  return { view, error, acknowledge };
}
