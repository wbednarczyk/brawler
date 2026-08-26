import { test, expect, openApp, setPaneSize, resetPaneSize, expectNoPageOverflow } from "./helpers/harness";
import type { Locator, Page } from "@playwright/test";

// U7 cluster A — density contracts for the two curated-dashboard company panels
// (Fundamentals + company Feed). Mirrors the table-driven runner in
// density-matrix.spec.ts (ADR 0076 D6): each PANEL_CONTRACTS entry opens the
// panel, returns the `.cockpit-pane` to size, and asserts the per-tier
// visibility/layout rules after `setPaneSize` forces the pane to the tier size
// (container queries resolve against the forced pane size — jsdom can't fire
// them, so the tier SWITCH is browser-only; the fold TOGGLE semantics are
// unit-tested in the panels' *.test.tsx files).

type PaneLocator = Locator;
type TierCheck = (page: Page, pane: PaneLocator) => Promise<void>;

type PanelContract = {
  panel: string;
  open: (page: Page) => Promise<PaneLocator>;
  tiers: { S?: TierCheck; M?: TierCheck; L?: TierCheck; short?: TierCheck };
};

const TIER_SIZE = {
  S: { width: 380, height: 700 },
  M: { width: 600, height: 700 },
  L: { width: 900, height: 700 },
  short: { width: 900, height: 440 },
} as const;

const TIER_ORDER = ["S", "M", "L", "short"] as const;

function nav(page: Page) {
  return page.getByLabel(/Primary navigation|Nawigacja główna/);
}

// Open the curated company dashboard for GPW:CDR (shell.spec "opening a company
// lands the cockpit dashboard"), then activate a pinned panel's tab and return
// the `.cockpit-pane` that hosts it.
async function openCompanyDashboard(page: Page, tabName: string, panelLabel: string): Promise<PaneLocator> {
  await nav(page).getByRole("button", { name: "Companies" }).click();
  await page.locator('[data-company-id="company_gpw_cdr"] .company-row-main').click();
  // F3a S1 (ADR 0107): the row opens Spółka; the legacy cockpit is reached via the sidebar Dashboard entry until S3 freezes it.
  await page.getByLabel("Primary navigation").getByRole("button", { name: "Dashboard" }).click();
  const cockpit = page.getByRole("region", { name: "Research cockpit" });
  await expect(cockpit).toBeVisible();
  // dockview mounts only the active panel body per group — activate the tab.
  await cockpit.getByRole("button", { name: tabName, exact: true }).first().click();
  const pane = page.locator(".cockpit-pane", { has: page.getByLabel(panelLabel) });
  await expect(pane).toBeVisible();
  return pane;
}

async function box(locator: Locator) {
  const bounds = await locator.boundingBox();
  expect(bounds, "element rendered with a box").not.toBeNull();
  return bounds!;
}

