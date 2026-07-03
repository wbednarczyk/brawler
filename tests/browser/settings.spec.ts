import { test, expect, openApp } from "./helpers/harness";
import type { Page } from "@playwright/test";

// Clickable Settings journey against the stateful browser mock runtime
// (ADR 0048): changing the theme persists through update_settings and applies
// to the document, the user-visible outcome of the setting.

function navTo(page: Page, name: string) {
  return page.getByLabel("Primary navigation").getByRole("button", { name });
}

test.describe("settings", { tag: "@clickable" }, () => {
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

  test("routes an AI capability to an ordered provider pool and persists across tab navigation", async ({
    page,
  }) => {
    await openApp(page);
    await navTo(page, "Settings").click();

    const settingsRegion = page.getByLabel("Application settings");
    await settingsRegion.getByRole("button", { name: "AI", exact: true }).click();

    const kpiRow = settingsRegion
      .locator(".capability-routing-row")
      .filter({ has: page.getByRole("heading", { name: "KPI extraction", level: 3 }) });

    // Two-member failover pool: the catalog default (Gemini), then a
    // OpenAI-compatible provider with a typed custom model.
    await kpiRow.getByRole("button", { name: "Add provider" }).click();
    await expect(page.getByLabel("Provider KPI extraction 1")).toHaveValue("provider_gemini");

    await kpiRow.getByRole("button", { name: "Add provider" }).click();
    await page.getByLabel("Provider KPI extraction 2").selectOption("provider_openai_compatible");
    await page.getByLabel("Model KPI extraction 2").fill("custom-model-x");

    await page.getByLabel("OpenAI-compatible base URL").fill("https://compat.example.com/v1");

    // Navigate away and back — the stateful mock runtime (ADR 0048) persists
    // the saved settings, so the routing pool and base URL re-render as saved.
    await settingsRegion.getByRole("button", { name: "Credentials" }).click();
    await settingsRegion.getByRole("button", { name: "AI", exact: true }).click();

    await expect(page.getByLabel("Provider KPI extraction 1")).toHaveValue("provider_gemini");
    await expect(page.getByLabel("Provider KPI extraction 2")).toHaveValue("provider_openai_compatible");
    await expect(page.getByLabel("Model KPI extraction 2")).toHaveValue("custom-model-x");
    await expect(page.getByLabel("OpenAI-compatible base URL")).toHaveValue("https://compat.example.com/v1");
  });
});
