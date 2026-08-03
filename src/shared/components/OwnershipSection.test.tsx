import { describe, it, expect, vi } from "vitest";
import { render, screen } from "@testing-library/react";
import userEvent from "@testing-library/user-event";

import { OwnershipSection } from "./OwnershipSection";
import type { OwnershipOverview } from "../../api/ownership";

const OVERVIEW: OwnershipOverview = {
  companyId: "company_gpw_cbf",
  asOf: "2025-12-31",
  source: "report_document",
  freeFloatPct: "46.8",
  freeFloatHistory: [
    { asOf: "2024-12-31", pct: "49.1" },
    { asOf: "2025-12-31", pct: "46.8" },
  ],
  disclosedSum: "53.2",
  holders: [
    {
      holderKey: "JACEK DUCH",
      name: "Jacek Duch",
      holderType: "founder_insider",
      capitalPct: "25.5",
      votesPct: "25.5",
      asOf: "2025-12-31",
      source: "report_document",
    },
    {
      holderKey: "NN PTE",
      name: "NN PTE",
      holderType: "ofe_pension",
      capitalPct: "6.0",
      votesPct: "6.0",
      asOf: "2025-12-31",
      source: "report_document",
    },
    {
      holderKey: "ITEMA VENTURES UAB",
      name: "Itema Ventures UAB",
      capitalPct: "5.5",
      votesPct: "5.5",
      asOf: "2025-12-31",
      source: "report_document",
    },
  ],
  history: [
    {
      holderKey: "JACEK DUCH",
      name: "Jacek Duch",
      holderType: "founder_insider",
      points: [
        // Disclosed by an ESPI filing → a threshold crossing the chart marks.
        { asOf: "2024-12-31", capitalPct: "24.0", source: "espi_filing" },
        { asOf: "2025-12-31", capitalPct: "25.5", source: "report_document" },
      ],
    },
  ],
  residuals: [],
};

const NOOPS = {
  onBackfill: vi.fn(),
  onSetHolderType: vi.fn(),
};

