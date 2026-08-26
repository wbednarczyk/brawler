import { describe, it, vi } from "vitest";
import { expect, renderApp, screen, userEvent, waitFor, within } from "../../test/appWorkflowHarness";
import { saveCockpitLayout } from "../../api/cockpit";

// Frozen-cockpit cross-links (F3a S3 + sol R2/R3): "Open recommendations" in
// Fundamentals and a coverage period row used to ADD a panel in place (a
// layout-structure mutation the freeze removed). They now commit a typed
// Spółka transition — this guard reddens if either lands on the core instead
// of its named tool. The vitest mock scenario carries neither price data nor
// coverage periods for CDR, so both hosted panels are stubbed to expose the
// exact callback props the cockpit wires — the regression lived in that
// wiring, not in the panels.
vi.mock("../../shared/components/CompanyCoveragePanel", () => ({
  CompanyCoveragePanel: ({ onOpenDocuments }: { onOpenDocuments?: () => void }) => (
    <button type="button" onClick={onOpenDocuments}>
      stub: open documents
    </button>
  ),
}));

vi.mock("./companyPanels", async (importOriginal) => {
  const actual = await importOriginal<typeof import("./companyPanels")>();
  return {
    ...actual,
    CockpitFundamentalsPanel: ({ onOpenRecommendations }: { onOpenRecommendations?: () => void }) => (
      <button type="button" onClick={onOpenRecommendations}>
        stub: open recommendations
      </button>
    ),
  };
});

async function openSeededView(user: ReturnType<typeof userEvent.setup>, name: string, pinned: object[]) {
  await saveCockpitLayout({
    name,
    panelsJson: JSON.stringify({
      pinned,
      openGlobals: [],
      closedLinked: ["feed", "inspector", "claims-sel", "diff-sel"],
      selectedFeedItemId: null,
      grid: null,
      cells: null,
      viewCompanyId: "company_gpw_cdr",
    }),
    layoutJson: null,
    dockviewVersion: null,
  });
  renderApp();
  await user.click(await screen.findByRole("button", { name }));
  return screen.findByLabelText("Research cockpit");
}

async function expectSpolkaTool(tool: string) {
  const spolka = await screen.findByRole("region", { name: "Company view" });
  expect(spolka).toHaveAttribute("data-company-id", "company_gpw_cdr");
  await waitFor(() => {
    expect(within(spolka).getByLabelText("Workshop tool")).toHaveAttribute("data-tool", tool);
  });
}

describe("frozen cockpit cross-links land on the named Spółka tool", () => {
  it("Fundamentals → Open recommendations opens {t:\"rekomendacje\"}", async () => {
    const user = userEvent.setup();
    const cockpit = await openSeededView(user, "Cross-link fundamentals view", [
      { id: "follow:fundamentals", kind: "fundamentals", mode: "follow" },
    ]);
    await user.click(await within(cockpit).findByRole("button", { name: "stub: open recommendations" }));
    await expectSpolkaTool("rekomendacje");
  });

  it("Coverage → Open documents opens {t:\"dokumenty\"}", async () => {
    const user = userEvent.setup();
    const cockpit = await openSeededView(user, "Cross-link coverage view", [
      { id: "follow:coverage", kind: "coverage", mode: "follow" },
    ]);
    await user.click(await within(cockpit).findByRole("button", { name: "stub: open documents" }));
    await expectSpolkaTool("dokumenty");
  });
});
