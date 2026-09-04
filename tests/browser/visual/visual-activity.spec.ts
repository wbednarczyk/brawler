import { test, expect, openApp } from "../helpers/harness";
import { shootPanel } from "./helpers";

// Visual baseline — Activity panel (ADR 0109, #133). Registered as
// `activity-open` in catalog.core.mjs (M_ONLY, figures min 3 — see the
// deviation note in the S2 handoff report re: `shootPanel`'s S/M/L pane
// resize vs the catalog's M-only declaration).

test.describe("visual — activity panel", () => {
  test("Activity panel open, seeded rows", async ({ page }) => {
    await openApp(page);
    await page.getByRole("button", { name: "Open activity" }).click();
    const dialog = page.getByRole("dialog", { name: "Activity" });
    await expect(dialog).toBeVisible();
    await expect(dialog.locator(".activity-item, [data-empty-kind]").first()).toBeVisible();
    await shootPanel(page, dialog, "activity-open");
  });
});
