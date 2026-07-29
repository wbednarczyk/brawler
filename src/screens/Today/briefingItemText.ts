import type { MorningBriefingItem } from "../../api/generated/MorningBriefingItem";
import type { LocaleCode } from "../../shared/locale";
import { formatSignalCategoryDisplayName } from "../../shared/formatting/labels";
import { isTokenizedSummary, renderAutopilotSummaryTokens } from "./autopilotRunSummary";

/**
 * The morning-briefing translation seam (ADR 0087 decision 4). The backend
 * composer writes ONLY verbatim source data or typed codes/tokens into a
 * briefing item's `title`/`detail`; this pure renderer is the single place they
 * become a localized display string. Legacy prose rows (composed before v0.60)
 * pass through verbatim — a tolerant read that never crashes and never shows a
 * raw token when it IS a token (the `isTokenizedSummary` pattern).
 */

type TextFn = (key: string) => string;

// Attention `trigger_type` code → its localized "what fired" label for the
// briefing. No rule/category context exists at this seam, so it is the generic
// per-trigger label (matching `attentionEventBadgeText`). A legacy prose title
// (e.g. "Alert fired: signal_category") is not a trigger code, so it is not in
// this map and passes through verbatim.
const ATTENTION_TRIGGER_LABEL: Record<string, string> = {
  signal_category: "Signal",
  autopilot_run_completed: "Autopilot",
  price_enters_range: "Price range",
  price_week52_low: "52-week low",
  source_reconciliation: "Reconciliation",
  // A company-scoped background job that failed terminally (ADR 0091 dec. 1).
  // Workspace-wide failures carry no company, so the composer skips them — a
  // briefing item always renders under a ticker.
  job_failed: "Background task",
};

function hasText(detail: string | null): detail is string {
  return detail !== null && detail.trim() !== "";
}

/** `title — detail` when a detail is present, else the title alone. */
function joinDetail(title: string, detail: string | null): string {
  return hasText(detail) ? `${title} — ${detail}` : title;
}

/**
 * Render a claim's `detail` due-payload: the typed `due:<period>:<year>` token
 * becomes a localized phrase; a legacy English `Due <period> <year>` prose
 * value passes through verbatim (tolerant read).
 */
function renderDueDetail(detail: string | null, text: TextFn): string | null {
  if (!hasText(detail)) return null;
  const match = /^due:([^:]+):(\d+)$/.exec(detail);
  if (!match) return detail; // legacy prose → verbatim
  return text("Due {period} {year}").replace("{period}", match[1]).replace("{year}", match[2]);
}

/**
 * The localized display string for one composed briefing item. Per `itemType`:
 * - `autopilot_run`: the summary title through the typed-token renderer (legacy
 *   prose verbatim), plus the translated status code.
 * - `attention_event`: the `trigger_type` code as a translated label (legacy
 *   "Alert fired: …" prose verbatim); the raw `signal_category · company_signal`
 *   join is never shown.
 * - `claim_due`: the claim statement plus the translated due token (legacy prose
 *   tolerated).
 * - `signal`: the signal title plus its category code through the shared display
 *   formatter.
 * - `report_date` (and any unknown type): the title plus its detail verbatim.
 */
export function briefingItemText(
  item: MorningBriefingItem,
  text: TextFn,
  locale: LocaleCode,
): string {
  const detail = item.detail ?? null;
  switch (item.itemType) {
    case "autopilot_run": {
      const base = isTokenizedSummary(item.title)
        ? renderAutopilotSummaryTokens(item.title, text, locale)
        : item.title; // legacy prose → verbatim
      const status = hasText(detail) ? text(detail) : "";
      return status ? `${base} · ${status}` : base;
    }
    case "attention_event": {
      const label = ATTENTION_TRIGGER_LABEL[item.title];
      return label ? text(label) : item.title; // legacy prose title → verbatim
    }
    case "claim_due":
      return joinDetail(item.title, renderDueDetail(detail, text));
    case "signal": {
      const category = hasText(detail) ? text(formatSignalCategoryDisplayName(detail)) : null;
      return joinDetail(item.title, category);
    }
    case "report_date":
    default:
      return joinDetail(item.title, detail);
  }
}
