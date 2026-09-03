import { test, expect, openApp } from "./helpers/harness";
import type { Page } from "@playwright/test";

// Structured-first provenance UI (ADR 0061). F3a S3 (ADR 0107): opening a
// company lands the Spółka screen; Fundamentals is the `fundamenty` workshop
// tool, opened via the ⌘K palette's "Open fundamentals" entry — each fact the
// deterministic pipeline produced carries a source-tier + validation badge,
// and a fact whose layout drifted from the confirmed company profile shows a
// clean "structure changed" label diff. The browser mock runtime
// (browserSmokeRuntime factProvenance) seeds Revenue Q3 as a clean, validated
// ESEF fact and Net profit Q3 as a flagged PDF parse carrying drift — so this
// asserts both badge states and the diff card render end to end (the gap a
// Vitest-mocked panel cannot catch).

async function openFundamentals(page: Page) {
  await openApp(page);
  await page.getByLabel("Primary navigation").getByRole("button", { name: "Companies" }).click();
  await page.locator('[data-company-id="company_gpw_cdr"] .company-row-main').click();
  await page.getByRole("region", { name: "Company view" }).waitFor();
  await page.keyboard.press("Control+K");
  const palette = page.getByRole("dialog", { name: "Command palette" });
  await palette.getByLabel("Search commands").fill("Open fundamentals");
  await palette.getByRole("option", { name: "Open fundamentals", exact: true }).first().click();
  await expect(page.getByLabel("Company fundamentals")).toBeVisible();
}

test.describe("structured-first fundamentals provenance (ADR 0061)", () => {
  test("a validated ESEF fact shows its source-tier + validation badges @clickable", async ({
    page,
  }) => {
    await openFundamentals(page);
    await page.getByRole("button", { name: "Revenue, 2025 Q3" }).click();

    const detail = page.getByLabel("Financial fact detail");
    await expect(detail).toBeVisible();
    // Tier = the tagged source of truth; validation = passed the "good" gate.
    await expect(detail.getByText("ESEF (tagged)")).toBeVisible();
    await expect(detail.getByText("Validated", { exact: true })).toBeVisible();
    // A clean fact does NOT render the drift card.
    await expect(page.getByRole("group", { name: "Structure changed" })).toHaveCount(0);
  });

  test("a drifted fact shows the 'structure changed' label diff card @clickable", async ({
    page,
  }) => {
    await openFundamentals(page);
    await page.getByRole("button", { name: "Net profit, 2025 Q3" }).click();

    const drift = page.getByRole("group", { name: "Structure changed" });
    await expect(drift).toBeVisible();
    // The clean label diff: the new line added, the profile line removed.
    await expect(drift.getByText("Zysk netto z działalności kontynuowanej")).toBeVisible();
    await expect(drift.getByText("Zysk netto", { exact: true })).toBeVisible();
  });
});
