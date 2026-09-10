import { test, expect, openApp, expectNoPageOverflow } from "./helpers/harness";
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

  // M4 (ADR 0078): the MCP section's connection snippet is unbreakable command
  // text — the widest content in Settings. It must scroll inside its own bounded
  // `data-hscroll` scroller, never force a global/panel horizontal scrollbar,
  // across the viewport matrix (incl. the narrow quarter-ultrawide projects).
  test("MCP section keeps the connection snippet inside a bounded scroller", async ({ page }, testInfo) => {
    await openApp(page);
    await navTo(page, "Settings").click();

    const settingsRegion = page.getByLabel("Application settings");
    await settingsRegion.getByRole("button", { name: "MCP server" }).click();

    // Reveal the widest content: the token field plus a token-bearing snippet.
    await settingsRegion.getByRole("button", { name: "Generate token" }).click();
    await expect(settingsRegion.getByLabel("Access token")).toBeVisible();

    const snippet = settingsRegion.locator(".mcp-snippet").first();
    await expect(snippet).toBeVisible();
    await expect(snippet).toHaveAttribute("data-hscroll");

    // No global or panel-internal horizontal scrollbar at this viewport.
    await expectNoPageOverflow(page);

    await testInfo.attach("mcp-settings", {
      body: await page.screenshot({ fullPage: true }),
      contentType: "image/png",
    });
  });
});
