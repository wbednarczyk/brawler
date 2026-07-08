import type { LocaleCode } from "../locale";
import { groupFormat, SCALE_WORDS } from "./financialValue";

/**
 * Render a quality-criterion measured value for the scorecard row (owner T7
 * round-2 finding: the raw decimal-exact engine text must never render inline).
 *
 * - A criterion written against a percent literal ("roe >= 15%") measures a
 *   ratio: render as a locale percentage with one decimal ("28,6%"). The
 *   engine normalizes the stored threshold to a plain decimal ("0.15"), so the
 *   percent intent is detected from the threshold OR the criterion expression.
 * - Large magnitudes humanize with the shared scale words ("128,5 mln").
 * - Plain ratios keep two decimals ("2,33").
 * - Non-numeric text falls back verbatim (never worse than before).
 */
export function formatCriterionMeasure(
  value: string,
  percentContext: string | null,
  locale: LocaleCode,
): string {
  const num = Number(value);
  if (!Number.isFinite(num)) return value;

  if (percentContext?.includes("%")) {
    return `${groupFormat(num * 100, locale, 1)}%`;
  }

  const abs = Math.abs(num);
  const words = SCALE_WORDS[locale] ?? SCALE_WORDS.en;
  if (abs >= 1_000_000_000) return `${groupFormat(num / 1_000_000_000, locale, 1)} ${words.billion}`;
  if (abs >= 1_000_000) return `${groupFormat(num / 1_000_000, locale, 1)} ${words.million}`;
  if (abs >= 1_000) return `${groupFormat(num / 1_000, locale, 1)} ${words.thousand}`;

  return new Intl.NumberFormat(locale === "pl" ? "pl-PL" : "en-US", {
    minimumFractionDigits: 2,
    maximumFractionDigits: 2,
  }).format(num);
}
