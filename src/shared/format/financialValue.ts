import type { LocaleCode } from "../locale";

// Financial values are stored as raw decimal base units (correct for math) but
// should be presented the way the report printed them. This is the single shared
// helper for that, used by the fundamentals panel and the KPI extraction review.
//
// Two paths:
//  1. As-reported (preferred): the extractor captured the figure exactly as
//     printed (`asReportedValue`, e.g. "142 312") plus its scale word
//     (`asReportedScale`, e.g. "tys." / "mln") verbatim from the document. We
//     render those as-is so the user sees what the filing showed, regardless of
//     UI locale, and append a currency/% suffix from the value kind.
//  2. Fallback (manual entry, no as-reported): format the base value with
//     locale-aware grouping and a sensible auto-scale, respecting value_kind.

export type FormattableValue = {
  valueNumeric: string;
  currency?: string | null;
  asReportedValue?: string | null;
  asReportedScale?: string | null;
  // value_kind from the KPI definition: monetary | percentage | count | ratio.
  valueKind?: string | null;
  // unit from the KPI definition: per_share | shares | ... (null for plain monetary).
  unit?: string | null;
  // metric_key from the KPI definition — lets the format layer apply per-metric
  // display conventions (see OUTFLOW_DISPLAY_METRIC_KEYS).
  metricKey?: string | null;
};

// Display sign convention (#156, decided ONCE here per ADR 0076's one
// formatting layer — never per panel): metric keys stored POSITIVE by catalog
// convention (fcf = ocf - capex) but conventionally read as cash OUTFLOWS
// render with a leading minus in the numeric fallback path, so a cash-flow
// context reads naturally. The as-reported path is exempt on purpose: it
// renders the figure exactly as the filing printed it.
export const OUTFLOW_DISPLAY_METRIC_KEYS: ReadonlySet<string> = new Set(["capex"]);

function outflowDisplaySign(value: FormattableValue, num: number): number {
  if (!value.metricKey || !OUTFLOW_DISPLAY_METRIC_KEYS.has(value.metricKey)) return num;
  return num > 0 ? -num : num;
}

const LOCALE_TAGS: Record<LocaleCode, string> = {
  en: "en-US",
  pl: "pl-PL",
};

export const SCALE_WORDS: Record<LocaleCode, { thousand: string; million: string; billion: string }> = {
  en: { thousand: "k", million: "M", billion: "B" },
  pl: { thousand: "tys.", million: "mln", billion: "mld" },
};

export function localeTag(locale: LocaleCode): string {
  return LOCALE_TAGS[locale] ?? LOCALE_TAGS.en;
}

export function groupFormat(value: number, locale: LocaleCode, maximumFractionDigits: number): string {
  return new Intl.NumberFormat(localeTag(locale), {
    minimumFractionDigits: 0,
    maximumFractionDigits,
  })
    .format(value)
    // Normalize non-breaking / narrow-no-break grouping spaces to plain spaces
    // so display and tests are consistent across ICU versions.
    .replace(/[\u00a0\u202f]/g, " ");
}

/**
 * Locale-aware decimal with a FIXED number of fraction digits (comma in pl, dot
 * in en). Unlike `groupFormat` (min 0 digits) this keeps trailing zeros, so
 * `2.4 → "2,40"` — needed where the second decimal is meaningful (KNF net short
 * positions, e.g. `0.53%` vs a rounded `0.5%`).
 */
export function formatFixedDecimal(value: number, locale: LocaleCode, digits = 2): string {
  if (!Number.isFinite(value)) return String(value);
  // Normalize negative zero (and values rounding to it, e.g. -0.001 at 2 digits)
  // so the UI never shows "-0,00" (surfaced live on the shortPositions panel).
  const rounded = Number(value.toFixed(digits));
  const normalized = Object.is(rounded, -0) || rounded === 0 ? 0 : value;
  return new Intl.NumberFormat(localeTag(locale), {
    minimumFractionDigits: digits,
    maximumFractionDigits: digits,
  })
    .format(normalized)
    // Normalize NBSP / narrow-NBSP grouping separators to plain spaces.
    .replace(/[\u00a0\u202f]/g, " ");
}

