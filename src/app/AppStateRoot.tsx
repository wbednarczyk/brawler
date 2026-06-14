import {
  type CSSProperties,
  type FormEvent,
  type KeyboardEvent,
  useEffect,
  useMemo,
  useRef,
  useState,
} from "react";
import { openUrl } from "@tauri-apps/plugin-opener";
import { AppShell } from "./AppShell";
import type { DbRefreshState, SourceRefreshState } from "./appTypes";
import { useAppDataController } from "./useAppDataController";
import { useCompanyController } from "./useCompanyController";
import { useCompanyEventsController } from "./useCompanyEventsController";
import { useDetailPaneResize } from "./useDetailPaneResize";
import {
  companyEventStatusOptions,
  companyEventTypeOptions,
  emptyCompanyEventForm,
} from "./eventForms";
import { useFeedController } from "./useFeedController";
import { detailPaneDefaultWidth, detailPaneMaxWidth, detailPaneMinWidth } from "./layout";
import { type Section } from "./navigation";
import {
  emptyNotebookForm,
  manualNotebookOrigins,
} from "./notebookForms";
import { feedPruneRetentionDays } from "./sourceScheduler";
import * as sourcesApi from "../api/sources";
import * as financialsApi from "../api/financials";
import { emptyTranscriptJobForm } from "./transcriptForms";
import { useFundamentalsController } from "./useFundamentalsController";
import { useAppLifecycleEffects } from "./useAppLifecycleEffects";
import { useAppViewModel } from "./useAppViewModel";
import { useNotebookController } from "./useNotebookController";
import { useLicenseController } from "./useLicenseController";
import { useResearchController } from "./useResearchController";
import { useSettingsController } from "./useSettingsController";
import { useSourceDisplayController } from "./useSourceDisplayController";
import { useSourceRefreshController } from "./useSourceRefreshController";
import { useTranscriptController } from "./useTranscriptController";
import { useWorkspaceNavigationController } from "./useWorkspaceNavigationController";
import { resolveAppShortcutReferenceItems, type AppShortcutActionMap } from "./shortcuts";
import { CompaniesScreen } from "../screens/Companies/CompaniesScreen";
import type { CompanyWorkspaceTab } from "../screens/Companies/companyTypes";
import { DiagnosticsScreen } from "../screens/Diagnostics/DiagnosticsScreen";
import { EventsScreen } from "../screens/Events/EventsScreen";
import { InboxScreen } from "../screens/Inbox/InboxScreen";
import type { InboxStatusFilter } from "../screens/Inbox/inboxTypes";
import { NotebooksScreen } from "../screens/Notebooks/NotebooksScreen";
import { ResearchScreen } from "../screens/Research/ResearchScreen";
import { SettingsScreen } from "../screens/Settings/SettingsScreen";
import { SourcesScreen } from "../screens/Sources/SourcesScreen";
import { TranscriptsScreen } from "../screens/Transcripts/TranscriptsScreen";
import { WatchlistsScreen } from "../screens/Watchlists/WatchlistsScreen";
import type { TranscriptJobForm } from "../screens/Transcripts/transcriptTypes";
import { MarkdownNoteBody } from "../shared/components/MarkdownNoteBody";
import { NotebookDateField } from "../shared/components/NotebookDateField";
import { NotebookQuarterField } from "../shared/components/NotebookQuarterField";
import {
  addLocalDays,
  companyEventDueClass,
  companyEventDueLabel,
  formatLocalDate,
  formatPollInterval,
  formatTimestamp,
  formatWeekRange,
  parseLocalDate,
} from "../shared/formatting/date";
import {
  formatAiProvider,
  formatCompanyEventSourceType,
  formatCompanyEventStatus,
  formatCompanyEventType,
  formatCredentialConfigured,
  formatCredentialKind,
  formatEnumLabel,
  formatGeminiModel,
} from "../shared/formatting/labels";
import { LocaleContext, makeTextTranslator, makeTranslator } from "../shared/locale";
import type { CompanyEventForm, CompanyEventMode, CompanyEventViewMode } from "../shared/types/events";
import type { NotebookForm } from "../shared/types/notebook";
import type {
  AiAnalysisJob,
  Company,
  CompanyForm,
  CompanyEvent,
  CompanyRegistryEntry,
  CompanyRegistryRefreshResult,
  CredentialStatus,
  DatabaseStatus,
  FeedDeleteResult,
  FeedItem,
  FeedPruneResult,
  HealthResponse,
  LicenseStatus,
  NotebookDraftOrigin,
  NotebookEntry,
  SourceAdapter,
  SourceIngestionResult,
  Theme,
  TranscriptJob,
  TranscriptSegment,
  UnmatchedSourceItem,
  UserSettings,
  Watchlist,
  WatchlistMembership,
} from "../api/types";
import {
  listAiAnalysis,
  retryAiAnalysis,
  startAiAnalysis,
} from "../api/aiAnalysis";
import type { ResearchEvidenceItem } from "../api/researchTypes";

type AppStateRootProps = {
  initialLicenseStatus?: LicenseStatus | null;
};

const aiAnalysisPollIntervalMs = 1500;

