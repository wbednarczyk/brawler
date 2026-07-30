import { describe, it, expect, vi, beforeEach } from "vitest";
import { render, screen } from "@testing-library/react";

import { CoverageFlaggedFacts } from "./CoverageFlaggedFacts";
import { listFlaggedFactProvenance } from "../../api/fundamentalsExtraction";
import type { FlaggedFact } from "../../api/fundamentalsExtraction";

vi.mock("../../api/fundamentalsExtraction", () => ({
  listFlaggedFactProvenance: vi.fn(),
}));

const listFlaggedFactProvenanceMock = vi.mocked(listFlaggedFactProvenance);

function flaggedFact(overrides: Partial<FlaggedFact> = {}): FlaggedFact {
  return {
    factId: "fact_1",
    companyId: "company_gpw_cdr",
    metricKey: "total_assets",
    label: "Total assets",
    valueNumeric: "45000000",
    currency: "PLN",
    fiscalYear: 2025,
    periodType: "FY",
    sourceTier: "esef",
    validationStatus: "flagged",
    driftJson: null,
    citation: "ifrs-full:Assets",
    ...overrides,
  };
}

describe("CoverageFlaggedFacts", () => {
  beforeEach(() => {
    vi.clearAllMocks();
    listFlaggedFactProvenanceMock.mockResolvedValue([]);
  });

  it("lists each flagged figure with its period, metric, value, reader and source label", async () => {
    listFlaggedFactProvenanceMock.mockResolvedValue([
      flaggedFact(),
      flaggedFact({
        factId: "fact_2",
        metricKey: "revenue",
        label: "Revenue",
        valueNumeric: "12000000",
        periodType: "Q3",
        sourceTier: "html_aggregator",
        citation: null,
      }),
    ]);
    render(<CoverageFlaggedFacts companyId="company_gpw_cdr" />);

    // The read is company-scoped — a global list in a company panel would show
    // another company's figures.
    expect(await screen.findByText(/Total assets/)).toBeInTheDocument();
    expect(listFlaggedFactProvenanceMock).toHaveBeenCalledWith("company_gpw_cdr");
    const first = screen.getByTitle(/2025 FY · Total assets/);
    expect(first).toHaveTextContent("45 M PLN");
    expect(screen.getByTitle(/2025 Q3 · Revenue/)).toHaveTextContent("12 M PLN");
    // The reader that produced each value is named, never a raw tier token.
    expect(screen.getByText("ESEF")).toBeInTheDocument();
    expect(screen.getByText("Web cross-check")).toBeInTheDocument();
    expect(screen.queryByText("html_aggregator")).not.toBeInTheDocument();
    // The citation travels with the value (attribution stays visible).
    expect(screen.getByText("ifrs-full:Assets")).toBeInTheDocument();
  });

  it("shows a reassuring empty state when nothing is flagged", async () => {
    listFlaggedFactProvenanceMock.mockResolvedValue([]);
    render(<CoverageFlaggedFacts companyId="company_gpw_cdr" />);

    expect(
      await screen.findByText("No flagged figures — every recorded value passed its checks."),
    ).toBeInTheDocument();
  });

  it("shows an explicit, retryable error when the read fails — never the empty state", async () => {
    listFlaggedFactProvenanceMock.mockRejectedValueOnce(new Error("read exploded"));
    render(<CoverageFlaggedFacts companyId="company_gpw_cdr" />);

    expect(await screen.findByText("Couldn't load flagged figures.")).toBeInTheDocument();
    expect(screen.getByText("read exploded")).toBeInTheDocument();
    expect(
      screen.queryByText("No flagged figures — every recorded value passed its checks."),
    ).not.toBeInTheDocument();
    expect(screen.getByRole("button", { name: "Retry" })).toBeInTheDocument();
  });
});
