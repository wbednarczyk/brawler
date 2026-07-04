import { test, expect, openApp, expectNoPageOverflow } from "./helpers/harness";

// The Quality panel (ADR 0075) gains a qualitative-criterion form — a kind switch
// plus a full-width guidance textarea — and agent-assessed result rows. Guard that
// revealing that form does not force a horizontal scrollbar at a narrow window
// (the quarter-ultrawide range, DoD §B). The Quality panel is part of the default
// company dashboard (ADR 0057); the dual-execution mock runtime seeds a framework.
test("quality panel qualitative form does not horizontally overflow at a narrow window", async ({
  page,
}) => {
  await page.setViewportSize({ width: 1008, height: 900 });
  await openApp(page);
  await page.getByLabel(/Primary navigation|Nawigacja główna/).getByRole("button", { name: "Companies" }).click();
  await page.locator('[data-company-id="company_gpw_cdr"] .company-row-main').click();

  // Activate the Quality dock tab so its panel body is on screen (dockview tabs
  // are accessible buttons, ADR 0047).
  await page.getByRole("button", { name: /Quality/ }).first().click();

  // Reveal the qualitative-criterion form (full-width guidance textarea — the new
  // overflow risk alongside the existing quantitative expression row).
  await page.getByRole("button", { name: "Qualitative", exact: true }).click();
  await expect(page.getByLabel("Assessment guidance")).toBeVisible();

  await expectNoPageOverflow(page);
});