export function AppStateRoot({ initialLicenseStatus = null }: AppStateRootProps) {
  const contentGridRef = useRef<HTMLElement | null>(null);
  const sourceRefreshInFlightRef = useRef(false);
  const sourceAdaptersRef = useRef<SourceAdapter[]>([]);
  const eventWeekFetchAttemptedRef = useRef<Set<string>>(new Set());
  const aiAnalysisPollTimersRef = useRef<Record<string, number>>({});
  const companyLookupVersionRef = useRef(0);
  const skipNextCompanyLookupRef = useRef(false);
  const companyFieldRefs = useRef<Record<keyof CompanyForm, HTMLInputElement | null>>({
    exchange: null,
    ticker: null,
    displayName: null,
    isin: null,
  });
  const [activeSection, setActiveSection] = useState<Section>("Inbox");
  const [theme, setTheme] = useState<Theme>("dark");
  const [accentPalette, setAccentPalette] = useState<UserSettings["accentPalette"]>("night-neon");
  const [locale, setLocale] = useState<UserSettings["locale"]>("en");
  const [health, setHealth] = useState<HealthResponse | null>(null);
  const [healthError, setHealthError] = useState<string | null>(null);
  const [databaseStatus, setDatabaseStatus] = useState<DatabaseStatus | null>(null);
  const [databaseError, setDatabaseError] = useState<string | null>(null);
  const [companies, setCompanies] = useState<Company[]>([]);
  const [companiesError, setCompaniesError] = useState<string | null>(null);
  const [watchlists, setWatchlists] = useState<Watchlist[]>([]);
  const [watchlistMemberships, setWatchlistMemberships] = useState<WatchlistMembership[]>([]);
  const [watchlistsError, setWatchlistsError] = useState<string | null>(null);
  const [notebookEntries, setNotebookEntries] = useState<NotebookEntry[]>([]);
  const [notebookError, setNotebookError] = useState<string | null>(null);
  const [inboxWatchlistFilter, setInboxWatchlistFilter] = useState("all");
  const [inboxCompanyFilter, setInboxCompanyFilter] = useState("all");
  const [inboxTypeFilter, setInboxTypeFilter] = useState("all");
  const [inboxSourceFilter, setInboxSourceFilter] = useState("all");
  const [inboxStatusFilter, setInboxStatusFilter] = useState<InboxStatusFilter>("all");
  const [feedState, setFeedState] = useState<FeedItem[]>([]);
  const [feedError, setFeedError] = useState<string | null>(null);
  const [aiAnalysisJobsByFeedItemId, setAiAnalysisJobsByFeedItemId] = useState<Record<string, AiAnalysisJob[]>>({});
  const [aiAnalysisErrorByFeedItemId, setAiAnalysisErrorByFeedItemId] = useState<Record<string, string | null>>({});
  const [aiAnalysisRequestInFlightByFeedItemId, setAiAnalysisRequestInFlightByFeedItemId] = useState<Record<string, boolean>>({});
  const [companyEvents, setCompanyEvents] = useState<CompanyEvent[]>([]);
  const [companyEventsError, setCompanyEventsError] = useState<string | null>(null);
  const [transcriptJobs, setTranscriptJobs] = useState<TranscriptJob[]>([]);
  const [transcriptJobsError, setTranscriptJobsError] = useState<string | null>(null);
  const [transcriptJobForm, setTranscriptJobForm] = useState<TranscriptJobForm>(emptyTranscriptJobForm);
  const [transcriptJobCreateError, setTranscriptJobCreateError] = useState<string | null>(null);
  const [transcriptJobCreateState, setTranscriptJobCreateState] = useState<DbRefreshState>("idle");
  const [transcriptJobRunInFlight, setTranscriptJobRunInFlight] = useState<string | null>(null);
  const [selectedTranscriptJobId, setSelectedTranscriptJobId] = useState<string | null>(null);
  const [transcriptSegmentsByJobId, setTranscriptSegmentsByJobId] = useState<Record<string, TranscriptSegment[]>>({});
  const [transcriptSegmentsErrorByJobId, setTranscriptSegmentsErrorByJobId] = useState<Record<string, string | null>>({});
  const [transcriptSegmentSearchByJobId, setTranscriptSegmentSearchByJobId] = useState<Record<string, string>>({});
  const [selectedTranscriptSegmentIdsByJobId, setSelectedTranscriptSegmentIdsByJobId] = useState<Record<string, string[]>>({});
  const [transcriptNoteDraftJobId, setTranscriptNoteDraftJobId] = useState<string | null>(null);
  const [transcriptNoteForm, setTranscriptNoteForm] = useState<NotebookForm>(emptyNotebookForm);
  const [transcriptNoteErrorByJobId, setTranscriptNoteErrorByJobId] = useState<Record<string, string | null>>({});
  const [transcriptNoteSaveInFlight, setTranscriptNoteSaveInFlight] = useState<string | null>(null);
  const [transcriptLinkQueryByJobId, setTranscriptLinkQueryByJobId] = useState<Record<string, string>>({});
  const [transcriptLinkErrorByJobId, setTranscriptLinkErrorByJobId] = useState<Record<string, string | null>>({});
  const [transcriptLinkInFlight, setTranscriptLinkInFlight] = useState<string | null>(null);
  const [transcriptDeleteInFlight, setTranscriptDeleteInFlight] = useState<string | null>(null);
  const [transcriptDescriptionDraftByJobId, setTranscriptDescriptionDraftByJobId] = useState<Record<string, string>>({});
  const [transcriptDescriptionErrorByJobId, setTranscriptDescriptionErrorByJobId] = useState<Record<string, string | null>>({});
  const [transcriptDescriptionSaveInFlight, setTranscriptDescriptionSaveInFlight] = useState<string | null>(null);
  const [companyEventViewMode, setCompanyEventViewMode] = useState<CompanyEventViewMode>("week");
  const [companyEventMode, setCompanyEventMode] = useState<CompanyEventMode>("upcoming");
  const [companyEventWeekAnchorDate, setCompanyEventWeekAnchorDate] = useState(() =>
    formatLocalDate(new Date()),
  );
  const [companyEventWatchlistFilter, setCompanyEventWatchlistFilter] = useState("all");
  const [companyEventCompanyFilter, setCompanyEventCompanyFilter] = useState("all");
  const [companyEventTypeFilter, setCompanyEventTypeFilter] = useState("all");
  const [companyEventStatusFilter, setCompanyEventStatusFilter] = useState("all");
  const [companyEventDateFrom, setCompanyEventDateFrom] = useState("");
  const [companyEventDateTo, setCompanyEventDateTo] = useState("");
  const [selectedCompanyEventId, setSelectedCompanyEventId] = useState<string | null>(null);
  const [isCompanyEventComposerOpen, setCompanyEventComposerOpen] = useState(false);
  const [companyEventForm, setCompanyEventForm] = useState<CompanyEventForm>(emptyCompanyEventForm);
  const [companyEventCreateError, setCompanyEventCreateError] = useState<string | null>(null);
  const [sourceAdapters, setSourceAdapters] = useState<SourceAdapter[]>([]);
  const [sourceAdaptersError, setSourceAdaptersError] = useState<string | null>(null);
  const [selectedSourceAdapterId, setSelectedSourceAdapterId] = useState<string | null>(null);
  const [settings, setSettings] = useState<UserSettings | null>(null);
  const [settingsError, setSettingsError] = useState<string | null>(null);
  const [licenseStatus, setLicenseStatus] = useState<LicenseStatus | null>(initialLicenseStatus);
  const [licenseError, setLicenseError] = useState<string | null>(null);
  const [licenseKeyDraft, setLicenseKeyDraft] = useState("");
  const [licenseInFlight, setLicenseInFlight] = useState(false);
  const [geminiCredentialStatus, setGeminiCredentialStatus] = useState<CredentialStatus | null>(null);
  const [geminiCredentialError, setGeminiCredentialError] = useState<string | null>(null);
  const [geminiApiKeyDraft, setGeminiApiKeyDraft] = useState("");
  const [geminiCredentialInFlight, setGeminiCredentialInFlight] = useState(false);
  const [selectedFeedItemId, setSelectedFeedItemId] = useState<string | null>(null);
  const [selectedCompanyId, setSelectedCompanyId] = useState<string | null>(null);
  const [workspaceAutoFocusId, setWorkspaceAutoFocusId] = useState<string | null>(null);
  const [selectedCompanyFeedItemId, setSelectedCompanyFeedItemId] = useState<string | null>(null);
  const [selectedNotebookEntryId, setSelectedNotebookEntryId] = useState<string | null>(null);
  const [isNotebookComposerOpen, setNotebookComposerOpen] = useState(false);
  const [isNotebookEditMode, setNotebookEditMode] = useState(false);
  const [selectedClaimEntryId, setSelectedClaimEntryId] = useState<string | null>(null);
  const [claimStatusDraft, setClaimStatusDraft] = useState("");
  const [selectedNotebookCompanyId, setSelectedNotebookCompanyId] = useState<string | null>(null);
  const [selectedNotebookScreenEntryId, setSelectedNotebookScreenEntryId] = useState<string | null>(null);
  const [isNotebookScreenComposerOpen, setNotebookScreenComposerOpen] = useState(false);
  const [isNotebookScreenEditMode, setNotebookScreenEditMode] = useState(false);
  const [notebookScreenKindFilter, setNotebookScreenKindFilter] = useState("all");
  const [notebookScreenWatchlistFilter, setNotebookScreenWatchlistFilter] = useState("all");
  const [notebookScreenClaimStatusFilter, setNotebookScreenClaimStatusFilter] = useState("all");
  const [notebookScreenFollowUpFilter, setNotebookScreenFollowUpFilter] = useState("all");
  const [notebookScreenTagFilter, setNotebookScreenTagFilter] = useState("");
  const [notebookScreenForm, setNotebookScreenForm] = useState<NotebookForm>(emptyNotebookForm);
  const [notebookScreenDraftOrigins, setNotebookScreenDraftOrigins] =
    useState<NotebookDraftOrigin[]>(manualNotebookOrigins);
  const [notebookScreenEditForm, setNotebookScreenEditForm] = useState<NotebookForm>(emptyNotebookForm);
  const [companyWorkspaceTab, setCompanyWorkspaceTab] = useState<CompanyWorkspaceTab>("Feed");
  const [detailPaneWidth, setDetailPaneWidth] = useState(detailPaneDefaultWidth);
  const [dbRefreshState, setDbRefreshState] = useState<DbRefreshState>("idle");
  const [deleteUnsavedFeedState, setDeleteUnsavedFeedState] = useState<DbRefreshState>("idle");
  const [deleteUnsavedFeedError, setDeleteUnsavedFeedError] = useState<string | null>(null);
  const [feedPruneResult, setFeedPruneResult] = useState<FeedPruneResult | null>(null);
  const [sourceRefreshState, setSourceRefreshState] = useState<SourceRefreshState>("idle");
  const [sourceRefreshResult, setSourceRefreshResult] = useState<SourceIngestionResult | null>(null);
  const [sourceRefreshError, setSourceRefreshError] = useState<string | null>(null);
  const [sourceRefreshFailureCount, setSourceRefreshFailureCount] = useState(0);
  const [sourceAdapterRefreshInFlight, setSourceAdapterRefreshInFlight] = useState<string | null>(null);
  const [registryRefreshState, setRegistryRefreshState] = useState<SourceRefreshState>("idle");
  const [registryRefreshResult, setRegistryRefreshResult] = useState<CompanyRegistryRefreshResult | null>(null);
  const [registryRefreshError, setRegistryRefreshError] = useState<string | null>(null);
  const [nextSourceRefreshAtByAdapterId, setNextSourceRefreshAtByAdapterId] = useState<Record<string, number>>({});
  const [nextRegistryRefreshAt, setNextRegistryRefreshAt] = useState<number | null>(null);
  const [unmatchedSourceItems, setUnmatchedSourceItems] = useState<Record<string, UnmatchedSourceItem[]>>({});
  const [unmatchedSourceItemsError, setUnmatchedSourceItemsError] = useState<string | null>(null);
  const [expandedUnmatchedAdapters, setExpandedUnmatchedAdapters] = useState<Record<string, boolean>>({});
  const [companyRegistryEntries, setCompanyRegistryEntries] = useState<CompanyRegistryEntry[]>([]);
  const [companyRegistryEntriesError, setCompanyRegistryEntriesError] = useState<string | null>(null);
  const [isCompanyRegistryListExpanded, setCompanyRegistryListExpanded] = useState(false);
  const [companyRegistrySearch, setCompanyRegistrySearch] = useState("");
  const [selectedCompanyRegistryTicker, setSelectedCompanyRegistryTicker] = useState<string | null>(null);
  const [addingRegistryTicker, setAddingRegistryTicker] = useState<string | null>(null);
  const [companyListSearch, setCompanyListSearch] = useState("");
  const [companyWatchlistFilter, setCompanyWatchlistFilter] = useState("all");
  const [selectedManagedWatchlistId, setSelectedManagedWatchlistId] = useState<string | null>(null);
  const [searchQuery, setSearchQuery] = useState("");
  const [lookupStatus, setLookupStatus] = useState<string | null>(null);
  const [companyForm, setCompanyForm] = useState<CompanyForm>({
    exchange: "GPW",
    ticker: "",
    displayName: "",
    isin: "",
  });
  const [notebookForm, setNotebookForm] = useState<NotebookForm>(emptyNotebookForm);
  const [notebookEditForm, setNotebookEditForm] = useState<NotebookForm>(emptyNotebookForm);
  const [financialPeriods, setFinancialPeriods] = useState<any[]>([]);
  const [financialFacts, setFinancialFacts] = useState<any[]>([]);
  const [kpiDefinitions, setKpiDefinitions] = useState<any[]>([]);
  const [financialPeriodsError, setFinancialPeriodsError] = useState<string | null>(null);
  const [financialFactsError, setFinancialFactsError] = useState<string | null>(null);
  const [kpiDefinitionsError, setKpiDefinitionsError] = useState<string | null>(null);
  const [fundamentalsForm, setFundamentalsForm] = useState({ periodFiscalYear: "", periodType: "annual" });
  const [financialFactForm, setFinancialFactForm] = useState({ definitionId: "", valueNumeric: "", currency: "", periodId: "" });
  const [selectedFinancialFactId, setSelectedFinancialFactId] = useState<string | null>(null);
  const [isFinancialFactEditMode, setIsFinancialFactEditMode] = useState(false);
  const [fundamentalsError, setFundamentalsError] = useState<string | null>(null);

  const {
    companiesById,
    companyEventStatuses,
    companyEventTypes,
    companyEventWeekRange,
    companyEventWeekendDays,
    companyEventWeekendEvents,
    companyEventWorkingWeekDays,
    companyEventsByDate,
    companyFormRegistryMatches,
    effectiveTheme,
    feedSources,
    feedTypes,
    filteredCompanies,
    filteredCompanyRegistryEntries,
    filteredFeedItems,
    filteredNotebookScreenCompanies,
    hasActiveInboxFilters,
    inboxEmptyState,
    inboxReviewStats,
    isNotebookEditDirty,
    isNotebookScreenEditDirty,
    membershipsByCompany,
    registryAdapter,
    scheduledSourceAdapterKey,
    scheduledSourceAdapters,
    selectedClaimEntry,
    selectedCompany,
    selectedCompanyClaimEntries,
    selectedCompanyEvent,
    selectedCompanyFeedItem,
    selectedCompanyFeedItems,
    selectedCompanyFeedStats,
    selectedCompanyNotebookEntries,
    selectedFeedCompany,
    selectedFeedItem,
    selectedNotebookEntry,
    selectedNotebookScreenCompany,
    selectedNotebookScreenEntries,
    selectedNotebookScreenEntry,
    sourceStatusSummary,
    totalUnreadFeedItems,
    transcriptCompanySuggestions,
  } = useAppViewModel({
    companies,
    companyEventViewMode,
    companyEventWeekAnchorDate,
    companyEvents,
    companyForm,
    companyListSearch,
    companyWatchlistFilter,
    companyRegistryEntries,
    companyRegistrySearch,
    feedState,
    inboxCompanyFilter,
    inboxSourceFilter,
    inboxStatusFilter,
    inboxTypeFilter,
    inboxWatchlistFilter,
    notebookEditForm,
    notebookEntries,
    notebookScreenWatchlistFilter,
    notebookScreenClaimStatusFilter,
    notebookScreenEditForm,
    notebookScreenFollowUpFilter,
    notebookScreenKindFilter,
    notebookScreenTagFilter,
    searchQuery,
    selectedClaimEntryId,
    selectedCompanyFeedItemId,
    selectedCompanyId,
    selectedCompanyRegistryTicker,
    selectedCompanyEventId,
    selectedFeedItemId,
    selectedNotebookCompanyId,
    selectedNotebookEntryId,
    selectedNotebookScreenEntryId,
    settings,
    sourceAdapters,
    sourceAdaptersError,
    theme,
    transcriptJobForm,
    watchlistMemberships,
  });

  const text = makeTextTranslator(locale);
  const shortcutBindings = settings?.shortcutBindings ?? {};
  const shortcutReferences = resolveAppShortcutReferenceItems(shortcutBindings);
  const licenseCanUseApp = licenseStatus?.canUseApp !== false;

  const {
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
    setResearchMode,
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
  } = useResearchController({
    activeSection,
    companies,
    watchlists,
    watchlistMemberships,
    text,
  });

  const {
    clearCompanyEventFilters,
    createCompanyEvent,
    openCompanyEventComposer,
    refreshCompanyEvents,
  } = useCompanyEventsController({
    companies,
    companyEventCompanyFilter,
    companyEventDateFrom,
    companyEventDateTo,
    companyEventForm,
    companyEventMode,
    companyEventStatusFilter,
    companyEventTypeFilter,
    companyEventViewMode,
    companyEventWatchlistFilter,
    companyEventWeekAnchorDate,
    companyEventWeekRange,
    setCompanyEventCompanyFilter,
    setCompanyEventComposerOpen,
    setCompanyEventCreateError,
    setCompanyEventDateFrom,
    setCompanyEventDateTo,
    setCompanyEventForm,
    setCompanyEventStatusFilter,
    setCompanyEventTypeFilter,
    setCompanyEvents,
    setCompanyEventsError,
    setCompanyEventWatchlistFilter,
    setSelectedCompanyEventId,
  });

  const {
    deleteUnsavedFeedItems,
    pruneOldFeedItems,
    refreshCompanies,
    refreshCompanyRegistryEntries,
    refreshDatabaseBackedViews,
    refreshDatabaseStatus,
    refreshFeedItems,
    refreshGeminiCredentialStatus,
    refreshHealth,
    refreshSettings,
    refreshSourceAdapters,
    refreshUnmatchedSourceItems,
    refreshWatchlistMemberships,
    refreshWatchlists,
  } = useAppDataController({
    feedPruneRetentionDays,
    refreshCompanyEvents,
    setCompanies,
    setCompaniesError,
    setCompanyRegistryEntries,
    setCompanyRegistryEntriesError,
    setDatabaseError,
    setDatabaseStatus,
    setDbRefreshState,
    setDeleteUnsavedFeedError,
    setDeleteUnsavedFeedState,
    setFeedError,
    setFeedPruneResult,
    setFeedState,
    setGeminiCredentialError,
    setGeminiCredentialStatus,
    setHealth,
    setHealthError,
    setSelectedFeedItemId,
    setSettings,
    setSettingsError,
    setAccentPalette,
    setLocale,
    setSourceAdapters,
    setSourceAdaptersError,
    setTheme,
    setUnmatchedSourceItems,
    setUnmatchedSourceItemsError,
    setWatchlistMemberships,
    setWatchlists,
    setWatchlistsError,
    text,
  });

  const {
    clearLicenseKey,
    refreshLicenseStatus,
    submitLicenseKey,
  } = useLicenseController({
    licenseKeyDraft,
    setLicenseError,
    setLicenseInFlight,
    setLicenseKeyDraft,
    setLicenseStatus,
  });

  const {
    refreshBankierCalendarWeek,
    refreshCompanyRegistry,
    refreshCompanyRegistryIfStale,
    refreshEventSources,
    refreshScheduledSource,
    refreshSingleSource,
    refreshSources,
  } = useSourceRefreshController({
    refreshCompanyEvents,
    refreshCompanyRegistryEntries,
    refreshDatabaseStatus,
    refreshFeedItems,
    refreshSourceAdapters,
    refreshUnmatchedSourceItems,
    scheduledSourceAdapters,
    settings,
    sourceAdapterRefreshInFlight,
    sourceAdaptersRef,
    sourceRefreshInFlightRef,
    setRegistryRefreshError,
    setRegistryRefreshResult,
    setRegistryRefreshState,
    setSelectedSourceAdapterId,
    setSourceAdapterRefreshInFlight,
    setSourceRefreshError,
    setSourceRefreshFailureCount,
    setSourceRefreshResult,
    setSourceRefreshState,
  });

  const {
    clearGeminiApiKey,
    disableDeveloperMode,
    saveGeminiApiKey,
    unlockDeveloperMode,
    updateAccentPalette,
    updateLocale,
    updateGeneralAnalysisModel,
    updateGeneralAnalysisProvider,
    updateGeneralAnalysisTimeout,
    updatePollInterval,
    updateLogLevel,
    updateLogMaxFileBytes,
    updateLogMaxFiles,
    updateShortcutBindings,
    updateTheme,
    updateYoutubeTranscriptionModel,
    updateYoutubeTranscriptionTimeout,
  } = useSettingsController({
    geminiApiKeyDraft,
    setGeminiApiKeyDraft,
    setGeminiCredentialError,
    setGeminiCredentialInFlight,
    setGeminiCredentialStatus,
    setSettings,
    setSettingsError,
    setAccentPalette,
    setLocale,
    setTheme,
    text,
  });

  useEffect(() => {
    if (settings?.developerMode) {
      return undefined;
    }

    function unlockFromHiddenChord(event: globalThis.KeyboardEvent) {
      if (!event.ctrlKey || !event.altKey || !event.shiftKey || event.key.toLowerCase() !== "d") {
        return;
      }

      event.preventDefault();
      const passphrase = window.prompt("Developer mode unlock");
      if (!passphrase) {
        return;
      }

      unlockDeveloperMode(passphrase);
    }

    window.addEventListener("keydown", unlockFromHiddenChord);

    return () => {
      window.removeEventListener("keydown", unlockFromHiddenChord);
    };
  }, [settings?.developerMode, unlockDeveloperMode]);

  useEffect(() => {
    if (activeSection === "Diagnostics" && !settings?.developerMode) {
      setActiveSection("Settings");
    }
  }, [activeSection, settings?.developerMode]);

  function resetDeletedWatchlistFilters(watchlistId: string) {
    setInboxWatchlistFilter((current) => (current === watchlistId ? "all" : current));
    setCompanyEventWatchlistFilter((current) => (current === watchlistId ? "all" : current));
    setCompanyWatchlistFilter((current) => (current === watchlistId ? "all" : current));
    setNotebookScreenWatchlistFilter((current) => (current === watchlistId ? "all" : current));
    setSelectedManagedWatchlistId((current) => (current === watchlistId ? null : current));
  }

  function openWatchlistFromCompanyRow(watchlistId: string) {
    setSelectedManagedWatchlistId(watchlistId);
    setActiveSection("Watchlists");
  }

  const {
    addCompanyToWatchlist,
    addCompanyFromRegistry,
    applyRegistryEntryToCompanyForm,
    clearCompanyFormField,
    createCompany,
    createWatchlist,
    deleteWatchlist,
    deleteCompany,
    lookupCompany,
    lookupCompanyIfUseful,
    removeCompanyFromWatchlist,
    renameWatchlist,
    updateCompanyForm,
  } = useCompanyController({
    companyFieldRefs,
    companyForm,
    companyLookupVersionRef,
    refreshCompanies,
    refreshCompanyRegistryEntries,
    refreshDatabaseStatus,
    refreshWatchlistMemberships,
    refreshWatchlists,
    setAddingRegistryTicker,
    setCompaniesError,
    setCompanyForm,
    setCompanyListSearch,
    setCompanyWatchlistFilter,
    setLookupStatus,
    setSelectedCompanyRegistryTicker,
    setWatchlistsError,
    skipNextCompanyLookupRef,
    resetDeletedWatchlistFilters,
    text,
  });

  const {
    cancelNotebookEdit,
    cancelNotebookScreenEdit,
    createNotebookEntry,
    createNotebookScreenEntry,
    deleteNotebookScreenEntry,
    discardNotebookScreenDraft,
    feedItemSummary,
    openFeedItemNoteDraft,
    refreshNotebookEntries,
    saveClaimStatus,
    saveNotebookEntry,
    saveNotebookScreenEntry,
    selectNotebookScreenCompany,
    showNotebookCompanyFollowUps,
    showNotebookCompanyOpenClaims,
    toggleClaimEntry,
    toggleNotebookScreenComposer,
    toggleNotebookScreenEntry,
    updateNotebookEditForm,
    updateNotebookForm,
    updateNotebookScreenEditForm,
    updateNotebookScreenForm,
  } = useNotebookController({
    claimStatusDraft,
    companies,
    notebookEditForm,
    notebookForm,
    notebookScreenDraftOrigins,
    notebookScreenEditForm,
    notebookScreenForm,
    selectedCompany,
    selectedNotebookCompanyId,
    selectedNotebookEntry,
    selectedNotebookScreenCompany,
    selectedNotebookScreenEntry,
    setActiveSection,
    setNotebookComposerOpen,
    setNotebookEditForm,
    setNotebookEditMode,
    setNotebookEntries,
    setNotebookError,
    setNotebookForm,
    setNotebookScreenComposerOpen,
    setNotebookScreenDraftOrigins,
    setNotebookScreenEditForm,
    setNotebookScreenEditMode,
    setNotebookScreenFollowUpFilter,
    setNotebookScreenForm,
    setNotebookScreenKindFilter,
    setNotebookScreenClaimStatusFilter,
    setNotebookScreenTagFilter,
    setSelectedClaimEntryId,
    setSelectedNotebookCompanyId,
    setSelectedNotebookEntryId,
    setSelectedNotebookScreenEntryId,
    text,
  });

  const {
    createTranscriptJob,
    createTranscriptNotebookEntry,
    deleteTranscriptJob,
    discardTranscriptNoteDraft,
    linkTranscriptJobCompany,
    openTranscriptNoteDraft,
    refreshTranscriptJobs,
    refreshTranscriptSegments,
    runTranscriptJob,
    selectTranscriptCompany,
    toggleTranscriptJob,
    toggleTranscriptJobFromKeyboard,
    toggleTranscriptSegment,
    updateTranscriptJobDescription,
    updateTranscriptLinkQuery,
    updateTranscriptNoteForm,
  } = useTranscriptController({
    geminiCredentialStatus,
    refreshNotebookEntries,
    selectedTranscriptJobId,
    selectedTranscriptSegmentIdsByJobId,
    settings,
    setNotebookEntries,
    setSelectedNotebookCompanyId,
    setSelectedNotebookScreenEntryId,
    setSelectedTranscriptJobId,
    setSelectedTranscriptSegmentIdsByJobId,
    setTranscriptDeleteInFlight,
    setTranscriptDescriptionDraftByJobId,
    setTranscriptDescriptionErrorByJobId,
    setTranscriptDescriptionSaveInFlight,
    setTranscriptJobCreateError,
    setTranscriptJobCreateState,
    setTranscriptJobForm,
    setTranscriptJobRunInFlight,
    setTranscriptJobs,
    setTranscriptJobsError,
    setTranscriptLinkErrorByJobId,
    setTranscriptLinkInFlight,
    setTranscriptLinkQueryByJobId,
    setTranscriptNoteDraftJobId,
    setTranscriptNoteErrorByJobId,
    setTranscriptNoteForm,
    setTranscriptNoteSaveInFlight,
    setTranscriptSegmentsByJobId,
    setTranscriptSegmentsErrorByJobId,
    transcriptDescriptionDraftByJobId,
    transcriptJobForm,
    transcriptNoteForm,
    text,
  });

  const {
    clearInboxFilters,
    inspectCompanyFeedItem,
    markVisibleInboxAsRead,
    openCompanyWorkspaceFromFeedItem,
    selectFeedItemFromKeyboard,
    toggleFeedItemReadState,
    updateFeedItemState,
    updateSelectedFeedItem,
  } = useFeedController({
    companies,
    filteredFeedItems,
    selectedFeedItem,
    setActiveSection,
    setCompanyWorkspaceTab,
    setFeedError,
    setFeedState,
    setInboxCompanyFilter,
    setInboxSourceFilter,
    setInboxStatusFilter,
    setInboxTypeFilter,
    setInboxWatchlistFilter,
    setSearchQuery,
    setSelectedCompanyFeedItemId,
    setSelectedCompanyId,
    setSelectedFeedItemId,
    setWorkspaceAutoFocusId,
  });

  async function refreshFinancialPeriods() {
    if (!selectedCompanyId) return;
    try {
      const periods = await financialsApi.listFinancialPeriods({ companyId: selectedCompanyId });
      setFinancialPeriods(periods);
      setFinancialPeriodsError(null);
    } catch (error) {
      setFinancialPeriodsError(error instanceof Error ? error.message : String(error));
    }
  }

  async function refreshFinancialFacts() {
    if (!selectedCompanyId) return;
    try {
      const facts = await financialsApi.listFinancialFacts({ companyId: selectedCompanyId });
      setFinancialFacts(facts);
      setFinancialFactsError(null);
    } catch (error) {
      setFinancialFactsError(error instanceof Error ? error.message : String(error));
    }
  }

  async function refreshKpiDefinitions() {
    try {
      // Load all definitions (canonical + sector + any global), not just one
      // scope: confirmed facts reference canonical/sector definitions, and the
      // fundamentals matrix needs them to resolve labels and render rows.
      const definitions = await financialsApi.listKpiDefinitions({});
      setKpiDefinitions(definitions);
      setKpiDefinitionsError(null);
    } catch (error) {
      setKpiDefinitionsError(error instanceof Error ? error.message : String(error));
    }
  }

  const {
    createFinancialPeriod,
    saveFinancialFact,
    deleteFinancialFact,
    selectFinancialFact,
    startEditingFinancialFact,
    cancelEditingFinancialFact,
    updateFundamentalsForm,
    updateFinancialFactForm,
  } = useFundamentalsController({
    companyId: selectedCompanyId || "",
    financialPeriods,
    financialFacts,
    kpiDefinitions,
    fundamentalsForm,
    setFundamentalsForm,
    financialFactForm,
    setFinancialFactForm,
    selectedFinancialFactId,
    setSelectedFinancialFactId,
    isFinancialFactEditMode,
    setIsFinancialFactEditMode,
    fundamentalsError,
    setFundamentalsError,
    refreshFinancialPeriods,
    refreshFinancialFacts,
    refreshKpiDefinitions,
    text,
  });

  function storeAiAnalysisJobs(feedItemId: string, jobs: AiAnalysisJob[]) {
    setAiAnalysisJobsByFeedItemId((current) => ({
      ...current,
      [feedItemId]: jobs,
    }));
  }

  function setAiAnalysisFeedError(feedItemId: string, error: string | null) {
    setAiAnalysisErrorByFeedItemId((current) => ({
      ...current,
      [feedItemId]: error,
    }));
  }

  function setAiAnalysisFeedInFlight(feedItemId: string, inFlight: boolean) {
    setAiAnalysisRequestInFlightByFeedItemId((current) => ({
      ...current,
      [feedItemId]: inFlight,
    }));
  }

  function aiAnalysisJobIsActive(job: AiAnalysisJob | null | undefined) {
    return job?.status === "queued" || job?.status === "running";
  }

  function clearAiAnalysisPollTimer(feedItemId: string) {
    const timer = aiAnalysisPollTimersRef.current[feedItemId];
    if (timer === undefined) return;

    window.clearTimeout(timer);
    delete aiAnalysisPollTimersRef.current[feedItemId];
  }

  function scheduleAiAnalysisPoll(feedItemId: string) {
    if (aiAnalysisPollTimersRef.current[feedItemId] !== undefined) return;

    aiAnalysisPollTimersRef.current[feedItemId] = window.setTimeout(() => {
      delete aiAnalysisPollTimersRef.current[feedItemId];
      void pollFeedItemAiAnalysis(feedItemId);
    }, aiAnalysisPollIntervalMs);
  }

  async function refreshFeedItemAiAnalysis(feedItemId: string) {
    setAiAnalysisFeedInFlight(feedItemId, true);
    setAiAnalysisFeedError(feedItemId, null);

    try {
      const jobs = await listAiAnalysis({ feedItemId });
      storeAiAnalysisJobs(feedItemId, jobs);
      if (aiAnalysisJobIsActive(jobs[0])) {
        scheduleAiAnalysisPoll(feedItemId);
      }
    } catch (error) {
      setAiAnalysisFeedError(feedItemId, error instanceof Error ? error.message : String(error));
    } finally {
      setAiAnalysisFeedInFlight(feedItemId, false);
    }
  }

  async function pollFeedItemAiAnalysis(feedItemId: string) {
    try {
      const jobs = await listAiAnalysis({ feedItemId });
      storeAiAnalysisJobs(feedItemId, jobs);
      setAiAnalysisFeedError(feedItemId, null);

      if (aiAnalysisJobIsActive(jobs[0])) {
        scheduleAiAnalysisPoll(feedItemId);
      } else {
        clearAiAnalysisPollTimer(feedItemId);
      }
    } catch (error) {
      setAiAnalysisFeedError(feedItemId, error instanceof Error ? error.message : String(error));
    }
  }

  async function startFeedItemAiAnalysis(item: FeedItem, promptPresetId?: string, customQuestion?: string) {
    setAiAnalysisFeedInFlight(item.id, true);
    setAiAnalysisFeedError(item.id, null);

    try {
      const job = await startAiAnalysis({
        feedItemId: item.id,
        promptPresetId,
        customQuestion,
      });
      storeAiAnalysisJobs(item.id, [job, ...(aiAnalysisJobsByFeedItemId[item.id] ?? []).filter((existingJob) => existingJob.id !== job.id)]);
      if (aiAnalysisJobIsActive(job)) {
        scheduleAiAnalysisPoll(item.id);
      }
    } catch (error) {
      setAiAnalysisFeedError(item.id, error instanceof Error ? error.message : String(error));
    } finally {
      setAiAnalysisFeedInFlight(item.id, false);
    }
  }

  async function retryFeedItemAiAnalysis(jobId: string, itemId: string) {
    setAiAnalysisFeedInFlight(itemId, true);
    setAiAnalysisFeedError(itemId, null);

    try {
      const job = await retryAiAnalysis(jobId);
      storeAiAnalysisJobs(itemId, [job, ...(aiAnalysisJobsByFeedItemId[itemId] ?? []).filter((existingJob) => existingJob.id !== job.id)]);
      if (aiAnalysisJobIsActive(job)) {
        scheduleAiAnalysisPoll(itemId);
      }
    } catch (error) {
      setAiAnalysisFeedError(itemId, error instanceof Error ? error.message : String(error));
    } finally {
      setAiAnalysisFeedInFlight(itemId, false);
    }
  }

  const selectedFeedAiAnalysisJob = selectedFeedItem
    ? aiAnalysisJobsByFeedItemId[selectedFeedItem.id]?.[0] ?? null
    : null;
  const selectedCompanyFeedAiAnalysisJob = selectedCompanyFeedItem
    ? aiAnalysisJobsByFeedItemId[selectedCompanyFeedItem.id]?.[0] ?? null
    : null;

  useEffect(() => {
    if (!selectedFeedItem) return;
    if (!aiAnalysisJobsByFeedItemId[selectedFeedItem.id]) {
      void refreshFeedItemAiAnalysis(selectedFeedItem.id);
      return;
    }

    if (aiAnalysisJobIsActive(selectedFeedAiAnalysisJob)) {
      scheduleAiAnalysisPoll(selectedFeedItem.id);
    }
  }, [selectedFeedItem?.id, selectedFeedAiAnalysisJob?.id, selectedFeedAiAnalysisJob?.status]);

  useEffect(() => {
    if (!selectedCompanyFeedItem) return;
    if (!aiAnalysisJobsByFeedItemId[selectedCompanyFeedItem.id]) {
      void refreshFeedItemAiAnalysis(selectedCompanyFeedItem.id);
      return;
    }

    if (aiAnalysisJobIsActive(selectedCompanyFeedAiAnalysisJob)) {
      scheduleAiAnalysisPoll(selectedCompanyFeedItem.id);
    }
  }, [
    selectedCompanyFeedItem?.id,
    selectedCompanyFeedAiAnalysisJob?.id,
    selectedCompanyFeedAiAnalysisJob?.status,
  ]);

  useEffect(() => {
    return () => {
      Object.keys(aiAnalysisPollTimersRef.current).forEach(clearAiAnalysisPollTimer);
    };
  }, []);

  const {
    focusCompanyWorkspace,
    openCompanyInboxFilter,
    openCompanyWorkspace,
    openCompanyWorkspaceFromKeyboard,
    renderNotebookOrigins,
    selectCompanyFeedItemFromKeyboard,
    toggleCompanyFeedItem,
  } = useWorkspaceNavigationController({
    companiesById,
    feedState,
    selectedCompanyFeedItemId,
    selectedCompanyId,
    setActiveSection,
    setCompanyWorkspaceTab,
    setInboxCompanyFilter,
    setInboxSourceFilter,
    setInboxStatusFilter,
    setInboxTypeFilter,
    setInboxWatchlistFilter,
    setSearchQuery,
    setSelectedCompanyFeedItemId,
    setSelectedCompanyId,
    setSelectedFeedItemId,
  });

  const {
    formatNextRefresh,
    formatSourceScheduler,
    formatSourceTrigger,
    openSourceStatus,
    toggleCompanyRegistryList,
    toggleSourceAdapter,
    toggleSourceAdapterFromKeyboard,
    toggleUnmatchedSourceItems,
  } = useSourceDisplayController({
    nextRegistryRefreshAt,
    nextSourceRefreshAtByAdapterId,
    refreshCompanyRegistryEntries,
    refreshUnmatchedSourceItems,
    setActiveSection,
    setCompanyRegistryListExpanded,
    setExpandedUnmatchedAdapters,
    setSelectedSourceAdapterId,
    settings,
    sourceAdapters,
    sourceRefreshFailureCount,
  });

  function setSourceEnabled(adapter: SourceAdapter, enabled: boolean) {
    if (!adapter.userConfigurable || adapter.enabled === enabled) {
      return;
    }

    sourcesApi.setSourceAdapterEnabled({ adapterId: adapter.id, enabled })
      .then((updatedAdapter) => {
        setSourceAdapters((current) =>
          current.map((sourceAdapter) =>
            sourceAdapter.id === updatedAdapter.id ? updatedAdapter : sourceAdapter,
          ),
        );
        setSourceAdaptersError(null);
      })
      .catch((error) => {
        setSourceAdaptersError(String(error));
        refreshSourceAdapters();
      });
  }

  const {
    resizeDetailPane,
    resizeDetailPaneWithKeyboard,
    startDetailPaneResize,
    stopDetailPaneResize,
  } = useDetailPaneResize({
    contentGridRef,
    setDetailPaneWidth,
  });

  useAppLifecycleEffects({
    activeSection,
    companies,
    companyEventCompanyFilter,
    companyEventDateFrom,
    companyEventDateTo,
    companyEventMode,
    companyEventStatusFilter,
    companyEventTypeFilter,
    companyEventViewMode,
    companyEventWatchlistFilter,
    companyEventWeekRange,
    accentPalette,
    effectiveTheme,
    eventWeekFetchAttemptedRef,
    filteredFeedItems,
    pruneOldFeedItems,
    refreshBankierCalendarWeek,
    refreshCompanies,
    refreshCompanyEvents,
    refreshCompanyRegistryEntries,
    refreshCompanyRegistryIfStale,
    refreshDatabaseStatus,
    refreshFeedItems,
    refreshGeminiCredentialStatus,
    refreshHealth,
    refreshLicenseStatus,
    refreshNotebookEntries,
    refreshScheduledSource,
    refreshSettings,
    refreshSourceAdapters,
    refreshTranscriptJobs,
    refreshWatchlistMemberships,
    refreshWatchlists,
    registryAdapter,
    scheduledSourceAdapterKey,
    scheduledSourceAdapters,
    selectedClaimEntry,
    selectedCompanyId,
    selectedFeedItemId,
    selectedNotebookCompanyId,
    selectedNotebookEntry,
    selectedNotebookScreenEntry,
    settings,
    licenseCanUseApp,
    setClaimStatusDraft,
    setNextRegistryRefreshAt,
    setNextSourceRefreshAtByAdapterId,
    setNotebookEditForm,
    setNotebookEditMode,
    setNotebookScreenEditForm,
    setNotebookScreenEditMode,
    setSelectedClaimEntryId,
    setSelectedFeedItemId,
    setSelectedNotebookCompanyId,
    setSelectedNotebookEntryId,
    sourceAdapters,
    sourceAdaptersRef,
    sourceRefreshFailureCount,
  });

  useEffect(() => {
    if (selectedCompanyId && activeSection === "Companies" && companyWorkspaceTab === "Fundamentals") {
      void refreshFinancialPeriods();
      void refreshFinancialFacts();
      void refreshKpiDefinitions();
    }
  }, [selectedCompanyId, activeSection, companyWorkspaceTab]);

  function openExternalUrl(url: string) {
    void openUrl(url).catch((error) => {
      console.error("Failed to open external URL", error);
    });
  }

  function openResearchEvidence(item: ResearchEvidenceItem) {
    const itemCompanyTicker = companiesById[item.companyId]?.qualifiedTicker ?? "all";

    switch (item.sourceDomain) {
      case "feed":
        setInboxCompanyFilter(itemCompanyTicker);
        setInboxStatusFilter("all");
        setInboxTypeFilter("all");
        setInboxSourceFilter("all");
        setInboxWatchlistFilter("all");
        setSearchQuery("");
        setSelectedFeedItemId(item.sourceId);
        setActiveSection("Inbox");
        break;
      case "notebooks":
        setSelectedNotebookCompanyId(item.companyId);
        setSelectedNotebookScreenEntryId(item.sourceId);
        setActiveSection("Notebooks");
        break;
      case "research":
        if (item.evidenceType === "research_question") {
          if (researchMode !== "company") {
            setResearchMode("company");
          }
          if (selectedResearchCompanyId !== item.companyId) {
            setSelectedResearchCompanyId(item.companyId);
          }
          setSelectedResearchQuestionId(item.sourceId);
        }
        setActiveSection("Research");
        break;
      case "events":
        setCompanyEventCompanyFilter(item.companyId);
        setSelectedCompanyEventId(item.sourceId);
        setActiveSection("Events");
        break;
      case "transcripts":
        setActiveSection("Transcripts");
        break;
      case "ai_analysis":
        setInboxCompanyFilter(itemCompanyTicker);
        setActiveSection("Inbox");
        break;
    }
  }

  function selectAdjacentInboxItem(direction: 1 | -1) {
    if (activeSection !== "Inbox" || filteredFeedItems.length === 0) {
      return false;
    }

    const currentIndex = selectedFeedItem
      ? filteredFeedItems.findIndex((item) => item.id === selectedFeedItem.id)
      : -1;
    const nextIndex = currentIndex === -1
      ? 0
      : Math.min(Math.max(currentIndex + direction, 0), filteredFeedItems.length - 1);

    setSelectedFeedItemId(filteredFeedItems[nextIndex]?.id ?? null);
    return true;
  }

  function selectAdjacentCompany(direction: 1 | -1) {
    if (activeSection !== "Companies" || filteredCompanies.length === 0) {
      return false;
    }

    const currentIndex = selectedCompany
      ? filteredCompanies.findIndex((company) => company.id === selectedCompany.id)
      : -1;
    const nextIndex = currentIndex === -1
      ? 0
      : Math.min(Math.max(currentIndex + direction, 0), filteredCompanies.length - 1);
    const nextCompany = filteredCompanies[nextIndex];

    if (!nextCompany) {
      return false;
    }

    setSelectedCompanyId(nextCompany.id);
    return true;
  }

  function switchCompanyWorkspaceTab(direction: 1 | -1) {
    if (activeSection !== "Companies" || !selectedCompany) {
      return false;
    }

    const tabs: CompanyWorkspaceTab[] = ["Feed", "Notebook", "Claims", "Transcripts", "Fundamentals", "Metadata"];
    const currentIndex = tabs.indexOf(companyWorkspaceTab);
    const nextIndex = Math.min(Math.max(currentIndex + direction, 0), tabs.length - 1);

    setCompanyWorkspaceTab(tabs[nextIndex]);
    return true;
  }

  const shortcutActions = useMemo<AppShortcutActionMap>(() => ({
    "app.openInbox": () => undefined,
    "app.openCompanies": () => undefined,
    "app.openWatchlists": () => undefined,
    "app.openNotebooks": () => undefined,
    "app.openEvents": () => undefined,
    "app.openTranscripts": () => undefined,
    "app.openSources": () => undefined,
    "app.openSettings": () => undefined,
    "app.focusSearch": () => undefined,
    "app.refreshSources": () => undefined,
    "app.refreshDatabase": () => undefined,
    "inbox.nextItem": () => selectAdjacentInboxItem(1),
    "inbox.previousItem": () => selectAdjacentInboxItem(-1),
    "inbox.toggleRead": () => {
      if (activeSection !== "Inbox" || !selectedFeedItem) {
        return false;
      }

      updateSelectedFeedItem((item) => ({ ...item, unread: !item.unread }));
      return true;
    },
    "inbox.toggleSaved": () => {
      if (activeSection !== "Inbox" || !selectedFeedItem) {
        return false;
      }

      updateSelectedFeedItem((item) => ({ ...item, saved: !item.saved }));
      return true;
    },
    "inbox.openSource": () => {
      if (activeSection !== "Inbox" || !selectedFeedItem) {
        return false;
      }

      openExternalUrl(selectedFeedItem.sourceUrl);
      return true;
    },
    "inbox.createNote": () => {
      if (activeSection !== "Inbox" || !selectedFeedItem) {
        return false;
      }

      openFeedItemNoteDraft(selectedFeedItem);
      return true;
    },
    "company.nextCompany": () => selectAdjacentCompany(1),
    "company.previousCompany": () => selectAdjacentCompany(-1),
    "company.nextTab": () => switchCompanyWorkspaceTab(1),
    "company.previousTab": () => switchCompanyWorkspaceTab(-1),
    "notebook.editSelected": () => {
      if (activeSection === "Companies" && companyWorkspaceTab === "Notebook" && selectedNotebookEntry) {
        setNotebookEditMode(true);
        return true;
      }

      if (activeSection === "Notebooks" && selectedNotebookScreenEntry) {
        setNotebookScreenEditMode(true);
        return true;
      }

      return false;
    },
    "notebook.saveCurrent": () => {
      if (activeSection === "Companies" && companyWorkspaceTab === "Notebook") {
        if (isNotebookComposerOpen && selectedCompany) {
          createNotebookEntry();
          return true;
        }

        if (isNotebookEditMode && selectedNotebookEntry) {
          saveNotebookEntry();
          return true;
        }
      }

      if (activeSection === "Notebooks") {
        if (isNotebookScreenComposerOpen && selectedNotebookScreenCompany) {
          createNotebookScreenEntry();
          return true;
        }

        if (isNotebookScreenEditMode && selectedNotebookScreenEntry) {
          saveNotebookScreenEntry();
          return true;
        }
      }

      return false;
    },
  }), [
    activeSection,
    companyWorkspaceTab,
    createNotebookEntry,
    createNotebookScreenEntry,
    filteredCompanies,
    filteredFeedItems,
    isNotebookComposerOpen,
    isNotebookEditMode,
    isNotebookScreenComposerOpen,
    isNotebookScreenEditMode,
    openFeedItemNoteDraft,
    saveNotebookEntry,
    saveNotebookScreenEntry,
    selectedCompany,
    selectedFeedItem,
    selectedNotebookEntry,
    selectedNotebookScreenCompany,
    selectedNotebookScreenEntry,
    updateSelectedFeedItem,
  ]);

  return (
    <LocaleContext.Provider value={{ locale, t: makeTranslator(locale), text }}>
      <AppShell
        activeSection={activeSection}
        dbRefreshState={dbRefreshState}
        effectiveTheme={effectiveTheme}
        health={health}
        openSourceStatus={openSourceStatus}
        refreshDatabaseBackedViews={refreshDatabaseBackedViews}
        refreshSources={refreshSources}
        setActiveSection={setActiveSection}
        sourceRefreshError={sourceRefreshError}
        sourceRefreshResult={sourceRefreshResult}
        sourceRefreshState={sourceRefreshState}
        sourceStatusSummary={sourceStatusSummary}
        theme={theme}
        locale={locale}
        shortcutBindings={shortcutBindings}
        shortcutActions={shortcutActions}
        totalUnreadFeedItems={totalUnreadFeedItems}
        updateTheme={updateTheme}
        developerMode={Boolean(settings?.developerMode)}
      >
        <section
          className={activeSection === "Inbox" ? "content-grid" : "content-grid content-grid-single"}
          ref={contentGridRef}
          style={
            activeSection === "Inbox"
              ? ({ "--detail-pane-width": `${detailPaneWidth}px` } as CSSProperties)
              : undefined
          }
        >
          {activeSection === "Inbox" ? (
            <InboxScreen
              watchlists={watchlists}
              companies={companies}
              feedTypes={feedTypes}
              feedSources={feedSources}
              filteredFeedItems={filteredFeedItems}
              selectedFeedItem={selectedFeedItem}
              selectedFeedCompany={selectedFeedCompany}
              aiAnalysisJobsByFeedItemId={aiAnalysisJobsByFeedItemId}
              aiAnalysisErrorByFeedItemId={aiAnalysisErrorByFeedItemId}
              aiAnalysisRequestInFlightByFeedItemId={aiAnalysisRequestInFlightByFeedItemId}
              aiAnalysisProviderConfigured={Boolean(settings?.aiProviders.generalAnalysisProvider)}
              inboxStatusFilter={inboxStatusFilter}
              searchQuery={searchQuery}
              inboxWatchlistFilter={inboxWatchlistFilter}
              inboxCompanyFilter={inboxCompanyFilter}
              inboxTypeFilter={inboxTypeFilter}
              inboxSourceFilter={inboxSourceFilter}
              inboxReviewStats={inboxReviewStats}
              inboxEmptyState={inboxEmptyState}
              hasActiveInboxFilters={hasActiveInboxFilters}
              deleteUnsavedFeedState={deleteUnsavedFeedState}
              sourceRefreshState={sourceRefreshState}
              detailPaneWidth={detailPaneWidth}
              detailPaneMinWidth={detailPaneMinWidth}
              detailPaneMaxWidth={detailPaneMaxWidth}
              feedError={feedError}
              deleteUnsavedFeedError={deleteUnsavedFeedError}
              sourceRefreshError={sourceRefreshError}
              healthError={healthError}
              databaseError={databaseError}
              setInboxStatusFilter={setInboxStatusFilter}
              setSearchQuery={setSearchQuery}
              setInboxWatchlistFilter={setInboxWatchlistFilter}
              setInboxCompanyFilter={setInboxCompanyFilter}
              setInboxTypeFilter={setInboxTypeFilter}
              setInboxSourceFilter={setInboxSourceFilter}
              setSelectedFeedItemId={setSelectedFeedItemId}
              setActiveSection={setActiveSection}
              markVisibleInboxAsRead={markVisibleInboxAsRead}
              deleteUnsavedFeedItems={deleteUnsavedFeedItems}
              clearInboxFilters={clearInboxFilters}
              refreshSources={refreshSources}
              openSourceStatus={openSourceStatus}
              toggleFeedItemReadState={toggleFeedItemReadState}
              selectFeedItemFromKeyboard={selectFeedItemFromKeyboard}
              updateSelectedFeedItem={updateSelectedFeedItem}
              openCompanyWorkspaceFromFeedItem={openCompanyWorkspaceFromFeedItem}
              openFeedItemNoteDraft={openFeedItemNoteDraft}
              startFeedItemAiAnalysis={startFeedItemAiAnalysis}
              retryFeedItemAiAnalysis={retryFeedItemAiAnalysis}
              resizeDetailPaneWithKeyboard={resizeDetailPaneWithKeyboard}
              startDetailPaneResize={startDetailPaneResize}
              resizeDetailPane={resizeDetailPane}
              stopDetailPaneResize={stopDetailPaneResize}
              feedItemSummary={feedItemSummary}
              formatTimestamp={formatTimestamp}
            />
          ) : null}
          {activeSection === "Companies" ? (
            <CompaniesScreen
              watchlists={watchlists}
              companyFieldRefs={companyFieldRefs}
              companyForm={companyForm}
              companyFormRegistryMatches={companyFormRegistryMatches}
              companyListSearch={companyListSearch}
              companyWatchlistFilter={companyWatchlistFilter}
              filteredCompanies={filteredCompanies}
              companies={companies}
              selectedCompany={selectedCompany}
              workspaceAutoFocusId={workspaceAutoFocusId}
              clearWorkspaceAutoFocus={() => setWorkspaceAutoFocusId(null)}
              membershipsByCompany={membershipsByCompany}
              selectedCompanyFeedStats={selectedCompanyFeedStats}
              companyWorkspaceTab={companyWorkspaceTab}
              selectedCompanyFeedItems={selectedCompanyFeedItems}
              selectedCompanyFeedItem={selectedCompanyFeedItem}
              aiAnalysisJobsByFeedItemId={aiAnalysisJobsByFeedItemId}
              aiAnalysisErrorByFeedItemId={aiAnalysisErrorByFeedItemId}
              aiAnalysisRequestInFlightByFeedItemId={aiAnalysisRequestInFlightByFeedItemId}
              aiAnalysisProviderConfigured={Boolean(settings?.aiProviders.generalAnalysisProvider)}
              selectedCompanyNotebookEntries={selectedCompanyNotebookEntries}
              isNotebookComposerOpen={isNotebookComposerOpen}
              notebookForm={notebookForm}
              selectedNotebookEntryId={selectedNotebookEntryId}
              selectedNotebookEntry={selectedNotebookEntry}
              notebookEditMode={isNotebookEditMode}
              notebookEditForm={notebookEditForm}
              isNotebookEditDirty={isNotebookEditDirty}
              notebookError={notebookError}
              selectedCompanyClaimEntries={selectedCompanyClaimEntries}
              selectedClaimEntry={selectedClaimEntry}
              claimStatusDraft={claimStatusDraft}
              companiesError={companiesError}
              lookupStatus={lookupStatus}
              createCompany={createCompany}
              updateCompanyForm={updateCompanyForm}
              clearCompanyFormField={clearCompanyFormField}
              lookupCompanyIfUseful={lookupCompanyIfUseful}
              lookupCompany={lookupCompany}
              applyRegistryEntryToCompanyForm={applyRegistryEntryToCompanyForm}
              setCompanyListSearch={setCompanyListSearch}
              setCompanyWatchlistFilter={setCompanyWatchlistFilter}
              openWatchlistFromCompanyRow={openWatchlistFromCompanyRow}
              openCompanyWorkspace={openCompanyWorkspace}
              openCompanyWorkspaceFromKeyboard={openCompanyWorkspaceFromKeyboard}
              deleteCompany={deleteCompany}
              setCompanyWorkspaceTab={setCompanyWorkspaceTab}
              toggleCompanyFeedItem={toggleCompanyFeedItem}
              selectCompanyFeedItemFromKeyboard={selectCompanyFeedItemFromKeyboard}
              updateFeedItemState={updateFeedItemState}
              inspectCompanyFeedItem={inspectCompanyFeedItem}
              openFeedItemNoteDraft={openFeedItemNoteDraft}
              startFeedItemAiAnalysis={startFeedItemAiAnalysis}
              retryFeedItemAiAnalysis={retryFeedItemAiAnalysis}
              openCompanyInboxFilter={openCompanyInboxFilter}
              setNotebookComposerOpen={setNotebookComposerOpen}
              updateNotebookForm={updateNotebookForm}
              createNotebookEntry={createNotebookEntry}
              setSelectedNotebookEntryId={setSelectedNotebookEntryId}
              saveNotebookEntry={saveNotebookEntry}
              cancelNotebookEdit={cancelNotebookEdit}
              setNotebookEditMode={setNotebookEditMode}
              updateNotebookEditForm={updateNotebookEditForm}
              toggleClaimEntry={toggleClaimEntry}
              setClaimStatusDraft={setClaimStatusDraft}
              saveClaimStatus={saveClaimStatus}
              NotebookDateField={NotebookDateField}
              NotebookQuarterField={NotebookQuarterField}
              MarkdownNoteBody={MarkdownNoteBody}
              renderNotebookOrigins={renderNotebookOrigins}
              formatTimestamp={formatTimestamp}
              feedItemSummary={feedItemSummary}
              financialPeriods={financialPeriods}
              financialFacts={financialFacts}
              kpiDefinitions={kpiDefinitions}
              fundamentalsForm={fundamentalsForm}
              financialFactForm={financialFactForm}
              selectedFinancialFactId={selectedFinancialFactId}
              isFinancialFactEditMode={isFinancialFactEditMode}
              fundamentalsError={fundamentalsError}
              createFinancialPeriod={createFinancialPeriod}
              saveFinancialFact={saveFinancialFact}
              deleteFinancialFact={deleteFinancialFact}
              selectFinancialFact={selectFinancialFact}
              startEditingFinancialFact={startEditingFinancialFact}
              cancelEditingFinancialFact={cancelEditingFinancialFact}
              updateFundamentalsForm={updateFundamentalsForm}
              updateFinancialFactForm={updateFinancialFactForm}
            />
          ) : null}
          {activeSection === "Watchlists" ? (
            <WatchlistsScreen
              companies={companies}
              watchlists={watchlists}
              watchlistMemberships={watchlistMemberships}
              watchlistsError={watchlistsError}
              selectedWatchlistId={selectedManagedWatchlistId}
              setSelectedWatchlistId={setSelectedManagedWatchlistId}
              createWatchlist={createWatchlist}
              renameWatchlist={renameWatchlist}
              deleteWatchlist={deleteWatchlist}
              addCompanyToWatchlist={addCompanyToWatchlist}
              removeCompanyFromWatchlist={removeCompanyFromWatchlist}
            />
          ) : null}
          {activeSection === "Research" ? (
            <ResearchScreen
              companies={companies}
              watchlists={watchlists}
              watchlistMemberships={watchlistMemberships}
              mode={researchMode}
              selectedCompanyId={selectedResearchCompanyId}
              selectedWatchlistId={selectedResearchWatchlistId}
              selectedWatchlistCompanyId={selectedResearchWatchlistCompanyId}
              cascadeToCompanies={researchCascadeToCompanies}
              selectedEvidenceTypes={researchEvidenceTypes}
              changedOnly={researchChangedOnly}
              timeline={researchTimeline}
              questions={researchQuestions}
              selectedQuestionId={selectedResearchQuestionId}
              questionTitle={researchQuestionTitle}
              questionBody={researchQuestionBody}
              questionLinks={researchQuestionLinks}
              briefJobs={researchBriefJobs}
              digestJobs={researchDigestJobs}
              reminders={researchReminders}
              error={researchError}
              loading={researchLoading}
              reviewInFlight={researchReviewInFlight}
              questionInFlight={researchQuestionInFlight}
              briefInFlight={researchBriefInFlight}
              digestInFlight={researchDigestInFlight}
              reminderInFlight={researchReminderInFlight}
              setMode={setResearchMode}
              setSelectedCompanyId={setSelectedResearchCompanyId}
              setSelectedWatchlistId={setSelectedResearchWatchlistId}
              setSelectedWatchlistCompanyId={setSelectedResearchWatchlistCompanyId}
              setSelectedQuestionId={setSelectedResearchQuestionId}
              setQuestionTitle={setResearchQuestionTitle}
              setQuestionBody={setResearchQuestionBody}
              setCascadeToCompanies={setResearchCascadeToCompanies}
              setChangedOnly={setResearchChangedOnly}
              toggleEvidenceType={toggleResearchEvidenceType}
              clearEvidenceTypes={clearResearchEvidenceTypes}
              refreshTimeline={() => {
                void refreshResearchTimeline();
              }}
              markReviewed={() => {
                void markResearchReviewed();
              }}
              createQuestion={() => {
                void createResearchQuestion();
              }}
              updateQuestionStatus={(questionId, status) => {
                void updateResearchQuestionStatus(questionId, status);
              }}
              deleteQuestion={(questionId) => {
                void deleteResearchQuestion(questionId);
              }}
              linkEvidence={(item) => {
                void linkEvidenceToSelectedQuestion(item);
              }}
              unlinkEvidence={(linkId) => {
                void unlinkEvidenceFromSelectedQuestion(linkId);
              }}
              startBrief={() => {
                void startResearchBrief();
              }}
              startDigest={() => {
                void startResearchDigest();
              }}
              createReminder={(title, body, dueAt) => {
                void createResearchReminder(title, body, dueAt);
              }}
              completeReminder={(reminderId) => {
                void completeResearchReminder(reminderId);
              }}
              snoozeReminder={(reminderId) => {
                void snoozeResearchReminder(reminderId);
              }}
              reopenReminder={(reminderId) => {
                void reopenResearchReminder(reminderId);
              }}
              deleteReminder={(reminderId) => {
                void deleteResearchReminder(reminderId);
              }}
              openEvidence={openResearchEvidence}
              openEvidenceUrl={openExternalUrl}
              formatTimestamp={formatTimestamp}
            />
          ) : null}
          {activeSection === "Notebooks" ? (
            <NotebooksScreen
              companies={filteredNotebookScreenCompanies}
              totalCompanyCount={companies.length}
              watchlists={watchlists}
              notebookEntries={notebookEntries}
              selectedNotebookScreenCompany={selectedNotebookScreenCompany}
              selectedNotebookScreenEntries={selectedNotebookScreenEntries}
              selectedNotebookScreenEntry={selectedNotebookScreenEntry}
              isNotebookScreenComposerOpen={isNotebookScreenComposerOpen}
              isNotebookScreenEditMode={isNotebookScreenEditMode}
              isNotebookScreenEditDirty={isNotebookScreenEditDirty}
              notebookScreenKindFilter={notebookScreenKindFilter}
              notebookScreenWatchlistFilter={notebookScreenWatchlistFilter}
              notebookScreenClaimStatusFilter={notebookScreenClaimStatusFilter}
              notebookScreenFollowUpFilter={notebookScreenFollowUpFilter}
              notebookScreenTagFilter={notebookScreenTagFilter}
              notebookScreenForm={notebookScreenForm}
              notebookScreenEditForm={notebookScreenEditForm}
              notebookError={notebookError}
              selectNotebookScreenCompany={selectNotebookScreenCompany}
              showNotebookCompanyOpenClaims={showNotebookCompanyOpenClaims}
              showNotebookCompanyFollowUps={showNotebookCompanyFollowUps}
              focusCompanyWorkspace={focusCompanyWorkspace}
              toggleNotebookScreenComposer={toggleNotebookScreenComposer}
              discardNotebookScreenDraft={discardNotebookScreenDraft}
              createNotebookScreenEntry={createNotebookScreenEntry}
              toggleNotebookScreenEntry={toggleNotebookScreenEntry}
              saveNotebookScreenEntry={saveNotebookScreenEntry}
              deleteNotebookScreenEntry={deleteNotebookScreenEntry}
              cancelNotebookScreenEdit={cancelNotebookScreenEdit}
              setNotebookScreenEditMode={setNotebookScreenEditMode}
              setNotebookScreenKindFilter={setNotebookScreenKindFilter}
              setNotebookScreenWatchlistFilter={setNotebookScreenWatchlistFilter}
              setNotebookScreenClaimStatusFilter={setNotebookScreenClaimStatusFilter}
              setNotebookScreenFollowUpFilter={setNotebookScreenFollowUpFilter}
              setNotebookScreenTagFilter={setNotebookScreenTagFilter}
              updateNotebookScreenForm={updateNotebookScreenForm}
              updateNotebookScreenEditForm={updateNotebookScreenEditForm}
              NotebookDateField={NotebookDateField}
              NotebookQuarterField={NotebookQuarterField}
              MarkdownNoteBody={MarkdownNoteBody}
              renderNotebookOrigins={renderNotebookOrigins}
            />
          ) : null}
          {activeSection === "Events" ? (
            <EventsScreen
              companies={companies}
              watchlists={watchlists}
              companyEvents={companyEvents}
              companyEventsError={companyEventsError}
              selectedCompanyEventId={selectedCompanyEventId}
              sourceRefreshState={sourceRefreshState}
              selectedSourceAdapterId={selectedSourceAdapterId}
              sourceAdapterRefreshInFlight={sourceAdapterRefreshInFlight}
              companyEventViewMode={companyEventViewMode}
              companyEventMode={companyEventMode}
              companyEventWeekRange={companyEventWeekRange}
              companyEventWorkingWeekDays={companyEventWorkingWeekDays}
              companyEventWeekendDays={companyEventWeekendDays}
              companyEventWeekendEvents={companyEventWeekendEvents}
              companyEventsByDate={companyEventsByDate}
              companyEventWatchlistFilter={companyEventWatchlistFilter}
              companyEventCompanyFilter={companyEventCompanyFilter}
              companyEventTypeFilter={companyEventTypeFilter}
              companyEventStatusFilter={companyEventStatusFilter}
              companyEventDateFrom={companyEventDateFrom}
              companyEventDateTo={companyEventDateTo}
              companyEventTypes={companyEventTypes}
              companyEventStatuses={companyEventStatuses}
              isCompanyEventComposerOpen={isCompanyEventComposerOpen}
              companyEventForm={companyEventForm}
              companyEventCreateError={companyEventCreateError}
              companyEventTypeOptions={companyEventTypeOptions}
              companyEventStatusOptions={companyEventStatusOptions}
              refreshEventSources={refreshEventSources}
              openCompanyEventComposer={openCompanyEventComposer}
              setCompanyEventViewMode={setCompanyEventViewMode}
              setCompanyEventMode={setCompanyEventMode}
              setCompanyEventWeekAnchorDate={setCompanyEventWeekAnchorDate}
              setCompanyEventWatchlistFilter={setCompanyEventWatchlistFilter}
              setCompanyEventCompanyFilter={setCompanyEventCompanyFilter}
              setCompanyEventTypeFilter={setCompanyEventTypeFilter}
              setCompanyEventStatusFilter={setCompanyEventStatusFilter}
              setCompanyEventDateFrom={setCompanyEventDateFrom}
              setCompanyEventDateTo={setCompanyEventDateTo}
              setCompanyEventComposerOpen={setCompanyEventComposerOpen}
              setCompanyEventCreateError={setCompanyEventCreateError}
              setCompanyEventForm={setCompanyEventForm}
              setSelectedCompanyEventId={setSelectedCompanyEventId}
              clearCompanyEventFilters={clearCompanyEventFilters}
              createCompanyEvent={createCompanyEvent}
              NotebookDateField={NotebookDateField}
              formatLocalDate={formatLocalDate}
              parseLocalDate={parseLocalDate}
              addLocalDays={addLocalDays}
              formatWeekRange={formatWeekRange}
              formatTimestamp={formatTimestamp}
              formatCompanyEventType={formatCompanyEventType}
              formatCompanyEventStatus={formatCompanyEventStatus}
              formatCompanyEventSourceType={formatCompanyEventSourceType}
              companyEventDueLabel={companyEventDueLabel}
              companyEventDueClass={companyEventDueClass}
              openExternalUrl={openExternalUrl}
            />
          ) : null}
          {activeSection === "Transcripts" ? (
            <TranscriptsScreen
              companies={companies}
              settings={settings}
              geminiCredentialStatus={geminiCredentialStatus}
              transcriptJobs={transcriptJobs}
              transcriptJobsError={transcriptJobsError}
              transcriptJobForm={transcriptJobForm}
              transcriptJobCreateError={transcriptJobCreateError}
              transcriptJobCreateState={transcriptJobCreateState}
              transcriptJobRunInFlight={transcriptJobRunInFlight}
              selectedTranscriptJobId={selectedTranscriptJobId}
              transcriptSegmentsByJobId={transcriptSegmentsByJobId}
              transcriptSegmentsErrorByJobId={transcriptSegmentsErrorByJobId}
              transcriptSegmentSearchByJobId={transcriptSegmentSearchByJobId}
              selectedTranscriptSegmentIdsByJobId={selectedTranscriptSegmentIdsByJobId}
              transcriptNoteDraftJobId={transcriptNoteDraftJobId}
              transcriptNoteForm={transcriptNoteForm}
              transcriptNoteErrorByJobId={transcriptNoteErrorByJobId}
              transcriptNoteSaveInFlight={transcriptNoteSaveInFlight}
              transcriptLinkQueryByJobId={transcriptLinkQueryByJobId}
              transcriptLinkErrorByJobId={transcriptLinkErrorByJobId}
              transcriptLinkInFlight={transcriptLinkInFlight}
              transcriptDeleteInFlight={transcriptDeleteInFlight}
              transcriptDescriptionDraftByJobId={transcriptDescriptionDraftByJobId}
              transcriptDescriptionErrorByJobId={transcriptDescriptionErrorByJobId}
              transcriptDescriptionSaveInFlight={transcriptDescriptionSaveInFlight}
              transcriptCompanySuggestions={transcriptCompanySuggestions}
              NotebookDateField={NotebookDateField}
              NotebookQuarterField={NotebookQuarterField}
              setTranscriptJobForm={setTranscriptJobForm}
              setTranscriptJobCreateError={setTranscriptJobCreateError}
              setTranscriptSegmentSearchByJobId={setTranscriptSegmentSearchByJobId}
              setTranscriptDescriptionDraftByJobId={setTranscriptDescriptionDraftByJobId}
              refreshTranscriptJobs={refreshTranscriptJobs}
              createTranscriptJob={createTranscriptJob}
              toggleTranscriptJob={toggleTranscriptJob}
              toggleTranscriptJobFromKeyboard={toggleTranscriptJobFromKeyboard}
              runTranscriptJob={runTranscriptJob}
              deleteTranscriptJob={deleteTranscriptJob}
              updateTranscriptJobDescription={updateTranscriptJobDescription}
              updateTranscriptLinkQuery={updateTranscriptLinkQuery}
              linkTranscriptJobCompany={linkTranscriptJobCompany}
              toggleTranscriptSegment={toggleTranscriptSegment}
              openTranscriptNoteDraft={openTranscriptNoteDraft}
              createTranscriptNotebookEntry={createTranscriptNotebookEntry}
              discardTranscriptNoteDraft={discardTranscriptNoteDraft}
              updateTranscriptNoteForm={updateTranscriptNoteForm}
              selectTranscriptCompany={selectTranscriptCompany}
              formatAiProvider={formatAiProvider}
              formatGeminiModel={formatGeminiModel}
              formatCredentialConfigured={formatCredentialConfigured}
              formatEnumLabel={formatEnumLabel}
            />
          ) : null}
          {activeSection === "Sources" ? (
            <SourcesScreen
              sourceAdapters={sourceAdapters}
              sourceAdaptersError={sourceAdaptersError}
              developerMode={Boolean(settings?.developerMode)}
              selectedSourceAdapterId={selectedSourceAdapterId}
              sourceRefreshState={sourceRefreshState}
              sourceRefreshResult={sourceRefreshResult}
              sourceRefreshError={sourceRefreshError}
              sourceAdapterRefreshInFlight={sourceAdapterRefreshInFlight}
              registryRefreshState={registryRefreshState}
              registryRefreshResult={registryRefreshResult}
              registryRefreshError={registryRefreshError}
              companyRegistryEntries={companyRegistryEntries}
              filteredCompanyRegistryEntries={filteredCompanyRegistryEntries}
              companyRegistryEntriesError={companyRegistryEntriesError}
              isCompanyRegistryListExpanded={isCompanyRegistryListExpanded}
              companyRegistrySearch={companyRegistrySearch}
              addingRegistryTicker={addingRegistryTicker}
              unmatchedSourceItems={unmatchedSourceItems}
              unmatchedSourceItemsError={unmatchedSourceItemsError}
              expandedUnmatchedAdapters={expandedUnmatchedAdapters}
              refreshSources={refreshSources}
              refreshCompanyRegistry={refreshCompanyRegistry}
              setSourceEnabled={setSourceEnabled}
              toggleSourceAdapter={toggleSourceAdapter}
              toggleSourceAdapterFromKeyboard={toggleSourceAdapterFromKeyboard}
              toggleCompanyRegistryList={toggleCompanyRegistryList}
              toggleUnmatchedSourceItems={toggleUnmatchedSourceItems}
              setCompanyRegistrySearch={setCompanyRegistrySearch}
              addCompanyFromRegistry={addCompanyFromRegistry}
              openExternalUrl={openExternalUrl}
              formatSourceScheduler={formatSourceScheduler}
              formatNextRefresh={formatNextRefresh}
              formatTimestamp={formatTimestamp}
            />
          ) : null}
          {activeSection === "Diagnostics" && settings?.developerMode ? (
            <DiagnosticsScreen
              developerMode={settings.developerMode}
              onDisableDeveloperMode={disableDeveloperMode}
            />
          ) : null}
          {activeSection === "Settings" ? (
            <SettingsScreen
              theme={theme}
              accentPalette={accentPalette}
              locale={locale}
              settings={settings}
              settingsError={settingsError}
              licenseError={licenseError}
              licenseInFlight={licenseInFlight}
              licenseKeyDraft={licenseKeyDraft}
              licenseStatus={licenseStatus}
              feedPruneRetentionDays={feedPruneRetentionDays}
              feedPruneResult={feedPruneResult}
              geminiCredentialStatus={geminiCredentialStatus}
              geminiCredentialError={geminiCredentialError}
              geminiCredentialInFlight={geminiCredentialInFlight}
              geminiApiKeyDraft={geminiApiKeyDraft}
              shortcutBindings={shortcutBindings}
              shortcutReferences={shortcutReferences}
              onThemeChange={updateTheme}
              onAccentPaletteChange={updateAccentPalette}
              onLocaleChange={updateLocale}
              onPollIntervalChange={updatePollInterval}
              onShortcutBindingsChange={updateShortcutBindings}
              onYoutubeTranscriptionModelChange={updateYoutubeTranscriptionModel}
              onYoutubeTranscriptionTimeoutChange={updateYoutubeTranscriptionTimeout}
              onGeneralAnalysisProviderChange={updateGeneralAnalysisProvider}
              onGeneralAnalysisModelChange={updateGeneralAnalysisModel}
              onGeneralAnalysisTimeoutChange={updateGeneralAnalysisTimeout}
              onLogLevelChange={updateLogLevel}
              onLogMaxFilesChange={updateLogMaxFiles}
              onLogMaxFileBytesChange={updateLogMaxFileBytes}
              onClearLicenseKey={clearLicenseKey}
              onLicenseKeyDraftChange={setLicenseKeyDraft}
              onSubmitLicenseKey={submitLicenseKey}
              onGeminiApiKeyDraftChange={setGeminiApiKeyDraft}
              onSaveGeminiApiKey={saveGeminiApiKey}
              onClearGeminiApiKey={clearGeminiApiKey}
              onOpenGeminiApiKeyPage={() => {
                void openUrl("https://aistudio.google.com/app/apikey");
              }}
              onImportApplied={refreshDatabaseBackedViews}
              formatTimestamp={formatTimestamp}
              formatPollInterval={formatPollInterval}
              formatAiProvider={formatAiProvider}
              formatGeminiModel={formatGeminiModel}
              formatCredentialConfigured={formatCredentialConfigured}
              formatCredentialKind={formatCredentialKind}
            />
          ) : null}

        </section>
      </AppShell>
    </LocaleContext.Provider>
  );
}
