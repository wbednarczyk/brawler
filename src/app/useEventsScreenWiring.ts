import type { ComponentType, Dispatch, SetStateAction } from "react";
import type { Company, CompanyEvent, SourceAdapter, Watchlist } from "../api/types";
import type { EventsScreenProps, CompanyEventWeekDay } from "../screens/Events/eventTypes";
import type { CompanyEventForm, CompanyEventMode, CompanyEventViewMode } from "../shared/types/events";
import type { NotebookDateLikeFieldProps } from "../shared/types/notebook";

const CALENDAR_SOURCE_TYPES = new Set(["official_calendar", "public_calendar"]);

/**
 * The Events screen's max calendar-adapter `lastSuccessAt` (F4b S3 contract §
 * Events, State matrix "Empty" row) — the timestamp the empty-week
 * invitation's "refreshed {time}" line reads. Deliberately its own value
 * rather than an extension of the global three-field `SourceStatusSummary`
 * (F4b contract § Events, State matrix note).
 */
export function calendarLastSuccessAt(sourceAdapters: SourceAdapter[]): string | null {
  let latest: string | null = null;
  for (const adapter of sourceAdapters) {
    if (!CALENDAR_SOURCE_TYPES.has(adapter.sourceType) || !adapter.lastSuccessAt) continue;
    if (!latest || Date.parse(adapter.lastSuccessAt) > Date.parse(latest)) {
      latest = adapter.lastSuccessAt;
    }
  }
  return latest;
}

type EventsScreenWiringInput = {
  companies: Company[];
  watchlists: Watchlist[];
  companyEvents: CompanyEvent[];
  companyEventsError: string | null;
  companyEventsLoading: boolean;
  selectedCompanyEventId: string | null;
  sourceRefreshState: string;
  sourceAdapterRefreshInFlight: string | null;
  sourceAdapters: SourceAdapter[];
  findNextWeekWithEvents: () => Promise<CompanyEvent | null>;
  openCompanyWorkspaceById: (companyId: string) => void;
  companyEventViewMode: CompanyEventViewMode;
  companyEventMode: CompanyEventMode;
  companyEventWeekRange: { start: string; end: string };
  companyEventWorkingWeekDays: CompanyEventWeekDay[];
  companyEventWeekendDays: CompanyEventWeekDay[];
  companyEventWeekendEvents: CompanyEvent[];
  companyEventsByDate: Record<string, CompanyEvent[]>;
  companyEventWatchlistFilter: string;
  companyEventCompanyFilter: string;
  companyEventTypeFilter: string;
  companyEventStatusFilter: string;
  companyEventDateFrom: string;
  companyEventDateTo: string;
  companyEventTypes: string[];
  companyEventStatuses: string[];
  isCompanyEventComposerOpen: boolean;
  companyEventForm: CompanyEventForm;
  companyEventCreateError: string | null;
  companyEventTypeOptions: string[];
  companyEventStatusOptions: string[];
  refreshEventSources: (trigger: "manual", weekStart: string) => void;
  openCompanyEventComposer: () => void;
  setCompanyEventViewMode: (value: CompanyEventViewMode) => void;
  setCompanyEventMode: (value: CompanyEventMode) => void;
  setCompanyEventWeekAnchorDate: Dispatch<SetStateAction<string>>;
  setCompanyEventWatchlistFilter: (value: string) => void;
  setCompanyEventCompanyFilter: (value: string) => void;
  setCompanyEventTypeFilter: (value: string) => void;
  setCompanyEventStatusFilter: (value: string) => void;
  setCompanyEventDateFrom: (value: string) => void;
  setCompanyEventDateTo: (value: string) => void;
  setCompanyEventComposerOpen: (value: boolean) => void;
  setCompanyEventCreateError: (value: string | null) => void;
  setCompanyEventForm: Dispatch<SetStateAction<CompanyEventForm>>;
  setSelectedCompanyEventId: Dispatch<SetStateAction<string | null>>;
  clearCompanyEventFilters: () => void;
  createCompanyEvent: () => void;
  NotebookDateField: ComponentType<NotebookDateLikeFieldProps>;
  formatLocalDate: (date: Date) => string;
  parseLocalDate: (value: string) => Date;
  addLocalDays: (date: Date, days: number) => Date;
  formatWeekRange: (startDate: string, endDate: string) => string;
  formatCompanyEventType: (value: string) => string;
  formatCompanyEventStatus: (value: string) => string;
  companyEventDueLabel: (eventDate: string) => string | null;
  companyEventDueClass: (eventDate: string) => string;
  openExternalUrl: (url: string) => void;
  confirmDerivedEvent: (eventId: string, action: "confirm" | "reject") => Promise<void>;
};

