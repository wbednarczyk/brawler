import type { LocaleCode } from "../locale";

// The single date/time formatting layer (ADR 0076 Decision 4). Replaces the two
// competing formatters (`shared/format/timestamp` + `shared/formatting/date`)
// whose split brain produced the dual-format and "oday 09:12" bugs. Locale data
// lives in-module keyed by LocaleCode — the same seam `financialValue.ts` uses —
// so these stay pure, unit-testable functions with no React dependency.

const MS_PER_DAY = 24 * 60 * 60 * 1000;

// Genitive-style month abbreviations (as they read inside a date: "2 lip").
const MONTHS: Record<LocaleCode, readonly string[]> = {
  en: ["Jan", "Feb", "Mar", "Apr", "May", "Jun", "Jul", "Aug", "Sep", "Oct", "Nov", "Dec"],
  pl: ["sty", "lut", "mar", "kwi", "maj", "cze", "lip", "sie", "wrz", "paź", "lis", "gru"],
};

// Weekday abbreviations indexed by Date.getDay() (0 = Sunday).
const WEEKDAYS: Record<LocaleCode, readonly string[]> = {
  en: ["Sun", "Mon", "Tue", "Wed", "Thu", "Fri", "Sat"],
  pl: ["ndz", "pon", "wt", "śr", "czw", "pt", "sob"],
};

const RELATIVE: Record<LocaleCode, { today: string; yesterday: string }> = {
  en: { today: "today", yesterday: "yesterday" },
  pl: { today: "dziś", yesterday: "wczoraj" },
};

function months(locale: LocaleCode) {
  return MONTHS[locale] ?? MONTHS.en;
}
function weekdays(locale: LocaleCode) {
  return WEEKDAYS[locale] ?? WEEKDAYS.en;
}
function relativeWords(locale: LocaleCode) {
  return RELATIVE[locale] ?? RELATIVE.en;
}

// Accepts YYYY-MM-DD, optionally with a T/space time (HH:MM[:SS[.ms]]) and an
// optional Z / ±HH:MM offset. Anything else is "not ISO" and rendered verbatim.
// The stated wall-clock is taken as-is (the offset is ignored, not converted):
// report/feed times display as published, tz-independent and deterministic —
// matching the removed detail formatter and avoiding machine-tz drift.
const ISO_RE =
  /^(\d{4})-(\d{2})-(\d{2})(?:[T ](\d{2}):(\d{2})(?::\d{2})?(?:\.\d+)?(?:Z|[+-]\d{2}:\d{2})?)?$/;

const pad = (n: number) => String(n).padStart(2, "0");
const hhmm = (date: Date) => `${pad(date.getHours())}:${pad(date.getMinutes())}`;

function parseIso(trimmed: string): Date | null {
  const match = ISO_RE.exec(trimmed);
  if (!match) return null;
  const [, year, month, day, hour, minute] = match;
  const date = new Date(
    Number(year),
    Number(month) - 1,
    Number(day),
    hour ? Number(hour) : 0,
    minute ? Number(minute) : 0,
  );
  return Number.isNaN(date.getTime()) ? null : date;
}

function startOfDayMs(date: Date): number {
  return new Date(date.getFullYear(), date.getMonth(), date.getDate()).getTime();
}

/**
 * List-context timestamp (feed rows, "what changed" stream) per ADR 0076 D4:
 * relative for recent times, calendar for older, seconds never shown. A
 * domain-date-only value (bare `YYYY-MM-DD`, or an explicit `00:00`) renders as
 * the date alone — never a misleading "… 00:00" (ADR 0087 amendment, 2026-07-23
 * live-checkpoint). Non-ISO inputs (already-friendly sample labels) render
 * verbatim — never string-surgery.
 */
