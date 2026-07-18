import { test } from "@playwright/test";
import { mkdirSync } from "node:fs";
import { connectToLiveApp, type LiveConnection } from "./helpers/liveConnect";

// v0.57 pass-2 screenshot evidence. Best-effort, non-mutating: navigates by
// tab/sidebar clicks only (never rearranges saved layouts). Captures the Today
// view (toast-cap vs the owner's wall complaint), a company cockpit, the
// ownership+insider block (skin badge), the quality/health tab, and a
// narrow-viewport (1008px via CDP emulation) pass.

const SHOTS = "test-results/live/v057-2";
mkdirSync(SHOTS, { recursive: true });

let connection: LiveConnection;

test.beforeAll(async () => {
  connection = await connectToLiveApp();
});
test.afterAll(async () => {
  await connection.browser.close();
});

test("v0.57 pass-2 surfaces", async () => {
  test.setTimeout(240_000);
  const page = connection.page;

  const shot = async (name: string) => {
    try {
      await page.screenshot({ path: `${SHOTS}/${name}.png`, fullPage: true });
      console.log(`captured ${name}`);
    } catch (e) {
      console.log(`shot ${name} FAILED: ${String(e).slice(0, 120)}`);
    }
  };
  const dismiss = async () => {
    for (let i = 0; i < 3; i++) { await page.keyboard.press("Escape").catch(() => {}); await page.waitForTimeout(150); }
  };
  const clickBtn = async (name: RegExp, label: string) => {
    try {
      await page.getByRole("button", { name }).first().click({ timeout: 8000, force: true });
      await page.waitForTimeout(1200);
      console.log(`clicked ${label}`);
      return true;
    } catch (e) { console.log(`click ${label} skipped: ${String(e).slice(0, 90)}`); return false; }
  };
  const domCount = async (sel: string, label: string) => {
    try { console.log(`DOM ${label}: ${await page.locator(sel).count()}`); } catch { console.log(`DOM ${label}: query failed`); }
  };

  // 1) Today / landing view — toast cap evidence (owner's wall complaint).
  await dismiss();
  await shot("20-today-landing");
  await domCount("[class*='toast']", "toast elements");
  await domCount("[class*='attention'], [class*='signal-card']", "attention/signal cards");

  // 2) Company cockpit — click Dashboard/Pulpit.
  await clickBtn(/^(Dashboard|Pulpit)/, "Dashboard");
  await shot("21-cockpit");

  // 3) Try to activate a founder-led company from the sidebar pinned list.
  const founderNames = [/CD ?Projekt/i, /Kruk/i, /Asseco/i, /cyber/i, /AB S\.?A\.?|^AB\b/i, /Text/i, /Digitree|Digital/i];
  let activated = "";
  for (const rx of founderNames) {
    try {
      const btn = page.getByRole("button", { name: rx }).first();
      if (await btn.count()) {
        await btn.click({ timeout: 4000, force: true });
        await page.waitForTimeout(1000);
        activated = rx.source;
        console.log(`activated company matching ${rx.source}`);
        break;
      }
    } catch { /* keep trying */ }
  }
  if (!activated) console.log("no founder-led company found in sidebar; capturing current active company");

  // 4) Basic info tab — ownership + insider block (skin badge).
  let tab = page.getByRole("button", { name: /^(Podstawowe informacje|Basic info)$/ }).first();
  if ((await tab.count()) === 0) {
    await clickBtn(/Dodaj panel|Add panel/, "Add panel");
    await clickBtn(/(Otwórz panel|Open panel).*(Podstawowe informacje|Basic info)/, "Open Basic info panel");
    tab = page.getByRole("button", { name: /^(Podstawowe informacje|Basic info)$/ }).first();
  }
  if (await tab.count()) { await tab.click({ force: true }).catch(() => {}); await page.waitForTimeout(1500); }
  await shot("22-basic-info-ownership-insider");
  await domCount(".ownership-skin-badge, [class*='skin']", "skin badge");
  await domCount("[class*='insider']", "insider block");
  await domCount(".ownership-holder-row", "ownership holder rows");

  // 5) Quality tab — health tiles + red flags.
  await clickBtn(/^(Jakość|Quality)$/, "Quality tab");
  await shot("23-quality-health");
  await domCount(".company-health-section, [class*='health']", "health section");
  await domCount("[class*='red-flag'], [class*='redFlag']", "red flag rows");

  // 6) Narrow-viewport pass at 1008px via CDP emulation (real window can't resize).
  try {
    const client = await page.context().newCDPSession(page);
    await client.send("Emulation.setDeviceMetricsOverride", {
      width: 1008, height: 900, deviceScaleFactor: 1, mobile: false,
    });
    await page.waitForTimeout(1000);
    await shot("24-narrow-1008-basic-info");
    // Check for global horizontal scroll (body scrollWidth > clientWidth).
    const overflow = await page.evaluate(() => {
      const d = document.documentElement;
      return { scrollW: d.scrollWidth, clientW: d.clientWidth, overflow: d.scrollWidth > d.clientWidth + 2 };
    });
    console.log(`NARROW 1008 overflow: ${JSON.stringify(overflow)}`);
    await clickBtn(/^(Jakość|Quality)$/, "Quality tab (narrow)");
    await shot("25-narrow-1008-quality-health");
    const overflow2 = await page.evaluate(() => {
      const d = document.documentElement;
      return { scrollW: d.scrollWidth, clientW: d.clientWidth, overflow: d.scrollWidth > d.clientWidth + 2 };
    });
    console.log(`NARROW 1008 quality overflow: ${JSON.stringify(overflow2)}`);
    await clickBtn(/^(Dashboard|Pulpit)/, "Dashboard (narrow)");
    await shot("26-narrow-1008-cockpit");
    const overflow3 = await page.evaluate(() => {
      const d = document.documentElement;
      return { scrollW: d.scrollWidth, clientW: d.clientWidth, overflow: d.scrollWidth > d.clientWidth + 2 };
    });
    console.log(`NARROW 1008 cockpit overflow: ${JSON.stringify(overflow3)}`);
    // restore
    await client.send("Emulation.clearDeviceMetricsOverride");
  } catch (e) {
    console.log(`narrow-viewport emulation failed: ${String(e).slice(0, 150)}`);
  }
});
