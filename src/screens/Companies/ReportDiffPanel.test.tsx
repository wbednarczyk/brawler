import { describe, it, expect, vi, beforeEach } from "vitest";
import { render, screen, waitFor, within } from "@testing-library/react";
import userEvent from "@testing-library/user-event";

import { ReportDiffPanel } from "./ReportDiffPanel";
import {
  extractReportSections,
  fetchReportDocument,
  getReportDiff,
  listReportDiffCandidates,
  type ReportDiffCandidate,
  type ReportDiffResult,
} from "../../api/reportDiff";

vi.mock("../../api/reportDiff", () => ({
  extractReportSections: vi.fn(),
  fetchReportDocument: vi.fn(),
  getReportDiff: vi.fn(),
  listReportDiffCandidates: vi.fn(),
}));

const listCandidatesMock = vi.mocked(listReportDiffCandidates);
const getDiffMock = vi.mocked(getReportDiff);
const extractMock = vi.mocked(extractReportSections);
const fetchMock = vi.mocked(fetchReportDocument);

function candidate(overrides: Partial<ReportDiffCandidate> = {}): ReportDiffCandidate {
  return {
    statementType: "ssf",
    sourceFormat: "pdf",
    older: {
      reportDocumentId: "doc_q3",
      title: "2025 Q3 SSF",
      periodLabel: "2025 Q3",
      extractionStatus: "extracted",
    },
    newer: {
      reportDocumentId: "doc_q1",
      title: "2026 Q1 SSF",
      periodLabel: "2026 Q1",
      extractionStatus: "extracted",
    },
    ...overrides,
  };
}

describe("ReportDiffPanel", () => {
  beforeEach(() => {
    vi.clearAllMocks();
    listCandidatesMock.mockResolvedValue({ companyId: "company_gpw_cbf", candidates: [] });
    extractMock.mockResolvedValue({
      extractionState: "extracted",
      sourceFormat: "pdf",
      sectionCount: 5,
      skipped: false,
    });
    fetchMock.mockResolvedValue({ reportDocumentId: "doc", fetched: true });
  });

  it("shows the empty state when there are no comparable statements", async () => {
    render(<ReportDiffPanel companyId="company_gpw_cbf" />);
    await waitFor(() => {
      expect(screen.getByText("No comparable financial statements yet.")).toBeInTheDocument();
    });
  });

  it("lists a candidate pair and renders the changed sections on compare", async () => {
    listCandidatesMock.mockResolvedValue({
      companyId: "company_gpw_cbf",
      candidates: [candidate()],
    });
    const diff: ReportDiffResult = {
      statementType: "ssf",
      status: "ok",
      diff: {
        alignedCount: 3,
        sections: [
          { status: "unchanged", heading: "wybrane dane finansowe", olderOrdinal: 0, newerOrdinal: 0, addedLines: 0, removedLines: 0 },
          { status: "changed", heading: "bilans", olderOrdinal: 1, newerOrdinal: 1, addedLines: 4, removedLines: 2 },
          { status: "only_newer", heading: "nowe ryzyka", addedLines: 0, removedLines: 0 },
        ],
      },
    };
    getDiffMock.mockResolvedValue(diff);

    const user = userEvent.setup();
    render(<ReportDiffPanel companyId="company_gpw_cbf" />);

    await waitFor(() => {
      expect(screen.getByText("2025 Q3 → 2026 Q1")).toBeInTheDocument();
    });
    await user.click(screen.getByRole("button", { name: "Compare" }));

    await waitFor(() => {
      // The changed and added sections appear; the unchanged one is omitted.
      expect(screen.getByText("bilans")).toBeInTheDocument();
      expect(screen.getByText("nowe ryzyka")).toBeInTheDocument();
      expect(screen.queryByText("wybrane dane finansowe")).not.toBeInTheDocument();
    });
    expect(screen.getByText("+4 −2")).toBeInTheDocument();
  });

  it("opens the active diff in a full-screen Focus reader and exits on Esc (ADR 0054)", async () => {
    listCandidatesMock.mockResolvedValue({
      companyId: "company_gpw_cbf",
      candidates: [candidate()],
    });
    getDiffMock.mockResolvedValue({
      statementType: "ssf",
      status: "ok",
      diff: {
        alignedCount: 1,
        sections: [
          { status: "changed", heading: "bilans", olderOrdinal: 1, newerOrdinal: 1, addedLines: 4, removedLines: 2 },
        ],
      },
    });

    const user = userEvent.setup();
    render(<ReportDiffPanel companyId="company_gpw_cbf" />);
    await waitFor(() => expect(screen.getByText("2025 Q3 → 2026 Q1")).toBeInTheDocument());
    await user.click(screen.getByRole("button", { name: "Compare" }));
    await waitFor(() => expect(screen.getByText("bilans")).toBeInTheDocument());

    await user.click(screen.getByRole("button", { name: "Focus" }));
    const overlay = await screen.findByRole("dialog", { name: "Report comparison" });
    expect(within(overlay).getByText("bilans")).toBeInTheDocument();

    await user.keyboard("{Escape}");
    await waitFor(() =>
      expect(screen.queryByRole("dialog", { name: "Report comparison" })).not.toBeInTheDocument(),
    );
  });

  it("flags a scanned (not-diffable) report instead of an empty diff", async () => {
    listCandidatesMock.mockResolvedValue({
      companyId: "company_gpw_cbf",
      candidates: [candidate()],
    });
    getDiffMock.mockResolvedValue({ statementType: "ssf", status: "not_diffable", diff: undefined });

    const user = userEvent.setup();
    render(<ReportDiffPanel companyId="company_gpw_cbf" />);
    await waitFor(() => expect(screen.getByText("2025 Q3 → 2026 Q1")).toBeInTheDocument());
    await user.click(screen.getByRole("button", { name: "Compare" }));

    await waitFor(() => {
      expect(
        screen.getByText("Can't compare — no extractable text (scanned report)."),
      ).toBeInTheDocument();
    });
  });
});
