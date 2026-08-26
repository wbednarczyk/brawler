import { test } from "@playwright/test";
import { mkdirSync } from "node:fs";
import { connectToLiveApp, type LiveConnection } from "./helpers/liveConnect";

// v0.57 screenshot evidence (T9). Robust, best-effort: every step is wrapped so
// a stale page handle or a toast interception never aborts the capture. Backend
// outcome evidence lives in v057-company-health.live.spec.ts; this spec only
// gathers the visual surfaces for the human charter.

const SHOTS = "test-results/live/v057";
mkdirSync(SHOTS, { recursive: true });

let connection: LiveConnection;

test.beforeAll(async () => {
  connection = await connectToLiveApp();
});
test.afterAll(async () => {
  await connection.browser.close();
});

test("v0.57 surfaces — screenshots", async () => {
  test.setTimeout(180_000);
  const page = connection.page;

  const dismiss = async () => {
    try {
      for (let i = 0; i < 4; i += 1) {
        await page.keyboard.press("Escape").catch(() => {});
        await page.waitForTimeout(200);
      }
      // Clear any lingering toast viewport by waiting it out.
      await page.waitForTimeout(500);
    } catch {
      /* ignore */
    }
  };

  const shot = async (name: string) => {
    try {
      await page.screenshot({ path: `${SHOTS}/${name}.png`, fullPage: true });
      console.log(`captured ${name}`);
    } catch (e) {
      console.log(`shot ${name} failed: ${String(e)}`);
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
  await shot("10-initial");

  await click(/^(Legacy dashboard|Dawny dashboard)/, "Legacy dashboard row");
  await shot("11-cockpit");

  await click(/^(Podstawowe informacje|Basic info)$/, "Basic info tab");
  await shot("12-basic-info-ownership-insider");

  await click(/^(Jakość|Quality)$/, "Quality tab");
  await shot("13-quality-health");

  // Report which v0.57 surfaces are present in the DOM (best-effort).
  const present = async (sel: string, label: string) => {
    try {
      const n = await page.locator(sel).count();
      console.log(`DOM ${label}: ${n}`);
    } catch (e) {
      console.log(`DOM ${label}: query failed (${String(e).slice(0, 80)})`);
    }
  };
  await present(".company-health-section", "company-health-section");
  await present(".insider-net, [class*='insider']", "insider block");
  await present(".ownership-skin-badge", "skin badge");
  await present(".red-flags-row, [class*='red-flag']", "red-flags rows");
  await shot("14-final");
});
