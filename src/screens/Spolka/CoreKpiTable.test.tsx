import { describe, expect, it, vi } from "vitest";
import { act, fireEvent, render, screen } from "@testing-library/react";
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
  // The ONE provenance action (dogfooding #1b, ADR 0104 dec. 7 wording): the
  // threaded newest cell's figure IS the button — accessible name names the
  // metric/period/source — and it is the ONLY control that opens the source;
  // the footer ticket is its non-interactive twin. A document-id ref (an
  // internal report) hands that ref to `onOpenDocument` verbatim.
  it("exactly one button opens the source; Enter navigates to the document", async () => {
    const onOpenDocument = vi.fn();
    render(
      <CoreKpiTable kpi={kpi()} error={false} onOpenTool={vi.fn()} onOpenDocument={onOpenDocument} onOpenExternalUrl={vi.fn()} />,
    );

    const buttons = screen.getAllByRole("button", { name: /Open source:/ });
    expect(buttons).toHaveLength(1);
    const thread = buttons[0];
    expect(thread).toHaveAccessibleName("Open source: Revenue · 2025 · report_doc_2025");
    // The button's visible content is the formatted FIGURE (formatting itself
    // is `formatFinancialValue`'s contract, exercised elsewhere) — just
    // pin that it's the real value, not the empty-cell dash.
    expect(thread.textContent).not.toBe("—");

    act(() => thread.focus());
    await userEvent.keyboard("{Enter}");
    expect(onOpenDocument).toHaveBeenCalledWith("report_doc_2025");

    // The footer twin carries the same source label but is NOT a button.
    expect(screen.getByText("report_doc_2025", { selector: "span.spolka-provenance-ticket" })).toBeInTheDocument();
  });

  // A URL ref (a BiznesRadar aggregator fact, ADR 0086) carries no internal
  // document id — its ticket shows a human label instead of the raw URL and
  // opens externally, never through the `dokumenty` tool.
  it("URL source opens externally with a human label", async () => {
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

    const thread = screen.getByRole("button", { name: "Open source: Revenue · 2025 · BiznesRadar · RZiS" });
    await userEvent.click(thread);

    expect(onOpenExternalUrl).toHaveBeenCalledWith(url);
    expect(onOpenDocument).not.toHaveBeenCalled();
  });

  it("renders no thread button or ticket when no cell carries a source document ref", () => {
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

    expect(screen.queryByRole("button", { name: /Open source:/ })).not.toBeInTheDocument();
    expect(document.querySelector(".spolka-provenance-ticket")).not.toBeInTheDocument();
  });

  // Hover/focus on EITHER side of the one provenance action lights the pair
  // (`data-thread-hot` on the card) — ADR 0104 dec. 7 wording.
  it("hover on the thread button or the footer ticket lights the pair", async () => {
    const user = userEvent.setup();
    render(
      <CoreKpiTable kpi={kpi()} error={false} onOpenTool={vi.fn()} onOpenDocument={vi.fn()} onOpenExternalUrl={vi.fn()} />,
    );
    const card = screen.getByRole("article", { name: "Annual KPI table" });
    expect(card).not.toHaveAttribute("data-thread-hot");

    const thread = screen.getByRole("button", { name: /Open source:/ });
    await user.hover(thread);
    expect(card).toHaveAttribute("data-thread-hot", "true");
    await user.unhover(thread);
    expect(card).not.toHaveAttribute("data-thread-hot");

    const ticket = screen.getByText("report_doc_2025", { selector: "span.spolka-provenance-ticket" });
    await user.hover(ticket);
    expect(card).toHaveAttribute("data-thread-hot", "true");
    await user.unhover(ticket);
    expect(card).not.toHaveAttribute("data-thread-hot");

    fireEvent.focus(thread);
    expect(card).toHaveAttribute("data-thread-hot", "true");
    fireEvent.blur(thread);
    expect(card).not.toHaveAttribute("data-thread-hot");
  });

  // Zebra striping (dogfooding #13): alternate body rows carry a token tint —
  // a row-parity assertion, not a pixel/contrast measurement (that's the
  // visual gate's job).
  it("alternate body rows carry the zebra tint", () => {
    render(
      <CoreKpiTable
        kpi={kpi({
          rows: [
            { metricKey: "revenue", cells: [{ fiscalYear: 2025, valueNumeric: "1000", sourceDocumentRef: "doc" }], yoyPct: undefined },
            { metricKey: "operating_profit", cells: [{ fiscalYear: 2025, valueNumeric: "200" }], yoyPct: undefined },
            { metricKey: "net_profit", cells: [{ fiscalYear: 2025, valueNumeric: "50" }], yoyPct: undefined },
          ],
        })}
        error={false}
        onOpenTool={vi.fn()}
        onOpenDocument={vi.fn()}
        onOpenExternalUrl={vi.fn()}
      />,
    );
    expect(document.querySelector(".spolka-kpi-table")).toHaveClass("ui-zebra");
    const rows = document.querySelectorAll(".spolka-kpi-table tbody tr");
    expect(rows).toHaveLength(3);
    // nth-child is 1-based and CSS-driven (`utilities.css`'s `.ui-zebra`
    // `tbody tr:nth-child(even) > *` rule) — this pins the ROW
    // ORDER/PARITY the selector keys off (jsdom does not apply CSS, so the
    // computed background itself is the visual gate's job, not a unit test's).
    expect(rows[0].matches("tr:nth-child(even)")).toBe(false);
    expect(rows[1].matches("tr:nth-child(even)")).toBe(true);
    expect(rows[2].matches("tr:nth-child(even)")).toBe(false);
  });
});
