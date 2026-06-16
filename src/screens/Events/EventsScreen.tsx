import {
  CheckCircle2,
  ChevronLeft,
  ChevronRight,
  LocateFixed,
  Plus,
  RefreshCw,
  Save,
  X,
} from "lucide-react";
import {
  ActionRow,
  Button,
  ErrorText,
  FilterToolbar,
  PanelHeader,
  SectionHeader,
  SegmentedControl,
  SegmentedControlOption,
  SelectField,
  TextField,
} from "../../ui";
import { useLocale } from "../../shared/locale";
import { EventListView } from "./EventListView";
import type { EventsScreenProps } from "./eventTypes";
import { WeekEventsView } from "./WeekEventsView";

export function EventsScreen({
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
  confirmDerivedEvent,
}: EventsScreenProps) {
  const { t, text } = useLocale();

  return (
    <section className="feed-panel" aria-labelledby="events-title">
      <PanelHeader
        title={t("events.title")}
        description={t("events.description")}
        titleId="events-title"
        actions={
          <>
            <Button
              className="compact-button"
              disabled={sourceRefreshState === "refreshing"}
              onClick={() => refreshEventSources("manual", companyEventWeekRange.start)}
            >
              {sourceRefreshState === "done" && selectedSourceAdapterId === "bankier-kalendarium-html" ? (
                <CheckCircle2 size={15} />
              ) : (
                <RefreshCw size={15} />
              )}
              {sourceRefreshState === "refreshing" && sourceAdapterRefreshInFlight === "events"
                ? t("events.action.refreshing")
                : t("events.action.refreshSources")}
            </Button>
            <Button className="compact-button" onClick={openCompanyEventComposer} variant="primary">
              <Plus size={15} />
              {t("action.addEvent")}
            </Button>
          </>
        }
      />

      <FilterToolbar ariaLabel={text("Event view mode")} className="events-filter-toolbar">
        <SegmentedControl ariaLabel={text("Event layout")}>
          {(["week", "list"] as const).map((viewMode) => (
            <SegmentedControlOption
              active={companyEventViewMode === viewMode}
              key={viewMode}
              onClick={() => setCompanyEventViewMode(viewMode)}
            >
              {viewMode === "week" ? text("Week") : text("List")}
            </SegmentedControlOption>
          ))}
        </SegmentedControl>
        {companyEventViewMode === "week" ? (
          <div className="week-toolbar" aria-label={text("Week navigation")}>
            <Button
              className="compact-button icon-only-button"
              onClick={() => {
                setCompanyEventWeekAnchorDate((current) =>
                  formatLocalDate(addLocalDays(parseLocalDate(current), -7)),
                );
              }}
              aria-label={text("Previous week")}
            >
              <ChevronLeft size={15} />
            </Button>
            <span>{formatWeekRange(companyEventWeekRange.start, companyEventWeekRange.end)}</span>
            <Button
              className="compact-button icon-only-button"
              onClick={() => {
                setCompanyEventWeekAnchorDate((current) =>
                  formatLocalDate(addLocalDays(parseLocalDate(current), 7)),
                );
              }}
              aria-label={text("Next week")}
            >
              <ChevronRight size={15} />
            </Button>
            <Button
              className="compact-button"
              onClick={() => setCompanyEventWeekAnchorDate(formatLocalDate(new Date()))}
            >
              <LocateFixed size={15} />
              {text("Current week")}
            </Button>
          </div>
        ) : (
          <SegmentedControl ariaLabel={text("Event date range")}>
            {(["upcoming", "historical", "all"] as const).map((mode) => (
              <SegmentedControlOption
                active={companyEventMode === mode}
                key={mode}
                onClick={() => setCompanyEventMode(mode)}
              >
                {mode === "upcoming" ? text("Upcoming") : mode === "historical" ? text("History") : text("All")}
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
        {companyEventViewMode === "list" ? (
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
        ) : null}
        <Button
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
        </Button>
      </FilterToolbar>

      {isCompanyEventComposerOpen ? (
        <div className="event-composer" aria-label={text("Create manual event")}>
          <SectionHeader
            className="event-composer-header"
            title={text("Manual event")}
            description={text("Add a missing date for one tracked company.")}
            actions={
              <Button
                className="compact-button"
                onClick={() => {
                  setCompanyEventComposerOpen(false);
                  setCompanyEventCreateError(null);
                }}
              >
                <X size={15} />
                {text("Discard")}
              </Button>
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
            <Button className="compact-button" onClick={createCompanyEvent} variant="primary">
              <Save size={15} />
              {text("Save")}
            </Button>
          </ActionRow>
        </div>
      ) : null}

      <div className="events-layout" aria-label={text("Company events")}>
        {companyEventViewMode === "week" ? (
          <WeekEventsView
            companyEventWorkingWeekDays={companyEventWorkingWeekDays}
            companyEventWeekendDays={companyEventWeekendDays}
            companyEventWeekendEvents={companyEventWeekendEvents}
            companyEventsByDate={companyEventsByDate}
            companyEventsError={companyEventsError}
            selectedCompanyEventId={selectedCompanyEventId}
            setSelectedCompanyEventId={setSelectedCompanyEventId}
            formatCompanyEventType={formatCompanyEventType}
            formatCompanyEventStatus={formatCompanyEventStatus}
            formatCompanyEventSourceType={formatCompanyEventSourceType}
            companyEventDueLabel={companyEventDueLabel}
            companyEventDueClass={companyEventDueClass}
            openExternalUrl={openExternalUrl}
          />
        ) : (
          <EventListView
            companyEvents={companyEvents}
            companyEventMode={companyEventMode}
            companyEventsError={companyEventsError}
            selectedCompanyEventId={selectedCompanyEventId}
            setSelectedCompanyEventId={setSelectedCompanyEventId}
            formatTimestamp={formatTimestamp}
            formatCompanyEventType={formatCompanyEventType}
            formatCompanyEventStatus={formatCompanyEventStatus}
            formatCompanyEventSourceType={formatCompanyEventSourceType}
            companyEventDueLabel={companyEventDueLabel}
            companyEventDueClass={companyEventDueClass}
            openExternalUrl={openExternalUrl}
            confirmDerivedEvent={confirmDerivedEvent}
          />
        )}
      </div>
    </section>
  );
}
