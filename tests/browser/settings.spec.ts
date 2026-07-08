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

    // Selecting a theme applies it to the document in both directions. The seed
    // theme is intentionally not asserted here: the `chromium-compact-light`
    // project forces the seed to light (ADR 0076 D3), so this test must exercise
    // the switch itself, not a fixed starting mode.
    await page.getByLabel("Settings theme").selectOption("light");
    await expect(page.locator("html")).toHaveAttribute("data-theme", "light");

    // Switching back also takes effect.
    await page.getByLabel("Settings theme").selectOption("dark");
    await expect(page.locator("html")).toHaveAttribute("data-theme", "dark");
  });

  // Both accent palettes render cleanly in both modes (ADR 0076 D3). The
  // semantic tone tokens are defined per palette × mode, so a token missing from
  // one block surfaces as a console error / invalid color here; the harness
  // console-error gate fails the test if any combination breaks. Full visual
  // coverage of the light theme lives in the `chromium-compact-light` project.
  test("renders both palettes in both modes without errors", async ({ page }) => {
    await openApp(page);
    await navTo(page, "Settings").click();

    for (const palette of ["night-neon", "midnight-horizon"] as const) {
      await page.getByLabel("Settings palette").selectOption(palette);
      await expect(page.locator("html")).toHaveAttribute("data-palette", palette);

      for (const theme of ["light", "dark"] as const) {
        await page.getByLabel("Settings theme").selectOption(theme);
        await expect(page.locator("html")).toHaveAttribute("data-theme", theme);
        // The toned primitives gallery / appearance surface must stay visible —
        // a hard render failure would drop it and trip the console-error gate.
        await expect(page.getByLabel("Settings palette")).toBeVisible();
      }
    }
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
