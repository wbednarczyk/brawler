import {
  type CSSProperties,
  useEffect,
  useMemo,
  useRef,
  useState,
} from "react";
import { openUrl } from "@tauri-apps/plugin-opener";
import { AppShell, type PinnedCompany } from "./AppShell";
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
import { detailPaneDefaultFraction, detailPaneMaxFraction, detailPaneMinFraction } from "./layout";
import { type Section } from "./navigation";
import {
  emptyNotebookForm,
  manualNotebookOrigins,
} from "./notebookForms";
import { feedPruneRetentionDays } from "./sourceScheduler";
import * as sourcesApi from "../api/sources";
import * as financialsApi from "../api/financials";
import type { FinancialFact, FinancialPeriod, KpiDefinition } from "../api/financialsTypes";
import * as eventsApi from "../api/events";
import * as signalsApi from "../api/signals";
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
import { CockpitScreen } from "../screens/Cockpit/CockpitScreen";
import { TodayScreen } from "../screens/Today/TodayScreen";
import { CompareScreen } from "../screens/Compare/CompareScreen";
import type { CompanyWorkspaceTab } from "../screens/Companies/companyTypes";
import { ErrorBoundary } from "./ErrorBoundary";
import { AppContentErrorFallback } from "./AppErrorFallback";
import { DiagnosticsScreen } from "../screens/Diagnostics/DiagnosticsScreen";
import { EventsScreen } from "../screens/Events/EventsScreen";
import type { EventsScreenProps } from "../screens/Events/eventTypes";
import { InboxScreen } from "../screens/Inbox/InboxScreen";
import { ReportSeasonScreen } from "../screens/ReportSeason/ReportSeasonScreen";
import type { InboxStatusFilter } from "../screens/Inbox/inboxTypes";
import { NotebooksScreen } from "../screens/Notebooks/NotebooksScreen";
import type { NotebooksScreenProps } from "../screens/Notebooks/notebookTypes";
import { ResearchScreen, type ResearchScreenProps } from "../screens/Research/ResearchScreen";
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
import { SettingsProvider } from "./state/SettingsContext";
import { SourcesProvider } from "./state/SourcesContext";
import {
  CompaniesProvider,
  EventsProvider,
  InboxProvider,
  NotebooksProvider,
  ReportSeasonProvider,
  ResearchProvider,
  SettingsScreenProvider,
  TranscriptsProvider,
  WatchlistsProvider,
} from "./state/screenViewModels";
import type { CompanyEventForm, CompanyEventMode, CompanyEventViewMode } from "../shared/types/events";
import type { NotebookForm } from "../shared/types/notebook";
import type {
  Company,
  CompanyForm,
  CompanyEvent,
  CompanyRegistryEntry,
  CompanyRegistryRefreshResult,
  CompanySignal,
  CredentialStatus,
  DatabaseStatus,
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
import { useAiAnalysisController } from "./useAiAnalysisController";
import type { ResearchEvidenceItem } from "../api/researchTypes";
import type { SearchMatch } from "../api/search";

type AppStateRootProps = {
  initialLicenseStatus?: LicenseStatus | null;
  initialSection?: Section;
};

export function AppStateRoot({
  initialLicenseStatus = null,
  initialSection = "Today",
}: AppStateRootProps) {
  const contentGridRef = useRef<HTMLElement | null>(null);
  const sourceRefreshInFlightRef = useRef(false);
  const sourceAdaptersRef = useRef<SourceAdapter[]>([]);
  const eventWeekFetchAttemptedRef = useRef<Set<string>>(new Set());
  const companyLookupVersionRef = useRef(0);
  const skipNextCompanyLookupRef = useRef(false);
  const companyFieldRefs = useRef<Record<keyof CompanyForm, HTMLInputElement | null>>({
    exchange: null,
    ticker: null,
    displayName: null,
    isin: null,
  });
  // Today/Pulse is the default shell home (ADR 0054); `initialSection` overrides
  // it for deep links and screen-focused tests.
  const [activeSection, setActiveSection] = useState<Section>(initialSection);
  // The company a workspace "Advanced layout" toggle scopes the dockview cockpit
  // to (ADR 0054); null = the cockpit opened directly, unscoped.
  const [cockpitInitialCompanyId, setCockpitInitialCompanyId] = useState<string | null>(null);
  const [theme, setTheme] = useState<Theme>("dark");
  const [accentPalette, setAccentPalette] = useState<UserSettings["accentPalette"]>("night-neon");
  const [locale, setLocale] = useState<UserSettings["locale"]>("en");
  const [health, setHealth] = useState<HealthResponse | null>(null);
  const [healthError, setHealthError] = useState<string | null>(null);
  const [, setDatabaseStatus] = useState<DatabaseStatus | null>(null);
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
  const [inboxSignalFilter, setInboxSignalFilter] = useState("all");
  const [inboxSourceFilter, setInboxSourceFilter] = useState("all");
  const [inboxStatusFilter, setInboxStatusFilter] = useState<InboxStatusFilter>("all");
  const [feedState, setFeedState] = useState<FeedItem[]>([]);
  const [feedError, setFeedError] = useState<string | null>(null);
  const [signals, setSignals] = useState<CompanySignal[]>([]);
  const [signalsError, setSignalsError] = useState<string | null>(null);
  const [aiSignalClassificationState, setAiSignalClassificationState] = useState<string>("idle");
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
  const [searchFocusSelector, setSearchFocusSelector] = useState<string | null>(null);
  const [selectedCompanyFeedItemId, setSelectedCompanyFeedItemId] = useState<string | null>(null);
  const [selectedNotebookEntryId, setSelectedNotebookEntryId] = useState<string | null>(null);
  const [isNotebookComposerOpen, setNotebookComposerOpen] = useState(false);
  const [isNotebookEditMode, setNotebookEditMode] = useState(false);
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
  const [detailPaneFraction, setDetailPaneFraction] = useState(detailPaneDefaultFraction);
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
  const [financialPeriods, setFinancialPeriods] = useState<FinancialPeriod[]>([]);
  const [financialFacts, setFinancialFacts] = useState<FinancialFact[]>([]);
  const [kpiDefinitions, setKpiDefinitions] = useState<KpiDefinition[]>([]);
  const [financialPeriodsError, setFinancialPeriodsError] = useState<string | null>(null);
  const [financialFactsError, setFinancialFactsError] = useState<string | null>(null);
  const [kpiDefinitionsError, setKpiDefinitionsError] = useState<string | null>(null);
  // Surfaced together as a single fundamentals data load-error line in FundamentalsPanel.
  const fundamentalsLoadError = financialPeriodsError ?? financialFactsError ?? kpiDefinitionsError;
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
    feedSignalCategories,
    feedSources,
    feedTypes,
    filteredCompanies,
    filteredCompanyRegistryEntries,
    filteredFeedItems,
    signalsByFeedItemId,
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
    selectedCompany,
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
    inboxSignalFilter,
    inboxSourceFilter,
    inboxStatusFilter,
    inboxTypeFilter,
    inboxWatchlistFilter,
    signals,
    notebookEditForm,
    notebookEntries,
    notebookScreenWatchlistFilter,
    notebookScreenClaimStatusFilter,
    notebookScreenEditForm,
    notebookScreenFollowUpFilter,
    notebookScreenKindFilter,
    notebookScreenTagFilter,
    searchQuery,
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

  const {
    aiAnalysisJobsByFeedItemId,
    aiAnalysisErrorByFeedItemId,
    aiAnalysisRequestInFlightByFeedItemId,
    startFeedItemAiAnalysis,
    retryFeedItemAiAnalysis,
  } = useAiAnalysisController({ selectedFeedItem, selectedCompanyFeedItem });

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
    refreshSignals,
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
    setSignals,
    setSignalsError,
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
    refreshSources,
  } = useSourceRefreshController({
    refreshCompanyEvents,
    refreshCompanyRegistryEntries,
    refreshDatabaseStatus,
    refreshFeedItems,
    refreshSignals,
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
    updateEspiAiFallbackEnabled,
    updatePollInterval,
    updateLogLevel,
    updateLogMaxFileBytes,
    updateLogMaxFiles,
    updateDbMaxConnections,
    updateDbBusyTimeoutMs,
    updateDbAcquireTimeoutMs,
    resetDatabaseSettings,
    updatePinnedCompanyIds,
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
    saveNotebookEntry,
    saveNotebookScreenEntry,
    selectNotebookScreenCompany,
    showNotebookCompanyFollowUps,
    showNotebookCompanyOpenClaims,
    toggleNotebookScreenComposer,
    toggleNotebookScreenEntry,
    updateNotebookEditForm,
    updateNotebookForm,
    updateNotebookScreenEditForm,
    updateNotebookScreenForm,
  } = useNotebookController({
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
    setInboxSignalFilter,
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

  async function confirmCompanySignal(signalId: string) {
    try {
      await signalsApi.confirmCompanySignal(signalId);
      await refreshSignals();
      setSignalsError(null);
    } catch (error) {
      setSignalsError(String(error));
    }
  }

  async function rejectCompanySignal(signalId: string) {
    try {
      await signalsApi.rejectCompanySignal(signalId);
      await refreshSignals();
      setSignalsError(null);
    } catch (error) {
      setSignalsError(String(error));
    }
  }

  async function confirmDerivedEvent(eventId: string, action: "confirm" | "reject") {
    try {
      await signalsApi.confirmDerivedEvent(eventId, action);
      await refreshCompanyEvents();
      setCompanyEventsError(null);
    } catch (error) {
      setCompanyEventsError(String(error));
    }
  }

  async function runAiSignalClassification() {
    setAiSignalClassificationState("running");
    setSignalsError(null);
    try {
      await signalsApi.runAiSignalClassification();
      await refreshSignals();
      setAiSignalClassificationState("done");
      window.setTimeout(() => {
        setAiSignalClassificationState("idle");
      }, 900);
    } catch (error) {
      setSignalsError(String(error));
      setAiSignalClassificationState("idle");
    }
  }

  // After a global-search navigation, scroll the opened item into view and focus
  // it once it has rendered (rows may appear a frame or two after the section and
  // selection update, including after an async list load).
  useEffect(() => {
    if (!searchFocusSelector) {
      return undefined;
    }

    let frame = 0;
    let attempts = 0;
    const tryFocus = () => {
      const element = document.querySelector<HTMLElement>(searchFocusSelector);
      if (element) {
        element.scrollIntoView?.({ block: "nearest" });
        element.focus?.({ preventScroll: true });
        setSearchFocusSelector(null);
        return;
      }
      attempts += 1;
      if (attempts < 30) {
        frame = requestAnimationFrame(tryFocus);
      } else {
        setSearchFocusSelector(null);
      }
    };
    frame = requestAnimationFrame(tryFocus);
    return () => cancelAnimationFrame(frame);
  }, [searchFocusSelector]);

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
    setDetailPaneFraction,
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
    refreshSignals,
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
    selectedCompanyId,
    selectedFeedItemId,
    selectedNotebookCompanyId,
    selectedNotebookEntry,
    selectedNotebookScreenEntry,
    settings,
    licenseCanUseApp,
    setNextRegistryRefreshAt,
    setNextSourceRefreshAtByAdapterId,
    setNotebookEditForm,
    setNotebookEditMode,
    setNotebookScreenEditForm,
    setNotebookScreenEditMode,
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
    // eslint-disable-next-line react-hooks/exhaustive-deps -- refresh fundamentals when the Companies/Fundamentals tab is opened; the non-memoized refresh callbacks are intentionally excluded to avoid re-fetching every render
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

  function navigateToSearchResult(match: SearchMatch) {
    switch (match.contentType) {
      case "company":
        focusCompanyWorkspace(match.sourceId);
        // Reuse the workspace's tuned scroll/focus-on-open behavior.
        setWorkspaceAutoFocusId(match.sourceId);
        break;
      case "watchlist":
        setSelectedManagedWatchlistId(match.sourceId);
        setActiveSection("Watchlists");
        setSearchFocusSelector(`[data-watchlist-id="${match.sourceId}"]`);
        break;
      case "feed_item":
        // Clear filters so the selected item is not hidden by an active filter.
        setInboxCompanyFilter("all");
        setInboxStatusFilter("all");
        setInboxTypeFilter("all");
        setInboxSourceFilter("all");
        setInboxWatchlistFilter("all");
        setSearchQuery("");
        setSelectedFeedItemId(match.sourceId);
        setActiveSection("Inbox");
        setSearchFocusSelector(`[data-feed-item-id="${match.sourceId}"]`);
        break;
      case "notebook_entry":
        if (match.companyId) {
          setSelectedNotebookCompanyId(match.companyId);
        }
        setSelectedNotebookScreenEntryId(match.sourceId);
        setActiveSection("Notebooks");
        setSearchFocusSelector(`[data-notebook-entry-id="${match.sourceId}"]`);
        break;
      case "research_brief":
      case "digest":
        if (match.companyId) {
          if (researchMode !== "company") {
            setResearchMode("company");
          }
          setSelectedResearchCompanyId(match.companyId);
        }
        setActiveSection("Research");
        break;
      case "event":
        if (match.companyId) {
          setCompanyEventCompanyFilter(match.companyId);
        }
        setSelectedCompanyEventId(match.sourceId);
        setActiveSection("Events");
        setSearchFocusSelector(`[data-event-id="${match.sourceId}"]`);
        // The week view is anchored on a date the search result does not carry,
        // so look up the event and move the week view to its week.
        void eventsApi
          .listCompanyEvents({
            mode: "all",
            companyId: match.companyId,
            watchlistId: null,
            eventType: null,
            status: null,
            dateFrom: null,
            dateTo: null,
          })
          .then((events) => {
            const target = events.find((event) => event.id === match.sourceId);
            if (target) {
              setCompanyEventWeekAnchorDate(target.eventDate);
            }
          })
          .catch(() => undefined);
        break;
      case "transcript_segment":
        if (match.parentId) {
          setSelectedTranscriptJobId(match.parentId);
          setSearchFocusSelector(`[data-transcript-job-id="${match.parentId}"]`);
        }
        setActiveSection("Transcripts");
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
    // eslint-disable-next-line react-hooks/exhaustive-deps -- view-model memo keyed on the listed UI state; the plain keyboard-navigation helpers (selectAdjacentCompany/selectAdjacentInboxItem/switchCompanyWorkspaceTab) are excluded to keep it from recomputing every render — they read the same state already in the dep list
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

  // Global-screen view-models, extracted so both the top-nav section blocks and
  // the Research cockpit (which hosts these screens as panels, ADR 0053 phase 4c)
  // share one source of truth. Lifting these providers to also wrap the cockpit
  // is the decoupling the cockpit-as-host end state (phase 6) needs.
  const watchlistsViewModel = {
    companies,
    watchlists,
    watchlistMemberships,
    watchlistsError,
    selectedWatchlistId: selectedManagedWatchlistId,
    setSelectedWatchlistId: setSelectedManagedWatchlistId,
    createWatchlist,
    renameWatchlist,
    deleteWatchlist,
    addCompanyToWatchlist,
    removeCompanyFromWatchlist,
  };
  const reportSeasonViewModel = {
    watchlists,
    openCompanyWorkspace: (companyId: string, tab: CompanyWorkspaceTab) => {
      setSelectedCompanyId(companyId);
      setCompanyWorkspaceTab(tab);
      setActiveSection("Companies");
    },
  };
  const researchViewModel: ResearchScreenProps = {
    companies,
    watchlists,
    watchlistMemberships,
    mode: researchMode,
    selectedCompanyId: selectedResearchCompanyId,
    selectedWatchlistId: selectedResearchWatchlistId,
    selectedWatchlistCompanyId: selectedResearchWatchlistCompanyId,
    cascadeToCompanies: researchCascadeToCompanies,
    selectedEvidenceTypes: researchEvidenceTypes,
    changedOnly: researchChangedOnly,
    timeline: researchTimeline,
    questions: researchQuestions,
    selectedQuestionId: selectedResearchQuestionId,
    questionTitle: researchQuestionTitle,
    questionBody: researchQuestionBody,
    questionLinks: researchQuestionLinks,
    briefJobs: researchBriefJobs,
    digestJobs: researchDigestJobs,
    reminders: researchReminders,
    error: researchError,
    loading: researchLoading,
    reviewInFlight: researchReviewInFlight,
    questionInFlight: researchQuestionInFlight,
    briefInFlight: researchBriefInFlight,
    digestInFlight: researchDigestInFlight,
    reminderInFlight: researchReminderInFlight,
    setMode: setResearchMode,
    setSelectedCompanyId: setSelectedResearchCompanyId,
    setSelectedWatchlistId: setSelectedResearchWatchlistId,
    setSelectedWatchlistCompanyId: setSelectedResearchWatchlistCompanyId,
    setSelectedQuestionId: setSelectedResearchQuestionId,
    setQuestionTitle: setResearchQuestionTitle,
    setQuestionBody: setResearchQuestionBody,
    setCascadeToCompanies: setResearchCascadeToCompanies,
    setChangedOnly: setResearchChangedOnly,
    toggleEvidenceType: toggleResearchEvidenceType,
    clearEvidenceTypes: clearResearchEvidenceTypes,
    refreshTimeline: () => {
      void refreshResearchTimeline();
    },
    markReviewed: () => {
      void markResearchReviewed();
    },
    createQuestion: () => {
      void createResearchQuestion();
    },
    updateQuestionStatus: (questionId, status) => {
      void updateResearchQuestionStatus(questionId, status);
    },
    deleteQuestion: (questionId) => {
      void deleteResearchQuestion(questionId);
    },
    linkEvidence: (item) => {
      void linkEvidenceToSelectedQuestion(item);
    },
    unlinkEvidence: (linkId) => {
      void unlinkEvidenceFromSelectedQuestion(linkId);
    },
    startBrief: () => {
      void startResearchBrief();
    },
    startDigest: () => {
      void startResearchDigest();
    },
    createReminder: (title, body, dueAt) => {
      void createResearchReminder(title, body, dueAt);
    },
    completeReminder: (reminderId) => {
      void completeResearchReminder(reminderId);
    },
    snoozeReminder: (reminderId) => {
      void snoozeResearchReminder(reminderId);
    },
    reopenReminder: (reminderId) => {
      void reopenResearchReminder(reminderId);
    },
    deleteReminder: (reminderId) => {
      void deleteResearchReminder(reminderId);
    },
    openEvidence: openResearchEvidence,
    openEvidenceUrl: openExternalUrl,
    formatTimestamp,
  };
  const notebooksViewModel: NotebooksScreenProps = {
    companies: filteredNotebookScreenCompanies,
    totalCompanyCount: companies.length,
    watchlists,
    notebookEntries,
    selectedNotebookScreenCompany,
    selectedNotebookScreenEntries,
    selectedNotebookScreenEntry,
    isNotebookScreenComposerOpen,
    isNotebookScreenEditMode,
    isNotebookScreenEditDirty,
    notebookScreenKindFilter,
    notebookScreenWatchlistFilter,
    notebookScreenClaimStatusFilter,
    notebookScreenFollowUpFilter,
    notebookScreenTagFilter,
    notebookScreenForm,
    notebookScreenEditForm,
    notebookError,
    selectNotebookScreenCompany,
    showNotebookCompanyOpenClaims,
    showNotebookCompanyFollowUps,
    focusCompanyWorkspace,
    toggleNotebookScreenComposer,
    discardNotebookScreenDraft,
    createNotebookScreenEntry,
    toggleNotebookScreenEntry,
    saveNotebookScreenEntry,
    deleteNotebookScreenEntry,
    cancelNotebookScreenEdit,
    setNotebookScreenEditMode,
    setNotebookScreenKindFilter,
    setNotebookScreenWatchlistFilter,
    setNotebookScreenClaimStatusFilter,
    setNotebookScreenFollowUpFilter,
    setNotebookScreenTagFilter,
    updateNotebookScreenForm,
    updateNotebookScreenEditForm,
    NotebookDateField,
    NotebookQuarterField,
    MarkdownNoteBody,
    renderNotebookOrigins,
  };
  const eventsViewModel: EventsScreenProps = {
    companies,
    watchlists,
    companyEvents,
    companyEventsError,
    selectedCompanyEventId,
    sourceRefreshState,
    selectedSourceAdapterId,
    sourceAdapterRefreshInFlight,
    companyEventViewMode,
    companyEventMode,
    companyEventWeekRange,
    companyEventWorkingWeekDays,
    companyEventWeekendDays,
    companyEventWeekendEvents,
    companyEventsByDate,
    companyEventWatchlistFilter,
    companyEventCompanyFilter,
    companyEventTypeFilter,
    companyEventStatusFilter,
    companyEventDateFrom,
    companyEventDateTo,
    companyEventTypes,
    companyEventStatuses,
    isCompanyEventComposerOpen,
    companyEventForm,
    companyEventCreateError,
    companyEventTypeOptions,
    companyEventStatusOptions,
    refreshEventSources,
    confirmDerivedEvent,
    openCompanyEventComposer,
    setCompanyEventViewMode,
    setCompanyEventMode,
    setCompanyEventWeekAnchorDate,
    setCompanyEventWatchlistFilter,
    setCompanyEventCompanyFilter,
    setCompanyEventTypeFilter,
    setCompanyEventStatusFilter,
    setCompanyEventDateFrom,
    setCompanyEventDateTo,
    setCompanyEventComposerOpen,
    setCompanyEventCreateError,
    setCompanyEventForm,
    setSelectedCompanyEventId,
    clearCompanyEventFilters,
    createCompanyEvent,
    NotebookDateField,
    formatLocalDate,
    parseLocalDate,
    addLocalDays,
    formatWeekRange,
    formatTimestamp,
    formatCompanyEventType,
    formatCompanyEventStatus,
    formatCompanyEventSourceType,
    companyEventDueLabel,
    companyEventDueClass,
    openExternalUrl,
  };

  // Pinned-company spine (ADR 0054). Resolve persisted IDs against the live
  // company list, dropping any that no longer exist, and preserve pin order.
  const pinnedCompanyIds = settings?.pinnedCompanyIds ?? [];
  const pinnedCompanies: PinnedCompany[] = pinnedCompanyIds
    .map((id) => companies.find((company) => company.id === id))
    .filter((company): company is Company => Boolean(company))
    .map((company) => ({ id: company.id, name: company.displayName, ticker: company.ticker }));

  function openPinnedCompany(companyId: string) {
    const company = companies.find((candidate) => candidate.id === companyId);
    if (company) {
      openCompanyWorkspace(company);
    }
  }

  function unpinCompany(companyId: string) {
    updatePinnedCompanyIds(pinnedCompanyIds.filter((id) => id !== companyId));
  }

  function togglePinnedCompany(companyId: string) {
    updatePinnedCompanyIds(
      pinnedCompanyIds.includes(companyId)
        ? pinnedCompanyIds.filter((id) => id !== companyId)
        : [...pinnedCompanyIds, companyId],
    );
  }

  function openCompanyWorkspaceById(companyId: string, tab: CompanyWorkspaceTab) {
    setSelectedCompanyId(companyId);
    setCompanyWorkspaceTab(tab);
    setActiveSection("Companies");
  }

  // Opt-in dockview "Advanced layout" for a company (ADR 0054): open the cockpit
  // scoped to it. All cockpit/dockview work carries forward as this engine.
  function openAdvancedLayout(companyId: string) {
    setCockpitInitialCompanyId(companyId);
    setActiveSection("Cockpit");
  }

  return (
    <LocaleContext.Provider value={{ locale, t: makeTranslator(locale), text }}>
      <SettingsProvider value={settings ?? null}>
      <AppShell
        activeSection={activeSection}
        dbRefreshState={dbRefreshState}
        effectiveTheme={effectiveTheme}
        health={health}
        openSourceStatus={openSourceStatus}
        refreshDatabaseBackedViews={refreshDatabaseBackedViews}
        refreshSources={refreshSources}
        setActiveSection={setActiveSection}
        onNavigateToSearchResult={navigateToSearchResult}
        pinnedCompanies={pinnedCompanies}
        selectedCompanyId={selectedCompanyId}
        onOpenCompany={openPinnedCompany}
        onUnpinCompany={unpinCompany}
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
      >
        <section
          className={activeSection === "Inbox" ? "content-grid" : "content-grid content-grid-single"}
          ref={contentGridRef}
          style={
            activeSection === "Inbox"
              ? ({ "--detail-pane-width": `${Math.round(detailPaneFraction * 100)}%` } as CSSProperties)
              : undefined
          }
        >
          <ErrorBoundary
            resetKey={activeSection}
            fallback={(error, reset) => <AppContentErrorFallback error={error} reset={reset} />}
          >
          {activeSection === "Inbox" ? (
            <InboxProvider
              value={{
                watchlists,
                companies,
                feedTypes,
                feedSources,
                feedSignalCategories,
                filteredFeedItems,
                signalsByFeedItemId,
                signalsError,
                aiSignalClassificationState,
                selectedFeedItem,
                selectedFeedCompany,
                aiAnalysisJobsByFeedItemId,
                aiAnalysisErrorByFeedItemId,
                aiAnalysisRequestInFlightByFeedItemId,
                inboxStatusFilter,
                searchQuery,
                inboxWatchlistFilter,
                inboxCompanyFilter,
                inboxTypeFilter,
                inboxSignalFilter,
                inboxSourceFilter,
                inboxReviewStats,
                inboxEmptyState,
                hasActiveInboxFilters,
                deleteUnsavedFeedState,
                sourceRefreshState,
                detailPaneFraction,
                detailPaneMinFraction,
                detailPaneMaxFraction,
                feedError,
                deleteUnsavedFeedError,
                sourceRefreshError,
                healthError,
                databaseError,
                setInboxStatusFilter,
                setSearchQuery,
                setInboxWatchlistFilter,
                setInboxCompanyFilter,
                setInboxTypeFilter,
                setInboxSignalFilter,
                setInboxSourceFilter,
                confirmCompanySignal,
                rejectCompanySignal,
                runAiSignalClassification,
                setSelectedFeedItemId,
                setActiveSection,
                markVisibleInboxAsRead,
                deleteUnsavedFeedItems,
                clearInboxFilters,
                refreshSources,
                openSourceStatus,
                toggleFeedItemReadState,
                selectFeedItemFromKeyboard,
                updateSelectedFeedItem,
                openCompanyWorkspaceFromFeedItem,
                openFeedItemNoteDraft,
                startFeedItemAiAnalysis,
                retryFeedItemAiAnalysis,
                resizeDetailPaneWithKeyboard,
                startDetailPaneResize,
                resizeDetailPane,
                stopDetailPaneResize,
                feedItemSummary,
                formatTimestamp,
              }}
            >
              <InboxScreen />
            </InboxProvider>
          ) : null}
          {activeSection === "Cockpit" ? (
            // The cockpit hosts the global app screens as panels (ADR 0053 phase
            // 4c), so it shares their view-model providers with the top-nav.
            <WatchlistsProvider value={watchlistsViewModel}>
              <ResearchProvider value={researchViewModel}>
                <NotebooksProvider value={notebooksViewModel}>
                  <ReportSeasonProvider value={reportSeasonViewModel}>
                    <EventsProvider value={eventsViewModel}>
                      <CockpitScreen
                        companies={companies}
                        feedItems={feedState}
                        initialCompanyId={cockpitInitialCompanyId}
                      />
                    </EventsProvider>
                  </ReportSeasonProvider>
                </NotebooksProvider>
              </ResearchProvider>
            </WatchlistsProvider>
          ) : null}
          {activeSection === "Today" ? (
            <TodayScreen
              companies={companies}
              watchlists={watchlists}
              pinnedCompanyIds={pinnedCompanyIds}
              recentFeedItems={feedState}
              openCompanyWorkspace={openCompanyWorkspaceById}
              openInbox={() => setActiveSection("Inbox")}
            />
          ) : null}
          {activeSection === "Compare" ? <CompareScreen /> : null}
          {activeSection === "Companies" ? (
            <CompaniesProvider
              value={{
                watchlists,
                companyFieldRefs,
                companyForm,
                companyFormRegistryMatches,
                companyListSearch,
                companyWatchlistFilter,
                filteredCompanies,
                companies,
                selectedCompany,
                onTogglePinnedCompany: togglePinnedCompany,
                onOpenAdvancedLayout: openAdvancedLayout,
                workspaceAutoFocusId,
                clearWorkspaceAutoFocus: () => setWorkspaceAutoFocusId(null),
                membershipsByCompany,
                selectedCompanyFeedStats,
                companyWorkspaceTab,
                selectedCompanyFeedItems,
                selectedCompanyFeedItem,
                aiAnalysisJobsByFeedItemId,
                aiAnalysisErrorByFeedItemId,
                aiAnalysisRequestInFlightByFeedItemId,
                selectedCompanyNotebookEntries,
                isNotebookComposerOpen,
                notebookForm,
                selectedNotebookEntryId,
                selectedNotebookEntry,
                notebookEditMode: isNotebookEditMode,
                notebookEditForm,
                isNotebookEditDirty,
                notebookError,
                companiesError,
                lookupStatus,
                createCompany,
                updateCompanyForm,
                clearCompanyFormField,
                lookupCompanyIfUseful,
                lookupCompany,
                applyRegistryEntryToCompanyForm,
                setCompanyListSearch,
                setCompanyWatchlistFilter,
                openWatchlistFromCompanyRow,
                openCompanyWorkspace,
                openCompanyWorkspaceFromKeyboard,
                deleteCompany,
                setCompanyWorkspaceTab,
                toggleCompanyFeedItem,
                selectCompanyFeedItemFromKeyboard,
                updateFeedItemState,
                inspectCompanyFeedItem,
                openFeedItemNoteDraft,
                startFeedItemAiAnalysis,
                retryFeedItemAiAnalysis,
                openCompanyInboxFilter,
                setNotebookComposerOpen,
                updateNotebookForm,
                createNotebookEntry,
                setSelectedNotebookEntryId,
                saveNotebookEntry,
                cancelNotebookEdit,
                setNotebookEditMode,
                updateNotebookEditForm,
                NotebookDateField,
                NotebookQuarterField,
                MarkdownNoteBody,
                renderNotebookOrigins,
                formatTimestamp,
                feedItemSummary,
                financialPeriods,
                financialFacts,
                kpiDefinitions,
                fundamentalsForm,
                financialFactForm,
                selectedFinancialFactId,
                isFinancialFactEditMode,
                fundamentalsError,
                fundamentalsLoadError,
                createFinancialPeriod,
                saveFinancialFact,
                deleteFinancialFact,
                selectFinancialFact,
                startEditingFinancialFact,
                cancelEditingFinancialFact,
                updateFundamentalsForm,
                updateFinancialFactForm,
              }}
            >
              <CompaniesScreen />
            </CompaniesProvider>
          ) : null}
          {activeSection === "Watchlists" ? (
            <WatchlistsProvider value={watchlistsViewModel}>
              <WatchlistsScreen />
            </WatchlistsProvider>
          ) : null}
          {activeSection === "Research" ? (
            <ResearchProvider value={researchViewModel}>
              <ResearchScreen />
            </ResearchProvider>
          ) : null}
          {activeSection === "Notebooks" ? (
            <NotebooksProvider value={notebooksViewModel}>
              <NotebooksScreen />
            </NotebooksProvider>
          ) : null}
          {activeSection === "ReportSeason" ? (
            <ReportSeasonProvider value={reportSeasonViewModel}>
              <ReportSeasonScreen />
            </ReportSeasonProvider>
          ) : null}
          {activeSection === "Events" ? (
            <EventsProvider value={eventsViewModel}>
              <EventsScreen />
            </EventsProvider>
          ) : null}
          {activeSection === "Transcripts" ? (
            <TranscriptsProvider
              value={{
                companies,
                settings,
                geminiCredentialStatus,
                transcriptJobs,
                transcriptJobsError,
                transcriptJobForm,
                transcriptJobCreateError,
                transcriptJobCreateState,
                transcriptJobRunInFlight,
                selectedTranscriptJobId,
                transcriptSegmentsByJobId,
                transcriptSegmentsErrorByJobId,
                transcriptSegmentSearchByJobId,
                selectedTranscriptSegmentIdsByJobId,
                transcriptNoteDraftJobId,
                transcriptNoteForm,
                transcriptNoteErrorByJobId,
                transcriptNoteSaveInFlight,
                transcriptLinkQueryByJobId,
                transcriptLinkErrorByJobId,
                transcriptLinkInFlight,
                transcriptDeleteInFlight,
                transcriptDescriptionDraftByJobId,
                transcriptDescriptionErrorByJobId,
                transcriptDescriptionSaveInFlight,
                transcriptCompanySuggestions,
                NotebookDateField,
                NotebookQuarterField,
                setTranscriptJobForm,
                setTranscriptJobCreateError,
                setTranscriptSegmentSearchByJobId,
                setTranscriptDescriptionDraftByJobId,
                refreshTranscriptJobs,
                createTranscriptJob,
                toggleTranscriptJob,
                toggleTranscriptJobFromKeyboard,
                runTranscriptJob,
                deleteTranscriptJob,
                updateTranscriptJobDescription,
                updateTranscriptLinkQuery,
                linkTranscriptJobCompany,
                toggleTranscriptSegment,
                openTranscriptNoteDraft,
                createTranscriptNotebookEntry,
                discardTranscriptNoteDraft,
                updateTranscriptNoteForm,
                selectTranscriptCompany,
                formatAiProvider,
                formatGeminiModel,
                formatCredentialConfigured,
                formatEnumLabel,
              }}
            >
              <TranscriptsScreen />
            </TranscriptsProvider>
          ) : null}
          {activeSection === "Sources" ? (
            <SourcesProvider
              value={{
                sourceAdapters,
                sourceAdaptersError,
                selectedSourceAdapterId,
                sourceRefreshState,
                sourceRefreshResult,
                sourceRefreshError,
                sourceAdapterRefreshInFlight,
                registryRefreshState,
                registryRefreshResult,
                registryRefreshError,
                companyRegistryEntries,
                filteredCompanyRegistryEntries,
                companyRegistryEntriesError,
                isCompanyRegistryListExpanded,
                companyRegistrySearch,
                addingRegistryTicker,
                unmatchedSourceItems,
                unmatchedSourceItemsError,
                expandedUnmatchedAdapters,
                refreshSources,
                refreshCompanyRegistry,
                setSourceEnabled,
                toggleSourceAdapter,
                toggleSourceAdapterFromKeyboard,
                toggleCompanyRegistryList,
                toggleUnmatchedSourceItems,
                setCompanyRegistrySearch,
                addCompanyFromRegistry,
                openExternalUrl,
                formatSourceScheduler,
                formatNextRefresh,
                formatTimestamp,
              }}
            >
              <SourcesScreen />
            </SourcesProvider>
          ) : null}
          {activeSection === "Diagnostics" && settings?.developerMode ? (
            <DiagnosticsScreen onDisableDeveloperMode={disableDeveloperMode} />
          ) : null}
          {activeSection === "Settings" ? (
            <SettingsScreenProvider
              value={{
                theme,
                accentPalette,
                locale,
                settings,
                settingsError,
                licenseError,
                licenseInFlight,
                licenseKeyDraft,
                licenseStatus,
                feedPruneRetentionDays,
                feedPruneResult,
                geminiCredentialStatus,
                geminiCredentialError,
                geminiCredentialInFlight,
                geminiApiKeyDraft,
                shortcutBindings,
                shortcutReferences,
                onThemeChange: updateTheme,
                onAccentPaletteChange: updateAccentPalette,
                onLocaleChange: updateLocale,
                onPollIntervalChange: updatePollInterval,
                onShortcutBindingsChange: updateShortcutBindings,
                onYoutubeTranscriptionModelChange: updateYoutubeTranscriptionModel,
                onYoutubeTranscriptionTimeoutChange: updateYoutubeTranscriptionTimeout,
                onGeneralAnalysisProviderChange: updateGeneralAnalysisProvider,
                onGeneralAnalysisModelChange: updateGeneralAnalysisModel,
                onGeneralAnalysisTimeoutChange: updateGeneralAnalysisTimeout,
                onEspiAiFallbackChange: updateEspiAiFallbackEnabled,
                onLogLevelChange: updateLogLevel,
                onLogMaxFilesChange: updateLogMaxFiles,
                onLogMaxFileBytesChange: updateLogMaxFileBytes,
                onDbMaxConnectionsChange: updateDbMaxConnections,
                onDbBusyTimeoutMsChange: updateDbBusyTimeoutMs,
                onDbAcquireTimeoutMsChange: updateDbAcquireTimeoutMs,
                onResetDatabaseSettings: resetDatabaseSettings,
                onClearLicenseKey: clearLicenseKey,
                onLicenseKeyDraftChange: setLicenseKeyDraft,
                onSubmitLicenseKey: submitLicenseKey,
                onGeminiApiKeyDraftChange: setGeminiApiKeyDraft,
                onSaveGeminiApiKey: saveGeminiApiKey,
                onClearGeminiApiKey: clearGeminiApiKey,
                onOpenGeminiApiKeyPage: () => {
                  void openUrl("https://aistudio.google.com/app/apikey");
                },
                onImportApplied: refreshDatabaseBackedViews,
                formatTimestamp,
                formatPollInterval,
                formatAiProvider,
                formatGeminiModel,
                formatCredentialConfigured,
                formatCredentialKind,
              }}
            >
              <SettingsScreen />
            </SettingsScreenProvider>
          ) : null}
          </ErrorBoundary>

        </section>
      </AppShell>
      </SettingsProvider>
    </LocaleContext.Provider>
  );
}
