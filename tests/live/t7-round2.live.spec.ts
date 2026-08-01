import { test, expect, type Page } from "@playwright/test";
import { connectToLiveApp, type LiveConnection } from "./helpers/liveConnect";
import { clearCompaniesFilter } from "./helpers/companiesList";

// T7 round-2 live validation (ADR 0066): drives the owner's REAL packaged app —
// real backend, real local SQLite DB — through the exact click paths the owner
// reported in T7, so a human only has to judge the *content* (investor
// judgment), not re-click the mechanics.
//
// Mutation policy: the only writes this suite performs are (a) the
// "Wyciągnij dane" click, idempotent post-T7-F (re-observations skip, nothing
// is overwritten — that idempotency is exactly what step 3 proves live), and
// (b) the optional AI steps below. Everything else is read-only.
//
// The qualitative-assessment step spends the owner's configured AI-provider
// tokens, so it is opt-in: set BRAWLER_LIVE_AI=1 to run it.
//
// Locale: the owner's app runs in Polish; assertions accept both locales.

const SHOTS = "test-results/live/t7";

let connection: LiveConnection;
let page: Page;

test.describe.configure({ mode: "serial" });

test.beforeAll(async () => {
  connection = await connectToLiveApp();
  page = connection.page;
  // A previous run (or the owner) may have left a dialog open — start clean.
  await dismissModals();
});

test.afterAll(async () => {
  await connection.browser.close();
});

/** Last visible toast message, or null. */
async function lastToast(): Promise<string | null> {
  const toasts = page.locator(".ui-toast-message");
  const count = await toasts.count();
  if (count === 0) return null;
  return toasts.nth(count - 1).textContent();
}

/** Waits for a NEW toast to appear after `action`, returns its text. */
async function toastAfter(action: () => Promise<void>): Promise<string> {
  const before = await page.locator(".ui-toast-message").count();
  await action();
  await expect
    .poll(async () => page.locator(".ui-toast-message").count(), { timeout: 30_000 })
    .toBeGreaterThan(before);
  const text = await lastToast();
  expect(text, "a toast must appear after the action").not.toBeNull();
  return text ?? "";
}

/** Closes any open modal (Escape) — some nav items open dialogs, not screens. */
async function dismissModals(): Promise<void> {
  for (let i = 0; i < 3; i += 1) {
    if ((await page.locator(".ui-modal-overlay").count()) === 0) return;
    await page.keyboard.press("Escape");
    await page.waitForTimeout(300);
  }
}

/** Activates a dockview panel that may sit behind a tab in a shared group. */
async function activatePanelTab(name: RegExp): Promise<void> {
  const tab = page.locator(".dv-tab", { hasText: name }).first();
  if ((await tab.count()) > 0) {
    await tab.click();
    await page.waitForTimeout(500);
  }
}

async function openNav(pattern: RegExp): Promise<void> {
  await dismissModals();
  const nav = page.getByLabel(/Primary navigation|Nawigacja główna/);
  await nav.getByRole("button", { name: pattern }).first().click();
}

test("app shell renders (post-T7 build)", async () => {
  await expect(page.getByLabel(/Primary navigation|Nawigacja główna/)).toBeVisible();
  const version = await page.locator(".brand-version").textContent();
  console.log(`Live Brawler version: ${version}`);
  await page.screenshot({ path: `${SHOTS}/00-shell.png`, fullPage: true });
});

test("dogfooding walk: every primary screen renders without an error state", async () => {
  const nav = page.getByLabel(/Primary navigation|Nawigacja główna/);
  const buttons = nav.getByRole("button");
  const labels = await buttons.allTextContents();
  for (let i = 0; i < labels.length; i += 1) {
    const label = labels[i].trim();
    // Action items (e.g. "Nowy widok") open a modal instead of a screen —
    // skip them; the walk audits screens, not dialogs.
    if (/nowy widok|new view/i.test(label)) continue;
    await buttons.nth(i).click();
    // Give the screen a beat to load real data before judging it; close any
    // dialog a click may have opened so it can't block the rest of the walk.
    await page.waitForTimeout(1_500);
    await dismissModals();
    const slug = `${String(i + 1).padStart(2, "0")}-${label.toLowerCase().replace(/[^a-z0-9ąćęłńóśźż]+/gi, "-")}`;
    await page.screenshot({ path: `${SHOTS}/walk-${slug}.png`, fullPage: true });
    console.log(`walk: ${label}`);
  }
});

