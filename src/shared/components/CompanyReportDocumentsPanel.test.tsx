import { describe, it, expect, vi, beforeEach } from "vitest";
import { render, screen, waitFor, within } from "@testing-library/react";
import userEvent from "@testing-library/user-event";
import type { ReactElement } from "react";

import { CompanyReportDocumentsPanel, middleTruncate } from "./CompanyReportDocumentsPanel";
import { getReportDocumentsView, reclassifyReportDocuments } from "../../api/reportDocuments";
import { extractReportDocumentData } from "../../api/fundamentalsExtraction";
import type { ReportDocument, ReportDocumentViewRow } from "../../api/reportDocumentsTypes";
import { ToastProvider } from "../../ui";

vi.mock("../../api/reportDocuments", () => ({
  getReportDocumentsView: vi.fn(),
  reclassifyReportDocuments: vi.fn(),
}));

vi.mock("../../api/fundamentalsExtraction", () => ({
  extractReportDocumentData: vi.fn(),
}));

const getReportDocumentsViewMock = vi.mocked(getReportDocumentsView);
const reclassifyReportDocumentsMock = vi.mocked(reclassifyReportDocuments);
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
    docKind: "periodic_ssf",
    ...overrides,
  };
}

// A view row: a document + its derived period + canonical flag. Defaults to a
// canonical FY2025 periodic report so single-row tests read the common case.
function viewRow(
  document: Partial<ReportDocument> = {},
  over: Partial<ReportDocumentViewRow> = {},
): ReportDocumentViewRow {
  return {
    document: reportDocument(document),
    fiscalYear: 2025,
    periodType: "FY",
    canonical: true,
    ...over,
  };
}

function mockView(rows: ReportDocumentViewRow[]) {
  getReportDocumentsViewMock.mockResolvedValue({ companyId: "company_gpw_cdr", rows });
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
    expect(out.startsWith("cyber_Folks")).toBe(true);
    expect(out.endsWith(".pdf")).toBe(true);
  });
});

