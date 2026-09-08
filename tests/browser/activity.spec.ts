import { test, expect, openApp, expectNoA11yViolations, expectNoHorizontalOverflow, openPalette } from "./helpers/harness";
import { captureMarkStyle, expectVisiblyMarked } from "./helpers/paint";

// Activity center (ADR 0109, #133) — the first red journey test (plan § 1):
// seeded active + queued + a failed reading + a sweep parent via the
// "rich" scenario `browserSmokeRuntime.ts` boots by default. Opens from the
// topbar and from the palette; "Otwórz dokument" on the failed reading row
// lands on Spółka › Dokumenty with the document highlighted; no horizontal
// overflow inside the dialog; axe clean with the dialog open.

test.describe("Activity panel — journey-independent utility", { tag: "@clickable" }, () => {
  test("opens from the topbar, no overflow inside the dialog, axe clean", async ({ page }) => {
    await openApp(page);

    const indicator = page.getByRole("button", { name: "Open activity" });
    await expect(indicator).toBeVisible();
    await indicator.click();

    const dialog = page.getByRole("dialog", { name: "Activity" });
    await expect(dialog).toBeVisible();
    await expect(dialog.locator(".activity-item, [data-empty-kind]").first()).toBeVisible();

    await expectNoHorizontalOverflow(dialog);
    await expectNoHorizontalOverflow(dialog.locator(".activity-panel"));
    await expectNoA11yViolations(page, "Activity panel open");

    await page.keyboard.press("Escape");
    await expect(dialog).toBeHidden();
    await expect(indicator).toBeFocused();
  });

  test("opens from the palette (Ctrl+K → Otwórz aktywność / Open activity)", async ({ page }) => {
    await openApp(page);
    const palette = await openPalette(page);
    await palette.getByLabel("Search commands").fill("Open activity");
    await palette.getByRole("option", { name: "Open activity", exact: true }).first().click();

    await expect(page.getByRole("dialog", { name: "Activity" })).toBeVisible();
  });

  test("Otwórz dokument on the failed reading row lands on Spółka › Dokumenty with the document highlighted", async ({ page }) => {
    await openApp(page);
    await page.getByRole("button", { name: "Open activity" }).click();
    const dialog = page.getByRole("dialog", { name: "Activity" });
    await expect(dialog).toBeVisible();

    const failedRow = dialog.locator(".activity-item").filter({ hasText: "Raport roczny 2025 skrócony.pdf" });
    await expect(failedRow).toBeVisible();
    // sol diff R2 finding 8: the row declares its target precisely enough
    // for the live harness to tell Coverage/Overview/Documents apart and
    // catch a wrong document ID, not just "some company view opened".
    await expect(failedRow).toHaveAttribute("data-activity-target", "company");
    await expect(failedRow).toHaveAttribute("data-activity-tool", "dokumenty");
    await expect(failedRow).toHaveAttribute("data-activity-document", "doc_cdr_q3_2025");
    await failedRow.getByRole("button", { name: "Open document" }).click();

    await expect(dialog).toBeHidden();
    await expect(page.getByRole("region", { name: "Company view" })).toBeVisible();
    await expect(page.getByRole("group", { name: "Workshop tool" })).toHaveAttribute("data-tool", "dokumenty");

    const documentRow = page.locator('[data-document-id="doc_cdr_q3_2025"]');
    // Lands on the Documents tool with the exact `documentId` the row
    // declared and the document row flashed (`data-document-highlighted`,
    // CompanyReportDocumentsPanel — the seeded reading targets the smoke
    // runtime's `doc_cdr_q3_2025`). Live today (not the paint it stands in
    // for): the attribute IS set, even though no CSS rule currently paints it.
    // eslint-disable-next-line no-restricted-syntax -- interim until #476 lands the paint (dogfooding #11)
    await expect(documentRow).toHaveAttribute("data-document-highlighted", "true");

    // G11 (dogfooding #11): `data-document-highlighted` is set for 4s with NO
    // CSS rule painting it (verified: no `document-highlighted` selector
    // anywhere in src/styles) — the row never visibly changes even though the
    // attribute is present. The browser-smoke seed also provides only this
    // one report document (entities.ts: "the one report document the browser
    // smoke runtime seeds"), so a before/after paint comparison has no window
    // to observe a pre-highlight baseline here regardless. Fixme, not skip:
    // the assertion is correct and reddens on the real bug; a product CSS fix
    // (out of scope here — no product CSS added per G11 instructions) plus a
    // second seeded document turn it green.
    test.fixme(
      true,
      "dogfooding #11: data-document-highlighted has no CSS paint rule, and the smoke seed has no second document to compare against — see docs/testing.md hygiene gate; tracked for a follow-up product/test-data fix",
    );
    const beforeMark = await captureMarkStyle(documentRow);
    await expectVisiblyMarked(documentRow, beforeMark);
  });
});
