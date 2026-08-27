import { Button, ErrorText } from "../../ui";
import { useLocale } from "../../shared/locale";
import { CALENDAR_EVENT_FORMS, CLAIM_FORMS, pluralNoun, SIGNAL_FORMS } from "../../shared/locale/plural";
import { formatFinancialValue } from "../../shared/format/financialValue";
import { formatSignalCategoryDisplayName } from "../../shared/formatting/labels";
import type { CompanyViewCounters, CompanyViewSectionErrors } from "../../api/companyView";
import type { Tool } from "./route";

// The Spółka glance bar's four counter buttons (F3a S1, plan §8): each one
// doubles as a workshop-tool shortcut. `counters`/`sectionErrors` are the
// composed read's atomic counters slot — a storage failure degrades the
// WHOLE group (contracts.md § Company View), never a per-counter partial.

export type GlanceBarProps = {
  counters: CompanyViewCounters | undefined;
  sectionErrors: CompanyViewSectionErrors;
  onOpenTool: (tool: Tool) => void;
};

// Dense-state cap (plan §11 "9 sygnałów"/mockup Wasko.dc.html): a triple-digit
// count would blow out the fixed-width chip, so it reads "99+" past the cap —
// the exact figure lives one click away behind the counter's own tool.
export function formatCount(n: number): string {
  return n > 99 ? "99+" : String(n);
}

export function GlanceBar({ counters, sectionErrors, onOpenTool }: GlanceBarProps) {
  const { text, locale } = useLocale();

  return (
    <div role="group" aria-label={text("Company glance bar")} className="spolka-glance">
      {sectionErrors.counters || !counters ? (
        <ErrorText>{text("Couldn't load the glance bar counters.")}</ErrorText>
      ) : (
        <>
          <Button
            variant="ghost"
            aria-label={text("Signals counter")}
            className="spolka-glance-counter"
            onClick={() => onOpenTool({ t: "sygnaly" })}
          >
            <span className="num-tabular spolka-glance-figure">{formatCount(counters.signals.unacked)}</span>
            <span className="spolka-glance-label">{pluralNoun(locale, counters.signals.unacked, SIGNAL_FORMS)}</span>
            {counters.signals.byCategory.length > 0 ? (
              <span className="spolka-glance-sub">
                {counters.signals.byCategory
                  .map((c) => `${formatCount(c.count)} ${text(formatSignalCategoryDisplayName(c.category))}`)
                  .join(" · ")}
              </span>
            ) : null}
          </Button>

          <Button
            variant="ghost"
            aria-label={text("Claims counter")}
            className="spolka-glance-counter"
            onClick={() => onOpenTool({ t: "tezy" })}
          >
            <span className="num-tabular spolka-glance-figure">{formatCount(counters.claims.open)}</span>
            <span className="spolka-glance-label">
              {pluralNoun(locale, counters.claims.open, CLAIM_FORMS)} {text("to settle")}
            </span>
            {counters.claims.nearestDue ? (
              <span className="spolka-glance-sub">{counters.claims.nearestDue}</span>
            ) : null}
          </Button>

          <Button
            variant="ghost"
            aria-label={text("Shorts counter")}
            className="spolka-glance-counter"
            onClick={() => onOpenTool({ t: "akcjonariat" })}
          >
            <span className="num-tabular spolka-glance-figure">
              {formatFinancialValue(
                { valueNumeric: String(counters.shorts.activeSumPct), valueKind: "percentage" },
                locale,
              )}
            </span>
            <span className="spolka-glance-label">{text("short")}</span>
            {counters.shorts.largestHolder ? (
              <span className="spolka-glance-sub">{counters.shorts.largestHolder}</span>
            ) : null}
          </Button>

          <Button
            variant="ghost"
            aria-label={text("Events counter")}
            className="spolka-glance-counter"
            onClick={() => onOpenTool({ t: "wydarzenia" })}
          >
            {counters.events.upcoming === 0 ? (
              <span className="spolka-glance-label">{text("No events scheduled")}</span>
            ) : (
              <>
                <span className="num-tabular spolka-glance-figure">{formatCount(counters.events.upcoming)}</span>
                <span className="spolka-glance-label">
                  {pluralNoun(locale, counters.events.upcoming, CALENDAR_EVENT_FORMS)}
                </span>
                <span className="spolka-glance-sub">{text("30 days")}</span>
              </>
            )}
          </Button>
        </>
      )}
    </div>
  );
}