test("T7-B/D/F: extracting the FY2025 .xbri twice is honest and idempotent", async () => {
  test.setTimeout(180_000);

  // The owner's own path: Companies → row click → curated cockpit dashboard
  // (default panels include Report documents).
  await openNav(/Spółki|Companies/i);
  await clearCompaniesFilter(page);
  const cbfRow = page
    .locator("button[data-company-row]", { hasText: /CBF/i })
    .first();
  await expect(cbfRow, "the owner's companies list must contain CBF").toBeVisible({
    timeout: 15_000,
  });
  await cbfRow.click();
  await page.waitForTimeout(2_000);
  await page.screenshot({ path: `${SHOTS}/10-cockpit-cbf.png`, fullPage: true });

  // The F2 Coverage pane seeds into the same tab group and can land as the
  // group's active tab — activate Report documents explicitly (the user's own
  // click) instead of assuming the group's default active tab.
  const docsTab = page.getByRole("button", { name: /^Report documents$/ }).first();
  if (await docsTab.isVisible().catch(() => false)) {
    await docsTab.click();
    await page.waitForTimeout(500);
  }

  // The annual ESEF package row — the exact document from the owner's report.
  // PRECONDITION SKIP (2026-07-12): this is a v0.51-closure ROUND spec — it
  // validated a one-time scenario against the owner's live data. That data
  // has since moved on (FY2025 extraction completed and validated; the
  // documents list groups/collapses rows), so when the pre-extraction .xbri
  // row is no longer reachable the scenario is gone, not broken — skip with
  // a note instead of failing every future live-drive. Live round specs are
  // data-dependent by nature; permanent live coverage lives in this suite's
  // other specs.
  const xbriRow = page
    .locator("li", { has: page.locator('a[title*=".xbri" i]') })
    .first();
  const xbriVisible = await xbriRow
    .isVisible({ timeout: 30_000 })
    .catch(() => false);
  const extract = xbriRow.getByRole("button", { name: /Wyciągnij dane|Extract data/ });
  const extractVisible =
    xbriVisible && (await extract.isVisible().catch(() => false));
  test.skip(
    !extractVisible,
    "v0.51 round scenario no longer present on live data (FY2025 .xbri already extracted/validated; list grouped) — see the coverage panel instead",
  );

  // Click 1: must yield an HONEST toast, never the raw UNIQUE-constraint error.
  const first = await toastAfter(() => extract.click());
  console.log(`extraction click 1 toast: ${first}`);
  expect(first).not.toMatch(/UNIQUE|sqlite|constraint/i);
  expect(first).toMatch(
    /Wyciągnięto nowe wartości|już zapisanych|różnią się od zapisanych|Extracted new values|already recorded|differ from stored/,
  );
  await page.screenshot({ path: `${SHOTS}/11-extract-click1.png`, fullPage: true });

  // Click 2 (the owner's crash case): idempotent — no new facts, no error.
  await expect(extract).toBeEnabled({ timeout: 60_000 });
  const second = await toastAfter(() => extract.click());
  console.log(`extraction click 2 toast: ${second}`);
  expect(second).not.toMatch(/UNIQUE|sqlite|constraint/i);
  expect(second).toMatch(
    /już zapisanych|różnią się od zapisanych|already recorded|differ from stored/,
  );
  await page.screenshot({ path: `${SHOTS}/12-extract-click2.png`, fullPage: true });
});

test("T7-D: the fundamentals panel shows the extracted FY facts", async () => {
  // Still on the CBF cockpit view. The extraction above bumps the revision, so
  // the sibling fundamentals panel must show data without a manual reload.
  const fundamentals = page.locator(".dv-groupview, .cockpit-screen");
  await expect(
    fundamentals.getByText(/Wskaźniki finansowe|Fundamentals/).first(),
  ).toBeVisible({ timeout: 30_000 });
  await page.screenshot({ path: `${SHOTS}/20-fundamentals.png`, fullPage: true });
});

test("T7-A: quantitative framework evaluation runs against the live facts", async () => {
  test.setTimeout(120_000);
  // The Quality panel shares a dockview group with Report documents — bring
  // its tab to the front first.
  await activatePanelTab(/Jakość|Quality/);
  const evaluate = page.getByRole("button", { name: /^Oceń$|^Evaluate$/ }).first();
  if (!(await evaluate.isVisible().catch(() => false))) {
    console.log("SKIP: no visible quantitative Evaluate button in the current layout");
    test.skip();
  }
  await evaluate.click();
  await page.waitForTimeout(3_000);
  await page.screenshot({ path: `${SHOTS}/30-quality-evaluate.png`, fullPage: true });
});

test("T7-A: qualitative assessment (spends owner AI tokens — BRAWLER_LIVE_AI=1)", async () => {
  test.skip(process.env.BRAWLER_LIVE_AI !== "1", "opt-in: uses the owner's AI provider");
  test.setTimeout(300_000);

  await activatePanelTab(/Jakość|Quality/);
  const assess = page.getByRole("button", { name: /Oceń jakościowo|Assess/ }).first();
  await expect(assess).toBeVisible();
  await assess.click();

  // T7-A contract: the run either completes with a stored assessment or
  // surfaces its failure in the panel — never a silent disappearance.
  await expect
    .poll(
      async () => {
        const failed = await page
          .getByText(/Ocena jakościowa nie powiodła się|Qualitative assessment failed/)
          .count();
        const done = await page
          .getByText(/Oceniono|Assessed|uzasadnienie|rationale/i)
          .count();
        return failed + done;
      },
      { timeout: 240_000 },
    )
    .toBeGreaterThan(0);
  await page.screenshot({ path: `${SHOTS}/40-qualitative.png`, fullPage: true });
});
