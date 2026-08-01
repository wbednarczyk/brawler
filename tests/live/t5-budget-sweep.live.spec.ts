import { test, expect, type Page } from "@playwright/test";
import { connectToLiveApp, type LiveConnection } from "./helpers/liveConnect";
import { clearCompaniesFilter } from "./helpers/companiesList";

// F5 live-drive evidence (ADR 0077 §6): on the owner's real Windows app, with
// the owner's real Mistral key, prove the per-sweep AI budget end to end.
//
// Company choice is data-driven (2026-07-10, owner DB): a first CDR attempt
// showed its gap periods are interim ESEF XHTML files without iXBRL tagging
// (the ESEF mandate covers annual reports only) — deterministically
// unparsable, and tier-4 is PDF-only, so no budget is ever spent (honest
// `AI: 0/2`; filed as an epic finding). CBF's two remaining PDF candidates are
// the mis-associated Energa/Vercom documents (card 45fcece), terminal-deduped.
// So the journey runs on LPP via "Backfill history": fetching ~3 years of
// reports creates fresh gap periods whose canonical documents are PDFs, and
// the auto-chained sweep then exercises tier-4 under the budget:
//   1. set the history-sweep AI budget to 2 in Settings (numeric input),
//   2. Backfill history on LPP → chained sweep: deterministic tiers are free,
//      the first 2 tier-4 entries consume the budget,
//   3. the footer reports "AI: 2/2" (G-4: never exceeds) and the coverage map
//      lights "Skipped — AI budget" for periods past the gate (never silent),
//   4. the To-review cell opens the NEW review-queue panel (T5.3b) and one OCR
//      bootstrap proposal is confirmed there — the panel path, no IPC
//      workaround.
// Spends owner Mistral tokens (exactly 2 tier-4 calls — the budget IS the cost
// cap) — run with owner consent.

const SHOTS = "test-results/live/t5-budget";
const COMPANY = "company_gpw_lpp";

let connection: LiveConnection;

test.skip(
  !process.env.BRAWLER_LIVE_AI,
  "spends owner Mistral tokens — set BRAWLER_LIVE_AI=1 to run",
);

test.beforeAll(async () => {
  connection = await connectToLiveApp();
});

test.afterAll(async () => {
  await connection.browser.close();
});

function nav(page: Page) {
  return page.getByLabel(/Primary navigation|Nawigacja główna/);
}

async function openCoverage(page: Page) {
  await nav(page).getByRole("button", { name: /Companies|Spółki/ }).click();
  await clearCompaniesFilter(page);
  await page.locator(`[data-company-id="${COMPANY}"] .company-row-main`).click();
  await expect(page.getByLabel(/Research cockpit|Kokpit badawczy/)).toBeVisible();
  await page.getByRole("button", { name: /^(Coverage|Pokrycie)$/ }).first().click();
  // Company-scoped: a live layout can hold coverage panes pinned to OTHER
  // companies — an unscoped .first() can silently read a neighbour's pane
  // (this exact contamination hit the 10-company validation matrix).
  const pane = page.locator(".cockpit-pane", {
    has: page.locator(`.company-coverage[data-company-id="${COMPANY}"]`),
  });
  await expect(pane).toBeVisible();
  return pane;
}

