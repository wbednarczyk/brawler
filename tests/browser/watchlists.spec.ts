import { test, expect, openApp } from "./helpers/harness";
import { expectPrimaryActionCount } from "./helpers/interactionContracts";
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

test.describe("watchlists", { tag: "@clickable" }, () => {
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

    // Remove — an in-place InlineConfirm (ADR 0076 D5), not a native dialog:
    // open then confirm, and the row disappears (stateful). "Remove" is the
    // only collection-removal verb (ADR 0104 dec. 3 amendment) — "Delete" is
    // retired from this screen.
    await detail.getByRole("button", { name: "Remove" }).click();
    await detail.getByRole("button", { name: "Remove" }).click();
    await expect(watchlistRows(page).filter({ hasText: "Renamed list" })).toHaveCount(0);
  });

  // First red journey test named by the F4a Watchlists contract
  // (docs/plans/frontend-v2-f4a.md § Watchlists, item 1): before the redesign
  // the member row carried an action column and the screen rendered two
  // filled buttons at rest (the header's "Create" plus the detail's "Add
  // companies") — this fails against the pre-redesign screen for both
  // reasons and passes once the row's two actions are separate real buttons
  // and exactly one filled action remains (ADR 0104 dec. 3 amendment).
  test("member row offers Open company and Remove as two focusable controls; the selected list shows one filled action", async ({
    page,
  }) => {
    await openApp(page);
    await navTo(page, "Watchlists").click();

    const region = page.getByRole("region", { name: "Watchlists" });
    const members = page.getByLabel("Companies in watchlist");
    const row = members.locator(".watchlist-member-row").first();
    await expect(row).toBeVisible();

    const openButton = row.getByRole("button", { name: "Open company" });
    const removeButton = row.getByRole("button", { name: "Remove from list" });
    // Two distinct, independently focusable controls — not a single
    // row-wide click target with an action column.
    await expect(openButton).toBeVisible();
    await expect(removeButton).toBeVisible();
    await openButton.focus();
    await expect(openButton).toBeFocused();
    await page.keyboard.press("Tab");
    await expect(removeButton).toBeFocused();

    await expectPrimaryActionCount(region, { max: 1 });
  });
});
