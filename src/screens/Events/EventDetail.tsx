import { ExternalLink } from "lucide-react";
import type { CompanyEvent } from "../../api/types";
import { ActionButton, ActionRow, Figure, Hint, InfoGrid } from "../../ui";
import { TickerLabel } from "../../shared/components/TickerLabel";
import { useLocale } from "../../shared/locale";
import { formatCompanyEventSourceType } from "../../shared/formatting/labels";
import { eventStatusLabel, eventTypeLabel } from "./eventLabels";

export type EventDetailProps = {
  event: CompanyEvent;
  openExternalUrl: (url: string) => void;
  openCompanyEventCompanyWorkspace: (companyId: string) => void;
  confirmDerivedEvent: (eventId: string, action: "confirm" | "reject") => Promise<void>;
};

// The event detail panel (F4b S3 contract § Events point 3) — shared by week
// mode (rendered under the grid) and list mode (rendered under the row),
// extracted so both call sites stay in lockstep instead of drifting apart.
export function EventDetail({
  event,
  openExternalUrl,
  openCompanyEventCompanyWorkspace,
  confirmDerivedEvent,
}: EventDetailProps) {
  const { locale, text } = useLocale();
  const isProposed = event.status === "proposed" && event.sourceType === "derived_signal";

  return (
    <div className="event-detail-panel" aria-label={text("Event details")}>
      <div className="event-detail-title">
        <strong>{event.title}</strong>
        <span>
          <Figure value={event.eventDate} kind="date" />
          {event.eventTime ? ` ${event.eventTime}` : ""}
        </span>
      </div>
      <InfoGrid
        className="metadata-grid"
        items={[
          { label: text("Company"), value: <TickerLabel value={event.company} /> },
          { label: text("Type"), value: eventTypeLabel(event, text, locale) },
          { label: text("Status"), value: eventStatusLabel(event.status, text) },
          { label: text("Source"), value: formatCompanyEventSourceType(event.sourceType) },
          { label: text("Fetched"), value: event.fetchedAt ? <Figure value={event.fetchedAt} kind="datetime" /> : text("Not fetched") },
        ]}
      />
      {isProposed ? (
        <div className="derived-event-actions" aria-label={text("Confirm derived event")}>
          <Hint className="derived-event-hint">
            {text(
              "This date comes from a filing. Confirm to add it to the calendar for good, or reject it.",
            )}
          </Hint>
          <ActionRow className="derived-event-buttons">
            <ActionButton verb="reject" onClick={() => confirmDerivedEvent(event.id, "reject")}>
              {text("Reject")}
            </ActionButton>
            <ActionButton
              verb="confirm"
              variant="primary"
              data-ux-primary-action="true"
              onClick={() => confirmDerivedEvent(event.id, "confirm")}
            >
              {text("Confirm")}
            </ActionButton>
          </ActionRow>
        </div>
      ) : null}
      <ActionRow className="event-detail-actions">
        {event.sourceUrl ? (
          <ActionButton verb="open" onClick={() => openExternalUrl(event.sourceUrl as string)}>
            <ExternalLink size={15} />
            {text("Open source")}
          </ActionButton>
        ) : null}
        <ActionButton
          kind="destination"
          onClick={() => openCompanyEventCompanyWorkspace(event.companyId)}
        >
          {text("Open company")}
        </ActionButton>
      </ActionRow>
    </div>
  );
}
