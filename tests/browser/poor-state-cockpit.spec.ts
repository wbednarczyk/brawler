import { test, expect, openApp, expectNoPageOverflow } from "./helpers/harness";
import { primeChaos, primeMockScenario } from "./helpers/mockRuntime";

// Epic #40 S2 (ADR 0091) — the company workspace walked on a POOR-STATE
// COMBINATION: `partial-data` (a company whose financial periods exist but whose
// facts read is missing) plus a chaos rule breaking ONE company-scoped read
// (`get_red_flags`) for the whole session.
//
// What must hold: the broken tool NAMES its failure (and the failing command),
// the partial tool reports its emptiness honestly, and neither corrupts the
// other — a company's workshop tools degrade independently, never as one
// silent blank. F3a S3 (ADR 0107): `company_gpw_partial` has no legacy
// `dashboard:` layout, so this walks the Spółka screen's tool path (the
// frozen cockpit shell is not reachable for this company at all) — only one
// tool is open at a time, so "neither corrupts the other" is proven by
// re-opening Fundamentals after the signals failure and finding it unchanged.

const PARTIAL_COMPANY_ID = "company_gpw_partial";
const CHAOS_MESSAGE = "warning-signal index unavailable";

async function openSpolkaTool(page: import("@playwright/test").Page, label: string) {
  await page.keyboard.press("Control+K");
  const palette = page.getByRole("dialog", { name: "Command palette" });
  await palette.getByLabel("Search commands").fill(label);
  await palette.getByRole("button", { name: label, exact: true }).first().click();
  return page.getByRole("group", { name: "Workshop tool" });
}

test.describe("poor state — Spółka workshop tools with a broken read", { tag: "@clickable" }, () => {
  test("the broken tool names its failure without corrupting an unrelated tool", async ({ page }) => {
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
    await page.getByRole("region", { name: "Company view" }).waitFor();

    // The partial-data company's Fundamentals tool renders and states its own
    // emptiness (periods exist, facts do not) rather than showing nothing.
    let tool = await openSpolkaTool(page, "Open fundamentals");
    await expect(tool.locator(".fundamentals-panel")).toBeVisible();
    await expect(tool.getByText("0 facts recorded")).toBeVisible();

    // The broken read: the warning-signals tool names the failure it hit.
    tool = await openSpolkaTool(page, "Open signals");
    const redFlags = tool.locator(".red-flags-panel");
    await expect(redFlags).toBeVisible();
    await expect(redFlags.getByText(/Could not load warning signals/)).toContainText(CHAOS_MESSAGE);
    await expectNoPageOverflow(page);

    // ...and Fundamentals is unaffected — re-opening it renders exactly as before.
    tool = await openSpolkaTool(page, "Open fundamentals");
    await expect(tool.locator(".fundamentals-panel")).toBeVisible();
    await expect(tool.getByText("0 facts recorded")).toBeVisible();
    await expectNoPageOverflow(page);
  });
});
