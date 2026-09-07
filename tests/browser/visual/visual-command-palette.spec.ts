import { test, expect, openApp, openPalette } from "../helpers/harness";
import { shootRegion } from "./helpers";

// Visual baseline — the ⌘K command palette (dogfooding wave 2026-09, #3).
// Registered as `command-palette` in catalog.core.mjs (M_ONLY, no `figures`
// proof — a command list carries no numeric data by design). Same pattern as
// `visual-activity.spec.ts`: the dialog is a portal over the workspace, so
// it's shot once at the M pane width via `shootRegion` (S/L would only
// resize the pane behind it).

test.describe("visual — command palette", () => {
  test("palette open, unfiltered command list", async ({ page }) => {
    await openApp(page);
    const dialog = await openPalette(page);
    await expect(dialog.getByRole("listbox", { name: "Commands" })).toBeVisible();
    await shootRegion(page, page.locator(".workspace"), dialog, "command-palette");
  });
});
