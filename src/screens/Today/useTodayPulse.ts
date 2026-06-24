import { useEffect, useMemo, useState } from "react";

import { listClaimsToVerify, type ClaimToVerify } from "../../api/managementClaims";
import type { Company } from "../../api/types";
import { useReportSeason } from "../ReportSeason/useReportSeason";

/** A claim awaiting verification, decorated with its company for the home digest. */
export type PulseClaim = {
  claim: ClaimToVerify["claim"];
  companyId: string;
  qualifiedTicker: string;
  displayName: string;
  bucket: "overdue" | "due";
};

export type TodayPulse = {
  season: ReturnType<typeof useReportSeason>;
  claims: PulseClaim[];
  claimsLoading: boolean;
  claimsError: string | null;
};

/**
 * Today/Pulse attention data (ADR 0054). Composes app-wide read models that
 * already exist: report season (recently-reported + upcoming, one call) and
 * claims-to-verify for the user's **pinned** companies (a bounded set — the
 * curated spine, not every watchlist company — so the home never fans out an
 * unbounded N+1). Full app-wide triage state (accept/snooze/dismiss) and the
 * autonomous-pipeline "one notification" land with the v0.48/v0.49 epics; this
 * is the home structure they plug into.
 */
export function useTodayPulse(pinnedCompanyIds: string[], companies: Company[]): TodayPulse {
  const season = useReportSeason(null);
  const [claims, setClaims] = useState<PulseClaim[]>([]);
  const [claimsLoading, setClaimsLoading] = useState(false);
  const [claimsError, setClaimsError] = useState<string | null>(null);

  const companyById = useMemo(
    () => new Map(companies.map((company) => [company.id, company])),
    [companies],
  );
  // Key on the joined id list so the effect re-runs when the pin set changes,
  // not on every render (settings hands us a fresh array each time).
  const pinnedKey = pinnedCompanyIds.join(",");

  useEffect(() => {
    const ids = pinnedKey ? pinnedKey.split(",") : [];
    if (ids.length === 0) {
      setClaims([]);
      setClaimsLoading(false);
      setClaimsError(null);
      return;
    }

    let cancelled = false;
    setClaimsLoading(true);
    setClaimsError(null);

    Promise.all(
      ids.map(async (id) => {
        const result = await listClaimsToVerify(id);
        const company = companyById.get(id);
        const decorate = (item: ClaimToVerify, bucket: "overdue" | "due"): PulseClaim => ({
          claim: item.claim,
          companyId: id,
          qualifiedTicker: company?.qualifiedTicker ?? "",
          displayName: company?.displayName ?? "",
          bucket,
        });
        return [
          ...result.overdue.map((item) => decorate(item, "overdue")),
          ...result.due.map((item) => decorate(item, "due")),
        ];
      }),
    )
      .then((perCompany) => {
        if (!cancelled) {
          setClaims(perCompany.flat());
        }
      })
      .catch((cause) => {
        if (!cancelled) {
          setClaimsError(cause instanceof Error ? cause.message : String(cause));
        }
      })
      .finally(() => {
        if (!cancelled) {
          setClaimsLoading(false);
        }
      });

    return () => {
      cancelled = true;
    };
  }, [pinnedKey, companyById]);

  return { season, claims, claimsLoading, claimsError };
}
