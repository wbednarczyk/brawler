import {
  test,
  expect,
  openApp,
  expectNoPageOverflow,
  expectInternalScroll,
} from "./helpers/harness";

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
  // F3a S1 (ADR 0107): the row opens Spółka; the legacy cockpit is reached via the sidebar Dashboard entry until S3 freezes it.
  await page.getByLabel("Primary navigation").getByRole("button", { name: "Dashboard" }).click();

  // Activate the Quality dock tab so its panel body is on screen (dockview tabs
  // are accessible buttons, ADR 0047).
  await page.getByRole("button", { name: /Quality/ }).first().click();

  // Reveal the qualitative-criterion form (full-width guidance textarea — the new
  // overflow risk alongside the existing quantitative expression row).
  await page.getByRole("button", { name: "Qualitative", exact: true }).click();
  await expect(page.getByLabel("Assessment guidance")).toBeVisible();

  await expectNoPageOverflow(page);
});

// The Quality panel gains a "Company health" section (ADR 0083, v0.57 T2):
// Piotroski F + Altman Z″ tiles with expandable per-component breakdowns and an
// explicit insufficient-data state. The mock runtime seeds a populated report
// for company_gpw_cdr (F headline 8/9; Z″ insufficient with a missing input).
test("company health scores render, expand, and do not overflow at a narrow window", async ({
  page,
}) => {
  await page.setViewportSize({ width: 1008, height: 900 });
  await openApp(page);
  await page.getByLabel(/Primary navigation|Nawigacja główna/).getByRole("button", { name: "Companies" }).click();
  await page.locator('[data-company-id="company_gpw_cdr"] .company-row-main').click();
  // F3a S1 (ADR 0107): the row opens Spółka; the legacy cockpit is reached via the sidebar Dashboard entry until S3 freezes it.
  await page.getByLabel("Primary navigation").getByRole("button", { name: "Dashboard" }).click();

  await page.getByRole("button", { name: /Quality/ }).first().click();

  const healthSection = page.locator(".company-health-section");
  await expect(healthSection.getByRole("heading", { name: "Company health" })).toBeVisible();
  // Piotroski headline tile + Altman insufficient-data tile both render.
  await expect(healthSection.getByText("8/9")).toBeVisible();
  await expect(healthSection.getByText("Insufficient data")).toBeVisible();

  // Expanding the Piotroski tile reveals the per-signal breakdown with inputs.
  await healthSection.getByRole("button", { name: /Piotroski F \(2000\)/ }).click();
  await expect(page.getByText(/Return on assets is positive/)).toBeVisible();
  await expect(page.getByText(/net_profit@FY2025 = 100/)).toBeVisible();

  // Expanding the Altman tile shows its partial breakdown + the missing input,
  // named with its localized KPI name (never the raw `retained_earnings` key —
  // bug fixed 2026-07-18, owner screenshot).
  await healthSection.getByRole("button", { name: /Altman Z/ }).click();
  await expect(page.getByText(/Missing inputs: Retained earnings \(FY2025\)/)).toBeVisible();

  // The panel scrolls internally; the page itself never gains a horizontal
  // scrollbar at the quarter-ultrawide width (DoD §B).
  await expectInternalScroll(page.locator(".cockpit-pane").first());
  await expectNoPageOverflow(page);
});
