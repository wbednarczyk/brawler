import { describe, it, expect, vi } from "vitest";
import { render, screen } from "@testing-library/react";
import type { Company } from "../../api/types";
import type { AnalystRecommendationsView } from "../../api/analystRecommendations";
import { AnalystRecommendationsSection } from "../../shared/components/AnalystRecommendationsSection";
import wireFixture from "./analystRecommendationsPopulated.json";

// Cross-language populated-path contract (v0.58 A3, ADR 0073; mirrors the
// ownership T6 fixture). The fixture is the EXACT wire JSON the Rust command
// emits (minus the wall-clock lastRefreshedAt) — pinned by
// `commands::analyst_recommendations::tests::populated_view_matches_the_shared_wire_fixture`.
// Rendering it through the real component (typed as the generated DTO) closes the
// gap the mock-fidelity corpus (empty-only) leaves: a populated-branch drift
// (field casing, null vs omitted, direction/prev derivation) would otherwise pass
// silently. If the fixture disagrees with the generated type, `tsc` reddens; if a
// value stops rendering, this reddens; if the Rust wire format moves, Rust reddens.
describe("populated AnalystRecommendationsView wire contract (v0.58 A3)", () => {
  const data: AnalystRecommendationsView = wireFixture;
  const company = {
    id: "company_gpw_rec",
    qualifiedTicker: "GPW:REC",
    displayName: "REC S.A.",
  } as Company;

  it("the real Rust wire JSON renders through AnalystRecommendationsSection", () => {
    render(
      <AnalystRecommendationsSection
        company={company}
        view={data}
        error={null}
        loading={false}
        onRetry={vi.fn()}
      />,
    );

    // Every firm in the history renders; ratings stay verbatim.
    expect(screen.getAllByText("Noble Securities").length).toBeGreaterThan(0);
    expect(screen.getByText("BM mBank")).toBeInTheDocument();
    expect(screen.getByText("akumuluj")).toBeInTheDocument();
    // The upgrade carries its derived same-firm prior rating.
    expect(screen.getByText(/▲ from trzymaj/)).toBeInTheDocument();
    // Attribution inseparable from the latest target (firm + local date).
    expect(screen.getByText(/Noble Securities · 18\.06\.2026/)).toBeInTheDocument();
  });

  it("the wire fixture carries the pinned decimal + attribution conventions", () => {
    // Targets are decimal-exact TEXT (never floats); optionals are null, not absent.
    expect(data.latestTarget?.targetPrice).toBe("250.00");
    expect(typeof data.entries[0].targetPrice).toBe("string");
    expect(data.entries[2].targetPrice).toBeNull();
    expect(data.entries[2].analyst).toBeNull();
    // The populated payload keys by the deterministic derived company id.
    expect(data.companyId).toBe("company_gpw_rec");
  });
});
