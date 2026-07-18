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
        { asOf: "2024-12-31", capitalPct: "24.0" },
        { asOf: "2025-12-31", capitalPct: "25.5" },
      ],
    },
  ],
  residuals: [],
  pendingProposals: [
    {
      id: "prop1",
      holderKey: "ITEMA VENTURES UAB",
      proposedType: "other_institutional",
      confidence: 0.7,
      rationale: "foreign fund",
    },
  ],
  ocrProposals: [],
};

const NOOPS = {
  onBackfill: vi.fn(),
  onRunClassification: vi.fn(),
  onSetHolderType: vi.fn(),
  onConfirmProposal: vi.fn(),
  onRejectProposal: vi.fn(),
  onRunOcr: vi.fn(),
  onConfirmOcrProposal: vi.fn(),
  onRejectOcrProposal: vi.fn(),
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
        data={{ ...OVERVIEW, holders: [], history: [], residuals: [], pendingProposals: [] }}
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

  it("shows a pending AI proposal with confirm/reject wired to callbacks", async () => {
    const user = userEvent.setup();
    const onConfirmProposal = vi.fn();
    const onRejectProposal = vi.fn();
    render(
      <OwnershipSection
        data={OVERVIEW}
        {...NOOPS}
        onConfirmProposal={onConfirmProposal}
        onRejectProposal={onRejectProposal}
      />,
    );
    expect(screen.getByText("type? to confirm")).toBeInTheDocument();
    await user.click(screen.getByRole("button", { name: "Confirm classification" }));
    expect(onConfirmProposal).toHaveBeenCalledWith("prop1");
    await user.click(screen.getByRole("button", { name: "Reject classification" }));
    expect(onRejectProposal).toHaveBeenCalledWith("prop1");
  });

  it("surfaces the residual warnbox with a Run OCR action for an eligible residual", async () => {
    const user = userEvent.setup();
    const onRunOcr = vi.fn();
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
        onRunOcr={onRunOcr}
      />,
    );
    expect(screen.getByText(/unreadable text layer/i)).toBeInTheDocument();
    await user.click(screen.getByRole("button", { name: "Read with OCR" }));
    expect(onRunOcr).toHaveBeenCalledTimes(1);
  });

  it("shows a running state and disables the OCR action while busy", () => {
    render(
      <OwnershipSection
        data={{
          ...OVERVIEW,
          residuals: [{ reportDocumentId: "doc1", parseState: "glyph_encoded" }],
        }}
        {...NOOPS}
        busy
      />,
    );
    const button = screen.getByRole("button", { name: "Reading with OCR…" });
    expect(button).toBeDisabled();
  });

  it("notes a rejected residual and offers no run action for it", () => {
    render(
      <OwnershipSection
        data={{
          ...OVERVIEW,
          residuals: [
            { reportDocumentId: "doc1", parseState: "glyph_encoded", ocrState: "rejected" },
          ],
        }}
        {...NOOPS}
      />,
    );
    expect(screen.getByText(/rejected — not re-proposed/i)).toBeInTheDocument();
    expect(screen.queryByRole("button", { name: "Read with OCR" })).not.toBeInTheDocument();
  });

  it("renders an OCR shareholders-table proposal with per-holder rows and confirm/reject", async () => {
    const user = userEvent.setup();
    const onConfirmOcrProposal = vi.fn();
    const onRejectOcrProposal = vi.fn();
    render(
      <OwnershipSection
        data={{
          ...OVERVIEW,
          residuals: [{ reportDocumentId: "doc1", parseState: "glyph_encoded", ocrState: "proposed" }],
          ocrProposals: [
            {
              reportDocumentId: "doc1",
              sourceDocumentId: "doc1_pdf",
              asOf: "2023-12-31",
              matchedHeading: "Akcjonariusze",
              providerId: "mistral",
              holders: [
                { holderNameRaw: "Jan Kowalski", capitalPct: "12.34", votesPct: "15.00" },
                { holderNameRaw: "Aviva OFE", capitalPct: "9.88", votesPct: "7.41" },
              ],
            },
          ],
        }}
        {...NOOPS}
        onConfirmOcrProposal={onConfirmOcrProposal}
        onRejectOcrProposal={onRejectOcrProposal}
      />,
    );
    expect(screen.getByText("OCR — confirm to save")).toBeInTheDocument();
    // Both proposed holder rows render.
    expect(screen.getByText("Jan Kowalski")).toBeInTheDocument();
    expect(screen.getByText("Aviva OFE")).toBeInTheDocument();
    await user.click(screen.getByRole("button", { name: "Confirm and save" }));
    expect(onConfirmOcrProposal).toHaveBeenCalledWith("doc1");
    await user.click(screen.getByRole("button", { name: "Reject" }));
    expect(onRejectOcrProposal).toHaveBeenCalledWith("doc1");
  });

  it("offers the AI classification trigger only for unclassified holders without a proposal", async () => {
    const user = userEvent.setup();
    const onRunClassification = vi.fn();

    // Base overview: Itema is unclassified but already has a pending proposal —
    // no trigger (the proposal row carries the confirm/reject actions).
    const { unmount } = render(
      <OwnershipSection data={OVERVIEW} {...NOOPS} onRunClassification={onRunClassification} />,
    );
    expect(
      screen.queryByRole("button", { name: "Classify unknown holders (AI)" }),
    ).not.toBeInTheDocument();
    unmount();

    // Same overview without proposals: the unclassified holder exposes the
    // explicit, user-initiated AI trigger (ADR 0072 §3 — proposals only).
    render(
      <OwnershipSection
        data={{ ...OVERVIEW, pendingProposals: [] }}
        {...NOOPS}
        onRunClassification={onRunClassification}
      />,
    );
    await user.click(screen.getByRole("button", { name: "Classify unknown holders (AI)" }));
    expect(onRunClassification).toHaveBeenCalledTimes(1);
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
