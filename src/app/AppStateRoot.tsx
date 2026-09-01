import {
  type CSSProperties,
  useCallback,
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
import { openExternalUrl } from "./openExternalUrl";
import {
  companyEventStatusOptions,
  companyEventTypeOptions,
  emptyCompanyEventForm,
} from "./eventForms";
import { useFeedController } from "./useFeedController";
import {
  detailPaneDefaultFraction,
  detailPaneMaxFraction,
  detailPaneMinFraction,
} from "./layout";
import { type Section } from "./navigation";
import { emptyNotebookForm, manualNotebookOrigins } from "./notebookForms";
import * as sourcesApi from "../api/sources";
import * as eventsApi from "../api/events";
import * as signalsApi from "../api/signals";
import { emptyTranscriptJobForm } from "./transcriptForms";
import { useAppLifecycleEffects } from "./useAppLifecycleEffects";
import { useAttentionController } from "./useAttentionController";
import { AlertsScreenHost } from "./useAlertsScreenWiring";
import { useAppViewModel } from "./useAppViewModel";
import { useNotebookController } from "./useNotebookController";
import { useLicenseController } from "./useLicenseController";
import { useResearchController } from "./useResearchController";
import { useSettingsController } from "./useSettingsController";
import { useSourceDisplayController } from "./useSourceDisplayController";
import { useSourceRefreshController } from "./useSourceRefreshController";
import { buildEventsScreenProps } from "./useEventsScreenWiring";
import { buildTodayScreenProps, useRefreshCompletionSignal } from "./useTodayScreenWiring";
import { useTranscriptController } from "./useTranscriptController";
import { buildWatchlistsScreenProps } from "./useWatchlistsScreenWiring";
import { useWorkspaceNavigationController } from "./useWorkspaceNavigationController";
import {
  resolveAppShortcutReferenceItems,
  type AppShortcutActionMap,
} from "./shortcuts";
import { CompaniesScreen } from "../screens/Companies/CompaniesScreen";
import { useToast, useUndoableDelete } from "../ui";
import { DecisionJournalGlobalPanel } from "../screens/Spolka/panels/DecisionJournalGlobalPanel";
import { TodayScreen } from "../screens/Today/TodayScreen";
import { SpolkaScreenHost, useSpolkaToolHost } from "./useSpolkaScreenWiring";
import { useCompanyEntryActions } from "./useCompanyEntryActions";
import { useSpolkaNavigate } from "./useSpolkaNavigate";
import { ErrorBoundary } from "./ErrorBoundary";
import { AppContentErrorFallback } from "./AppErrorFallback";
import { DiagnosticsScreen } from "../screens/Diagnostics/DiagnosticsScreen";
import { EventsScreen } from "../screens/Events/EventsScreen";
import { InboxScreen } from "../screens/Inbox/InboxScreen";
import { ReportSeasonScreen } from "../screens/ReportSeason/ReportSeasonScreen";
import type { InboxStatusFilter } from "../screens/Inbox/inboxTypes";
import { NotebooksScreen } from "../screens/Notebooks/NotebooksScreen";
import type { NotebooksScreenProps } from "../screens/Notebooks/notebookTypes";
import {
  ResearchScreen,
  type ResearchScreenProps,
} from "../screens/Research/ResearchScreen";
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
  formatDetailTimestamp,
  formatLocalDate,
  formatPollInterval,
  formatWeekRange as formatWeekRangeBase,
  parseLocalDate,
} from "../shared/format/datetime";
import {
  formatCompanyEventStatus,
  formatCompanyEventType,
  formatCredentialConfigured,
  formatCredentialKind,
  formatGeminiModel,
} from "../shared/formatting/labels";
import {
  LocaleContext,
  makeTextTranslator,
  makeTranslator,
} from "../shared/locale";
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
import type {
  CompanyEventForm,
  CompanyEventMode,
  CompanyEventViewMode,
} from "../shared/types/events";
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
  HealthResponse,
  LicenseStatus,
  NotebookDraftOrigin,
  NotebookEntry,
  SourceAdapter,
  SourceIngestionResult,
  Theme,
  TranscriptJob,
  TranscriptSegment,
  UserSettings,
  Watchlist,
  WatchlistMembership,
} from "../api/types";
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
  const companyFieldRefs = useRef<
    Record<keyof CompanyForm, HTMLInputElement | null>
  >({
    exchange: null,
    ticker: null,
    displayName: null,
    isin: null,
  });
  // Today/Pulse is the default shell home (ADR 0054); `initialSection` overrides
  // it for deep links and screen-focused tests.
  const [activeSection, setActiveSectionRaw] = useState<Section>(initialSection);
  // The Spółka workshop's tool-host state (F3a S2, ADR 0107), declared this
  // early so `setActiveSection` below can gate every cross-section navigation
  // through its dirty-tool check (`guardNavigation`), not just the ones that
  // go through `SpolkaScreenHost` itself.
  const spolkaTool = useSpolkaToolHost();
  // EVERY `setActiveSection` call in this file goes through the SAME
  // stay/discard gate the Spółka tool host uses for tool close/switch/company
  // switch (plan §S2, "navigating away"): a clean (or absent) tool proceeds
  // immediately; a dirty one opens the confirm dialog first.
  const setActiveSection = useCallback(
    (next: Section | ((current: Section) => Section)) =>
      spolkaTool.guardNavigation(() => setActiveSectionRaw(next)),
    // eslint-disable-next-line react-hooks/exhaustive-deps -- `spolkaTool` is a fresh object every render; `guardNavigation` itself is the only stable (useCallback) piece read here, and listing the whole object would make setActiveSection's identity churn every render
    [spolkaTool.guardNavigation],
  );
  const [theme, setTheme] = useState<Theme>("dark");
  const [accentPalette, setAccentPalette] =
    useState<UserSettings["accentPalette"]>("night-neon");
  const [locale, setLocale] = useState<UserSettings["locale"]>("en");
  const [health, setHealth] = useState<HealthResponse | null>(null);
  const [healthError, setHealthError] = useState<string | null>(null);
  const [, setDatabaseStatus] = useState<DatabaseStatus | null>(null);
  const [databaseError, setDatabaseError] = useState<string | null>(null);
  const [companies, setCompanies] = useState<Company[]>([]);
  const [companiesError, setCompaniesError] = useState<string | null>(null);
  const [watchlists, setWatchlists] = useState<Watchlist[]>([]);
  const [watchlistMemberships, setWatchlistMemberships] = useState<
    WatchlistMembership[]
  >([]);
  const [watchlistsError, setWatchlistsError] = useState<string | null>(null);
  const [notebookEntries, setNotebookEntries] = useState<NotebookEntry[]>([]);
  const [notebookError, setNotebookError] = useState<string | null>(null);
  const [inboxWatchlistFilter, setInboxWatchlistFilter] = useState("all");
  const [inboxCompanyFilter, setInboxCompanyFilter] = useState("all");
  const [inboxTypeFilter, setInboxTypeFilter] = useState("all");
  const [inboxSignalFilter, setInboxSignalFilter] = useState("all");
  const [inboxSourceFilter, setInboxSourceFilter] = useState("all");
  const [inboxStatusFilter, setInboxStatusFilter] =
    useState<InboxStatusFilter>("all");
  const [feedState, setFeedState] = useState<FeedItem[]>([]);
  const [feedError, setFeedError] = useState<string | null>(null);
  const [signals, setSignals] = useState<CompanySignal[]>([]);
  const [signalsError, setSignalsError] = useState<string | null>(null);
  const [companyEvents, setCompanyEvents] = useState<CompanyEvent[]>([]);
  const [companyEventsError, setCompanyEventsError] = useState<string | null>(
    null,
  );
  const [transcriptJobs, setTranscriptJobs] = useState<TranscriptJob[]>([]);
  const [transcriptJobsError, setTranscriptJobsError] = useState<string | null>(
    null,
  );
  const [transcriptJobForm, setTranscriptJobForm] = useState<TranscriptJobForm>(
    emptyTranscriptJobForm,
  );
  const [transcriptJobCreateError, setTranscriptJobCreateError] = useState<
    string | null
  >(null);
  const [transcriptJobCreateState, setTranscriptJobCreateState] =
    useState<DbRefreshState>("idle");
  const [transcriptJobRunInFlight, setTranscriptJobRunInFlight] = useState<
    string | null
  >(null);
  const [selectedTranscriptJobId, setSelectedTranscriptJobId] = useState<
    string | null
  >(null);
  const [transcriptSegmentsByJobId, setTranscriptSegmentsByJobId] = useState<
    Record<string, TranscriptSegment[]>
  >({});
  const [transcriptSegmentsErrorByJobId, setTranscriptSegmentsErrorByJobId] =
    useState<Record<string, string | null>>({});
  const [transcriptSegmentSearchByJobId, setTranscriptSegmentSearchByJobId] =
    useState<Record<string, string>>({});
  const [
    selectedTranscriptSegmentIdsByJobId,
    setSelectedTranscriptSegmentIdsByJobId,
  ] = useState<Record<string, string[]>>({});
  const [transcriptNoteDraftJobId, setTranscriptNoteDraftJobId] = useState<
    string | null
  >(null);
  const [transcriptNoteForm, setTranscriptNoteForm] =
    useState<NotebookForm>(emptyNotebookForm);
  const [transcriptNoteErrorByJobId, setTranscriptNoteErrorByJobId] = useState<
    Record<string, string | null>
  >({});
  const [transcriptNoteSaveInFlight, setTranscriptNoteSaveInFlight] = useState<
    string | null
  >(null);
  const [transcriptLinkQueryByJobId, setTranscriptLinkQueryByJobId] = useState<
    Record<string, string>
  >({});
  const [transcriptLinkErrorByJobId, setTranscriptLinkErrorByJobId] = useState<
    Record<string, string | null>
  >({});
  const [transcriptLinkInFlight, setTranscriptLinkInFlight] = useState<
    string | null
  >(null);
  const [transcriptDeleteInFlight, setTranscriptDeleteInFlight] = useState<
    string | null
  >(null);
  const [
    transcriptDescriptionDraftByJobId,
    setTranscriptDescriptionDraftByJobId,
  ] = useState<Record<string, string>>({});
  const [
    transcriptDescriptionErrorByJobId,
    setTranscriptDescriptionErrorByJobId,
  ] = useState<Record<string, string | null>>({});
  const [
    transcriptDescriptionSaveInFlight,
    setTranscriptDescriptionSaveInFlight,
  ] = useState<string | null>(null);
  const [companyEventViewMode, setCompanyEventViewMode] =
    useState<CompanyEventViewMode>("week");
  const [companyEventMode, setCompanyEventMode] =
    useState<CompanyEventMode>("upcoming");
  const [companyEventWeekAnchorDate, setCompanyEventWeekAnchorDate] = useState(
    () => formatLocalDate(new Date()),
  );
  const [companyEventWatchlistFilter, setCompanyEventWatchlistFilter] =
    useState("all");
  const [companyEventCompanyFilter, setCompanyEventCompanyFilter] =
    useState("all");
  const [companyEventTypeFilter, setCompanyEventTypeFilter] = useState("all");
  const [companyEventStatusFilter, setCompanyEventStatusFilter] =
    useState("all");
  const [companyEventDateFrom, setCompanyEventDateFrom] = useState("");
  const [companyEventDateTo, setCompanyEventDateTo] = useState("");
  const [selectedCompanyEventId, setSelectedCompanyEventId] = useState<
    string | null
  >(null);
  const [isCompanyEventComposerOpen, setCompanyEventComposerOpen] =
    useState(false);
  const [companyEventForm, setCompanyEventForm] = useState<CompanyEventForm>(
    emptyCompanyEventForm,
  );
  const [companyEventCreateError, setCompanyEventCreateError] = useState<
    string | null
  >(null);
  const [sourceAdapters, setSourceAdapters] = useState<SourceAdapter[]>([]);
  const [sourceAdaptersError, setSourceAdaptersError] = useState<string | null>(
    null,
  );
  const [selectedSourceAdapterId, setSelectedSourceAdapterId] = useState<
    string | null
  >(null);
  const [settings, setSettings] = useState<UserSettings | null>(null);
  const [settingsError, setSettingsError] = useState<string | null>(null);
  const [licenseStatus, setLicenseStatus] = useState<LicenseStatus | null>(
    initialLicenseStatus,
  );
  const [licenseError, setLicenseError] = useState<string | null>(null);
  const [licenseKeyDraft, setLicenseKeyDraft] = useState("");
  const [licenseInFlight, setLicenseInFlight] = useState(false);
  const [geminiCredentialStatus, setGeminiCredentialStatus] =
    useState<CredentialStatus | null>(null);
  const [geminiCredentialError, setGeminiCredentialError] = useState<
    string | null
  >(null);
  const [geminiApiKeyDraft, setGeminiApiKeyDraft] = useState("");
  const [geminiCredentialInFlight, setGeminiCredentialInFlight] =
    useState(false);
  const [selectedFeedItemId, setSelectedFeedItemId] = useState<string | null>(
    null,
  );
  const [selectedCompanyId, setSelectedCompanyId] = useState<string | null>(
    null,
  );
  const [searchFocusSelector, setSearchFocusSelector] = useState<string | null>(
    null,
  );
  const [selectedCompanyFeedItemId, setSelectedCompanyFeedItemId] = useState<
    string | null
  >(null);
  const [selectedNotebookEntryId, setSelectedNotebookEntryId] = useState<
    string | null
  >(null);
  const [isNotebookComposerOpen, setNotebookComposerOpen] = useState(false);
  const [isNotebookEditMode, setNotebookEditMode] = useState(false);
  const [selectedNotebookCompanyId, setSelectedNotebookCompanyId] = useState<
    string | null
  >(null);
  const [selectedNotebookScreenEntryId, setSelectedNotebookScreenEntryId] =
    useState<string | null>(null);
  const [isNotebookScreenComposerOpen, setNotebookScreenComposerOpen] =
    useState(false);
  const [isNotebookScreenEditMode, setNotebookScreenEditMode] = useState(false);
  const [notebookScreenKindFilter, setNotebookScreenKindFilter] =
    useState("all");
  const [notebookScreenWatchlistFilter, setNotebookScreenWatchlistFilter] =
    useState("all");
  const [notebookScreenClaimStatusFilter, setNotebookScreenClaimStatusFilter] =
    useState("all");
  const [notebookScreenFollowUpFilter, setNotebookScreenFollowUpFilter] =
    useState("all");
  const [notebookScreenTagFilter, setNotebookScreenTagFilter] = useState("");
  const [notebookScreenForm, setNotebookScreenForm] =
    useState<NotebookForm>(emptyNotebookForm);
  const [notebookScreenDraftOrigins, setNotebookScreenDraftOrigins] = useState<
    NotebookDraftOrigin[]
  >(manualNotebookOrigins);
  const [notebookScreenEditForm, setNotebookScreenEditForm] =
    useState<NotebookForm>(emptyNotebookForm);
  const [detailPaneFraction, setDetailPaneFraction] = useState(
    detailPaneDefaultFraction,
  );
  const [dbRefreshState, setDbRefreshState] = useState<DbRefreshState>("idle");
  const [sourceRefreshState, setSourceRefreshState] =
    useState<SourceRefreshState>("idle");
  const [sourceRefreshResult, setSourceRefreshResult] =
    useState<SourceIngestionResult | null>(null);
  const [sourceRefreshError, setSourceRefreshError] = useState<string | null>(
    null,
  );
  const [sourceRefreshFailureCount, setSourceRefreshFailureCount] = useState(0);
  const [sourceAdapterRefreshInFlight, setSourceAdapterRefreshInFlight] =
    useState<string | null>(null);
  const [registryRefreshState, setRegistryRefreshState] =
    useState<SourceRefreshState>("idle");
  const [registryRefreshResult, setRegistryRefreshResult] =
    useState<CompanyRegistryRefreshResult | null>(null);
  const [registryRefreshError, setRegistryRefreshError] = useState<
    string | null
  >(null);
  const [nextSourceRefreshAtByAdapterId, setNextSourceRefreshAtByAdapterId] =
    useState<Record<string, number>>({});
  const [nextRegistryRefreshAt, setNextRegistryRefreshAt] = useState<
    number | null
  >(null);
  const [companyRegistryEntries, setCompanyRegistryEntries] = useState<
    CompanyRegistryEntry[]
  >([]);
  const [companyRegistryEntriesError, setCompanyRegistryEntriesError] =
    useState<string | null>(null);
  const [isCompanyRegistryListExpanded, setCompanyRegistryListExpanded] =
    useState(false);
  const [companyRegistrySearch, setCompanyRegistrySearch] = useState("");
  const [selectedCompanyRegistryTicker, setSelectedCompanyRegistryTicker] =
    useState<string | null>(null);
  const [addingRegistryTicker, setAddingRegistryTicker] = useState<
    string | null
  >(null);
  const [companyListSearch, setCompanyListSearch] = useState("");
  const [companyWatchlistFilter, setCompanyWatchlistFilter] = useState("all");
  const [selectedManagedWatchlistId, setSelectedManagedWatchlistId] = useState<
    string | null
  >(null);
  const [searchQuery, setSearchQuery] = useState("");
  const [lookupStatus, setLookupStatus] = useState<string | null>(null);
  const [companyForm, setCompanyForm] = useState<CompanyForm>({
    exchange: "GPW",
    ticker: "",
    displayName: "",
    isin: "",
  });
  const [notebookForm, setNotebookForm] =
    useState<NotebookForm>(emptyNotebookForm);
  const [notebookEditForm, setNotebookEditForm] =
    useState<NotebookForm>(emptyNotebookForm);

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
    isNotebookScreenEditDirty,
    membershipsByCompany,
    scheduledSourceAdapters,
    selectedCompany,
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

  // Memoized so locale-keyed consumers (effects, memos) rebind only on a real
  // locale change, not every AppStateRoot render.
  const text = useMemo(() => makeTextTranslator(locale), [locale]);

  // ADR 0076 D5: reversible-destroy orchestration (immediate delete + undo toast).
  const runUndoableDelete = useUndoableDelete();
  // ADR 0068 T6: transient async-success feedback on the shared Toast surface.
  const toast = useToast();
  // ADR 0076 D4: threaded date formatters. Detail/audit rows (provenance,
  // diagnostics) show the full `YYYY-MM-DD HH:MM`; the week label is locale-bound.
  // List rows (feed streams) format at the leaf via formatListTimestamp.
  const formatTimestamp = formatDetailTimestamp;
  const formatWeekRange = (startDate: string, endDate: string) =>
    formatWeekRangeBase(startDate, endDate, locale);
  // Due labels ("Dziś"/"Za 2 dni") are user copy, so they bind the locale here
  // like formatWeekRange does (audit K8: no English date words in the PL UI).
  const companyEventDueLabelLocalized = (eventDate: string) =>
    companyEventDueLabel(eventDate, locale);
  const shortcutBindings = settings?.shortcutBindings ?? {};
  const shortcutReferences = resolveAppShortcutReferenceItems(shortcutBindings);
  const licenseCanUseApp = licenseStatus?.canUseApp !== false;

  // THE attention state (ADR 0097 dec. 6): Today's stream, the Alerts fired
  // list, and the sidebar Today badge all consume this one controller.
  const attention = useAttentionController(licenseCanUseApp);

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
    researchReminders,
    researchError,
    researchLoading,
    researchReviewInFlight,
    researchQuestionInFlight,
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
    runUndoableDelete,
  });

  const {
    clearCompanyEventFilters,
    companyEventsLoading,
    createCompanyEvent,
    findNextWeekWithEvents,
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
    refreshWatchlistMemberships,
    refreshWatchlists,
  } = useAppDataController({
    refreshCompanyEvents,
    setCompanies,
    setCompaniesError,
    setCompanyRegistryEntries,
    setCompanyRegistryEntriesError,
    setDatabaseError,
    setDatabaseStatus,
    setDbRefreshState,
    setFeedError,
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
    setWatchlistMemberships,
    setWatchlists,
    setWatchlistsError,
  });

  const { clearLicenseKey, refreshLicenseStatus, submitLicenseKey } =
    useLicenseController({
      licenseKeyDraft,
      setLicenseError,
      setLicenseInFlight,
      setLicenseKeyDraft,
      setLicenseStatus,
    });

  const { refreshCompletionCount, bumpRefreshCompletionCount } = useRefreshCompletionSignal();

  const {
    refreshBankierCalendarWeek,
    refreshCompanyRegistry,
    refreshEventSources,
    refreshSources,
  } = useSourceRefreshController({
    refreshAttention: attention.refresh,
    onRefreshCompletion: bumpRefreshCompletionCount,
    refreshCompanyEvents,
    refreshCompanyRegistryEntries,
    refreshDatabaseStatus,
    refreshFeedItems,
    refreshSignals,
    refreshSourceAdapters,
    onManualRefreshSuccess: () =>
      toast.show({ message: text("Sources refreshed"), tone: "positive" }),
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
    updatePollInterval,
    updateBackfillYears,
    updateMcpPort,
    updateMcpWritesEnabled,
    updateKpiAcquisitionEnabled,
    updateLogLevel,
    updateLogMaxFileBytes,
    updateLogMaxFiles,
    updateDbMaxConnections,
    updateDbBusyTimeoutMs,
    updateDbAcquireTimeoutMs,
    resetDatabaseSettings,
    updateSourcesWorkers,
    updateAutopilotWorkers,
    resetQueueSettings,
    updatePinnedCompanyIds,
    updateShortcutBindings,
    updateTheme,
    updateTodayReviewedDays,
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
      if (
        !event.ctrlKey ||
        !event.altKey ||
        !event.shiftKey ||
        event.key.toLowerCase() !== "d"
      ) {
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
  }, [activeSection, settings?.developerMode, setActiveSection]);

  function resetDeletedWatchlistFilters(watchlistId: string) {
    setInboxWatchlistFilter((current) =>
      current === watchlistId ? "all" : current,
    );
    setCompanyEventWatchlistFilter((current) =>
      current === watchlistId ? "all" : current,
    );
    setCompanyWatchlistFilter((current) =>
      current === watchlistId ? "all" : current,
    );
    setNotebookScreenWatchlistFilter((current) =>
      current === watchlistId ? "all" : current,
    );
    setSelectedManagedWatchlistId((current) =>
      current === watchlistId ? null : current,
    );
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
    cancelNotebookScreenEdit,
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
    runUndoableDelete,
  });

  const {
    createTranscriptJob,
    createTranscriptNotebookEntry,
    deleteTranscriptJob,
    discardTranscriptNoteDraft,
    linkTranscriptJobCompany,
    openTranscriptNoteDraft,
    refreshTranscriptJobs,
    retryTranscriptSegments,
    runTranscriptJob,
    selectTranscriptCompany,
    toggleTranscriptJob,
    toggleTranscriptJobFromKeyboard,
    toggleTranscriptSegment,
    transcriptsLoading,
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
  });

  const { navigate, highlightClaimId } = useSpolkaNavigate({
    spolkaTool,
    setSelectedCompanyId,
    setActiveSectionRaw,
  });
  const {
    clearInboxFilters,
    inboxDetailActivationToken,
    markVisibleInboxAsRead,
    openCompanyWorkspaceFromFeedItem,
    openInboxItem,
    scopeInboxToCompany,
    selectFeedItemFromKeyboard,
    toggleFeedItemReadState,
    updateSelectedFeedItem,
  } = useFeedController({
    companies,
    filteredFeedItems,
    selectedFeedItem,
    setActiveSection,
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
    navigate,
    setSelectedFeedItemId,
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

  async function confirmDerivedEvent(
    eventId: string,
    action: "confirm" | "reject",
  ) {
    try {
      await signalsApi.confirmDerivedEvent(eventId, action);
      await refreshCompanyEvents();
      setCompanyEventsError(null);
    } catch (error) {
      setCompanyEventsError(String(error));
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
    openCompanyClaims,
    openCompanyInboxFilter,
    openCompanyWorkspace,
    openCompanyWorkspaceFromKeyboard,
    renderNotebookOrigins,
  } = useWorkspaceNavigationController({
    companiesById,
    feedState,
    scopeInboxToCompany,
    selectedCompanyFeedItemId,
    selectedCompanyId,
    setActiveSection,
    setSelectedCompanyFeedItemId,
    setSelectedCompanyId,
    setSelectedFeedItemId,
    navigate,
  });

  const {
    formatNextRefresh,
    formatSourceScheduler,
    openSourceStatus,
    toggleCompanyRegistryList,
    toggleSourceAdapter,
    toggleSourceAdapterFromKeyboard,
  } = useSourceDisplayController({
    locale,
    nextRegistryRefreshAt,
    nextSourceRefreshAtByAdapterId,
    refreshCompanyRegistryEntries,
    setActiveSection,
    setCompanyRegistryListExpanded,
    setSelectedSourceAdapterId,
    settings,
    sourceAdapters,
    sourceRefreshFailureCount,
  });

  function setSourceEnabled(adapter: SourceAdapter, enabled: boolean) {
    if (!adapter.userConfigurable || adapter.enabled === enabled) {
      return;
    }

    sourcesApi
      .setSourceAdapterEnabled({ adapterId: adapter.id, enabled })
      .then((updatedAdapter) => {
        setSourceAdapters((current) =>
          current.map((sourceAdapter) =>
            sourceAdapter.id === updatedAdapter.id
              ? updatedAdapter
              : sourceAdapter,
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
    spolkaTool,
    refreshAttention: attention.refresh,
    onRefreshCompletion: bumpRefreshCompletionCount,
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
    refreshBankierCalendarWeek,
    refreshCompanies,
    refreshCompanyEvents,
    refreshCompanyRegistryEntries,
    refreshDatabaseStatus,
    refreshFeedItems,
    refreshSignals,
    refreshGeminiCredentialStatus,
    refreshHealth,
    refreshLicenseStatus,
    refreshNotebookEntries,
    refreshSettings,
    refreshSourceAdapters,
    refreshTranscriptJobs,
    refreshWatchlistMemberships,
    refreshWatchlists,
    selectedCompanyId,
    selectedFeedItemId,
    selectedNotebookCompanyId,
    selectedNotebookEntry,
    selectedNotebookScreenEntry,
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
  });

  function openResearchEvidence(item: ResearchEvidenceItem) {
    const itemCompanyTicker =
      companiesById[item.companyId]?.qualifiedTicker ?? "all";

    switch (item.sourceDomain) {
      case "feed":
        scopeInboxToCompany(itemCompanyTicker);
        setSelectedFeedItemId(item.sourceId);
        setActiveSection("Inbox");
        break;
      case "notebooks":
        setSelectedNotebookCompanyId(item.companyId);
        setSelectedNotebookScreenEntryId(item.sourceId);
        setActiveSection("Notebooks");
        break;
      case "research":
        // Evidence deep-links land on the Spółka `research` tool (F3a, ADR
        // 0107 mapping "preset 'evidence'→research").
        if (item.evidenceType === "research_question") {
          if (researchMode !== "company") {
            setResearchMode("company");
          }
          if (selectedResearchCompanyId !== item.companyId) {
            setSelectedResearchCompanyId(item.companyId);
          }
          setSelectedResearchQuestionId(item.sourceId);
        }
        navigate({ companyId: item.companyId, section: "Spolka", tool: { t: "research" } });
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
        scopeInboxToCompany(itemCompanyTicker);
        setActiveSection("Inbox");
        break;
    }
  }

  function navigateToSearchResult(match: SearchMatch) {
    switch (match.contentType) {
      case "company":
        // A company result opens the Spółka screen (F3a S1, ADR 0107).
        openCompanyWorkspaceById(match.sourceId);
        break;
      case "watchlist":
        setSelectedManagedWatchlistId(match.sourceId);
        setActiveSection("Watchlists");
        setSearchFocusSelector(`[data-watchlist-id="${match.sourceId}"]`);
        break;
      case "feed_item":
        // Clear ALL filters (via the canonical reset) so the selected item is
        // not hidden by any active filter (c80dabe).
        clearInboxFilters();
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
        // A company-scoped brief/digest opens the Spółka `research` tool
        // (F3a, ADR 0107); a companyless one opens the standalone Research
        // screen (ADR 0108). Briefs/digests render no row of their own (ADR
        // 0084 retired them), so there is nothing to focus.
        if (match.companyId) {
          if (researchMode !== "company") {
            setResearchMode("company");
          }
          setSelectedResearchCompanyId(match.companyId);
          navigate({ companyId: match.companyId, section: "Spolka", tool: { t: "research" } });
        } else {
          setActiveSection("Research");
        }
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
          setSearchFocusSelector(
            `[data-transcript-job-id="${match.parentId}"]`,
          );
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
    const nextIndex =
      currentIndex === -1
        ? 0
        : Math.min(
            Math.max(currentIndex + direction, 0),
            filteredFeedItems.length - 1,
          );

    setSelectedFeedItemId(filteredFeedItems[nextIndex]?.id ?? null);
    return true;
  }

  function selectAdjacentCompany(direction: 1 | -1) {
    if (activeSection !== "Companies" || filteredCompanies.length === 0) {
      return false;
    }

    const currentIndex = selectedCompany
      ? filteredCompanies.findIndex(
          (company) => company.id === selectedCompany.id,
        )
      : -1;
    const nextIndex =
      currentIndex === -1
        ? 0
        : Math.min(
            Math.max(currentIndex + direction, 0),
            filteredCompanies.length - 1,
          );
    const nextCompany = filteredCompanies[nextIndex];

    if (!nextCompany) {
      return false;
    }

    setSelectedCompanyId(nextCompany.id);
    return true;
  }

  const shortcutActions = useMemo<AppShortcutActionMap>(
    () => ({
      "app.openInbox": () => undefined,
      "app.openCompanies": () => undefined,
      "app.openWatchlists": () => undefined,
      "app.openNotebooks": () => undefined,
      "app.openEvents": () => undefined,
      "app.openTranscripts": () => undefined,
      "app.openSources": () => undefined,
      "app.openSettings": () => undefined,
      "app.openAlerts": () => undefined,
      "app.commandPalette": () => undefined,
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

        updateSelectedFeedItem((item) => ({ ...item, unread: false }));
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
      // The company deep-dive is the Spółka screen (ADR 0107); there is no
      // tabbed workspace to cycle. The shortcuts are kept as no-ops so existing
      // bindings/config stay valid.
      "company.nextTab": () => false,
      "company.previousTab": () => false,
      "notebook.editSelected": () => {
        if (activeSection === "Notebooks" && selectedNotebookScreenEntry) {
          setNotebookScreenEditMode(true);
          return true;
        }

        return false;
      },
      "notebook.saveCurrent": () => {
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
    }),
    // eslint-disable-next-line react-hooks/exhaustive-deps -- view-model memo keyed on the listed UI state; the plain keyboard-navigation helpers (selectAdjacentCompany/selectAdjacentInboxItem/switchCompanyWorkspaceTab) are excluded to keep it from recomputing every render — they read the same state already in the dep list
    [
      activeSection,
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
    ],
  );

  // Global-screen view-models, extracted so every render site (a standalone
  // route, a palette entry point) shares one source of truth.
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
    reminders: researchReminders,
    error: researchError,
    loading: researchLoading,
    reviewInFlight: researchReviewInFlight,
    questionInFlight: researchQuestionInFlight,
    reminderInFlight: researchReminderInFlight,
    setMode: setResearchMode,
    setSelectedCompanyId: setSelectedResearchCompanyId,
    setSelectedWatchlistId: setSelectedResearchWatchlistId,
    setSelectedWatchlistCompanyId: setSelectedResearchWatchlistCompanyId,
    openCompanyWorkspaceById: (companyId: string) => openCompanyWorkspaceById(companyId),
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
  // Pinned-company spine (ADR 0054). Resolve persisted IDs against the live
  // company list, dropping any that no longer exist, and preserve pin order.
  const pinnedCompanyIds = settings?.pinnedCompanyIds ?? [];
  const pinnedCompanies: PinnedCompany[] = pinnedCompanyIds
    .map((id) => companies.find((company) => company.id === id))
    .filter((company): company is Company => Boolean(company))
    .map((company) => ({
      id: company.id,
      name: company.displayName,
      ticker: company.ticker,
    }));

  // Every tracked company (F3a S3) — feeds the palette's `Open company:
  // TICKER` entries for every company, not only pinned ones.
  const trackedCompanies: PinnedCompany[] = companies.map((company) => ({
    id: company.id,
    name: company.displayName,
    ticker: company.ticker,
  }));
  function openPinnedCompany(companyId: string) {
    const company = companies.find((candidate) => candidate.id === companyId);
    if (company) {
      openCompanyWorkspace(company);
    }
  }

  function unpinCompany(companyId: string) {
    updatePinnedCompanyIds(pinnedCompanyIds.filter((id) => id !== companyId));
  }

  // Company deep-dive entry points (Spółka default, ADR 0107) — extracted to
  // useCompanyEntryActions.
  const { openCompanyWorkspaceById, openSpolkaMode } = useCompanyEntryActions({
    companies,
    pinnedCompanyIds,
    selectedCompanyId,
    setActiveSection,
    navigate,
  });
  const { watchlistsViewModel, reportSeasonViewModel } = buildWatchlistsScreenProps({
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
    openCompanyWorkspaceById,
  });
  const eventsViewModel = buildEventsScreenProps({
    companies,
    watchlists,
    companyEvents,
    companyEventsError,
    companyEventsLoading,
    selectedCompanyEventId,
    sourceRefreshState,
    sourceAdapterRefreshInFlight,
    sourceAdapters,
    findNextWeekWithEvents,
    openCompanyWorkspaceById,
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
    formatCompanyEventType,
    formatCompanyEventStatus,
    companyEventDueLabel: companyEventDueLabelLocalized,
    companyEventDueClass,
    openExternalUrl,
    confirmDerivedEvent,
  });
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
          onOpenSpolkaMode={openSpolkaMode}
          onNavigateToSearchResult={navigateToSearchResult}
          pinnedCompanies={pinnedCompanies}
          trackedCompanies={trackedCompanies}
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
          unseenAttentionCount={attention.unseenCount}
          attentionHydrated={attention.hydrated}
          updateTheme={updateTheme}
        >
          <section
            className={
              activeSection === "Inbox"
                ? "content-grid"
                : "content-grid content-grid-single"
            }
            ref={contentGridRef}
            style={
              activeSection === "Inbox"
                ? ({
                    "--detail-pane-width": `${Math.round(detailPaneFraction * 100)}%`,
                  } as CSSProperties)
                : undefined
            }
          >
            <ErrorBoundary
              resetKey={activeSection}
              fallback={(error, reset) => (
                <AppContentErrorFallback error={error} reset={reset} />
              )}
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
                    selectedFeedItem,
                    selectedFeedCompany,
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
                    sourceRefreshState,
                    detailPaneFraction,
                    detailPaneMinFraction,
                    detailPaneMaxFraction,
                    feedError,
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
                    setSelectedFeedItemId,
                    setActiveSection,
                    markVisibleInboxAsRead,
                    clearInboxFilters,
                    refreshSources,
                    openSourceStatus,
                    toggleFeedItemReadState,
                    selectFeedItemFromKeyboard,
                    updateSelectedFeedItem,
                    openCompanyWorkspaceFromFeedItem,
                    openFeedItemNoteDraft,
                    resizeDetailPaneWithKeyboard,
                    startDetailPaneResize,
                    resizeDetailPane,
                    stopDetailPaneResize,
                    feedItemSummary,
                    formatTimestamp,
                    inboxDetailActivationToken,
                  }}
                >
                  <InboxScreen />
                </InboxProvider>
              ) : null}
              {activeSection === "Spolka" ? (
                // The `research` workshop tool hosts the real, context-driven
                // ResearchScreen (F3a S2 — no company-scope prop of its own
                // yet), so this branch needs its own provider.
                <ResearchProvider value={researchViewModel}>
                  <SpolkaScreenHost
                    companies={companies}
                    selectedCompanyId={selectedCompanyId}
                    spolkaTool={spolkaTool}
                    feedItems={feedState}
                    rootHighlightClaimId={highlightClaimId}
                    openInboxItem={openInboxItem}
                    onSwitchCompany={(companyId) => navigate({ companyId, section: "Spolka" })}
                    refreshCompletionCount={refreshCompletionCount}
                  />
                </ResearchProvider>
              ) : null}
              {activeSection === "Journal" ? (
                // Decision journal, all companies (F3a S3, ADR 0107 plan
                // "Trasy powierzchni globalnych"): a standalone screen route.
                // No separate PanelHeader: the panel's own
                // "Decision journal" heading (SectionHeader h3) is this
                // screen's only heading — a second h1 above it would repeat
                // the same title and break heading order (axe).
                <section className="feed-panel" role="region" aria-label={text("Decision journal")}>
                  <DecisionJournalGlobalPanel companies={companies} />
                </section>
              ) : null}
              {activeSection === "Today" ? (
                <TodayScreen
                  {...buildTodayScreenProps({
                    attention,
                    companies,
                    openCompanyWorkspace: openCompanyWorkspaceById,
                    openInboxItem,
                    openCompanyInbox: openCompanyInboxFilter,
                    openCompanyClaims,
                    openExternalUrl,
                    sourceAdapters,
                    refreshSources,
                    todayReviewedDays: settings?.todayReviewedDays ?? [],
                    updateTodayReviewedDays,
                    refreshCompletionCount,
                    setActiveSection,
                  })}
                />
              ) : null}
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
                    membershipsByCompany,
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
              {activeSection === "Alerts" ? (
                <AlertsScreenHost attention={attention} openCompanyWorkspaceById={openCompanyWorkspaceById} setActiveSection={setActiveSection} />
              ) : null}
              {/* Research: palette/deep-link route until F4c adds its nav entry (#94). */}
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
                    geminiCredentialStatus,
                    transcriptJobs,
                    transcriptJobsError,
                    transcriptsLoading,
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
                    retryTranscriptSegments,
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
                    openCompanyWorkspaceById,
                    openSettings: () => setActiveSection("Settings"),
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
                    refreshSources,
                    refreshCompanyRegistry,
                    setSourceEnabled,
                    toggleSourceAdapter,
                    toggleSourceAdapterFromKeyboard,
                    toggleCompanyRegistryList,
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
                <DiagnosticsScreen
                  onDisableDeveloperMode={disableDeveloperMode}
                />
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
                    onBackfillYearsChange: updateBackfillYears,
                    onMcpPortChange: updateMcpPort,
                    onMcpWritesEnabledChange: updateMcpWritesEnabled,
    onKpiAcquisitionEnabledChange: updateKpiAcquisitionEnabled,
                    onShortcutBindingsChange: updateShortcutBindings,
                    onYoutubeTranscriptionModelChange:
                      updateYoutubeTranscriptionModel,
                    onYoutubeTranscriptionTimeoutChange:
                      updateYoutubeTranscriptionTimeout,
                    onLogLevelChange: updateLogLevel,
                    onLogMaxFilesChange: updateLogMaxFiles,
                    onLogMaxFileBytesChange: updateLogMaxFileBytes,
                    onDbMaxConnectionsChange: updateDbMaxConnections,
                    onDbBusyTimeoutMsChange: updateDbBusyTimeoutMs,
                    onDbAcquireTimeoutMsChange: updateDbAcquireTimeoutMs,
                    onResetDatabaseSettings: resetDatabaseSettings,
                    onSourcesWorkersChange: updateSourcesWorkers,
                    onAutopilotWorkersChange: updateAutopilotWorkers,
                    onResetQueueSettings: resetQueueSettings,
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
                    formatPollInterval,
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