const PANEL_CONTRACTS: PanelContract[] = [
  {
    panel: "Fundamentals",
    open: (page) => openCompanyDashboard(page, "Fundamentals", "Company fundamentals"),
    tiers: {
      // S: Autopilot section collapses to one row + expand (summary toggle shown,
      // full field folded); sections stack; matrix scrolls.
      S: async (_page, pane) => {
        await expect(pane.locator(".fundamentals-autopilot-toggle")).toBeVisible();
        await expect(pane.locator(".fundamentals-autopilot-body")).toBeHidden();
        await expect(pane.getByLabel("Financial facts matrix")).toBeVisible();
      },
      // M: Autopilot expanded (no toggle); the two forms stack in one column.
      M: async (_page, pane) => {
        await expect(pane.locator(".fundamentals-autopilot-toggle")).toBeHidden();
        await expect(pane.locator(".fundamentals-autopilot-body")).toBeVisible();
        const create = await box(pane.getByLabel("Create reporting period"));
        const add = await box(pane.getByLabel("Add financial fact"));
        expect(add.y, "add-fact form stacked below the period form at M").toBeGreaterThan(create.y + 10);
        expect(Math.abs(add.x - create.x), "forms share one column at M").toBeLessThan(6);
      },
      // L: forms side-by-side beside the matrix.
      L: async (_page, pane) => {
        const create = await box(pane.getByLabel("Create reporting period"));
        const add = await box(pane.getByLabel("Add financial fact"));
        expect(add.x, "add-fact form beside the period form at L").toBeGreaterThan(create.x + create.width / 2);
        expect(Math.abs(add.y - create.y), "forms on one row at L").toBeLessThan(40);
      },
      // short: only matrix + section headers; forms fold behind a disclosure.
      short: async (_page, pane) => {
        await expect(pane.locator(".fundamentals-forms-toggle")).toBeVisible();
        await expect(pane.locator(".fundamentals-forms-grid")).toBeHidden();
        await expect(pane.getByLabel("Financial facts matrix")).toBeVisible();
      },
    },
  },
  {
    panel: "Feed (company)",
    open: async (page) => {
      const pane = await openCompanyDashboard(page, "Feed", "Company feed");
      // Select a feed item so the detail renders (split-pane at L, stacked otherwise).
      await pane.locator("[data-company-feed-row]").first().click();
      await expect(pane.locator(".company-feed-detail")).toBeVisible();
      return pane;
    },
    tiers: {
      // S: item = badge+title+date; the summary line folds away.
      S: async (_page, pane) => {
        await expect(pane.locator(".company-feed-row h3").first()).toBeVisible();
        await expect(pane.locator(".feed-row-summary").first()).toBeHidden();
      },
      // M: + summary line.
      M: async (_page, pane) => {
        await expect(pane.locator(".feed-row-summary").first()).toBeVisible();
      },
      // L: detail split-pane — detail sits to the RIGHT of the selected row.
      L: async (_page, pane) => {
        const row = await box(pane.locator(".company-feed-row-block:has(.company-feed-detail) .company-feed-row"));
        const detail = await box(pane.locator(".company-feed-detail"));
        expect(detail.x, "detail beside the row at L").toBeGreaterThan(row.x + row.width / 2);
        expect(Math.abs(detail.y - row.y), "row and detail on one row at L").toBeLessThan(40);
      },
      // short: list only — the split-pane collapses; detail stacks below the row
      // (not the L-tier side column). Base detail styling insets it by a small
      // margin, so assert it is NOT in the right column rather than pixel-aligned.
      short: async (_page, pane) => {
        const row = await box(pane.locator(".company-feed-row-block:has(.company-feed-detail) .company-feed-row"));
        const detail = await box(pane.locator(".company-feed-detail"));
        expect(detail.y, "detail stacked below the row at short").toBeGreaterThan(row.y + 10);
        expect(detail.x, "detail is not a right-hand side pane at short").toBeLessThan(row.x + row.width / 2);
      },
    },
  },
];

test.describe("company panel density matrix", { tag: "@clickable" }, () => {
  for (const contract of PANEL_CONTRACTS) {
    test(`${contract.panel} honors its pane-density tiers`, async ({ page }) => {
      await openApp(page);
      const pane = await contract.open(page);
      for (const tier of TIER_ORDER) {
        const check = contract.tiers[tier];
        if (!check) continue;
        await setPaneSize(page, { ...TIER_SIZE[tier], pane });
        await check(page, pane);
        await expectNoPageOverflow(page);
        await resetPaneSize(page, pane);
      }
    });
  }
});

// Periods × deltas section (v0.61 §A5, ADR 0089 dec. 1). The quarterly view is
// deliberate wide content (value + Δ QoQ + Δ YoY per period); the narrow-window
// rule requires it to scroll inside its own bounded, contained scroller so it
// never forces a pane- or page-level horizontal scrollbar.
test.describe("Fundamentals periods × deltas layout", { tag: "@clickable" }, () => {
  test("the wide quarterly table scrolls inside its own container, not the pane", async ({
    page,
  }) => {
    await openApp(page);
    const pane = await openCompanyDashboard(page, "Fundamentals", "Company fundamentals");
    const section = pane.locator("section.fundamentals-periods");
    await expect(section).toBeVisible();

    // Switch to the wide quarterly grain (three sub-columns per period).
    await section.getByRole("button", { name: /Quarterly|Kwartalny/ }).click();
    const scroller = section.locator(".fundamentals-periods-scroll");
    await expect(scroller).toBeVisible();
    // The sanctioned wide-content scroller carries data-hscroll (layout-gate exempt).
    await expect(scroller).toHaveAttribute("data-hscroll");

    await setPaneSize(page, { width: 380, height: 700, pane });

    // No global horizontal scroll despite the wide table.
    await expectNoPageOverflow(page);

    // The table overflows its OWN scroller (proving it is genuinely wide)…
    const inner = await scroller.evaluate((el) => ({
      scrollWidth: el.scrollWidth,
      clientWidth: el.clientWidth,
    }));
    expect(
      inner.scrollWidth,
      "the quarterly table is wide enough to exercise its scroller",
    ).toBeGreaterThan(inner.clientWidth);

    // …while the pane itself never overflows horizontally (containment holds).
    const paneBox = await pane.evaluate((el) => ({
      scrollWidth: el.scrollWidth,
      clientWidth: el.clientWidth,
    }));
    expect(
      paneBox.scrollWidth,
      `the pane overflows horizontally (${paneBox.scrollWidth} > ${paneBox.clientWidth})`,
    ).toBeLessThanOrEqual(paneBox.clientWidth + 1);

    await resetPaneSize(page, pane);
  });
});
