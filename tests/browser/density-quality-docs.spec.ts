import { test, expect, openApp, setPaneSize, resetPaneSize, expectNoPageOverflow } from "./helpers/harness";
import type { Locator, Page } from "@playwright/test";

// Panel density matrix — cluster B (ADR 0076 D6 / U7-B): the Quality panel and
// the Report-documents panel. A table-driven runner mirroring
// density-matrix.spec.ts (kept separate so concurrent cluster tasks don't touch
// one file). Each panel opens on the curated company dashboard; `setPaneSize`
// forces the hosting pane to a tier size so the `@container pane (…)` rules fire
// regardless of the real dock cell.

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

// F3a S3 (ADR 0107): opening a company lands the Spółka screen for the seeded
// GPW:CDR company; each tool opens via the ⌘K palette.
async function openCompany(page: Page) {
  await nav(page).getByRole("button", { name: "Companies" }).click();
  await page.locator('[data-company-id="company_gpw_cdr"] .company-row-main').click();
  await page.getByRole("region", { name: "Company view" }).waitFor();
}

// Returns `.spolka-layout` — the density contracts' `container: pane / size`
// (spolka.css), not the "Workshop tool" group itself.
async function openTool(page: Page, label: string): Promise<PaneLocator> {
  await openCompany(page);
  await page.keyboard.press("Control+K");
  const palette = page.getByRole("dialog", { name: "Command palette" });
  await palette.getByLabel("Search commands").fill(label);
  await palette.getByRole("option", { name: label, exact: true }).first().click();
  await expect(page.getByRole("group", { name: "Workshop tool" })).toBeVisible();
  return page.locator(".spolka-layout");
}

async function openQuality(page: Page): Promise<PaneLocator> {
  return openTool(page, "Open quality");
}

async function openDocuments(page: Page): Promise<PaneLocator> {
  return openTool(page, "Open documents");
}

async function openCoverage(page: Page): Promise<PaneLocator> {
  return openTool(page, "Open coverage");
}

async function box(locator: Locator) {
  const b = await locator.boundingBox();
  expect(b, "element rendered").not.toBeNull();
  return b!;
}

const PANEL_CONTRACTS: PanelContract[] = [
  {
    panel: "Quality",
    open: openQuality,
    tiers: {
      // S: scorecard chips + criteria list; the inline expression folds away
      // (reachable behind the row's expansion — asserted in the RTL unit test).
      S: async (_page, pane) => {
        await expect(pane.locator(".quality-scorecard-summary")).toBeVisible();
        await expect(pane.locator(".quality-main .ui-list-rows").first()).toBeVisible();
        await expect(pane.locator(".quality-expression").first()).toBeHidden();
      },
      // M: the expression column is shown inline.
      M: async (_page, pane) => {
        await expect(pane.locator(".quality-expression").first()).toBeVisible();
      },
      // L: the evaluation history is a side panel to the RIGHT of the criteria.
      L: async (_page, pane) => {
        const list = pane.locator(".quality-history-list");
        await expect(list).toBeVisible();
        const main = await box(pane.locator(".quality-main"));
        const history = await box(pane.locator(".quality-history"));
        expect(history.x, "history beside the criteria column").toBeGreaterThan(
          main.x + main.width / 2,
        );
      },
      // short: chips + criteria stay; the history folds (collapsed by default).
      short: async (_page, pane) => {
        await expect(pane.locator(".quality-main .ui-list-rows").first()).toBeVisible();
        await expect(pane.locator(".quality-history-list")).toHaveCount(0);
      },
    },
  },
  {
    panel: "Report documents",
    open: openDocuments,
    tiers: {
      // S: kind label + filename (middle-ellipsis link) + date — no chips, no
      // trailing action slot (ADR 0077 §2 grouped redesign).
      S: async (_page, pane) => {
        await expect(pane.getByRole("link").first()).toBeVisible();
        await expect(pane.locator(".doc-date").first()).toBeVisible();
        await expect(pane.locator(".doc-chips").first()).toBeHidden();
      },
      // M: + kind/status chips. The extract action is present but folded to an
      // icon (its label hides until L).
      M: async (_page, pane) => {
        await expect(pane.locator(".doc-chips").first()).toBeVisible();
        await expect(pane.getByRole("button", { name: "Extract data" }).first()).toBeVisible();
        await expect(pane.locator(".doc-extract-label").first()).toBeHidden();
      },
      // L: the per-document extract-data action gains its label.
      L: async (_page, pane) => {
        await expect(pane.getByRole("button", { name: "Extract data" }).first()).toBeVisible();
        await expect(pane.locator(".doc-extract-label").first()).toBeVisible();
      },
      // short: list only — kind label + filename + date; chips and the extract
      // action are dropped.
      short: async (_page, pane) => {
        await expect(pane.getByRole("link").first()).toBeVisible();
        await expect(pane.locator(".doc-date").first()).toBeVisible();
        await expect(pane.locator(".doc-chips").first()).toBeHidden();
        await expect(pane.locator(".doc-extract").first()).toBeHidden();
      },
    },
  },
  {
    panel: "Coverage",
    open: openCoverage,
    tiers: {
      // The coverage table does not fold columns by tier; instead the whole table
      // is wide content that scrolls inside its bounded `.coverage-scroll` wrapper
      // (data-hscroll). At S the table still renders in full inside the scroller,
      // and the runner's per-tier `expectNoPageOverflow` proves the pane itself
      // does not gain a horizontal scrollbar.
      S: async (_page, pane) => {
        await expect(pane.locator(".coverage-scroll")).toBeVisible();
        await expect(pane.locator("table.coverage-table")).toBeVisible();
      },
      M: async (_page, pane) => {
        await expect(pane.locator("table.coverage-table")).toBeVisible();
      },
      L: async (_page, pane) => {
        await expect(pane.locator("table.coverage-table")).toBeVisible();
        await expect(pane.locator(".coverage-legend")).toBeVisible();
      },
    },
  },
];

test.describe("panel density matrix — quality + documents", { tag: "@clickable" }, () => {
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
