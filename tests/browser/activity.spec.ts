import { test, expect, openApp, expectNoA11yViolations, expectNoHorizontalOverflow, openPalette } from "./helpers/harness";
import { expectActionInsideScroller } from "./helpers/interactionContracts";

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
    // Dogfooding #12: the destination names where it actually lands.
    await failedRow.getByRole("button", { name: "Open in documents" }).click();

    await expect(dialog).toBeHidden();
    await expect(page.getByRole("region", { name: "Company view" })).toBeVisible();
    // Lands on the Documents tool with the exact `documentId` the row
    // declared, and the document row stays marked — a persistent deep-link
    // target (dogfooding #11), not a 4s flash: it carries `aria-current` and
    // actually PAINTS a real background, not just a DOM attribute with no
    // matching CSS rule (the original defect — a `.doc-row` has no
    // background at rest, so a computed color other than fully transparent
    // proves the rule matched and painted). The browser smoke runtime seeds
    // exactly one report document for CDR (`browserSmokeRuntime.ts`, out of
    // this slice's file list), so there is no sibling row to diff against —
    // `spolka-documents.spec.ts` covers the same paint contract with the
    // same single-document constraint.
    await expect(page.getByRole("group", { name: "Workshop tool" })).toHaveAttribute("data-tool", "dokumenty");
    const targetRow = page.locator('[data-document-id="doc_cdr_q3_2025"]');
    await expect(targetRow).toHaveAttribute("data-document-highlighted", "true");
    await expect(targetRow).toHaveAttribute("aria-current", "true");
    const targetColor = await targetRow.evaluate((el) => getComputedStyle(el).backgroundColor);
    expect(targetColor).not.toBe("rgba(0, 0, 0, 0)");
  });

  // Dogfooding #10: `.activity-panel` used to be its OWN padding-less
  // scroller nested inside the padded `.ui-modal-body`; the fix makes
  // `.ui-modal-body` the sole scroll owner (`ui.css`, already padded +
  // `overflow-y: auto`) and `.activity-panel` a plain, unconstrained flex
  // column that grows with its content — so the SCROLLER under test is
  // `.ui-modal-body` now, not `.activity-panel`. entities.ts (the row seed)
  // is out of this slice's file list, so a shrunk viewport — not more seeded
  // rows — is the forcing mechanism here: a small enough window that even
  // the seeded 8 rows (2 active + 1 queued + 5 recent) overflow the dialog.
  test("every row's destination action stays inside the scroller when the dialog overflows", async ({ page }) => {
    await page.setViewportSize({ width: 1280, height: 420 });
    await openApp(page);
    await page.getByRole("button", { name: "Open activity" }).click();
    const dialog = page.getByRole("dialog", { name: "Activity" });
    const scroller = dialog.locator(".ui-modal-body");
    await expect(scroller).toBeVisible();
    await expect
      .poll(() => scroller.evaluate((el) => el.scrollHeight > el.clientHeight))
      .toBe(true);

    const destinations = dialog.locator('[data-action-kind="destination"]');
    const count = await destinations.count();
    expect(count).toBeGreaterThan(0);
    for (let i = 0; i < count; i += 1) {
      await expectActionInsideScroller(destinations.nth(i), scroller);
    }
  });
});
