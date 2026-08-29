import { X } from "lucide-react";

import type { AlertRule, AttentionEvent } from "../../api/attention";
import { TickerLabel } from "../../shared/components/TickerLabel";
import { formatListTimestamp } from "../../shared/format/datetime";
import { useLocale } from "../../shared/locale";
import { pluralNoun } from "../../shared/locale/plural";
import { Button, EmptyState, ErrorText, SectionHeader } from "../../ui";
import { NEW_FORMS, eventDescription, eventScope, eventWhat, triggerLabel } from "./alertLabels";

export type FiredAlertsSectionProps = {
  events: AttentionEvent[];
  rules: AlertRule[];
  companyName: Map<string, string>;
  /** The shared `AttentionController`'s last read failure, if any (ADR 0097 dec. 6). */
  eventsError: string | null;
  onRetry: () => void;
  onDismiss: (event: AttentionEvent) => void;
  onMarkSeen: (event: AttentionEvent) => void;
};

/**
 * Alerts screen card 3 — fired events (review), reading from the shared
 * `AttentionController` via `useAlertsQuery` (extracted from `AlertsScreen.tsx`,
 * F4a S4a). A failed attention read must never look quiet (ADR 0097 dec. 6):
 * last-known-good events stay listed below the error strip.
 */
export function FiredAlertsSection({
  events,
  rules,
  companyName,
  eventsError,
  onRetry,
  onDismiss,
  onMarkSeen,
}: FiredAlertsSectionProps) {
  const { text, locale } = useLocale();

  const unseenCount = events.filter((event) => !event.seen).length;
  const formatFiredAt = (iso: string): string => formatListTimestamp(iso, locale, iso);

  return (
    <div className="alerts-card">
      <SectionHeader
        className="alerts-card-header"
        level="h2"
        title={text("Fired alerts")}
        meta={unseenCount > 0 ? `${unseenCount} ${pluralNoun(locale, unseenCount, NEW_FORMS)}` : undefined}
      />
      {/* A failed attention read must never look quiet (ADR 0097 dec. 6):
          last-known-good events stay listed below the strip. */}
      {eventsError ? (
        <div className="alerts-attention-error">
          <ErrorText>{text("Couldn't load attention events.")}</ErrorText>
          <Button onClick={onRetry} variant="ghost">
            {text("Try again")}
          </Button>
        </div>
      ) : null}
      {events.length === 0 && eventsError ? null : events.length === 0 ? (
        <EmptyState>{text("All quiet — nothing has fired. That's the point.")}</EmptyState>
      ) : (
        <ul className="alerts-list" aria-label={text("Fired alerts")}>
          {events.map((event) => {
            const description = eventDescription(event, text, companyName);
            const ruleForEvent = rules.find((r) => r.id === event.ruleId);
            const ticker = eventScope(event, companyName, text);
            return (
              <li key={event.id} aria-label={`${text("Fired alert")}: ${description}`} className="alerts-row alerts-fired">
                <span
                  aria-hidden="true"
                  className={["alerts-fired-dot", event.seen ? "alerts-fired-dot-seen" : ""]
                    .filter(Boolean)
                    .join(" ")}
                />
                <TickerLabel value={ticker} className="alerts-fired-ticker" />
                <div className="alerts-row-main">
                  <div className="alerts-row-title">{eventWhat(event, text)}</div>
                  <div className="alerts-row-sub alerts-fired-meta">
                    {formatFiredAt(event.firedAt)}
                    {ruleForEvent ? ` · ${text("Rule")}: ${triggerLabel(ruleForEvent, text)}` : ""}
                  </div>
                </div>
                <div className="alerts-row-slots">
                  {event.seen ? null : (
                    <Button onClick={() => onMarkSeen(event)} variant="ghost">
                      {text("Review")}
                    </Button>
                  )}
                  <Button aria-label={text("Dismiss")} onClick={() => onDismiss(event)} variant="ghost">
                    <X size={14} aria-hidden={true} />
                  </Button>
                </div>
              </li>
            );
          })}
        </ul>
      )}
    </div>
  );
}
