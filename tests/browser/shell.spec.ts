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
  test("Today is the default home: a prioritized stream + counters column, no clipping", async ({
    page,
  }, testInfo) => {
    await openApp(page);

    await expect(page.getByRole("heading", { name: "Today", exact: true })).toBeVisible();

    // Redesigned to journey J1 (ADR 0076 U-Rb): one stream region + a counters
    // column with three filter tiles — no 4-column card splat.
    await expect(page.getByRole("region", { name: "Attention stream" })).toBeVisible();
    const counters = page.getByRole("group", { name: "Filter the stream" });
    await expect(counters.getByRole("button", { name: /Autopilot/ })).toBeVisible();
    await expect(counters.getByRole("button", { name: /To verify/ })).toBeVisible();
    await expect(counters.getByRole("button", { name: /Upcoming reports/ })).toBeVisible();

    // The stream must not clip its Review buttons or overflow the page — the
    // exact regression the maintainer caught on Windows — and no row may force a
    // panel-internal horizontal scrollbar (ticker/date never truncate, K1).
    await expectNoPageOverflow(page);
    await expectNoHorizontalOverflow(page.locator(".today-body"));
    await expectNoHorizontalOverflow(page.locator(".today-stream-region"));

    await page.screenshot({ path: `${EVIDENCE}/today-${testInfo.project.name}.png`, fullPage: true });
  });

  test("the sidebar spine groups modes, library and utilities", async ({ page }) => {
    await openApp(page);
    const nav = page.getByLabel("Primary navigation");
    await expect(nav.getByText("Modes", { exact: true })).toBeVisible();
    await expect(nav.getByText("Library", { exact: true })).toBeVisible();
    await expect(nav.getByText("Utilities", { exact: true })).toBeVisible();
    await expect(nav.getByRole("button", { name: "Today" })).toBeVisible();
    // Compare is a live mode destination again (ADR 0089, v0.61) — restored
    // under Dashboard once the comparison read model gave the mode content.
    await expect(nav.getByRole("button", { name: "Compare" })).toBeVisible();
  });

  test("the cockpit feed marks only unread items bold and the inspector reads cleanly", async ({
    page,
  }, testInfo) => {
    await openApp(page);
    // Dashboard redesign (epic c793ca1): the cockpit is one company-scoped
    // Dashboard; the linked Feed + Inspector panels (the feed-selection model) are
    // no longer a preset, but stay reachable via the command palette. Open a
    // company Dashboard from the Companies list, then show the Feed panel.
    await page.getByLabel("Primary navigation").getByRole("button", { name: "Companies" }).click();
    await page.locator('[data-company-id="company_gpw_cdr"] .company-row-main').click();
    const cockpit = page.getByLabel("Research cockpit");
    await expect(cockpit).toBeVisible();

    await page.keyboard.press("Control+K");
    const palette = page.getByRole("dialog", { name: "Command palette" });
    await palette.getByLabel("Search commands").fill("Show panel: Feed");
    await palette.getByRole("button", { name: "Show panel: Feed", exact: true }).click();
    await expect(page.locator(".cockpit-feed-item").first()).toBeVisible();

    // Read vs unread weight must differ (real-feed behavior): not every row is bold.
    const weights = await page.locator(".cockpit-feed-title").evaluateAll((nodes) =>
      nodes.map((node) => Number(getComputedStyle(node).fontWeight)),
    );
    if (weights.length > 1) {
      const distinct = new Set(weights);
      expect(distinct.size, `feed title weights should differ for read/unread, got ${[...distinct].join(",")}`).toBeGreaterThan(1);
    }

    // Inspect a feed item; the inspector header carries the ticker + title. Open
    // the Inspector panel from the palette (also closed by default on a Dashboard).
    await page.locator(".cockpit-feed-item").first().click();
    await page.keyboard.press("Control+K");
    const palette2 = page.getByRole("dialog", { name: "Command palette" });
    await palette2.getByLabel("Search commands").fill("Show panel: Inspector");
    await palette2.getByRole("button", { name: "Show panel: Inspector", exact: true }).click();
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
