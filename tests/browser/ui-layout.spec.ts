import { expect, type Locator, type Page, test } from "@playwright/test";

test.describe("browser UI regression smoke", () => {
  test("keeps app chrome fixed and avoids a global application scrollbar", async ({ page }) => {
    await openApp(page);

    await expect(page.getByRole("heading", { name: "Inbox" })).toBeVisible();
    await expect(page.getByLabel("Primary navigation")).toBeVisible();
    await expect(page.locator(".topbar")).toBeVisible();

    const bodyScroll = await page.evaluate(() => {
      window.scrollTo(0, 100);
      return {
        documentOverflowY: window.getComputedStyle(document.documentElement).overflowY,
        bodyOverflowY: window.getComputedStyle(document.body).overflowY,
        scrollY: window.scrollY,
      };
    });

    expect(bodyScroll.documentOverflowY).toBe("hidden");
    expect(bodyScroll.bodyOverflowY).toBe("hidden");
    expect(bodyScroll.scrollY).toBe(0);
    await expect(page.getByLabel("Primary navigation")).toBeInViewport();
    await expect(page.locator(".topbar")).toBeInViewport();
  });

  test("navigates the main screens through the sidebar", async ({ page }) => {
    await openApp(page);

    for (const screenName of ["Companies", "Watchlists", "Sources", "Notebooks", "Settings", "Inbox"]) {
      await navButton(page, screenName).click();
      await expect(page.getByRole("heading", { name: screenName })).toBeVisible();
    }
  });

  test("keeps the Companies list usable and internally scrollable", async ({ page }) => {
    await openApp(page);
    await navButton(page, "Companies").click();

    const companiesList = page.getByLabel("Companies list");
    await expect(companiesList).toBeVisible();
    await expect(page.getByLabel("Company list search")).toBeVisible();
    await expect(page.getByLabel("Company list search")).toBeInViewport();

    const box = await expectBox(companiesList);
    expect(box.height).toBeGreaterThan(240);
    await expectScrollable(companiesList);

    await companiesList.evaluate((element) => {
      element.scrollTop = element.scrollHeight;
    });

    await expect(page.getByLabel("Company list search")).toBeInViewport();
  });

  test("keeps notebook panes independently usable", async ({ page }) => {
    await openApp(page);
    await navButton(page, "Notebooks").click();

    const workspace = page.getByLabel("Notebooks workspace");
    const companyNav = page.getByLabel("Notebook companies");
    const noteList = page.getByLabel("Notebook note list", { exact: true });
    const detailPane = page.getByLabel("Notebook selected entry");

    await expect(workspace).toBeVisible();
    await expect(companyNav).toBeVisible();
    await expect(noteList).toBeVisible();
    await expect(detailPane).toBeVisible();

    expect((await expectBox(companyNav)).height).toBeGreaterThan(260);
    expect((await expectBox(noteList)).height).toBeGreaterThan(220);
    expect((await expectBox(detailPane)).height).toBeGreaterThan(220);

    await expectScrollable(companyNav);
    await expectScrollable(noteList);
  });

  test("keeps Sources rows compact and expanded rows readable", async ({ page }) => {
    await openApp(page);
    await navButton(page, "Sources").click();

    const sourceList = page.getByLabel("Source list");
    await expect(sourceList).toBeVisible();

    const groupHeader = page.locator(".source-group-header").first();
    const sourceRow = page.locator(".source-row").first();
    await expect(groupHeader).toBeVisible();
    await expect(sourceRow).toBeVisible();

    expect((await expectBox(groupHeader)).height).toBeLessThanOrEqual(38);
    expect((await expectBox(sourceRow)).height).toBeLessThanOrEqual(58);

    await sourceRow.click();
    await expect(sourceRow).toHaveClass(/source-row-selected/);
    await expect(page.getByLabel("Source details").first()).toBeVisible();
  });

  test("keeps Watchlists member and add-company lists internally scrollable", async ({ page }) => {
    await openApp(page);
    await navButton(page, "Watchlists").click();

    const selectedWatchlist = page.getByLabel("Selected watchlist");
    const memberTable = selectedWatchlist.locator(".watchlist-member-table");
    await expect(selectedWatchlist).toBeVisible();
    await expect(memberTable).toBeVisible();

    expect((await expectBox(memberTable)).height).toBeGreaterThan(160);
    await expectScrollable(memberTable);

    await selectedWatchlist.getByRole("button", { name: "Add companies" }).click();
    const pickerList = selectedWatchlist.locator(".watchlist-picker-list");
    await expect(pickerList).toBeVisible();
    await expectScrollable(pickerList);
  });
});

async function openApp(page: Page) {
  await page.goto("/");
  await expect(page.getByLabel("Primary navigation")).toBeVisible();
}

function navButton(page: Page, name: string) {
  return page.getByLabel("Primary navigation").getByRole("button", { name });
}

async function expectBox(locator: Locator) {
  const box = await locator.boundingBox();
  expect(box).not.toBeNull();
  return box!;
}

async function expectScrollable(locator: Locator) {
  const scroll = await locator.evaluate((element) => ({
    clientHeight: element.clientHeight,
    scrollHeight: element.scrollHeight,
    overflowY: window.getComputedStyle(element).overflowY,
  }));

  expect(["auto", "scroll"]).toContain(scroll.overflowY);
  expect(scroll.scrollHeight).toBeGreaterThan(scroll.clientHeight);
}
