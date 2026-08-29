import { test, expect, openApp, expectNoA11yViolations } from "./helpers/harness";
import { expectPrimaryActionCount } from "./helpers/interactionContracts";
import type { Locator, Page } from "@playwright/test";

// Alerts — the changed-workflow first red journey test (F4a S4b, contract §
// Alerts "First red journey test"): a fired alert row must be reachable and
// must land on its target surface, and a created rule must survive a
// re-visit through the stateful mock runtime (ADR 0048). The browser smoke
// runtime boots the "rich" scenario by default (`browserSmokeRuntime.ts`),
// which already seeds one watchlist-scoped rule + one company-scoped fired
// event (`companies[0]`) — no extra scenario seed needed for either case.

function nav(page: Page) {
  return page.getByLabel(/Primary navigation|Nawigacja główna/);
}

// At the narrow (`chromium-quarter-uw`) project the composer folds behind its
// own `Add alert` disclosure control (contract § Alerts § 5/10, S tier) — the
// trigger chips only exist once it is open. Wide projects render the
// composer open already, so this is a no-op there.
async function ensureComposerOpen(region: Locator) {
  const chip = region.getByRole("button", { name: "Insider transactions" });
  if (!(await chip.isVisible().catch(() => false))) {
    await region.getByRole("button", { name: "Add alert" }).click();
  }
}

test.describe("Alerts — changed workflow", { tag: "@clickable" }, () => {
  test("a new rule persists and renders", async ({ page }) => {
    await openApp(page);
    await nav(page).getByRole("button", { name: "Alerts" }).click();
    const region = page.getByRole("region", { name: "Alerts" });

    await ensureComposerOpen(region);
    await region.getByRole("button", { name: "Insider transactions" }).click();
    await region.getByRole("button", { name: "Add alert" }).click();

    const newRow = region.getByRole("listitem", { name: /alert rule/i }).filter({ hasText: "Insider transaction" });
    await expect(newRow).toBeVisible();

    // Navigate away and back — the stateful mock runtime (ADR 0048) persists
    // the rule, so it re-renders as saved rather than resetting.
    await nav(page).getByRole("button", { name: "Today" }).click();
    await nav(page).getByRole("button", { name: "Alerts" }).click();
    await expect(
      page.getByRole("region", { name: "Alerts" }).getByRole("listitem", { name: /alert rule/i }).filter({ hasText: "Insider transaction" }),
    ).toBeVisible();

    await expectPrimaryActionCount(page.getByRole("region", { name: "Alerts" }), { max: 1 });
    await expectNoA11yViolations(page, "Alerts — after creating a rule");
  });

  test("a seeded fired alert row opens its target surface", async ({ page }) => {
    await openApp(page);
    await nav(page).getByRole("button", { name: "Alerts" }).click();
    const region = page.getByRole("region", { name: "Alerts" });

    const firedRow = region.getByRole("listitem", { name: /fired alert/i }).first();
    await expect(firedRow).toBeVisible();
    await expectPrimaryActionCount(region, { max: 1 });

    await firedRow.getByRole("button", { name: "Open company" }).click();
    await expect(page.getByRole("region", { name: "Company view" })).toBeVisible();

    await expectNoA11yViolations(page, "Spółka — reached from a fired alert row");
  });
});
