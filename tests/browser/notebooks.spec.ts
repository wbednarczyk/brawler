import { test, expect, openApp, openCockpitPanel } from "./helpers/harness";

// Clickable Notebooks lifecycle against the stateful browser mock runtime
// (ADR 0048): create a note, edit its title, then delete it — each step
// asserts the runtime reflected the write back into the note list. The standalone
// Notebooks screen now lives as a cockpit panel (ADR 0054), opened via the palette.

test.describe("notebooks", { tag: "@clickable" }, () => {
  test("create, edit, and delete a note for a company", async ({ page }) => {
    await openApp(page);
    await openCockpitPanel(page, "Notebooks");

    // Scope generic-named buttons (New note / Save) to the notebook panel — the
    // cockpit hosts several panels, so an unscoped "Save" is ambiguous.
    const notebook = page.getByLabel("Notebooks workspace");

    // Pick a company so the composer can be opened.
    await page.getByLabel("Open notebook company: GPW:CDR").click();

    // Compose a new note — Save is gated on title + body.
    await notebook.getByRole("button", { name: "New note" }).click();
    await page.getByLabel("Notebook screen note title").fill("Margin watch Q3");
    await page
      .getByLabel("Notebook screen note body")
      .fill("Gross margin trend worth tracking into the next report.");
    await notebook.getByRole("button", { name: "Save" }).click();

    // The new note must appear in the list (stateful create). The controller
    // auto-selects it in view mode, so the detail editor is already open —
    // clicking the row again would toggle it back off. Presence (not
    // visibility) is asserted: at the S density tier the open detail hides the
    // list by contract (ADR 0076 D6); tier presentation is covered by the
    // density specs.
    await expect(page.getByLabel("Select notebook screen entry: Margin watch Q3")).toHaveCount(1);

    // Edit the title; Save is enabled once the form is dirty.
    const detail = page.getByLabel("Notebook screen entry detail");
    await detail.getByRole("button", { name: "Edit" }).click();
    await page.getByLabel("Notebook screen selected title").fill("Margin watch Q3 (rev)");
    await detail.getByRole("button", { name: "Save" }).click();

    await expect(
      page.getByLabel("Select notebook screen entry: Margin watch Q3 (rev)"),
    ).toHaveCount(1);
    await expect(
      page.getByLabel("Select notebook screen entry: Margin watch Q3", { exact: true }),
    ).toHaveCount(0);

    // Delete — a reversible destroy (ADR 0076 D5): the note leaves immediately
    // with an undo toast, no native dialog. The edited note stays selected in
    // view mode, so Delete is already reachable.
    await detail.getByRole("button", { name: "Delete" }).click();
    await expect(
      page.getByLabel("Select notebook screen entry: Margin watch Q3 (rev)"),
    ).toHaveCount(0);
  });
});
