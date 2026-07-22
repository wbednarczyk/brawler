import { test, expect, openApp, expectNoPageOverflow } from "./helpers/harness";

// The Coverage panel's "Flagged periods" section (ADR 0061 decision 2, ADR 0084
// decision 4/6) — the UI half of the "never silently wrong" guarantee: the
// periods the deterministic pipeline ran on and REFUSED to record, each with its
// typed reason translated into plain language, and a per-period re-run.
//
// Guarded here (browser, not jsdom): the section really renders inside the
// cockpit Coverage pane, the SENTINEL period of a `no_period_derived` failure
// reads as "Period unknown" rather than a fabricated fiscal year, the reassuring
// empty state shows for a company with nothing flagged, and neither state forces
// a horizontal scrollbar at the narrow quarter-ultrawide width (DoD §B). The
// dual-execution mock runtime serves the seeded outcomes (CD PROJEKT populated,
// ORLEN empty).

async function openCoveragePanel(page: import("@playwright/test").Page, companyId: string) {
  await page.setViewportSize({ width: 1008, height: 900 });
  await openApp(page);
  await page
    .getByLabel(/Primary navigation|Nawigacja główna/)
    .getByRole("button", { name: "Companies" })
    .click();
  await page.locator(`[data-company-id="${companyId}"] .company-row-main`).click();
  // Coverage ships in the DEFAULT cockpit set, as a background dockview tab —
  // its content is not in the DOM until activated. Activate it as a user would.
  await page.getByRole("button", { name: "Coverage", exact: true }).first().click();
}

test("flagged periods render with translated reasons and do not overflow — populated", async ({
  page,
}) => {
  await openCoveragePanel(page, "company_gpw_cdr");

  const section = page.locator(`.company-coverage[data-company-id="company_gpw_cdr"] .coverage-flagged`);
  await expect(section).toBeVisible();

  // Typed reason codes arrive translated — a raw code must never reach the user.
  await expect(section.getByText("The figures failed a consistency check")).toBeVisible();
  await expect(section.getByText("The report's layout changed")).toBeVisible();
  await expect(section.getByText("validation_failed")).toHaveCount(0);
  await expect(section.getByText("structure_drift")).toHaveCount(0);

  // The attempting tier is named. A legacy stored `pdf`-token row renders the
  // positional structural label — the "PDF profile" chip died with the PDF fact
  // arm (ADR 0086 dec. 1).
  await expect(section.getByText("Structured read (xHTML)")).toBeVisible();
  // `exact` — the drift CHIP (legacy stored rows still carry structureChanged),
  // not the reason line that also contains the phrase.
  await expect(section.getByText("Layout changed", { exact: true })).toBeVisible();

  // The SENTINEL period (fiscalYear 0 / empty periodType) of a period-less
  // failure reads as unknown, never as a real — invented — period.
  await expect(section.getByText("Period unknown")).toBeVisible();
  await expect(section.getByText("No reader")).toBeVisible();

  await expectNoPageOverflow(page);
});

test("the failing check expands in place under its period", async ({ page }) => {
  await openCoveragePanel(page, "company_gpw_cdr");

  const section = page.locator(`.company-coverage[data-company-id="company_gpw_cdr"] .coverage-flagged`);
  await section.getByRole("button", { name: /2025 H1/ }).click();

  // The gate's expected/actual/residual evidence is readable, not summarized away.
  await expect(section.getByText("aktywa = zobowiązania + kapitał własny")).toBeVisible();
  await expect(section.getByText("11 200")).toBeVisible();

  await expectNoPageOverflow(page);
});

test("re-running a flagged period clears it from the list", async ({ page }) => {
  await openCoveragePanel(page, "company_gpw_cdr");

  const section = page.locator(`.company-coverage[data-company-id="company_gpw_cdr"] .coverage-flagged`);
  const rowsBefore = await section.locator(".coverage-flagged-list > li").count();
  expect(rowsBefore).toBeGreaterThan(1);

  await section.getByRole("button", { name: "Try again" }).first().click();

  // The backend updates the row in place; a period that now emits leaves the
  // list, so the section must refetch rather than keep a stale flag.
  await expect
    .poll(async () => section.locator(".coverage-flagged-list > li").count())
    .toBe(rowsBefore - 1);
});

test("a company with nothing flagged shows the reassuring empty state — not an error", async ({
  page,
}) => {
  await openCoveragePanel(page, "company_gpw_pkn");

  const section = page.locator(`.company-coverage[data-company-id="company_gpw_pkn"] .coverage-flagged`);
  await expect(section).toBeVisible();
  await expect(
    section.getByText("Nothing flagged — every attempted period produced data."),
  ).toBeVisible();
  // "Nothing flagged" is a GOOD state — no error styling, no retry affordance.
  await expect(section.locator(".coverage-flagged-failed")).toHaveCount(0);

  await expectNoPageOverflow(page);
});