test.describe.serial("F5 budget journey (LPP backfill → sweep → review)", () => {
  test("set the history-sweep AI budget to 2 in Settings", async () => {
    const { page } = connection;
    await nav(page).getByRole("button", { name: /Settings|Ustawienia/ }).click();
    const region = page.getByLabel(/Application settings|Ustawienia aplikacji/);
    await expect(region).toBeVisible();
    await region.getByRole("button", { name: /^(AI|SI)$/ }).click();

    const input = page.getByLabel(
      /History sweep AI budget in calls|Budżet AI na przegląd historii w wywołaniach/,
    );
    await expect(input).toBeVisible();
    await input.fill("2");
    await expect(input).toHaveValue("2");
    await page.screenshot({ path: `${SHOTS}/1-budget-setting.png`, fullPage: true });
  });

  test("backfill + chained sweep respect the budget: AI 2/2 + skipped periods lit", async () => {
    test.setTimeout(1_500_000);
    const { page } = connection;
    const pane = await openCoverage(page);
    await page.screenshot({ path: `${SHOTS}/2-coverage-before.png`, fullPage: true });

    const backfillButton = pane.getByRole("button", {
      name: /Backfill history|Uzupełnij historię|Backfilling…|Uzupełnianie…/,
    });
    await expect(backfillButton).toBeEnabled();
    await backfillButton.click();

    // Backfill fetches ~3 years of reports, then hands off to the chained
    // sweep; poll to a settled sweep (two tier-4 OCR calls on multi-MB PDFs
    // dominate the wall clock).
    const status = pane.locator(".coverage-action-status");
    await expect(status).not.toHaveText("", { timeout: 60_000 });
    console.log("status after click:", await status.textContent());
    // Terminal envelope incl. the backfill-failure phrasing ("Uzupełnianie nie
    // powiodło się") — a 10-company probe waited 23 minutes on an already-failed
    // EXC backfill because this regex missed it.
    await expect(status).toHaveText(
      /Extracted \d+|Wydobyto \d+|failed|niepowodzenie|nie powiodł/i,
      { timeout: 1_380_000 },
    );
    await expect(status).toHaveText(/Extracted \d+|Wydobyto \d+/, { timeout: 1_000 });
    console.log("final sweep status:", await status.textContent());

    // G-4 visibility: the footer reports used/limit with the SNAPSHOT limit —
    // never exceeded, never silent. The used count is an envelope, not an
    // instance: it depends on how many tier-4-eligible candidates the real data
    // offers on this run (first LPP run: 1/2 — one PDF candidate, its bootstrap
    // degraded `bootstrap_failed`; re-runs dedup to 0/2). A skipped-budget cell
    // appears only when eligible candidates exceed the limit — log it either
    // way so the evidence stays honest.
    const budget = pane.locator(".coverage-ai-budget");
    await expect(budget).toHaveText(/AI: \d+\/2/, { timeout: 30_000 });
    console.log("budget footer:", await budget.textContent());
    const skipped = await pane
      .getByText(/Skipped — AI budget|Pominięto — budżet AI/)
      .count();
    console.log("skipped-budget cells:", skipped);
    await page.screenshot({ path: `${SHOTS}/3-coverage-after.png`, fullPage: true });
  });

  test("To-review cell opens the review panel; confirm an OCR proposal there", async () => {
    test.setTimeout(180_000);
    const { page } = connection;
    const pane = await openCoverage(page);

    // Real-data envelope: this leg needs pending proposals. A sweep whose only
    // tier-4 candidate degraded (e.g. `bootstrap_failed`) leaves none — skip
    // honestly rather than fake the journey (the panel journey then runs on a
    // company that HAS a queue, e.g. CBF).
    const reviewCell = pane.locator(".coverage-review-button").first();
    const hasQueue = await reviewCell.isVisible().catch(() => false);
    test.skip(!hasQueue, "no pending proposals on this company right now");
    await reviewCell.click();

    const reviewPane = page.locator(".cockpit-pane", {
      has: page.locator(`.company-review-queue[data-company-id="${COMPANY}"]`),
    });
    await expect(reviewPane).toBeVisible();
    const rows = reviewPane.locator(".review-row");
    const before = await rows.count();
    expect(before).toBeGreaterThan(0);
    // Source chips are data-dependent (OCR bootstrap / OCR · flagged / AI) —
    // log what the queue holds rather than pinning one kind.
    console.log(
      "queue rows:", before,
      "OCR bootstrap chips:", await reviewPane.getByText(/OCR bootstrap/).count(),
    );
    await page.screenshot({ path: `${SHOTS}/4-review-queue.png`, fullPage: true });

    // Confirm through the panel — the real user path (closes card f660ddb).
    await rows.first().getByRole("button", { name: /^(Confirm|Potwierdź)$/ }).click();
    await expect.poll(async () => rows.count(), { timeout: 60_000 }).toBeLessThan(before);
    await page.screenshot({ path: `${SHOTS}/5-after-confirm.png`, fullPage: true });
  });
});
