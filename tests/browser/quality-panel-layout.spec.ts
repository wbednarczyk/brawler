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
// (the quarter-ultrawide range, DoD §B). F3a S3 (ADR 0107): the Quality panel is
// the `jakosc` workshop tool, opened via the ⌘K palette's "Open quality" entry;
// the dual-execution mock runtime seeds a framework.
async function openQualityTool(page: import("@playwright/test").Page) {
  await page.setViewportSize({ width: 1008, height: 900 });
  await openApp(page);
  await page.getByLabel(/Primary navigation|Nawigacja główna/).getByRole("button", { name: "Companies" }).click();
  await page.locator('[data-company-id="company_gpw_cdr"] .company-row-main').click();
  await page.getByRole("region", { name: "Company view" }).waitFor();
  await page.keyboard.press("Control+K");
  const palette = page.getByRole("dialog", { name: "Command palette" });
  await palette.getByLabel("Search commands").fill("Open quality");
  await palette.getByRole("button", { name: "Open quality", exact: true }).first().click();
}

test("quality panel qualitative form does not horizontally overflow at a narrow window", async ({
  page,
}) => {
  await openQualityTool(page);

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
  await openQualityTool(page);

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

  // `.spolka-layout` (not the tool group itself) is the scroll region; the
  // page itself never gains a horizontal scrollbar at the quarter-ultrawide
  // width (DoD §B).
  await expectInternalScroll(page.locator(".spolka-body-scroll"));
  await expectNoPageOverflow(page);
});

// Mechanical defect #3 (F3a study): the evaluation-history timestamp used to
// render the raw ISO string (`2026-06-08T10:00:00Z`) and wrap mid-string at
// narrow widths. Formatted via formatDetailTimestamp + `white-space: nowrap`
// (QualityPanel/company-workspace.css) — the element must never overflow its
// own box, at the narrow quarter-ultrawide window (DoD §B).
test("evaluation history timestamp never overflows and never shows raw ISO", async ({ page }) => {
  await openQualityTool(page);

  const when = page.locator(".quality-history-when").first();
  await expect(when).toBeVisible();
  await expect(when).not.toHaveText(/\d{4}-\d{2}-\d{2}T\d{2}:\d{2}:\d{2}Z/);

  const box = await when.evaluate((el) => ({
    scrollWidth: el.scrollWidth,
    clientWidth: el.clientWidth,
  }));
  expect(box.scrollWidth).toBeLessThanOrEqual(box.clientWidth + 1);

  await expectNoPageOverflow(page);
});
