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
import { Button } from "../../shared/components/Button";
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
}: EventsScreenProps) {
  return (
    <section className="feed-panel" aria-labelledby="events-title">
      <div className="panel-header">
        <div>
          <h1 id="events-title">Events</h1>
          <p>Company calendar events across tracked companies.</p>
        </div>
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
            ? "Refreshing"
            : "Refresh event sources"}
        </Button>
        <Button className="compact-button" onClick={openCompanyEventComposer} variant="primary">
          <Plus size={15} />
          Add event
        </Button>
      </div>

      <div className="filter-toolbar events-filter-toolbar" aria-label="Event view mode">
        <div className="segmented-control" role="group" aria-label="Event layout">
          {(["week", "list"] as const).map((viewMode) => (
            <button
              className={companyEventViewMode === viewMode ? "segment-active" : ""}
              key={viewMode}
              onClick={() => setCompanyEventViewMode(viewMode)}
              type="button"
            >
              {viewMode === "week" ? "Week" : "List"}
            </button>
          ))}
        </div>
        {companyEventViewMode === "week" ? (
          <div className="week-toolbar" aria-label="Week navigation">
            <Button
              className="compact-button icon-only-button"
              onClick={() => {
                setCompanyEventWeekAnchorDate((current) =>
                  formatLocalDate(addLocalDays(parseLocalDate(current), -7)),
                );
              }}
              aria-label="Previous week"
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
              aria-label="Next week"
            >
              <ChevronRight size={15} />
            </Button>
            <Button
              className="compact-button"
              onClick={() => setCompanyEventWeekAnchorDate(formatLocalDate(new Date()))}
            >
              <LocateFixed size={15} />
              Current week
            </Button>
          </div>
        ) : (
          <div className="segmented-control" role="group" aria-label="Event date range">
            {(["upcoming", "historical", "all"] as const).map((mode) => (
              <button
                className={companyEventMode === mode ? "segment-active" : ""}
                key={mode}
                onClick={() => setCompanyEventMode(mode)}
                type="button"
              >
                {mode === "upcoming" ? "Upcoming" : mode === "historical" ? "History" : "All"}
              </button>
            ))}
          </div>
        )}
        <label>
          Watchlist
          <select
            aria-label="Event watchlist filter"
            value={companyEventWatchlistFilter}
            onChange={(event) => setCompanyEventWatchlistFilter(event.target.value)}
          >
            <option value="all">All watchlists</option>
            {watchlists.map((watchlist) => (
              <option key={watchlist.id} value={watchlist.id}>
                {watchlist.name}
              </option>
            ))}
          </select>
        </label>
        <label>
          Company
          <select
            aria-label="Event company filter"
            value={companyEventCompanyFilter}
            onChange={(event) => setCompanyEventCompanyFilter(event.target.value)}
          >
            <option value="all">All companies</option>
            {companies.map((company) => (
              <option key={company.id} value={company.id}>
                {company.qualifiedTicker}
              </option>
            ))}
          </select>
        </label>
        <label>
          Type
          <select
            aria-label="Event type filter"
            value={companyEventTypeFilter}
            onChange={(event) => setCompanyEventTypeFilter(event.target.value)}
          >
            <option value="all">All types</option>
            {companyEventTypes.map((eventType) => (
              <option key={eventType} value={eventType}>
                {formatCompanyEventType(eventType)}
              </option>
            ))}
          </select>
        </label>
        <label>
          Status
          <select
            aria-label="Event status filter"
            value={companyEventStatusFilter}
            onChange={(event) => setCompanyEventStatusFilter(event.target.value)}
          >
            <option value="all">All statuses</option>
            {companyEventStatuses.map((status) => (
              <option key={status} value={status}>
                {formatCompanyEventStatus(status)}
              </option>
            ))}
          </select>
        </label>
        {companyEventViewMode === "list" ? (
          <>
            <NotebookDateField
              ariaLabel="Event date from filter"
              label="From"
              value={companyEventDateFrom}
              onChange={setCompanyEventDateFrom}
            />
            <NotebookDateField
              ariaLabel="Event date to filter"
              label="To"
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
          Clear filters
        </Button>
      </div>

      {isCompanyEventComposerOpen ? (
        <div className="event-composer" aria-label="Create manual event">
          <div className="event-composer-header">
            <div>
              <h2>Manual event</h2>
              <p>Add a missing date for one tracked company.</p>
            </div>
            <Button
              className="compact-button"
              onClick={() => {
                setCompanyEventComposerOpen(false);
                setCompanyEventCreateError(null);
              }}
            >
              <X size={15} />
              Discard
            </Button>
          </div>
          <div className="event-composer-grid">
            <label>
              Company
              <select
                aria-label="Manual event company"
                value={companyEventForm.companyId}
                onChange={(event) =>
                  setCompanyEventForm((current) => ({
                    ...current,
                    companyId: event.target.value,
                  }))
                }
              >
                <option value="">Select company</option>
                {companies.map((company) => (
                  <option key={company.id} value={company.id}>
                    {company.qualifiedTicker}
                  </option>
                ))}
              </select>
            </label>
            <label>
              Type
              <select
                aria-label="Manual event type"
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
              </select>
            </label>
            <label>
              Status
              <select
                aria-label="Manual event status"
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
              </select>
            </label>
            <NotebookDateField
              ariaLabel="Manual event date"
              label="Date"
              value={companyEventForm.eventDate}
              onChange={(value) =>
                setCompanyEventForm((current) => ({
                  ...current,
                  eventDate: value,
                }))
              }
            />
            <label>
              Time
              <input
                aria-label="Manual event time"
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
            <label className="event-composer-title">
              Title
              <input
                aria-label="Manual event title"
                value={companyEventForm.title}
                onChange={(event) =>
                  setCompanyEventForm((current) => ({
                    ...current,
                    title: event.target.value,
                  }))
                }
              />
            </label>
          </div>
          <div className="event-composer-actions">
            {companyEventCreateError ? <p className="error-text">{companyEventCreateError}</p> : null}
            <Button className="compact-button" onClick={createCompanyEvent} variant="primary">
              <Save size={15} />
              Save
            </Button>
          </div>
        </div>
      ) : null}

      <div className="events-layout" aria-label="Company events">
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
          />
        )}
      </div>
    </section>
  );
}
