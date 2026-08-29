import { useLocale, type LocaleCode } from "../shared/locale";
import { formatCount, formatFinancialValue, formatFixedPercent, groupFormat } from "../shared/format/financialValue";
import { formatDetailTimestamp, formatLocalIsoDate } from "../shared/format/datetime";

export type FigureKind = "count" | "percent" | "date" | "datetime" | "money" | "badge";

export type FigureProps = {
  value: number | string;
  kind?: FigureKind;
  className?: string;
};

// A figure/date/percent value (ADR 0104 dec. 2 amendment, F4a S1): always the
// UI face with lining numerals via `.num-tabular` — never mono, which spaces
// punctuation ("15 , 2 mld PLN"). One shared formatting seam per kind so
// screens stop hand-formatting counts/percents/dates inconsistently.
export function Figure({ value, kind = "count", className }: FigureProps) {
  const { locale } = useLocale();
  return (
    <span className={["num-tabular", className].filter(Boolean).join(" ")} data-figure={kind}>
      {formatFigureValue(value, kind, locale)}
    </span>
  );
}

function formatFigureValue(value: number | string, kind: FigureKind, locale: LocaleCode): string {
  switch (kind) {
    case "percent":
      return formatFixedPercent(Number(value), locale);
    case "money":
      return formatFinancialValue({ valueNumeric: String(value), valueKind: "monetary" }, locale);
    case "date":
      return formatLocalIsoDate(String(value));
    case "datetime":
      return formatDetailTimestamp(String(value));
    // A fixed-width chip (badge) caps at "99+" so a triple-digit count never
    // blows out its layout — the exact figure lives one click away. A prose/table count (the default)
    // has no such constraint and always renders the real, locale-grouped
    // number — capping a "18 members"/"342 companies" figure at "99+" would
    // just be wrong.
    case "badge":
      return formatCount(Number(value));
    case "count":
    default:
      return groupFormat(Number(value), locale, 0);
  }
}
