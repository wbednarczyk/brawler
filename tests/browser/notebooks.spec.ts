import { test, expect, openApp } from "./helpers/harness";

// Clickable Notebook lifecycle against the stateful browser mock runtime
// (ADR 0048): create a note, edit its title, then delete it — each step
// asserts the runtime reflected the write back into the note list. F4c S2
// (ADR 0108 amendment): the Notebooks-global screen retired — note work
// happens in the Spółka `notatnik` workshop tool (F3a S2/S3, ADR 0107),
// reached from the company's workshop bar.

function nav(page: Parameters<typeof openApp>[0]) {
  return page.getByLabel(/Primary navigation|Nawigacja główna/);
}

test.describe("notebooks", { tag: "@clickable" }, () => {
  test("create, edit, and delete a note for a company", async ({ page }) => {
    await openApp(page);

    await nav(page).getByRole("button", { name: "Companies" }).click();
    await page.getByRole("button", { name: "Open GPW:CDR" }).click();
    await expect(page.getByRole("region", { name: "Company view" })).toBeVisible();
    await page.getByRole("group", { name: "Workshop" }).getByRole("button", { name: "Notebook", exact: true }).click();

    const notebookPane = page.getByRole("group", { name: "Workshop tool" });
    await expect(notebookPane).toHaveAttribute("data-tool", "notatnik");
    const notebook = notebookPane.getByLabel("Company notebook");

    // Compose a new note — Save is gated on title + body.
    await notebook.getByRole("button", { name: "New note" }).click();
    await notebook.getByLabel("Notebook note title").fill("Margin watch Q3");
    await notebook
      .getByLabel("Notebook note body")
      .fill("Gross margin trend worth tracking into the next report.");
    await notebook.getByRole("button", { name: "Save" }).click();

    // The new note must appear in the list (stateful create). The panel
    // auto-selects it in view mode, so the detail editor is already open —
    // clicking the row again would toggle it back off. Presence (not
    // visibility) is asserted: at the S density tier the open detail hides the
    // list by contract (ADR 0076 D6); tier presentation is covered by the
    // density specs.
    await expect(notebook.getByLabel("Select notebook entry: Margin watch Q3")).toHaveCount(1);

    // Edit the title; Save is enabled once the form is dirty.
    await notebook.getByRole("button", { name: "Edit" }).click();
    await notebook.getByLabel("Selected notebook title").fill("Margin watch Q3 (rev)");
    await notebook.getByRole("button", { name: "Save" }).click();

    await expect(
      notebook.getByLabel("Select notebook entry: Margin watch Q3 (rev)"),
    ).toHaveCount(1);
    await expect(
      notebook.getByLabel("Select notebook entry: Margin watch Q3", { exact: true }),
    ).toHaveCount(0);

    // Delete — a reversible destroy (ADR 0076 D5): the note leaves immediately
    // with an undo toast, no native dialog. The edited note stays selected in
    // view mode, so Delete is already reachable.
    await notebook.getByRole("button", { name: "Delete" }).click();
    await expect(
      notebook.getByLabel("Select notebook entry: Margin watch Q3 (rev)"),
    ).toHaveCount(0);
  });
});
