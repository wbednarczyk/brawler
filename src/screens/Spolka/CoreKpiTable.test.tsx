import { describe, expect, it, vi } from "vitest";
import { render, screen } from "@testing-library/react";
import userEvent from "@testing-library/user-event";

import { CoreKpiTable } from "./CoreKpiTable";
import type { CompanyView } from "../../api/generated/CompanyView";

function kpi(overrides: Partial<CompanyView["kpi"]> = {}): NonNullable<CompanyView["kpi"]> {
  return {
    currency: "PLN",
    years: [2024, 2025],
    rows: [
      {
        metricKey: "revenue",
        cells: [
          { fiscalYear: 2024, valueNumeric: "800", sourceDocumentRef: "report_doc_2024" },
          { fiscalYear: 2025, valueNumeric: "1000", sourceDocumentRef: "report_doc_2025" },
        ],
        yoyPct: 25,
      },
      { metricKey: "operating_profit", cells: [], yoyPct: undefined },
      { metricKey: "net_profit", cells: [], yoyPct: undefined },
    ],
    ...overrides,
  };
}

describe("CoreKpiTable", () => {
  // KPI provenance ticket navigation (sol-review finding 8, ADR 0104 dec. 7):
  // the footer ticket is the newest populated cell's `sourceDocumentRef`
  // across all rows. A document-id ref (an internal report) hands that ref to
  // `onOpenDocument` verbatim, the value `{t:"dokumenty", documentId}`
  // navigates on (owner dogfooding v0.74 wave 2, item 3).
  it("document-id ticket opens the documents tool", async () => {
    const onOpenDocument = vi.fn();
    render(
      <CoreKpiTable kpi={kpi()} error={false} onOpenTool={vi.fn()} onOpenDocument={onOpenDocument} onOpenExternalUrl={vi.fn()} />,
    );

    const ticket = screen.getByRole("button", { name: "Open source document" });
    expect(ticket.textContent).toBe("report_doc_2025");
    await userEvent.click(ticket);

    expect(onOpenDocument).toHaveBeenCalledWith("report_doc_2025");
  });

  // A URL ref (a BiznesRadar aggregator fact, ADR 0086) carries no internal
  // document id — its ticket shows a human label instead of the raw URL and
  // opens externally, never through the `dokumenty` tool.
  it("URL ticket opens externally with a human label", async () => {
    const onOpenExternalUrl = vi.fn();
    const onOpenDocument = vi.fn();
    const url = "https://www.biznesradar.pl/raporty-finansowe-rachunek-zyskow-i-strat/CDR";
    render(
      <CoreKpiTable
        kpi={kpi({
          rows: [
            { metricKey: "revenue", cells: [{ fiscalYear: 2025, valueNumeric: "1000", sourceDocumentRef: url }], yoyPct: undefined },
            { metricKey: "operating_profit", cells: [], yoyPct: undefined },
            { metricKey: "net_profit", cells: [], yoyPct: undefined },
          ],
        })}
        error={false}
        onOpenTool={vi.fn()}
        onOpenDocument={onOpenDocument}
        onOpenExternalUrl={onOpenExternalUrl}
      />,
    );

    const ticket = screen.getByRole("button", { name: "Open source document" });
    expect(ticket.textContent).toBe("BiznesRadar · RZiS");
    await userEvent.click(ticket);

    expect(onOpenExternalUrl).toHaveBeenCalledWith(url);
    expect(onOpenDocument).not.toHaveBeenCalled();
  });

  it("renders no ticket when no cell carries a source document ref", () => {
    render(
      <CoreKpiTable
        kpi={kpi({
          rows: [
            { metricKey: "revenue", cells: [{ fiscalYear: 2025, valueNumeric: "1000" }], yoyPct: undefined },
            { metricKey: "operating_profit", cells: [], yoyPct: undefined },
            { metricKey: "net_profit", cells: [], yoyPct: undefined },
          ],
        })}
        error={false}
        onOpenTool={vi.fn()}
        onOpenDocument={vi.fn()}
        onOpenExternalUrl={vi.fn()}
      />,
    );

    expect(screen.queryByRole("button", { name: "Open source document" })).not.toBeInTheDocument();
  });
});
