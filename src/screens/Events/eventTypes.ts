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

export type EventsScreenProps = {
  companies: Company[];
  watchlists: Watchlist[];
  companyEvents: CompanyEvent[];
  companyEventsError: string | null;
  selectedCompanyEventId: string | null;
  sourceRefreshState: string;
  selectedSourceAdapterId: string | null;
  sourceAdapterRefreshInFlight: string | null;
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
  formatTimestamp: (value: string | null | undefined, emptyLabel?: string) => string;
  formatCompanyEventType: (value: string) => string;
  formatCompanyEventStatus: (value: string) => string;
  formatCompanyEventSourceType: (value: string) => string;
  companyEventDueLabel: (eventDate: string) => string | null;
  companyEventDueClass: (eventDate: string) => string;
  openExternalUrl: (url: string) => void;
  // Confirm or reject a proposed derived calendar event, then reload events (ADR 0036).
  confirmDerivedEvent: (eventId: string, action: "confirm" | "reject") => Promise<void>;
};
