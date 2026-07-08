import { test, expect, openApp, openCockpitPanel, setPaneSize, expectNoPageOverflow } from "./helpers/harness";
import type { Locator, Page } from "@playwright/test";

// Density contracts — Notebook + Claims panels (ADR 0076 D6 / U7 cluster C).
// The cross-cutting infrastructure (named `pane` size container on .cockpit-pane
// and .workspace, `setPaneSize`) lands in U7-core; this spec asserts the two
// panel rows verbatim from docs/ui-authoring.md:
//   Notebook — S: single column (list OR detail, toggled) · short: list only,
//              editor on select.
//   Claims   — S: list only, composer behind "Dodaj obietnicę" button ·
//              M: list + inline composer · L: + verdict detail column ·
//              short: queue counts + top 3 due.
// The tier switch itself is CSS-only (container queries), so — like the shared
// density-matrix runner — it can only be exercised in a real browser. The toggle
// STATE (data flags, disclosure aria) is unit-tested in the component test files.

function nav(page: Page) {
  return page.getByLabel(/Primary navigation|Nawigacja główna/);
}

async function openCompanyDashboardPanel(page: Page, tab: string, rootSelector: string): Promise<Locator> {
  await nav(page).getByRole("button", { name: "Companies" }).click();
  await page.getByRole("button", { name: "Open GPW:CDR dashboard" }).click();
  const cockpit = page.getByLabel("Research cockpit");
  await expect(cockpit).toBeVisible();
  // dockview mounts only the active panel body per group, so activate the tab.
  await cockpit.getByRole("button", { name: tab, exact: true }).first().click();
  const pane = page.locator(".cockpit-pane", { has: page.locator(rootSelector) });
  await expect(pane).toBeVisible();
  return pane;
}

type Box = NonNullable<Awaited<ReturnType<Locator["boundingBox"]>>>;

async function box(locator: Locator): Promise<Box> {
  const value = await locator.boundingBox();
  expect(value, "element has a bounding box").not.toBeNull();
  return value!;
}

