import { describe, it, expect } from "vitest";
import { render, screen } from "@testing-library/react";
import type { Company } from "../../api/types";
import type { ShortPositionsView } from "../../api/shortPositions";
import { ShortPositionsSection } from "./ShortPositionsSection";

const company = { id: "c1", qualifiedTicker: "GPW:CDR", displayName: "CD PROJEKT S.A." } as Company;

const populated: ShortPositionsView = {
  positions: [
    {
      holderName: "Qube Research & Technologies Ltd",
      netPositionPct: 1.81,
      positionDate: "2026-06-01",
      recentlyChanged: true,
    },
    {
      holderName: "Marshall Wace LLP",
      netPositionPct: 0.59,
      positionDate: "2026-04-15",
      recentlyChanged: false,
    },
  ],
  events: [
    { kind: "increased", holderName: "Qube Research & Technologies Ltd", fromPct: 1.49, toPct: 1.81, positionDate: "2026-06-01" },
    { kind: "exited", holderName: "AQR Capital Management", fromPct: 0.55, toPct: null, positionDate: "2026-05-12" },
  ],
  lastExit: null,
  aggregatePct: 2.4,
  delta30dPp: 1.26,
  registerUpdatedAt: "2026-07-15T06:30:00Z",
};

const emptyView: ShortPositionsView = {
  positions: [],
  events: [],
  lastExit: { holderName: "Point72 Asset Management", exitedOn: "2024-11-03" },
  aggregatePct: 0,
  delta30dPp: 0,
  registerUpdatedAt: null,
};

describe("ShortPositionsSection (v0.55 T4b, ADR 0069 decision 3)", () => {
  it("renders active holders, the aggregate, and history phrased by kind", () => {
    render(<ShortPositionsSection company={company} view={populated} error={null} />);

    // Attribution carries the register's last refresh (v0.55 follow-up).
    expect(screen.getByText(/updated 2026-07-15/)).toBeInTheDocument();

    // Summary tiles.
    expect(screen.getByText("Total net short position")).toBeInTheDocument();
    expect(screen.getByText("2.40%")).toBeInTheDocument();
    expect(screen.getByText("Change / 30 days")).toBeInTheDocument();

    // Positions table lists each holder; only the recently-changed one gets the chip.
    // Qube also appears in the history row, so it shows up more than once.
    expect(screen.getAllByText("Qube Research & Technologies Ltd").length).toBeGreaterThan(0);
    expect(screen.getByText("Marshall Wace LLP")).toBeInTheDocument();
    expect(screen.getByText("changed")).toBeInTheDocument();

    // Change history is phrased by kind (Increased / Exited).
    expect(screen.getByText("Increased")).toBeInTheDocument();
    expect(screen.getByText("Exited")).toBeInTheDocument();
    expect(screen.getByText("1.49% → 1.81%")).toBeInTheDocument();
  });

  it("renders the empty state with the last register presence", () => {
    render(<ShortPositionsSection company={company} view={emptyView} error={null} />);

    expect(screen.getByText("No registered short positions")).toBeInTheDocument();
    expect(screen.getByText(/Last presence in the register/)).toBeInTheDocument();
    expect(screen.getByText(/Point72 Asset Management/)).toBeInTheDocument();
    // No positions table header in the empty state.
    expect(screen.queryByText("Position holder")).not.toBeInTheDocument();
  });
});
