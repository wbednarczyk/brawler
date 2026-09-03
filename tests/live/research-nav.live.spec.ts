import { test, expect } from "@playwright/test";
import { connectToLiveApp, type LiveConnection } from "./helpers/liveConnect";

// Live proof (ADR 0066) for the Spółka company surface (ADR 0107/0108): the
// left-nav "Company" mode entry opens the ONE company-scoped Spółka screen —
// never blank — against the owner's real database. Deliberately
// non-destructive: navigation + selector reads only, no create/edit/delete.

let connection: LiveConnection;

test.beforeAll(async () => {
  connection = await connectToLiveApp();
});

test.afterAll(async () => {
  await connection.browser.close();
});

test("the Company mode nav entry opens the Spółka screen scoped to a company on the real app", async () => {
  const { page } = connection;

  const nav = page.getByLabel(/Primary navigation|Nawigacja główna/);
  await expect(nav).toBeVisible();

  // ADR 0107 amendment / ADR 0108: the sidebar "Dashboard" mode and the
  // cockpit it used to open are retired outright — the Modes "Company" nav
  // item opens the Spółka screen scoped to a company directly, never blank.
  const entry = nav.getByRole("button", { name: /^(Company|Spółka)$/ }).first();
  await expect(entry).toBeVisible();
  await page.screenshot({ path: "test-results/live/dashboard-nav-entry.png", fullPage: true });

  await entry.click();
  const spolka = page.getByRole("region", { name: /Widok spółki|Company view/ });
  await expect(spolka).toBeVisible();

  // Company-scoped, never blank: the glance bar (always-visible core, never a
  // hosted tool) is present, and no cockpit-era "Add panel" control exists.
  await expect(spolka).not.toHaveAttribute("data-company-id", "");
  await expect(page.getByRole("group", { name: /Pasek informacyjny spółki|Company glance bar/ })).toBeVisible();
  await expect(page.getByRole("button", { name: /Add panel|Dodaj panel/ })).toHaveCount(0);
  await page.screenshot({ path: "test-results/live/dashboard-nav-spolka.png", fullPage: true });

  // Research is a Library nav destination now (F4c S3, contract § Decisions
  // #4) — reached from the sidebar, one click, no palette round-trip.
  await nav.getByRole("button", { name: "Research" }).click();
  await expect(page.locator(".research-panel")).toBeVisible();
  await page.screenshot({ path: "test-results/live/research-screen.png", fullPage: true });

  // The palette path stays reachable too (command-palette.spec.ts covers it
  // in the browser-mock suite; this is the one live-app case).
  await page.keyboard.press("Control+K");
  const palette = page.getByRole("dialog", { name: /Command palette|Paleta poleceń/ });
  await palette.getByLabel(/Search commands|Szukaj poleceń/).fill("Research");
  await expect(
    palette.getByRole("option", { name: /^(Open screen|Otwórz ekran): Research/ }).first(),
  ).toBeVisible();
  await page.keyboard.press("Escape");
});
