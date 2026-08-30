import type { CompanyEvent } from "../../api/types";
import { ActionButton, EmptyState, Figure } from "../../ui";
import { TickerLabel } from "../../shared/components/TickerLabel";
import { useLocale } from "../../shared/locale";
import { EventDetail } from "./EventDetail";
import { eventSourceLine, eventTypeLabel } from "./eventLabels";
import type { EventsScreenProps, EventsWeekEmptyState } from "./eventTypes";

type WeekEventsViewProps = Pick<
  EventsScreenProps,
  | "companyEventWorkingWeekDays"
  | "companyEventWeekendDays"
  | "companyEventWeekendEvents"
  | "companyEventsByDate"
  | "selectedCompanyEventId"
  | "setSelectedCompanyEventId"
  | "companyEventDueLabel"
  | "companyEventDueClass"
  | "openExternalUrl"
  | "openCompanyEventCompanyWorkspace"
  | "confirmDerivedEvent"
> & {
  emptyState: EventsWeekEmptyState | null;
  onJumpToNextWeek: () => void;
  onClearFilters: () => void;
  onAddEvent: () => void;
  onRefreshCalendar: () => void;
};

export function WeekEventsView({
  companyEventWorkingWeekDays,
  companyEventWeekendDays,
  companyEventWeekendEvents,
  companyEventsByDate,
  selectedCompanyEventId,
  setSelectedCompanyEventId,
  companyEventDueLabel,
  companyEventDueClass,
  openExternalUrl,
  openCompanyEventCompanyWorkspace,
  confirmDerivedEvent,
  emptyState,
  onJumpToNextWeek,
  onClearFilters,
  onAddEvent,
  onRefreshCalendar,
}: WeekEventsViewProps) {
  const { locale, text } = useLocale();

  function toggleEvent(event: CompanyEvent) {
    setSelectedCompanyEventId((current) => (current === event.id ? null : event.id));
  }

  function renderCompanyEventWeekCard(event: CompanyEvent) {
    const isSelected = selectedCompanyEventId === event.id;
    const dueLabel = companyEventDueLabel(event.eventDate);
    const dueClass = companyEventDueClass(event.eventDate);
    const source = eventSourceLine(event, text);

    return (
      <div className="event-week-card-block" key={event.id} data-event-id={event.id}>
        <button
          type="button"
          aria-label={`${text("Open event")}: ${event.title}`}
          aria-pressed={isSelected}
          data-action-kind="control"
          className={[
            "event-week-card",
            event.manual ? "event-manual" : "",
            dueClass,
            isSelected ? "event-week-card-selected" : "",
          ]
            .filter(Boolean)
            .join(" ")}
          onClick={() => toggleEvent(event)}
        >
          <div className="event-week-card-topline">
            <strong><TickerLabel value={event.company} /></strong>
            {dueLabel ? <em>{dueLabel}</em> : null}
          </div>
          <span className="event-week-card-type">{eventTypeLabel(event, text, locale)}</span>
          <span className="event-week-card-company">{event.companyName}</span>
          <span className={source.proposed ? "event-week-card-source event-week-card-source-proposed" : "event-week-card-source"}>
            {source.label}
          </span>
        </button>

        {isSelected ? (
          <EventDetail
            event={event}
            openExternalUrl={openExternalUrl}
            openCompanyEventCompanyWorkspace={openCompanyEventCompanyWorkspace}
            confirmDerivedEvent={confirmDerivedEvent}
          />
        ) : null}
      </div>
    );
  }

  return (
    <>
      {/* The week calendar is DELIBERATE wide content (5 day columns, min
          920px at the M density tier — a `@container pane (min-width: 900px)`
          override drops the minimum at L so the five columns fit inside the
          pane, #431): it scrolls inside this bounded wrapper; data-hscroll
          exempts it from the panel-overflow layout gate (mirrors
          facts-matrix-scroll). tabIndex + role/label make the horizontal
          scroller keyboard-reachable (axe scrollable-region-focusable) even
          when a week has no event cards to tab into. */}
      <div
        className="event-week-scroll"
        data-hscroll
        role="group"
        aria-label={text("Working week events")}
        tabIndex={0}
      >
      <div className="event-week-grid">
        {companyEventWorkingWeekDays.map((day) => {
          const dayEvents = companyEventsByDate[day.date] ?? [];
          const isQuiet = dayEvents.length === 0;

          return (
            <section
              className={isQuiet ? "event-week-day event-week-day-quiet" : "event-week-day"}
              key={day.date}
              aria-label={`${day.label} ${day.date}`}
            >
              <div className="event-week-day-header" data-ux-text-fit>
                <strong>{day.label}</strong>
                <Figure value={day.date} kind="date-short" />
                {dayEvents.length > 0 ? <Figure value={dayEvents.length} kind="count" /> : null}
              </div>
              <div className="event-week-day-body">{dayEvents.map(renderCompanyEventWeekCard)}</div>
            </section>
          );
        })}
        {companyEventWeekendEvents.length > 0 ? (
          <section className="event-weekend-row" aria-label={text("Weekend events")}>
            <div className="event-week-day-header">
              <strong>{text("Weekend")}</strong>
              <span>
                {companyEventWeekendDays[0]?.date} - {companyEventWeekendDays[1]?.date}
              </span>
            </div>
            <div className="event-weekend-list">{companyEventWeekendEvents.map(renderCompanyEventWeekCard)}</div>
          </section>
        ) : null}
      </div>
      </div>

      {emptyState ? (
        <WeekEmptyPanel
          emptyState={emptyState}
          locale={locale}
          onJumpToNextWeek={onJumpToNextWeek}
          onClearFilters={onClearFilters}
          onAddEvent={onAddEvent}
          onRefreshCalendar={onRefreshCalendar}
        />
      ) : null}
    </>
  );
}

