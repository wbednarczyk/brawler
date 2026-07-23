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

/** Localized label for a run's raw status (the autopilot event's statement detail). */
function autopilotStatusLabel(status: string | null, text: Translate): string {
  switch (status) {
    case "succeeded":
      return text("Succeeded");
    case "failed":
      return text("Failed");
    case "partial":
      return text("Partial");
    case "running":
      return text("Running");
    case "pending":
      return text("Pending");
    default:
      return text("Finished");
  }
}

/**
 * The "what happened" statement for a fired attention event — the CONCRETE event,
 * never a bare category (v0.60 D6, owner dogfooding 2026-07-23). Each event now
 * carries its evidence specifics from the backend join (`evidenceTitle` /
 * `evidenceDetail`, raw source data); this composes the localized sentence
 * client-side (ADR 0087 dec. 4 / ui-authoring §6 — the backend never bakes
 * prose). A missing `evidenceTitle` (legacy row / pruned evidence) falls back to
 * the prior generic copy so a row never crashes or blanks. Decision support only.
 */
export function attentionEventTitleText(
  event: AttentionEvent,
  rule: AlertRule | undefined,
  text: Translate,
): string {
  switch (event.triggerType) {
    case "signal_category":
      // The signal's own filing title IS the statement (e.g. "Wstępne wyniki
      // produkcyjne i sprzedażowe za czerwiec 2026") — the category stays on the
      // badge. Raw source data, shown verbatim. Fallback (no title): the D3-fix
      // category display name via the rule, or the trigger type when even that is
      // unknown (a system-raised event with nothing to look up).
      if (event.evidenceTitle) return event.evidenceTitle;
      return rule?.signalCategory
        ? text(formatSignalCategoryDisplayName(rule.signalCategory))
        : event.triggerType;
    case "autopilot_run_completed":
      // Which report the autopilot finished, and how it ended (owner: "Autopilot
      // zakończony → co to oznacza?"). Title is raw source data; the status is
      // translated. Fallback keeps the prior generic copy.
      if (event.evidenceTitle) {
        return `${event.evidenceTitle} — ${autopilotStatusLabel(event.evidenceDetail, text)}`;
      }
      return text("Autopilot finished");
    case "price_enters_range":
      return rule
        ? `${rule.priceMin ?? "?"}–${rule.priceMax ?? "?"}`
        : text("Price range");
    case "price_week52_low":
      return text("52-week low");
    case "source_reconciliation":
      // The missed report + the registry that caught it (owner: "jaki dokładniej
      // raport? które to źródło główne?"). The primary channel missed it; the
      // GPW ESPI/EBI witness (`evidenceDetail`) backfilled it from the registry.
      if (event.evidenceTitle) {
        const source = event.evidenceDetail ?? text("the official registry");
        return text("{title} — missed by the primary source, backfilled from {source}")
          .replace("{title}", event.evidenceTitle)
          .replace("{source}", source);
      }
      return text("Official report missed by the primary source");
    default:
      return event.triggerType;
  }
}
