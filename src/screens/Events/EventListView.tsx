import { CalendarDays } from "lucide-react";
import type { CompanyEvent } from "../../api/types";
import { ActionButton, EmptyState, Figure } from "../../ui";
import { TickerLabel } from "../../shared/components/TickerLabel";
import { useLocale } from "../../shared/locale";
import { EventDetail } from "./EventDetail";
import { eventSourceLine, eventTypeLabel } from "./eventLabels";
import type { EventsScreenProps } from "./eventTypes";

type EventListViewProps = Pick<
  EventsScreenProps,
  | "companyEvents"
  | "selectedCompanyEventId"
  | "setSelectedCompanyEventId"
  | "companyEventDueLabel"
  | "companyEventDueClass"
  | "openExternalUrl"
  | "openCompanyEventCompanyWorkspace"
  | "confirmDerivedEvent"
> & {
  hasActiveFilters: boolean;
  onClearFilters: () => void;
};

export function EventListView({
  companyEvents,
  selectedCompanyEventId,
  setSelectedCompanyEventId,
  companyEventDueLabel,
  companyEventDueClass,
  openExternalUrl,
  openCompanyEventCompanyWorkspace,
  confirmDerivedEvent,
  hasActiveFilters,
  onClearFilters,
}: EventListViewProps) {
  const { locale, text } = useLocale();

  function toggleEvent(event: CompanyEvent) {
    setSelectedCompanyEventId((current) => (current === event.id ? null : event.id));
  }

  return (
    <>
      {companyEvents.map((event) => {
        const dueLabel = companyEventDueLabel(event.eventDate);
        const dueClass = companyEventDueClass(event.eventDate);
        const isSelected = selectedCompanyEventId === event.id;
        const source = eventSourceLine(event, text);

        return (
          <div className="event-row-block" key={event.id} data-event-id={event.id}>
            <button
              type="button"
              aria-label={`${text("Open event")}: ${event.title}`}
              aria-pressed={isSelected}
              data-action-kind="control"
              className={[
                "event-row",
                event.manual ? "event-manual" : "",
                dueClass,
                isSelected ? "event-row-selected" : "",
              ]
                .filter(Boolean)
                .join(" ")}
              onClick={() => toggleEvent(event)}
            >
              <div className="event-date-box">
                <CalendarDays size={16} aria-hidden="true" />
                <Figure value={event.eventDate} kind="date" />
                {event.eventTime ? <span>{event.eventTime}</span> : null}
                {dueLabel ? <em>{dueLabel}</em> : null}
              </div>
              <div className="event-row-main">
                <div className="event-title-line">
                  <h2>{event.title}</h2>
                  <span>{eventTypeLabel(event, text, locale)}</span>
                </div>
                <p>
                  <strong><TickerLabel value={event.company} /></strong> · {event.companyName}
                </p>
              </div>
              <div className="event-row-status">
                <span className={source.proposed ? "event-row-source-proposed" : undefined}>{source.label}</span>
              </div>
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
      })}
      {companyEvents.length === 0 && hasActiveFilters ? (
        <EmptyState
          kind="quiet"
          reason={text("No event matches the filters")}
          action={
            <ActionButton kind="control" onClick={onClearFilters}>
              {text("Clear filters")}
            </ActionButton>
          }
        />
      ) : null}
      {companyEvents.length === 0 && !hasActiveFilters ? (
        <EmptyState kind="quiet" reason={text("No events in this range.")} />
      ) : null}
    </>
  );
}
