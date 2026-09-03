import type { Locator, Page } from "@playwright/test";
import { test, expect, openApp, openPalette, expectNoA11yViolations } from "./helpers/harness";
import { WORKSHOP_TOOLS } from "../../src/screens/Spolka/route";

// F3c (#197, contract docs/plans/frontend-v2-f3c.md § 8 done-well 2/3/5):
// every workshop entry and every palette screen/tool command is reachable by
// keyboard; the hosted tools named by the issue (Research, Documents, Events)
// hold no keyboard trap; axe's focus-order rules stay clean on Spółka at rest,
// with a tool open, and with the palette open.

// The palette's tool commands (`SPOLKA_TOOL_COMMANDS`, SpolkaScreen.tsx) are
// `Open <entry label, lower-cased>` plus `Open overview`; derived here rather
// than imported — a browser spec must not pull the screen module (and its
// `import.meta.env` chart) into the tests tsconfig.
const SPOLKA_TOOL_COMMANDS = [
  { tool: null as { t: string } | null, label: "Open overview" },
  ...WORKSHOP_TOOLS.map(({ tool, label }) => ({ tool: tool as { t: string } | null, label: `Open ${label.toLowerCase()}` })),
];

const TABBABLE =
  'a[href], button:not([disabled]), input:not([disabled]), select:not([disabled]), textarea:not([disabled]), [tabindex]:not([tabindex="-1"])';

async function openCdrSpolka(page: Page): Promise<Locator> {
  await openApp(page);
  const palette = await openPalette(page);
  await palette.getByRole("combobox", { name: "Search commands" }).fill("Open company: CDR");
  await page.keyboard.press("Enter");
  const spolka = page.getByRole("region", { name: "Company view", exact: true });
  await expect(spolka).toBeVisible();
  return spolka;
}

// Type the label, then ArrowDown until the EXACT option is active — the
// filter is a substring match, so `Open research` (tool) also lists
// `Open Research` (the Ctrl+4 screen shortcut) first.
async function runPaletteCommand(page: Page, label: string) {
  const palette = await openPalette(page);
  await palette.getByRole("combobox", { name: "Search commands" }).fill(label);
  const option = palette.getByRole("option", { name: label, exact: true });
  await expect(option).toBeVisible();
  for (let i = 0; i < 10 && (await option.getAttribute("aria-selected")) !== "true"; i += 1) {
    await page.keyboard.press("ArrowDown");
  }
  await expect(option).toHaveAttribute("aria-selected", "true");
  await page.keyboard.press("Enter");
  await expect(palette).toBeHidden();
}

async function activeElementKey(page: Page): Promise<string> {
  return page.evaluate(() => {
    const el = document.activeElement as HTMLElement | null;
    if (!el || el === document.body) return "body";
    if (!el.dataset.kbdWalkId) el.dataset.kbdWalkId = String(Math.random());
    return el.dataset.kbdWalkId;
  });
}

