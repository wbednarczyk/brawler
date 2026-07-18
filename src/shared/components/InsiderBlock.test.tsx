import { describe, it, expect } from "vitest";
import { render, screen } from "@testing-library/react";

import { InsiderBlock } from "./InsiderBlock";
import type { InsiderOverview, WindowAggregate } from "../../api/insider";

const COMPUTED: WindowAggregate = {
  state: "computed",
  count: 3,
  buys: 2,
  sells: 1,
  undetermined: 0,
  net: 1,
  buyVolume: "5000",
  sellVolume: "1500",
  volumeKnown: 2,
  volumeTotal: 3,
};

const POPULATED: InsiderOverview = {
  companyId: "company_gpw_gtn",
  holdings: [
    {
      person: "Jakub Dwernicki",
      role: "management",
      shares: null,
      indirectVia: "Dwernicki Fundacja Rodzinna",
      asOf: "2025-12-31",
    },
    {
      person: "Anna Nowak",
      role: "supervisory",
      shares: "12000",
      indirectVia: null,
      asOf: "2025-12-31",
    },
  ],
  transactions: [
    {
      id: "tx1",
      person: "Jakub Dwernicki",
      role: "management",
      relatedPdmr: null,
      direction: "buy",
      instrument: "shares",
      volume: "5000",
      price: "24.80",
      currency: "PLN",
      txDate: "2026-05-20",
      effectiveDate: "2026-05-20",
      dateSource: "transaction",
      feedItemId: "feed1",
      sourceUrl: "https://example.test/filing-1",
    },
    {
      id: "tx2",
      person: "Dwernicki Fundacja Rodzinna",
      role: "closely_associated",
      relatedPdmr: "Jakub Dwernicki",
      direction: "buy",
      instrument: "shares",
      volume: null,
      price: null,
      currency: null,
      txDate: null,
      effectiveDate: "2026-04-30",
      dateSource: "filing",
      feedItemId: "feed2",
      sourceUrl: null,
    },
  ],
  window90d: COMPUTED,
  window12m: COMPUTED,
};

describe("InsiderBlock", () => {
  it("renders a calm empty state when nothing is parsed", () => {
    render(
      <InsiderBlock
        data={{
          companyId: "c1",
          transactions: [],
          holdings: [],
          window90d: { state: "belowMinimum", count: 0 },
          window12m: { state: "belowMinimum", count: 0 },
        }}
      />,
    );
    expect(screen.getByText(/No insider filings parsed/i)).toBeInTheDocument();
  });

  it("renders the computed aggregate strip for both windows with the coverage note", () => {
    render(<InsiderBlock data={POPULATED} />);
    expect(screen.getByText("Last 90 days")).toBeInTheDocument();
    expect(screen.getByText("Last 12 months")).toBeInTheDocument();
    // Net readout with the buys/sells split.
    expect(screen.getAllByText("+1").length).toBeGreaterThan(0);
    expect(screen.getAllByText(/2 buys · 1 sells/).length).toBe(2);
    // Coverage note (2 of 3 disclosed a volume).
    expect(screen.getAllByText(/volume known for 2\/3/).length).toBe(2);
  });

  it("renders holdings with the via-vehicle and the explicit not-stated count", () => {
    render(<InsiderBlock data={POPULATED} />);
    expect(screen.getAllByText("Jakub Dwernicki").length).toBeGreaterThan(0);
    expect(screen.getByText(/via Dwernicki Fundacja Rodzinna/)).toBeInTheDocument();
    expect(screen.getByText(/count not stated/)).toBeInTheDocument();
    // A real share count is shown for the other person.
    expect(screen.getByText(/12,?000/)).toBeInTheDocument();
  });

  it("renders the transaction timeline with direction labels and a filing-date marker", () => {
    render(<InsiderBlock data={POPULATED} />);
    // Two buys → two 'buy' direction labels in the timeline.
    expect(screen.getAllByText("buy").length).toBeGreaterThan(0);
    // The attachment-only transaction states its figures are pending.
    expect(screen.getByText(/figures in the attachment/)).toBeInTheDocument();
    // The filing-dated transaction labels its date honestly.
    expect(screen.getByText(/filing date/)).toBeInTheDocument();
    // The closely-associated anchor is shown.
    expect(screen.getByText(/for Jakub Dwernicki/)).toBeInTheDocument();
  });

  it("shows the below-minimum state and NO aggregate under 2 transactions", () => {
    render(
      <InsiderBlock
        data={{
          companyId: "c1",
          transactions: POPULATED.transactions.slice(0, 1),
          holdings: [],
          window90d: { state: "belowMinimum", count: 1 },
          window12m: { state: "belowMinimum", count: 1 },
        }}
      />,
    );
    expect(
      screen.getAllByText(/too few for an aggregate/i).length,
    ).toBeGreaterThan(0);
    // No net readout renders below the minimum.
    expect(screen.queryByText(/buys · /)).not.toBeInTheDocument();
  });

  it("surfaces the undetermined bucket without hiding it in the net", () => {
    const withUndetermined: WindowAggregate = {
      state: "computed",
      count: 3,
      buys: 1,
      sells: 0,
      undetermined: 2,
      net: 1,
      buyVolume: null,
      sellVolume: null,
      volumeKnown: 0,
      volumeTotal: 3,
    };
    render(
      <InsiderBlock
        data={{
          ...POPULATED,
          window90d: withUndetermined,
          window12m: { state: "belowMinimum", count: 0 },
        }}
      />,
    );
    expect(screen.getByText(/2 undetermined/)).toBeInTheDocument();
    // Empty window states its emptiness explicitly.
    expect(screen.getByText(/No transactions in this window/)).toBeInTheDocument();
  });
});
