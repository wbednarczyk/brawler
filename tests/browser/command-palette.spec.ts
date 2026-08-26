import { test, expect, openApp, expectNoPageOverflow } from "./helpers/harness";
import type { Page } from "@playwright/test";

// Global command palette (v0.50 U6): Ctrl/⌘+K opens a shared palette from any
// screen. It lists app-level commands (derived from the shortcut registry plus
// saved views / pinned companies / global screens) and, on the Spółka screen,
// that screen's own contextual "Open <tool>" workshop commands. F3a S3 (ADR
// 0107 decision 5) froze the cockpit's local palette entirely — "Open panel:
// …" and every other structure-mutating entry it used to contribute is gone.

function nav(page: Page) {
  return page.getByLabel(/Primary navigation|Nawigacja główna/);
}

test.describe("command palette", { tag: "@clickable" }, () => {
  test("Ctrl+K opens the palette and a listed command navigates", async ({ page }) => {
    await openApp(page);

    await page.keyboard.press("Control+K");
    const palette = page.getByRole("dialog", { name: "Command palette" });
    await expect(palette).toBeVisible();

    // App-level commands come from the shortcut registry; running one navigates.
    await palette.getByLabel("Search commands").fill("Open Settings");
    await palette.getByRole("button", { name: "Open Settings", exact: true }).click();

    await expect(page.getByRole("heading", { name: "Settings" })).toBeVisible();
    await expect(palette).toBeHidden();
  });

  test("Escape closes the palette", async ({ page }) => {
    await openApp(page);

    await page.keyboard.press("Control+K");
    const palette = page.getByRole("dialog", { name: "Command palette" });
    await expect(palette).toBeVisible();

    await page.keyboard.press("Escape");
    await expect(palette).toBeHidden();
  });

  // F3a S3 (ADR 0107 decision 5): the cockpit freeze retired the whole
  // structure-mutating local palette — "New view" (nav), "Open panel: …" and
  // every other "Add panel"/cell-fill command are gone; the surviving palette
  // dictionary is app-level navigation only. Assert what's actually still
  // offered: global "Open screen: …" / "Open company: …" entries from any
  // screen, and the Spółka screen's own contextual "Open <tool>" entries
  // (never "Open panel: …", the retired label).
  test("the palette lists the global navigation commands from any screen", async ({ page }) => {
    await openApp(page);

    await page.keyboard.press("Control+K");
    const palette = page.getByRole("dialog", { name: "Command palette" });
    await expect(palette).toBeVisible();

    await palette.getByLabel("Search commands").fill("Open screen");
    await expect(palette.getByRole("button", { name: "Open screen: Research", exact: true })).toBeVisible();

    await palette.getByLabel("Search commands").fill("Open company: CDR");
    await expect(
      palette.getByRole("button", { name: "Open company: CDR", exact: true }),
    ).toBeVisible();

    await expect(palette.getByRole("button", { name: /^Open panel:/ })).toHaveCount(0);
  });

  test("inside Spółka the palette also lists the workshop tool commands", async ({ page }) => {
    await openApp(page);
    await nav(page).getByRole("button", { name: "Companies" }).click();
    await page.getByRole("button", { name: "Open GPW:CDR dashboard" }).click();
    await expect(page.getByRole("region", { name: "Company view" })).toBeVisible();
    await expectNoPageOverflow(page);

    await page.keyboard.press("Control+K");
    const palette = page.getByRole("dialog", { name: "Command palette" });
    await expect(palette).toBeVisible();

    await palette.getByLabel("Search commands").fill("Open notebook");
    await expect(palette.getByRole("button", { name: "Open notebook", exact: true })).toBeVisible();

    await expect(palette.getByRole("button", { name: /^Open panel:/ })).toHaveCount(0);
  });
});
