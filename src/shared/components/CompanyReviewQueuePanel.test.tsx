import { describe, it, expect, vi, beforeEach } from "vitest";
import { render, screen, waitFor, within } from "@testing-library/react";
import userEvent from "@testing-library/user-event";

import { CompanyReviewQueuePanel } from "./CompanyReviewQueuePanel";
import {
  confirmKpiProposal,
  listPendingKpiProposals,
  rejectKpiProposal,
} from "../../api/kpiExtraction";
import type { PendingKpiProposal } from "../../api/kpiExtraction";

vi.mock("../../api/kpiExtraction", () => ({
  listPendingKpiProposals: vi.fn(),
  confirmKpiProposal: vi.fn(),
  rejectKpiProposal: vi.fn(),
}));

const listMock = vi.mocked(listPendingKpiProposals);
const confirmMock = vi.mocked(confirmKpiProposal);
const rejectMock = vi.mocked(rejectKpiProposal);

function proposal(overrides: Partial<PendingKpiProposal> = {}): PendingKpiProposal {
  return {
    id: "p_rev",
    jobId: "job_1",
    metricKey: "revenue",
    label: "Przychody ze sprzedaży",
    valueNumeric: "120400000",
    unit: null,
    currency: "PLN",
    sourceSnippet: "ocr_bootstrap: przychody",
    status: "pending",
    fiscalYear: 2025,
    periodType: "Q3",
    documentId: "doc_1",
    documentTitle: "CBF 2025 Q3 raport kwartalny",
    documentUrl: "https://x/doc1.pdf",
    createdAt: "2026-06-15T10:00:00Z",
    updatedAt: "2026-06-15T10:00:00Z",
    ...overrides,
  };
}

describe("CompanyReviewQueuePanel", () => {
  beforeEach(() => {
    vi.clearAllMocks();
    listMock.mockResolvedValue([]);
    confirmMock.mockResolvedValue({
      // FinancialFact fields are irrelevant to the panel; only validationStatus is read.
      fact: { id: "fact_1" } as never,
      validationStatus: "unreviewed",
    });
    rejectMock.mockResolvedValue({ id: "p_rev", status: "rejected" } as never);
  });

  it("renders proposals grouped by period with the source chip derived from the snippet", async () => {
    listMock.mockResolvedValue([
      proposal({ id: "p_rev", metricKey: "revenue", sourceSnippet: "ocr_bootstrap: przychody" }),
      proposal({
        id: "p_ebitda",
        metricKey: "ebitda",
        label: "EBITDA",
        sourceSnippet: "ebitda",
      }),
    ]);
    render(<CompanyReviewQueuePanel companyId="company_gpw_cbf" />);

    // Grouped under the detected period.
    expect(await screen.findByText("2025 Q3")).toBeInTheDocument();
    // An ocr_bootstrap snippet → the accent "OCR bootstrap" chip; a plain
    // (prefix-less) snippet → the "AI" chip.
    expect(screen.getByText("OCR bootstrap")).toBeInTheDocument();
    expect(screen.getByText("AI")).toBeInTheDocument();
    expect(screen.getByText("Przychody ze sprzedaży")).toBeInTheDocument();
  });

  it("confirms a proposal by id and refetches the shrunk list", async () => {
    listMock
      .mockResolvedValueOnce([proposal({ id: "p_rev" }), proposal({ id: "p_np", metricKey: "net_profit", label: "Zysk netto" })])
      .mockResolvedValue([proposal({ id: "p_np", metricKey: "net_profit", label: "Zysk netto" })]);
    const onReviewed = vi.fn();
    render(<CompanyReviewQueuePanel companyId="company_gpw_cbf" onReviewed={onReviewed} />);

    const revenueRow = (await screen.findByText("Przychody ze sprzedaży")).closest(".review-row")!;
    await userEvent.click(within(revenueRow as HTMLElement).getByRole("button", { name: "Confirm" }));

    expect(confirmMock).toHaveBeenCalledWith({ proposalId: "p_rev" });
    await waitFor(() =>
      expect(screen.queryByText("Przychody ze sprzedaży")).not.toBeInTheDocument(),
    );
    expect(screen.getByText("Zysk netto")).toBeInTheDocument();
    expect(onReviewed).toHaveBeenCalled();
  });

  it("surfaces a flagged validation outcome without blocking", async () => {
    listMock
      .mockResolvedValueOnce([proposal({ id: "p_np", metricKey: "net_profit", label: "Zysk netto" })])
      .mockResolvedValue([]);
    confirmMock.mockResolvedValue({ fact: { id: "fact_1" } as never, validationStatus: "flagged" });
    render(<CompanyReviewQueuePanel companyId="company_gpw_cbf" />);

    await userEvent.click(await screen.findByRole("button", { name: "Confirm" }));

    await waitFor(() =>
      expect(
        screen.getByText("Confirmed — validation flagged Zysk netto. Check it in the coverage map."),
      ).toBeInTheDocument(),
    );
  });

  it("rejects a proposal by id and refetches", async () => {
    listMock
      .mockResolvedValueOnce([proposal({ id: "p_rev" })])
      .mockResolvedValue([]);
    const onReviewed = vi.fn();
    render(<CompanyReviewQueuePanel companyId="company_gpw_cbf" onReviewed={onReviewed} />);

    await userEvent.click(await screen.findByRole("button", { name: "Reject" }));

    expect(rejectMock).toHaveBeenCalledWith("p_rev");
    await waitFor(() =>
      expect(
        screen.getByText("No proposals to review — deterministic extractions write facts directly."),
      ).toBeInTheDocument(),
    );
    expect(onReviewed).toHaveBeenCalled();
  });

  it("maps a value-conflict confirm error to a localized message and keeps the row", async () => {
    listMock.mockResolvedValue([proposal({ id: "p_rev", label: "Przychody ze sprzedaży" })]);
    // The backend rejects the confirm because the slot already holds a different
    // value (ADR 0077 value_conflict) — the panel must not leak the raw sqlite text.
    confirmMock.mockRejectedValue(
      new Error(
        "invalid financials value for value_conflict: stored=300000000 proposed=120400000",
      ),
    );
    render(<CompanyReviewQueuePanel companyId="company_gpw_cbf" />);

    await userEvent.click(await screen.findByRole("button", { name: "Confirm" }));

    // A friendly, localized message carrying both values — never the raw error.
    const message = await screen.findByText(/a different value is already saved/i);
    expect(message.textContent).toContain("300000000");
    expect(message.textContent).toContain("120400000");
    expect(screen.queryByText(/UNIQUE constraint failed/i)).toBeNull();
    expect(screen.queryByText(/value_conflict: stored=/i)).toBeNull();

    // The row stays in the queue — the proposal is still pending.
    expect(screen.getByText("Przychody ze sprzedaży")).toBeInTheDocument();
  });

  it("renders the empty state when there are no pending proposals", async () => {
    listMock.mockResolvedValue([]);
    render(<CompanyReviewQueuePanel companyId="company_gpw_cbf" />);

    await waitFor(() =>
      expect(
        screen.getByText("No proposals to review — deterministic extractions write facts directly."),
      ).toBeInTheDocument(),
    );
  });
});
