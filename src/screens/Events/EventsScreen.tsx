import {
  ChevronLeft,
  ChevronRight,
  LocateFixed,
  Plus,
  RefreshCw,
  Save,
  X,
} from "lucide-react";
import {
  ActionButton,
  ActionRow,
  ErrorText,
  FilterToolbar,
  PanelHeader,
  SectionHeader,
  SegmentedControl,
  SegmentedControlOption,
  SelectField,
  Skeleton,
  TextField,
} from "../../ui";
import { useEffect, useRef, useState, type RefObject } from "react";
import { useLocale } from "../../shared/locale";
import { useEventsViewModel } from "../../app/state/screenViewModels";
import { EventListView } from "./EventListView";
import { WeekEventsView } from "./WeekEventsView";
import { resolveEventViewMode } from "./eventTypes";
import type { EventsWeekEmptyState, NextWeekLookup } from "./eventTypes";
import { derivePrimary } from "./eventsPrimary";

// U7-D density (ADR 0076 D6): the Events week grid can't collapse purely in CSS —
// week vs list are distinct component trees — so the tier is measured on the
// hosting `pane` size container (.workspace, the same subject the
// `@container pane` rules resolve against) and surfaced to React. Compact = S
// width tier (<420px) OR short height tier (<480px). No ResizeObserver (jsdom,
// some SSR) → not compact, i.e. the stored preference is honored.
function usePaneCompact(ref: RefObject<HTMLElement | null>): boolean {
  const [compact, setCompact] = useState(false);

  useEffect(() => {
    const host = ref.current;
    if (!host || typeof ResizeObserver === "undefined") return;
    const pane = (host.closest(".workspace") as HTMLElement | null) ?? host;
    const measure = () => {
      // Guard against an unmeasured pane (0×0 in jsdom / before first layout):
      // only a real, positive dimension below the tier boundary counts as compact,
      // so the persisted mode is honored until the pane is actually laid out.
      const width = pane.clientWidth;
      const height = pane.clientHeight;
      setCompact((width > 0 && width < 420) || (height > 0 && height < 480));
    };
    measure();
    const observer = new ResizeObserver(measure);
    observer.observe(pane);
    return () => observer.disconnect();
  }, [ref]);

  return compact;
}

