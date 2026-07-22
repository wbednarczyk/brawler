import { describe, it, expect, vi } from "vitest";
import { render, screen } from "@testing-library/react";
import type { OwnershipOverview } from "../../api/ownership";
import { OwnershipSection } from "../../shared/components/OwnershipSection";
import wireFixture from "./ownershipOverviewPopulated.json";

// Cross-language populated-path contract (v0.56 T6, mirrors the price-context
// M1 fixture). The fixture is the EXACT wire JSON the Rust command emits —
// pinned byte-for-byte by
// `commands::ownership::tests::populated_ownership_overview_matches_the_shared_wire_fixture`.
// Rendering it through the real component (typed as the generated DTO) closes
// the gap the mock-fidelity corpus leaves: the corpus only covers the empty
// state, so a populated-branch drift (field casing, omitted optionals, series
// grouping) would otherwise pass the gate silently. If this file's shape ever
// disagrees with the generated type, `tsc` reddens; if a value stops rendering,
// this test reddens; if the Rust wire format moves, the Rust side reddens.
describe("populated OwnershipOverview wire contract (v0.56 T6)", () => {
  const data: OwnershipOverview = wireFixture;
  const noops = {
    onBackfill: vi.fn(),
    onSetHolderType: vi.fn(),
  };

  it("the real Rust wire JSON renders through OwnershipSection", () => {
    render(<OwnershipSection data={data} {...noops} />);

    expect(screen.getByRole("heading", { name: "Ownership" })).toBeInTheDocument();
    expect(
      screen.getByRole("img", { name: "Ownership structure by holder type" }),
    ).toBeInTheDocument();
    // Current holders render (Jacek Duch also appears as a trajectory caption).
    expect(screen.getAllByText("Jacek Duch").length).toBeGreaterThan(0);
    expect(screen.getByText("NN PTE")).toBeInTheDocument();
    expect(screen.getByText("Itema Ventures UAB")).toBeInTheDocument();
    // ADR 0084 clean cut: the holder-type proposal review is gone. An
    // unclassified holder renders as a normal stake row the user can re-type.
    expect(screen.queryByText("type? to confirm")).not.toBeInTheDocument();
    expect(
      screen.getByRole("button", { name: "Change type: Itema Ventures UAB" }),
    ).toBeInTheDocument();
  });

  it("the wire fixture carries the pinned decimal-exact conventions", () => {
    // Percentages are decimal-exact TEXT (never floats), and free float is derived.
    expect(data.freeFloatPct).toBe("68.5");
    expect(data.disclosedSum).toBe("31.5");
    expect(typeof data.freeFloatPct).toBe("string");
    // Absent optionals are omitted, never null-filled (serde skip).
    expect("holderType" in data.holders[0]).toBe(false);
    // A populated payload keys by the deterministic derived company id.
    expect(data.companyId).toBe("company_gpw_cbf");
  });
});
