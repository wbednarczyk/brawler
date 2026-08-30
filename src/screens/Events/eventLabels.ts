import type { CompanyEvent } from "../../api/types";
import type { LocaleCode } from "../../shared/locale";
import {
  formatCompanyEventSourceType,
  formatCompanyEventStatus,
  formatCompanyEventType,
} from "../../shared/formatting/labels";

type TextFn = (value: string) => string;

const ROMAN_INDEX: Record<string, number> = { I: 1, II: 2, III: 3, IV: 4 };
const QUARTER_ORDINAL = ["Q1", "Q2", "Q3", "Q4"];
const HALF_ORDINAL = ["H1", "H2"];

// Real GPW/Bankier calendar titles carry their period in Polish regardless of
// app locale ("Raport za I półrocze") — this reads it back out rather than
// re-deriving it from data the event doesn't otherwise carry (F4b S3 contract
// § Events point 2).
function parsePeriodicReportPeriod(
  title: string,
): { roman: string; unit: "quarter" | "half" } | "annual" | null {
  if (/za\s+rok\b/i.test(title)) return "annual";
  const match = title.match(/za\s+(I{1,3}|IV)\s+(kwartał|półrocze)/i);
  if (!match) return null;
  return { roman: match[1].toUpperCase(), unit: match[2].toLowerCase() === "półrocze" ? "half" : "quarter" };
}

/**
 * Human event-type label (F4b S3 contract § Events point 2): a locale-aware
 * formatter (never a bare `text()` wrap of an already-composed string, per
 * the F4b dev-vocabulary guardrail) — `periodic_report` reads its period back
 * out of the title, `dividend`/`shareholder_meeting` branch on a title
 * substring, everything else falls back to the existing static dictionary
 * (`formatCompanyEventType`, unchanged — Today/Research still call it raw).
 */
export function eventTypeLabel(event: CompanyEvent, text: TextFn, locale: LocaleCode): string {
  if (event.eventType === "periodic_report") {
    const period = parsePeriodicReportPeriod(event.title);
    if (period === "annual") return text("Annual report");
    if (!period) return text("Periodic report");
    const index = ROMAN_INDEX[period.roman] ?? 1;
    if (locale === "pl") {
      return `Raport za ${period.roman} ${period.unit === "half" ? "półrocze" : "kwartał"}`;
    }
    return period.unit === "half"
      ? `Report for ${HALF_ORDINAL[index - 1] ?? `H${index}`}`
      : `Report for ${QUARTER_ORDINAL[index - 1] ?? `Q${index}`}`;
  }

  if (event.eventType === "dividend") {
    return /bez dywidendy/i.test(event.title) ? text("Ex-dividend day") : text("Dividend");
  }

  if (event.eventType === "shareholder_meeting") {
    return /Nadzwyczajn/i.test(event.title)
      ? text("Extraordinary shareholder meeting")
      : text("Shareholder Meeting");
  }

  return formatCompanyEventType(event.eventType);
}

/**
 * Status label (F4b S3 contract § Events point 3): `proposed` reads
 * "Awaiting confirmation" — never the raw-enum `formatEnumLabel` fallback
 * `formatCompanyEventStatus` otherwise uses.
 */
export function eventStatusLabel(status: string, text: TextFn): string {
  return status === "proposed" ? text("Awaiting confirmation") : formatCompanyEventStatus(status);
}

export type EventSourceLine = { label: string; proposed: boolean };

/**
 * The card/detail source line (F4b S3 contract § Events point 2): a quiet
 * dictionary label over `sourceType` (GPW's own calendar vs. the Bankier
 * public calendar), a `derived_signal` `proposed` event gets the amber
 * "awaiting confirmation" line instead, and a manual entry says so.
 */
export function eventSourceLine(event: CompanyEvent, text: TextFn): EventSourceLine {
  if (event.manual) {
    return { label: text("Manual"), proposed: false };
  }
  if (event.sourceType === "derived_signal" && event.status === "proposed") {
    return { label: text("◔ from a filing · awaiting confirmation"), proposed: true };
  }
  if (event.sourceType === "official_calendar") {
    return { label: "GPW", proposed: false };
  }
  if (event.sourceType === "public_calendar") {
    return { label: text("Bankier · calendar"), proposed: false };
  }
  return { label: formatCompanyEventSourceType(event.sourceType), proposed: false };
}
