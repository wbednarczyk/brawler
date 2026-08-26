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

test("a Views-group legacy dashboard row opens the frozen company-scoped cockpit on the real app", async () => {
  const { page } = connection;

  const nav = page.getByLabel(/Primary navigation|Nawigacja główna/);
  await expect(nav).toBeVisible();

  // F3a S3 (ADR 0107 dec. 5): the sidebar "Dashboard" mode is gone — the four
  // saved `dashboard:*` layouts survive read-only as "Legacy dashboard · TICKER"
  // rows in the Views group.
  const entry = nav.getByRole("button", { name: /^(Legacy dashboard|Dawny dashboard)/ }).first();
  await expect(entry).toBeVisible();
  await page.screenshot({ path: "test-results/live/dashboard-nav-entry.png", fullPage: true });

  await entry.click();
  const cockpit = page.getByLabel(/Research cockpit|Kokpit badawczy/);
  await expect(cockpit).toBeVisible();

  // Company-scoped, structure frozen: the view company is set and the frozen
  // strip is shown; no preset/add-panel controls exist any more.
  await expect(cockpit).not.toHaveAttribute("data-company-id", "");
  await expect(cockpit.getByText(/Layout frozen|Układ zamrożony/)).toBeVisible();
  await expect(cockpit.getByRole("button", { name: /Add panel|Dodaj panel/ })).toHaveCount(0);
  await page.screenshot({ path: "test-results/live/dashboard-nav-cockpit.png", fullPage: true });

  // Research is a standalone screen now — reached from the ⌘K palette.
  await page.keyboard.press("Control+K");
  const palette = page.getByRole("dialog", { name: /Command palette|Paleta poleceń/ });
  await palette.getByLabel(/Search commands|Szukaj poleceń/).fill("Research");
  await palette.getByRole("button", { name: /^(Open screen|Otwórz ekran): Research/ }).first().click();
  await expect(page.locator(".research-panel")).toBeVisible();
  await page.screenshot({ path: "test-results/live/research-screen.png", fullPage: true });
});
