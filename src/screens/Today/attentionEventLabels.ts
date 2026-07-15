import type { AlertRule, AttentionEvent } from "../../api/attention";

/**
 * Today attention list + toast wiring (ADR 0068 T4): composes the badge/title
 * text for a fired attention event from its rule's trigger. Pure so the Today
 * row builder and the toast-raising effect render the identical "what fired"
 * sentence — decision support only (facts + the trigger's own category/price
 * context), never phrased as advice. Reuses the exact `text()` keys the
 * Alerts screen's rules manager already translates (`AlertsScreen.tsx`
 * `triggerLabel`) so no new locale strings are needed.
 */

type Translate = (key: string) => string;

/** The category chip label for a fired event's row/toast. */
export function attentionEventBadgeText(event: AttentionEvent, text: Translate): string {
  switch (event.triggerType) {
    case "signal_category":
      return text("Signal");
    case "autopilot_run_completed":
      return text("Autopilot");
    case "price_enters_range":
      return text("Price range");
    case "price_week52_low":
      return text("52-week low");
    default:
      return event.triggerType;
  }
}

/** The "what fired" detail sentence, using the rule's category/price context when known. */
export function attentionEventTitleText(
  event: AttentionEvent,
  rule: AlertRule | undefined,
  text: Translate,
): string {
  switch (event.triggerType) {
    case "signal_category":
      return rule?.signalCategory ?? event.triggerType;
    case "autopilot_run_completed":
      return text("Autopilot finished");
    case "price_enters_range":
      return rule
        ? `${rule.priceMin ?? "?"}–${rule.priceMax ?? "?"}`
        : text("Price range");
    case "price_week52_low":
      return text("52-week low");
    default:
      return event.triggerType;
  }
}
