import { describe, it, expect, vi, beforeEach } from "vitest";
import { render, screen, waitFor } from "@testing-library/react";
import userEvent from "@testing-library/user-event";

import { CompanyBackfillPanel } from "./CompanyBackfillPanel";
import { backfillCompanyHistory, getBackfillProgress } from "../../api/sources";
import type { BackfillProgress } from "../../api/types";

vi.mock("../../api/sources", () => ({
  backfillCompanyHistory: vi.fn(),
  getBackfillProgress: vi.fn(),
}));

const getBackfillProgressMock = vi.mocked(getBackfillProgress);
const backfillCompanyHistoryMock = vi.mocked(backfillCompanyHistory);

const completed: BackfillProgress = {
  companyId: "company_gpw_cdr",
  status: "completed",
  pagesFetched: 3,
  itemsIngested: 12,
  documentsStored: 4,
  detailErrors: 0,
  error: null,
  startedAt: "2026-06-15T10:00:00Z",
  updatedAt: "2026-06-15T10:01:00Z",
};

describe("CompanyBackfillPanel", () => {
  beforeEach(() => {
    vi.clearAllMocks();
    getBackfillProgressMock.mockResolvedValue(null);
  });

  it("runs a backfill and shows the completed progress diagnostics", async () => {
    backfillCompanyHistoryMock.mockResolvedValue(completed);
    const user = userEvent.setup();

    render(<CompanyBackfillPanel companyId="company_gpw_cdr" />);

    await user.click(screen.getByRole("button", { name: "Backfill history" }));

    await waitFor(() => {
      expect(backfillCompanyHistoryMock).toHaveBeenCalledWith("company_gpw_cdr");
      expect(screen.getByText("Backfill complete")).toBeInTheDocument();
    });
    expect(screen.getByText(/3 pages fetched/)).toBeInTheDocument();
    expect(screen.getByText(/12 items ingested/)).toBeInTheDocument();
    expect(screen.getByText(/4 documents stored/)).toBeInTheDocument();
  });

  it("surfaces a failed backfill error", async () => {
    backfillCompanyHistoryMock.mockRejectedValue(new Error("source unreachable"));
    const user = userEvent.setup();

    render(<CompanyBackfillPanel companyId="company_gpw_cdr" />);
    await user.click(screen.getByRole("button", { name: "Backfill history" }));

    await waitFor(() => {
      expect(screen.getByText("Backfill failed")).toBeInTheDocument();
      expect(screen.getByText("source unreachable")).toBeInTheDocument();
    });
  });
});
