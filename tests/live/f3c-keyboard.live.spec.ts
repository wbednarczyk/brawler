import { test, expect } from "@playwright/test";
import { connectToLiveApp, type LiveConnection } from "./helpers/liveConnect";

// F3c real-data check (DoD § G, docs/plans/frontend-v2-f3c.md § Verification):
// on the owner's real Windows app (PL locale, real DB) — the Spółka screen is
// walked without the mouse: palette → company, Ctrl+. → arrows → Enter,
// Escape back, H/L, Shift+J, Ctrl+K listbox. Screenshots capture the focus
// ring in the owner's theme; assertions stay structural.

let connection: LiveConnection;

test.beforeAll(async () => {
  connection = await connectToLiveApp();
});

test.afterAll(async () => {
  await connection.browser.close();
});

const SHOTS = "test-results/live-f3c";

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

async function activeDescription(page: LiveConnection["page"]) {
  return page.evaluate(() => {
    const el = document.activeElement as HTMLElement | null;
    if (!el || el === document.body) return "body";
    return `${el.tagName.toLowerCase()} "${(el.getAttribute("aria-label") ?? el.textContent ?? "").trim().slice(0, 40)}"`;
  });
}

test("keyboard-only company review on the real app", async ({}, testInfo) => {
  const { page } = connection;
  test.setTimeout(180_000);

  // Palette → first tracked company, typed by keyboard.
  await page.keyboard.press("Escape");
  await page.evaluate(() => (document.activeElement as HTMLElement | null)?.blur?.());
  await page.keyboard.press("Control+K");
  const palette = page.getByRole("dialog", { name: /Command palette|Paleta poleceń/ });
  await expect(palette).toBeVisible({ timeout: 10_000 });
  const combobox = palette.getByRole("combobox");
  await expect(combobox).toBeFocused();
  await combobox.pressSequentially("spółk");
  await expect(palette.getByRole("option").first()).toHaveAttribute("aria-selected", "true");
  await shoot(page, "palette-listbox", testInfo);
  await page.keyboard.press("Enter");
  const spolka = page.getByRole("region", { name: /Widok spółki|Company view/ });
  await expect(spolka).toBeVisible({ timeout: 20_000 });
  expect(await activeDescription(page)).not.toBe("body");

  // Ctrl+. → toolbar tab stop → arrows → Enter → heading focused.
  await page.keyboard.press("Control+.");
  const bar = spolka.getByRole("toolbar");
  await expect(bar.locator('[tabindex="0"]')).toBeFocused();
  await shoot(page, "toolbar-focused", testInfo);
  await page.keyboard.press("ArrowRight");
  await page.keyboard.press("ArrowRight");
  await page.keyboard.press("Enter");
  const tool = spolka.getByRole("group", { name: /Narzędzie warsztatu|Workshop tool/ });
  await expect(tool).toBeVisible({ timeout: 20_000 });
  await expect(tool.getByRole("heading", { level: 2 }).first()).toBeFocused();
  await shoot(page, "tool-heading-focused", testInfo);

  // L → next tool, heading again; Escape → Overview with focus on the entry.
  await page.keyboard.press("l");
  await expect(tool.getByRole("heading", { level: 2 }).first()).toBeFocused({ timeout: 20_000 });
  const closedKind = await tool.getAttribute("data-tool");
  await page.keyboard.press("Escape");
  await expect(tool).toBeHidden({ timeout: 10_000 });
  expect(await bar.evaluate((el) => el.contains(document.activeElement))).toBe(true);
  await shoot(page, `escape-back-to-overview-from-${closedKind}`, testInfo);

  // Shift+J → adjacent company, picker focused.
  const before = await spolka.getAttribute("data-company-id");
  await page.keyboard.press("Shift+J");
  await expect.poll(() => page.getByRole("region", { name: /Widok spółki|Company view/ }).getAttribute("data-company-id"), { timeout: 20_000 }).not.toBe(before);
  expect(await activeDescription(page)).toMatch(/^select/);
  await shoot(page, "shift-j-company-picker-focused", testInfo);

  // Ctrl+K works from the focused picker; Escape returns focus to it.
  await page.keyboard.press("Control+K");
  await expect(palette).toBeVisible({ timeout: 10_000 });
  await page.keyboard.press("Escape");
  await expect(palette).toBeHidden();
  expect(await activeDescription(page)).toMatch(/^select/);
});
