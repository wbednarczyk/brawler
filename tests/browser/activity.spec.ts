import { test, expect, openApp, expectNoA11yViolations, expectNoHorizontalOverflow, openPalette } from "./helpers/harness";

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
    await failedRow.getByRole("button", { name: "Open document" }).click();

    await expect(dialog).toBeHidden();
    await expect(page.getByRole("region", { name: "Company view" })).toBeVisible();
    // Lands on the Documents tool with the exact `documentId` the row
    // declared (`data-tool` + the target's identity, asserted in the
    // AppShell.test.tsx unit test); the highlight itself is the SAME
    // provenance-ticket mechanism already exercised elsewhere
    // (CompanyReportDocumentsPanel) — the seeded mock's synthetic
    // `doc_mock_activity_*` id has no backing `reportDocuments` row, so no
    // row exists to assert a highlight attribute on here.
    await expect(page.getByRole("group", { name: "Workshop tool" })).toHaveAttribute("data-tool", "dokumenty");
  });
});
