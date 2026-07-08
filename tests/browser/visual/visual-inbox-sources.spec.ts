import { test, expect, openApp } from "../helpers/harness";
import { shootScreen } from "./helpers";
import type { Page } from "@playwright/test";

// Visual baseline — Inbox + Sources full screens (ADR 0076 D7 / U11), mirroring
// density-inbox-sources.spec.ts. Both are `.workspace`-hosted sidebar screens:
// the M-equivalent is the workspace at the project viewport, and the density spec
// forces `.workspace` at S and L — so the dark pass adds those forced tiers.

function nav(page: Page) {
  return page.getByLabel(/Primary navigation|Nawigacja główna/);
}

test.describe("visual — inbox + sources", () => {
  test("Inbox across pane tiers", async ({ page }) => {
    await openApp(page);
    await nav(page).getByRole("button", { name: "Inbox" }).click();
    await expect(page.getByRole("heading", { name: "Inbox", exact: true })).toBeVisible();
    await expect(page.locator(".feed-panel")).toBeVisible();
    await shootScreen(page, "inbox", { forced: ["S", "L"] });
  });

  test("Sources across pane tiers", async ({ page }) => {
    await openApp(page);
    await nav(page).getByRole("button", { name: "Sources" }).click();
    await expect(page.getByLabel("Source list")).toBeVisible();
    await shootScreen(page, "sources", { forced: ["S", "L"] });
  });
});
