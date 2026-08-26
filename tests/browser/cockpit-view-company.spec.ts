import { test, expect, openApp, expectNoPageOverflow } from "./helpers/harness";
import { primeCockpitLayout } from "./helpers/mockRuntime";
import type { Page } from "@playwright/test";

// U-Ra (ADR 0076): a cockpit view carries one "view company". Company-scoped
// panels follow it by default and retarget IN PLACE when it changes (tab titles
// stay kind-only, the layout is preserved); a single panel may pin a frozen
// company via the tab's pin toggle.
//
// F3a S3 (ADR 0107 decision 5): the cockpit's "Add panel"/"Save dashboard"/
// "+ New view" surface is frozen/removed, so a browser test can no longer build
// a NAMED view with follow panels through the UI (only a per-company "Legacy
// dashboard · TICKER" row exists out of the box, and it carries no "View
// company" selector — it's fixed to the company that named it). Every test here
// seeds a named view directly (`primeCockpitLayout`, the E2E equivalent of
// `saveCockpitLayout` in paneLandmarks.test.tsx) and opens it from the sidebar
// "Views" group — the one surviving entry point for a saved view (ADR 0107
// decision 5, "Widoki" group). The pre-freeze "persist the view across a
// reload" behavior is gone with `saveDashboard` (CockpitScreen.tsx) — a view
// company change is in-session only now, so that leg is dropped rather than
// re-pointed.

const VIEW_NAME = "CDR follow view";

function nav(page: Page) {
  return page.getByLabel(/Primary navigation|Nawigacja główna/);
}

// Follow Feed + Report comparison panels, plus the research global panel, all
// scoped to CD PROJEKT — the fixed set every test in this file opens against.
async function seedFollowView(page: Page) {
  await primeCockpitLayout(page, {
    id: "cockpit_layout_test_cdr_follow_view",
    name: VIEW_NAME,
    ordinal: 0,
    panelsJson: JSON.stringify({
      pinned: [
        { kind: "companyFeed", mode: "follow" },
        { kind: "reportDiff", mode: "follow" },
      ],
      openGlobals: ["research"],
      closedLinked: ["feed", "inspector", "claims-sel", "diff-sel"],
      selectedFeedItemId: null,
      viewCompanyId: "company_gpw_cdr",
    }),
    layoutJson: null,
    dockviewVersion: null,
    createdAt: "2026-06-05T09:00:00Z",
    updatedAt: "2026-06-05T09:00:00Z",
  });
}

async function openFollowView(page: Page) {
  await openApp(page);
  await nav(page).getByRole("button", { name: VIEW_NAME, exact: true }).click();
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
  test("the header selector retargets follow panels in place", async ({ page }) => {
    await seedFollowView(page);
    await openFollowView(page);
    const cockpit = page.getByLabel("Research cockpit");

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
  });

  test("a follow panel's tab pins it to the current view company", async ({ page }) => {
    await seedFollowView(page);
    await openFollowView(page);
    const cockpit = page.getByLabel("Research cockpit");

    // The seeded view already carries a follow Report comparison panel (opening
    // one from the palette is no longer possible — "Add panel" is retired).
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

  test("a saved view's research global panel renders and follows the view company", async ({ page }) => {
    // Dashboard redesign (epic c793ca1): the retired standalone Research screen
    // was reachable as the cockpit's "Evidence / Research" preset — the whole
    // preset surface (a structure-mutating command) is gone with the freeze
    // (F3a S3), so this now exercises a research global panel already open in
    // the seeded view. It still follows the view company like every other
    // follow panel (jsdom cannot mount dockview panel bodies after a rebuild,
    // so this real render is proven here, in a real browser).
    await seedFollowView(page);
    await openFollowView(page);
    const cockpit = page.getByLabel("Research cockpit");

    // The real research evidence panel renders (jsdom cannot mount it after a
    // rebuild; here it does). Assert the panel, not the `.research-timeline`
    // sub-element, which density-collapses on narrow viewports (ADR 0076 D6).
    await expect(cockpit.locator(".research-panel")).toBeVisible();
    // The panel follows the view company: the cockpit is still scoped to CD PROJEKT.
    await expect(cockpit).toHaveAttribute("data-company-id", "company_gpw_cdr");
    await expectNoPageOverflow(page);

    // Switch the view company — the research panel follows: still the one panel
    // (retargets in place), and the cockpit is now scoped to KGHM.
    await cockpit.getByLabel("View company").selectOption("company_gpw_kgh");
    await expect(cockpit.locator(".research-panel")).toBeVisible();
    await expect(cockpit).toHaveAttribute("data-company-id", "company_gpw_kgh");
    await expectNoPageOverflow(page);
  });
});
