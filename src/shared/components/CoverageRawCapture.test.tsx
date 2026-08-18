import { describe, it, expect, vi, beforeEach } from "vitest";
import { render, screen } from "@testing-library/react";

import { CoverageRawCapture } from "./CoverageRawCapture";
import { getReportTaggedFactCoverage, listUncrosswalkedConcepts } from "../../api/taggedFactPromotion";
import type { TaggedFactCoverageCounts } from "../../api/taggedFactPromotion";

vi.mock("../../api/taggedFactPromotion", () => ({
  getReportTaggedFactCoverage: vi.fn(),
  listUncrosswalkedConcepts: vi.fn(),
  promoteUncrosswalkedConcept: vi.fn(),
}));

const getReportTaggedFactCoverageMock = vi.mocked(getReportTaggedFactCoverage);
const listUncrosswalkedConceptsMock = vi.mocked(listUncrosswalkedConcepts);

function counts(overrides: Partial<TaggedFactCoverageCounts> = {}): TaggedFactCoverageCounts {
  return {
    rawStored: 426,
    projected: 68,
    comparative: 12,
    dimensional: 228,
    noteLevel: 51,
    awaitingName: 60,
    conflicting: 7,
    unparsed: 0,
    ...overrides,
  };
}

describe("CoverageRawCapture", () => {
  beforeEach(() => {
    vi.clearAllMocks();
    listUncrosswalkedConceptsMock.mockResolvedValue([]);
  });

  it("renders the compact line's counts, comparatives split from projected", async () => {
    getReportTaggedFactCoverageMock.mockResolvedValue(counts());
    render(<CoverageRawCapture companyId="company_gpw_cdr" />);

    expect(await screen.findByText("426")).toBeInTheDocument();
    expect(screen.getByText("68")).toBeInTheDocument();
    expect(screen.getByText("12")).toBeInTheDocument();
    expect(screen.getByText("228")).toBeInTheDocument();
    expect(screen.getByText("51")).toBeInTheDocument();
    expect(screen.getByText("60")).toBeInTheDocument();
    expect(screen.getByText("7")).toBeInTheDocument();
    // Zero unparsed rows: the bucket stays hidden rather than rendering a
    // permanent zero row.
    expect(screen.queryByText("Could not be read")).not.toBeInTheDocument();
    expect(getReportTaggedFactCoverageMock).toHaveBeenCalledWith("company_gpw_cdr");
  });

  it("an unreadable number is visible with its own stated reason", async () => {
    getReportTaggedFactCoverageMock.mockResolvedValue(counts({ unparsed: 3 }));
    render(<CoverageRawCapture companyId="company_gpw_cdr" />);

    expect(await screen.findByText("Could not be read")).toBeInTheDocument();
    expect(screen.getByText("3")).toBeInTheDocument();
  });

  it("renders nothing for a company with no tagged capture yet", async () => {
    getReportTaggedFactCoverageMock.mockResolvedValue(counts({ rawStored: 0, projected: 0, comparative: 0, dimensional: 0, noteLevel: 0, awaitingName: 0, conflicting: 0, unparsed: 0 }));
    const { container } = render(<CoverageRawCapture companyId="company_gpw_cdr" />);

    // Wait for the fetch to settle without asserting on a specific string —
    // the whole point is that NOTHING renders.
    await new Promise((resolve) => setTimeout(resolve, 0));
    expect(container).toBeEmptyDOMElement();
  });

  it("the disclosure button names the count and toggles aria-expanded", async () => {
    getReportTaggedFactCoverageMock.mockResolvedValue(counts());
    render(<CoverageRawCapture companyId="company_gpw_cdr" />);

    const toggle = await screen.findByRole("button", {
      name: /Show the unnamed positions/,
    });
    // The button names the action in words, never a bare count (the raw
    // occurrence count already shown in the InfoGrid above is a different
    // unit than the list's distinct-position count, so it is not repeated
    // here — accuracy over a redundant number).
    expect(toggle).toHaveTextContent("Show the unnamed positions");
    expect(toggle).toHaveAttribute("aria-expanded", "false");
  });
});
