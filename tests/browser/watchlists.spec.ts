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

  // Fix-B keyboard proof (F4a R1 findings 4/5): real Tab traversal — not
  // `.focus()` straight onto the target — from a control the contract names
  // as an anchor (the previous row's Remove, alongside the list search
  // field), through the row's own Open->Remove order and back, ending with
  // Enter on Open triggering the same navigation a click would.
  test("keyboard: Tab reaches a member row's Open from the previous row's Remove, moves Open<->Remove, and Enter on Open navigates", async ({
    page,
  }) => {
    await openApp(page);
    await navTo(page, "Watchlists").click();

    const rows = page.getByLabel("Companies in watchlist").locator(".watchlist-member-row");
    await expect(rows.nth(1)).toBeVisible();

    const firstRemove = rows.nth(0).getByRole("button", { name: "Remove from list" });
    const secondOpen = rows.nth(1).getByRole("button", { name: "Open company" });
    const secondRemove = rows.nth(1).getByRole("button", { name: "Remove from list" });

    await firstRemove.focus();
    await page.keyboard.press("Tab");
    await expect(secondOpen).toBeFocused();

    await page.keyboard.press("Tab");
    await expect(secondRemove).toBeFocused();
    await page.keyboard.press("Shift+Tab");
    await expect(secondOpen).toBeFocused();

    await page.keyboard.press("Enter");
    await expect(page.getByRole("region", { name: "Company view" })).toBeVisible();
  });

  // Fix-B picker proof: picker rows are labelled checkboxes, so an
  // already-listed company can be asserted disabled by its accessible name
  // (never `.locator(".watchlist-picker-row").first()`), and selection goes
  // through the checkbox role rather than a class-selector click.
  test("picker rows are labelled checkboxes: an already-listed company is disabled with a note; selection uses the checkbox role", async ({
    page,
  }) => {
    await openApp(page);
    await navTo(page, "Watchlists").click();

    const detail = page.getByLabel("Selected watchlist");
    await detail.getByRole("button", { name: "Add companies" }).click();
    const picker = page.getByLabel("Add companies", { exact: true });

    // CDR is already a member of the default-selected list (seed data) — its
    // row stays visible, disabled, with the "already on the list" note,
    // instead of disappearing (F4a S3 redesign).
    const cdrCheckbox = picker.getByRole("checkbox", { name: /GPW:CDR/ });
    await expect(cdrCheckbox).toBeDisabled();
    const cdrRow = cdrCheckbox.locator("xpath=..");
    await expect(cdrRow).toContainText("already on the list");

    // An available company (not yet a member) selects via the checkbox role.
    const availableCheckbox = picker.getByRole("checkbox", { name: /GPW:T07/ });
    await expect(availableCheckbox).toBeEnabled();
    await availableCheckbox.check();
    await picker.getByRole("button", { name: "Add selected" }).click();

    await expect(page.getByLabel("Companies in watchlist").locator(".watchlist-member-row")).toHaveCount(17);
  });
});
