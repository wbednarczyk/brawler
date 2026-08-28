import {
  test,
  expect,
  openApp,
  expectNoPageOverflow,
  expectNoHorizontalOverflow,
} from "./helpers/harness";

// Coverage for the mode-based shell (ADR 0054): the Today/Pulse home, the
// left-sidebar spine, and the Spółka company feed workshop tool. Runs across
// the viewport matrix (playwright.config.ts), including the tall/narrow
// quarter-ultrawide windows. Captures evidence PNGs into __shell-evidence__/
// for manual review. The browser runtime locale is en.

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

  test("the sidebar spine groups modes, library and utilities — no Views group", async ({ page }) => {
    await openApp(page);
    const nav = page.getByLabel("Primary navigation");
    await expect(nav.getByText("Modes", { exact: true })).toBeVisible();
    await expect(nav.getByText("Library", { exact: true })).toBeVisible();
    await expect(nav.getByText("Utilities", { exact: true })).toBeVisible();
    await expect(nav.getByRole("button", { name: "Today" })).toBeVisible();
    // ADR 0108: the docking engine, its named views, and the "Widoki"/Views
    // sidebar group are retired — Modes is Today/Inbox/Company only.
    await expect(nav.getByText("Views", { exact: true })).toHaveCount(0);
    await expect(nav.getByRole("button", { name: /^Legacy dashboard/ })).toHaveCount(0);
  });

  // ADR 0108: opening a company lands the Spółka screen directly (no cockpit
  // dashboard to host it); the company-scoped Feed workshop tool still
  // distinguishes read/unread weight.
  test("the Spółka feed tool marks only unread items bold", async ({
    page,
  }, testInfo) => {
    await openApp(page);
    await page.getByLabel("Primary navigation").getByRole("button", { name: "Companies" }).click();
    await page.getByRole("button", { name: "Open GPW:CDR" }).click();
    const spolka = page.getByRole("region", { name: "Company view", exact: true });
    await expect(spolka).toBeVisible();
    await spolka.getByLabel("Workshop").getByRole("button", { name: "Feed", exact: true }).click();
    const tool = spolka.getByLabel("Workshop tool");
    await expect(tool).toBeVisible();
    await expect(tool).toHaveAttribute("data-tool", "feed");

    const rows = page.locator(".company-feed-row");
    await expect(rows.first()).toBeVisible();

    // Unread rows render bold (weight 700); a read row (if any is seeded for
    // this company) renders a lighter weight — per-row correctness rather
    // than requiring a specific read/unread MIX in the data (CD PROJEKT's
    // company-scoped feed has no interactive mark-read affordance).
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
    await expectNoHorizontalOverflow(tool.getByLabel("Company feed", { exact: true }));

    await page.screenshot({ path: `${EVIDENCE}/spolka-feed-${testInfo.project.name}.png`, fullPage: true });
  });

  test("opening a company lands the Spółka screen directly, fundamentals reachable via the workshop", async ({
    page,
  }, testInfo) => {
    await openApp(page);
    await page.getByLabel("Primary navigation").getByRole("button", { name: "Companies" }).click();
    await page.getByRole("button", { name: "Open GPW:CDR" }).click();

    const spolka = page.getByRole("region", { name: "Company view", exact: true });
    await expect(spolka).toBeVisible();
    await spolka.getByLabel("Workshop").getByRole("button", { name: "Fundamentals", exact: true }).click();
    await expect(page.getByLabel("Company fundamentals")).toBeVisible();

    await page.screenshot({ path: `${EVIDENCE}/workspace-${testInfo.project.name}.png`, fullPage: true });
    await expectNoPageOverflow(page);
  });
});
