import { test, expect, openApp } from "../helpers/harness";
import { shootPanel, shootScreen } from "./helpers";
import type { Page } from "@playwright/test";

// Visual baseline — Watchlists (sidebar) · Transcripts · Settings (ADR 0076 D7 /
// U11), mirroring density-utility.spec.ts. Watchlists + Transcripts follow the
// default panel rule (S/M/L forced on `.workspace`); Settings is a named full
// screen (M at the project viewport + forced S/L). Diagnostics is developer-gated
// and unreachable in the smoke runtime (see density-utility.spec.ts) — no shot.

function nav(page: Page) {
  return page.getByLabel(/Primary navigation|Nawigacja główna/);
}

test.describe("visual — utility screens", () => {
  test("Watchlists (sidebar) across pane tiers", async ({ page }) => {
    await openApp(page);
    await nav(page).getByRole("button", { name: "Watchlists" }).click();

    // Seed a watchlist with one member so the company table has content at L.
    await page.getByLabel("Watchlist name").fill("Density check");
    await page.getByRole("button", { name: "Create" }).click();
    const row = page
      .getByLabel("Watchlists", { exact: true })
      .getByRole("button")
      .filter({ hasText: "Density check" });
    await row.click();
    const detail = page.getByLabel("Selected watchlist");
    await detail.getByRole("button", { name: "Add companies" }).click();
    const picker = page.getByLabel("Add companies", { exact: true });
    await picker.locator(".watchlist-picker-row").first().click();
    await picker.getByRole("button", { name: "Add selected" }).click();
    await expect(page.locator(".watchlist-member-row")).toHaveCount(1);

    await shootPanel(page, page.locator(".workspace"), "watchlists");
  });

  test("Transcripts across pane tiers", async ({ page }) => {
    await openApp(page);
    await nav(page).getByRole("button", { name: "Transcripts" }).click();
    await expect(page.getByLabel("Transcript jobs")).toBeVisible();

    // The smoke runtime seeds no transcript jobs; create one so the list + detail
    // have content, then expand it.
    await page.getByLabel("Transcript URL").fill("https://www.youtube.com/watch?v=densitycheck");
    await page.getByRole("button", { name: "Create job" }).click();
    await expect(page.locator(".transcript-row")).toHaveCount(1);
    await page.locator(".transcript-row").first().click();

    await shootPanel(page, page.locator(".workspace"), "transcripts");
  });

  test("Settings across pane tiers", async ({ page }) => {
    await openApp(page);
    await nav(page).getByRole("button", { name: "Settings" }).click();
    await expect(page.getByLabel("Application settings")).toBeVisible();
    await shootScreen(page, "settings", { forced: ["S", "L"] });
  });
});
