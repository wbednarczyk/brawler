import { captureMarkStyle, expectVisiblyMarked } from "./helpers/paint";
import { test, expect, openApp, openPalette, expectNoA11yViolations } from "./helpers/harness";

// Documents deep-link target contract (dogfooding #11, ADR 0107 amendment) —
// scoped to the Documents tool itself, complementing the Activity journey
// assertion in `activity.spec.ts`. `doc_cdr_q3_2025` is the one report
// document `entities.ts` § makeActivityView targets with a real
// `{t:"dokumenty", documentId}` — reached via the Activity failed-reading
// row, the only live path to it (no query-param deep link exists in the
// app). `doc_cdr_q2_2025` (#476) is its neutral, never-marked sibling, seeded
// alongside it in `browserSmokeRuntime.ts`.


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
  // #476 sol R2 correction: the strongest proof is the SAME element's paint
  // before vs after it becomes the deep-link target — open Documents on CDR
  // directly first (no target yet), capture that row's paint, THEN trigger
  // the Activity deep link and re-capture the same row. The sibling-differs
  // check (`doc_cdr_q2_2025`) stays as a second, independent signal, not the
  // primary proof.
  test("the targeted row carries aria-current and paints differently once marked, proven on the same element", async ({
    page,
  }) => {
    await openApp(page);

    const openCompany = await openPalette(page);
    await openCompany.getByRole("combobox", { name: "Search commands" }).fill("Open company: CDR");
    await page.keyboard.press("Enter");
    await expect(page.getByRole("region", { name: "Company view", exact: true })).toBeVisible();
    const openDocuments = await openPalette(page);
    await openDocuments.getByRole("combobox", { name: "Search commands" }).fill("Open tool: Documents");
    await page.keyboard.press("Enter");
    const tool = page.getByRole("group", { name: "Workshop tool" });
    await expect(tool).toHaveAttribute("data-tool", "dokumenty");

    const targetRow = tool.locator('[data-document-id="doc_cdr_q3_2025"]');
    const siblingRow = tool.locator('[data-document-id="doc_cdr_q2_2025"]');
    await expect(targetRow).toBeVisible();
    const prePaint = await captureMarkStyle(targetRow);
    // The proof is on the SAME node: keep its handle and re-verify it after the deep link.
    const targetHandle = await targetRow.elementHandle();

    await page.getByRole("button", { name: "Open activity" }).click();
    const dialog = page.getByRole("dialog", { name: "Activity" });
    const failedRow = dialog.locator(".activity-item").filter({ hasText: "Raport roczny 2025 skrócony.pdf" });
    await failedRow.getByRole("button", { name: "Open in documents" }).click();
    await expect(dialog).toBeHidden();
    await expect(tool).toHaveAttribute("data-tool", "dokumenty");

    await expect(targetRow).toHaveAttribute("aria-current", "true");
    await expect(siblingRow).not.toHaveAttribute("aria-current", "true");

    expect(await targetHandle!.evaluate((el) => el.isConnected), "target row survived the navigation").toBe(true);
    expect(await targetRow.evaluate((el, handle) => el === handle, targetHandle), "locator resolves to the original node").toBe(true);
    await expectVisiblyMarked(targetRow, prePaint);
    const postPaint = await captureMarkStyle(targetRow);

    // Extra (#476): the neutral sibling also stays visibly different from
    // the now-marked target.
    const siblingPaint = await captureMarkStyle(siblingRow);
    expect(JSON.stringify(siblingPaint), "the unmarked sibling paints differently").not.toBe(JSON.stringify(postPaint));

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
