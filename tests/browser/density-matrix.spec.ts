import { test, expect, openApp, setPaneSize, resetPaneSize, expectNoPageOverflow } from "./helpers/harness";
import type { Locator, Page } from "@playwright/test";

// Panel density matrix (ADR 0076 D6 / U7). Every cockpit panel/screen defines
// behavior at pane-width tiers (S <420 · M 420–760 · L >760) and height tiers
// (short <480 · tall ≥480), driven by `@container pane (…)` rules on the named
// `pane` size container (.cockpit-pane / .workspace, added in U7). This spec is
// the table-driven runner the per-panel CLUSTER tasks extend: each appends a
// PANEL_CONTRACTS entry with its opener and per-tier assertions. `setPaneSize`
// forces the hosting pane's inline size so the container queries fire regardless
// of the window/dock-cell size (jsdom has no container queries, so the tier
// behavior can only be asserted here in a real browser — the disclosure/toggle
// semantics are unit-tested in src/ui/primitives.test.tsx).

type PaneLocator = Locator;
// Each check runs with the pane already forced to its tier size.
type TierCheck = (page: Page, pane: PaneLocator) => Promise<void>;

type PanelContract = {
  panel: string;
  // Opens the panel and returns the pane Locator to size (a panel may live in a
  // multi-pane dashboard, so the pane is not always the first `.cockpit-pane`).
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

// Reference opener: the company Notebook panel owns the "existing container
// query" the Notebook contract row references — the `.notebook-workspace`
// list∥detail grid in CompanyNotebookSection. NOTE: the palette entry
// `openCockpitPanel("Notebook")` opens the GLOBAL NotebooksScreen (a rigid
// 3-column grid with no container query yet — its contract is a separate cluster
// task), so the tier behavior is exercised through the per-company dashboard
// panel instead, which hosts the retiered query (notebook-shared.css).
async function openCompanyNotebook(page: Page): Promise<PaneLocator> {
  await nav(page).getByRole("button", { name: "Companies" }).click();
  await page.getByRole("button", { name: "Open GPW:CDR dashboard" }).click();
  const cockpit = page.getByLabel("Research cockpit");
  await expect(cockpit).toBeVisible();
  // dockview mounts only the active panel body per group, so activate the tab.
  await cockpit.getByRole("button", { name: "Notebook", exact: true }).first().click();
  const pane = page.locator(".cockpit-pane", { has: page.locator(".notebook-workspace") });
  await expect(pane).toBeVisible();
  return pane;
}

async function notebookColumns(page: Page): Promise<{ list: NonNullable<Awaited<ReturnType<Locator["boundingBox"]>>>; detail: NonNullable<Awaited<ReturnType<Locator["boundingBox"]>>> }> {
  const list = await page.locator(".notebook-list").first().boundingBox();
  const detail = await page.locator(".notebook-detail").first().boundingBox();
  expect(list, "notebook list rendered").not.toBeNull();
  expect(detail, "notebook detail rendered").not.toBeNull();
  return { list: list!, detail: detail! };
}

async function expectStacked(page: Page) {
  const { list, detail } = await notebookColumns(page);
  // One column: list and detail share the left edge, detail sits BELOW the list.
  expect(Math.abs(detail.x - list.x), "detail shares the list's column").toBeLessThan(6);
  expect(detail.y, "detail stacked below the list").toBeGreaterThan(list.y + 10);
}

async function expectSideBySide(page: Page) {
  const { list, detail } = await notebookColumns(page);
  // Two columns: detail sits to the RIGHT of the list on the same row.
  expect(detail.x, "detail beside the list").toBeGreaterThan(list.x + list.width / 2);
  expect(Math.abs(detail.y - list.y), "list and detail on one row").toBeLessThan(40);
}

const PANEL_CONTRACTS: PanelContract[] = [
  {
    panel: "Notebook",
    open: openCompanyNotebook,
    tiers: {
      // S/M stack below the L tier (pane ≤ 759px); L shows list ∥ detail. The
      // S-tier single-column TOGGLE (list OR detail) is a follow-up cluster task
      // (cluster C) — only the stacking boundary is asserted here.
      S: (page) => expectStacked(page),
      M: (page) => expectStacked(page),
      L: (page) => expectSideBySide(page),
    },
  },
];

test.describe("panel density matrix", { tag: "@clickable" }, () => {
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
