import { useEffect, useMemo, useRef, useState } from "react";
import { ChevronRight, ExternalLink } from "lucide-react";

import { getKpiComparison } from "../../api/comparison";
import type { KpiComparison } from "../../api/comparison";
import { useLocale, type LocaleCode } from "../../shared/locale";
import { formatFinancialValue, groupFormat } from "../../shared/format/financialValue";
import {
  periodExpanderAccessibleName,
  periodExpanderVisibleLabel,
  useVisiblePeriods, naturalCellWidth } from "./useVisiblePeriods";
import {
  ActionButton,
  Button,
  EmptyState,
  ErrorText,
  SectionHeader,
  SegmentedControl,
  SegmentedControlOption,
  StatusChip,
} from "../../ui";

type Granularity = "annual" | "quarterly";
type LoadState = "idle" | "loading" | "error";

// Sticky column widths (dogfooding #6): fixed for the same reason as the
// facts matrix (FundamentalsPanel.tsx) — a dynamic `left` offset would need
// inline `style={{…}}`. Mirrored in companies.css
// `.fundamentals-periods-kpi`/`-expander`.
const PERIODS_KPI_COLUMN_WIDTH = 140;
const PERIODS_EXPANDER_COLUMN_WIDTH = 44;

type FundamentalsPeriodsSectionProps = {
  companyId: string;
  // The panel's own KPI set (fact-matrix rows), in display order. Drives the
  // N=1 comparison call and the row order.
  metricKeys: string[];
  // metricKey → localized KPI label (the panel already computes these).
  kpiLabelByMetricKey: Record<string, string>;
  selectedFactId: string | null;
  // Evidence affordance: selecting the fact reveals its detail above (the same
  // idiom the fact matrix uses — the panel links facts by selecting them).
  onSelectFact: (factId: string) => void;
};

function isRatioKind(valueKind: string | null): boolean {
  return valueKind === "ratio" || valueKind === "percentage";
}

function formatDelta(locale: LocaleCode, decimal: string, valueKind: string | null): string {
  const parsed = Number(decimal);
  if (!Number.isFinite(parsed)) return decimal;
  const sign = parsed > 0 ? "+" : "";
  const magnitude = groupFormat(parsed, locale, 1);
  const unit = isRatioKind(valueKind) ? " p.p." : "%";
  return `${sign}${magnitude}${unit}`;
}

/**
 * The company Dashboard's periods × deltas view (v0.61 §A5, ADR 0089 dec. 1).
 * The N=1 case of the cross-company comparison read model: rows are the panel's
 * KPIs, columns the recent aligned periods, with QoQ (quarterly only) / YoY
 * deltas inline per period. Deltas + typed flags come from the read model — this
 * only renders them, never recomputes (the mockup's "delty liczone z faktów").
 */
