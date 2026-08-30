import { useRef, useState, type Dispatch, type SetStateAction } from "react";
import * as eventsApi from "../api/events";
import type { Company, CompanyEvent } from "../api/types";
import { addLocalDays, formatLocalDate, parseLocalDate } from "../shared/format/datetime";
import type { CompanyEventForm, CompanyEventMode, CompanyEventViewMode } from "../shared/types/events";
import { emptyCompanyEventForm } from "./eventForms";

type CompanyEventsControllerInput = {
  companies: Company[];
  companyEventCompanyFilter: string;
  companyEventDateFrom: string;
  companyEventDateTo: string;
  companyEventForm: CompanyEventForm;
  companyEventMode: CompanyEventMode;
  companyEventStatusFilter: string;
  companyEventTypeFilter: string;
  companyEventViewMode: CompanyEventViewMode;
  companyEventWatchlistFilter: string;
  companyEventWeekAnchorDate: string;
  companyEventWeekRange: { start: string; end: string };
  setCompanyEventCompanyFilter: Dispatch<SetStateAction<string>>;
  setCompanyEventComposerOpen: Dispatch<SetStateAction<boolean>>;
  setCompanyEventCreateError: Dispatch<SetStateAction<string | null>>;
  setCompanyEventDateFrom: Dispatch<SetStateAction<string>>;
  setCompanyEventDateTo: Dispatch<SetStateAction<string>>;
  setCompanyEventForm: Dispatch<SetStateAction<CompanyEventForm>>;
  setCompanyEventStatusFilter: Dispatch<SetStateAction<string>>;
  setCompanyEventTypeFilter: Dispatch<SetStateAction<string>>;
  setCompanyEvents: Dispatch<SetStateAction<CompanyEvent[]>>;
  setCompanyEventsError: Dispatch<SetStateAction<string | null>>;
  setCompanyEventWatchlistFilter: Dispatch<SetStateAction<string>>;
  setSelectedCompanyEventId: Dispatch<SetStateAction<string | null>>;
};

export function useCompanyEventsController({
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
}: CompanyEventsControllerInput) {
  const [companyEventsLoading, setCompanyEventsLoading] = useState(false);
  // Request-sequence (last-intent) guard (F4b S3 contract § Events point 7):
  // an older read resolving after a newer week/filter change must not
  // clobber the newer one's result — every call bumps this counter and only
  // applies its outcome if it is still the latest when it resolves.
  const requestSequenceRef = useRef(0);

  function activeFilters() {
    return {
      companyId: companyEventCompanyFilter === "all" ? null : companyEventCompanyFilter,
      watchlistId: companyEventWatchlistFilter === "all" ? null : companyEventWatchlistFilter,
      eventType: companyEventTypeFilter === "all" ? null : companyEventTypeFilter,
      status: companyEventStatusFilter === "all" ? null : companyEventStatusFilter,
    };
  }

  function refreshCompanyEvents(mode: CompanyEventMode = companyEventMode) {
    const isWeekView = companyEventViewMode === "week";
    const sequence = ++requestSequenceRef.current;
    setCompanyEventsLoading(true);

    return eventsApi
      .listCompanyEvents({
        mode: isWeekView ? "all" : mode,
        ...activeFilters(),
        dateFrom: isWeekView ? companyEventWeekRange.start : companyEventDateFrom.trim() || null,
        dateTo: isWeekView ? companyEventWeekRange.end : companyEventDateTo.trim() || null,
      })
      .then((response) => {
        if (sequence !== requestSequenceRef.current) return;
        setCompanyEvents(response);
        setCompanyEventsError(null);
        setCompanyEventsLoading(false);
        setSelectedCompanyEventId((current) => {
          if (current && response.some((event) => event.id === current)) {
            return current;
          }

          return null;
        });
      })
      .catch((error) => {
        if (sequence !== requestSequenceRef.current) return;
        setCompanyEvents([]);
        setCompanyEventsError(String(error));
        setCompanyEventsLoading(false);
      });
  }

  // The empty-week jump target (F4b contract § Events point 4 / decision 4):
  // one read starting the day after the displayed week, every active filter
  // retained, returning the first (soonest) match or `null`.
  function findNextWeekWithEvents(): Promise<CompanyEvent | null> {
    const dateFrom = formatLocalDate(addLocalDays(parseLocalDate(companyEventWeekRange.end), 1));

    return eventsApi
      .listCompanyEvents({
        mode: "upcoming",
        ...activeFilters(),
        dateFrom,
        dateTo: null,
      })
      .then((response) => response[0] ?? null)
      .catch(() => null);
  }

  function clearCompanyEventFilters() {
    setCompanyEventWatchlistFilter("all");
    setCompanyEventCompanyFilter("all");
    setCompanyEventTypeFilter("all");
    setCompanyEventStatusFilter("all");
    setCompanyEventDateFrom("");
    setCompanyEventDateTo("");
    setSelectedCompanyEventId(null);
  }

  function openCompanyEventComposer() {
    setCompanyEventForm({
      ...emptyCompanyEventForm(),
      companyId:
        companyEventCompanyFilter !== "all"
          ? companyEventCompanyFilter
          : companies[0]?.id ?? "",
      eventDate:
        companyEventViewMode === "week" ? companyEventWeekAnchorDate : formatLocalDate(new Date()),
    });
    setCompanyEventCreateError(null);
    setCompanyEventComposerOpen(true);
  }

  function createCompanyEvent() {
    const trimmedTitle = companyEventForm.title.trim();

    if (!companyEventForm.companyId || !trimmedTitle || !companyEventForm.eventDate) {
      setCompanyEventCreateError("Company, title, and date are required.");
      return;
    }

    void eventsApi.createCompanyEvent({
      companyId: companyEventForm.companyId,
      eventType: companyEventForm.eventType,
      title: trimmedTitle,
      eventDate: companyEventForm.eventDate,
      eventTime: companyEventForm.eventTime.trim() || null,
      status: companyEventForm.status,
      sourceType: "manual",
      sourceAdapterId: null,
      sourceEventKey: null,
      sourceUrl: null,
      attribution: "Manual",
      fetchedAt: null,
    })
      .then((createdEvent) => {
        setCompanyEventCreateError(null);
        setCompanyEventComposerOpen(false);
        setCompanyEventForm(emptyCompanyEventForm());
        setSelectedCompanyEventId(createdEvent.id);
        return refreshCompanyEvents();
      })
      .catch((error) => {
        setCompanyEventCreateError(String(error));
      });
  }

  return {
    clearCompanyEventFilters,
    companyEventsLoading,
    createCompanyEvent,
    findNextWeekWithEvents,
    openCompanyEventComposer,
    refreshCompanyEvents,
  };
}
