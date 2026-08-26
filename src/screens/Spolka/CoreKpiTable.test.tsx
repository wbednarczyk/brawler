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
  // across all rows — clicking it must hand that ref to `onOpenDocument`
  // verbatim, the value `{t:"dokumenty", documentId}` navigates on.
  it("KPI ticket navigates to its document", async () => {
    const onOpenDocument = vi.fn();
    render(
      <CoreKpiTable kpi={kpi()} error={false} onOpenTool={vi.fn()} onOpenDocument={onOpenDocument} />,
    );

    await userEvent.click(screen.getByRole("button", { name: "Open source document" }));

    expect(onOpenDocument).toHaveBeenCalledWith("report_doc_2025");
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
      />,
    );

    expect(screen.queryByRole("button", { name: "Open source document" })).not.toBeInTheDocument();
  });
});
