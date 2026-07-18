import type { AlertRule, AttentionEvent } from "../../api/attention";
import { formatSignalCategoryDisplayName } from "../../shared/formatting/labels";

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
    case "source_reconciliation":
      // System event (no user rule): the primary channel missed an official
      // report the ESPI/EBI witness saw (ADR 0069 D2).
      return text("Reconciliation");
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
      // D3 fix (v0.57 fix wave 2, owner screenshot 27-toast-stack.png): the
      // raw category code ("insider_transaction") rendered verbatim in both
      // locales — this must resolve through the SAME category → display-name
      // mapping `text(signal.categoryDisplayName)` uses elsewhere
      // (`formatSignalCategoryDisplayName`, matching the backend's
      // `signal_categories` seed), never a raw enum value. No rule (a
      // system-raised event with nothing to look up) keeps the prior
      // trigger-type fallback.
      return rule?.signalCategory
        ? text(formatSignalCategoryDisplayName(rule.signalCategory))
        : event.triggerType;
    case "autopilot_run_completed":
      return text("Autopilot finished");
    case "price_enters_range":
      return rule
        ? `${rule.priceMin ?? "?"}–${rule.priceMax ?? "?"}`
        : text("Price range");
    case "price_week52_low":
      return text("52-week low");
    case "source_reconciliation":
      return text("Official report missed by the primary source");
    default:
      return event.triggerType;
  }
}
