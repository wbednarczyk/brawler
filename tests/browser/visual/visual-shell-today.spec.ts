import { test, expect, openApp } from "../helpers/harness";
import { shootScreen } from "./helpers";

// Visual baseline — Today home (ADR 0076 D7 / U11). Does not force `.workspace`
// in its density/shell spec, so this is the M-equivalent only: the workspace
// at the project viewport (light shoots the same single M).

test.describe("visual — shell + today", () => {
  test("Today home", async ({ page }) => {
    await openApp(page);
    await expect(page.getByRole("heading", { name: "Today", exact: true })).toBeVisible();
    // Dziś v2 readiness anchor (F2): the delta header + the first day section
    // render only after get_today_view resolves — the strip/stream is gone.
    await expect(page.locator(".dayq-delta-header")).toBeVisible();
    await expect(page.locator(".dayq-section").first()).toBeVisible();
    await shootScreen(page, "today");
  });
});
