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
};

const LOCALE_TAGS: Record<LocaleCode, string> = {
  en: "en-US",
  pl: "pl-PL",
};

const SCALE_WORDS: Record<LocaleCode, { thousand: string; million: string; billion: string }> = {
  en: { thousand: "k", million: "M", billion: "B" },
  pl: { thousand: "tys.", million: "mln", billion: "mld" },
};

function localeTag(locale: LocaleCode): string {
  return LOCALE_TAGS[locale] ?? LOCALE_TAGS.en;
}

function groupFormat(value: number, locale: LocaleCode, maximumFractionDigits: number): string {
  return new Intl.NumberFormat(localeTag(locale), {
    minimumFractionDigits: 0,
    maximumFractionDigits,
  })
    .format(value)
    // Normalize non-breaking / narrow-no-break grouping spaces to plain spaces
    // so display and tests are consistent across ICU versions.
    .replace(/[  ]/g, " ");
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
  const num = Number(value.valueNumeric);
  if (!Number.isFinite(num)) {
    // Not parseable as a number — show the raw stored string rather than "NaN".
    return value.valueNumeric;
  }

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
