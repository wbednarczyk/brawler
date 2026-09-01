import type { ComponentType, Dispatch, SetStateAction } from "react";
import type { Company, CompanyEvent, Watchlist } from "../../api/types";
import type { CompanyEventForm, CompanyEventMode, CompanyEventViewMode } from "../../shared/types/events";
import type { NotebookDateLikeFieldProps } from "../../shared/types/notebook";

export type CompanyEventWeekDay = {
  date: string;
  label: string;
};

// U7-D density (ADR 0076 D6): Events forces list mode at the S width tier and the
// short height tier — the week grid is not offered/rendered there. This resolves
// the *presentation* mode without mutating the persisted `companyEventViewMode`
// preference, so widening the pane restores the stored choice.
export function resolveEventViewMode(
  stored: CompanyEventViewMode,
  compact: boolean,
): CompanyEventViewMode {
  return compact ? "list" : stored;
}

// The next-week "later match" lookup's own state (F4b sol R1): distinct from
// "no match" so a failed read never renders as a false "nothing later"
// invitation, and nothing renders as primary while it's in flight.
export type NextWeekLookup =
  | { status: "idle" }
  | { status: "pending" }
  | { status: "match"; event: CompanyEvent | null }
  | { status: "error" };

// The empty-week invitation/quiet variant (F4b S3 contract § Events point 4 /
// decision 4) — computed once in `EventsScreen` from `weekIsEmpty` +
// `hasActiveEventFilters` + `calendarLastSuccessAt` + the lookup above, then
// handed to `WeekEventsView` as a ready-made value.
export type EventsWeekEmptyState =
  | { kind: "pending" }
  | { kind: "error" }
  | { kind: "jump"; match: CompanyEvent; calendarLastSuccessAt: string | null }
  | { kind: "noMatchFilters" }
  | { kind: "neverRefreshed" }
  | { kind: "addEvent" };

export type EventsScreenProps = {
  companies: Company[];
  watchlists: Watchlist[];
  companyEvents: CompanyEvent[];
  companyEventsError: string | null;
  companyEventsLoading: boolean;
  selectedCompanyEventId: string | null;
  sourceRefreshState: string;
  sourceAdapterRefreshInFlight: string | null;
  /** Max `lastSuccessAt` over the calendar adapters (`official_calendar`/
   * `public_calendar`), derived in `AppStateRoot` — `null` when neither has
   * ever succeeded. */
  calendarLastSuccessAt: string | null;
  /** One `list_company_events` read (`mode: "upcoming"`) starting the day
   * after the displayed week, all active filters retained (F4b S3 contract §
   * Events point 4 / decision 4). */
  findNextWeekWithEvents: () => Promise<CompanyEvent | null>;
  openCompanyEventCompanyWorkspace: (companyId: string) => void;
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
  // Confirm or reject a proposed derived calendar event, then reload events (ADR 0036).
  confirmDerivedEvent: (eventId: string, action: "confirm" | "reject") => Promise<void>;
};
