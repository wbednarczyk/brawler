import { describe, it, expect, vi, beforeEach } from "vitest";
import { render, screen, waitFor } from "@testing-library/react";
import userEvent from "@testing-library/user-event";
import type { ReactElement } from "react";

import { CompanyReportDocumentsPanel, middleTruncate } from "./CompanyReportDocumentsPanel";
import { listReportDocuments } from "../../api/reportDocuments";
import { extractReportDocumentData } from "../../api/fundamentalsExtraction";
import type { ReportDocument } from "../../api/reportDocumentsTypes";
import { ToastProvider } from "../../ui";

vi.mock("../../api/reportDocuments", () => ({
  listReportDocuments: vi.fn(),
}));

vi.mock("../../api/fundamentalsExtraction", () => ({
  extractReportDocumentData: vi.fn(),
}));

const listReportDocumentsMock = vi.mocked(listReportDocuments);
const extractReportDocumentDataMock = vi.mocked(extractReportDocumentData);

// The panel calls useToast(), so every render needs a ToastProvider ancestor.
function renderPanel(node: ReactElement) {
  return render(<ToastProvider>{node}</ToastProvider>);
}

function reportDocument(overrides: Partial<ReportDocument> = {}): ReportDocument {
  return {
    id: "report_doc_1",
    companyId: "company_gpw_cdr",
    periodId: "period_1",
    sourceType: "ir_page",
    originRef: "https://example.test/ir/cdr",
    url: "https://example.test/ir/cdr/annual-2026.pdf",
    localPath: null,
    contentType: "application/pdf",
    contentHash: "hash_cdr",
    byteSize: 1048576,
    title: "CD Projekt annual report 2026",
    attribution: "CD Projekt IR",
    fetchStatus: "fetched",
    fetchError: null,
    fetchedAt: "2026-06-01T09:12:00Z",
    createdAt: "2026-06-01T09:12:00Z",
    updatedAt: "2026-06-01T09:12:00Z",
    ...overrides,
  };
}

describe("middleTruncate", () => {
  it("returns short names unchanged", () => {
    expect(middleTruncate("short.pdf", 44)).toBe("short.pdf");
  });

  it("truncates the middle of a long name and keeps the extension/tail visible", () => {
    const name =
      "cyber_Folks_SA_30.06.2023_raport_okresowy_skonsolidowany_bardzo_dlugi.pdf";
    const out = middleTruncate(name, 44);
    expect(out.length).toBeLessThanOrEqual(44);
    expect(out).toContain("…");
    // Identity portion (head prefix + extension tail) survives.
    expect(out.startsWith("cyber_Folks")).toBe(true);
    expect(out.endsWith(".pdf")).toBe(true);
  });
});

