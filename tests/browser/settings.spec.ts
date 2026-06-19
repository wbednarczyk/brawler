import { test, expect, openApp } from "./helpers/harness";
import type { Page } from "@playwright/test";

// Clickable Settings journey against the stateful browser mock runtime
// (ADR 0048): changing the theme persists through update_settings and applies
// to the document, the user-visible outcome of the setting.

function navTo(page: Page, name: string) {
  return page.getByLabel("Primary navigation").getByRole("button", { name });
}

test.describe("settings", () => {
  test("switching the theme applies it to the document", async ({ page }) => {
    await openApp(page);
    await navTo(page, "Settings").click();

    // The mock seeds the dark theme; the Appearance section is the default tab.
    await expect(page.locator("html")).toHaveAttribute("data-theme", "dark");

    await page.getByLabel("Settings theme").selectOption("light");
    await expect(page.locator("html")).toHaveAttribute("data-theme", "light");

    // Switching back also takes effect.
    await page.getByLabel("Settings theme").selectOption("dark");
    await expect(page.locator("html")).toHaveAttribute("data-theme", "dark");
  });
});
