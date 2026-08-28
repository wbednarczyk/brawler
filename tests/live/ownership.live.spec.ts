import { test, expect } from "@playwright/test";
import { connectToLiveApp, type LiveConnection } from "./helpers/liveConnect";

// v0.56 live verification (ADR 0072, DoD §G): the Ownership section renders in
// the real Windows app's Basic info panel, fed by the startup catch-up
// extraction over the owner's real report documents. Tolerant of timing: right
// after an update the queue may still be draining, so the assertion accepts
// populated / empty / in-progress — but the SECTION must exist, and when
// populated it must show the donut and the derived free float.

let connection: LiveConnection;

test.beforeAll(async () => {
  connection = await connectToLiveApp();
});

test.afterAll(async () => {
  await connection.browser.close();
});

test("Ownership section renders in Basic info on the live app", async ({}) => {
  const { page } = connection;
  test.setTimeout(120_000);

  // Self-contained navigation: the suite may start on Today — open a company
  // (ADR 0107/0108: lands the Spółka screen directly, no cockpit dashboard),
  // then the Ownership workshop tool that hosts the Basic info panel.
  await page.getByRole("button", { name: /^(Spółki|Companies)$/ }).first().click();
  const row = page.locator("button[data-company-row]").first();
  await expect(row).toBeVisible({ timeout: 15_000 });
  await row.click();
  const spolka = page.getByRole("region", { name: /Widok spółki|Company view/ });
  await expect(spolka).toBeVisible({ timeout: 15_000 });

  await page
    .getByRole("group", { name: /Warsztat|Workshop/ })
    .getByRole("button", { name: /^(Akcjonariat|Ownership)$/ })
    .click();
  const tool = page.getByRole("group", { name: /Narzędzie warsztatu|Workshop tool/ });
  await expect(tool).toBeVisible({ timeout: 15_000 });

  const panel = tool.locator(".basic-info-panel");
  await expect(panel).toBeVisible({ timeout: 15_000 });

  const section = panel.locator(".ownership-section");
  await expect(section).toBeVisible({ timeout: 15_000 });
  await expect(
    section.getByRole("heading", { name: /Akcjonariat|Ownership/ }),
  ).toBeVisible();

  const holderRows = section.locator(".ownership-holder-row");
  const holders = await holderRows.count();
  if (holders > 0) {
    // Populated: the donut and the derived free-float hint must be present.
    await expect(section.locator(".ui-donut-chart, [role='img']").first()).toBeVisible();
    // The derived-free-float explanation moved to the donut's hover tooltip
    // (owner dogfooding round 2) — assert it's attached to the chart SVG.
    await expect(
      section.locator(".ui-donut-svg > title", {
        hasText: /derived: 100%|pochodn/,
      }),
    ).toHaveCount(1);
  } else {
    // Fresh/in-progress: the section must say so, never render blank.
    await expect(
      section.getByText(/No ownership disclosures|Brak ujawnień|Czytam|reading/i).first(),
    ).toBeVisible();
  }

  await page.screenshot({ path: "test-results/live/ownership-section.png", fullPage: true });
  console.log(`live ownership: holder rows visible = ${holders}`);
});
