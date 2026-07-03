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
    // There is no standalone blank "Cockpit" nav entry (ADR 0057 decision 5): the
    // entry point is the "+ New view" creator. Apply the "Daily triage" preset
    // (feed + inspector + claims) to reach the same feed/inspector panels this
    // test exercises.
    const nav = page.getByLabel("Primary navigation");
    await nav.getByRole("button", { name: "New view" }).click();
    const createModal = page.getByRole("dialog", { name: "New view" });
    await createModal.getByLabel("View name").fill("Daily triage test view");
    await createModal.getByRole("button", { name: "Create view" }).click();

    const cockpit = page.getByLabel("Research cockpit");
    await expect(cockpit).toBeVisible();
    await cockpit.getByLabel("Preset").selectOption({ label: "Daily triage" });
    await expect(page.locator(".cockpit-feed-item").first()).toBeVisible();

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

  test("opening a company lands the cockpit dashboard scoped to it", async ({
    page,
  }, testInfo) => {
    await openApp(page);
    await page.getByLabel("Primary navigation").getByRole("button", { name: "Companies" }).click();
    // Click the ticker/title area of the row — at narrow widths the row stacks and
    // its lower context block stops click propagation (tracked bug), so target the
    // primary area to reliably select the company across the viewport matrix.
    await page.locator('[data-company-id="company_gpw_cdr"] .company-row-main').click();

    // Opening a company IS the deep-dive now (ADR 0057): no intermediate classic
    // workspace / Advanced-layout button — it lands the curated cockpit dashboard
    // scoped to the company, showing every surface (Fundamentals, report diff, …)
    // at once.
    const cockpit = page.getByRole("region", { name: "Research cockpit" });
    await expect(cockpit).toBeVisible();
    await expect(page.getByLabel("Company fundamentals")).toBeVisible();

    await page.screenshot({ path: `${EVIDENCE}/workspace-${testInfo.project.name}.png`, fullPage: true });
    await expectNoPageOverflow(page);
  });
});
