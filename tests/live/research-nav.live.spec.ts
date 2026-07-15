import { test, expect } from "@playwright/test";
import { connectToLiveApp, type LiveConnection } from "./helpers/liveConnect";

// Live proof (ADR 0066) for the Dashboard redesign (epic c793ca1): the left-nav
// "Dashboard" entry opens the ONE company-scoped Dashboard — never blank —
// carrying a view-company + preset selector, against the owner's real database.
// Deliberately non-destructive: navigation + selector reads only, no
// create/edit/delete.

let connection: LiveConnection;

test.beforeAll(async () => {
  connection = await connectToLiveApp();
});

test.afterAll(async () => {
  await connection.browser.close();
});

test("Dashboard nav opens the company-scoped Dashboard on the real app", async () => {
  const { page } = connection;

  const nav = page.getByLabel(/Primary navigation|Nawigacja główna/);
  await expect(nav).toBeVisible();

  const entry = nav.getByRole("button", { name: "Dashboard" });
  await expect(entry).toBeVisible();
  await page.screenshot({ path: "test-results/live/dashboard-nav-entry.png", fullPage: true });

  await entry.click();
  const cockpit = page.getByLabel(/Research cockpit|Kokpit badawczy/);
  await expect(cockpit).toBeVisible();

  // Company-scoped, never blank: the view company is set and the preset selector
  // is present (the two Dashboard selectors).
  await expect(cockpit).not.toHaveAttribute("data-company-id", "");
  await expect(cockpit.getByLabel(/View company|Spółka widoku/)).toBeVisible();
  await expect(cockpit.getByLabel(/Preset/)).toBeVisible();
  await page.screenshot({ path: "test-results/live/dashboard-nav-cockpit.png", fullPage: true });

  // Switch to the Evidence / Research preset — the retired standalone Research
  // screen's new home — and confirm the research evidence panel renders.
  await cockpit.getByLabel(/Preset/).selectOption("evidence");
  await expect(cockpit.locator(".research-panel")).toBeVisible();
  await page.screenshot({ path: "test-results/live/dashboard-evidence-preset.png", fullPage: true });
});
