import { expect, type Page, test } from "./helpers/harness";

// Bug 228762e — the developer-gated Diagnostics "Source reconciliation" table
// reuses the event-log grid whose first track is a fixed severity-chip size
// (minmax(72px, 92px)). The reconciliation StatusChip carries a variable-length
// label; the `espi_only` "Pominięte przez główne" chip (Polish) overran that
// track and painted over the ticker column. A plain scrollWidth check misses
// this — the chip overflows a `visible` inline-flex box onto the next grid cell
// without inflating any scroller — so this asserts the chip's painted bounding
// box does not intersect the ticker cell's box. Driven in Polish (`?locale=pl`),
// the locale whose longest label reproduces the overlap.
//
// Diagnostics is developer-gated. The real unlock (hidden chord → passphrase) is
// racy to drive across the viewport matrix, so the spec uses the harness-only
// `?dev=1` override (browserSmokeRuntime) to seed developer mode deterministically
// on every project, then drives the Polish (`?locale=pl`) UI where the long label
// reproduces.

function nav(page: Page) {
  return page.getByLabel(/Primary navigation|Nawigacja główna/);
}

test.describe("Diagnostics source reconciliation layout (bug 228762e)", () => {
  test("the variable-length status chip never overlaps the ticker column", async ({ page }) => {
    // Narrow ~960px window (quarter-ultrawide) — the tightest supported size.
    await page.setViewportSize({ width: 960, height: 900 });
    await page.goto("/?locale=pl&dev=1");
    await expect(nav(page)).toBeVisible();

    await expect(nav(page).getByRole("button", { name: "Diagnostyka" })).toBeVisible();
    await nav(page).getByRole("button", { name: "Diagnostyka" }).click();

    // Open the collapsed Source reconciliation section ("Uzgadnianie źródeł").
    await page.getByRole("button", { name: /Uzgadnianie źródeł/ }).first().click();
    const list = page.getByLabel("Uzgadnianie źródeł").locator(".diagnostic-event");
    await expect(list).toHaveCount(3);

    // All three seeded labels render (the long `espi_only` one included).
    for (const label of ["Dopasowano", "Pominięte przez główne", "Tylko Bankier"]) {
      await expect(page.getByText(label, { exact: true })).toBeVisible();
    }

    // Per row, two independent overlap signals (a px-capped track fails BOTH):
    //   1. the chip's own label is not clipped/overflowing its box — the grid
    //      stretches the chip to a fixed 92px track while its nowrap text paints
    //      out the right edge onto the ticker (scrollWidth > clientWidth), which
    //      a chip-element bounding-box check alone cannot see;
    //   2. the chip's painted content box does not cross into the ticker cell.
    const rows = await list.all();
    expect(rows.length).toBe(3);
    for (const row of rows) {
      const chip = row.locator(".ui-status-chip");
      const ticker = row.locator(".diagnostic-event-stage");
      const label = (await chip.textContent())?.trim() ?? "";

      const overflow = await chip.evaluate((el) => ({
        scrollWidth: el.scrollWidth,
        clientWidth: el.clientWidth,
        right: el.getBoundingClientRect().right,
      }));
      const tickerLeft = await ticker.evaluate((el) => el.getBoundingClientRect().left);

      expect(
        overflow.scrollWidth,
        `status chip "${label}" label is clipped/overflows its box (scrollWidth ${overflow.scrollWidth} > clientWidth ${overflow.clientWidth})`,
      ).toBeLessThanOrEqual(overflow.clientWidth + 1);
      expect(
        overflow.right,
        `status chip "${label}" (right ${overflow.right}) overlaps the ticker cell (left ${tickerLeft})`,
      ).toBeLessThanOrEqual(tickerLeft + 1);
    }
  });
});
