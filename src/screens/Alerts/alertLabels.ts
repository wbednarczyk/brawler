import { BarChart3, Bot, Coins, FileWarning, TrendingDown, Users, type LucideIcon } from "lucide-react";

import type { AlertRule, AttentionEvent } from "../../api/attention";
import { formatSignalCategoryDisplayName } from "../../shared/formatting/labels";

export type ScopeType = AlertRule["scopeType"];
export type TriggerType = AlertRule["triggerType"];
export type IconComponent = LucideIcon;
type Translate = (key: string) => string;

// Preset rule chips (ADR 0068 T3, visual-first per docs/ui-authoring.md): a click
// pre-fills the trigger (and its signal category) so the user only picks a scope.
// The lucide icon rides through to the matching rule row's leading tile so the
// creation choice and the resulting rule read as the same thing.
export type Preset = {
  key: string;
  label: string;
  triggerType: TriggerType;
  signalCategory: string | null;
  icon: IconComponent;
};

export const PRESETS: readonly Preset[] = [
  { key: "profit_warning", label: "Profit warning", triggerType: "signal_category", signalCategory: "profit_warning", icon: TrendingDown },
  { key: "insider", label: "Insider transactions", triggerType: "signal_category", signalCategory: "insider_transaction", icon: Users },
  { key: "auditor_opinion", label: "Auditor opinion", triggerType: "signal_category", signalCategory: "auditor_opinion", icon: FileWarning },
  { key: "short_position", label: "Short position", triggerType: "signal_category", signalCategory: "short_position_change", icon: TrendingDown },
  { key: "recommendation", label: "Analyst recommendation", triggerType: "signal_category", signalCategory: "recommendation_change", icon: FileWarning },
  { key: "week52_low", label: "52-week low", triggerType: "price_week52_low", signalCategory: null, icon: BarChart3 },
  { key: "price_range", label: "Price range", triggerType: "price_enters_range", signalCategory: null, icon: Coins },
  { key: "autopilot", label: "Autopilot finished", triggerType: "autopilot_run_completed", signalCategory: null, icon: Bot },
];

export const RULE_FORMS = { en: ["rule", "rules"], pl: ["reguła", "reguły", "reguł"] } as const;
export const NEW_FORMS = { en: ["new", "new"], pl: ["nowa", "nowe", "nowych"] } as const;

export function parsePrice(value: string): number | null {
  const parsed = Number.parseFloat(value);
  return Number.isFinite(parsed) ? parsed : null;
}

export function priceText(value: number | null): string {
  return value === null ? "?" : String(value);
}

// Leading tile / preset icon for a rule's trigger (mirrors the PRESETS icons so a
// created rule keeps the glyph of the choice that made it).
export function triggerIcon(triggerType: TriggerType, signalCategory: string | null): IconComponent {
  switch (triggerType) {
    case "signal_category":
      return signalCategory === "insider_transaction"
        ? Users
        : signalCategory === "auditor_opinion"
          ? FileWarning
          : TrendingDown; // profit_warning + short_position_change share the glyph
    case "price_enters_range":
      return Coins;
    case "price_week52_low":
      return BarChart3;
    case "autopilot_run_completed":
      return Bot;
    default:
      return TrendingDown;
  }
}

// Short, human title for a rule's trigger (the rule row's bold first line).
export function ruleTitle(rule: AlertRule, text: Translate): string {
  switch (rule.triggerType) {
    case "signal_category":
      // Issue #71 (the D3 raw-enum class): resolve EVERY category through
      // the shared display-name map (mirrors signal_categories.display_name)
      // — a hand-rolled subset leaks raw enum codes for the rest.
      return rule.signalCategory
        ? text(formatSignalCategoryDisplayName(rule.signalCategory))
        : text("Signal");
    case "autopilot_run_completed":
      return text("Autopilot finished");
    case "price_enters_range":
      return text("Price range");
    case "price_week52_low":
      return text("52-week low");
    default:
      // Every AlertRule.triggerType member is handled above; this backstops
      // a future backend addition (fixA finding 4: never a raw enum token).
      return text("Signal");
  }
}

export function triggerLabel(rule: AlertRule, text: Translate): string {
  if (rule.triggerType === "price_enters_range") {
    return `${text("Price range")} ${rule.priceMin ?? "?"}–${rule.priceMax ?? "?"}`;
  }
  return ruleTitle(rule, text);
}

export function scopeName(
  scope: ScopeType,
  ref: string,
  companyName: Map<string, string>,
  watchlistName: Map<string, string>,
): string {
  return scope === "watchlist" ? watchlistName.get(ref) ?? ref : companyName.get(ref) ?? ref;
}

export function ruleDescription(
  rule: AlertRule,
  text: Translate,
  companyName: Map<string, string>,
  watchlistName: Map<string, string>,
): string {
  return `${triggerLabel(rule, text)} · ${scopeName(rule.scopeType, rule.scopeRef, companyName, watchlistName)}`;
}

// Fired-event "what" line, from the trigger type joined onto the event.
// Covers EVERY `AttentionEvent.triggerType` member (fixA finding 4): unlike
// `AlertRule.triggerType`, an event can also be a system trigger raised with
// no user rule (`source_reconciliation`, `job_failed` — ADR 0069/0091), so
// those two need their own cases or they fall through to the raw enum token.
export function eventWhat(event: AttentionEvent, text: Translate): string {
  switch (event.triggerType) {
    case "signal_category":
      return text("Signal");
    case "autopilot_run_completed":
      return text("Autopilot finished");
    case "price_enters_range":
      return text("Price range");
    case "price_week52_low":
      return text("52-week low");
    case "source_reconciliation":
      return text("Reconciliation");
    case "job_failed":
      return text("Background task");
    default:
      return text("Signal");
  }
}

// A system event may carry no company at all (a failed workspace-wide job,
// ADR 0091 dec. 2) — it is scoped to the workspace, not to an issuer.
export function eventScope(event: AttentionEvent, companyName: Map<string, string>, text: Translate): string {
  return (
    (event.companyId ? companyName.get(event.companyId) : null) ?? event.companyId ?? text("System")
  );
}

export function eventDescription(
  event: AttentionEvent,
  text: Translate,
  companyName: Map<string, string>,
): string {
  // `eventWhat` (fixA finding 4), never the raw `evidenceType` enum code.
  return `${eventScope(event, companyName, text)} · ${eventWhat(event, text)}`;
}

// The fired row's destination action name (F4a S4b, contract § Alerts action
// inventory: "the label names the target surface"). Mirrors Today's
// `openAttentionRowAction` routing (`src/screens/Today/TodayScreen.tsx`): a
// missed-report event with its own witness URL opens that report; a
// company-scoped event opens the company; anything else (a workspace-wide
// SYSTEM event) falls back to the Inbox.
export function eventDestinationLabel(event: AttentionEvent, text: Translate): string {
  if (event.triggerType === "source_reconciliation" && event.witnessUrl) return text("Open report");
  if (event.companyId) return text("Open company");
  return text("Open Inbox");
}
