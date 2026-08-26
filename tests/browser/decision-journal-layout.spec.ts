import { test, expect, openApp, expectNoPageOverflow } from "./helpers/harness";

// The decision-journal panel (ADR 0071, J3) renders variable, potentially
// unbreakable content — a long Markdown rationale and evidence rows (long ESPI
// filenames) inside a list∥detail grid. Guard that composing an entry and
// selecting it does not force a horizontal scrollbar at a narrow window (the
// quarter-ultrawide range, DoD §B). F3a S3 (ADR 0107): opening a company lands
// the Spółka screen; the journal is the `dziennik` workshop tool, opened via
// the ⌘K palette's "Open decision journal" entry. The dual-execution mock
// runtime serves the (command-only) journal + the company timeline as the
// evidence pool.
test("decision journal panel does not horizontally overflow at a narrow window", async ({
  page,
}) => {
  await page.setViewportSize({ width: 1008, height: 900 });
  await openApp(page);
  await page.getByLabel(/Primary navigation|Nawigacja główna/).getByRole("button", { name: "Companies" }).click();
  await page.locator('[data-company-id="company_gpw_cdr"] .company-row-main').click();
  await page.getByRole("region", { name: "Company view" }).waitFor();
  await page.keyboard.press("Control+K");
  const palette = page.getByRole("dialog", { name: "Command palette" });
  await palette.getByLabel("Search commands").fill("Open decision journal");
  await palette.getByRole("button", { name: "Open decision journal", exact: true }).first().click();

  // Open the composer.
  const journalPanel = page.locator(".decision-journal-panel");
  await journalPanel.getByRole("button", { name: "New entry" }).click();

  // A long, unbreakable rationale token is the overflow risk the grid containment
  // (min-width:0 + minmax(0,1fr)) must absorb.
  await journalPanel
    .getByLabel("Decision rationale")
    .fill(
      "Entering-a-position-after-the-Q3-report_cyber_Folks_SA_30.06.2026_raport_okresowy_skonsolidowany.pdf",
    );
  await journalPanel.getByRole("button", { name: "Save" }).click();

  // Saving auto-selects the new entry, so the detail + evidence rows render (at a
  // narrow pane the list folds behind the detail — the container-query collapse).
  await expect(journalPanel.getByLabel("Decision entry detail")).toBeVisible();

  await expectNoPageOverflow(page);
});
