import { test, expect } from "@playwright/test";
import { connectToLiveApp, type LiveConnection } from "./helpers/liveConnect";

// F3d real-data check (DoD § G, docs/plans/frontend-v2-f3d.md § 12): on the
// owner's real Windows app (PL locale, real DB) the Activity panel opens from
// the top bar, reads honestly (an empty ledger is a valid state), and every
// row's destination lands where the row says. Time-dependent content is never
// asserted — the spec reads each row's declared target and verifies THAT.

let connection: LiveConnection;

test.beforeAll(async () => {
  connection = await connectToLiveApp();
});

test.afterAll(async () => {
  await connection.browser.close();
});

const SHOTS = "test-results/live-f3d";

async function shoot(
  page: LiveConnection["page"],
  name: string,
  testInfo: { attach: (n: string, o: { body: Buffer; contentType: string }) => Promise<unknown> },
) {
  const body = await page.screenshot({ fullPage: false });
  await testInfo.attach(name, { body, contentType: "image/png" });
  const fs = await import("node:fs/promises");
  await fs.mkdir(SHOTS, { recursive: true });
  await fs.writeFile(`${SHOTS}/${name}.png`, body);
}

test("activity panel on the real app: open, read, land on a declared destination", async ({}, testInfo) => {
  const { page } = connection;
  test.setTimeout(180_000);

  await page.keyboard.press("Escape");
  const indicator = page.getByRole("button", { name: /Otwórz aktywność|Open activity/ });
  await expect(indicator).toBeVisible({ timeout: 10_000 });
  await shoot(page, "topbar-indicator", testInfo);

  await indicator.click();
  const dialog = page.getByRole("dialog", { name: /^(Aktywność|Activity)$/ });
  await expect(dialog).toBeVisible({ timeout: 10_000 });
  // Wait for hydration: either rows or the quiet empty state, never a skeleton.
  await expect(dialog.locator("[data-empty-kind='quiet'], .activity-item").first()).toBeVisible({ timeout: 20_000 });
  await shoot(page, "panel-open", testInfo);

  // No horizontal scroll inside the dialog on the owner's real window size.
  const overflow = await dialog.evaluate((el) => ({
    dialog: el.scrollWidth - el.clientWidth,
    body: (() => {
      const body = el.querySelector(".activity-panel") as HTMLElement | null;
      return body ? body.scrollWidth - body.clientWidth : 0;
    })(),
  }));
  expect(overflow.dialog).toBeLessThanOrEqual(1);
  expect(overflow.body).toBeLessThanOrEqual(1);

  const rows = dialog.locator(".activity-item");
  const rowCount = await rows.count();
  testInfo.annotations.push({ type: "activity-rows", description: String(rowCount) });

  if (rowCount === 0) {
    // Honest quiet state: the ledger is empty on this database right now.
    await expect(dialog.locator("[data-empty-kind='quiet']")).toBeVisible();
    await page.keyboard.press("Escape");
    await expect(dialog).toBeHidden();
    await expect(indicator).toBeFocused();
    return;
  }

  // Every row carries exactly one destination action.
  for (let index = 0; index < rowCount; index += 1) {
    await expect(rows.nth(index).locator("[data-action-kind='destination']")).toHaveCount(1);
  }

  // Follow every row's declared destination (capped at 5 on a busy database):
  // re-open the panel for each row, read the target kind the backend shipped
  // (data-activity-target — the row never guesses), click its one action and
  // assert the screen it named.
  const toVerify = Math.min(rowCount, 5);
  for (let index = 0; index < toVerify; index += 1) {
    if (index > 0) {
      await indicator.click();
      await expect(dialog).toBeVisible({ timeout: 10_000 });
      await expect(rows.first()).toBeVisible({ timeout: 20_000 });
    }
    const row = rows.nth(index);
    const targetKind = await row.getAttribute("data-activity-target");
    const action = row.locator("[data-action-kind='destination']");
    const label = (await action.textContent())?.trim() ?? "";
    testInfo.annotations.push({ type: `row-${index}-target`, description: `${targetKind} · ${label}` });
    await action.click();
    await expect(dialog).toBeHidden();
    switch (targetKind) {
      case "company":
        await expect(page.getByRole("region", { name: /Widok spółki|Company view/ })).toBeVisible({ timeout: 15_000 });
        break;
      case "sources":
        await expect(page.getByRole("region", { name: /^(Źródła|Sources)$/ })).toBeVisible({ timeout: 15_000 });
        break;
      case "today":
        await expect(page.getByRole("region", { name: /^(Dziś|Today)$/ })).toBeVisible({ timeout: 15_000 });
        break;
      case "transcripts":
        await expect(page.getByRole("region", { name: /^(Transkrypcje|Transcripts)$/ })).toBeVisible({ timeout: 15_000 });
        break;
      default:
        throw new Error(`row ${index} declared an unknown target kind: ${targetKind}`);
    }
    if (index === 0) {
      await shoot(page, "after-open", testInfo);
    }
  }
});