/** Fixed-precision percent (default 2 places), no space before `%` (KNF style). */
export function formatFixedPercent(value: number, locale: LocaleCode, digits = 2): string {
  return `${formatFixedDecimal(value, locale, digits)}%`;
}

// Suffix that applies to the as-reported figure: currency for monetary/per-share,
// "%" for percentages, nothing for counts/ratios.
function valueSuffix(value: FormattableValue): string | null {
  const kind = value.valueKind ?? "monetary";
  if (kind === "percentage") return "%";
  if (kind === "monetary") return value.currency?.trim() || null;
  return null;
}

function formatAsReported(value: FormattableValue): string | null {
  const asReported = value.asReportedValue?.trim();
  if (!asReported) return null;

  const parts = [asReported];
  // A real scale is a word (tys., mln, mld). Extractors sometimes emit a bare
  // number like "1" (meaning ×1 / base units) — that is noise, not a scale, and
  // must not render as a stray "1" (e.g. "15 661 260 1" or "8,63 1 PLN").
  const scale = value.asReportedScale?.trim();
  if (scale && !/^[\d.,\s]+$/.test(scale)) parts.push(scale);

  const suffix = valueSuffix(value);
  // Avoid doubling "%" if the as-reported figure already carries it.
  if (suffix && !(suffix === "%" && asReported.endsWith("%"))) {
    parts.push(suffix);
  }
  return parts.join(" ");
}

function formatFallback(value: FormattableValue, locale: LocaleCode): string {
  const raw = Number(value.valueNumeric);
  if (!Number.isFinite(raw)) {
    // Not parseable as a number — show the raw stored string rather than "NaN".
    return value.valueNumeric;
  }
  const num = outflowDisplaySign(value, raw);

  const kind = value.valueKind ?? "monetary";

  if (kind === "percentage") {
    return `${groupFormat(num, locale, 1)} %`;
  }
  if (kind === "ratio") {
    return groupFormat(num, locale, 2);
  }
  if (kind === "count") {
    return groupFormat(num, locale, 0);
  }
  if (value.unit === "per_share") {
    const body = groupFormat(num, locale, 2);
    const currency = value.currency?.trim();
    return currency ? `${body} ${currency}` : body;
  }

  // Monetary: auto-scale large figures to thousand/million/billion for readability.
  const words = SCALE_WORDS[locale] ?? SCALE_WORDS.en;
  const abs = Math.abs(num);
  let scaled = num;
  let word: string | null = null;
  if (abs >= 1_000_000_000) {
    scaled = num / 1_000_000_000;
    word = words.billion;
  } else if (abs >= 1_000_000) {
    scaled = num / 1_000_000;
    word = words.million;
  } else if (abs >= 1_000) {
    scaled = num / 1_000;
    word = words.thousand;
  }

  const body = groupFormat(scaled, locale, word ? 1 : 0);
  const parts = [body];
  if (word) parts.push(word);
  const currency = value.currency?.trim();
  if (currency) parts.push(currency);
  return parts.join(" ");
}

/**
 * Render a financial value the way the report presented it, falling back to a
 * locale-aware formatted base value when no as-reported figure was captured.
 */
export function formatFinancialValue(value: FormattableValue, locale: LocaleCode = "en"): string {
  return formatAsReported(value) ?? formatFallback(value, locale);
}

/**
 * The CSS class for a signed delta/percentage figure (ADR 0104 visual
 * hierarchy amendment, owner dogfooding v0.74 wave 2): positive → the
 * `--tone-positive` text color, negative → `--tone-negative`, zero/undefined
 * → no tone (the neutral figure color). Shared by every r/r, YTD, and 1M
 * delta figure so the sign convention never drifts panel to panel.
 */
export function deltaToneClass(value: number | null | undefined): string {
  if (value === null || value === undefined || value === 0) return "";
  return value > 0 ? "num-tone-positive" : "num-tone-negative";
}
