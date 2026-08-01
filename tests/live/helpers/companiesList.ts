import type { Page } from "@playwright/test";

/**
 * Clears the tracked-companies search filter if it is present and non-empty.
 *
 * The live app is the owner's REAL, stateful instance: a filter typed during a
 * manual/agent-driven session persists in the UI, and a filtered list silently
 * hides the row a spec is about to click — three specs false-failed on exactly
 * this (epic #285 T11, 2026-08-01: a leftover "XTB" filter hid CBF). Every
 * spec that selects a company row calls this first, so the suite never
 * depends on the list being in its default state.
 */
export async function clearCompaniesFilter(page: Page): Promise<void> {
  const filter = page.getByPlaceholder(/Szukaj obserwowanych|Search tracked/);
  try {
    if (await filter.isVisible({ timeout: 2_000 })) {
      const value = await filter.inputValue();
      if (value !== "") {
        await filter.fill("");
        // Give the list a beat to re-render unfiltered.
        await page.waitForTimeout(300);
      }
    }
  } catch {
    // No filter field on this surface — nothing to clear.
  }
}
