import type { CompanyEvent } from "../../api/types";
import { ActionButton, EmptyState, ErrorText, Figure } from "../../ui";
import { TickerLabel } from "../../shared/components/TickerLabel";
import { useLocale } from "../../shared/locale";
import { formatDetailTimestamp } from "../../shared/format/datetime";
import { EventDetail } from "./EventDetail";
import { eventSourceLine, eventTypeLabel } from "./eventLabels";
import type { EventsPrimaryState } from "./eventsPrimary";
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
  primary: EventsPrimaryState;
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
  primary,
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
            {dueClass !== "event-due-past" && dueLabel ? (
              <span className="event-week-card-due">{dueLabel}</span>
            ) : null}
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
            confirmIsPrimary={primary === "confirmProposed"}
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
          primary={primary}
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
  primary: EventsPrimaryState;
  onJumpToNextWeek: () => void;
  onClearFilters: () => void;
  onAddEvent: () => void;
  onRefreshCalendar: () => void;
};

// The empty-week invitation/quiet state (F4b contract § Events point 4 /
// decision 4): the grid above stays visible (quiet dashed columns), this
// panel renders underneath with the one action the state calls for. Every
// primary-styled action here consults the single `primary` enum (F4b sol
// R1) instead of assuming it whenever this branch renders — a composer
// opened over an empty week still moves the primary to "Zapisz".
function WeekEmptyPanel({
  emptyState,
  locale,
  primary,
  onJumpToNextWeek,
  onClearFilters,
  onAddEvent,
  onRefreshCalendar,
}: WeekEmptyPanelProps) {
  const { text } = useLocale();

  if (emptyState.kind === "pending") {
    return (
      <EmptyState
        kind="quiet"
        className="event-week-empty-panel"
        reason={text("Checking later weeks…")}
      />
    );
  }

  if (emptyState.kind === "error") {
    return (
      <div className="event-week-empty-panel events-error-strip">
        <ErrorText>{text("Failed to check later weeks")}</ErrorText>
        <ActionButton verb="refresh" onClick={onRefreshCalendar}>
          {text("Refresh calendar")}
        </ActionButton>
      </div>
    );
  }

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
    const isPrimary = primary === "jumpNextWeek";
    return (
      <EmptyState
        kind="invitation"
        className="event-week-empty-panel"
        title={text("Nothing this week")}
        source={
          <>
            {emptyState.calendarLastSuccessAt
              ? text("Bankier and GPW calendars · refreshed {time}").replace(
                  "{time}",
                  formatDetailTimestamp(emptyState.calendarLastSuccessAt),
                )
              : text("Bankier and GPW calendars")}
            {" "}
            {text("Next date: {date} — {company}, {type}")
              .replace("{date}", emptyState.match.eventDate)
              .replace("{company}", emptyState.match.companyName)
              .replace("{type}", eventTypeLabel(emptyState.match, text, locale))}
          </>
        }
        action={
          <ActionButton
            kind="control"
            variant={isPrimary ? "primary" : "secondary"}
            data-ux-primary-action={isPrimary ? "true" : undefined}
            onClick={onJumpToNextWeek}
          >
            {text("Show next week with events")}
          </ActionButton>
        }
      />
    );
  }

  // "neverRefreshed" and "addEvent" both fall under the primary enum's
  // `addEvent` bucket (decision 5's table folds them together) — a composer
  // open at the same time still moves `primary` to "saveComposer", so both
  // buttons below correctly go quiet instead of double-filling with Zapisz.
  const isPrimary = primary === "addEvent";

  if (emptyState.kind === "neverRefreshed") {
    return (
      <EmptyState
        kind="invitation"
        className="event-week-empty-panel"
        title={text("Nothing this week")}
        source={text("The calendar has not been refreshed yet.")}
        action={
          <ActionButton
            verb="refresh"
            variant={isPrimary ? "primary" : "secondary"}
            data-ux-primary-action={isPrimary ? "true" : undefined}
            onClick={onRefreshCalendar}
          >
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
        <ActionButton
          verb="add"
          variant={isPrimary ? "primary" : "secondary"}
          data-ux-primary-action={isPrimary ? "true" : undefined}
          onClick={onAddEvent}
        >
          {text("Add event")}
        </ActionButton>
      }
    />
  );
}