test.describe("Spółka keyboard reachability", { tag: "@journey" }, () => {
  test("Ctrl+. → arrows → Enter opens every workshop entry; Escape returns to its entry", async ({ page }) => {
    const spolka = await openCdrSpolka(page);
    const bar = spolka.getByRole("toolbar", { name: "Workshop" });
    const tool = spolka.getByRole("group", { name: "Workshop tool" });

    for (const [index, { tool: target, label }] of WORKSHOP_TOOLS.entries()) {
      await page.keyboard.press("Control+.");
      await page.keyboard.press("Home");
      await expect(bar.getByRole("button", { name: "Overview" })).toBeFocused();
      for (let i = 0; i <= index; i += 1) await page.keyboard.press("ArrowRight");
      await expect(bar.getByRole("button", { name: label, exact: true })).toBeFocused();
      await page.keyboard.press("Enter");
      await expect(tool).toHaveAttribute("data-tool", target.t);
      await expect(tool.getByRole("heading", { level: 2 }).first()).toBeFocused();
      await page.keyboard.press("Escape");
      await expect(tool).toBeHidden();
      await expect(bar.getByRole("button", { name: label, exact: true })).toBeFocused();
    }

    // The 15th entry — Overview — activated by keyboard closes the open tool
    // and keeps focus on itself.
    await page.keyboard.press("Control+.");
    await page.keyboard.press("End");
    await page.keyboard.press("Enter");
    await expect(tool).toHaveAttribute("data-tool", "wydarzenia");
    await page.keyboard.press("Control+.");
    await page.keyboard.press("Home");
    await expect(bar.getByRole("button", { name: "Overview" })).toBeFocused();
    await page.keyboard.press("Enter");
    await expect(tool).toBeHidden();
    await expect(bar.getByRole("button", { name: "Overview" })).toBeFocused();
    await expect(bar.getByRole("button", { name: "Overview" })).toHaveAttribute("aria-pressed", "true");
  });

  test("every palette tool command and screen command runs from the keyboard", async ({ page }) => {
    const spolka = await openCdrSpolka(page);
    const tool = spolka.getByRole("group", { name: "Workshop tool" });

    for (const { tool: target, label } of SPOLKA_TOOL_COMMANDS) {
      await runPaletteCommand(page, label);
      if (target) await expect(tool).toHaveAttribute("data-tool", target.t);
      else await expect(tool).toBeHidden();
    }

    for (const { label, heading } of [
      { label: "Open screen: Today", heading: "Today" },
      { label: "Open screen: Events", heading: "Events" },
      { label: "Open screen: Report Season", heading: "Report Season" },
      { label: "Open screen: Research", heading: "Research" },
    ]) {
      await runPaletteCommand(page, label);
      await expect(page.getByRole("heading", { name: heading, exact: true }).first()).toBeVisible();
    }
  });

  for (const kind of ["research", "dokumenty", "wydarzenia"] as const) {
    test(`hosted ${kind} tool has no keyboard trap (Tab and Shift+Tab leave it)`, async ({ page }) => {
      const spolka = await openCdrSpolka(page);
      const bar = spolka.getByRole("toolbar", { name: "Workshop" });
      const tool = spolka.getByRole("group", { name: "Workshop tool" });
      const label = WORKSHOP_TOOLS.find((entry) => entry.tool.t === kind)!.label;

      await runPaletteCommand(page, SPOLKA_TOOL_COMMANDS.find((c) => c.tool?.t === kind)!.label);
      await expect(tool).toHaveAttribute("data-tool", kind);
      await expect(tool.getByRole("heading", { level: 2 }).first()).toBeFocused();

      // Forward: bounded by the frame's own tabbables + 1; a repeated focus
      // target before the toolbar is a trap.
      const frameTabbables = await tool.locator(TABBABLE).count();
      const seen = new Set<string>();
      let reached = false;
      for (let i = 0; i <= frameTabbables + 1; i += 1) {
        await page.keyboard.press("Tab");
        if (await bar.getByRole("button", { name: label, exact: true }).evaluate((el) => el === document.activeElement)) {
          reached = true;
          break;
        }
        const key = await activeElementKey(page);
        expect(seen.has(key), `Tab cycled back to an element inside the ${kind} tool after ${i + 1} presses`).toBe(false);
        seen.add(key);
      }
      expect(reached, `Tab never reached the workshop bar within ${frameTabbables + 2} presses`).toBe(true);

      // Backward: from the heading, bounded by every document tabbable
      // (header, shell, sidebar sit between the heading and the wrapped-to
      // toolbar).
      await tool.getByRole("heading", { level: 2 }).first().focus();
      const documentTabbables = await page.locator(TABBABLE).count();
      seen.clear();
      reached = false;
      for (let i = 0; i <= documentTabbables + 1; i += 1) {
        await page.keyboard.press("Shift+Tab");
        if (await bar.evaluate((el) => el.contains(document.activeElement))) {
          reached = true;
          break;
        }
        const key = await activeElementKey(page);
        expect(seen.has(key), `Shift+Tab cycled without leaving the ${kind} tool after ${i + 1} presses`).toBe(false);
        seen.add(key);
      }
      expect(reached, `Shift+Tab never reached the workshop bar within ${documentTabbables + 2} presses`).toBe(true);
    });
  }

  test("axe focus-order rules: Spółka at rest, with a tool open, with the palette open", async ({ page }) => {
    const spolka = await openCdrSpolka(page);
    const extraRules = ["focus-order-semantics", "tabindex"];
    await expectNoA11yViolations(page, "Spółka at rest", { extraRules });

    await page.keyboard.press("Control+.");
    await page.keyboard.press("End");
    await page.keyboard.press("Enter");
    await expect(spolka.getByRole("group", { name: "Workshop tool" })).toHaveAttribute("data-tool", "wydarzenia");
    await expectNoA11yViolations(page, "Spółka with the Events tool open", { extraRules });

    await openPalette(page);
    await expectNoA11yViolations(page, "Spółka with the palette open", { extraRules });
  });
});