describe("OwnershipSection (v0.56 T6, ADR 0072)", () => {
  it("renders the heading in every state and a skeleton while loading", () => {
    render(<OwnershipSection data={null} {...NOOPS} />);
    expect(screen.getByRole("heading", { name: "Ownership" })).toBeInTheDocument();
    // No holders rendered while data is null.
    expect(screen.queryByText("Jacek Duch")).not.toBeInTheDocument();
  });

  it("shows the backfill CTA in the empty state and fires onBackfill", async () => {
    const user = userEvent.setup();
    const onBackfill = vi.fn();
    render(
      <OwnershipSection
        data={{ ...OVERVIEW, holders: [], history: [], residuals: [] }}
        {...NOOPS}
        onBackfill={onBackfill}
      />,
    );
    const button = screen.getByRole("button", { name: "Extract from reports" });
    await user.click(button);
    expect(onBackfill).toHaveBeenCalledTimes(1);
  });

  it("renders the donut, holders, and derived free-float in the populated state", () => {
    render(<OwnershipSection data={OVERVIEW} {...NOOPS} />);
    expect(
      screen.getByRole("img", { name: "Ownership structure by holder type" }),
    ).toBeInTheDocument();
    expect(screen.getAllByText("Jacek Duch").length).toBeGreaterThan(0);
    expect(screen.getByText("NN PTE")).toBeInTheDocument();
    // Founder type chip label surfaces (legend + row).
    expect(screen.getAllByText(/Founders?\/insiders?/).length).toBeGreaterThan(0);
  });

  // ADR 0072 decision 5: threshold crossings are markers on the stake-over-time
  // chart. A holder files an ESPI major-holdings notification only when it
  // crosses a statutory band, so an `espi_filing`-sourced point IS the crossing —
  // and a periodic-report point beside it must NOT be marked, or every sample
  // would read as an event.
  it("marks only the ESPI-sourced points on the stakes-over-time chart", () => {
    const { container } = render(<OwnershipSection data={OVERVIEW} {...NOOPS} />);

    const markers = container.querySelectorAll("line.ui-multi-line-marker");
    expect(markers).toHaveLength(1);
    expect(markers[0].querySelector("title")?.textContent).toContain(
      "Jacek Duch — threshold crossing",
    );
    expect(markers[0].querySelector("title")?.textContent).toContain("2024-12-31");
  });

  it("marks nothing when every disclosure is a periodic report", () => {
    const noFilings: OwnershipOverview = {
      ...OVERVIEW,
      history: [
        {
          ...OVERVIEW.history[0],
          points: OVERVIEW.history[0].points.map((point) => ({
            ...point,
            source: "report_document",
          })),
        },
      ],
    };
    const { container } = render(<OwnershipSection data={noFilings} {...NOOPS} />);
    expect(container.querySelectorAll("line.ui-multi-line-marker")).toHaveLength(0);
  });

  // ADR 0084 decision 4: tier-4 OCR is retired. An unreadable document is a
  // FLAGGED gap the user can see — never silently absent, never guessed — but
  // the panel offers no OCR run action any more.
  it("flags an unreadable residual document with no OCR run action (ADR 0084)", () => {
    render(
      <OwnershipSection
        data={{
          ...OVERVIEW,
          residuals: [
            {
              reportDocumentId: "doc1",
              parseState: "glyph_encoded",
              detectedAsOf: "2023-12-31",
              matchedHeading: "Akcjonariat",
            },
          ],
        }}
        {...NOOPS}
      />,
    );
    expect(screen.getByText(/unreadable text layer/i)).toBeInTheDocument();
    for (const name of ["Read with OCR", "Retry OCR", "Reading with OCR…"]) {
      expect(screen.queryByRole("button", { name })).not.toBeInTheDocument();
    }
  });

  // ADR 0084 clean cut: the AI holder-type classifier AND every proposal it
  // stored are gone. Holder types stay fully user-editable; no proposal review
  // surface remains.
  it("offers no AI classification or proposal review surface (ADR 0084)", () => {
    render(<OwnershipSection data={OVERVIEW} {...NOOPS} />);

    expect(
      screen.queryByRole("button", { name: "Classify unknown holders (AI)" }),
    ).not.toBeInTheDocument();
    expect(screen.queryByText("type? to confirm")).not.toBeInTheDocument();
    for (const name of ["Confirm classification", "Reject classification", "Confirm and save"]) {
      expect(screen.queryByRole("button", { name })).not.toBeInTheDocument();
    }

    // The manual re-type path — the ONLY classification path now — still works.
    expect(
      screen.getByRole("button", { name: "Change type: Jacek Duch" }),
    ).toBeInTheDocument();
  });

  it("labels the basis source per its actual value", () => {
    // Aggregator (BiznesRadar) basis: the header meta must name it, never the
    // hardcoded "periodic report" (ADR 0072 amended 2026-07-16).
    const { unmount } = render(
      <OwnershipSection data={{ ...OVERVIEW, source: "aggregator" }} {...NOOPS} />,
    );
    expect(screen.getByText(/BiznesRadar/)).toBeInTheDocument();
    expect(screen.queryByText(/periodic report/)).not.toBeInTheDocument();
    unmount();

    // Default report_document basis keeps the periodic-report label.
    render(<OwnershipSection data={OVERVIEW} {...NOOPS} />);
    expect(screen.getByText(/periodic report/)).toBeInTheDocument();
  });

  it("re-types a holder and offers an undo that restores the previous type", async () => {
    const user = userEvent.setup();
    const onSetHolderType = vi.fn();
    render(<OwnershipSection data={OVERVIEW} {...NOOPS} onSetHolderType={onSetHolderType} />);

    // Open the inline re-type editor for the founder.
    await user.click(screen.getByRole("button", { name: "Change type: Jacek Duch" }));
    const select = screen.getByRole("combobox", { name: "Change type to" });
    await user.selectOptions(select, "parent_company");
    await user.click(screen.getByRole("button", { name: "Save" }));
    expect(onSetHolderType).toHaveBeenCalledWith("JACEK DUCH", "parent_company");

    // Undo restores the previous (founder_insider) type.
    await user.click(screen.getByRole("button", { name: "Undo" }));
    expect(onSetHolderType).toHaveBeenLastCalledWith("JACEK DUCH", "founder_insider");
  });

  it("shows the skin-in-the-game badge for a direct founder match", () => {
    const data: OwnershipOverview = {
      ...OVERVIEW,
      holders: [
        {
          ...OVERVIEW.holders[0],
          skinInTheGame: { person: "Jacek Duch" },
        },
        OVERVIEW.holders[1],
      ],
    };
    render(<OwnershipSection data={data} {...NOOPS} />);
    const badges = screen.getAllByText("skin in the game");
    expect(badges.length).toBe(1);
    expect(badges[0].closest(".ownership-skin-badge")).toHaveAttribute(
      "title",
      "Corroborated by: Jacek Duch",
    );
  });

  it("shows the badge with the vehicle detail for an indirect (via-vehicle) match", () => {
    const data: OwnershipOverview = {
      ...OVERVIEW,
      holders: [
        {
          holderKey: "MELHUS COMPANY LTD",
          name: "Melhus Company Ltd",
          holderType: "family_foundation",
          capitalPct: "24.0",
          votesPct: "24.0",
          asOf: "2025-12-31",
          source: "report_document",
          skinInTheGame: { person: "Cezary Kozielski", via: "Melhus Company Ltd" },
        },
      ],
    };
    render(<OwnershipSection data={data} {...NOOPS} />);
    expect(screen.getByText("skin in the game").closest(".ownership-skin-badge")).toHaveAttribute(
      "title",
      "Corroborated by: Cezary Kozielski (via Melhus Company Ltd)",
    );
  });

  it("shows no badge when a holder has no management/insider corroboration", () => {
    render(<OwnershipSection data={OVERVIEW} {...NOOPS} />);
    expect(screen.queryByText("skin in the game")).not.toBeInTheDocument();
  });
});
