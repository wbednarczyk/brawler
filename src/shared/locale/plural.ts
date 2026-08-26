import type { LocaleCode } from "./index";

// Locale-aware noun pluralization. English has two forms (one / other); Polish has three
// (one / few / many), which a simple `n === 1 ? a : b` ternary gets wrong (e.g. "18 spółki"
// instead of "18 spółek"). Counts in the UI should render their noun through this helper.

export type PluralForms = {
  // [one, other], e.g. ["company", "companies"].
  en: readonly [string, string];
  // [one, few, many], e.g. ["spółka", "spółki", "spółek"].
  pl: readonly [string, string, string];
};

// Polish plural category: 1 -> one; 2–4 (but not 12–14) -> few; everything else -> many.
function polishCategory(n: number): 0 | 1 | 2 {
  const abs = Math.abs(n);
  const mod10 = abs % 10;
  const mod100 = abs % 100;
  if (abs === 1) return 0;
  if (mod10 >= 2 && mod10 <= 4 && !(mod100 >= 12 && mod100 <= 14)) return 1;
  return 2;
}

// The correct noun form for `n` in the active locale (the noun only — callers prepend the count).
export function pluralNoun(locale: LocaleCode, n: number, forms: PluralForms): string {
  if (locale === "pl") return forms.pl[polishCategory(n)];
  return n === 1 ? forms.en[0] : forms.en[1];
}

// Shared across screens (Today's autopilot run card, Companies' fundamentals
// header) that report a raw `financial_facts`/produced-fact count.
export const FACT_FORMS: PluralForms = { en: ["fact", "facts"], pl: ["fakt", "fakty", "faktów"] };

// Today's cross-company routine aggregate chip ("×K spółek", ADR 0087).
export const COMPANY_FORMS: PluralForms = {
  en: ["company", "companies"],
  pl: ["spółka", "spółki", "spółek"],
};

// Today's per-company group count chips carry their unit so "×4" is not
// opaque: attention groups count fired events, autopilot groups count runs;
// any other category falls back to a generic item noun.
export const EVENT_FORMS: PluralForms = {
  en: ["event", "events"],
  pl: ["zdarzenie", "zdarzenia", "zdarzeń"],
};

export const RUN_FORMS: PluralForms = {
  en: ["run", "runs"],
  pl: ["run", "runy", "runów"],
};

export const ITEM_FORMS: PluralForms = {
  en: ["item", "items"],
  pl: ["pozycja", "pozycje", "pozycji"],
};

// Grouped fundamentals matrix (card #307): "N metryk" in a collapsible
// group's header.
export const METRIC_FORMS: PluralForms = {
  en: ["metric", "metrics"],
  pl: ["metryka", "metryki", "metryk"],
};

// Inbox v2 report-kind detail (F1 S4): "N dokumenty" count line above the
// document list.
export const DOCUMENT_FORMS: PluralForms = {
  en: ["document", "documents"],
  pl: ["dokument", "dokumenty", "dokumentów"],
};

// Dziś v2 delta header (F2 S3): "N raportów, M komunikatów" since the last visit.
export const REPORT_FORMS: PluralForms = {
  en: ["report", "reports"],
  pl: ["raport", "raporty", "raportów"],
};

export const FILING_FORMS: PluralForms = {
  en: ["filing", "filings"],
  pl: ["komunikat", "komunikaty", "komunikatów"],
};

// Dziś v2 delta header secondary note (F2 S4): "Plus N media items".
export const MEDIA_ITEM_FORMS: PluralForms = {
  en: ["media item", "media items"],
  pl: ["pozycja medialna", "pozycje medialne", "pozycji medialnych"],
};

// Dziś v2 day-section header (F2 S3): "N pozycji · M nieprzejrzane" — an
// adjective form (agrees like a noun-count suffix), not a noun, but the same
// three-category Polish plural machinery applies.
export const UNSEEN_FORMS: PluralForms = {
  en: ["unread", "unread"],
  pl: ["nieprzejrzana", "nieprzejrzane", "nieprzejrzanych"],
};

// Statement-switcher completeness bar (epic #398): "N czeka na nazwanie" — the
// verb, not just the noun, needs Polish agreement (singular "czeka" for
// one/many-genitive, plural "czekają" for the few category), mirroring the
// RECORDED_FORMS precedent in FundamentalsPanel.tsx.
export const AWAITS_NAMING_FORMS: PluralForms = {
  en: ["awaits", "await"],
  pl: ["czeka", "czekają", "czeka"],
};

// Spółka screen glance bar (F3a S1): signals/claims/calendar-event counters.
export const SIGNAL_FORMS: PluralForms = {
  en: ["signal", "signals"],
  pl: ["sygnał", "sygnały", "sygnałów"],
};

export const CLAIM_FORMS: PluralForms = {
  en: ["claim", "claims"],
  pl: ["teza", "tezy", "tez"],
};

export const CALENDAR_EVENT_FORMS: PluralForms = {
  en: ["event", "events"],
  pl: ["wydarzenie", "wydarzenia", "wydarzeń"],
};
