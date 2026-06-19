import { test, expect, openApp } from "./helpers/harness";
import type { Page } from "@playwright/test";

// Clickable Companies journey against the stateful browser mock runtime
// (ADR 0048): adding a company through the form writes into runtime state and
// the new row appears in the tracked-companies list.

function navTo(page: Page, name: string) {
  return page.getByLabel("Primary navigation").getByRole("button", { name });
}

test.describe("companies", () => {
  test("add a company; it appears in the tracked list", async ({ page }) => {
    await openApp(page);
    await navTo(page, "Companies").click();

    await page.getByLabel("Exchange", { exact: true }).fill("GPW");
    await page.getByLabel("Ticker", { exact: true }).fill("TST");
    await page.getByLabel("Name", { exact: true }).fill("Test Co S.A.");
    await page.getByRole("button", { name: "Add", exact: true }).click();

    // The new company is now tracked and listed (stateful create + refresh).
    const list = page.getByLabel("Companies list");
    await expect(list.getByLabel("Open GPW:TST workspace")).toBeVisible();

    // ...and it survives a list search that matches it.
    await page.getByPlaceholder("Search tracked companies").fill("Test Co");
    await expect(list.getByLabel("Open GPW:TST workspace")).toBeVisible();
  });
});
