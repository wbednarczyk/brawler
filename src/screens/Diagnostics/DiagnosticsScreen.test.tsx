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
