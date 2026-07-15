import { test, expect, openApp, expectNoPageOverflow } from "./helpers/harness";
import type { Page } from "@playwright/test";

// U-Ra (ADR 0076): a cockpit view carries one "view company". Company-scoped
// panels follow it by default and retarget IN PLACE when it changes (tab titles
// stay kind-only, the layout is preserved); a single panel may pin a frozen
// company via the tab's pin toggle; a saved view persists the view company.

function nav(page: Page) {
  return page.getByLabel(/Primary navigation|Nawigacja główna/);
}

async function openCdrDashboard(page: Page) {
  await nav(page).getByRole("button", { name: "Companies" }).click();
  await page.getByRole("button", { name: "Open GPW:CDR dashboard" }).click();
  await expect(page.getByLabel("Research cockpit")).toBeVisible();
}

// dockview only mounts the ACTIVE panel's body per group, so activate a tab by
// its kind-only title before reading its content.
async function activateFeedTab(page: Page) {
  const cockpit = page.getByLabel("Research cockpit");
  await cockpit.getByRole("button", { name: "Feed", exact: true }).first().click();
  return cockpit;
}

test.describe("cockpit view company", { tag: "@clickable" }, () => {
  test("the header selector retargets follow panels in place and the view company persists", async ({
    page,
  }) => {
    await openApp(page);
    await openCdrDashboard(page);
    let cockpit = page.getByLabel("Research cockpit");

    // The follow Feed panel's tab is kind-only (no ticker prefix); activating it
    // shows CD PROJEKT's feed item.
    await expect(cockpit.getByRole("button", { name: "Feed", exact: true }).first()).toBeVisible();
    await activateFeedTab(page);
    await expect(
      cockpit.getByRole("button", {
        name: "Open company feed item: CD PROJEKT S.A. source item for browser layout smoke",
      }),
    ).toBeVisible();
    await expectNoPageOverflow(page);

    // Switch the view company — the follow panel retargets IN PLACE: the tab stays
    // kind-only "Feed" (no re-add, no prefix) and the content shows KGHM's item.
    await cockpit.getByLabel("View company").selectOption("company_gpw_kgh");
    await activateFeedTab(page);
    await expect(
      cockpit.getByRole("button", {
        name: "Open company feed item: KGHM POLSKA MIEDZ S.A. source item for browser layout smoke",
      }),
    ).toBeVisible();
    await expect(cockpit.getByRole("button", { name: "Feed", exact: true }).first()).toBeVisible();
    await expect(cockpit.getByRole("button", { name: /GPW:CDR · Feed/ })).toHaveCount(0);
    await expectNoPageOverflow(page);

    // Persist the view: save the dashboard, leave, reopen — the saved view carries
    // its view company (KGHM), so the follow Feed reopens on KGHM.
    await cockpit.getByRole("button", { name: "Save dashboard" }).click();
    await nav(page).getByRole("button", { name: "Today" }).click();
    await openCdrDashboard(page);
    cockpit = await activateFeedTab(page);
    await expect(
      cockpit.getByRole("button", {
        name: "Open company feed item: KGHM POLSKA MIEDZ S.A. source item for browser layout smoke",
      }),
    ).toBeVisible();
    await expectNoPageOverflow(page);
  });

  test("opens a follow panel from the palette and pins it to the current company", async ({
    page,
  }) => {
    await openApp(page);
    await openCdrDashboard(page);
    const cockpit = page.getByLabel("Research cockpit");

    // Open a follow Report comparison panel from the ⌘K palette (only offered
    // because a view company is set). It tracks the view company (CD PROJEKT).
    await page.keyboard.press("Control+K");
    const palette = page.getByRole("dialog", { name: "Command palette" });
    await palette.getByLabel("Search commands").fill("Open panel: Report comparison");
    await palette
      .getByRole("button", { name: "Open panel: Report comparison", exact: true })
      .click();
    await expect(
      cockpit.getByRole("button", { name: "Report comparison", exact: true }).first(),
    ).toBeVisible();

    // The tab's pin toggle freezes the current view company onto the panel: its
    // title gains the ticker prefix (GPW:CDR · Report comparison).
    const tab = cockpit.locator(".cockpit-tab").filter({ hasText: "Report comparison" });
    await tab.getByRole("button", { name: "Pin company" }).click();
    await expect(
      cockpit.getByRole("button", { name: "GPW:CDR · Report comparison", exact: true }),
    ).toBeVisible();
    await expectNoPageOverflow(page);
  });

  test("the Dowody / Research preset renders the evidence panel following the view company", async ({
    page,
  }) => {
    // Dashboard redesign (epic c793ca1): the retired standalone Research screen is
    // now the "Evidence / Research" preset. Selecting it composes the research
    // evidence panel; it follows the view company like every other preset (jsdom
    // cannot mount dockview panel bodies after a rebuild, so this real render is
    // proven here, in a real browser).
    await openApp(page);
    await openCdrDashboard(page);
    const cockpit = page.getByLabel("Research cockpit");

    await cockpit.getByLabel("Preset").selectOption("evidence");

    // The real research evidence panel renders (jsdom cannot mount it after a
    // rebuild; here it does). Assert the panel, not the `.research-timeline`
    // sub-element, which density-collapses on narrow viewports (ADR 0076 D6).
    await expect(cockpit.locator(".research-panel")).toBeVisible();
    // The preset follows the view company: the cockpit is still scoped to CD PROJEKT.
    await expect(cockpit).toHaveAttribute("data-company-id", "company_gpw_cdr");
    await expectNoPageOverflow(page);

    // Switch the view company — the evidence panel follows: still the one research
    // panel (retargets in place), and the cockpit is now scoped to KGHM.
    await cockpit.getByLabel("View company").selectOption("company_gpw_kgh");
    await expect(cockpit.locator(".research-panel")).toBeVisible();
    await expect(cockpit).toHaveAttribute("data-company-id", "company_gpw_kgh");
    await expectNoPageOverflow(page);
  });
});