/**
 * Composes the Events screen's view model (F4b S3) — extracted from
 * AppStateRoot (file-size ratchet, ADR 0103; same composer pattern as
 * `buildWatchlistsScreenProps`). Also derives `calendarLastSuccessAt`, the
 * one genuinely new piece of state the redesign needs at the root level.
 */
export function buildEventsScreenProps(input: EventsScreenWiringInput): EventsScreenProps {
  return {
    companies: input.companies,
    watchlists: input.watchlists,
    companyEvents: input.companyEvents,
    companyEventsError: input.companyEventsError,
    companyEventsLoading: input.companyEventsLoading,
    selectedCompanyEventId: input.selectedCompanyEventId,
    sourceRefreshState: input.sourceRefreshState,
    sourceAdapterRefreshInFlight: input.sourceAdapterRefreshInFlight,
    calendarLastSuccessAt: calendarLastSuccessAt(input.sourceAdapters),
    findNextWeekWithEvents: input.findNextWeekWithEvents,
    openCompanyEventCompanyWorkspace: (companyId) => input.openCompanyWorkspaceById(companyId),
    companyEventViewMode: input.companyEventViewMode,
    companyEventMode: input.companyEventMode,
    companyEventWeekRange: input.companyEventWeekRange,
    companyEventWorkingWeekDays: input.companyEventWorkingWeekDays,
    companyEventWeekendDays: input.companyEventWeekendDays,
    companyEventWeekendEvents: input.companyEventWeekendEvents,
    companyEventsByDate: input.companyEventsByDate,
    companyEventWatchlistFilter: input.companyEventWatchlistFilter,
    companyEventCompanyFilter: input.companyEventCompanyFilter,
    companyEventTypeFilter: input.companyEventTypeFilter,
    companyEventStatusFilter: input.companyEventStatusFilter,
    companyEventDateFrom: input.companyEventDateFrom,
    companyEventDateTo: input.companyEventDateTo,
    companyEventTypes: input.companyEventTypes,
    companyEventStatuses: input.companyEventStatuses,
    isCompanyEventComposerOpen: input.isCompanyEventComposerOpen,
    companyEventForm: input.companyEventForm,
    companyEventCreateError: input.companyEventCreateError,
    companyEventTypeOptions: input.companyEventTypeOptions,
    companyEventStatusOptions: input.companyEventStatusOptions,
    refreshEventSources: input.refreshEventSources,
    openCompanyEventComposer: input.openCompanyEventComposer,
    setCompanyEventViewMode: input.setCompanyEventViewMode,
    setCompanyEventMode: input.setCompanyEventMode,
    setCompanyEventWeekAnchorDate: input.setCompanyEventWeekAnchorDate,
    setCompanyEventWatchlistFilter: input.setCompanyEventWatchlistFilter,
    setCompanyEventCompanyFilter: input.setCompanyEventCompanyFilter,
    setCompanyEventTypeFilter: input.setCompanyEventTypeFilter,
    setCompanyEventStatusFilter: input.setCompanyEventStatusFilter,
    setCompanyEventDateFrom: input.setCompanyEventDateFrom,
    setCompanyEventDateTo: input.setCompanyEventDateTo,
    setCompanyEventComposerOpen: input.setCompanyEventComposerOpen,
    setCompanyEventCreateError: input.setCompanyEventCreateError,
    setCompanyEventForm: input.setCompanyEventForm,
    setSelectedCompanyEventId: input.setSelectedCompanyEventId,
    clearCompanyEventFilters: input.clearCompanyEventFilters,
    createCompanyEvent: input.createCompanyEvent,
    NotebookDateField: input.NotebookDateField,
    formatLocalDate: input.formatLocalDate,
    parseLocalDate: input.parseLocalDate,
    addLocalDays: input.addLocalDays,
    formatWeekRange: input.formatWeekRange,
    formatCompanyEventType: input.formatCompanyEventType,
    formatCompanyEventStatus: input.formatCompanyEventStatus,
    companyEventDueLabel: input.companyEventDueLabel,
    companyEventDueClass: input.companyEventDueClass,
    openExternalUrl: input.openExternalUrl,
    confirmDerivedEvent: input.confirmDerivedEvent,
  };
}
