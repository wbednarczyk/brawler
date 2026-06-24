import {
  test,
  expect,
  openApp,
  expectNoPageOverflow,
  expectNoHorizontalOverflow,
} from "./helpers/harness";

// Coverage for the mode-based shell (ADR 0054): the Today/Pulse home, the
// left-sidebar spine, the company workspace pin + Advanced-layout entry, and the
// cockpit feed/inspector. Runs across the viewport matrix (playwright.config.ts),
// including the tall/narrow quarter-ultrawide windows. Captures evidence PNGs
// into __shell-evidence__/ for manual review. The browser runtime locale is en.

const EVIDENCE = "tests/browser/__shell-evidence__";

test.describe("mode-based shell (ADR 0054)", () => {
  test("Today is the default home and lays out its attention sections without clipping", async ({
    page,
  }, testInfo) => {
    await openApp(page);

    await expect(page.getByRole("heading", { name: "Today", exact: true })).toBeVisible();
    for (const section of ["What changed", "To verify", "Upcoming reports", "Conviction", "Recent activity"]) {
      await expect(page.getByRole("heading", { name: section, exact: true })).toBeVisible();
    }

    // The card grid must not clip the Review buttons or overflow the page — the
    // exact regression the maintainer caught on Windows.
    await expectNoPageOverflow(page);
    await expectNoHorizontalOverflow(page.locator(".today-body"));

    await page.screenshot({ path: `${EVIDENCE}/today-${testInfo.project.name}.png`, fullPage: true });
  });

  test("the sidebar spine groups modes, library and utilities", async ({ page }) => {
    await openApp(page);
    const nav = page.getByLabel("Primary navigation");
    await expect(nav.getByText("Modes", { exact: true })).toBeVisible();
    await expect(nav.getByText("Library", { exact: true })).toBeVisible();
    await expect(nav.getByText("Utilities", { exact: true })).toBeVisible();
    await expect(nav.getByRole("button", { name: "Today" })).toBeVisible();
    await expect(nav.getByRole("button", { name: "Compare" })).toBeVisible();
  });

  test("the cockpit feed marks only unread items bold and the inspector reads cleanly", async ({
    page,
  }, testInfo) => {
    await openApp(page);
    await page.getByLabel("Primary navigation").getByRole("button", { name: "Cockpit" }).click();

    const cockpit = page.getByLabel("Research cockpit");
    await expect(cockpit).toBeVisible();

    // Read vs unread weight must differ (real-feed behavior): not every row is bold.
    const weights = await page.locator(".cockpit-feed-title").evaluateAll((nodes) =>
      nodes.map((node) => Number(getComputedStyle(node).fontWeight)),
    );
    if (weights.length > 1) {
      const distinct = new Set(weights);
      expect(distinct.size, `feed title weights should differ for read/unread, got ${[...distinct].join(",")}`).toBeGreaterThan(1);
    }

    // Inspect a feed item; the inspector header carries the ticker + title.
    await page.locator(".cockpit-feed-item").first().click();
    const inspector = page.getByLabel("Feed item inspector");
    await expect(inspector).toBeVisible();
    await expectNoHorizontalOverflow(inspector);

    await page.screenshot({ path: `${EVIDENCE}/cockpit-${testInfo.project.name}.png`, fullPage: true });
  });

  test("the company workspace exposes Pin and Advanced layout, and opens the cockpit scoped", async ({
    page,
  }, testInfo) => {
    await openApp(page);
    await page.getByLabel("Primary navigation").getByRole("button", { name: "Companies" }).click();
    // Click the ticker/title area of the row — at narrow widths the row stacks and
    // its lower context block stops click propagation (tracked bug), so target the
    // primary area to reliably select the company across the viewport matrix.
    await page
      .getByRole("button", { name: "Open GPW:CDR workspace" })
      .locator(".company-row-main")
      .click();

    const workspace = page.getByRole("region", { name: "Company workspace" });
    await expect(workspace).toBeVisible();
    await expect(workspace.getByRole("button", { name: /Pin|Pinned/ })).toBeVisible();
    const advanced = workspace.getByRole("button", { name: "Advanced layout" });
    await expect(advanced).toBeVisible();

    await page.screenshot({ path: `${EVIDENCE}/workspace-${testInfo.project.name}.png`, fullPage: true });

    await advanced.click();
    await expect(page.getByLabel("Research cockpit")).toBeVisible();
    await expectNoPageOverflow(page);
  });
});
