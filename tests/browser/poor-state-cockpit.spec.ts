import { test, expect, openApp, expectNoPageOverflow } from "./helpers/harness";
import { primeChaos, primeMockScenario } from "./helpers/mockRuntime";

// Epic #40 S2 (ADR 0091) — the company cockpit walked on a POOR-STATE
// COMBINATION: `partial-data` (a company whose financial periods exist but whose
// facts read is missing) plus a chaos rule breaking ONE company-scoped read
// (`get_red_flags`) for the whole session.
//
// What must hold: the broken panel NAMES its failure (and the failing command),
// the partial panel reports its emptiness honestly, and neither takes the rest of
// the cockpit down with it — a company workspace degrades panel by panel, never
// as one silent blank.

const PARTIAL_COMPANY_ID = "company_gpw_partial";
const CHAOS_MESSAGE = "warning-signal index unavailable";

test.describe("poor state — company cockpit with a broken panel read", { tag: "@clickable" }, () => {
  test("the broken panel names its failure while the rest of the cockpit renders", async ({ page }) => {
    await primeMockScenario(page, { base: "rich", overlays: ["partial-data"] });
    await primeChaos(page, [
      { command: "get_red_flags", error: { code: "internal", message: CHAOS_MESSAGE } },
    ]);
    await openApp(page);

    await page
      .getByLabel(/Primary navigation|Nawigacja główna/)
      .getByRole("button", { name: "Companies" })
      .click();
    await page.locator(`[data-company-id="${PARTIAL_COMPANY_ID}"] .company-row-main`).click();

    const cockpit = page.getByLabel("Research cockpit");
    await expect(cockpit).toBeVisible();

    // The partial-data company's Fundamentals panel renders and states its own
    // emptiness (periods exist, facts do not) rather than showing nothing.
    const fundamentals = cockpit.locator(".fundamentals-panel");
    await expect(fundamentals).toBeVisible();
    await expect(fundamentals.getByText("0 facts recorded")).toBeVisible();

    // The broken read: the warning-signals panel names the failure it hit.
    await cockpit.getByRole("button", { name: "Warning signals", exact: true }).first().click();
    const redFlags = cockpit.locator(".red-flags-panel");
    await expect(redFlags).toBeVisible();
    await expect(redFlags.getByText(/Could not load warning signals/)).toContainText(CHAOS_MESSAGE);

    // ...and the rest of the cockpit is still standing.
    await expect(fundamentals).toBeVisible();
    await expectNoPageOverflow(page);
  });
});
