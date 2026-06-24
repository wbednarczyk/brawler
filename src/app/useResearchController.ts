import { useCallback, useEffect, useMemo, useRef, useState } from "react";
import * as researchApi from "../api/research";
import type { Company, Watchlist, WatchlistMembership } from "../api/types";
import type {
  EvidenceLink,
  ResearchBriefJob,
  ResearchDigestJob,
  ResearchEvidenceItem,
  ResearchEvidenceType,
  ResearchReminderUpdate,
  ResearchQuestion,
  ResearchQuestionStatus,
  ResearchReminder,
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
  const [researchQuestions, setResearchQuestions] = useState<ResearchQuestion[]>([]);
  const [selectedResearchQuestionId, setSelectedResearchQuestionId] = useState<string | null>(null);
  const [researchQuestionTitle, setResearchQuestionTitle] = useState("");
  const [researchQuestionBody, setResearchQuestionBody] = useState("");
  const [researchQuestionLinks, setResearchQuestionLinks] = useState<EvidenceLink[]>([]);
  const [researchBriefJobs, setResearchBriefJobs] = useState<ResearchBriefJob[]>([]);
  const [researchDigestJobs, setResearchDigestJobs] = useState<ResearchDigestJob[]>([]);
  const [researchReminders, setResearchReminders] = useState<ResearchReminder[]>([]);
  const [researchError, setResearchError] = useState<string | null>(null);
  const [researchLoading, setResearchLoading] = useState(false);
  const [researchReviewInFlight, setResearchReviewInFlight] = useState(false);
  const [researchQuestionInFlight, setResearchQuestionInFlight] = useState(false);
  const [researchBriefInFlight, setResearchBriefInFlight] = useState(false);
  const [researchDigestInFlight, setResearchDigestInFlight] = useState(false);
  const [researchReminderInFlight, setResearchReminderInFlight] = useState(false);

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

  const refreshResearchQuestions = useCallback(async () => {
    if (researchMode !== "company" || !selectedResearchCompanyId) {
      setResearchQuestions([]);
      setSelectedResearchQuestionId(null);
      setResearchQuestionLinks([]);
      return;
    }

    try {
      const questions = await researchApi.listResearchQuestions({
        scopeType: "company",
        scopeId: selectedResearchCompanyId,
        status: null,
      });
      setResearchQuestions(questions);
      setSelectedResearchQuestionId((current) =>
        current && questions.some((question) => question.id === current)
          ? current
          : questions[0]?.id ?? null,
      );
    } catch (error) {
      setResearchError(error instanceof Error ? error.message : textRef.current("Research questions failed"));
    }
  }, [researchMode, selectedResearchCompanyId]);

  useEffect(() => {
    if (activeSection !== "Research" && activeSection !== "Cockpit") {
      return;
    }

    void refreshResearchQuestions();
  }, [activeSection, refreshResearchQuestions]);

  useEffect(() => {
    if (!selectedResearchQuestionId) {
      setResearchQuestionLinks([]);
      return;
    }

    let cancelled = false;
    researchApi
      .listEvidenceLinks({
        endpointType: "research_question",
        endpointId: selectedResearchQuestionId,
      })
      .then((links) => {
        if (!cancelled) {
          setResearchQuestionLinks(links);
        }
      })
      .catch((error) => {
        if (!cancelled) {
          setResearchError(error instanceof Error ? error.message : textRef.current("Research links failed"));
        }
      });

    return () => {
      cancelled = true;
    };
  }, [selectedResearchQuestionId]);

  const refreshResearchBriefs = useCallback(async () => {
    const scopeId = researchMode === "company" ? selectedResearchCompanyId : selectedResearchWatchlistId;
    if (!scopeId) {
      setResearchBriefJobs([]);
      return;
    }

    try {
      const jobs = await researchApi.listResearchBriefs({
        scopeType: researchMode,
        scopeId,
      });
      setResearchBriefJobs(jobs);
    } catch (error) {
      setResearchError(error instanceof Error ? error.message : textRef.current("Research briefs failed"));
    }
  }, [researchMode, selectedResearchCompanyId, selectedResearchWatchlistId]);

  const refreshResearchReminders = useCallback(async () => {
    const scopeId = researchMode === "company" ? selectedResearchCompanyId : selectedResearchWatchlistId;
    if (!scopeId) {
      setResearchReminders([]);
      return;
    }

    try {
      const reminders = await researchApi.listResearchReminders({
        scopeType: researchMode,
        scopeId,
        status: null,
      });
      setResearchReminders(reminders);
    } catch (error) {
      setResearchError(error instanceof Error ? error.message : textRef.current("Research reminders failed"));
    }
  }, [researchMode, selectedResearchCompanyId, selectedResearchWatchlistId]);

  const refreshResearchDigests = useCallback(async () => {
    const scopeId = researchMode === "company" ? selectedResearchCompanyId : selectedResearchWatchlistId;
    if (!scopeId) {
      setResearchDigestJobs([]);
      return;
    }

    try {
      const jobs = await researchApi.listResearchDigests({
        scopeType: researchMode,
        scopeId,
      });
      setResearchDigestJobs(jobs);
    } catch (error) {
      setResearchError(error instanceof Error ? error.message : textRef.current("Research digest failed"));
    }
  }, [researchMode, selectedResearchCompanyId, selectedResearchWatchlistId]);

  useEffect(() => {
    if (activeSection !== "Research" && activeSection !== "Cockpit") {
      return;
    }

    void refreshResearchTimeline();
  }, [activeSection, refreshResearchTimeline]);

  useEffect(() => {
    if (activeSection !== "Research" && activeSection !== "Cockpit") {
      return;
    }

    void refreshResearchBriefs();
    void refreshResearchReminders();
    void refreshResearchDigests();
  }, [activeSection, refreshResearchBriefs, refreshResearchDigests, refreshResearchReminders]);

  useEffect(() => {
    if (activeSection !== "Research" && activeSection !== "Cockpit") {
      return;
    }
    if (
      !researchBriefJobs.some((job) => job.status === "queued" || job.status === "running") &&
      !researchDigestJobs.some((job) => job.status === "queued" || job.status === "running")
    ) {
      return;
    }

    const intervalId = window.setInterval(() => {
      void refreshResearchBriefs();
      void refreshResearchDigests();
    }, 1500);

    return () => window.clearInterval(intervalId);
  }, [activeSection, refreshResearchBriefs, refreshResearchDigests, researchBriefJobs, researchDigestJobs]);

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
      await refreshResearchReminders();
    } catch (error) {
      setResearchError(error instanceof Error ? error.message : textRef.current("Research review update failed"));
    } finally {
      setResearchReviewInFlight(false);
    }
  }

  async function createResearchQuestion() {
    if (researchMode !== "company" || !selectedResearchCompanyId || researchQuestionInFlight) {
      return;
    }
    const title = researchQuestionTitle.trim();
    if (!title) {
      return;
    }

    setResearchQuestionInFlight(true);
    setResearchError(null);

    try {
      const question = await researchApi.createResearchQuestion({
        scopeType: "company",
        scopeId: selectedResearchCompanyId,
        title,
        body: researchQuestionBody.trim() || null,
      });
      setResearchQuestionTitle("");
      setResearchQuestionBody("");
      setSelectedResearchQuestionId(question.id);
      await refreshResearchQuestions();
      await refreshResearchTimeline();
    } catch (error) {
      setResearchError(error instanceof Error ? error.message : textRef.current("Research question save failed"));
    } finally {
      setResearchQuestionInFlight(false);
    }
  }

  async function updateResearchQuestionStatus(questionId: string, status: ResearchQuestionStatus) {
    if (researchQuestionInFlight) {
      return;
    }

    setResearchQuestionInFlight(true);
    setResearchError(null);

    try {
      await researchApi.updateResearchQuestion({ id: questionId, status });
      await refreshResearchQuestions();
      await refreshResearchTimeline();
    } catch (error) {
      setResearchError(error instanceof Error ? error.message : textRef.current("Research question update failed"));
    } finally {
      setResearchQuestionInFlight(false);
    }
  }

  async function deleteResearchQuestion(questionId: string) {
    if (researchQuestionInFlight) {
      return;
    }

    const question = researchQuestions.find((current) => current.id === questionId);
    const questionLabel = question?.title ?? textRef.current("this question");
    const confirmed = window.confirm(`${textRef.current("Delete research question")} "${questionLabel}"?`);
    if (!confirmed) {
      return;
    }

    setResearchQuestionInFlight(true);
    setResearchError(null);

    try {
      await researchApi.deleteResearchQuestion(questionId);
      setResearchQuestionLinks([]);
      await refreshResearchQuestions();
      await refreshResearchTimeline();
    } catch (error) {
      setResearchError(error instanceof Error ? error.message : textRef.current("Research question delete failed"));
    } finally {
      setResearchQuestionInFlight(false);
    }
  }

  async function linkEvidenceToSelectedQuestion(item: ResearchEvidenceItem) {
    if (!selectedResearchQuestionId || researchQuestionInFlight) {
      return;
    }
    if (item.evidenceType === "research_question" && item.sourceId === selectedResearchQuestionId) {
      return;
    }

    setResearchQuestionInFlight(true);
    setResearchError(null);

    try {
      await researchApi.createEvidenceLink({
        fromType: "research_question",
        fromId: selectedResearchQuestionId,
        toType: item.evidenceType,
        toId: item.sourceId,
        relationType: "related",
      });
      setResearchQuestionLinks(
        await researchApi.listEvidenceLinks({
          endpointType: "research_question",
          endpointId: selectedResearchQuestionId,
        }),
      );
    } catch (error) {
      setResearchError(error instanceof Error ? error.message : textRef.current("Evidence link failed"));
    } finally {
      setResearchQuestionInFlight(false);
    }
  }

  async function unlinkEvidenceFromSelectedQuestion(linkId: string) {
    if (researchQuestionInFlight) {
      return;
    }

    setResearchQuestionInFlight(true);
    setResearchError(null);

    try {
      await researchApi.deleteEvidenceLink(linkId);
      setResearchQuestionLinks((current) => current.filter((link) => link.id !== linkId));
    } catch (error) {
      setResearchError(error instanceof Error ? error.message : textRef.current("Evidence unlink failed"));
    } finally {
      setResearchQuestionInFlight(false);
    }
  }

  async function startResearchBrief() {
    const scopeId = researchMode === "company" ? selectedResearchCompanyId : selectedResearchWatchlistId;
    if (!scopeId || researchBriefInFlight) {
      return;
    }

    setResearchBriefInFlight(true);
    setResearchError(null);

    try {
      await researchApi.startResearchBrief({
        scopeType: researchMode,
        scopeId,
      });
      await refreshResearchBriefs();
    } catch (error) {
      setResearchError(error instanceof Error ? error.message : textRef.current("Research brief failed"));
    } finally {
      setResearchBriefInFlight(false);
    }
  }

  async function startResearchDigest() {
    const scope = activeResearchScope();
    const scopeId = scope?.scopeId;
    if (!scopeId || researchDigestInFlight) {
      return;
    }

    setResearchDigestInFlight(true);
    setResearchError(null);

    try {
      await researchApi.startResearchDigest({
        scopeType: scope.scopeType,
        scopeId,
      });
      await refreshResearchDigests();
    } catch (error) {
      setResearchError(error instanceof Error ? error.message : textRef.current("Research digest failed"));
    } finally {
      setResearchDigestInFlight(false);
    }
  }

  function activeResearchScope() {
    const scopeId = researchMode === "company" ? selectedResearchCompanyId : selectedResearchWatchlistId;
    if (!scopeId) {
      return null;
    }
    return {
      scopeType: researchMode,
      scopeId,
    };
  }

  async function completeResearchReminder(reminderId: string) {
    await updateResearchReminder({ id: reminderId, status: "completed" });
  }

  async function snoozeResearchReminder(reminderId: string) {
    const tomorrow = new Date();
    tomorrow.setDate(tomorrow.getDate() + 1);
    await updateResearchReminder({
      id: reminderId,
      status: "open",
      snoozedUntil: tomorrow.toISOString(),
    });
  }

  async function reopenResearchReminder(reminderId: string) {
    await updateResearchReminder({
      id: reminderId,
      status: "open",
      snoozedUntil: null,
    });
  }

  async function updateResearchReminder(input: ResearchReminderUpdate) {
    if (researchReminderInFlight) {
      return;
    }

    setResearchReminderInFlight(true);
    setResearchError(null);

    try {
      await researchApi.updateResearchReminder(input);
      await refreshResearchReminders();
    } catch (error) {
      setResearchError(error instanceof Error ? error.message : textRef.current("Research reminder update failed"));
    } finally {
      setResearchReminderInFlight(false);
    }
  }

  async function deleteResearchReminder(reminderId: string) {
    if (researchReminderInFlight) {
      return;
    }

    const confirmed = window.confirm(textRef.current("Delete research reminder?"));
    if (!confirmed) {
      return;
    }

    setResearchReminderInFlight(true);
    setResearchError(null);

    try {
      await researchApi.deleteResearchReminder(reminderId);
      await refreshResearchReminders();
    } catch (error) {
      setResearchError(error instanceof Error ? error.message : textRef.current("Research reminder delete failed"));
    } finally {
      setResearchReminderInFlight(false);
    }
  }

  async function createResearchReminder(title: string, body: string, dueAt: string | null) {
    if (researchReminderInFlight || !title.trim()) {
      return;
    }
    const scope = activeResearchScope();
    if (!scope) {
      return;
    }

    setResearchReminderInFlight(true);
    setResearchError(null);

    try {
      await researchApi.createResearchReminder({
        scopeType: scope.scopeType,
        scopeId: scope.scopeId,
        companyId: scope.scopeType === "company" ? scope.scopeId : selectedResearchWatchlistCompanyId,
        reminderKind: "manual_research",
        sourceType: null,
        sourceId: null,
        title: title.trim(),
        body: body.trim() || null,
        dueAt: dueAt?.trim() ? new Date(dueAt).toISOString() : null,
      });
      await refreshResearchReminders();
    } catch (error) {
      setResearchError(error instanceof Error ? error.message : textRef.current("Research reminder create failed"));
    } finally {
      setResearchReminderInFlight(false);
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
    researchQuestions,
    selectedResearchQuestionId,
    researchQuestionTitle,
    researchQuestionBody,
    researchQuestionLinks,
    researchBriefJobs,
    researchDigestJobs,
    researchReminders,
    researchError,
    researchLoading,
    researchReviewInFlight,
    researchQuestionInFlight,
    researchBriefInFlight,
    researchDigestInFlight,
    researchReminderInFlight,
    setResearchMode: setResearchModeAndReset,
    setSelectedResearchCompanyId,
    setSelectedResearchWatchlistId,
    setSelectedResearchWatchlistCompanyId,
    setSelectedResearchQuestionId,
    setResearchQuestionTitle,
    setResearchQuestionBody,
    setResearchCascadeToCompanies,
    setResearchChangedOnly,
    toggleResearchEvidenceType,
    clearResearchEvidenceTypes,
    refreshResearchTimeline,
    refreshResearchQuestions,
    refreshResearchBriefs,
    refreshResearchReminders,
    refreshResearchDigests,
    markResearchReviewed,
    createResearchQuestion,
    updateResearchQuestionStatus,
    deleteResearchQuestion,
    linkEvidenceToSelectedQuestion,
    unlinkEvidenceFromSelectedQuestion,
    startResearchBrief,
    startResearchDigest,
    createResearchReminder,
    completeResearchReminder,
    snoozeResearchReminder,
    reopenResearchReminder,
    deleteResearchReminder,
  };
}

export type ResearchEvidenceOpenHandler = (item: ResearchEvidenceItem) => void;
