import { useCallback, useEffect, useMemo, useRef, useState } from "react";
import * as researchApi from "../api/research";
import type { Company, Watchlist, WatchlistMembership } from "../api/types";
import type {
  ResearchEvidenceItem,
  ResearchEvidenceType,
  ResearchTimelineResult,
} from "../api/researchTypes";
import type { Section } from "./navigation";

export type ResearchMode = "company" | "watchlist";

type UseResearchControllerInput = {
  activeSection: Section;
  companies: Company[];
  watchlists: Watchlist[];
  watchlistMemberships: WatchlistMembership[];
  text: (value: string) => string;
};

export function useResearchController({
  activeSection,
  companies,
  watchlists,
  watchlistMemberships,
  text,
}: UseResearchControllerInput) {
  const requestVersionRef = useRef(0);
  const textRef = useRef(text);
  const [researchMode, setResearchMode] = useState<ResearchMode>("company");
  const [selectedResearchCompanyId, setSelectedResearchCompanyId] = useState<string | null>(null);
  const [selectedResearchWatchlistId, setSelectedResearchWatchlistId] = useState<string | null>(null);
  const [selectedResearchWatchlistCompanyId, setSelectedResearchWatchlistCompanyId] = useState<string | null>(null);
  const [researchCascadeToCompanies, setResearchCascadeToCompanies] = useState(false);
  const [researchEvidenceTypes, setResearchEvidenceTypes] = useState<ResearchEvidenceType[]>([]);
  const [researchChangedOnly, setResearchChangedOnly] = useState(false);
  const [researchTimeline, setResearchTimeline] = useState<ResearchTimelineResult | null>(null);
  const [researchError, setResearchError] = useState<string | null>(null);
  const [researchLoading, setResearchLoading] = useState(false);
  const [researchReviewInFlight, setResearchReviewInFlight] = useState(false);

  useEffect(() => {
    textRef.current = text;
  }, [text]);

  useEffect(() => {
    if (companies.length === 0) {
      setSelectedResearchCompanyId(null);
      setSelectedResearchWatchlistCompanyId(null);
      setResearchTimeline(null);
      return;
    }

    if (!selectedResearchCompanyId || !companies.some((company) => company.id === selectedResearchCompanyId)) {
      setSelectedResearchCompanyId(companies[0].id);
    }
  }, [companies, selectedResearchCompanyId]);

  useEffect(() => {
    if (watchlists.length === 0) {
      setSelectedResearchWatchlistId(null);
      setSelectedResearchWatchlistCompanyId(null);
      return;
    }

    if (
      !selectedResearchWatchlistId ||
      !watchlists.some((watchlist) => watchlist.id === selectedResearchWatchlistId)
    ) {
      setSelectedResearchWatchlistId(watchlists[0].id);
    }
  }, [selectedResearchWatchlistId, watchlists]);

  const selectedWatchlistCompanyIds = useMemo(
    () =>
      selectedResearchWatchlistId
        ? watchlistMemberships
            .filter((membership) => membership.watchlistId === selectedResearchWatchlistId)
            .map((membership) => membership.companyId)
        : [],
    [selectedResearchWatchlistId, watchlistMemberships],
  );

  useEffect(() => {
    if (selectedWatchlistCompanyIds.length === 0) {
      setSelectedResearchWatchlistCompanyId(null);
      return;
    }

    if (
      !selectedResearchWatchlistCompanyId ||
      !selectedWatchlistCompanyIds.includes(selectedResearchWatchlistCompanyId)
    ) {
      setSelectedResearchWatchlistCompanyId(selectedWatchlistCompanyIds[0]);
    }
  }, [selectedResearchWatchlistCompanyId, selectedWatchlistCompanyIds]);

  const refreshResearchTimeline = useCallback(async () => {
    if (researchMode === "company" && !selectedResearchCompanyId) {
      setResearchTimeline(null);
      return;
    }

    if (researchMode === "watchlist" && !selectedResearchWatchlistId) {
      setResearchTimeline(null);
      return;
    }

    const requestVersion = requestVersionRef.current + 1;
    requestVersionRef.current = requestVersion;
    setResearchLoading(true);
    setResearchError(null);

    try {
      const result = await researchApi.listResearchEvidence({
        companyId: researchMode === "company" ? selectedResearchCompanyId : null,
        watchlistId: researchMode === "watchlist" ? selectedResearchWatchlistId : null,
        evidenceTypes: researchEvidenceTypes.length > 0 ? researchEvidenceTypes : null,
        changedSinceReviewOnly: researchChangedOnly,
        limit: 100,
      });

      if (requestVersionRef.current === requestVersion) {
        setResearchTimeline(result);
      }
    } catch (error) {
      if (requestVersionRef.current === requestVersion) {
        setResearchError(error instanceof Error ? error.message : textRef.current("Research timeline failed"));
      }
    } finally {
      if (requestVersionRef.current === requestVersion) {
        setResearchLoading(false);
      }
    }
  }, [
    researchChangedOnly,
    researchEvidenceTypes,
    researchMode,
    selectedResearchCompanyId,
    selectedResearchWatchlistId,
  ]);

  useEffect(() => {
    if (activeSection !== "Research") {
      return;
    }

    void refreshResearchTimeline();
  }, [activeSection, refreshResearchTimeline]);

  function toggleResearchEvidenceType(evidenceType: ResearchEvidenceType) {
    setResearchEvidenceTypes((current) =>
      current.includes(evidenceType)
        ? current.filter((currentType) => currentType !== evidenceType)
        : [...current, evidenceType],
    );
  }

  function clearResearchEvidenceTypes() {
    setResearchEvidenceTypes([]);
  }

  function setResearchModeAndReset(mode: ResearchMode) {
    setResearchMode(mode);
    setResearchTimeline(null);
    setResearchError(null);
  }

  async function markResearchReviewed() {
    const scopeId =
      researchMode === "company" ? selectedResearchCompanyId : selectedResearchWatchlistId;

    if (!scopeId || researchReviewInFlight) {
      return;
    }

    setResearchReviewInFlight(true);
    setResearchError(null);

    try {
      await researchApi.markResearchScopeReviewed({
        scopeType: researchMode,
        scopeId,
        reviewedAt: new Date().toISOString(),
        cascadeToCompanies: researchMode === "watchlist" ? researchCascadeToCompanies : false,
      });
      await refreshResearchTimeline();
    } catch (error) {
      setResearchError(error instanceof Error ? error.message : textRef.current("Research review update failed"));
    } finally {
      setResearchReviewInFlight(false);
    }
  }

  return {
    researchMode,
    selectedResearchCompanyId,
    selectedResearchWatchlistId,
    selectedResearchWatchlistCompanyId,
    researchCascadeToCompanies,
    researchEvidenceTypes,
    researchChangedOnly,
    researchTimeline,
    researchError,
    researchLoading,
    researchReviewInFlight,
    setResearchMode: setResearchModeAndReset,
    setSelectedResearchCompanyId,
    setSelectedResearchWatchlistId,
    setSelectedResearchWatchlistCompanyId,
    setResearchCascadeToCompanies,
    setResearchChangedOnly,
    toggleResearchEvidenceType,
    clearResearchEvidenceTypes,
    refreshResearchTimeline,
    markResearchReviewed,
  };
}

export type ResearchEvidenceOpenHandler = (item: ResearchEvidenceItem) => void;
