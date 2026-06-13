import { describe, it, expect, vi, beforeEach } from "vitest";
import { render, screen, waitFor, within } from "@testing-library/react";
import userEvent from "@testing-library/user-event";

import { FeedKpiExtractionPanel } from "./FeedKpiExtractionPanel";
import { listCompanies } from "../../api/companies";
import { listReportDocuments, captureReportDocument } from "../../api/reportDocuments";
import {
  startKpiExtraction,
  listKpiExtraction,
  confirmKpiProposal,
  rejectKpiProposal,
} from "../../api/kpiExtraction";
import { resolveIrReport } from "../../api/ir";
import type { FeedItem } from "../../api/types";

vi.mock("../../api/companies", () => ({ listCompanies: vi.fn() }));
vi.mock("../../api/reportDocuments", () => ({
  listReportDocuments: vi.fn(),
  captureReportDocument: vi.fn(),
}));
vi.mock("../../api/kpiExtraction", () => ({
  startKpiExtraction: vi.fn(),
  listKpiExtraction: vi.fn(),
  confirmKpiProposal: vi.fn(),
  rejectKpiProposal: vi.fn(),
}));
vi.mock("../../api/ir", () => ({ resolveIrReport: vi.fn() }));

const feedItem: FeedItem = {
  id: "feed_1",
  company: "GPW:CDR",
  type: "official_report",
  source: "Bankier",
  time: "2026-06-03T10:00:00Z",
  title: "Q3 2025 report",
  unread: true,
  saved: false,
  sourceUrl: "https://example.com/report",
  language: "pl",
  publishedAt: "2026-06-03T10:00:00Z",
  fetchedAt: "2026-06-03T10:05:00Z",
  attribution: "Bankier",
  summary: "summary",
  bodyText: "body",
  attachments: [{ id: "att_1", label: "Report", url: "https://example.com/report.pdf" }],
};

function proposal(overrides: Record<string, unknown>) {
  return {
    id: "p",
    jobId: "job_1",
    metricKey: "revenue",
    label: "Revenue",
    valueNumeric: "142312000",
    unit: null,
    currency: null,
    asReportedValue: null,
    asReportedScale: null,
    measureWindow: null,
    confidence: "high",
    sourceSnippet: "przychody 142 312 tys.",
    isProposedKpi: false,
    status: "pending",
    factId: null,
    createdAt: "",
    updatedAt: "",
    ...overrides,
  };
}

function succeededJob(proposals: ReturnType<typeof proposal>[]) {
  return {
    id: "job_1",
    companyId: "co1",
    reportDocumentId: "doc1",
    providerId: "test_sample",
    model: "test",
    promptVersion: "kpi-extraction.v1",
    periodHint: null,
    status: "succeeded",
    errorCode: null,
    error: null,
    detectedFiscalYear: 2025,
    detectedPeriodType: "Q3",
    detectedPeriodEndDate: "2025-09-30",
    detectedCurrency: "PLN",
    detectedLanguage: "pl",
    createdAt: "",
    startedAt: "",
    finishedAt: "",
    proposals,
  };
}

beforeEach(() => {
  vi.clearAllMocks();
  vi.mocked(listCompanies).mockResolvedValue([
    {
      id: "co1",
      exchange: "GPW",
      ticker: "CDR",
      qualifiedTicker: "GPW:CDR",
      displayName: "CD PROJEKT S.A.",
      isin: null,
      cik: null,
      lei: null,
    },
  ]);
  vi.mocked(listReportDocuments).mockResolvedValue([]);
  vi.mocked(captureReportDocument).mockResolvedValue({
    documentId: "doc1",
    localPath: "doc1.pdf",
    success: true,
    error: null,
  });
});

describe("FeedKpiExtractionPanel", () => {
  it("extracts from an attachment and confirms only the chosen proposal", async () => {
    const user = userEvent.setup();
    vi.mocked(startKpiExtraction).mockResolvedValue(
      succeededJob([
        proposal({ id: "p_rev", metricKey: "revenue", label: "Revenue" }),
        proposal({
          id: "p_bk",
          metricKey: "backlog",
          label: "Backlog",
          valueNumeric: "410000000",
          isProposedKpi: true,
          confidence: "medium",
        }),
      ])
    );
    vi.mocked(confirmKpiProposal).mockResolvedValue({} as never);
    vi.mocked(listKpiExtraction).mockResolvedValue([
      succeededJob([
        proposal({ id: "p_rev", label: "Revenue", status: "confirmed", factId: "fact_1" }),
        proposal({ id: "p_bk", metricKey: "backlog", label: "Backlog", isProposedKpi: true }),
      ]),
    ]);

    render(<FeedKpiExtractionPanel feedItem={feedItem} providerConfigured />);

    const extract = await screen.findByRole("button", { name: "Extract from attachment" });
    await user.click(extract);

    // Proposals render after the (mocked) succeeded job.
    expect(await screen.findByText("Revenue")).toBeInTheDocument();
    expect(screen.getByText("Backlog")).toBeInTheDocument();

    // The out-of-taxonomy suggestion cannot be confirmed until accepted as a new KPI.
    const confirmButtons = screen.getAllByRole("button", { name: "Confirm" });
    const enabled = confirmButtons.filter((button) => !(button as HTMLButtonElement).disabled);
    expect(enabled).toHaveLength(1);

    await user.click(enabled[0]);
    await waitFor(() => expect(confirmKpiProposal).toHaveBeenCalledTimes(1));
    expect(vi.mocked(confirmKpiProposal).mock.calls[0][0]).toMatchObject({
      proposalId: "p_rev",
      fiscalYear: 2025,
      periodType: "Q3",
    });
  });

  it("rejects a proposal without committing a fact", async () => {
    const user = userEvent.setup();
    vi.mocked(startKpiExtraction).mockResolvedValue(
      succeededJob([proposal({ id: "p_rev", label: "Revenue" })])
    );
    vi.mocked(rejectKpiProposal).mockResolvedValue(
      proposal({ id: "p_rev", status: "rejected" })
    );
    vi.mocked(listKpiExtraction).mockResolvedValue([
      succeededJob([proposal({ id: "p_rev", label: "Revenue", status: "rejected" })]),
    ]);

    render(<FeedKpiExtractionPanel feedItem={feedItem} providerConfigured />);

    await user.click(await screen.findByRole("button", { name: "Extract from attachment" }));
    await screen.findByText("Revenue");
    await user.click(screen.getByRole("button", { name: "Reject" }));

    await waitFor(() => expect(rejectKpiProposal).toHaveBeenCalledWith("p_rev"));
    expect(confirmKpiProposal).not.toHaveBeenCalled();
  });

  it("falls back to candidate selection when IR resolution is not confident", async () => {
    const user = userEvent.setup();
    vi.mocked(resolveIrReport).mockResolvedValue({
      document: null,
      candidates: [{ url: "https://reports.example.com/q3.pdf", label: "Q3 2025" }],
      pickedUrl: null,
      confidence: "low",
    });

    render(<FeedKpiExtractionPanel feedItem={feedItem} providerConfigured />);

    await user.click(await screen.findByRole("button", { name: "Fetch report from IR page" }));
    expect(await screen.findByText("Q3 2025")).toBeInTheDocument();
    expect(screen.getByRole("button", { name: "Use this" })).toBeInTheDocument();
  });
});
