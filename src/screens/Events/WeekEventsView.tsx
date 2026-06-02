import { ExternalLink } from "lucide-react";
import type { CompanyEvent } from "../../api/types";
import { Button } from "../../shared/components/Button";
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
  function toggleEvent(event: CompanyEvent) {
    setSelectedCompanyEventId((current) => (current === event.id ? null : event.id));
  }

  function renderCompanyEventWeekCard(event: CompanyEvent) {
    const isSelected = selectedCompanyEventId === event.id;
    const dueLabel = companyEventDueLabel(event.eventDate);
    const dueClass = companyEventDueClass(event.eventDate);

    return (
      <div className="event-week-card-block" key={event.id}>
        <article
          aria-label={`Open event: ${event.title}`}
          className={[
            "event-week-card",
            event.manual ? "event-manual" : "",
            dueClass,
            isSelected ? "event-week-card-selected" : "",
          ]
            .filter(Boolean)
            .join(" ")}
          onClick={() => toggleEvent(event)}
          onKeyDown={(keyboardEvent) => {
            if (keyboardEvent.key === "Enter" || keyboardEvent.key === " ") {
              keyboardEvent.preventDefault();
              toggleEvent(event);
            }
          }}
          role="button"
          tabIndex={0}
        >
          <div className="event-week-card-topline">
            <strong>{event.company}</strong>
            <div>
              {dueLabel ? <em>{dueLabel}</em> : null}
              <span>{formatCompanyEventStatus(event.status)}</span>
            </div>
          </div>
          <h2>{event.title}</h2>
          <div className="event-week-card-meta">
            <span>{formatCompanyEventType(event.eventType)}</span>
            <span>{event.manual ? "Manual" : formatCompanyEventSourceType(event.sourceType)}</span>
          </div>
        </article>

        {isSelected ? (
          <div className="event-week-card-detail" aria-label="Event details">
            <div className="event-week-card-full-title">
              <span>Title</span>
              <strong>{event.title}</strong>
            </div>
            <dl>
              <div>
                <dt>Company</dt>
                <dd>{event.companyName}</dd>
              </div>
              <div>
                <dt>Date</dt>
                <dd>{event.eventTime ? `${event.eventDate} ${event.eventTime}` : event.eventDate}</dd>
              </div>
              <div>
                <dt>Source</dt>
                <dd>{formatCompanyEventSourceType(event.sourceType)}</dd>
              </div>
              <div>
                <dt>Attribution</dt>
                <dd>{event.attribution ?? "Not set"}</dd>
              </div>
            </dl>
            {event.sourceUrl ? (
              <Button
                className="compact-button"
                onClick={() => openExternalUrl(event.sourceUrl as string)}
              >
                <ExternalLink size={15} />
                Open source
              </Button>
            ) : null}
          </div>
        ) : null}
      </div>
    );
  }

  return (
    <>
      <div className="event-week-grid" aria-label="Working week events">
        {companyEventWorkingWeekDays.map((day) => {
          const dayEvents = companyEventsByDate[day.date] ?? [];

          return (
            <section className="event-week-day" key={day.date} aria-label={`${day.label} ${day.date}`}>
              <div className="event-week-day-header">
                <strong>{day.label}</strong>
                <span>{day.date}</span>
              </div>
              <div className="event-week-day-body">
                {dayEvents.length > 0 ? dayEvents.map(renderCompanyEventWeekCard) : <div className="event-week-empty">No events</div>}
              </div>
            </section>
          );
        })}
        {companyEventWeekendEvents.length > 0 ? (
          <section className="event-weekend-row" aria-label="Weekend events">
            <div className="event-week-day-header">
              <strong>Weekend</strong>
              <span>
                {companyEventWeekendDays[0]?.date} - {companyEventWeekendDays[1]?.date}
              </span>
            </div>
            <div className="event-weekend-list">{companyEventWeekendEvents.map(renderCompanyEventWeekCard)}</div>
          </section>
        ) : null}
      </div>
      {companyEventsError ? <p className="error-text">Events command failed: {companyEventsError}</p> : null}
    </>
  );
}
