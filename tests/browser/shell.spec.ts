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
  test("Today is the default home: the Dziś v2 day queue, no clipping", async ({
    page,
  }, testInfo) => {
    await openApp(page);

    await expect(page.getByRole("heading", { name: "Today", exact: true })).toBeVisible();

    // Rebuilt to Dziś v2 (F2 #422, docs/plans/frontend-v2-f2.md): the old
    // stream region + counter-tile filters are gone by design (plan decision
    // 7) — the delta header is the Today-recognition anchor now.
    await expect(page.locator(".dayq-delta-header")).toBeVisible();

    // The queue must not clip its row actions or overflow the page — the
    // exact regression the maintainer caught on Windows — and no row may force a
    // panel-internal horizontal scrollbar (ticker/date never truncate, K1).
    await expectNoPageOverflow(page);
    await expectNoHorizontalOverflow(page.locator(".dayq-screen-body"));

    await page.screenshot({ path: `${EVIDENCE}/today-${testInfo.project.name}.png`, fullPage: true });
  });

  test("the sidebar spine groups modes, library and utilities", async ({ page }) => {
    await openApp(page);
    const nav = page.getByLabel("Primary navigation");
    await expect(nav.getByText("Modes", { exact: true })).toBeVisible();
    await expect(nav.getByText("Library", { exact: true })).toBeVisible();
    await expect(nav.getByText("Utilities", { exact: true })).toBeVisible();
    await expect(nav.getByRole("button", { name: "Today" })).toBeVisible();
  });

  // F3a S3 (ADR 0107 decision 5): the frozen dashboard's linked Feed/Inspector
  // panels start CLOSED (DASHBOARD_CLOSED_LINKED) and the "Show panel: …"
  // command that used to reopen them is gone — there is no UI path left to
  // exercise the shared-selection linked workflow (feed → inspector →
  // claims/diff) from a legacy dashboard specifically; that regression is
  // covered instead in `CockpitScreen.test.tsx` ("linked selection still
  // drives inspector, claims and diff selection in a frozen view") via a
  // seeded named view. This spec keeps the achievable half: the dashboard's
  // own company-scoped Feed panel (`companyFeed`, always open by default)
  // still distinguishes read/unread weight.
  test("the legacy dashboard's company feed marks only unread items bold", async ({
    page,
  }, testInfo) => {
    await openApp(page);
    await page.getByLabel("Primary navigation").getByRole("button", { name: "Legacy dashboard · CDR" }).click();
    const cockpit = page.getByLabel("Research cockpit");
    await expect(cockpit).toBeVisible();
    await expect(cockpit.getByText("Layout frozen until the engine decision")).toBeVisible();

    const rows = page.locator(".company-feed-row");
    await expect(rows.first()).toBeVisible();

    // Unread rows render bold (weight 700); a read row (if any is seeded for
    // this company) renders a lighter weight — per-row correctness rather
    // than requiring a specific read/unread MIX in the data (CD PROJEKT's
    // company-scoped feed has no interactive mark-read affordance, unlike
    // the pre-freeze global/linked panel this spec used to drive).
    const rowInfo = await page.locator(".company-feed-row").evaluateAll((nodes) =>
      nodes.map((node) => ({
        unread: node.classList.contains("unread"),
        weight: Number(getComputedStyle(node.querySelector("h3")!).fontWeight),
      })),
    );
    for (const { unread, weight } of rowInfo) {
      if (unread) {
        expect(weight, "an unread row must render bold").toBe(700);
      } else {
        expect(weight, "a read row must not render bold").toBeLessThan(700);
      }
    }
    // Scoped to the company feed panel itself (matching the original test's
    // Inspector-scoped check) — a whole-cockpit sweep would also catch the
    // curated dashboard's OTHER default panels (e.g. a pre-existing Basic
    // info/insider-block overflow at this width), which is outside what this
    // spec is about.
    await expectNoHorizontalOverflow(page.getByLabel("Company feed", { exact: true }));

    await page.screenshot({ path: `${EVIDENCE}/cockpit-${testInfo.project.name}.png`, fullPage: true });
  });

  test("the legacy dashboard row opens the frozen cockpit scoped to its company", async ({
    page,
  }, testInfo) => {
    await openApp(page);
    // F3a S3 (ADR 0107 decision 5): opening a company now lands the Spółka
    // screen directly; the four legacy `dashboard:*` layouts stay reachable
    // read-only via their "Legacy dashboard · TICKER" Widoki row.
    await page.getByLabel("Primary navigation").getByRole("button", { name: "Legacy dashboard · CDR" }).click();

    const cockpit = page.getByRole("region", { name: "Research cockpit" });
    await expect(cockpit).toBeVisible();
    await expect(page.getByLabel("Company fundamentals")).toBeVisible();
    // Frozen: no structure-mutating toolbar control, no company/preset
    // selector (the row already fixed the company), and the strip is visible.
    await expect(cockpit.getByRole("button", { name: "Add panel" })).toHaveCount(0);
    await expect(cockpit.getByLabel("View company")).toHaveCount(0);
    await expect(cockpit.getByText("Layout frozen until the engine decision")).toBeVisible();

    await page.screenshot({ path: `${EVIDENCE}/workspace-${testInfo.project.name}.png`, fullPage: true });
    await expectNoPageOverflow(page);
  });
});
