import { test, expect, openApp } from "./helpers/harness";
import type { Page } from "@playwright/test";

// Clickable Watchlists CRUD journeys against the stateful browser mock runtime
// (ADR 0048): create/add-member/rename/delete all write into runtime state and
// reflect into subsequent reads, so each step is asserted end to end. These
// cover the screen that previously had no clickable coverage at all.

function navTo(page: Page, name: string) {
  return page.getByLabel("Primary navigation").getByRole("button", { name });
}

function watchlistRows(page: Page) {
  return page.getByLabel("Watchlists", { exact: true }).getByRole("button");
}

test.describe("watchlists", () => {
  test("create a watchlist, then add a company that reflects as a member", async ({ page }) => {
    await openApp(page);
    await navTo(page, "Watchlists").click();

    // Create — the new watchlist must appear in the list (stateful create).
    await page.getByLabel("Watchlist name").fill("Quarterly review");
    await page.getByRole("button", { name: "Create" }).click();

    const created = watchlistRows(page).filter({ hasText: "Quarterly review" });
    await expect(created).toBeVisible();
    await created.click();

    const detail = page.getByLabel("Selected watchlist");
    await expect(detail.getByRole("heading", { name: "Quarterly review" })).toBeVisible();

    // A freshly created watchlist has no members yet.
    const members = page.getByLabel("Companies in watchlist");
    await expect(members.locator(".watchlist-member-row")).toHaveCount(0);

    // Add the first available company; it must show up as a member row.
    await detail.getByRole("button", { name: "Add companies" }).click();
    const picker = page.getByLabel("Add companies", { exact: true });
    await picker.locator(".watchlist-picker-row").first().click();
    await picker.getByRole("button", { name: "Add selected" }).click();

    await expect(members.locator(".watchlist-member-row")).toHaveCount(1);
    // ...and the sidebar row's count badge reflects the new membership.
    await expect(created).toContainText("1");
  });

  test("rename and delete a watchlist reflect in the list", async ({ page }) => {
    await openApp(page);
    await navTo(page, "Watchlists").click();

    await page.getByLabel("Watchlist name").fill("Temp list");
    await page.getByRole("button", { name: "Create" }).click();
    const row = watchlistRows(page).filter({ hasText: "Temp list" });
    await expect(row).toBeVisible();
    await row.click();

    const detail = page.getByLabel("Selected watchlist");

    // Rename — the new name replaces the old one in both header and sidebar.
    await detail.getByRole("button", { name: "Rename" }).click();
    await page.getByLabel("Rename watchlist").fill("Renamed list");
    await detail.getByRole("button", { name: "Save" }).click();
    await expect(detail.getByRole("heading", { name: "Renamed list" })).toBeVisible();
    await expect(watchlistRows(page).filter({ hasText: "Renamed list" })).toBeVisible();
    await expect(watchlistRows(page).filter({ hasText: "Temp list" })).toHaveCount(0);

    // Delete — confirms via window.confirm, then the row disappears (stateful).
    page.once("dialog", (dialog) => dialog.accept());
    await detail.getByRole("button", { name: "Delete" }).click();
    await expect(watchlistRows(page).filter({ hasText: "Renamed list" })).toHaveCount(0);
  });
});