export function FundamentalsPeriodsSection({
  companyId,
  metricKeys,
  kpiLabelByMetricKey,
  selectedFactId,
  onSelectFact,
}: FundamentalsPeriodsSectionProps) {
  const { text, locale } = useLocale();

  const [granularity, setGranularity] = useState<Granularity>("annual");
  const [comparison, setComparison] = useState<KpiComparison | null>(null);
  const [status, setStatus] = useState<LoadState>("idle");
  const requestSeq = useRef(0);
  const scrollerRef = useRef<HTMLDivElement | null>(null);

  const metricKeysKey = metricKeys.join(",");
  useEffect(() => {
    if (metricKeys.length === 0) {
      setComparison(null);
      setStatus("idle");
      return;
    }
    const seq = (requestSeq.current += 1);
    let cancelled = false;
    setStatus("loading");
    getKpiComparison({ companyIds: [companyId], metricKeys, granularity })
      .then((result) => {
        if (cancelled || seq !== requestSeq.current) return;
        setComparison(result);
        setStatus("idle");
      })
      .catch(() => {
        if (cancelled || seq !== requestSeq.current) return;
        setStatus("error");
      });
    return () => {
      cancelled = true;
    };
    // metricKeysKey stands in for the metricKeys array identity.
    // eslint-disable-next-line react-hooks/exhaustive-deps
  }, [companyId, metricKeysKey, granularity]);

  const axis = useMemo(() => comparison?.axis ?? [], [comparison]);
  const quarterly = granularity === "quarterly";

  // Period-expander column (dogfooding #6): reuses the facts-matrix hook with
  // this host's own measured period-group width + fixed sticky-column width.
  const periodsVisible = useVisiblePeriods({
    scrollerRef,
    total: axis.length,
    // One period group = value + delta column(s); each column's width is the
    // widest natural cell in it (columns stretch, so rendered widths are circular).
    measurePeriodWidth: () => {
      const scroller = scrollerRef.current;
      if (!scroller) return 0;
      const widest = new Map<string, number>();
      for (const cell of scroller.querySelectorAll<HTMLElement>("[data-period-col]")) {
        const col = cell.dataset.periodCol as string;
        widest.set(col, Math.max(widest.get(col) ?? 0, naturalCellWidth(cell)));
      }
      let group = 0;
      for (const width of widest.values()) group += width;
      return group;
    },
    stickyWidth: PERIODS_KPI_COLUMN_WIDTH + PERIODS_EXPANDER_COLUMN_WIDTH,
  });
  // Autoscroll to the newest period only on the transition INTO the expanded
  // state — never on mount, never on a later resize.
  useEffect(() => {
    if (!periodsVisible.expanded) return;
    const scroller = scrollerRef.current;
    if (!scroller) return;
    scroller.scrollLeft = scroller.scrollWidth;
  }, [periodsVisible.expanded]);

  // The shown indices reference the full axis so cells (aligned 1:1) slice the same.
  const shownIndices = useMemo(
    () =>
      axis
        .map((_, index) => index)
        .slice(periodsVisible.visibleStart, periodsVisible.visibleStart + periodsVisible.visibleCount),
    [axis, periodsVisible.visibleStart, periodsVisible.visibleCount],
  );
  const focusIndex = axis.length - 1;
  // The column stays visible while expanded (showing "Collapse earlier") even
  // though `hiddenCount` is then 0 — "no column at all" only applies when
  // there was never anything to hide in the first place.
  const showExpanderColumn = periodsVisible.expanded || periodsVisible.hiddenCount > 0;

  const series = useMemo(
    () => (comparison?.series ?? []).filter((entry) => entry.companyId === companyId),
    [comparison, companyId],
  );

  const flagLabel = (flag: string): string => {
    switch (flag) {
      case "no_fact":
        return text("no data for period");
      case "fx_missing":
        return text("no FX rate");
      case "currency_unknown":
        return text("unknown currency");
      default:
        return flag;
    }
  };
  const deltaUndefinedReason = text(
    "Undefined change (non-positive or sign-flipped base) — an honest gap, not a fabricated number.",
  );

  const periodLabel = (fiscalYear: number, periodType: string): string =>
    periodType === "FY" ? String(fiscalYear) : `${fiscalYear} ${periodType}`;

  const renderDelta = (delta: string | null, undefinedFlag: boolean, valueKind: string | null) => {
    if (undefinedFlag || delta === null) {
      return (
        <span
          className="fundamentals-periods-empty"
          title={undefinedFlag ? deltaUndefinedReason : undefined}
        >
          —
        </span>
      );
    }
    const parsed = Number(delta);
    // Deliberate, mockup-approved simplification (epic #398): coloring is the
    // raw sign of the delta, not a metric-aware "is this good for the
    // business" judgment — a cost line stored as an increasingly negative
    // fact naturally comes out red on a growing cost, which reads correctly
    // by construction, but a metric with no such convention would not.
    // Reversible: swap in a per-metric "good direction" once one exists.
    const direction =
      parsed > 0 ? "fundamentals-periods-up" : parsed < 0 ? "fundamentals-periods-dn" : "";
    return <span className={direction}>{formatDelta(locale, delta, valueKind)}</span>;
  };

  const showTable = status === "idle" && comparison !== null && shownIndices.length > 0;
  const showEmpty = status === "idle" && comparison !== null && axis.length === 0;

  return (
    <section className="fundamentals-section fundamentals-periods" aria-label={text("Positions × periods")}>
      <div className="fundamentals-periods-head">
        <SectionHeader
          level="h4"
          title={text("Positions × periods")}
          description={text("QoQ/YoY deltas computed from facts, not re-parsed reports.")}
        />
        <SegmentedControl ariaLabel={text("Period granularity")}>
          <SegmentedControlOption active={!quarterly} onClick={() => setGranularity("annual")}>
            {text("Annual")}
          </SegmentedControlOption>
          <SegmentedControlOption active={quarterly} onClick={() => setGranularity("quarterly")}>
            {text("Quarterly")}
          </SegmentedControlOption>
        </SegmentedControl>
      </div>

      {status === "error" ? (
        <ErrorText>{text("Could not load the periods table.")}</ErrorText>
      ) : null}

      {showEmpty ? (
        <EmptyState>{text("No aligned periods yet.")}</EmptyState>
      ) : null}

      {showTable ? (
        <>
          {/* Deliberate wide content: the periods table scrolls inside its own
              bounded scroller (data-hscroll), contained so it never forces a
              panel- or pane-level horizontal scrollbar (narrow-window rule). */}
          <div
            className="fundamentals-periods-scroll"
            data-hscroll
            data-expanded={periodsVisible.expanded || undefined}
            data-visible-periods={shownIndices.length}
            aria-label={text("Positions and period deltas")}
            ref={scrollerRef}
          >
            <table className="fundamentals-periods-table ui-zebra">
              <thead>
                <tr>
                  <th className="fundamentals-periods-corner" scope="col">
                    {text("Position")}
                  </th>
                  {/* No column at all when nothing is hidden (owner storyboard). */}
                  {showExpanderColumn ? (
                    <th className="fundamentals-periods-expander" scope="col">
                      <span className="visually-hidden">{text("Earlier periods")}</span>
                    </th>
                  ) : null}
                  {shownIndices.flatMap((index) => {
                    const period = axis[index];
                    const focus = index === focusIndex;
                    const cells = [
                      <th
                        key={`${period.key}-value`}
                        scope="col"
                        className={focus ? "fundamentals-periods-focus" : undefined}
                        data-period-col="value"
                      >
                        {periodLabel(period.fiscalYear, period.periodType)}
                      </th>,
                      ...(quarterly
                        ? [
                            <th key={`${period.key}-qoq`} scope="col" data-period-col="qoq">
                              {text("Δ QoQ")}
                            </th>,
                          ]
                        : []),
                    ];
                    cells.push(
                      <th key={`${period.key}-yoy`} scope="col" data-period-col="yoy">
                        {text("Δ YoY")}
                      </th>,
                    );
                    return cells;
                  })}
                </tr>
              </thead>
              <tbody>
                {series.map((row, rowIndex) => {
                  const label = kpiLabelByMetricKey[row.metricKey] ?? row.metricKey;
                  return (
                    <tr key={row.metricKey}>
                      <th className="fundamentals-periods-kpi" scope="row" title={label}>
                        {label}
                      </th>
                      {/* The whole column is ONE clickable cell spanning every
                          body row (owner storyboard round 1). */}
                      {rowIndex === 0 && showExpanderColumn ? (
                        <td className="fundamentals-periods-expander" rowSpan={series.length}>
                          <ActionButton
                            kind="control"
                            className="fundamentals-periods-expander-button"
                            variant="ghost"
                            onClick={periodsVisible.toggle}
                            aria-label={periodExpanderAccessibleName(
                              periodsVisible.expanded,
                              periodsVisible.hiddenCount,
                              locale,
                              text,
                            )}
                          >
                            <ChevronRight
                              aria-hidden="true"
                              size={13}
                              className="fundamentals-periods-expander-chevron"
                            />
                            {periodExpanderVisibleLabel(periodsVisible.expanded, text)}
                          </ActionButton>
                        </td>
                      ) : null}
                      {shownIndices.flatMap((index) => {
                        const cell = row.cells[index];
                        const focus = index === focusIndex;
                        const focusClass = focus ? " fundamentals-periods-focus" : "";
                        const gapFlag = cell?.flags.find(
                          (flag) =>
                            flag === "no_fact" ||
                            flag === "fx_missing" ||
                            flag === "currency_unknown",
                        );
                        const valueCell = (
                          <td
                            key={`${row.metricKey}-${index}-value`}
                            className={`fundamentals-periods-value-cell${focusClass}`}
                            data-period-col="value"
                          >
                            {gapFlag ? (
                              <StatusChip tone="warn" className="fundamentals-periods-flag">
                                {flagLabel(gapFlag)}
                              </StatusChip>
                            ) : cell?.value != null ? (
                              <span className="fundamentals-periods-value">
                                {formatFinancialValue(
                                  {
                                    valueNumeric: cell.value,
                                    currency: cell.currency,
                                    valueKind: row.valueKind,
                                    metricKey: row.metricKey,
                                  },
                                  locale,
                                )}
                                {cell.factId ? (
                                  <Button
                                    variant="icon"
                                    className={`fundamentals-periods-evidence${
                                      selectedFactId === cell.factId ? " is-selected" : ""
                                    }`}
                                    icon={<ExternalLink size={13} aria-hidden="true" />}
                                    aria-label={`${text("Show evidence")}: ${label}, ${periodLabel(
                                      cell.fiscalYear,
                                      cell.periodType,
                                    )}`}
                                    onClick={() => onSelectFact(cell.factId as string)}
                                  />
                                ) : null}
                              </span>
                            ) : (
                              <span className="fundamentals-periods-empty">—</span>
                            )}
                          </td>
                        );
                        return [
                          valueCell,
                          ...(quarterly
                            ? [
                                <td
                                  key={`${row.metricKey}-${index}-qoq`}
                                  className={`fundamentals-periods-delta-cell${focusClass}`}
                                  data-period-col="qoq"
                                >
                                  {renderDelta(
                                    cell?.deltaQoQ ?? null,
                                    cell?.flags.includes("delta_qoq_undefined") ?? false,
                                    row.valueKind,
                                  )}
                                </td>,
                              ]
                            : []),
                          <td
                            key={`${row.metricKey}-${index}-yoy`}
                            className={`fundamentals-periods-delta-cell${focusClass}`}
                            data-period-col="yoy"
                          >
                            {renderDelta(
                              cell?.deltaYoY ?? null,
                              cell?.flags.includes("delta_yoy_undefined") ?? false,
                              row.valueKind,
                            )}
                          </td>,
                        ];
                      })}
                    </tr>
                  );
                })}
              </tbody>
            </table>
          </div>
        </>
      ) : null}
    </section>
  );
}
