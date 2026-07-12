import { ExternalLink } from "lucide-react";
import type { CompanyEvent } from "../../api/types";
import { Button, ErrorText } from "../../ui";
import { TickerLabel } from "../../shared/components/TickerLabel";
import { useLocale } from "../../shared/locale";
import type { EventsScreenProps } from "./eventTypes";

type WeekEventsViewProps = Pick<
  EventsScreenProps,
  | "companyEventWorkingWeekDays"
  | "companyEventWeekendDays"
  | "companyEventWeekendEvents"
  | "companyEventsByDate"
  | "companyEventsError"
  | "selectedCompanyEventId"
  | "setSelectedCompanyEventId"
  | "formatCompanyEventType"
  | "formatCompanyEventStatus"
  | "formatCompanyEventSourceType"
  | "companyEventDueLabel"
  | "companyEventDueClass"
  | "openExternalUrl"
>;

export function WeekEventsView({
  companyEventWorkingWeekDays,
  companyEventWeekendDays,
  companyEventWeekendEvents,
  companyEventsByDate,
  companyEventsError,
  selectedCompanyEventId,
  setSelectedCompanyEventId,
  formatCompanyEventType,
  formatCompanyEventStatus,
  formatCompanyEventSourceType,
  companyEventDueLabel,
  companyEventDueClass,
  openExternalUrl,
}: WeekEventsViewProps) {
  const { text } = useLocale();

  function toggleEvent(event: CompanyEvent) {
    setSelectedCompanyEventId((current) => (current === event.id ? null : event.id));
  }

  function renderCompanyEventWeekCard(event: CompanyEvent) {
    const isSelected = selectedCompanyEventId === event.id;
    const dueLabel = companyEventDueLabel(event.eventDate);
    const dueClass = companyEventDueClass(event.eventDate);

    return (
      <div className="event-week-card-block" key={event.id} data-event-id={event.id}>
        <button
          type="button"
          aria-label={`${text("Open event")}: ${event.title}`}
          aria-pressed={isSelected}
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
            <div>
              {dueLabel ? <em>{dueLabel}</em> : null}
              <span>{formatCompanyEventStatus(event.status)}</span>
            </div>
          </div>
          <h2>{event.title}</h2>
          <div className="event-week-card-meta">
            <span>{formatCompanyEventType(event.eventType)}</span>
            <span>{event.manual ? text("Manual") : formatCompanyEventSourceType(event.sourceType)}</span>
          </div>
        </button>

        {isSelected ? (
          <div className="event-week-card-detail" aria-label={text("Event details")}>
            <div className="event-week-card-full-title">
              <span>{text("Title")}</span>
              <strong>{event.title}</strong>
            </div>
            <dl>
              <div>
                <dt>{text("Company")}</dt>
                <dd><TickerLabel value={event.company} /> · {event.companyName}</dd>
              </div>
              <div>
                <dt>{text("Date")}</dt>
                <dd>{event.eventTime ? `${event.eventDate} ${event.eventTime}` : event.eventDate}</dd>
              </div>
              <div>
                <dt>{text("Source")}</dt>
                <dd>{formatCompanyEventSourceType(event.sourceType)}</dd>
              </div>
              <div>
                <dt>{text("Attribution")}</dt>
                <dd>{event.attribution ?? text("Not set")}</dd>
              </div>
            </dl>
            {event.sourceUrl ? (
              <Button
                className="compact-button"
                onClick={() => openExternalUrl(event.sourceUrl as string)}
              >
                <ExternalLink size={15} />
                {text("Open source")}
              </Button>
            ) : null}
          </div>
        ) : null}
      </div>
    );
  }

  return (
    <>
      {/* The week calendar is DELIBERATE wide content (5 day columns, min
          920px): it scrolls inside this bounded wrapper; data-hscroll exempts
          it from the panel-overflow layout gate (mirrors facts-matrix-scroll).
          tabIndex + role/label make the horizontal scroller keyboard-reachable
          (axe scrollable-region-focusable) even when a week has no event cards
          to tab into. */}
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

          return (
            <section className="event-week-day" key={day.date} aria-label={`${day.label} ${day.date}`}>
              <div className="event-week-day-header">
                <strong>{day.label}</strong>
                <span>{day.date}</span>
              </div>
              <div className="event-week-day-body">
                {dayEvents.length > 0 ? dayEvents.map(renderCompanyEventWeekCard) : <div className="event-week-empty">{text("No events.")}</div>}
              </div>
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
      {companyEventsError ? <ErrorText>{text("Events command failed")}: {companyEventsError}</ErrorText> : null}
    </>
  );
}