describe("CompanyReportDocumentsPanel", () => {
  beforeEach(() => {
    vi.clearAllMocks();
    mockView([viewRow()]);
    reclassifyReportDocumentsMock.mockResolvedValue({ total: 1, updated: 0, byKind: {} });
    extractReportDocumentDataMock.mockResolvedValue({
      acceptance: "accepted",
      tier: "pdf",
      emitted: true,
      producedFactIds: ["fact_1"],
      skippedFactIds: [],
      divergentCount: 0,
      driftJson: null,
      tier4: null,
      tier4Proposals: 0,
    });
  });

  it("renders the filename and preserves the full name in the link title", async () => {
    const longName =
      "cyber_Folks_SA_30.06.2023_raport_okresowy_skonsolidowany_bardzo_dlugi.xbri";
    mockView([viewRow({ title: longName })]);
    renderPanel(<CompanyReportDocumentsPanel companyId="company_gpw_cdr" />);

    const link = await screen.findByRole("link");
    // The full name (with .xbri) survives in the tooltip — a live-drive selector
    // hook (a[title*=".xbri"]) depends on it.
    expect(link).toHaveAttribute("title", longName);
    // …while the visible label is middle-ellipsized.
    expect(link.textContent).toContain("…");
  });

  it("leads with the document-kind label and keeps a storage-status chip in a fixed slot", async () => {
    const { container } = renderPanel(
      <CompanyReportDocumentsPanel companyId="company_gpw_cdr" />,
    );
    await screen.findByRole("link");
    // The kind label leads the row (line 1); the storage-status chip is trailing.
    // (Scope to the row — the kind label also appears as a filter-select option.)
    expect(container.querySelector(".doc-row-kind")?.textContent).toBe("Consolidated report");
    expect(container.querySelector(".doc-status")?.textContent).toContain("Stored");
    // The old provenance-preview column is gone in the redesign.
    expect(container.querySelector(".doc-preview")).toBeNull();
  });

  it("labels a link-only (not-stored) document with the neutral status chip", async () => {
    mockView([viewRow({ fetchStatus: "metadata_only" })]);
    const { container } = renderPanel(
      <CompanyReportDocumentsPanel companyId="company_gpw_cdr" />,
    );
    await screen.findByRole("link");
    expect(container.querySelector(".doc-status")?.textContent).toContain("Link only");
  });

  it("shows the empty state when there are no documents", async () => {
    mockView([]);
    renderPanel(<CompanyReportDocumentsPanel companyId="company_gpw_cdr" />);
    await waitFor(() => {
      expect(screen.getByText("No report documents stored yet.")).toBeInTheDocument();
    });
  });

  // --- Grouped view (ADR 0077 §2, mockup Panel B) ---

  it("groups documents by period with headers in descending order", async () => {
    mockView([
      viewRow({ id: "d_2024", title: "2024 report" }, { fiscalYear: 2024, periodType: "FY" }),
      viewRow({ id: "d_2025", title: "2025 report" }, { fiscalYear: 2025, periodType: "FY" }),
      viewRow(
        { id: "d_2025q1", title: "2025 Q1 report" },
        { fiscalYear: 2025, periodType: "Q1" },
      ),
    ]);
    const { container } = renderPanel(
      <CompanyReportDocumentsPanel companyId="company_gpw_cdr" />,
    );
    await screen.findByText("2025 report");
    const labels = [...container.querySelectorAll(".doc-grp-label")].map((n) => n.textContent);
    // Newest period first: 2025 FY, then 2025 Q1, then 2024 FY.
    expect(labels).toEqual(["2025 FY", "2025 Q1", "2024 FY"]);
  });

  it("marks the canonical report of a period with a star", async () => {
    mockView([
      viewRow(
        { id: "d_ssf", title: "Consolidated 2025", docKind: "periodic_ssf" },
        { canonical: true },
      ),
      viewRow(
        { id: "d_jsf", title: "Standalone 2025", docKind: "periodic_jsf" },
        { canonical: false },
      ),
    ]);
    renderPanel(<CompanyReportDocumentsPanel companyId="company_gpw_cdr" />);

    const ssfRow = (await screen.findByText("Consolidated 2025")).closest("li")!;
    const jsfRow = screen.getByText("Standalone 2025").closest("li")!;
    expect(within(ssfRow).getByLabelText("Canonical report for this period")).toBeInTheDocument();
    expect(within(jsfRow).queryByLabelText("Canonical report for this period")).toBeNull();
  });

  it("folds companion files away and expands them on click", async () => {
    mockView([
      viewRow(
        { id: "d_ssf", title: "Consolidated 2025", docKind: "periodic_ssf" },
        { canonical: true },
      ),
      viewRow(
        {
          id: "d_sig",
          title: "signature.xades",
          docKind: "other",
          fetchStatus: "metadata_only",
        },
        { canonical: false },
      ),
    ]);
    renderPanel(<CompanyReportDocumentsPanel companyId="company_gpw_cdr" />);

    // The companion is hidden behind the fold; the periodic statement is not.
    await screen.findByText("Consolidated 2025");
    expect(screen.queryByText("signature.xades")).not.toBeInTheDocument();
    const fold = screen.getByRole("button", { name: /companion file/ });
    await userEvent.click(fold);
    expect(await screen.findByText("signature.xades")).toBeInTheDocument();
  });

  it("never folds an extract-eligible row, even a companion kind", async () => {
    mockView([
      viewRow(
        { id: "d_ssf", title: "Consolidated 2025", docKind: "periodic_ssf" },
        { canonical: true },
      ),
      viewRow(
        // An "other"-kind but STORED (fetched) document: its Extract action must
        // stay reachable, so it is never hidden inside the fold.
        { id: "d_other", title: "extractable other.pdf", docKind: "other", fetchStatus: "fetched" },
        { canonical: false },
      ),
    ]);
    renderPanel(<CompanyReportDocumentsPanel companyId="company_gpw_cdr" />);

    expect(await screen.findByText("extractable other.pdf")).toBeInTheDocument();
    // No companion fold appears, because the only companion is extract-eligible.
    expect(screen.queryByRole("button", { name: /companion file/ })).not.toBeInTheDocument();
  });

  it("collapses the No-period group and expands it with the count", async () => {
    mockView([
      viewRow(
        { id: "d_ssf", title: "Consolidated 2025", docKind: "periodic_ssf" },
        { canonical: true },
      ),
      viewRow(
        { id: "d_gov1", title: "Governance notice A", docKind: "governance" },
        { fiscalYear: null, periodType: null, canonical: false },
      ),
      viewRow(
        { id: "d_gov2", title: "Governance notice B", docKind: "governance" },
        { fiscalYear: null, periodType: null, canonical: false },
      ),
    ]);
    renderPanel(<CompanyReportDocumentsPanel companyId="company_gpw_cdr" />);

    await screen.findByText("No period");
    expect(screen.queryByText("Governance notice A")).not.toBeInTheDocument();
    const showAll = screen.getByRole("button", { name: /Show all \(2\)/ });
    await userEvent.click(showAll);
    expect(await screen.findByText("Governance notice A")).toBeInTheDocument();
    expect(screen.getByText("Governance notice B")).toBeInTheDocument();
  });

  it("narrows the visible rows with the search field", async () => {
    mockView([
      viewRow(
        { id: "d_a", title: "Consolidated annual report", url: "https://example.test/a-fy.pdf" },
        { periodType: "FY" },
      ),
      viewRow(
        {
          id: "d_b",
          title: "Quarterly update deck",
          docKind: "presentation",
          url: "https://example.test/b-q1.pdf",
        },
        { periodType: "Q1", canonical: false },
      ),
    ]);
    renderPanel(<CompanyReportDocumentsPanel companyId="company_gpw_cdr" />);

    await screen.findByText("Consolidated annual report");
    expect(screen.getByText("Quarterly update deck")).toBeInTheDocument();

    await userEvent.type(screen.getByLabelText("Search documents"), "annual");
    expect(screen.getByText("Consolidated annual report")).toBeInTheDocument();
    expect(screen.queryByText("Quarterly update deck")).not.toBeInTheDocument();
  });

  it("falls back to a flat list when grouping is turned off", async () => {
    mockView([
      viewRow({ id: "d_2024", title: "2024 report" }, { fiscalYear: 2024, periodType: "FY" }),
      viewRow({ id: "d_2025", title: "2025 report" }, { fiscalYear: 2025, periodType: "FY" }),
    ]);
    const { container } = renderPanel(
      <CompanyReportDocumentsPanel companyId="company_gpw_cdr" />,
    );
    await screen.findByText("2025 report");
    // Grouped by default → group headers present.
    expect(container.querySelectorAll(".doc-grp-label").length).toBeGreaterThan(0);

    await userEvent.click(screen.getByLabelText("Group by period"));
    // Flat list → no group headers, both rows still present.
    expect(container.querySelectorAll(".doc-grp-label").length).toBe(0);
    expect(container.querySelector(".doc-rows-flat")).not.toBeNull();
    expect(screen.getByText("2024 report")).toBeInTheDocument();
    expect(screen.getByText("2025 report")).toBeInTheDocument();
  });

  it("offers an Extract data action on a stored document and reports the produced-fact count", async () => {
    extractReportDocumentDataMock.mockResolvedValue({
      acceptance: "accepted",
      tier: "pdf",
      emitted: true,
      producedFactIds: ["fact_1", "fact_2", "fact_3"],
      skippedFactIds: [],
      divergentCount: 0,
      driftJson: null,
      tier4: null,
      tier4Proposals: 0,
    });
    const onExtracted = vi.fn();
    renderPanel(
      <CompanyReportDocumentsPanel companyId="company_gpw_cdr" onExtracted={onExtracted} />,
    );

    const button = await screen.findByRole("button", { name: "Extract data" });
    expect(button).toHaveClass("secondary-button");
    await userEvent.click(button);

    expect(extractReportDocumentDataMock).toHaveBeenCalledWith({
      companyId: "company_gpw_cdr",
      reportDocumentId: "report_doc_1",
    });
    expect(await screen.findByText("Extracted new values: 3")).toBeInTheDocument();
    expect(onExtracted).toHaveBeenCalledTimes(1);
  });

  // Tier-4 bootstrap/pending runs emit PROPOSALS, not facts (ADR 0077 §4) — the
  // toast must say so instead of the misleading "no new values".
  it("reports tier-4 proposals honestly instead of 'no new values'", async () => {
    extractReportDocumentDataMock.mockResolvedValue({
      acceptance: "empty",
      tier: "pdf",
      emitted: false,
      producedFactIds: [],
      skippedFactIds: [],
      divergentCount: 0,
      driftJson: null,
      tier4: "bootstrap_proposals",
      tier4Proposals: 5,
    });
    renderPanel(<CompanyReportDocumentsPanel companyId="company_gpw_cdr" />);

    const button = await screen.findByRole("button", { name: "Extract data" });
    await userEvent.click(button);

    expect(await screen.findByText("OCR proposals to review: 5")).toBeInTheDocument();
  });

  it("reports a no-new-values result honestly (0 produced facts is not a success)", async () => {
    extractReportDocumentDataMock.mockResolvedValue({
      acceptance: "empty",
      tier: "pdf",
      emitted: false,
      producedFactIds: [],
      skippedFactIds: [],
      divergentCount: 0,
      driftJson: null,
      tier4: null,
      tier4Proposals: 0,
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
      tier4: null,
      tier4Proposals: 0,
    });
    renderPanel(<CompanyReportDocumentsPanel companyId="company_gpw_cdr" />);

    const button = await screen.findByRole("button", { name: "Extract data" });
    await userEvent.click(button);

    expect(
      await screen.findByText("No new values — the document was flagged for review"),
    ).toBeInTheDocument();
  });

  it("distinguishes a re-extraction (values already recorded) from an empty document", async () => {
    extractReportDocumentDataMock.mockResolvedValue({
      acceptance: "accepted",
      tier: "esef",
      emitted: false,
      producedFactIds: [],
      skippedFactIds: ["fact_1", "fact_2"],
      divergentCount: 0,
      driftJson: null,
      tier4: null,
      tier4Proposals: 0,
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
    extractReportDocumentDataMock.mockResolvedValue({
      acceptance: "accepted",
      tier: "esef",
      emitted: false,
      producedFactIds: [],
      skippedFactIds: ["fact_1"],
      divergentCount: 1,
      driftJson: null,
      tier4: null,
      tier4Proposals: 0,
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
    mockView([viewRow({ fetchStatus: "metadata_only" })]);
    renderPanel(<CompanyReportDocumentsPanel companyId="company_gpw_cdr" />);

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

  it("shows the product-facing kind label for each doc_kind (flat list)", async () => {
    mockView([
      viewRow({ id: "d_ssf", title: "A", docKind: "periodic_ssf" }),
      viewRow({ id: "d_jsf", title: "B", docKind: "periodic_jsf" }, { canonical: false }),
      viewRow({ id: "d_audit", title: "C", docKind: "auditor_opinion" }, { canonical: false }),
      viewRow({ id: "d_pres", title: "D", docKind: "presentation" }, { canonical: false }),
      viewRow({ id: "d_gov", title: "E", docKind: "governance" }, { canonical: false }),
      viewRow({ id: "d_other", title: "F", docKind: "other" }, { canonical: false }),
    ]);
    renderPanel(<CompanyReportDocumentsPanel companyId="company_gpw_cdr" />);

    // Flat list so every kind renders regardless of period grouping/folding.
    await screen.findByText("Group by period");
    await userEvent.click(screen.getByLabelText("Group by period"));

    const rows = within(screen.getByRole("list"));
    expect(rows.getByText("Consolidated report")).toBeInTheDocument();
    expect(rows.getByText("Standalone report")).toBeInTheDocument();
    expect(rows.getByText("Audit report")).toBeInTheDocument();
    expect(rows.getByText("Presentation")).toBeInTheDocument();
    expect(rows.getByText("Governance")).toBeInTheDocument();
    expect(rows.getByText("Other")).toBeInTheDocument();
  });

  it("narrows the visible rows when a document type is selected", async () => {
    mockView([
      viewRow({ id: "d_ssf", title: "Consolidated 2026", docKind: "periodic_ssf" }),
      viewRow(
        { id: "d_pres", title: "Investor deck", docKind: "presentation" },
        { canonical: false },
      ),
    ]);
    renderPanel(<CompanyReportDocumentsPanel companyId="company_gpw_cdr" />);
    // Flat list to avoid fold interference for the presentation companion.
    await screen.findByText("Group by period");
    await userEvent.click(screen.getByLabelText("Group by period"));

    expect(screen.getByText(/Consolidated 2026/)).toBeInTheDocument();
    expect(screen.getByText(/Investor deck/)).toBeInTheDocument();

    await userEvent.selectOptions(screen.getByLabelText("Document type"), "presentation");

    expect(screen.queryByText(/Consolidated 2026/)).not.toBeInTheDocument();
    expect(screen.getByText(/Investor deck/)).toBeInTheDocument();
  });

  it("matches unclassified documents with the Unclassified filter option", async () => {
    mockView([
      viewRow({ id: "d_ssf", title: "Consolidated 2026", docKind: "periodic_ssf" }),
      viewRow({ id: "d_null", title: "Mystery filing", docKind: null }, { canonical: false }),
    ]);
    renderPanel(<CompanyReportDocumentsPanel companyId="company_gpw_cdr" />);
    await screen.findByText("Group by period");
    await userEvent.click(screen.getByLabelText("Group by period"));

    expect(screen.getByText(/Consolidated 2026/)).toBeInTheDocument();
    await userEvent.selectOptions(screen.getByLabelText("Document type"), "unclassified");

    expect(screen.queryByText(/Consolidated 2026/)).not.toBeInTheDocument();
    expect(screen.getByText(/Mystery filing/)).toBeInTheDocument();
  });

  it("refreshes classification: invokes the command, reloads the list, and toasts the count", async () => {
    reclassifyReportDocumentsMock.mockResolvedValue({
      total: 4,
      updated: 3,
      byKind: { periodic_ssf: 2, other: 2 },
    });
    renderPanel(<CompanyReportDocumentsPanel companyId="company_gpw_cdr" />);

    await screen.findByRole("link");
    expect(getReportDocumentsViewMock).toHaveBeenCalledTimes(1);

    const button = await screen.findByRole("button", { name: "Refresh classification" });
    await userEvent.click(button);

    expect(reclassifyReportDocumentsMock).toHaveBeenCalledTimes(1);
    expect(await screen.findByText("Classified documents: 3 of 4")).toBeInTheDocument();
    await waitFor(() => expect(getReportDocumentsViewMock).toHaveBeenCalledTimes(2));
  });

  // Bug 3579234: reclassification changes doc_kind, which the coverage read
  // model consumes — it must invalidate the sibling coverage/fundamentals panels
  // so they don't show stale kinds until a remount.
  it("fires onExtracted on reclassify success so the coverage pane refetches", async () => {
    reclassifyReportDocumentsMock.mockResolvedValue({
      total: 4,
      updated: 3,
      byKind: { periodic_ssf: 2, other: 2 },
    });
    const onExtracted = vi.fn();
    renderPanel(
      <CompanyReportDocumentsPanel companyId="company_gpw_cdr" onExtracted={onExtracted} />,
    );

    const button = await screen.findByRole("button", { name: "Refresh classification" });
    await userEvent.click(button);

    await waitFor(() => expect(onExtracted).toHaveBeenCalledTimes(1));
  });

  it("surfaces a reclassification error on the toast", async () => {
    reclassifyReportDocumentsMock.mockRejectedValue(new Error("reclassify failed"));
    renderPanel(<CompanyReportDocumentsPanel companyId="company_gpw_cdr" />);

    const button = await screen.findByRole("button", { name: "Refresh classification" });
    await userEvent.click(button);

    expect(await screen.findByText("reclassify failed")).toBeInTheDocument();
  });
});
