import { test, expect, openApp } from "../helpers/harness";
import { shootRegion } from "./helpers";

// Visual baseline — Activity panel (ADR 0109, #133). Registered as
// `activity-open` in catalog.core.mjs (M_ONLY, figures min 3): the dialog is a
// portal over the workspace, so it is shot once at the M pane width via
// `shootRegion` (S/L tiers would only resize the pane behind it).

test.describe("visual — activity panel", () => {
  test("Activity panel open, seeded rows", async ({ page }) => {
    await openApp(page);
    await page.getByRole("button", { name: "Open activity" }).click();
    const dialog = page.getByRole("dialog", { name: "Activity" });
    await expect(dialog).toBeVisible();
    await expect(dialog.locator(".activity-item, [data-empty-kind]").first()).toBeVisible();
    await shootRegion(page, page.locator(".workspace"), dialog, "activity-open");
  });
});