describe("CompanyReportDocumentsPanel", () => {
  beforeEach(() => {
    vi.clearAllMocks();
    listReportDocumentsMock.mockResolvedValue([reportDocument()]);
    extractReportDocumentDataMock.mockResolvedValue({
      acceptance: "accepted",
      tier: "pdf",
      emitted: true,
      producedFactIds: ["fact_1"],
      skippedFactIds: [],
      divergentCount: 0,
      driftJson: null,
    });
  });

  it("renders the filename and preserves the full name in the link title", async () => {
    const longName =
      "cyber_Folks_SA_30.06.2023_raport_okresowy_skonsolidowany_bardzo_dlugi.pdf";
    listReportDocumentsMock.mockResolvedValue([reportDocument({ title: longName })]);
    renderPanel(<CompanyReportDocumentsPanel companyId="company_gpw_cdr" />);

    const link = await screen.findByRole("link");
    // The full name is preserved in the tooltip (title attribute)…
    expect(link).toHaveAttribute("title", longName);
    // …while the visible label is middle-ellipsized.
    expect(link.textContent).toContain("…");
  });

  it("keeps only the storage-status chip on the row and moves kind + source into the preview column", async () => {
    const { container } = renderPanel(
      <CompanyReportDocumentsPanel companyId="company_gpw_cdr" />,
    );
    // The differentiating storage-state chip + the date stay on the row line.
    expect(await screen.findByText("Stored")).toBeInTheDocument();
    expect(screen.getByText(/2026-06-01/)).toBeInTheDocument();

    // The repeated, non-differentiating type chip is gone from the row's chip
    // cluster (audit K-class noise): the chip cluster holds the status only.
    const chips = container.querySelector(".doc-chips");
    expect(chips?.textContent).toContain("Stored");
    expect(chips?.textContent).not.toContain("IR page");

    // Provenance is not deleted — document kind + source stay reachable in the
    // L-tier preview column (and its tooltip).
    const preview = container.querySelector<HTMLElement>(".doc-preview");
    expect(preview?.textContent).toContain("IR page");
    expect(preview?.textContent).toContain("CD Projekt IR");
    expect(preview?.title).toContain("IR page");
    expect(preview?.title).toContain("CD Projekt IR");
  });

  it("labels a link-only (not-stored) document with the neutral status chip", async () => {
    listReportDocumentsMock.mockResolvedValue([
      reportDocument({ fetchStatus: "metadata_only" }),
    ]);
    const { container } = renderPanel(
      <CompanyReportDocumentsPanel companyId="company_gpw_cdr" />,
    );
    await screen.findByRole("link");
    const chips = container.querySelector(".doc-chips");
    expect(chips?.textContent).toContain("Link only");
  });

  it("shows the empty state when there are no documents", async () => {
    listReportDocumentsMock.mockResolvedValue([]);
    renderPanel(<CompanyReportDocumentsPanel companyId="company_gpw_cdr" />);
    await waitFor(() => {
      expect(screen.getByText("No report documents stored yet.")).toBeInTheDocument();
    });
  });

  it("offers an Extract data action on a stored (fetched) document and reports the produced-fact count", async () => {
    extractReportDocumentDataMock.mockResolvedValue({
      acceptance: "accepted",
      tier: "pdf",
      emitted: true,
      producedFactIds: ["fact_1", "fact_2", "fact_3"],
      skippedFactIds: [],
      divergentCount: 0,
      driftJson: null,
    });
    const onExtracted = vi.fn();
    renderPanel(
      <CompanyReportDocumentsPanel companyId="company_gpw_cdr" onExtracted={onExtracted} />,
    );

    const button = await screen.findByRole("button", { name: "Extract data" });
    // Renders as a real secondary Button primitive (clear affordance), not a
    // text-like minimal control.
    expect(button).toHaveClass("secondary-button");
    await userEvent.click(button);

    expect(extractReportDocumentDataMock).toHaveBeenCalledWith({
      companyId: "company_gpw_cdr",
      reportDocumentId: "report_doc_1",
    });
    // Honest completion feedback: the actual count of new values, not a blanket
    // "done" — the toast reflects the summary the command returned.
    expect(await screen.findByText("Extracted new values: 3")).toBeInTheDocument();
    // A fact-producing run invalidates the sibling fundamentals view.
    expect(onExtracted).toHaveBeenCalledTimes(1);
  });

  it("reports a no-new-values result honestly (0 produced facts is not a success)", async () => {
    // The owner's real case: re-extracting an already-covered period yields no
    // new facts. The toast must say so instead of a false success.
    extractReportDocumentDataMock.mockResolvedValue({
      acceptance: "empty",
      tier: "pdf",
      emitted: false,
      producedFactIds: [],
      skippedFactIds: [],
      divergentCount: 0,
      driftJson: null,
    });
    const onExtracted = vi.fn();
    renderPanel(
      <CompanyReportDocumentsPanel companyId="company_gpw_cdr" onExtracted={onExtracted} />,
    );

    const button = await screen.findByRole("button", { name: "Extract data" });
    await userEvent.click(button);

    expect(
      await screen.findByText("No new values extracted from this document"),
    ).toBeInTheDocument();
    // Nothing changed → the fundamentals view is not invalidated.
    expect(onExtracted).not.toHaveBeenCalled();
  });

  it("names the reason when a flagged extraction produced no facts", async () => {
    extractReportDocumentDataMock.mockResolvedValue({
      acceptance: "flagged",
      tier: "pdf",
      emitted: false,
      producedFactIds: [],
      skippedFactIds: [],
      divergentCount: 0,
      driftJson: null,
    });
    renderPanel(<CompanyReportDocumentsPanel companyId="company_gpw_cdr" />);

    const button = await screen.findByRole("button", { name: "Extract data" });
    await userEvent.click(button);

    expect(
      await screen.findByText("No new values — the document was flagged for review"),
    ).toBeInTheDocument();
  });

  it("distinguishes a re-extraction (values already recorded) from an empty document", async () => {
    // T7-F: re-extracting a landed period re-observes every slot — the summary
    // carries them as skipped, and the toast says "already recorded" instead of
    // the misleading generic "no new values".
    extractReportDocumentDataMock.mockResolvedValue({
      acceptance: "accepted",
      tier: "esef",
      emitted: false,
      producedFactIds: [],
      skippedFactIds: ["fact_1", "fact_2"],
      divergentCount: 0,
      driftJson: null,
    });
    const onExtracted = vi.fn();
    renderPanel(
      <CompanyReportDocumentsPanel companyId="company_gpw_cdr" onExtracted={onExtracted} />,
    );

    const button = await screen.findByRole("button", { name: "Extract data" });
    await userEvent.click(button);

    expect(
      await screen.findByText("No new values — 2 already recorded from this document"),
    ).toBeInTheDocument();
    expect(onExtracted).not.toHaveBeenCalled();
  });

  it("surfaces divergent re-observed values without pretending success", async () => {
    // T7-F: a re-observed slot whose fresh value disagrees with the stored fact
    // is never silently overwritten — the toast points at the divergence.
    extractReportDocumentDataMock.mockResolvedValue({
      acceptance: "accepted",
      tier: "esef",
      emitted: false,
      producedFactIds: [],
      skippedFactIds: ["fact_1"],
      divergentCount: 1,
      driftJson: null,
    });
    renderPanel(<CompanyReportDocumentsPanel companyId="company_gpw_cdr" />);

    const button = await screen.findByRole("button", { name: "Extract data" });
    await userEvent.click(button);

    expect(
      await screen.findByText(
        "Extracted values differ from stored facts: 1 — stored values kept, see Diagnostics",
      ),
    ).toBeInTheDocument();
  });

  it("does not offer the action on an ineligible (not-stored) document", async () => {
    listReportDocumentsMock.mockResolvedValue([
      reportDocument({ fetchStatus: "metadata_only" }),
    ]);
    renderPanel(<CompanyReportDocumentsPanel companyId="company_gpw_cdr" />);

    // The row still renders (its link), but no extract action.
    await screen.findByRole("link");
    expect(screen.queryByRole("button", { name: "Extract data" })).not.toBeInTheDocument();
  });

  it("surfaces the command error message when extraction fails", async () => {
    extractReportDocumentDataMock.mockRejectedValue(
      new Error("Could not determine the reporting period for this document"),
    );
    renderPanel(<CompanyReportDocumentsPanel companyId="company_gpw_cdr" />);

    const button = await screen.findByRole("button", { name: "Extract data" });
    await userEvent.click(button);

    expect(
      await screen.findByText("Could not determine the reporting period for this document"),
    ).toBeInTheDocument();
  });
});
