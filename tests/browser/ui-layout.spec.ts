import { expect, type Locator, type Page, test } from "@playwright/test";
import { openCockpitPanel } from "./helpers/harness";

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

  test("navigates the sidebar spine destinations to their screens", async ({ page }) => {
    await openApp(page);

    // The sidebar destinations whose screen heading matches the nav label (ADR
    // 0054). Cockpit ("Research cockpit") + the cockpit-hosted screens are walked
    // by smoke-walk / screen-walk-extended; Notebooks/Research/Events/ReportSeason
    // are no longer sidebar buttons.
    // Compare is hidden from the sidebar until v0.53 (U-Rc, ADR 0076 Resolved).
    for (const screenName of [
      "Today",
      "Companies",
      "Inbox",
      "Watchlists",
      "Transcripts",
      "Sources",
      "Settings",
    ]) {
      await navButton(page, screenName).click();
      await expect(page.getByRole("heading", { name: screenName, exact: true })).toBeVisible();
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

  test("keeps Inbox readable at a narrow desktop package window", async ({ page }) => {
    await page.setViewportSize({ width: 1008, height: 796 });
    await openApp(page);

    const contentGrid = page.locator(".content-grid");
    const feedPanel = page.getByRole("region", { name: "Inbox" });
    const detailPane = page.getByLabel("Feed item details");
    const statusFilter = page.getByLabel("Feed status filter");
    const filterReset = page.getByLabel("Inbox filter reset");
    const filterToolbar = page.getByLabel("Inbox filters");

    await expect(feedPanel).toBeVisible();
    await expect(detailPane).toBeVisible();
    await expect(statusFilter).toBeVisible();
    await expect(filterReset).toBeVisible();

    // Select a report feed item so the detail pane renders its full content
    // (attachments, AI analysis panel, KPI extraction panel) — a wide descendant
    // there must not blow out the grid track and clip the pane.
    await page
      .getByLabel(/Select feed item: CD PROJEKT/)
      .first()
      .click();
    await expect(detailPane.getByLabel("AI KPI extraction")).toBeVisible();

    // The top navigation bar wraps to a second line at this width (ADR 0047)
    // rather than scrolling sideways — it must not introduce horizontal overflow.
    await expectNoHorizontalOverflow(page.getByLabel("Primary navigation"));

    // At this width the fractional 50/50 grid keeps both panes side-by-side
    // (feed minmax(0,1fr) + a percentage detail track), with no left sidebar to
    // fit beside and no horizontal overflow.
    expect((await expectBox(feedPanel)).width).toBeGreaterThanOrEqual(320);
    expect((await expectBox(detailPane)).width).toBeGreaterThanOrEqual(300);
    await expectNoHorizontalOverflow(feedPanel);
    await expectNoHorizontalOverflow(detailPane);
    await expectNoHorizontalOverflow(filterReset);
    await expectNoHorizontalOverflow(filterToolbar);

    const gridScroll = await contentGrid.evaluate((element) => ({
      clientWidth: element.clientWidth,
      scrollWidth: element.scrollWidth,
    }));
    expect(gridScroll.scrollWidth).toBeLessThanOrEqual(gridScroll.clientWidth + 1);
  });

  test("keeps the notebook panes independently usable as a cockpit panel", async ({ page }) => {
    // Notebooks moved off the sidebar into the cockpit (ADR 0054); open it there.
    await openApp(page);
    await openCockpitPanel(page, "Notebook");

    const workspace = page.getByLabel("Notebooks workspace");
    const companyNav = page.getByLabel("Notebook companies");

    // The panel mounts cleanly and stays usable at any pane width. At L (≥760)
    // the three panes scroll independently; below L the density contract (ADR
    // 0076 D6, U7) stacks them into one column and the SCREEN becomes the
    // bounded scroll container, so per-pane scrollability is tier-dependent.
    await expect(workspace).toBeVisible();
    await expect(companyNav).toBeVisible();
    const paneWidth = await workspace
      .locator("xpath=ancestor::*[contains(@class,'cockpit-pane')]")
      .evaluate((el) => el.clientWidth);
    if (paneWidth >= 760) {
      await expectScrollable(companyNav);
    } else {
      await expectScrollable(page.locator(".notebooks-screen"));
    }
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

    // Selection lives on the row's primary button (post-U9 container+button
    // a11y pattern); the U7 grid columns mean a container-center click can land
    // on a non-interactive meta column, so target the affordance directly.
    await sourceRow.locator(".source-row-main").click();
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
    // Bounded scroll region: configured to scroll without pushing the page. We
    // don't require actual overflow because on a tall window (e.g. the 1280x1440
    // quarter-ultrawide target) the list legitimately fits without scrolling.
    await expectScrollContainer(memberTable);

    await selectedWatchlist.getByRole("button", { name: "Add companies" }).click();
    const pickerList = selectedWatchlist.locator(".watchlist-picker-list");
    await expect(pickerList).toBeVisible();
    await expectScrollContainer(pickerList);
  });
});

async function openApp(page: Page) {
  await page.goto("/");
  await expect(page.getByLabel("Primary navigation")).toBeVisible();
  // Today is the default landing (ADR 0054); these layout checks start from the
  // Inbox feed. Tests for other screens navigate there explicitly after this.
  await page.getByLabel("Primary navigation").getByRole("button", { name: "Inbox" }).click();
  await expect(page.getByRole("heading", { name: "Inbox", exact: true })).toBeVisible();
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

// Asserts a region is a bounded internal scroll container (it will scroll rather
// than push the page) without requiring it to currently overflow — content may
// fit on tall viewports.
async function expectScrollContainer(locator: Locator) {
  const overflowY = await locator.evaluate((element) => window.getComputedStyle(element).overflowY);
  expect(["auto", "scroll"]).toContain(overflowY);
}

async function expectNoHorizontalOverflow(locator: Locator) {
  const overflow = await locator.evaluate((element) => ({
    clientWidth: element.clientWidth,
    scrollWidth: element.scrollWidth,
  }));

  expect(overflow.scrollWidth).toBeLessThanOrEqual(overflow.clientWidth + 1);
}
