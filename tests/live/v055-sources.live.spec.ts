import { test, expect, type Page } from "@playwright/test";
import { connectToLiveApp, type LiveConnection } from "./helpers/liveConnect";
import { clearCompaniesFilter } from "./helpers/companiesList";

// v0.55 live validation (ADR 0069): drives the owner's REAL packaged app — real
// backend, real local SQLite DB — through the milestone's new source paths:
//   * the KNF short-selling adapter appears in Sources and refreshes cleanly,
//   * the GPW ESPI/EBI witness carries its "Świadek" chip and reconciles
//     (never ingesting into the feed),
//   * the Diagnostics reconciliation ledger renders.
//
// Mutation policy: the "Odśwież źródła" click performs the app's normal manual
// sweep (the same write path the daily scheduler runs) — new feed items /
// signals / reconciliation rows it produces are real, wanted data. Everything
// else is read-only. Locale: assertions accept pl + en.

const SHOTS = "test-results/live/v055";

let connection: LiveConnection;
let page: Page;

test.describe.configure({ mode: "serial" });

test.beforeAll(async () => {
  connection = await connectToLiveApp();
  page = connection.page;
  await dismissModals();
});

async function dismissModals(): Promise<void> {
  for (let i = 0; i < 3; i += 1) {
    if ((await page.locator(".ui-modal-overlay").count()) === 0) return;
    await page.keyboard.press("Escape");
    await page.waitForTimeout(300);
  }
}

test.afterAll(async () => {
  await connection.browser.close();
});

async function openScreen(labelPattern: RegExp): Promise<void> {
  await dismissModals();
  const nav = page.getByLabel(/Primary navigation|Nawigacja główna/);
  await nav.getByRole("button", { name: labelPattern }).click();
  await page.waitForTimeout(1_000);
}

test("Sources lists the KNF register adapter and the ESPI witness chip", async () => {
  await openScreen(/Źródła|Sources/);
  // Issue #129: live-spec locators must be region-scoped — real-app toasts
  // float at page level and can carry the same phrase (e.g. a per-company skip
  // toast naming "GPW ESPI/EBI"), which strict-mode-collides with a bare
  // page-level getByText.
  const sourceList = page.getByLabel(/Source list|Lista źródeł/);
  await expect(sourceList.getByText("KNF Short Selling Register")).toBeVisible();
  await expect(sourceList.getByText("GPW ESPI/EBI")).toBeVisible();
  await expect(sourceList.getByText(/Świadek|Witness/).first()).toBeVisible();
  await page.screenshot({ path: `${SHOTS}/01-sources.png`, fullPage: true });
});

test("manual sweep refreshes KNF + witness without an error state", async () => {
  // Issue #164 calibration: full-sweep contention (see sources-refresh spec).
  test.setTimeout(480_000);
  await openScreen(/Źródła|Sources/);
  // Two controls carry this label (shell icon-button + the Sources panel
  // button) — drive the Sources panel one.
  await page
    .getByRole("button", { name: /Odśwież źródła|Refresh sources/ })
    .last()
    .click();
  // The sweep fetches several real sources; wait for the button to leave the
  // in-flight state ("Odświeżanie") and settle, then assert no failure banner.
  // Budget matches the raised test timeout minus the assertion tail (#164).
  await expect(
    page.getByRole("button", { name: /Odświeżanie|Refreshing/ }),
  ).toHaveCount(0, { timeout: 420_000 });
  await expect(
    page.getByText(/Source refresh failed|Odświeżanie źródła nie powiodło się/),
  ).toHaveCount(0);
  await page.screenshot({ path: `${SHOTS}/02-sources-after-sweep.png`, fullPage: true });
});

test("Diagnostics shows the reconciliation ledger section", async () => {
  await openScreen(/Diagnostyka|Diagnostics/);
  const ledger = page
    .getByText(/Rekoncyliacja|Reconciliation|Świadek|witness/i)
    .first();
  await ledger.scrollIntoViewIfNeeded();
  await expect(ledger).toBeVisible();
  await page.waitForTimeout(500);
  await page.screenshot({ path: `${SHOTS}/03-diagnostics.png`, fullPage: true });
});

test("shortPositions cockpit panel opens from the palette on a real company", async () => {
  test.setTimeout(120_000);
  await openScreen(/Spółki|Companies/);
  await clearCompaniesFilter(page);
  // Open the first tracked company's cockpit dashboard.
  await page.locator(".company-row, [data-company-row], tbody tr").first().click();
  await page.waitForTimeout(2_000);
  // Idempotent: a persisted layout may already carry the panel from a previous
  // run — only go through the palette when its tab is absent.
  const panelTab = page
    .locator(".dv-tab, .cockpit-tab-activate", { hasText: /Krótka sprzedaż|Short selling/ })
    .first();
  if ((await panelTab.count()) === 0) {
    await page.keyboard.press("Control+k");
    // The palette dialog has its own filter input — do not touch the global search box.
    const paletteFilter = page.getByPlaceholder(/filtrować polecenia|filter commands/i);
    await expect(paletteFilter).toBeVisible();
    await paletteFilter.fill("Krótka sprzedaż");
    await page.waitForTimeout(500);
    // Click the command INSIDE the palette overlay, never a matching element behind it.
    await page
      .locator(".ui-modal-overlay")
      .getByText(/Krótka sprzedaż \(KNF\)|Short selling \(KNF\)/)
      .first()
      .click();
    await page.waitForTimeout(1_500);
  }
  // The panel must actually be open: its tab is the evidence.
  await expect(
    page
      .locator(".dv-tab, .cockpit-tab-activate", { hasText: /Krótka sprzedaż|Short selling/ })
      .first(),
  ).toBeVisible();
  await page.screenshot({ path: `${SHOTS}/05-short-positions-panel.png`, fullPage: true });
  // Close any modal/palette overlay the flow left open so later tests can navigate.
  await page.keyboard.press("Escape");
  await page.waitForTimeout(300);
  await page.keyboard.press("Escape");
});

test("Today stream renders after the sweep (attention events, if any)", async () => {
  await openScreen(/Dziś|Today/);
  await page.waitForTimeout(1_500);
  await page.screenshot({ path: `${SHOTS}/04-today.png`, fullPage: true });
});
