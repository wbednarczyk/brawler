import { Button, EmptyState, ErrorText, SectionHeader } from "../../ui";
import { useLocale } from "../../shared/locale";
import { localizedKpiLabelForKey } from "../../shared/locale/kpiLabels";
import { deltaToneClass, formatFinancialValue } from "../../shared/format/financialValue";
import type { CompanyView } from "../../api/generated/CompanyView";
import type { Tool } from "./route";

const DASH = "—";

// RZiS = income statement (rachunek zysków i strat) — the one BiznesRadar
// page slug worth naming explicitly (owner dogfooding v0.74 wave 2, item 3);
// every other host falls back to its bare hostname.
const RZIS_SLUG = "raporty-finansowe-rachunek-zyskow-i-strat";

function isExternalUrl(ref: string): boolean {
  return ref.startsWith("http://") || ref.startsWith("https://");
}

/** A `sourceDocumentRef` that is itself a URL (BiznesRadar aggregator facts,
 * ADR 0086) carries no human title of its own — this derives one, never the
 * raw URL, for the provenance ticket's label. */
function humanSourceLabel(url: string): string {
  if (url.includes(RZIS_SLUG)) return "BiznesRadar · RZiS";
  try {
    return new URL(url).hostname;
  } catch {
    return url;
  }
}

export type CoreKpiTableProps = {
  kpi: CompanyView["kpi"];
  error: boolean;
  onOpenTool: (tool: Tool) => void;
  /** A document-id ticket (an internal report) opens the `dokumenty` tool. */
  onOpenDocument: (documentRef: string) => void;
  /** A URL ticket (a BiznesRadar aggregator fact, ADR 0086) opens in the
   * system browser instead — it names a page, not a stored document. */
  onOpenExternalUrl: (url: string) => void;
};

// "Wyniki roczne" — revenue/operating-profit/net-profit FY trend (F3a S1,
// mockup Main.dc.html). The provenance ticket (ADR 0104 dec. 7) sits once at
// the footer, on the newest populated cell across the three rows — not one
// per cell, matching the mockup's "source ticket of the newest cell"; that
// SAME cell carries the dotted provenance thread (ADR 0104 dec. 7).
export function CoreKpiTable({ kpi, error, onOpenTool, onOpenDocument, onOpenExternalUrl }: CoreKpiTableProps) {
  const { text, locale } = useLocale();

  const newestCell = (() => {
    if (!kpi) return undefined;
    for (let i = kpi.years.length - 1; i >= 0; i -= 1) {
      const year = kpi.years[i];
      for (const row of kpi.rows) {
        const cell = row.cells.find((c) => c.fiscalYear === year);
        if (cell?.sourceDocumentRef) return { metricKey: row.metricKey, fiscalYear: year, ref: cell.sourceDocumentRef };
      }
    }
    return undefined;
  })();
  const newestTicket = newestCell?.ref;

  // tabIndex + the existing role/label make the card's own scroll
  // keyboard-reachable (axe scrollable-region-focusable) now that it scrolls
  // its own overflow instead of the whole screen (owner dogfooding v0.74,
  // item 1).
  return (
    <div role="group" aria-label={text("Annual KPI table")} className="spolka-section spolka-kpi" tabIndex={0}>
      <SectionHeader level="h2" title={text("Annual results")} eyebrow={text("PLN million · consolidated")} />

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
                  {row.cells.map((cell) => {
                    const isThreadCell = newestCell?.metricKey === row.metricKey && newestCell.fiscalYear === cell.fiscalYear;
                    return (
                      <td
                        key={cell.fiscalYear}
                        className={["num-tabular", isThreadCell ? "spolka-kpi-thread" : ""].filter(Boolean).join(" ")}
                      >
                        {cell.valueNumeric === undefined
                          ? DASH
                          : formatFinancialValue(
                              { valueNumeric: cell.valueNumeric, currency: kpi.currency, valueKind: "monetary" },
                              locale,
                            )}
                      </td>
                    );
                  })}
                  <td className={["num-tabular", deltaToneClass(row.yoyPct)].filter(Boolean).join(" ")}>
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
                onClick={() =>
                  isExternalUrl(newestTicket) ? onOpenExternalUrl(newestTicket) : onOpenDocument(newestTicket)
                }
              >
                {isExternalUrl(newestTicket) ? humanSourceLabel(newestTicket) : newestTicket}
              </Button>
            ) : null}
            <span className="spolka-kpi-hint">{text("Every figure leads to its source")}</span>
          </div>
        </>
      )}

      <Button variant="secondary" onClick={() => onOpenTool({ t: "fundamenty" })}>
        {text("Fundamentals")}
      </Button>
    </div>
  );
}
