import {
  test,
  expect,
  openApp,
  setPaneSize,
  resetPaneSize,
  expectNoPageOverflow,
  expectNoOverlap,
} from "./helpers/harness";
import type { Locator, Page } from "@playwright/test";

// U7 cluster A — density contracts for two Spółka workshop tools (Fundamentals
// + company Feed, F3a S3, ADR 0107). Mirrors the table-driven runner in
// density-matrix.spec.ts (ADR 0076 D6): each PANEL_CONTRACTS entry opens the
// tool, returns the "Workshop tool" container to size, and asserts the
// per-tier visibility/layout rules after `setPaneSize` forces the pane to the
// tier size (container queries resolve against the forced pane size — jsdom
// can't fire them, so the tier SWITCH is browser-only; the fold TOGGLE
// semantics are unit-tested in the panels' *.test.tsx files).

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

// Open GPW:CDR's Spółka screen, then open the named workshop tool via the ⌘K
// palette and return `.spolka-layout` — the density contracts' `container:
// pane / size` (spolka.css), not the "Workshop tool" group itself.
async function openCompanyTool(page: Page, toolLabel: string): Promise<PaneLocator> {
  await nav(page).getByRole("button", { name: "Companies" }).click();
  await page.locator('[data-company-id="company_gpw_cdr"] .company-row-main').click();
  await page.getByRole("region", { name: "Company view" }).waitFor();
  await page.keyboard.press("Control+K");
  const palette = page.getByRole("dialog", { name: "Command palette" });
  await palette.getByLabel("Search commands").fill(toolLabel);
  await palette.getByRole("button", { name: toolLabel, exact: true }).first().click();
  await expect(page.getByRole("group", { name: "Workshop tool" })).toBeVisible();
  return page.locator(".spolka-layout");
}

async function box(locator: Locator) {
  const bounds = await locator.boundingBox();
  expect(bounds, "element rendered with a box").not.toBeNull();
  return bounds!;
}

const PANEL_CONTRACTS: PanelContract[] = [
  {
    panel: "Fundamentals",
    open: (page) => openCompanyTool(page, "Open fundamentals"),
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
      const pane = await openCompanyTool(page, "Open feed");
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
    const pane = await openCompanyTool(page, "Open fundamentals");
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

// F4a S2 — Companies library density cell (docs/plans/frontend-v2-f4a.md §
// Companies library, shared guardrail table). This is a LANGUAGE pass, not a
// redesign: no new fold/collapse tiers ship here — the test proves the
// existing library layout (add form + toolbar + row list) holds together
// (no sibling overlap, no page/pane horizontal overflow) across the pane
// width tiers, on the "rich" default seed (28 companies — the default
// smoke-runtime scenario, the densest of the three named scenarios; see
// src/test/scenarios/scenarios.ts).
test.describe("Companies library density (companies-library cell)", { tag: "@clickable" }, () => {
  test("the add form and the company list never overlap or overflow at S/M/L", async ({ page }) => {
    await openApp(page);
    await page.getByLabel(/Primary navigation|Nawigacja główna/).getByRole("button", { name: "Companies" }).click();
    await expect(page.getByLabel("Companies list")).toBeVisible();

    const pane = page.locator(".workspace");
    const form = page.locator(".company-form");
    const list = page.getByLabel("Companies list");

    for (const tier of ["S", "M", "L"] as const) {
      await setPaneSize(page, { ...TIER_SIZE[tier], pane });
      await expect(form).toBeVisible();
      await expect(list).toBeVisible();
      await expectNoOverlap(form, list, "company add form and company list");
      await expectNoPageOverflow(page);
      await resetPaneSize(page, pane);
    }
  });
});
