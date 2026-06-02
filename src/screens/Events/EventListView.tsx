import { CalendarDays, ExternalLink } from "lucide-react";
import type { CompanyEvent } from "../../api/types";
import { Button } from "../../shared/components/Button";
import { EmptyState } from "../../shared/components/EmptyState";
import { useLocale } from "../../shared/locale";
import type { EventsScreenProps } from "./eventTypes";

type EventListViewProps = Pick<
  EventsScreenProps,
  | "companyEvents"
  | "companyEventMode"
  | "companyEventsError"
  | "selectedCompanyEventId"
  | "setSelectedCompanyEventId"
  | "formatTimestamp"
  | "formatCompanyEventType"
  | "formatCompanyEventStatus"
  | "formatCompanyEventSourceType"
  | "companyEventDueLabel"
  | "companyEventDueClass"
  | "openExternalUrl"
>;

export function EventListView({
  companyEvents,
  companyEventMode,
  companyEventsError,
  selectedCompanyEventId,
  setSelectedCompanyEventId,
  formatTimestamp,
  formatCompanyEventType,
  formatCompanyEventStatus,
  formatCompanyEventSourceType,
  companyEventDueLabel,
  companyEventDueClass,
  openExternalUrl,
}: EventListViewProps) {
  const { text } = useLocale();

  function toggleEvent(event: CompanyEvent) {
    setSelectedCompanyEventId((current) => (current === event.id ? null : event.id));
  }

  return (
    <>
      {companyEvents.map((event) => {
        const dueLabel = companyEventDueLabel(event.eventDate);
        const dueClass = companyEventDueClass(event.eventDate);
        const isSelected = selectedCompanyEventId === event.id;

        return (
          <div className="event-row-block" key={event.id}>
            <article
              aria-label={`${text("Open event")}: ${event.title}`}
              className={[
                "event-row",
                event.manual ? "event-manual" : "",
                dueClass,
                isSelected ? "event-row-selected" : "",
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
              <div className="event-date-box">
                <CalendarDays size={16} aria-hidden="true" />
                <strong>{event.eventDate}</strong>
                {event.eventTime ? <span>{event.eventTime}</span> : null}
                {dueLabel ? <em>{dueLabel}</em> : null}
              </div>
              <div className="event-row-main">
                <div className="event-title-line">
                  <h2>{event.title}</h2>
                  <span>{formatCompanyEventType(event.eventType)}</span>
                </div>
                <p>
                  <strong>{event.company}</strong> · {event.companyName}
                </p>
              </div>
              <div className="event-row-status">
                <span>{formatCompanyEventStatus(event.status)}</span>
                <small>{event.manual ? text("Manual") : formatCompanyEventSourceType(event.sourceType)}</small>
              </div>
            </article>

            {isSelected ? (
              <div className="event-detail-panel" aria-label={text("Event details")}>
                <dl className="metadata-grid">
                  <div>
                    <dt>{text("Company")}</dt>
                    <dd>{event.company}</dd>
                  </div>
                  <div>
                    <dt>{text("Type")}</dt>
                    <dd>{formatCompanyEventType(event.eventType)}</dd>
                  </div>
                  <div>
                    <dt>{text("Status")}</dt>
                    <dd>{formatCompanyEventStatus(event.status)}</dd>
                  </div>
                  <div>
                    <dt>{text("Source")}</dt>
                    <dd>{formatCompanyEventSourceType(event.sourceType)}</dd>
                  </div>
                  <div>
                    <dt>{text("Attribution")}</dt>
                    <dd>{event.attribution ?? text("Not set")}</dd>
                  </div>
                  <div>
                    <dt>{text("Fetched")}</dt>
                    <dd>{formatTimestamp(event.fetchedAt, text("Not fetched"))}</dd>
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
      })}
      {companyEvents.length === 0 ? (
        <EmptyState>{companyEventMode === "upcoming" ? text("No upcoming events.") : text("No events.")}</EmptyState>
      ) : null}
      {companyEventsError ? <p className="error-text">{text("Events command failed")}: {companyEventsError}</p> : null}
    </>
  );
}
