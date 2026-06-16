import type { AiAnalysisJob, CompanySignal, Watchlist, WatchlistMembership } from "../../api/types";
import type { ManagementClaim } from "../../api/managementClaims";
import type {
  EvidenceLink,
  ResearchBriefJob,
  ResearchDigestJob,
  ResearchEvidenceInput,
  ResearchReminder,
  ResearchReviewCheckpoint,
} from "../../api/researchTypes";
import {
  cloneSourceAdapters,
  initialCompanies,
  initialCompanyEvents,
  initialCompanyRegistryEntries,
  initialFeedItems,
  initialGeminiCredentialStatus,
  initialLicenseStatus,
  initialLocalMetricsSnapshot,
  initialResearchEvidenceItems,
  initialResearchQuestions,
  initialSettings,
  initialTranscriptJobs,
  type TestNotebookEntry,
} from "./testData";

export const appTestState = {
  companiesResponse: initialCompanies,
  feedItemsResponse: initialFeedItems,
  researchEvidenceItemsResponse: initialResearchEvidenceItems,
  researchQuestionsResponse: initialResearchQuestions,
  evidenceLinksResponse: [] as EvidenceLink[],
  researchBriefJobsResponse: [] as ResearchBriefJob[],
  researchDigestJobsResponse: [] as ResearchDigestJob[],
  researchRemindersResponse: [] as ResearchReminder[],
  researchReviewCheckpointResponse: null as ResearchReviewCheckpoint | null,
  companyEventsResponse: initialCompanyEvents,
  companySignalsResponse: [] as CompanySignal[],
  transcriptJobsResponse: initialTranscriptJobs,
  companyRegistryEntriesResponse: initialCompanyRegistryEntries,
  watchlistsResponse: [
    {
      id: "watchlist_main_gpw",
      name: "Main GPW",
      description: null,
      companyCount: 1,
    },
  ] as Watchlist[],
  watchlistMembershipsResponse: [
    {
      watchlistId: "watchlist_main_gpw",
      watchlistName: "Main GPW",
      companyId: "company_gpw_cdr",
    },
  ] as WatchlistMembership[],
  notebookEntriesResponse: [] as TestNotebookEntry[],
  managementClaimsResponse: [] as ManagementClaim[],
  aiAnalysisJobsResponse: {} as Record<string, AiAnalysisJob[]>,
  settingsResponse: initialSettings,
  refreshSourcesError: null as string | null,
  geminiCredentialStatusResponse: initialGeminiCredentialStatus,
  localMetricsSnapshotResponse: initialLocalMetricsSnapshot,
  licenseStatusResponse: initialLicenseStatus,
  sourceAdaptersResponse: cloneSourceAdapters(),
  searchResponse: { groups: [] } as { groups: unknown[] },
  backupStatusResponse: { lastBackupAt: null, backupCount: 0, backups: [] } as {
    lastBackupAt: string | null;
    backupCount: number;
    backups: unknown[];
  },
};

export function buildResearchTimeline(input: ResearchEvidenceInput) {
  const selectedTypes = new Set(input.evidenceTypes ?? []);
  const watchlistCompanyIds = input.watchlistId
    ? appTestState.watchlistMembershipsResponse
        .filter((membership) => membership.watchlistId === input.watchlistId)
        .map((membership) => membership.companyId)
    : [];
  const filteredItems = appTestState.researchEvidenceItemsResponse.filter((item) => {
    const companyMatches = !input.companyId || item.companyId === input.companyId;
    const watchlistMatches = !input.watchlistId || watchlistCompanyIds.includes(item.companyId);
    const typeMatches = selectedTypes.size === 0 || selectedTypes.has(item.evidenceType);
    const changedSinceReview = input.watchlistId
      ? item.reviewState.changedSinceWatchlistReview
      : item.reviewState.changedSinceCompanyReview;
    const changedMatches = !input.changedSinceReviewOnly || changedSinceReview;

    return companyMatches && watchlistMatches && typeMatches && changedMatches;
  });
  const memberCompanyIds = input.watchlistId ? watchlistCompanyIds : [];
  const companySummaries = memberCompanyIds.map((companyId) => {
    const companyItems = filteredItems.filter((item) => item.companyId === companyId);
    return {
      companyId,
      total: companyItems.length,
      changedSinceReview: companyItems.filter((item) => item.reviewState.changedSinceWatchlistReview).length,
      lastReviewedAt: null,
    };
  });

  return {
    items: filteredItems.slice(0, input.limit ?? 100),
    summary: {
      total: filteredItems.length,
      changedSinceReview: filteredItems.filter((item) =>
        input.watchlistId
          ? item.reviewState.changedSinceWatchlistReview
          : item.reviewState.changedSinceCompanyReview,
      ).length,
      lastReviewedAt: appTestState.researchReviewCheckpointResponse?.reviewedAt ?? null,
      memberCompanyCount: memberCompanyIds.length,
      companiesWithChangedEvidence: companySummaries.filter((summary) => summary.changedSinceReview > 0).length,
      companySummaries,
    },
  };
}

export function resetAppTestState() {

  appTestState.companiesResponse = initialCompanies;
  appTestState.feedItemsResponse = initialFeedItems;
  appTestState.researchEvidenceItemsResponse = initialResearchEvidenceItems;
  appTestState.researchQuestionsResponse = initialResearchQuestions;
  appTestState.evidenceLinksResponse = [];
  appTestState.researchBriefJobsResponse = [];
  appTestState.researchDigestJobsResponse = [];
  appTestState.researchRemindersResponse = [
    {
      id: "research_reminder_claim_follow_up",
      scopeType: "company",
      scopeId: "company_gpw_cdr",
      companyId: "company_gpw_cdr",
      reminderKind: "claim_follow_up",
      sourceType: "claim",
      sourceId: "note_company_gpw_cdr_follow_up",
      title: "Review open claim follow-up",
      body: "Check whether management delivered.",
      dueAt: "2026-06-08T10:00:00Z",
      status: "open",
      snoozedUntil: null,
      completedAt: null,
      dismissedAt: null,
      createdAt: "2026-06-01T10:00:00Z",
      updatedAt: "2026-06-01T10:00:00Z",
    },
  ];
  appTestState.researchReviewCheckpointResponse = null;
  appTestState.companyEventsResponse = initialCompanyEvents;
  appTestState.companySignalsResponse = [];
  appTestState.transcriptJobsResponse = initialTranscriptJobs;
  appTestState.companyRegistryEntriesResponse = initialCompanyRegistryEntries;
  appTestState.watchlistsResponse = [
    {
      id: "watchlist_main_gpw",
      name: "Main GPW",
      description: null,
      companyCount: 1,
    },
  ];
  appTestState.watchlistMembershipsResponse = [
    {
      watchlistId: "watchlist_main_gpw",
      watchlistName: "Main GPW",
      companyId: "company_gpw_cdr",
    },
  ];
  appTestState.notebookEntriesResponse = [];
  appTestState.managementClaimsResponse = [];
  appTestState.aiAnalysisJobsResponse = {};
  appTestState.settingsResponse = initialSettings;
  appTestState.refreshSourcesError = null;
  appTestState.geminiCredentialStatusResponse = initialGeminiCredentialStatus;
  appTestState.localMetricsSnapshotResponse = initialLocalMetricsSnapshot;
  appTestState.licenseStatusResponse = initialLicenseStatus;
  appTestState.sourceAdaptersResponse = cloneSourceAdapters();
  appTestState.searchResponse = { groups: [] };
  appTestState.backupStatusResponse = { lastBackupAt: null, backupCount: 0, backups: [] };
}
