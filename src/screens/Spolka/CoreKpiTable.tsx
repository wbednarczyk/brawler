import { Button, EmptyState, ErrorText, SectionHeader } from "../../ui";
import { useLocale } from "../../shared/locale";
import { localizedKpiLabelForKey } from "../../shared/locale/kpiLabels";
import { formatFinancialValue } from "../../shared/format/financialValue";
import type { CompanyView } from "../../api/generated/CompanyView";
import type { Tool } from "./route";

const DASH = "—";

export type CoreKpiTableProps = {
  kpi: CompanyView["kpi"];
  error: boolean;
  onOpenTool: (tool: Tool) => void;
  onOpenDocument: (documentRef: string) => void;
};

// "Wyniki roczne" — revenue/operating-profit/net-profit FY trend (F3a S1,
// mockup Main.dc.html). The provenance ticket (ADR 0104 dec. 7) sits once at
// the footer, on the newest populated cell across the three rows — not one
// per cell, matching the mockup's "source ticket of the newest cell".
export function CoreKpiTable({ kpi, error, onOpenTool, onOpenDocument }: CoreKpiTableProps) {
  const { text, locale } = useLocale();

  const newestTicket = (() => {
    if (!kpi) return undefined;
    for (let i = kpi.years.length - 1; i >= 0; i -= 1) {
      const year = kpi.years[i];
      for (const row of kpi.rows) {
        const cell = row.cells.find((c) => c.fiscalYear === year);
        if (cell?.sourceDocumentRef) return cell.sourceDocumentRef;
      }
    }
    return undefined;
  })();

  return (
    <div role="group" aria-label={text("Annual KPI table")} className="spolka-section spolka-kpi">
      <SectionHeader level="h2" title={text("Annual results")} description={text("PLN million · consolidated")} />

      {error ? (
        <ErrorText>{text("Couldn't load the KPI table. The rest of the view is up to date.")}</ErrorText>
      ) : !kpi || kpi.rows.length === 0 ? (
        <EmptyState>{text("No confirmed annual figures yet — read a report to populate this table.")}</EmptyState>
      ) : (
        <>
          <table className="spolka-kpi-table">
            <thead>
              <tr>
                <th>{text("Line item")}</th>
                {kpi.years.map((year) => (
                  <th key={year} className="num-tabular">
                    {year}
                  </th>
                ))}
                <th>{text("y/y")}</th>
              </tr>
            </thead>
            <tbody>
              {kpi.rows.map((row) => (
                <tr key={row.metricKey}>
                  <td>{localizedKpiLabelForKey(row.metricKey, locale)}</td>
                  {row.cells.map((cell) => (
                    <td key={cell.fiscalYear} className="num-tabular">
                      {cell.valueNumeric === undefined
                        ? DASH
                        : formatFinancialValue(
                            { valueNumeric: cell.valueNumeric, currency: kpi.currency, valueKind: "monetary" },
                            locale,
                          )}
                    </td>
                  ))}
                  <td className="num-tabular">
                    {row.yoyPct === undefined
                      ? DASH
                      : formatFinancialValue({ valueNumeric: String(row.yoyPct), valueKind: "percentage" }, locale)}
                  </td>
                </tr>
              ))}
            </tbody>
          </table>
          <div className="spolka-kpi-footer">
            {newestTicket ? (
              <Button
                variant="ghost"
                className="spolka-provenance-ticket"
                aria-label={text("Open source document")}
                onClick={() => onOpenDocument(newestTicket)}
              >
                {newestTicket}
              </Button>
            ) : null}
            <span className="spolka-kpi-hint">{text("Every figure leads to its source")}</span>
          </div>
        </>
      )}

      <Button variant="secondary" onClick={() => onOpenTool({ t: "fundamenty" })}>
        {text("Open fundamentals")}
      </Button>
    </div>
  );
}
