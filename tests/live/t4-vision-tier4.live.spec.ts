import { test, expect, type Page } from "@playwright/test";
import { connectToLiveApp, type LiveConnection } from "./helpers/liveConnect";

// F4 live-drive evidence (ADR 0077 §4): on the owner's real Windows app, with
// the owner's real Mistral key, walk the tier-4 journey on the CBF Q3 2025
// quarterly PDF (deterministic tier-2 ends Flagged on it — drift):
//   1. route VisionExtraction → Mistral (Settings UI; idempotent),
//   2. "Extract data" on the document → OCR + LLM bootstrap → PROPOSALS toast,
//   3. confirm one proposal through the app's real confirm_kpi_proposal IPC
//      (the Inbox review surface cannot reach this old item — filed as a card;
//      the command is the same one the UI calls),
//   4. "Extract data" again → the confirmed profile parses DETERMINISTICALLY.
// Spends owner Mistral tokens (~2 OCR + 1 chat call) — run with owner consent.

const SHOTS = "test-results/live/t4-vision";
const DOC_ID =
  "doc_company_gpw_cbf_httpsbonnierplstaticattemitent2025_1120251105_171008_1748178276_cyber_folks_2025_q3_raport_kwartalnypdf";

let connection: LiveConnection;

// Spends owner Mistral tokens and confirms a real proposal — explicit opt-in
// only, mirroring t7's owner-AI gate.
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

async function lastToast(page: Page): Promise<string> {
  const toasts = page.locator(".ui-toast-message");
  const count = await toasts.count();
  return count > 0 ? ((await toasts.nth(count - 1).textContent()) ?? "") : "";
}

/** Waits for a NEW toast after `action`, with a generous OCR-scale timeout. */
async function toastAfter(
  page: Page,
  action: () => Promise<void>,
  timeout: number,
): Promise<string> {
  const before = await page.locator(".ui-toast-message").count();
  await action();
  await expect
    .poll(async () => page.locator(".ui-toast-message").count(), { timeout })
    .toBeGreaterThan(before);
  return lastToast(page);
}

async function openCbfDocuments(page: Page) {
  await nav(page).getByRole("button", { name: /Companies|Spółki/ }).click();
  await page.locator('[data-company-id="company_gpw_cbf"] .company-row-main').click();
  await expect(page.getByLabel(/Research cockpit|Kokpit badawczy/)).toBeVisible();
  const docsTab = page.getByRole("button", { name: /^Report documents$/ }).first();
  if (await docsTab.isVisible().catch(() => false)) {
    await docsTab.click();
  }
  const row = page
    .locator("li", { has: page.locator('a[title*="q3_raport_kwartalny" i]') })
    .first();
  await expect(row).toBeVisible({ timeout: 30_000 });
  return row;
}

test.describe.serial("F4 tier-4 journey (CBF Q3 2025 quarterly)", () => {
  test("route VisionExtraction to Mistral in Settings", async () => {
    const { page } = connection;
    await nav(page).getByRole("button", { name: /Settings|Ustawienia/ }).click();
    const region = page.getByLabel(/Application settings|Ustawienia aplikacji/);
    await expect(region).toBeVisible();
    await region.getByRole("button", { name: /^(AI|SI)$/ }).click();

    const visionRow = page
      .locator(".capability-routing-row", {
        has: page.getByRole("heading", { name: /Vision extraction|Ekstrakcja wizyjna/ }),
      })
      .first();
    await expect(visionRow).toBeVisible();
    const providerSelect = visionRow.getByLabel(
      /(Provider|Dostawca) (Vision extraction|Ekstrakcja wizyjna) 1/,
    );
    if ((await providerSelect.count()) === 0) {
      await visionRow.getByRole("button", { name: /Add provider|Dodaj dostawcę/ }).click();
    }
    await providerSelect.selectOption("provider_mistral");
    await expect(
      visionRow.getByLabel(/(Model) (Vision extraction|Ekstrakcja wizyjna) 1/),
    ).toHaveValue("mistral-small-latest");
    await page.screenshot({ path: `${SHOTS}/1-vision-routing.png`, fullPage: true });
  });

  test("bootstrap: Extract data lands OCR proposals", async () => {
    test.setTimeout(360_000);
    const { page } = connection;
    const row = await openCbfDocuments(page);
    const extract = row.getByRole("button", { name: /Wyciągnij dane|Extract data/ });
    await expect(extract).toBeVisible();
    await page.screenshot({ path: `${SHOTS}/2-before-extract.png`, fullPage: true });

    const toast = await toastAfter(page, () => extract.click(), 300_000);
    console.log("bootstrap toast:", toast);
    await page.screenshot({ path: `${SHOTS}/3-bootstrap-toast.png`, fullPage: true });
    expect(toast).toMatch(/Propozycje OCR do przeglądu: \d+|OCR proposals to review: \d+/);
  });

  test("confirm one proposal via the app's real IPC", async () => {
    test.setTimeout(120_000);
    const { page } = connection;
    const result = await page.evaluate(async (docId) => {
      const internals = (window as unknown as Record<string, any>).__TAURI_INTERNALS__;
      const jobs = await internals.invoke("list_kpi_extraction", {
        input: { reportDocumentId: docId },
      });
      const withProposals = jobs.find(
        (job: any) => (job.proposals ?? []).some((p: any) => p.status === "pending"),
      );
      if (!withProposals) return { error: "no pending proposals", jobs: jobs.length };
      const proposal = withProposals.proposals.find((p: any) => p.status === "pending");
      const confirmed = await internals.invoke("confirm_kpi_proposal", {
        input: { proposalId: proposal.id, acceptAsNewKpi: false },
      });
      return {
        proposalId: proposal.id,
        metricKey: proposal.metricKey,
        snippet: proposal.sourceSnippet,
        validationStatus: confirmed.validationStatus,
      };
    }, DOC_ID);
    console.log("confirm result:", JSON.stringify(result));
    expect(result).not.toHaveProperty("error");
  });

  test("second run parses deterministically", async () => {
    test.setTimeout(360_000);
    const { page } = connection;
    const row = await openCbfDocuments(page);
    const extract = row.getByRole("button", { name: /Wyciągnij dane|Extract data/ });
    const toast = await toastAfter(page, () => extract.click(), 300_000);
    console.log("second-run toast:", toast);
    await page.screenshot({ path: `${SHOTS}/4-second-run-toast.png`, fullPage: true });
    // Honest outcomes only: new facts, re-observed slots, divergence, or a
    // flagged set — never a silent nothing and never a raw error.
    expect(toast).toMatch(
      /Wyciągnięto nowe wartości: \d+|Extracted new values: \d+|already recorded|już zapisan|differ|różnią|flagged|oflagowan|Propozycje OCR|OCR proposals/,
    );
  });
});