test.describe("density contracts — Notebook + Claims", { tag: "@clickable" }, () => {
  test("company Notebook — S width tier toggles list ↔ detail", async ({ page }) => {
    await openApp(page);
    const pane = await openCompanyDashboardPanel(page, "Notebook", ".notebook-workspace");
    const workspace = pane.locator(".notebook-workspace");

    await setPaneSize(page, { width: 380, height: 700, pane });
    // No selection: the list is primary (list + empty detail stacked, matching
    // the density-matrix reference at S).
    await expect(workspace).not.toHaveAttribute("data-detail-open");
    await expect(pane.locator(".notebook-list")).toBeVisible();
    await expectNoPageOverflow(page);

    // Select a note → single column collapses to the detail + a back control.
    await pane.locator(".notebook-list .notebook-row").first().click();
    await expect(workspace).toHaveAttribute("data-detail-open", "");
    await expect(pane.locator(".notebook-list")).toBeHidden();
    await expect(pane.locator(".notebook-back-to-list")).toBeVisible();
    await expectNoPageOverflow(page);

    // Back to list restores the list and drops the detail-open state.
    await pane.locator(".notebook-back-to-list").click();
    await expect(workspace).not.toHaveAttribute("data-detail-open");
    await expect(pane.locator(".notebook-list")).toBeVisible();
  });

  test("company Notebook — short height tier folds to list, editor on select", async ({ page }) => {
    await openApp(page);
    const pane = await openCompanyDashboardPanel(page, "Notebook", ".notebook-workspace");

    await setPaneSize(page, { width: 900, height: 440, pane });
    // Short: list only — the empty detail folds so the list owns the height.
    await expect(pane.locator(".notebook-list")).toBeVisible();
    await expect(pane.locator(".notebook-detail")).toBeHidden();
    await expectNoPageOverflow(page);

    // Selecting a note swaps in the editor and folds the list.
    await pane.locator(".notebook-list .notebook-row").first().click();
    await expect(pane.locator(".notebook-detail")).toBeVisible();
    await expect(pane.locator(".notebook-list")).toBeHidden();
    await expect(pane.locator(".notebook-back-to-list")).toBeVisible();
    await expectNoPageOverflow(page);
  });

  test("global Notebooks screen — S / M / L tier compliance", async ({ page }) => {
    await openApp(page);
    await openCockpitPanel(page, "Notebook");
    const pane = page.locator(".cockpit-pane", { has: page.locator(".notebooks-screen") });
    await expect(pane).toBeVisible();
    const screenEl = pane.locator(".notebooks-screen");
    // Force the clean L layout first, then land on a company with notes so the
    // note list is populated for the S toggle.
    await setPaneSize(page, { width: 900, height: 700, pane });
    await pane.getByRole("button", { name: "Open notebook company: GPW:CDR" }).click();
    await expect(pane.locator(".notebooks-notes-list .notebook-row").first()).toBeVisible();

    // L: multi-column — company nav ∥ notes ∥ detail; resizers present.
    {
      const navBox = await box(pane.locator(".notebooks-company-nav"));
      const detailBox = await box(pane.locator(".notebooks-detail-pane"));
      expect(detailBox.x, "detail to the right of the company nav").toBeGreaterThan(navBox.x + navBox.width);
      expect(Math.abs(detailBox.y - navBox.y), "columns on one row").toBeLessThan(40);
      await expect(pane.locator(".notebooks-resizer").first()).toBeVisible();
      await expectNoPageOverflow(page);
    }

    // M: stacked single column — same left edge, detail below; resizers hidden.
    await setPaneSize(page, { width: 600, height: 700, pane });
    {
      const navBox = await box(pane.locator(".notebooks-company-nav"));
      const detailBox = await box(pane.locator(".notebooks-detail-pane"));
      expect(Math.abs(detailBox.x - navBox.x), "detail shares the nav's column").toBeLessThan(6);
      expect(detailBox.y, "detail stacked below the nav").toBeGreaterThan(navBox.y + 10);
      await expect(pane.locator(".notebooks-resizer").first()).toBeHidden();
      await expectNoPageOverflow(page);
    }

    // S: single column, list OR detail toggled.
    await setPaneSize(page, { width: 380, height: 700, pane });
    await expect(screenEl).not.toHaveAttribute("data-detail-open");
    await expect(pane.locator(".notebooks-company-nav")).toBeVisible();
    await expect(pane.locator(".notebooks-detail-pane")).toBeHidden();
    await expectNoPageOverflow(page);

    await pane.locator(".notebooks-notes-list .notebook-row").first().click();
    await expect(screenEl).toHaveAttribute("data-detail-open", "");
    await expect(pane.locator(".notebooks-company-nav")).toBeHidden();
    await expect(pane.locator(".notebooks-detail-pane")).toBeVisible();
    await expect(pane.locator(".notebooks-back-to-list")).toBeVisible();
    await expectNoPageOverflow(page);

    await pane.locator(".notebooks-back-to-list").click();
    await expect(screenEl).not.toHaveAttribute("data-detail-open");
    await expect(pane.locator(".notebooks-company-nav")).toBeVisible();
  });

  test("company Claims — S / M / L / short tiers", async ({ page }) => {
    await openApp(page);
    const pane = await openCompanyDashboardPanel(page, "Claims", ".company-claims-panel");

    // L: list ∥ verdict detail column; composer inline.
    await setPaneSize(page, { width: 900, height: 700, pane });
    {
      const mainBox = await box(pane.locator(".claims-main"));
      const colBox = await box(pane.locator(".claims-verdict-column"));
      expect(colBox.x, "verdict column to the right of the list").toBeGreaterThan(mainBox.x + mainBox.width / 2);
      expect(Math.abs(colBox.y - mainBox.y), "columns on one row").toBeLessThan(40);
      await expect(pane.locator(".claim-create-form")).toBeVisible();
      await expect(pane.locator(".claims-add-toggle")).toBeHidden();
      await expectNoPageOverflow(page);
    }

    // M: single column, inline composer; verdict work stacks above the list.
    await setPaneSize(page, { width: 600, height: 700, pane });
    {
      const mainBox = await box(pane.locator(".claims-main"));
      const colBox = await box(pane.locator(".claims-verdict-column"));
      expect(Math.abs(colBox.x - mainBox.x), "verdict column shares the list's column").toBeLessThan(6);
      expect(colBox.y, "verdict work above the list").toBeLessThan(mainBox.y);
      await expect(pane.locator(".claim-create-form")).toBeVisible();
      await expect(pane.locator(".claims-add-toggle")).toBeHidden();
      await expectNoPageOverflow(page);
    }

    // short: queue counts + top 3 due; the full panel folds behind expansion.
    await setPaneSize(page, { width: 900, height: 440, pane });
    await expect(pane.locator(".claims-queue-summary")).toBeVisible();
    await expect(pane.locator(".claims-body")).toBeHidden();
    await expect(pane.locator(".claims-queue-top-item").first()).toBeVisible();
    await expectNoPageOverflow(page);
    await pane.locator(".claims-short-toggle").click();
    await expect(pane.locator(".claims-body")).toBeVisible();
    await expectNoPageOverflow(page);

    // S: list only; the composer hides behind the "Add claim" disclosure.
    await setPaneSize(page, { width: 380, height: 700, pane });
    await expect(pane.locator(".claims-add-toggle")).toBeVisible();
    await expect(pane.locator(".claim-create-form")).toBeHidden();
    await expect(pane.locator(".claims-list")).toBeVisible();
    await expectNoPageOverflow(page);
    await pane.locator(".claims-add-toggle").click();
    await expect(pane.locator(".claim-create-form")).toBeVisible();
    await expectNoPageOverflow(page);
  });
});
