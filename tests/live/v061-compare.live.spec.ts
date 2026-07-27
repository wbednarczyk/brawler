import { test, expect } from "@playwright/test";
import { mkdirSync } from "node:fs";
import { connectToLiveApp, type LiveConnection } from "./helpers/liveConnect";

// v0.61 §A6 desktop verification (ADR 0089). Non-destructive: connects to the
// real running Brawler app over CDP, confirms the restored Compare (Porównaj)
// nav entry, screenshots the Compare screen and the company Dashboard's
// Fundamentals "Positions × periods" section (§A5), and logs DOM presence for
// the human charter. Best-effort — every step is guarded so a stale handle
// never aborts the capture.

const SHOTS = "test-results/live/v061";
mkdirSync(SHOTS, { recursive: true });

let connection: LiveConnection;

test.beforeAll(async () => {
  connection = await connectToLiveApp();
});
test.afterAll(async () => {
  await connection.browser.close();
});

test("v0.61 Compare + Fundamentals periods — surfaces render", async () => {
  test.setTimeout(180_000);
  const page = connection.page;

  const dismiss = async () => {
    for (let i = 0; i < 4; i += 1) {
      await page.keyboard.press("Escape").catch(() => {});
      await page.waitForTimeout(150);
    }
  };
  const shot = async (name: string) => {
    try {
      await page.screenshot({ path: `${SHOTS}/${name}.png`, fullPage: true });
      console.log(`captured ${name}`);
    } catch (e) {
      console.log(`shot ${name} failed: ${String(e).slice(0, 120)}`);
    }
  };
  const present = async (sel: string, label: string) => {
    try {
      console.log(`DOM ${label}: ${await page.locator(sel).count()}`);
    } catch (e) {
      console.log(`DOM ${label}: query failed (${String(e).slice(0, 80)})`);
    }
  };
  const click = async (name: RegExp, label: string) => {
    try {
      await dismiss();
      await page.getByRole("button", { name }).first().click({ timeout: 8000, force: true });
      await page.waitForTimeout(1200);
      console.log(`clicked ${label}`);
    } catch (e) {
      console.log(`click ${label} skipped: ${String(e).slice(0, 120)}`);
    }
  };

  await dismiss();

  // App shell + version.
  const version = await page.locator(".brand-version").textContent().catch(() => null);
  console.log(`Live Brawler app version: ${version}`);

  // Compare (Porównaj) nav entry present under Dashboard, then open it.
  const compareNav = page.getByRole("button", { name: /^(Porównaj|Compare)$/ });
  console.log(`nav Compare/Porównaj present: ${await compareNav.count()}`);
  await expect(compareNav.first()).toBeVisible({ timeout: 10_000 });
  await click(/^(Porównaj|Compare)$/, "Compare nav");
  await present(".compare-screen", "compare-screen");
  await present(".compare-companies", "compare-companies selector");
  await shot("01-compare");

  // Populate the compare set from the real DB: pick the first two real options
  // in the "Add company" select. Best-effort — the owner DB always has some.
  const addSelect = page.getByLabel(/\+ (Dodaj spółkę|Add company)…/).first();
  try {
    const options = await addSelect.locator("option").all();
    // option[0] is the placeholder; take the next two real ids.
    for (const optionIndex of [1, 2]) {
      const value = await options[optionIndex]?.getAttribute("value");
      if (value) {
        await addSelect.selectOption(value);
        await page.waitForTimeout(900);
        console.log(`added compare company #${optionIndex}`);
      }
    }
  } catch (e) {
    console.log(`add-company skipped: ${String(e).slice(0, 120)}`);
  }
  await page.waitForTimeout(1500);

  // Profil is the entry default (§A7): its segmented option is active and the
  // period selector renders. Log both so the human charter can confirm.
  const profilActive = await page
    .getByRole("button", { name: /^(Profil|Profile)$/ })
    .first()
    .getAttribute("aria-pressed")
    .catch(() => null);
  console.log(`Profil segmented aria-pressed: ${profilActive}`);
  await present("select", "compare selects (incl. period)");
  const periodPresent = await page.getByLabel(/^(Okres|Period)$/).count().catch(() => 0);
  console.log(`Profil period selector present: ${periodPresent}`);
  await present(".compare-table", "profil table");
  await shot("03-profil");

  // Trend toggle works: switch to Trend, confirm the one-metric picker appears,
  // screenshot the trend chart, then switch back to Profil.
  await click(/^(Trend)$/, "Trend toggle");
  const metricPicker = await page.getByLabel(/^(Metryka|Metric)$/).count().catch(() => 0);
  console.log(`Trend metric picker present: ${metricPicker}`);
  await present(".compare-chart, [role='img']", "trend chart");
  await shot("04-trend");
  await click(/^(Profil|Profile)$/, "Profil toggle back");

  // Valuation section (§B3): appears for the selected set — populated OR its
  // honest thin state. EITHER is a pass (owner DB has thin sectors).
  await present(".compare-valuation", "valuation section");
  const valuationThin = await page.locator(".compare-valuation-thin").count().catch(() => 0);
  const valuationChips = await page.locator(".compare-valuation-pctile").count().catch(() => 0);
  console.log(`valuation: thin=${valuationThin} percentileChips=${valuationChips}`);
  await shot("05-valuation");

  // Company Dashboard → Fundamentals periods × deltas (§A5).
  await click(/^(Pulpit|Dashboard)$/, "Dashboard nav");
  await present(".fundamentals-periods", "fundamentals-periods section");
  await present("[aria-label='Positions × periods'], [aria-label='Pozycje × okresy']", "periods aria");
  await shot("06-dashboard-fundamentals");

  // Diagnostics reconciliation chips must not overlap (§ layout). Open the
  // section, log the reconciliation chip count, screenshot for the human eye.
  await click(/^(Diagnostyka|Diagnostics)$/, "Diagnostics nav");
  const recon = page.locator("[aria-labelledby='diagnostics-reconciliation-title']");
  await present("[aria-labelledby='diagnostics-reconciliation-title']", "reconciliation section");
  try {
    const header = recon.getByRole("button").first();
    if (await header.count()) {
      await header.click({ timeout: 6000, force: true }).catch(() => {});
      await page.waitForTimeout(800);
    }
  } catch (e) {
    console.log(`reconciliation expand skipped: ${String(e).slice(0, 100)}`);
  }
  await present("[aria-label='Source reconciliation'] .ui-status-chip, [aria-label='Uzgadnianie źródeł'] .ui-status-chip", "reconciliation chips");
  await shot("07-diagnostics-reconciliation");
});
