import { describe, it } from "vitest";
import { appTestState, expect, renderApp, screen, userEvent, within } from "../../test/appWorkflowHarness";

// U7-E2 density contract (ADR 0076 D6): Diagnostics — S: log list; filters
// collapse · M: + module/severity columns · L: full table · short: list only.
// The tier switch itself is CSS-only (container queries, jsdom-invisible); the
// browser-smoke runtime cannot reach Diagnostics (developer mode is unlocked via
// a hidden chord + passphrase, not mocked), so the reachable-fold semantics are
// asserted here: module/severity stay reachable in the expanded row detail (so
// they are never lost when the row columns collapse at S), and the filters live
// behind the FilterToolbar "Filters" disclosure.
describe("Diagnostics density contract (U7-E2)", () => {
  async function openDiagnostics() {
    const user = userEvent.setup();
    appTestState.settingsResponse = {
      ...appTestState.settingsResponse,
      developerMode: true,
    };
    renderApp();
    await user.click(await screen.findByRole("button", { name: "Diagnostics" }));
    await screen.findByRole("heading", { name: "Diagnostic events" });
    return user;
  }

  it("keeps module and severity reachable in the expanded event detail", async () => {
    const user = await openDiagnostics();

    const eventButton = await screen.findByRole("button", {
      name: /Sample diagnostic event for the developer console/,
    });
    await user.click(eventButton);

    const detail = eventButton.closest("article")?.querySelector(".diagnostic-event-detail");
    expect(detail).not.toBeNull();
    // At the S tier the row columns collapse to message + timestamp, so the
    // module/severity metadata must survive in the row detail.
    const detailGrid = within(detail as HTMLElement);
    expect(detailGrid.getByText("Module")).toBeInTheDocument();
    expect(detailGrid.getByText("Severity")).toBeInTheDocument();
    expect(detailGrid.getByText("sources")).toBeInTheDocument();
    expect(detailGrid.getByText("info")).toBeInTheDocument();
  });

  it("renders the diagnostic filters behind the FilterToolbar disclosure", async () => {
    await openDiagnostics();

    // The FilterToolbar owns the S-tier "Filters" collapse (ADR 0076 D6); the
    // module + severity selects live inside it.
    const disclosure = screen.getByRole("button", { name: "Filters" });
    expect(disclosure).toHaveAttribute("aria-expanded");
    expect(screen.getByLabelText("Module")).toBeInTheDocument();
    expect(screen.getByLabelText("Severity")).toBeInTheDocument();
  });
});

// Source reconciliation ledger (ADR 0069 D2, plan v0.55 T3): the developer
// Diagnostics screen surfaces the GPW ESPI/EBI witness ↔ Bankier pair results
// with a per-status chip.
describe("Diagnostics source reconciliation section", () => {
  it("renders reconciliation results with their status chips", async () => {
    const user = userEvent.setup();
    appTestState.settingsResponse = {
      ...appTestState.settingsResponse,
      developerMode: true,
    };
    appTestState.reconciliationResultsResponse = [
      {
        id: "recon_espi",
        witnessAdapterId: "gpw-espi-ebi",
        companyId: "company_1",
        qualifiedTicker: "GPW:CDR",
        reportNumber: "15/2026",
        reportType: "Bieżący",
        disclosureDate: "2026-07-14",
        witnessTitle: "Zawarcie istotnej umowy",
        witnessUrl: "https://www.gpw.pl/komunikaty?id=15",
        status: "espi_only",
        primaryFeedItemId: null,
        createdAt: "2026-07-14T10:00:00Z",
        updatedAt: "2026-07-14T10:00:00Z",
      },
    ];
    renderApp();
    await user.click(await screen.findByRole("button", { name: "Diagnostics" }));
    await screen.findByRole("heading", { name: "Diagnostic events" });

    // Expand the reconciliation section, then assert its ledger row + status chip.
    await user.click(await screen.findByRole("button", { name: /Source reconciliation/ }));
    expect(await screen.findByText("Zawarcie istotnej umowy")).toBeInTheDocument();
    expect(screen.getByText("Missed by primary")).toBeInTheDocument();
    expect(screen.getByText("GPW:CDR")).toBeInTheDocument();
  });
});
