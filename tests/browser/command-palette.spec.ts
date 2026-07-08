import { test, expect, openApp, expectNoPageOverflow } from "./helpers/harness";
import type { Page } from "@playwright/test";

// Global command palette (v0.50 U6): Ctrl/⌘+K opens a shared palette from any
// screen. It lists app-level commands (derived from the shortcut registry plus
// saved views / pinned companies) and, on a cockpit view, the cockpit's own
// contextual panel commands. The cockpit's "Add panel" / cell-fill flows keep
// their local palette instance (covered by notebooks.spec / screen-walk).

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

  test("on a cockpit view the palette also lists cockpit commands", async ({ page }) => {
    await openApp(page);

    // A plain New view lands on the cockpit without touching its local palette.
    await nav(page).getByRole("button", { name: "New view" }).click();
    const createModal = page.getByRole("dialog", { name: "New view" });
    await createModal.getByLabel("View name").fill("Palette view");
    await createModal.getByRole("button", { name: "Create view" }).click();
    await expect(page.getByLabel("Research cockpit")).toBeVisible();
    await expectNoPageOverflow(page);

    await page.keyboard.press("Control+K");
    const palette = page.getByRole("dialog", { name: "Command palette" });
    await expect(palette).toBeVisible();

    // "Open panel: …" entries are contributed by the cockpit contextually; app
    // commands never carry that label, so filtering isolates the contextual set.
    await palette.getByLabel("Search commands").fill("Open panel");
    await expect(palette.getByRole("button", { name: /Open panel:/ }).first()).toBeVisible();
  });
});
