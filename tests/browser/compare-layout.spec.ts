import { expect, type Page, test } from "@playwright/test";

import { expectNoPageOverflow, openApp } from "./helpers/harness";
import { expectPrimaryActionCount } from "./helpers/interactionContracts";

// Compare mode (ADR 0089 §A3 + §A7, storyboard frames 1–7 + "Widok Profil").
// Drives the restored sidebar entry, the zestaw-spółek selection, and the
// reactive Profil view (the entry default), then asserts the narrow-window
// containment contract. The browser harness seeds a populated Profil comparison
// for GPW:CDR (PLN) + GPW:CBF (EUR).
function nav(page: Page) {
  return page.getByLabel("Primary navigation");
}

async function gotoCompare(page: Page) {
  await openApp(page);
  await nav(page).getByRole("button", { name: "Compare" }).click();
  await expect(page.getByRole("heading", { name: "Compare", exact: true })).toBeVisible();
}

async function selectSeededPair(page: Page) {
  await page.getByLabel(/Add company/).selectOption({ value: "company_gpw_cdr" });
  await page.getByLabel(/Add company/).selectOption({ value: "company_gpw_cbf" });
}

test.describe("Compare mode layout (ADR 0089 §A3 + §A7)", () => {
  test("Profil is the default view and computes reactively — no submit button", async ({ page }) => {
    await gotoCompare(page);

    // Frame 1: the companies selector + the quiet invite, no CTA-spam.
    await expect(page.locator("section.compare-companies")).toBeVisible();
    await expect(
      page.getByText(/Pick at least 2 companies with confirmed data/),
    ).toBeVisible();

    // §A7: selecting the pair computes reactively — no click, no submit button.
    await selectSeededPair(page);
    const table = page.getByRole("table");
    await expect(table).toBeVisible();

    // The Profil pivot: KPI rows down, companies across, a Różnica column for the
    // pair, evidence links + the EUR→PLN chip on the EUR company.
    await expect(table.getByRole("columnheader", { name: "KPI" })).toBeVisible();
    await expect(table.getByRole("columnheader", { name: "Difference" })).toBeVisible();
    await expect(table.getByRole("columnheader", { name: /GPW:CDR/ })).toBeVisible();
    await expect(table.getByRole("columnheader", { name: /GPW:CBF/ })).toBeVisible();
    await expect(table.getByText("Revenue")).toBeVisible();
    await expect(table.getByText("9.0×")).toBeVisible();
    await expect(table.getByText("EUR→PLN")).toBeVisible();
    await expect(table.getByRole("button", { name: /Open evidence for GPW:CDR/ }).first()).toBeVisible();

    // Experience contract §6 (amended): the entry primary is "+ Add company"; the
    // result surface is informational — no submit control on the screen.
    await expectPrimaryActionCount(page.locator(".compare-screen"), { max: 1 });
    await expect(
      page.locator(".compare-screen").getByRole("button", { name: /^(Compare|Porównaj)$/ }),
    ).toHaveCount(0);
  });

  test("renders the comparative valuation section (§B3): chips, football field, thin state", async ({
    page,
  }) => {
    await gotoCompare(page);
    await selectSeededPair(page);
    await expect(page.getByRole("table")).toBeVisible();

    // The valuation section is scoped to one company (default = first = CDR),
    // which the harness seeds populated: percentile chips (always with N) + the
    // football field of implied ranges by method.
    const valuation = page.getByRole("region", { name: "Valuation" });
    await expect(valuation).toBeVisible();
    await expect(valuation.getByText("Comparative valuation L1")).toBeVisible();
    await expect(valuation.getByText(/P\/E: p37 among peers \(N=5\)/)).toBeVisible();
    await expect(valuation.getByRole("group", { name: /football field/i })).toBeVisible();
    await expect(valuation.getByText("P/E × median")).toBeVisible();
    // The absent method names its typed reason, never a silent omission.
    await expect(valuation.getByText("too few peers")).toBeVisible();

    // Switching the scope to the thin peer set (CBF, N=2) surfaces the explicit
    // threshold message — the section never disappears silently.
    await page.getByLabel("Valuation company").selectOption({ value: "company_gpw_cbf" });
    await expect(
      valuation.getByText(/N=2 — too few peers for percentiles and median multiples \(threshold: 4\)/),
    ).toBeVisible();
  });

  test("Trend toggle restores the one-KPI × periods view", async ({ page }) => {
    await gotoCompare(page);
    await selectSeededPair(page);
    await expect(page.getByRole("table")).toBeVisible();

    await page.getByRole("button", { name: "Trend", exact: true }).click();

    // The Trend table aligns the single metric across the period axis; the
    // multi-series overlay is back.
    const table = page.getByRole("table");
    for (const year of ["2022", "2023", "2024"]) {
      await expect(table.getByRole("columnheader", { name: year, exact: true })).toBeVisible();
    }
    await expect(page.locator(".compare-chart").getByRole("img", { name: /Trend/ })).toBeVisible();
    await expect(table.getByRole("columnheader", { name: "KPI" })).toHaveCount(0);
  });

  test("narrow ~960px window stacks sections and scrolls the Profil table in its own container", async ({
    page,
  }) => {
    await page.setViewportSize({ width: 960, height: 900 });
    await gotoCompare(page);
    await selectSeededPair(page);
    await expect(page.getByRole("table")).toBeVisible();

    // The valuation section stacks in the vertical flow at narrow width (frame 6/7),
    // its football field width-bound like the trend overlay — never a new scroller.
    await expect(page.getByRole("region", { name: "Valuation" })).toBeVisible();

    // No global horizontal scroll, and no unmarked panel-internal scroller
    // (the table's scroller carries data-hscroll).
    await expectNoPageOverflow(page);

    // The panel + its padded body never overflow — the table's overflow is
    // contained by the inner scroller, not pushed onto the pane (storyboard
    // frame 7: "scrollWidth ≤ clientWidth+1 na kontenerze, nie dokumencie").
    for (const selector of [".compare-screen", ".compare-layout"]) {
      const box = await page.locator(selector).evaluate((el) => ({
        scrollWidth: el.scrollWidth,
        clientWidth: el.clientWidth,
      }));
      expect(
        box.scrollWidth,
        `${selector} overflows horizontally (${box.scrollWidth} > ${box.clientWidth})`,
      ).toBeLessThanOrEqual(box.clientWidth + 1);
    }

    // The inner scroll container is the sanctioned wide-content scroller.
    const scroller = page.locator(".compare-table-scroll");
    await expect(scroller).toHaveAttribute("data-hscroll");
    const overflowX = await scroller.evaluate((el) => getComputedStyle(el).overflowX);
    expect(["auto", "scroll"]).toContain(overflowX);
  });
});
