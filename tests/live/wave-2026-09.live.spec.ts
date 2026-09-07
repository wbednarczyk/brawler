import { test, expect } from "@playwright/test";
import { connectToLiveApp, type LiveConnection } from "./helpers/liveConnect";

// Dogfooding wave 2026-09 real-data check (DoD § G): on the owner's real Windows
// app (PL locale, real DB) — the Spółka picker is a centred type-ahead combobox,
// the Fundamentals facts matrix shows the newest measured-capacity periods with
// the full-height "Rozwiń starsze" column, the Activity panel's document action
// lands on a visibly selected document row, feed rows carry PL kind chips.
// Assertions are structural; screenshots capture the owner's theme.

let connection: LiveConnection;

test.beforeAll(async () => {
  connection = await connectToLiveApp();
});

test.afterAll(async () => {
  await connection.browser.close();
});

const SHOTS = "test-results/live-wave-2026-09";

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

test("wave 2026-09 on the real app: picker, period tables, document selection, feed chips", async ({}, testInfo) => {
  const { page } = connection;
  test.setTimeout(240_000);

  // 1. Spółka via the palette, then the picker: centred combobox with type-ahead.
  await page.keyboard.press("Escape");
  await page.keyboard.press("Control+K");
  const palette = page.getByRole("dialog", { name: /Command palette|Paleta poleceń/ });
  await expect(palette).toBeVisible({ timeout: 10_000 });
  await palette.getByRole("combobox").pressSequentially("Otwórz spółkę: ");
  await page.keyboard.press("Enter");
  const spolka = page.getByRole("region", { name: /Widok spółki|Company view/ });
  await expect(spolka).toBeVisible({ timeout: 15_000 });

  const picker = spolka.getByRole("combobox", { name: /^(Spółka|Company)$/ });
  await expect(picker).toBeVisible();
  const [pickerBox, headerBox] = await Promise.all([
    picker.boundingBox(),
    spolka.locator(".ui-panel-header").first().boundingBox(),
  ]);
  if (pickerBox && headerBox && headerBox.width >= 1100) {
    const pickerCenter = pickerBox.x + pickerBox.width / 2;
    const headerCenter = headerBox.x + headerBox.width / 2;
    expect(Math.abs(pickerCenter - headerCenter)).toBeLessThanOrEqual(24);
  }
  await picker.click();
  await picker.pressSequentially("xt");
  const listbox = page.getByRole("listbox");
  await expect(listbox).toBeVisible({ timeout: 5_000 });
  await shoot(page, "picker-typeahead", testInfo);
  await page.keyboard.press("Escape"); // closes the list only
  await expect(listbox).toBeHidden();
  await page.keyboard.press("Escape"); // clears the query
  await expect(picker).toHaveValue("");

  // 2. Fundamentals: newest periods fit; the expander column names the hidden count.
  await spolka.getByRole("toolbar", { name: /Warsztat|Workshop/ }).getByRole("button", { name: /^(Fundamenty|Fundamentals)$/ }).click();
  const matrixScroll = page.locator(".facts-matrix-scroll");
  await expect(matrixScroll).toBeVisible({ timeout: 15_000 });
  const geometry = await matrixScroll.evaluate((el) => {
    const newest = Array.from(el.querySelectorAll("thead th[data-period-cell]")).at(-1); // the newest period header
    const box = newest?.getBoundingClientRect();
    const scroller = el.getBoundingClientRect();
    return { newestRight: box ? box.right : null, clientRight: scroller.left + el.clientWidth, scrollLeft: el.scrollLeft };
  });
  expect(geometry.scrollLeft).toBe(0);
  if (geometry.newestRight !== null) {
    expect(geometry.newestRight).toBeLessThanOrEqual(geometry.clientRight + 1);
  }
  const expander = matrixScroll.getByRole("button", { name: /Rozwiń \d+ starsz|Expand \d+ earlier/ });
  if (await expander.count()) {
    await shoot(page, "matrix-collapsed", testInfo);
    await expander.click();
    await expect(matrixScroll.getByRole("button", { name: /Zwiń starsze|Collapse earlier/ })).toBeVisible();
    const scrolledRight = await matrixScroll.evaluate((el) => el.scrollLeft > 0);
    expect(scrolledRight).toBe(true);
    await shoot(page, "matrix-expanded", testInfo);
  } else {
    testInfo.annotations.push({ type: "matrix", description: "every period fits — no expander column" });
  }

  // 3. Activity → Otwórz w dokumentach → the document row is selected.
  const indicator = page.getByRole("button", { name: /Otwórz aktywność|Open activity/ });
  await indicator.click();
  const dialog = page.getByRole("dialog", { name: /^(Aktywność|Activity)$/ });
  await expect(dialog).toBeVisible({ timeout: 10_000 });
  const docRow = dialog.locator(".activity-item[data-activity-tool='dokumenty']").first();
  if (await docRow.count()) {
    const documentId = await docRow.getAttribute("data-activity-document");
    await docRow.getByRole("button", { name: /Otwórz w dokumentach|Open in documents/ }).click();
    await expect(dialog).toBeHidden();
    const selected = page.locator(`[data-document-id="${documentId}"][aria-current="true"]`);
    await expect(selected).toBeVisible({ timeout: 15_000 });
    await shoot(page, "document-selected", testInfo);
  } else {
    await page.keyboard.press("Escape");
    testInfo.annotations.push({ type: "activity", description: "no document-targeting row in the ledger right now" });
  }

  // 4. Feed tool rows: PL kind chips, no English literal, no dead subtitle.
  await spolka.getByRole("toolbar", { name: /Warsztat|Workshop/ }).getByRole("button", { name: /^Feed$/ }).click();
  const feedTool = page.getByRole("group", { name: /Workshop tool|Narzędzie warsztatu/ });
  await expect(feedTool).toBeVisible({ timeout: 15_000 });
  const feedText = await feedTool.innerText();
  expect(feedText).not.toContain("Official report");
  expect(feedText).not.toContain("Komunikat ESPI/EBI");
  await shoot(page, "feed-rows", testInfo);
});