export function formatListTimestamp(
  value: string | null | undefined,
  locale: LocaleCode = "en",
  emptyLabel = "",
  now: Date = new Date(),
): string {
  const trimmed = value?.trim();
  if (!trimmed) return emptyLabel;

  const date = parseIso(trimmed);
  if (!date) return trimmed;

  const dayDiff = Math.round((startOfDayMs(now) - startOfDayMs(date)) / MS_PER_DAY);
  const rel = relativeWords(locale);

  // A domain-date-only event (a bare YYYY-MM-DD, or an explicit 00:00) carries no
  // meaningful time: render the date alone rather than a misleading "… 00:00".
  const atMidnight = date.getHours() === 0 && date.getMinutes() === 0;
  const withTime = (label: string) => (atMidnight ? label : `${label} ${hhmm(date)}`);

  if (dayDiff === 0) return withTime(rel.today);
  if (dayDiff === 1) return withTime(rel.yesterday);
  if (dayDiff >= 2 && dayDiff < 7) return withTime(weekdays(locale)[date.getDay()]);

  const month = months(locale)[date.getMonth()];
  const day = date.getDate();

  if (date.getFullYear() === now.getFullYear()) {
    const base = locale === "pl" ? `${day} ${month}` : `${month} ${day}`;
    return atMidnight ? base : `${base}, ${hhmm(date)}`;
  }
  return locale === "pl"
    ? `${day} ${month} ${date.getFullYear()}`
    : `${month} ${day}, ${date.getFullYear()}`;
}

/**
 * Detail/audit-context timestamp (provenance, diagnostics): full local
 * `YYYY-MM-DD HH:MM`, no seconds. Locale-independent. Non-ISO inputs render
 * verbatim — this is the constructive fix for the "oday 09:12" bug (the removed
 * formatter did `replace("T", " ")` on labels that were never ISO).
 */
export function formatDetailTimestamp(
  value: string | null | undefined,
  emptyLabel = "Not set",
): string {
  const trimmed = value?.trim();
  if (!trimmed) return emptyLabel;

  const date = parseIso(trimmed);
  if (!date) return trimmed;

  return `${date.getFullYear()}-${pad(date.getMonth() + 1)}-${pad(date.getDate())} ${hhmm(date)}`;
}

/**
 * A wall-clock ISO date (`2026-06-18T08:40:00`, no timezone) → `DD.MM.YYYY`,
 * read TEXTUALLY from the date fields so a Warsaw-local timestamp is never
 * shifted through UTC (v0.58 analyst recommendations — `published_at` is Warsaw
 * wall-clock with no `Z`). Non-date input returns its trimmed self.
 */
export function formatLocalIsoDate(value: string | null | undefined): string {
  const trimmed = value?.trim() ?? "";
  const [year, month, day] = trimmed.slice(0, 10).split("-");
  return year && month && day ? `${day}.${month}.${year}` : trimmed;
}

/**
 * Same reading as `formatLocalIsoDate` but without the year (`DD.MM`) — the
 * Events week-grid day header (F4b S3, #431/#417): five narrow columns have
 * no room for a full date, and the year is redundant inside "this week".
 */
export function formatShortLocalDate(value: string | null | undefined): string {
  const trimmed = value?.trim() ?? "";
  const [, month, day] = trimmed.slice(0, 10).split("-");
  return month && day ? `${day}.${month}` : trimmed;
}

/** Localized weekday-range label for the Dziś v2 "Wcześniej" rollup (F2 S5,
 * Delta.dc.html "pn–wt"): one weekday abbreviation, or two joined by an en
 * dash. Safe within the read model's 7-day fetch window (`get_today_view`
 * clamps `dayLimit` to 1..7) — the whole range always falls inside the
 * current weekday cycle, so a bare abbreviation (never month/year) reads
 * unambiguously. */
export function formatDayRangeLabel(oldestDay: string, newestDay: string, locale: LocaleCode = "en"): string {
  const label = (day: string) => weekdays(locale)[parseLocalDate(day).getDay()];
  return oldestDay === newestDay ? label(oldestDay) : `${label(oldestDay)}–${label(newestDay)}`;
}

