import { useEffect, useState } from "react";
import type { Company } from "../../../api/types";
import { listShortPositions, type ShortPositionsView } from "../../../api/shortPositions";

// Company-scoped KNF short-selling state for one company (v0.55 T4b, ADR 0069
// decision 3). Read-only: the register is populated by the daily
// `knf-short-selling` adapter; this just loads the per-company view (active
// positions, change history, aggregate net short %, 30-day pp change) so the
// "Krótka sprzedaż (KNF)" panel works for any company. Mirrors the
// fetch-on-company-change shape of the other company-scoped panel hooks.
export function useShortPositionsPanel(company: Company) {
  const [view, setView] = useState<ShortPositionsView | null>(null);
  const [error, setError] = useState<string | null>(null);

  useEffect(() => {
    let active = true;
    setView(null);
    setError(null);
    listShortPositions({ companyId: company.id })
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

  return { view, error };
}
