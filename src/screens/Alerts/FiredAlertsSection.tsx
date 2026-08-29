import { useState } from "react";
import { X } from "lucide-react";

import type { AlertRule, AttentionEvent } from "../../api/attention";
import { TickerLabel } from "../../shared/components/TickerLabel";
import { useLocale } from "../../shared/locale";
import { pluralNoun } from "../../shared/locale/plural";
import { ActionButton, EmptyState, ErrorText, Figure, SectionHeader } from "../../ui";
import { NEW_FORMS, eventDescription, eventDestinationLabel, eventScope, eventWhat, triggerLabel } from "./alertLabels";

export type FiredAlertsSectionProps = {
  events: AttentionEvent[];
  rules: AlertRule[];
  companyName: Map<string, string>;
  /** The shared `AttentionController`'s last read failure, if any (ADR 0097 dec. 6). */
  eventsError: string | null;
  onRetry: () => void;
  /** Marks the event seen, then lands on its target surface (F4a S4b: folds
   * the old separate "Review"/mark-seen action into the destination click,
   * mirroring Today's `openAttentionRowAction`). */
  onOpen: (event: AttentionEvent) => void;
  onDismiss: (event: AttentionEvent) => void;
};

// Dense state (contract § Alerts, state matrix "Dense"): a real base can carry
// 100+ fired events; show the newest 50, the rest behind "Show older".
const VISIBLE_CAP = 50;

/**
 * Alerts screen card — fired events (review), reading from the shared
 * `AttentionController` via `useAlertsQuery` (extracted from `AlertsScreen.tsx`,
 * F4a S4a; language pass + dictionary verbs + dense cap, F4a S4b). A failed
 * attention read must never look quiet (ADR 0097 dec. 6): the quiet empty
 * state never renders while `eventsError` is set.
 */
export function FiredAlertsSection({
  events,
  rules,
  companyName,
  eventsError,
  onRetry,
  onOpen,
  onDismiss,
}: FiredAlertsSectionProps) {
  const { text, locale } = useLocale();
  const [showAll, setShowAll] = useState(false);

  const unseenCount = events.filter((event) => !event.seen).length;
  const visibleEvents = showAll ? events : events.slice(0, VISIBLE_CAP);
  const hiddenCount = events.length - visibleEvents.length;

  return (
    <div className="alerts-card">
      <SectionHeader
        className="alerts-card-header"
        level="h2"
        title={text("Fired alerts")}
        meta={
          unseenCount > 0 ? (
            <>
              <Figure value={unseenCount} /> {pluralNoun(locale, unseenCount, NEW_FORMS)}
            </>
          ) : undefined
        }
      />
      {/* A failed attention read must never look quiet (ADR 0097 dec. 6):
          last-known-good events stay listed below the strip. */}
      {eventsError ? (
        <div className="alerts-attention-error">
          <ErrorText>{text("Couldn't load the fired alerts. The rest of the view is up to date.")}</ErrorText>
          <ActionButton kind="control" onClick={onRetry} variant="ghost">
            {text("Try again")}
          </ActionButton>
        </div>
      ) : null}
      {events.length === 0 && eventsError ? null : events.length === 0 ? (
        <EmptyState kind="quiet" reason={text("All quiet — nothing has fired. That's the point.")} />
      ) : (
        <>
          <ul className="alerts-list" aria-label={text("Fired alerts")}>
            {visibleEvents.map((event) => {
              const description = eventDescription(event, text, companyName);
              const ruleForEvent = rules.find((r) => r.id === event.ruleId);
              const ticker = eventScope(event, companyName, text);
              return (
                <li
                  key={event.id}
                  aria-label={`${text("Fired alert")}: ${description}`}
                  className="alerts-row alerts-fired"
                >
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
                      <Figure value={event.firedAt} kind="datetime" />
                      {ruleForEvent ? (
                        <>
                          {" · "}
                          {text("Rule")}: {triggerLabel(ruleForEvent, text)}
                        </>
                      ) : null}
                    </div>
                  </div>
                  <div className="alerts-row-slots">
                    <ActionButton kind="destination" variant="ghost" onClick={() => onOpen(event)}>
                      {eventDestinationLabel(event, text)}
                    </ActionButton>
                    <ActionButton
                      kind="control"
                      aria-label={text("Dismiss")}
                      onClick={() => onDismiss(event)}
                      variant="ghost"
                    >
                      <X size={14} aria-hidden={true} />
                    </ActionButton>
                  </div>
                </li>
              );
            })}
          </ul>
          {hiddenCount > 0 ? (
            <ActionButton kind="control" variant="ghost" onClick={() => setShowAll(true)}>
              {text("Show older")} ({hiddenCount})
            </ActionButton>
          ) : null}
        </>
      )}
    </div>
  );
}
