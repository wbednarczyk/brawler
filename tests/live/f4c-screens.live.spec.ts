import { test, expect } from "@playwright/test";
import { connectToLiveApp, type LiveConnection } from "./helpers/liveConnect";

// F4c real-data check (DoD § G, docs/plans/frontend-v2-f4c.md § Verification):
// on the owner's real Windows app — Research is reachable from the Library nav
// (and Ctrl+4) at the real window size, Settings reads as product language on every tab, an Inbox item
// lands its note draft in the Spółka Notebook tool, and no path to the retired
// global Notebooks/Journal screens remains. Screenshots are attached for the
// integrator's review; assertions stay structural (the mock harness owns pixel
// proofs).

let connection: LiveConnection;

test.beforeAll(async () => {
  connection = await connectToLiveApp();
});

test.afterAll(async () => {
  await connection.browser.close();
});

// The live app keeps state between specs and runs: close any dialog a previous
// (possibly interrupted) test left open before the next one clicks the sidebar.
test.beforeEach(async () => {
  await connection.page.keyboard.press("Escape");
  await discardUnsavedTool(connection.page);
});

// The Spółka dirty-tool guard (ADR 0107) blocks navigation while a tool holds
// an unsaved draft — exactly what test 3 leaves behind. Discard it so the next
// test's sidebar click is not answered by the "Unsaved changes" dialog.
async function discardUnsavedTool(page: LiveConnection["page"]) {
  await page.getByRole("button", { name: /^(Dziś|Today)( |$)/ }).first().click({ timeout: 10_000 }).catch(() => {});
  const discard = page.getByRole("button", { name: /^(Odrzuć|Discard)$/ });
  if (await discard.isVisible().catch(() => false)) await discard.click();
}

const SHOTS = "test-results/live-f4c";

async function shoot(
  page: LiveConnection["page"],
  name: string,
  testInfo: { attach: (n: string, o: { body: Buffer; contentType: string }) => Promise<unknown> },
) {
  const body = await page.screenshot({ fullPage: false });
  await testInfo.attach(name, { body, contentType: "image/png" });
  const fs = await import("node:fs/promises");
  await fs.mkdir(SHOTS, { recursive: true });
  await fs.writeFile(`${SHOTS}/${name}.png`, body);
}

async function expectAtMostOneFilled(page: LiveConnection["page"]) {
  expect(await page.locator('[data-ui-button-variant="primary"]:visible').count()).toBeLessThanOrEqual(1);
  expect(await page.locator('[data-ux-primary-action="true"]:visible').count()).toBeLessThanOrEqual(1);
}


test("Research opens from the Library nav and via Ctrl+4, labelled actions, one primary at most", async ({}, testInfo) => {
  const { page } = connection;
  test.setTimeout(120_000);
  await page.getByRole("button", { name: /^Research$/ }).first().click();
  const panel = page.locator(".research-panel");
  await expect(panel).toBeVisible({ timeout: 15_000 });
  // Labelled actions (ADR 0104 dec. 3 amendment): no visible button without text.
  const unlabelled = panel.locator("button:visible").filter({ hasNotText: /\S/ });
  expect(await unlabelled.count()).toBe(0);
  await expectAtMostOneFilled(page);
  await shoot(page, "research-nav", testInfo);
  // Ctrl+4 lands on Research from another screen.
  await page.getByRole("button", { name: /^(Dziś|Today)( |$)/ }).first().click();
  await page.keyboard.press("Control+4");
  await expect(panel).toBeVisible({ timeout: 15_000 });
});

test("Settings: every tab renders product language (no ops vocabulary on screen)", async ({}, testInfo) => {
  const { page } = connection;
  test.setTimeout(180_000);
  await page.getByRole("button", { name: /^(Ustawienia|Settings)$/ }).first().click();
  const region = page.getByLabel(/Ustawienia aplikacji|Application settings/);
  await expect(region).toBeVisible({ timeout: 15_000 });
  const tabs = region.locator('[data-action-kind="control"]:visible');
  const count = await tabs.count();
  expect(count).toBeGreaterThanOrEqual(8);
  const banned = /\b(pool|workers?|threads?|adapter|stdio|backfill|telemetry|JSON)\b/i;
  for (let i = 0; i < count; i += 1) {
    const tab = tabs.nth(i);
    const name = (await tab.textContent())?.trim() ?? `tab-${i}`;
    await tab.click();
    const copy = await region.innerText();
    // Copy-paste snippets (the MCP registration commands) legitimately carry
    // implementation tokens; everything else on the tab must be product language.
    const prose = copy.replace(/claude mcp add[^\n]*|BRAWLER_MCP_TOKEN[^\n]*|brawler-mcp-stdio[^\n]*/g, "");
    expect(prose, `ops vocabulary on Settings tab ${name}`).not.toMatch(banned);
    await expectAtMostOneFilled(page);
    await shoot(page, `settings-${i}-${name.toLowerCase().replace(/[^a-z0-9]+/g, "-")}`, testInfo);
  }
});

test("Inbox item → Note lands in the Spółka Notebook tool with the draft prefilled", async ({}, testInfo) => {
  const { page } = connection;
  test.setTimeout(120_000);
  await page.getByRole("button", { name: /^Inbox( |$)/ }).first().click();
  const firstRow = page.locator(".feed-row, [data-feed-item-id]").first();
  await expect(firstRow).toBeVisible({ timeout: 15_000 });
  await firstRow.click();
  await page.getByRole("button", { name: /^(Notatka|Note)$/ }).first().click();
  const tool = page.getByRole("group", { name: /Narzędzie warsztatu|Workshop tool/ });
  await expect(tool).toBeVisible({ timeout: 15_000 });
  await expect(tool.locator("textarea:visible").first()).not.toHaveValue("");
  await shoot(page, "inbox-note-draft-in-spolka", testInfo);
  // Leave the live notebook as we found it: the draft is never saved.
  await discardUnsavedTool(page);
});

test("No path to the retired global Notebooks/Journal screens remains", async ({}) => {
  const { page } = connection;
  test.setTimeout(60_000);
  await page.keyboard.press("Control+K");
  const palette = page.getByRole("dialog", { name: /Command palette|Paleta poleceń/ });
  await expect(palette).toBeVisible({ timeout: 10_000 });
  const search = palette.getByLabel(/Search commands|Szukaj poleceń/);
  for (const needle of ["Notebook", "Notatnik", "Journal", "Dziennik"]) {
    await search.fill(needle);
    await expect(palette.getByRole("button", { name: /(Open screen|Otwórz ekran): / })).toHaveCount(0);
  }
  await search.fill("Research");
  await expect(palette.getByRole("button", { name: /(Open screen|Otwórz ekran): Research/ })).toHaveCount(1);
  await page.keyboard.press("Escape");
  expect(await page.getByRole("button", { name: /^(Notatniki|Notebooks|Dziennik|Journal)$/ }).count()).toBe(0);
});