export function EventsScreen() {
  const {
  companies,
  watchlists,
  companyEvents,
  companyEventsError,
  companyEventsLoading,
  selectedCompanyEventId,
  sourceRefreshState,
  sourceAdapterRefreshInFlight,
  calendarLastSuccessAt,
  findNextWeekWithEvents,
  openCompanyEventCompanyWorkspace,
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
  companyEventDueLabel,
  companyEventDueClass,
  openExternalUrl,
  confirmDerivedEvent,
  } = useEventsViewModel();
  const { t, text } = useLocale();
  const panelRef = useRef<HTMLElement | null>(null);
  const compact = usePaneCompact(panelRef);
  // Presentation-only override; the persisted `companyEventViewMode` is untouched.
  const effectiveViewMode = resolveEventViewMode(companyEventViewMode, compact);
  const layoutModes = compact ? (["list"] as const) : (["week", "list"] as const);
  const isWeekMode = effectiveViewMode === "week";

  const hasActiveEventFilters =
    companyEventWatchlistFilter !== "all" ||
    companyEventCompanyFilter !== "all" ||
    companyEventTypeFilter !== "all" ||
    companyEventStatusFilter !== "all";

  const weekIsEmpty =
    isWeekMode &&
    companyEventWorkingWeekDays.every((day) => (companyEventsByDate[day.date] ?? []).length === 0) &&
    companyEventWeekendEvents.length === 0;

  // The empty-week jump target (F4b contract § Events point 4 / decision 4;
  // pending/error modeled explicitly per F4b sol R1): one `list_company_events`
  // read, only while the displayed week is genuinely empty. `idle` = week has
  // events, no lookup running. A rejected read lands in `error`, never a false
  // "no match" — `findNextWeekWithEvents` no longer swallows failures.
  const [lookup, setLookup] = useState<NextWeekLookup>({ status: "idle" });

  useEffect(() => {
    if (!weekIsEmpty) {
      setLookup({ status: "idle" });
      return;
    }
    let cancelled = false;
    setLookup({ status: "pending" });
    findNextWeekWithEvents()
      .then((event) => {
        if (!cancelled) setLookup({ status: "match", event });
      })
      .catch(() => {
        if (!cancelled) setLookup({ status: "error" });
      });
    return () => {
      cancelled = true;
    };
    // eslint-disable-next-line react-hooks/exhaustive-deps -- re-check when the empty week or any active filter changes; findNextWeekWithEvents' identity is intentionally excluded
  }, [
    weekIsEmpty,
    companyEventWeekRange.end,
    companyEventWatchlistFilter,
    companyEventCompanyFilter,
    companyEventTypeFilter,
    companyEventStatusFilter,
  ]);

  const weekEmptyState: EventsWeekEmptyState | null = !weekIsEmpty
    ? null
    : lookup.status === "pending"
      ? { kind: "pending" }
      : lookup.status === "error"
        ? { kind: "error" }
        : lookup.status === "match" && lookup.event
          ? { kind: "jump", match: lookup.event, calendarLastSuccessAt }
          : hasActiveEventFilters
            ? { kind: "noMatchFilters" }
            : calendarLastSuccessAt === null
              ? { kind: "neverRefreshed" }
              : { kind: "addEvent" };

  const selectedEvent =
    [...companyEvents, ...Object.values(companyEventsByDate).flat(), ...companyEventWeekendEvents].find(
      (event) => event.id === selectedCompanyEventId,
    ) ?? null;

  const primary = derivePrimary({
    loading: companyEventsLoading,
    error: Boolean(companyEventsError),
    composerOpen: isCompanyEventComposerOpen,
    selectedEventStatus: selectedEvent?.status ?? null,
    weekMode: isWeekMode,
    weekIsEmpty,
    nextWeekLookupPending: lookup.status === "pending",
    nextWeekLookupError: lookup.status === "error",
    hasNextMatch: lookup.status === "match" && Boolean(lookup.event),
    hasActiveFilters: hasActiveEventFilters,
  });

  // The empty-week "addEvent" invitation carries its own primary "Add event"
  // (mirrors AlertRulesSection's "no rules yet" invitation) — the header
  // button goes quiet in that one state so the two never both render filled.
  const addEventPrimaryOnHeader = primary === "addEvent" && weekEmptyState?.kind !== "addEvent";

  function handleRefreshCalendar() {
    refreshEventSources("manual", companyEventWeekRange.start);
  }

  function handleJumpToNextWeek() {
    if (lookup.status === "match" && lookup.event) {
      setCompanyEventWeekAnchorDate(lookup.event.eventDate);
    }
  }

  const refreshing = sourceRefreshState === "refreshing" && sourceAdapterRefreshInFlight === "events";

  return (
    <section className="feed-panel" aria-labelledby="events-title" data-events-compact={compact || undefined} ref={panelRef}>
      <PanelHeader
        paneLead
        title={t("events.title")}
        description={t("events.description")}
        titleId="events-title"
        actions={
          <>
            <ActionButton
              verb="refresh"
              className={refreshing ? "compact-button icon-button-spinning" : "compact-button"}
              disabled={refreshing}
              onClick={handleRefreshCalendar}
            >
              <RefreshCw size={15} />
              {text("Refresh calendar")}
            </ActionButton>
            {isCompanyEventComposerOpen ? null : (
              <ActionButton
                verb="add"
                className="compact-button"
                variant={addEventPrimaryOnHeader ? "primary" : "secondary"}
                data-ux-primary-action={addEventPrimaryOnHeader ? "true" : undefined}
                onClick={openCompanyEventComposer}
              >
                <Plus size={15} />
                {t("action.addEvent")}
              </ActionButton>
            )}
          </>
        }
      />

      <FilterToolbar ariaLabel={text("Event view mode")} className="events-filter-toolbar">
        <SegmentedControl ariaLabel={text("Event layout")}>
          {layoutModes.map((viewMode) => (
            <SegmentedControlOption
              active={effectiveViewMode === viewMode}
              data-action-kind="control"
              key={viewMode}
              onClick={() => setCompanyEventViewMode(viewMode)}
            >
              {viewMode === "week" ? text("Week") : text("List")}
            </SegmentedControlOption>
          ))}
        </SegmentedControl>
        {isWeekMode ? (
          <div className="week-toolbar" aria-label={text("Week navigation")}>
            <ActionButton
              kind="control"
              className="compact-button icon-only-button"
              onClick={() => {
                setCompanyEventWeekAnchorDate((current) =>
                  formatLocalDate(addLocalDays(parseLocalDate(current), -7)),
                );
              }}
              aria-label={text("Previous week")}
            >
              <ChevronLeft size={15} />
            </ActionButton>
            <span>{formatWeekRange(companyEventWeekRange.start, companyEventWeekRange.end)}</span>
            <ActionButton
              kind="control"
              className="compact-button icon-only-button"
              onClick={() => {
                setCompanyEventWeekAnchorDate((current) =>
                  formatLocalDate(addLocalDays(parseLocalDate(current), 7)),
                );
              }}
              aria-label={text("Next week")}
            >
              <ChevronRight size={15} />
            </ActionButton>
            <ActionButton
              kind="control"
              className="compact-button"
              onClick={() => setCompanyEventWeekAnchorDate(formatLocalDate(new Date()))}
            >
              <LocateFixed size={15} />
              {text("Current week")}
            </ActionButton>
          </div>
        ) : (
          <SegmentedControl ariaLabel={text("Event date range")}>
            {(["upcoming", "historical", "all"] as const).map((mode) => (
              <SegmentedControlOption
                active={companyEventMode === mode}
                data-action-kind="control"
                key={mode}
                onClick={() => setCompanyEventMode(mode)}
              >
                {mode === "upcoming" ? text("Upcoming") : mode === "historical" ? text("Past") : text("All")}
              </SegmentedControlOption>
            ))}
          </SegmentedControl>
        )}
        <SelectField
          label={text("Watchlist")}
          aria-label={text("Event watchlist filter")}
          value={companyEventWatchlistFilter}
          onChange={(event) => setCompanyEventWatchlistFilter(event.target.value)}
        >
          <option value="all">{text("All watchlists")}</option>
          {watchlists.map((watchlist) => (
            <option key={watchlist.id} value={watchlist.id}>
              {watchlist.name}
            </option>
          ))}
        </SelectField>
        <SelectField
          label={text("Company")}
          aria-label={text("Event company filter")}
          value={companyEventCompanyFilter}
          onChange={(event) => setCompanyEventCompanyFilter(event.target.value)}
        >
          <option value="all">{text("All companies")}</option>
          {companies.map((company) => (
            <option key={company.id} value={company.id}>
              {company.qualifiedTicker}
            </option>
          ))}
        </SelectField>
        <SelectField
          label={text("Type")}
          aria-label={text("Event type filter")}
          value={companyEventTypeFilter}
          onChange={(event) => setCompanyEventTypeFilter(event.target.value)}
        >
          <option value="all">{text("All types")}</option>
          {companyEventTypes.map((eventType) => (
            <option key={eventType} value={eventType}>
              {formatCompanyEventType(eventType)}
            </option>
          ))}
        </SelectField>
        <SelectField
          label={text("Status")}
          aria-label={text("Event status filter")}
          value={companyEventStatusFilter}
          onChange={(event) => setCompanyEventStatusFilter(event.target.value)}
        >
          <option value="all">{text("All statuses")}</option>
          {companyEventStatuses.map((status) => (
            <option key={status} value={status}>
              {formatCompanyEventStatus(status)}
            </option>
          ))}
        </SelectField>
        {isWeekMode ? null : (
          <>
            <NotebookDateField
              ariaLabel={text("Event date from filter")}
              label={text("From")}
              value={companyEventDateFrom}
              onChange={setCompanyEventDateFrom}
            />
            <NotebookDateField
              ariaLabel={text("Event date to filter")}
              label={text("To")}
              value={companyEventDateTo}
              onChange={setCompanyEventDateTo}
            />
          </>
        )}
        <ActionButton
          kind="control"
          className="compact-button"
          disabled={
            companyEventWatchlistFilter === "all" &&
            companyEventCompanyFilter === "all" &&
            companyEventTypeFilter === "all" &&
            companyEventStatusFilter === "all" &&
            companyEventDateFrom.trim().length === 0 &&
            companyEventDateTo.trim().length === 0
          }
          onClick={clearCompanyEventFilters}
        >
          <X size={15} />
          {text("Clear filters")}
        </ActionButton>
      </FilterToolbar>

      {isCompanyEventComposerOpen ? (
        <div className="event-composer" aria-label={text("Create manual event")}>
          <SectionHeader
            className="event-composer-header"
            title={text("Manual event")}
            description={text("Add a missing date for one tracked company.")}
            actions={
              <ActionButton
                kind="control"
                className="compact-button"
                onClick={() => {
                  setCompanyEventComposerOpen(false);
                  setCompanyEventCreateError(null);
                }}
              >
                <X size={15} />
                {text("Discard")}
              </ActionButton>
            }
          />
          <div className="event-composer-grid">
            <SelectField
              label={text("Company")}
              aria-label={text("Manual event company")}
              value={companyEventForm.companyId}
              onChange={(event) =>
                setCompanyEventForm((current) => ({
                  ...current,
                  companyId: event.target.value,
                }))
              }
            >
              <option value="">{text("Select company")}</option>
              {companies.map((company) => (
                <option key={company.id} value={company.id}>
                  {company.qualifiedTicker}
                </option>
              ))}
            </SelectField>
            <SelectField
              label={text("Type")}
              aria-label={text("Manual event type")}
              value={companyEventForm.eventType}
              onChange={(event) =>
                setCompanyEventForm((current) => ({
                  ...current,
                  eventType: event.target.value,
                }))
              }
            >
              {companyEventTypeOptions.map((eventType) => (
                <option key={eventType} value={eventType}>
                  {formatCompanyEventType(eventType)}
                </option>
              ))}
            </SelectField>
            <SelectField
              label={text("Status")}
              aria-label={text("Manual event status")}
              value={companyEventForm.status}
              onChange={(event) =>
                setCompanyEventForm((current) => ({
                  ...current,
                  status: event.target.value,
                }))
              }
            >
              {companyEventStatusOptions.map((status) => (
                <option key={status} value={status}>
                  {formatCompanyEventStatus(status)}
                </option>
              ))}
            </SelectField>
            <NotebookDateField
              ariaLabel={text("Manual event date")}
              label={text("Date")}
              value={companyEventForm.eventDate}
              onChange={(value) =>
                setCompanyEventForm((current) => ({
                  ...current,
                  eventDate: value,
                }))
              }
            />
            <label>
              {text("Time")}
              <input
                aria-label={text("Manual event time")}
                type="time"
                value={companyEventForm.eventTime}
                onChange={(event) =>
                  setCompanyEventForm((current) => ({
                    ...current,
                    eventTime: event.target.value,
                  }))
                }
              />
            </label>
            <TextField
              className="event-composer-title"
              label={text("Title")}
              aria-label={text("Manual event title")}
              value={companyEventForm.title}
              onChange={(event) =>
                setCompanyEventForm((current) => ({
                  ...current,
                  title: event.target.value,
                }))
              }
            />
          </div>
          <ActionRow className="event-composer-actions">
            {companyEventCreateError ? <ErrorText>{text(companyEventCreateError)}</ErrorText> : null}
            <ActionButton
              verb="save"
              className="compact-button"
              variant={primary === "saveComposer" ? "primary" : "secondary"}
              data-ux-primary-action={primary === "saveComposer" ? "true" : undefined}
              onClick={createCompanyEvent}
            >
              <Save size={15} />
              {text("Save")}
            </ActionButton>
          </ActionRow>
        </div>
      ) : null}

      <div className="events-layout" aria-label={text("Company events")}>
        {companyEventsLoading ? (
          <Skeleton variant="list-row" count={5} label={text("Loading events…")} />
        ) : companyEventsError ? (
          <div className="events-error-strip">
            <ErrorText>{text("Failed to load events")}</ErrorText>
            <ActionButton verb="refresh" onClick={handleRefreshCalendar}>
              {text("Refresh calendar")}
            </ActionButton>
          </div>
        ) : isWeekMode ? (
          <WeekEventsView
            companyEventWorkingWeekDays={companyEventWorkingWeekDays}
            companyEventWeekendDays={companyEventWeekendDays}
            companyEventWeekendEvents={companyEventWeekendEvents}
            companyEventsByDate={companyEventsByDate}
            selectedCompanyEventId={selectedCompanyEventId}
            setSelectedCompanyEventId={setSelectedCompanyEventId}
            companyEventDueLabel={companyEventDueLabel}
            companyEventDueClass={companyEventDueClass}
            openExternalUrl={openExternalUrl}
            openCompanyEventCompanyWorkspace={openCompanyEventCompanyWorkspace}
            confirmDerivedEvent={confirmDerivedEvent}
            primary={primary}
            emptyState={weekEmptyState}
            onJumpToNextWeek={handleJumpToNextWeek}
            onClearFilters={clearCompanyEventFilters}
            onAddEvent={openCompanyEventComposer}
            onRefreshCalendar={handleRefreshCalendar}
          />
        ) : (
          <EventListView
            companyEvents={companyEvents}
            selectedCompanyEventId={selectedCompanyEventId}
            setSelectedCompanyEventId={setSelectedCompanyEventId}
            companyEventDueLabel={companyEventDueLabel}
            companyEventDueClass={companyEventDueClass}
            openExternalUrl={openExternalUrl}
            openCompanyEventCompanyWorkspace={openCompanyEventCompanyWorkspace}
            confirmDerivedEvent={confirmDerivedEvent}
            primary={primary}
            hasActiveFilters={
              hasActiveEventFilters ||
              companyEventDateFrom.trim().length > 0 ||
              companyEventDateTo.trim().length > 0
            }
            onClearFilters={clearCompanyEventFilters}
          />
        )}
      </div>
    </section>
  );
}
