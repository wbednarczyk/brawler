import { test, expect, openApp, expectNoA11yViolations } from "./helpers/harness";

// Documents deep-link target contract (dogfooding #11, ADR 0107 amendment) —
// scoped to the Documents tool itself, complementing the Activity journey
// assertion in `activity.spec.ts`. `doc_cdr_q3_2025` is the one report
// document the browser smoke runtime seeds a real `{t:"dokumenty",
// documentId}` target for (`entities.ts` § makeActivityView) — reached via
// the Activity failed-reading row, the only live path to it (no query-param
// deep link exists in the app).
async function openTargetedDocuments(page: Parameters<typeof openApp>[0]) {
  await openApp(page);
  await page.getByRole("button", { name: "Open activity" }).click();
  const dialog = page.getByRole("dialog", { name: "Activity" });
  const failedRow = dialog.locator(".activity-item").filter({ hasText: "Raport roczny 2025 skrócony.pdf" });
  await failedRow.getByRole("button", { name: "Open in documents" }).click();
  await expect(dialog).toBeHidden();
  const tool = page.getByRole("group", { name: "Workshop tool" });
  await expect(tool).toHaveAttribute("data-tool", "dokumenty");
  return tool;
}

test.describe("Spółka › Documents — deep-link target contract (dogfooding #11)", { tag: "@journey" }, () => {
  // The browser smoke runtime seeds exactly one report document for CDR
  // (`browserSmokeRuntime.ts`, out of this slice's file list) — no sibling
  // row exists to diff against, so the proof is that the highlighted row
  // actually paints a background at all: a `.doc-row` carries none at rest
  // (only `:hover` and the highlighted rule do), so anything other than
  // fully transparent means the CSS rule matched (the original defect: the
  // DOM attribute was set but no CSS rule styled it).
  test("the targeted row carries aria-current and a real painted background, not just a DOM attribute", async ({
    page,
  }) => {
    const tool = await openTargetedDocuments(page);

    const targetRow = tool.locator('[data-document-id="doc_cdr_q3_2025"]');
    await expect(targetRow).toHaveAttribute("aria-current", "true");
    // eslint-disable-next-line no-restricted-syntax -- asserted TOGETHER with the painted background below, not instead of it (dogfooding #11)
    await expect(targetRow).toHaveAttribute("data-document-highlighted", "true");

    const targetColor = await targetRow.evaluate((el) => getComputedStyle(el).backgroundColor);
    expect(targetColor).not.toBe("rgba(0, 0, 0, 0)");

    // Exactly one row carries the deep-link target.
    await expect(tool.locator('[aria-current="true"]')).toHaveCount(1);
  });

  test("the target row is scrolled into view, not merely marked off-screen", async ({ page }) => {
    const tool = await openTargetedDocuments(page);
    const targetRow = tool.locator('[data-document-id="doc_cdr_q3_2025"]');

    await expect(targetRow).toBeVisible();
    const box = await targetRow.boundingBox();
    expect(box, "target row must have a real layout box").not.toBeNull();
    const viewport = page.viewportSize();
    expect(viewport).not.toBeNull();
    expect(box!.y).toBeGreaterThanOrEqual(0);
    expect(box!.y + box!.height).toBeLessThanOrEqual(viewport!.height);
  });

  test("focus lands on the tool heading, not the targeted row (F3c heading-focus contract)", async ({ page }) => {
    const tool = await openTargetedDocuments(page);

    await expect(tool.getByRole("heading", { level: 2 }).first()).toBeFocused();
    const targetRow = tool.locator('[data-document-id="doc_cdr_q3_2025"]');
    await expect(targetRow).not.toBeFocused();

    await expectNoA11yViolations(page, "Documents tool with a deep-link target");
  });
});