type WeekEmptyPanelProps = {
  emptyState: EventsWeekEmptyState;
  locale: Parameters<typeof eventTypeLabel>[2];
  onJumpToNextWeek: () => void;
  onClearFilters: () => void;
  onAddEvent: () => void;
  onRefreshCalendar: () => void;
};

// The empty-week invitation/quiet state (F4b contract § Events point 4 /
// decision 4): the grid above stays visible (quiet dashed columns), this
// panel renders underneath with the one action the state calls for.
function WeekEmptyPanel({
  emptyState,
  locale,
  onJumpToNextWeek,
  onClearFilters,
  onAddEvent,
  onRefreshCalendar,
}: WeekEmptyPanelProps) {
  const { text } = useLocale();

  if (emptyState.kind === "noMatchFilters") {
    return (
      <EmptyState
        kind="quiet"
        className="event-week-empty-panel"
        reason={text("Later there are no events matching the filters")}
        action={
          <ActionButton kind="control" onClick={onClearFilters}>
            {text("Clear filters")}
          </ActionButton>
        }
      />
    );
  }

  if (emptyState.kind === "jump") {
    return (
      <EmptyState
        kind="invitation"
        className="event-week-empty-panel"
        title={text("Nothing this week")}
        source={
          <>
            {text("Bankier and GPW calendars · refreshed {time}").replace(
              "{time}",
              emptyState.calendarLastSuccessAt ?? "",
            )}
            {" "}
            {text("Next date: {date} — {company}, {type}")
              .replace("{date}", emptyState.match.eventDate)
              .replace("{company}", emptyState.match.companyName)
              .replace("{type}", eventTypeLabel(emptyState.match, text, locale))}
          </>
        }
        action={
          <ActionButton kind="control" variant="primary" data-ux-primary-action="true" onClick={onJumpToNextWeek}>
            {text("Show next week with events")}
          </ActionButton>
        }
      />
    );
  }

  if (emptyState.kind === "neverRefreshed") {
    return (
      <EmptyState
        kind="invitation"
        className="event-week-empty-panel"
        title={text("Nothing this week")}
        source={text("The calendar has not been refreshed yet.")}
        action={
          <ActionButton verb="refresh" variant="primary" data-ux-primary-action="true" onClick={onRefreshCalendar}>
            {text("Refresh calendar")}
          </ActionButton>
        }
      />
    );
  }

  return (
    <EmptyState
      kind="invitation"
      className="event-week-empty-panel"
      title={text("Nothing this week")}
      source={text("Bankier and GPW calendars have no later dates for the companies on your lists.")}
      action={
        <ActionButton verb="add" variant="primary" data-ux-primary-action="true" onClick={onAddEvent}>
          {text("Add event")}
        </ActionButton>
      }
    />
  );
}
