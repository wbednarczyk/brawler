import type { RefObject } from "react";
import { ChevronRight } from "lucide-react";
import type { FinancialPeriod } from "../../api/financialsTypes";
import type { FactMatrixRow } from "./factMatrix";
import { isSubtotalRow } from "./factMatrix";
import { localizedKpiLabel } from "../../shared/locale/kpiLabels";
import { formatFinancialValue } from "../../shared/format/financialValue";
import type { LocaleCode } from "../../shared/locale";
import { periodExpanderAccessibleName, periodExpanderVisibleLabel, type UseVisiblePeriodsResult } from "./useVisiblePeriods";
import { ActionButton, EmptyState, Sparkline } from "../../ui";
import { factQualityLabel } from "./factLabels";

// The KPI × period fact matrix (extracted from FundamentalsPanel.tsx, ADR
// 0103 file-size ratchet, dogfooding wave 2026-09 #6): the newest
// MEASURED-capacity periods render, oldest hidden behind the full-height
// clickable expander column (useVisiblePeriods.ts, owner storyboard round 1).

export type FundamentalsFactsMatrixProps = {
  text: (value: string) => string;
  locale: LocaleCode;
  visibleMatrixRows: FactMatrixRow[];
  visibleFactPeriods: FinancialPeriod[];
  factsScrollRef: RefObject<HTMLDivElement | null>;
  factsPeriodHeaderRef: RefObject<HTMLTableCellElement | null>;
  factsPeriods: UseVisiblePeriodsResult;
  factsShowExpanderColumn: boolean;
  selectedFinancialFactId: string | null;
  selectFinancialFact: (id: string) => void;
  seriesValuesFor: (row: FactMatrixRow) => number[];
};

export function FundamentalsFactsMatrix({
  text,
  locale,
  visibleMatrixRows,
  visibleFactPeriods,
  factsScrollRef,
  factsPeriodHeaderRef,
  factsPeriods,
  factsShowExpanderColumn,
  selectedFinancialFactId,
  selectFinancialFact,
  seriesValuesFor,
}: FundamentalsFactsMatrixProps) {
  if (visibleMatrixRows.length === 0) {
    return <EmptyState>{text("No positions match your search.")}</EmptyState>;
  }

  return (
    /* The KPI × period matrix is DELIBERATE wide content: it scrolls inside
       this bounded wrapper (data-hscroll exempts it from the layout gate). */
    <div
      className="facts-matrix-scroll"
      data-hscroll
      aria-label={text("Financial facts matrix")}
      ref={factsScrollRef}
    >
      <table className="facts-matrix">
        <thead>
          <tr>
            <th className="facts-matrix-corner" scope="col">
              {text("KPI")}
            </th>
            {/* No column at all when nothing is hidden (owner storyboard). */}
            {factsShowExpanderColumn ? <th className="facts-matrix-expander" aria-hidden="true" /> : null}
            {visibleFactPeriods.map((period, index) => (
              <th key={period.id} scope="col" ref={index === 0 ? factsPeriodHeaderRef : undefined}>
                {period.fiscalYear} {period.periodType.toUpperCase()}
              </th>
            ))}
            <th className="facts-matrix-trend-head" scope="col">
              {text("Trend")}
            </th>
          </tr>
        </thead>
        <tbody>
          {visibleMatrixRows.map((row, rowIndex) => (
            // Subtotal emphasis (approved mockup): a statement's own
            // subtotal lines render heavier with a top rule — the
            // hierarchy that makes ~38 rows in one statement legible.
            <tr key={row.definition.id} className={isSubtotalRow(row) ? "facts-matrix-subtotal" : undefined}>
              <th className="facts-matrix-kpi" scope="row" title={localizedKpiLabel(row.definition, locale)}>
                {localizedKpiLabel(row.definition, locale)}
              </th>
              {/* The whole column is ONE clickable cell spanning every
                  body row (owner storyboard round 1: a full-height
                  column, not a corner-cell expander). */}
              {rowIndex === 0 && factsShowExpanderColumn ? (
                <td className="facts-matrix-expander" rowSpan={visibleMatrixRows.length}>
                  <ActionButton
                    kind="control"
                    className="facts-matrix-expander-button"
                    variant="ghost"
                    onClick={factsPeriods.toggle}
                    aria-label={periodExpanderAccessibleName(
                      factsPeriods.expanded,
                      factsPeriods.hiddenCount,
                      locale,
                      text,
                    )}
                  >
                    <ChevronRight aria-hidden="true" size={13} className="facts-matrix-expander-chevron" />
                    {periodExpanderVisibleLabel(factsPeriods.expanded, text)}
                  </ActionButton>
                </td>
              ) : null}
              {visibleFactPeriods.map((period) => {
                const fact = row.cells[period.id];
                if (!fact) {
                  return (
                    <td key={period.id} className="facts-matrix-cell-empty">
                      <span aria-hidden="true">—</span>
                    </td>
                  );
                }
                return (
                  <td key={period.id}>
                    <button
                      aria-label={`${localizedKpiLabel(row.definition, locale)}, ${period.fiscalYear} ${period.periodType.toUpperCase()}`}
                      className={[
                        "facts-matrix-cell",
                        selectedFinancialFactId === fact.id ? "facts-matrix-cell-selected" : "",
                      ]
                        .filter(Boolean)
                        .join(" ")}
                      onClick={() => selectFinancialFact(fact.id)}
                      type="button"
                    >
                      {formatFinancialValue(
                        {
                          valueNumeric: fact.valueNumeric,
                          currency: fact.currency,
                          asReportedValue: fact.asReportedValue,
                          asReportedScale: fact.asReportedScale,
                          valueKind: row.definition.valueKind,
                          unit: row.definition.unit,
                          metricKey: row.definition.metricKey,
                        },
                        locale,
                      )}
                      {fact.annotation ? (
                        <span
                          className="fact-annotation-marker"
                          title={fact.annotation}
                          aria-label={`${text("Annotation")}: ${fact.annotation}`}
                        >
                          *
                        </span>
                      ) : null}
                      {fact.dataQuality !== "final" ? (
                        <span
                          className="fact-quality-marker"
                          title={factQualityLabel(fact.dataQuality, text)}
                          aria-label={`${text("Data quality")}: ${factQualityLabel(fact.dataQuality, text)}`}
                        >
                          ‡
                        </span>
                      ) : null}
                    </button>
                  </td>
                );
              })}
              <td className="facts-matrix-trend">
                <Sparkline
                  values={seriesValuesFor(row)}
                  ariaLabel={`${localizedKpiLabel(row.definition, locale)} ${text("trend")}`}
                />
              </td>
            </tr>
          ))}
        </tbody>
      </table>
    </div>
  );
}
