import { test, expect, openApp, openPalette, expectNoPageOverflow } from "./helpers/harness";
import type { Page } from "@playwright/test";

// Global command palette (v0.50 U6): Ctrl/⌘+K opens a shared palette from any
// screen. It lists app-level commands (derived from the shortcut registry plus
// every tracked company / global screens) and, on the Spółka screen, that
// screen's own contextual "Open <tool>" workshop commands. ADR 0108 (retiring
// the docking engine) removed every "Open view: …" / "Open panel: …" entry —
// the palette dictionary is app-level navigation only.

function nav(page: Page) {
  return page.getByLabel(/Primary navigation|Nawigacja główna/);
}

test.describe("command palette", { tag: "@clickable" }, () => {
  test("Ctrl+K opens the palette and a listed command navigates", async ({ page }) => {
    await openApp(page);

    const palette = await openPalette(page);
    await expect(palette).toBeVisible();

    // App-level commands come from the shortcut registry; running one navigates.
    await palette.getByLabel("Search commands").fill("Open Settings");
    await palette.getByRole("button", { name: "Open Settings", exact: true }).click();

    await expect(page.getByRole("heading", { name: "Settings" })).toBeVisible();
    await expect(palette).toBeHidden();
  });

  test("Escape closes the palette", async ({ page }) => {
    await openApp(page);

    const palette = await openPalette(page);
    await expect(palette).toBeVisible();

    await page.keyboard.press("Escape");
    await expect(palette).toBeHidden();
  });

  // ADR 0108 (retiring the docking engine, no named views, no "Widoki" group):
  // the palette dictionary is app-level navigation only. Assert what's
  // actually still offered: global "Open screen: …" / "Open company: …"
  // entries from any screen — never "Open view: …" or "Open panel: …", both
  // retired labels. F4c S2 (ADR 0108 amendment): "Open screen: Notebooks" /
  // "Open screen: Decision journal" retire with the global screens — every
  // deep link lands on the Spółka `notatnik` tool instead.
  test("the palette lists the global navigation commands from any screen, no Open view entries", async ({ page }) => {
    await openApp(page);

    const palette = await openPalette(page);
    await expect(palette).toBeVisible();

    await palette.getByLabel("Search commands").fill("Open screen");
    await expect(palette.getByRole("button", { name: "Open screen: Research", exact: true })).toBeVisible();
    await expect(palette.getByRole("button", { name: /^Open screen: Notebooks/ })).toHaveCount(0);
    await expect(palette.getByRole("button", { name: /^Open screen: Decision journal/ })).toHaveCount(0);

    await palette.getByLabel("Search commands").fill("Open company: CDR");
    await expect(
      palette.getByRole("button", { name: "Open company: CDR", exact: true }),
    ).toBeVisible();

    await palette.getByLabel("Search commands").fill("Open view");
    await expect(palette.getByRole("button", { name: /^Open view:/ })).toHaveCount(0);

    await palette.getByLabel("Search commands").fill("");
    await expect(palette.getByRole("button", { name: /^Open panel:/ })).toHaveCount(0);
  });

  test("inside Spółka the palette also lists the workshop tool commands", async ({ page }) => {
    await openApp(page);
    await nav(page).getByRole("button", { name: "Companies" }).click();
    await page.getByRole("button", { name: "Open GPW:CDR" }).click();
    await expect(page.getByRole("region", { name: "Company view" })).toBeVisible();
    await expectNoPageOverflow(page);

    const palette = await openPalette(page);
    await expect(palette).toBeVisible();

    await palette.getByLabel("Search commands").fill("Open notebook");
    await expect(palette.getByRole("button", { name: "Open notebook", exact: true })).toBeVisible();

    await expect(palette.getByRole("button", { name: /^Open panel:/ })).toHaveCount(0);
  });
});
