import { test, expect, openApp } from "./helpers/harness";
import { primeChaos } from "./helpers/mockRuntime";

// Epic #40 S1 (ADR 0091) — the chaos seam reaches the real browser runtime.
// `?chaos=<command>[:<code>]` installs a PERSISTENT failure rule before the app
// boots, so every invocation of that command settles with the ADR 0070 envelope
// a real typed backend rejection uses. The point of the assertion is that the
// failure is NAMED on screen: a broken read must not degrade into a silent
// empty list.
test.describe("chaos seam", { tag: "@clickable" }, () => {
  test("?chaos=list_companies renders a named failure, not an empty list", async ({ page }) => {
    await openApp(page, "/?chaos=list_companies");
    await page
      .getByLabel(/Primary navigation|Nawigacja główna/)
      .getByRole("button", { name: "Companies" })
      .click();

    const error = page.getByText(/Companies command failed/);
    await expect(error).toBeVisible();
    // The message names the command that failed — the operator can tell WHAT broke.
    await expect(error).toContainText("list_companies");
  });

  test("primeChaos installs the same persistent rule with a spec-chosen message", async ({ page }) => {
    await primeChaos(page, [
      { command: "list_companies", error: { code: "network", message: "company registry unreachable" } },
    ]);
    await openApp(page);
    await page
      .getByLabel(/Primary navigation|Nawigacja główna/)
      .getByRole("button", { name: "Companies" })
      .click();

    await expect(page.getByText(/Companies command failed/)).toContainText(
      "company registry unreachable",
    );
  });
});