/** Localized "D mon – D mon" week label (ADR 0076 D4: "30 cze – 4 lip"). */
export function formatWeekRange(startDate: string, endDate: string, locale: LocaleCode = "en"): string {
  const month = months(locale);
  const label = (date: Date) =>
    locale === "pl"
      ? `${date.getDate()} ${month[date.getMonth()]}`
      : `${month[date.getMonth()]} ${date.getDate()}`;
  return `${label(parseLocalDate(startDate))} – ${label(parseLocalDate(endDate))}`;
}

// --- Local-date arithmetic + duration helpers (moved verbatim from the removed
// `shared/formatting/date` — this module is their single home now). ---

export function formatLocalDate(date: Date): string {
  return `${date.getFullYear()}-${pad(date.getMonth() + 1)}-${pad(date.getDate())}`;
}

export function parseLocalDate(value: string): Date {
  const [year, month, day] = value.split("-").map(Number);
  if (!year || !month || !day) {
    return new Date();
  }
  return new Date(year, month - 1, day);
}

export function addLocalDays(date: Date, days: number): Date {
  const nextDate = new Date(date);
  nextDate.setDate(nextDate.getDate() + days);
  return nextDate;
}

function daysBetweenLocalDates(fromDate: string, toDate: string): number {
  const from = parseLocalDate(fromDate).getTime();
  const to = parseLocalDate(toDate).getTime();
  return Math.round((to - from) / MS_PER_DAY);
}

export function startOfIsoWeek(date: Date): Date {
  const startDate = new Date(date);
  const day = startDate.getDay();
  const mondayOffset = day === 0 ? -6 : 1 - day;
  startDate.setDate(startDate.getDate() + mondayOffset);
  return startDate;
}

const DUE_WORDS: Record<
  LocaleCode,
  { past: string; today: string; tomorrow: string; inDays: (n: number) => string }
> = {
  en: { past: "Past", today: "Today", tomorrow: "Tomorrow", inDays: (n) => `In ${n} days` },
  pl: { past: "Po terminie", today: "Dziś", tomorrow: "Jutro", inDays: (n) => `Za ${n} dni` },
};

export function companyEventDueLabel(eventDate: string, locale: LocaleCode = "en"): string | null {
  const words = DUE_WORDS[locale] ?? DUE_WORDS.en;
  const daysUntil = daysBetweenLocalDates(formatLocalDate(new Date()), eventDate);
  if (daysUntil < 0) return words.past;
  if (daysUntil === 0) return words.today;
  if (daysUntil === 1) return words.tomorrow;
  if (daysUntil <= 3) return words.inDays(daysUntil);
  return null;
}

export function companyEventDueClass(eventDate: string): string {
  const daysUntil = daysBetweenLocalDates(formatLocalDate(new Date()), eventDate);
  if (daysUntil < 0) return "event-due-past";
  if (daysUntil === 0) return "event-due-today";
  if (daysUntil <= 3) return "event-due-soon";
  return "";
}

export function formatPollInterval(seconds: number): string {
  if (seconds >= 86400 && seconds % 86400 === 0) {
    const days = seconds / 86400;
    return days === 1 ? "1 day" : `${days} days`;
  }

  if (seconds >= 86400) {
    const days = Math.floor(seconds / 86400);
    const hours = Math.floor((seconds % 86400) / 3600);
    if (hours === 0) {
      return days === 1 ? "1 day" : `${days} days`;
    }
    return `${days}d ${hours}h`;
  }

  if (seconds >= 3600) {
    const hours = Math.floor(seconds / 3600);
    const minutes = Math.floor((seconds % 3600) / 60);
    if (minutes === 0) {
      return `${hours}h`;
    }
    return `${hours}h ${minutes}m`;
  }

  if (seconds < 60) {
    return `${seconds}s`;
  }

  if (seconds % 60 === 0) {
    return `${seconds / 60} min`;
  }

  const minutes = Math.floor(seconds / 60);
  const remainingSeconds = seconds % 60;
  return `${minutes} min ${remainingSeconds}s`;
}
