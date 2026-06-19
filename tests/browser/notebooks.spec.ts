import { test, expect, openApp } from "./helpers/harness";
import type { Page } from "@playwright/test";

// Clickable Notebooks lifecycle against the stateful browser mock runtime
// (ADR 0048): create a note, edit its title, then delete it — each step
// asserts the runtime reflected the write back into the note list.

function navTo(page: Page, name: string) {
  return page.getByLabel("Primary navigation").getByRole("button", { name });
}

test.describe("notebooks", () => {
  test("create, edit, and delete a note for a company", async ({ page }) => {
    await openApp(page);
    await navTo(page, "Notebooks").click();

    // Pick a company so the composer can be opened.
    await page.getByLabel("Open notebook company: GPW:CDR").click();

    // Compose a new note — Save is gated on title + body.
    await page.getByRole("button", { name: "New note" }).click();
    await page.getByLabel("Notebook screen note title").fill("Margin watch Q3");
    await page
      .getByLabel("Notebook screen note body")
      .fill("Gross margin trend worth tracking into the next report.");
    await page.getByRole("button", { name: "Save" }).click();

    // The new note must appear in the list (stateful create). The controller
    // auto-selects it in view mode, so the detail editor is already open —
    // clicking the row again would toggle it back off.
    await expect(page.getByLabel("Select notebook screen entry: Margin watch Q3")).toBeVisible();

    // Edit the title; Save is enabled once the form is dirty.
    const detail = page.getByLabel("Notebook screen entry detail");
    await detail.getByRole("button", { name: "Edit" }).click();
    await page.getByLabel("Notebook screen selected title").fill("Margin watch Q3 (rev)");
    await detail.getByRole("button", { name: "Save" }).click();

    await expect(
      page.getByLabel("Select notebook screen entry: Margin watch Q3 (rev)"),
    ).toBeVisible();
    await expect(
      page.getByLabel("Select notebook screen entry: Margin watch Q3", { exact: true }),
    ).toHaveCount(0);

    // Delete (confirms via window.confirm) — the entry leaves the list. The
    // edited note stays selected in view mode, so Delete is already reachable.
    page.once("dialog", (dialog) => dialog.accept());
    await detail.getByRole("button", { name: "Delete" }).click();
    await expect(
      page.getByLabel("Select notebook screen entry: Margin watch Q3 (rev)"),
    ).toHaveCount(0);
  });
});
